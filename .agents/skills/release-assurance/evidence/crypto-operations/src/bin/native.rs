use crypto_bigint::{U256, Word};
use deos_0387_operations::*;
use hmac::{Hmac, Mac};
use k256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use p256::{elliptic_curve::Curve, pkcs8::EncodePrivateKey, NistP256};
use sha2::Sha256;

// Mirrors sc-network 0.60.0 src/webrtc.rs derive_keys, not its certificate/DTLS layer.
#[no_mangle]
#[inline(never)]
pub fn selected_webrtc_derive(node_key: &[u8; 32]) -> p256::ecdsa::SigningKey {
    (0u8..255u8).find_map(|counter| {
        let okm = Hmac::<Sha256>::new_from_slice(node_key)
            .expect("HMAC accepts 32 bytes") // deos-bypass: panic-owner — README.md fixture oracle.
            .chain_update(b"substrate-webrtc-p256-v1")
            .chain_update([counter]).finalize().into_bytes();
        let scalar = U256::from_be_slice(&okm);
        (scalar != U256::ZERO && scalar < NistP256::ORDER)
            .then(|| p256::ecdsa::SigningKey::from_slice(&okm).expect("range checked")) // deos-bypass: panic-owner — README.md fixture oracle.
    }).expect("derivation exhausted") // deos-bypass: panic-owner — README.md fixture oracle.
}

#[no_mangle]
#[inline(never)]
pub fn selected_p256_sign(key: &[u8; 32], hash: &[u8; 32]) -> [u8; 64] {
    let key = p256::ecdsa::SigningKey::from_bytes(key.into()).expect("valid P256 scalar"); // deos-bypass: panic-owner — README.md fixture oracle.
    let sig: p256::ecdsa::Signature = key.sign_prehash(hash).expect("P256 signing"); // deos-bypass: panic-owner — README.md fixture oracle.
    key.verifying_key().verify_prehash(hash, &sig).expect("P256 verification"); // deos-bypass: panic-owner — README.md fixture oracle.
    sig.to_bytes().into()
}

#[no_mangle]
#[inline(never)]
pub fn selected_k256_sign(key: &[u8; 32], hash: &[u8; 32]) -> [u8; 64] {
    let key = k256::ecdsa::SigningKey::from_bytes(key.into()).expect("valid K256 scalar"); // deos-bypass: panic-owner — README.md fixture oracle.
    let sig: k256::ecdsa::Signature = key.sign_prehash(hash).expect("K256 signing"); // deos-bypass: panic-owner — README.md fixture oracle.
    key.verifying_key().verify_prehash(hash, &sig).expect("K256 verification"); // deos-bypass: panic-owner — README.md fixture oracle.
    sig.to_bytes().into()
}

fn parse_hex(s: &str) -> Result<[u8; 32], &'static str> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("expected exactly 64 ASCII hex digits"); }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| "invalid hex")?;
    }
    Ok(out)
}
fn emit(label: &str, bytes: &[u8]) {
    print!("{label}=");
    for byte in bytes { print!("{byte:02x}"); }
    println!();
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 { return Err("usage: live-native KEY_64HEX HASH_64HEX (synthetic fixture material only)".into()); }
    let key = std::hint::black_box(parse_hex(&args[0])?);
    let hash = std::hint::black_box(parse_hex(&args[1])?);
    // Validate the complete common scalar domain, without reduction or seed truncation.
    let p = p256::ecdsa::SigningKey::from_slice(&key)?;
    let k = k256::ecdsa::SigningKey::from_slice(&key)?;
    let a = U256::from_be_slice(&key);
    let b = U256::from_be_slice(&hash);
    std::hint::black_box(selected_sub_mod(&a, &b, &U256::MAX));
    let limb = a.as_words()[0];
    std::hint::black_box(std::hint::black_box(selected_sbb as fn(Word, Word, Word) -> _)(limb, b.as_words()[0], 0));
    std::hint::black_box(std::hint::black_box(selected_limb_clz as fn(Word) -> usize)(limb));
    std::hint::black_box(std::hint::black_box(selected_limb_ctz as fn(Word) -> usize)(limb));
    std::hint::black_box(std::hint::black_box(selected_uint_ctz as fn(&U256) -> usize)(&a));
    let scalar_k = selected_k256_reduce(a);
    let scalar_p = selected_p256_reduce(a);
    std::hint::black_box(std::hint::black_box(selected_k256_neg as fn(k256::Scalar) -> _)(scalar_k));
    std::hint::black_box(std::hint::black_box(selected_p256_neg as fn(p256::Scalar) -> _)(scalar_p));
    // Actual K256 lincomb drives its scalar decomposition, not only a negation wrapper.
    use k256::elliptic_curve::ops::LinearCombination;
    std::hint::black_box(k256::ProjectivePoint::lincomb(
        &k256::ProjectivePoint::GENERATOR, &scalar_k,
        &k256::ProjectivePoint::GENERATOR, &selected_k256_reduce(b)));
    let ps = std::hint::black_box(selected_p256_sign as fn(&[u8; 32], &[u8; 32]) -> [u8; 64])(&key, &hash);
    let ks = std::hint::black_box(selected_k256_sign as fn(&[u8; 32], &[u8; 32]) -> [u8; 64])(&key, &hash);
    emit("p256.pub", p.verifying_key().to_encoded_point(false).as_bytes());
    emit("p256.sig", &ps);
    emit("p256.der", p256::ecdsa::Signature::from_slice(&ps)?.to_der().as_bytes());
    emit("k256.pub", k.verifying_key().to_encoded_point(false).as_bytes());
    emit("k256.sig", &ks);
    emit("k256.der", k256::ecdsa::Signature::from_slice(&ks)?.to_der().as_bytes());
    let derived = std::hint::black_box(selected_webrtc_derive as fn(&[u8; 32]) -> p256::ecdsa::SigningKey)(&key);
    let sig: p256::ecdsa::Signature = derived.sign_prehash(&hash)?;
    let der: p256::ecdsa::DerSignature = derived.sign_prehash(&hash)?;
    assert_eq!(der.as_bytes(), sig.to_der().as_bytes());
    derived.verifying_key().verify_prehash(&hash, &sig)?;
    emit("webrtc.pub", derived.verifying_key().to_encoded_point(false).as_bytes());
    emit("webrtc.sig", sig.to_bytes().as_slice());
    emit("webrtc.der", der.as_bytes());
    // PKCS8 output is synthetic key material, used only for baseline byte comparison.
    emit("webrtc.pkcs8", derived.to_pkcs8_der()?.as_bytes());
    Ok(())
}
fn main() {
    if let Err(error) = run() { eprintln!("fixture rejected: {error}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_width_input_domain() {
        assert!(parse_hex("1").is_err());
        assert!(parse_hex(&"gg".repeat(32)).is_err());
        assert!(parse_hex(&"é".repeat(32)).is_err());
        for pair in 0..32 {
            let mut input = "01".repeat(32);
            input.replace_range(pair * 2..pair * 2 + 2, "+1");
            // Both CLI arguments use this parser; reject a sign in every pair.
            assert!(parse_hex(&input).is_err());
        }
        let top = parse_hex(&format!("80{}01", "00".repeat(30))).unwrap(); // deos-bypass: panic-owner — README.md fixture test oracle.
        assert_eq!(top[0], 128);
        assert_eq!(top[31], 1);
        assert!(p256::ecdsa::SigningKey::from_slice(&[0; 32]).is_err());
        assert!(p256::ecdsa::SigningKey::from_slice(&[255; 32]).is_err());
        assert!(k256::ecdsa::SigningKey::from_slice(&[0; 32]).is_err());
        assert!(k256::ecdsa::SigningKey::from_slice(&[255; 32]).is_err());
    }
}
