use 0x0::LibraAccount;
fun main<Token>(
    fresh_address: address,
    auth_key_prefix: vector<u8>,
    human_name: vector<u8>,
    base_url: vector<u8>,
    ca_cert: vector<u8>,
) {
    LibraAccount::create_root_vasp_account<Token>(
        fresh_address,
        auth_key_prefix,
        human_name,
        base_url,
        ca_cert,
    )
}
