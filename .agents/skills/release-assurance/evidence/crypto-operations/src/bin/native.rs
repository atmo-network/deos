use deos_0387_operations::*;
use crypto_bigint::{U256, Word};
use k256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
fn main() {
    let n: u64 = std::env::args().nth(1).expect("numeric input").parse().unwrap(); // deos-bypass: panic-owner — README.md native acceptance fixture; invalid input invalidates the run.
    let a = U256::from_u64(n);
    let b = U256::from_u64(n.rotate_left(19));
    let p = U256::MAX;
    std::hint::black_box(selected_sub_mod(&a, &b, &p));
    std::hint::black_box(selected_sbb(n as Word, n.rotate_left(19) as Word, 0));
    let negate = std::hint::black_box(selected_k256_neg as fn(k256::Scalar) -> k256::Scalar);
    std::hint::black_box(negate(k256::Scalar::from(n)));
    std::hint::black_box(selected_p256_neg(p256::Scalar::from(n)));
    let mut key = [0u8; 32];
    key[24..].copy_from_slice(&(n | 1).to_be_bytes());
    let hash = std::hint::black_box([n as u8; 32]);
    let k = k256::ecdsa::SigningKey::from_bytes((&key).into()).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; key rejection invalidates the run.
    let sig: k256::ecdsa::Signature = k.sign_prehash(&hash).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; signing failure invalidates the run.
    k.verifying_key().verify_prehash(&hash, &sig).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; verification failure invalidates the run.
    let p = p256::ecdsa::SigningKey::from_bytes((&key).into()).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; key rejection invalidates the run.
    let sig: p256::ecdsa::Signature = p.sign_prehash(&hash).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; signing failure invalidates the run.
    p.verifying_key().verify_prehash(&hash, &sig).unwrap(); // deos-bypass: panic-owner — README.md native fixture oracle; verification failure invalidates the run.
    println!("live arithmetic and both prehash sign/verify paths passed");
}
