#![no_std]
#![no_main]
use deos_0387_operations::*;
use crypto_bigint::U256;
use k256::ecdsa::signature::hazmat::PrehashSigner;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! { loop { core::hint::spin_loop(); } }
#[no_mangle]
pub fn live_neg(a: &U256) -> k256::Scalar { selected_k256_neg(selected_k256_reduce(*a)) }
#[no_mangle]
pub fn live_lincomb(a: &k256::Scalar, b: &k256::Scalar) -> k256::ProjectivePoint {
    use k256::elliptic_curve::ops::LinearCombination;
    k256::ProjectivePoint::lincomb(&k256::ProjectivePoint::GENERATOR, a, &k256::ProjectivePoint::GENERATOR, b)
}
#[no_mangle]
pub fn live_sign(key: &[u8; 32], hash: &[u8; 32], out: &mut [u8; 64]) -> bool {
    if let Ok(k) = k256::ecdsa::SigningKey::from_bytes(key.into()) {
        let sig: Result<k256::ecdsa::Signature, _> = k.sign_prehash(hash);
        if let Ok(sig) = sig { out.copy_from_slice(&sig.to_bytes()); return true; }
    }
    false
}
