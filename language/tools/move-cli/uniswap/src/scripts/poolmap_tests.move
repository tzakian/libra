script {
use 0x1::Debug;
use 0x2::PoolMap;
use 0x2::Tokens;

fun main(registrar: &signer) {
    PoolMap::init(registrar);
    PoolMap::register_pool_address<Tokens::A, Tokens::B>(0x3);
    PoolMap::register_pool_address<Tokens::A, Tokens::C>(0x3);
    PoolMap::register_pool_address<Tokens::A, Tokens::D>(0x3);

    PoolMap::register_pool_address<Tokens::B, Tokens::C>(0x4);
    PoolMap::register_pool_address<Tokens::B, Tokens::D>(0x4);

    Debug::print(&PoolMap::get_pool_address<Tokens::A, Tokens::B>());
    Debug::print(&PoolMap::get_pool_address<Tokens::A, Tokens::C>());
    Debug::print(&PoolMap::get_pool_address<Tokens::A, Tokens::D>());

    Debug::print(&PoolMap::get_pool_address<Tokens::B, Tokens::C>());
    Debug::print(&PoolMap::get_pool_address<Tokens::B, Tokens::D>());
}
}
