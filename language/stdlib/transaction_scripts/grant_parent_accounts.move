use 0x0::VASP;
fun main(root_vasp_addr: address) {
    VASP::grant_parent_accounts(root_vasp_addr)
}
