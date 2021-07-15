module 0x1::A {
    use 0x1::B;
    use 0x1::D;
    public fun a() {
        B::b();
        D::d()
    }
}
