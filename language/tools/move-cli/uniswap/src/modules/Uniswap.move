address 0x2 {

module Tokens {
    resource struct A {}
    resource struct B {}
    resource struct C {}
    resource struct D {}
    resource struct E {}
    resource struct F {}
    resource struct G {}
}

module Token {
    use 0x1::Signer;

    resource struct TokenRegistration<Token> {
        code: vector<u8>,
    }

    resource struct Token<A> {
        value: u64,
    }

    const EALREADY_EXISTS: u64 = 0;
    const EBAD_ADDR: u64 = 1;

    const REGISTRATION_ADDRESS: address = 0x2;

    public fun register_token<Token>(registration_signer: &signer, code: vector<u8>) {
        let registration_addr = Signer::address_of(registration_signer);
        assert(registration_addr == REGISTRATION_ADDRESS, EBAD_ADDR);
        assert(!exists<TokenRegistration<Token>>(registration_addr), EALREADY_EXISTS);
        move_to(registration_signer, TokenRegistration<Token> { code })
    }

    public fun code<Token>(): vector<u8>
    acquires TokenRegistration {
        assert(exists<TokenRegistration<Token>>(REGISTRATION_ADDRESS), EALREADY_EXISTS);
        *&borrow_global<TokenRegistration<Token>>(REGISTRATION_ADDRESS).code
    }

    public fun create<A>(value: u64): Token<A> {
        Token { value }
    }

    public fun value<A>(tok: &Token<A>): u64 {
        tok.value
    }

    public fun destroy<A>(tok: Token<A>) {
        Token { value: _ } = tok;
    }

    public fun withdraw<A>(x: &mut Token<A>, amount: u64): Token<A> {
        x.value = x.value - amount;
        Token<A> { value: amount }
    }

    public fun deposit<A>(x: &mut Token<A>, other: Token<A>) {
        let Token { value } = other;
        x.value = x.value + value;
    }
}

module PoolMap {
    use 0x2::Token;
    use 0x1::Option::{Self, Option};
    use 0x1::Signer;
    use 0x1::Vector;

    struct TupleMap {
        tok0: vector<u8>,
        tok1: vector<u8>,
        addr: address,
    }

    resource struct PoolMap {
        pool_map: vector<TupleMap>,
    }

    const EPOOL_ALREADY_REGISTERED: u64 = 0;

    const POOL_ROOT_ADDR: address = 0x2;

    public fun init(pool_root: &signer) {
        assert(Signer::address_of(pool_root) == POOL_ROOT_ADDR, 0);
        move_to(pool_root, PoolMap {
            pool_map: Vector::empty(),
        })
    }

    /// Get the address for the pool containing a `Token0` to `Token1` LP
    public fun get_pool_address<Token0, Token1>(): address
    acquires PoolMap {
        let tok0 = Token::code<Token0>();
        let tok1 = Token::code<Token1>();

        let pool_map = borrow_global<PoolMap>(POOL_ROOT_ADDR);
        let addr = find_pool_address(tok0, tok1, pool_map);
        Option::extract(&mut addr)
    }

    public fun register_pool_address<Token0, Token1>(pool_address: address)
    acquires PoolMap {
        let tok0 = Token::code<Token0>();
        let tok1 = Token::code<Token1>();
        let pool_map = borrow_global_mut<PoolMap>(POOL_ROOT_ADDR);

        // make sure the pool isn't already registered
        let addr = find_pool_address(copy tok0, copy tok1, pool_map);
        assert(Option::is_none(&addr), EPOOL_ALREADY_REGISTERED);

        Vector::push_back(&mut pool_map.pool_map, TupleMap {
            tok0,
            tok1,
            addr: pool_address
        });
    }

    fun find_pool_address(tok0: vector<u8>, tok1: vector<u8>, pool_map: &PoolMap): Option<address> {
        let len = Vector::length(&pool_map.pool_map);
        let i = 0;
        let ret = Option::none();

        while (i < len) {
            let elem = Vector::borrow(&pool_map.pool_map, i);
            if (&elem.tok0 == &tok0 && &elem.tok1 == &tok1) {
                ret = Option::some(elem.addr);
                break
            };
            i = i + 1;
        };

        ret
    }
}

module Uniswap {
    use 0x2::PoolMap;
    use 0x2::Token::{Self, Token};
    use 0x1::Signer;
    use 0x1::FixedPoint32::{Self, FixedPoint32};
    use 0x1::Compare;

    // Question: How to do order-agnostic type-indexed pairs?
    // Question: How to represent bi-directional swaps without code duplication?

    const LIQUIDITY_RATE: u64 = 4;

    resource struct Pool<Token0, Token1> {
        reserve0: Token<Token0>,
        reserve1: Token<Token1>,
        // reserve0.value * reserve1.value
        k_last: u64,
        last_timestamp: u64,
    }

    // Question: How best to compute the number of LPs to return?
    // Struct for now so we don't need to worry about storing it at the moment
    struct LiquidityToken<Token0, Token1> {
        shares: u64,
    }

    public fun dtor<A, B>(x: LiquidityToken<A, B>) {
        LiquidityToken { shares: _ } = x;
    }

    fun is_ordered<A, B>(): bool {
        Compare::cmp_bcs_bytes(&Token::code<A>(), &Token::code<B>()) == 1
    }

    fun ensure_ordering<A, B>() {
        assert(is_ordered<A, B>(), 0);
    }

    public fun initialize<Token0, Token1>(
        account: &signer,
        reserve0: Token<Token0>,
        reserve1: Token<Token1>
    ): LiquidityToken<Token0, Token1> {
        let val0 = Token::value(&reserve0);
        let val1 = Token::value(&reserve1);
        assert(val0 > 0, 0);
        assert(val1 > 0, 1);
        ensure_ordering<Token0, Token1>();
        PoolMap::register_pool_address<Token0, Token1>(Signer::address_of(account));
        move_to(account, Pool {
            reserve0,
            reserve1,
            k_last: val0 * val1,
            last_timestamp: 0
        });
        LiquidityToken<Token0, Token1> { shares: LIQUIDITY_RATE * val0 }
    }


    /// > Question: Should the exchange rate be calculated against the
    /// pre-withdrawal amount, or the post-withdrawal amount?
    public fun calculate_current_exchange_rate<A, B>(): (FixedPoint32, FixedPoint32)
    acquires Pool {
        ensure_ordering<A, B>();
        let pool_address = PoolMap::get_pool_address<A, B>();
        let pool = borrow_global<Pool<A, B>>(pool_address);

        let x_amount = Token::value(&pool.reserve0);
        let y_amount = Token::value(&pool.reserve1);

        // x * y = k ==> y = k/x, x = k/y

        let x_exchange_rate = FixedPoint32::create_from_rational(x_amount, y_amount);
        let y_exchange_rate = FixedPoint32::create_from_rational(y_amount, x_amount);

        (/* x = */ x_exchange_rate,  /* y = */ y_exchange_rate)
    }

    /// Withdraw enough from `swapping_out` to satisfy the request for `amount_to_swap_into` of `swapping_in` tokens
    /// TODO: Need to case on the type, and swap/change logic based on if the types are in order or not.
    public fun get_amount_out<A, B>(swapping_in: &mut Token<A>, swapping_out: &mut Token<B>, amount_to_swap_into: u64)
    acquires Pool {
        // Need to make sure the type arguments have the right order
        ensure_ordering<A, B>();
        let (x_exchange_rate, _y_exchange_rate) = calculate_current_exchange_rate<A, B>();
        let amount_to_withdraw = FixedPoint32::multiply_u64(amount_to_swap_into, x_exchange_rate);

        let pool_address = PoolMap::get_pool_address<A, B>();
        let pool = borrow_global_mut<Pool<A, B>>(pool_address);

        // Deposit into the pool
        let tokens_to_deposit = Token::withdraw(swapping_out, amount_to_withdraw);
        Token::deposit(&mut pool.reserve1, tokens_to_deposit);

        let additional_tokens_to_swap_into = Token::withdraw(&mut pool.reserve0, amount_to_swap_into);
        Token::deposit(swapping_in, additional_tokens_to_swap_into);
    }

    /// Swap into as many tokens as possible for the value of `amount_to_swap_out` of `swapping_out` tokens
    public fun get_amount_in<A, B>(swapping_in: &mut Token<A>, swapping_out: &mut Token<B>, amount_to_swap_out: u64)
    acquires Pool {
        ensure_ordering<A, B>();
        let (_x_exchange_rate, y_exchange_rate) = calculate_current_exchange_rate<A, B>();
        let amount_to_xchng = FixedPoint32::multiply_u64(amount_to_swap_out, y_exchange_rate);

        let pool_address = PoolMap::get_pool_address<A, B>();
        let pool = borrow_global_mut<Pool<A, B>>(pool_address);

        let tokens_to_deposit = Token::withdraw(swapping_out, amount_to_swap_out);
        Token::deposit(&mut pool.reserve1, tokens_to_deposit);

        let additional_tokens_to_swap_into = Token::withdraw(&mut pool.reserve0, amount_to_xchng);
        Token::deposit(swapping_in, additional_tokens_to_swap_into);
    }
}


}
