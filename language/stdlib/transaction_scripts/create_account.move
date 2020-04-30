use 0x0::LibraAccount;

// TODO: remove initial_amount?
fun main<Token>(fresh_address: address, auth_key_prefix: vector<u8>, initial_amount: u64) {
  if (!LibraAccount::exists(fresh_address))
      LibraAccount::create_root_vasp_account<Token>(
          fresh_address,
          auth_key_prefix,
          // "testnet"
          x"746573746E6574",
          // "https://libra.org"
          x"68747470733A2F2F6C696272612E6F72672F",
          x"",
  );
  if (initial_amount > 0) LibraAccount::deposit(
        fresh_address,
        LibraAccount::withdraw_from_sender<Token>(initial_amount)
     );
}
