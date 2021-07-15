module 0x1::B {
    use 0x1::C;
    public fun b() { C::c() }
}
