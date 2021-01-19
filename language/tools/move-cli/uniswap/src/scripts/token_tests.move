script {
use 0x1::Debug;
use 0x2::Token;
use 0x2::Tokens;
fun main(registrar: &signer) {
    Token::register_token<Tokens::A>(registrar, b"A");
    Token::register_token<Tokens::B>(registrar, b"B");
    Token::register_token<Tokens::C>(registrar, b"C");
    Token::register_token<Tokens::D>(registrar, b"D");
    Token::register_token<Tokens::E>(registrar, b"E");
    Token::register_token<Tokens::F>(registrar, b"F");
    Token::register_token<Tokens::G>(registrar, b"G");

    Debug::print(&Token::code<Tokens::A>());
    Debug::print(&Token::code<Tokens::B>());
    Debug::print(&Token::code<Tokens::C>());
    Debug::print(&Token::code<Tokens::D>());
    Debug::print(&Token::code<Tokens::E>());
    Debug::print(&Token::code<Tokens::F>());
    Debug::print(&Token::code<Tokens::G>());
}
}
