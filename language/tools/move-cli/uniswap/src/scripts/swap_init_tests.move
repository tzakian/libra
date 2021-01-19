script {
use 0x2::Uniswap;
use 0x2::Tokens;
use 0x2::Token;
use 0x1::Debug;
use 0x1::FixedPoint32;

fun main(account: &signer) {
    let tokensa = Token::create<Tokens::C>(1200);
    let tokensb = Token::create<Tokens::D>(400);
    let lp_token = Uniswap::initialize<Tokens::C, Tokens::D>(account, tokensa, tokensb);
    // This should create a pool where 3 of D is equal to 1 of C, and where 1/3 of a C is equal to 1 D
    Debug::print(&lp_token);
    Uniswap::dtor(lp_token);

    let (d_xchng, c_xchng) = Uniswap::calculate_current_exchange_rate<Tokens::C, Tokens::D>();
    Debug::print(&d_xchng);
    Debug::print(&c_xchng);
    Debug::print(&FixedPoint32::multiply_u64(10, d_xchng));
    Debug::print(&FixedPoint32::multiply_u64(10, c_xchng));
}
}
