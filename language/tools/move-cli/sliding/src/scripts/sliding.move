script {
use 0x1::SlidingNonce;
fun main(account: &signer) {
    SlidingNonce::publish(account);

    SlidingNonce::record_nonce_or_abort(account, 1);
    SlidingNonce::record_nonce_or_abort(account, 2);
    /*SlidingNonce::record_nonce_or_abort(account, 129);*/
    SlidingNonce::record_nonce_or_abort(account, 10000);
}
}
