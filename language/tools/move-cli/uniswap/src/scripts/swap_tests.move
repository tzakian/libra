script {
use 0x2::Token;
use 0x2::Tokens;
use 0x2::Uniswap;

use 0x1::Debug;
use 0x1::FixedPoint32;
fun main() {
    let tokensc = Token::create<Tokens::C>(1000);
    let tokensd = Token::create<Tokens::D>(1000);

    // print out the current exchange rates
    let (d_xchng, c_xchng) = Uniswap::calculate_current_exchange_rate<Tokens::C, Tokens::D>();
    // multiply by 100 to see the a couple decimal places
    Debug::print(&FixedPoint32::multiply_u64(100, d_xchng));
    Debug::print(&FixedPoint32::multiply_u64(100, c_xchng));

    // If I swap into 100 of C tokens, I need to deposit 300 D tokens
    Uniswap::get_amount_out(&mut tokensc, &mut tokensd, 100);
    // This should have increased by 100 (= 1100)
    Debug::print(&tokensc);
    // This should have decreased by 300 (= 700)
    Debug::print(&tokensd);

    // print out the updated exchange rates
    let (d_xchng, c_xchng) = Uniswap::calculate_current_exchange_rate<Tokens::C, Tokens::D>();
    // multiply by 100 to see the a couple decimal places
    Debug::print(&FixedPoint32::multiply_u64(100, d_xchng));
    Debug::print(&FixedPoint32::multiply_u64(100, c_xchng));

    // Now lets try again. The exchange rate should be different now!
    Uniswap::get_amount_out(&mut tokensc, &mut tokensd, 100);
    // This should have increased by 100 (= 1200)
    Debug::print(&tokensc);
    // This should have decreased by 157 ( = 543)
    Debug::print(&tokensd);

    // print out the updated exchange rates
    let (d_xchng, c_xchng) = Uniswap::calculate_current_exchange_rate<Tokens::C, Tokens::D>();
    // multiply by 100 to see the a couple decimal places
    Debug::print(&FixedPoint32::multiply_u64(100, d_xchng));
    Debug::print(&FixedPoint32::multiply_u64(100, c_xchng));

    Uniswap::get_amount_in(&mut tokensc, &mut tokensd, 117);
    // This should have increased by 100 (= 1300)
    Debug::print(&tokensc);
    // This should have decreased by 117 ( = 426)
    Debug::print(&tokensd);

    // print out the updated exchange rates
    let (d_xchng, c_xchng) = Uniswap::calculate_current_exchange_rate<Tokens::C, Tokens::D>();
    // multiply by 100 to see the a couple decimal places
    Debug::print(&FixedPoint32::multiply_u64(100, d_xchng));
    Debug::print(&FixedPoint32::multiply_u64(100, c_xchng));

    Token::destroy(tokensc);
    Token::destroy(tokensd);
}
}
