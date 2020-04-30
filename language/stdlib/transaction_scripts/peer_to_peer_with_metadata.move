use 0x0::LibraAccount;

fun main<Token>(payee: address, auth_key_prefix: vector<u8>, amount: u64, metadata: vector<u8>) {
  if (!LibraAccount::exists(payee))
      LibraAccount::create_root_vasp_account<Token>(
          payee,
          auth_key_prefix,
          // "testnet"
          x"746573746E6574",
          // "https://libra.org"
          x"68747470733A2F2F6C696272612E6F72672F",
          x"",
  );
  LibraAccount::pay_from_sender_with_metadata<Token>(payee, amount, metadata)
}
