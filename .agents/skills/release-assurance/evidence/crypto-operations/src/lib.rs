#![no_std]
use crypto_bigint::{Limb, U256, Word};
use p256::elliptic_curve::{ops::Reduce, scalar::IsHigh};
#[no_mangle]
pub fn selected_sbb(a: Word, b: Word, borrow: Word) -> (Limb, Limb) { Limb(a).sbb(Limb(b), Limb(borrow)) }
#[no_mangle]
pub fn selected_sub_mod(a: &U256, b: &U256, p: &U256) -> U256 { a.sub_mod(b, p) }
#[no_mangle]
pub fn selected_u256_sbb(a: &U256, b: &U256) -> (U256, Limb) { a.sbb(b, Limb::ZERO) }
#[no_mangle]
pub fn selected_limb_ctz(a: Word) -> usize { Limb(a).trailing_zeros() }
#[no_mangle]
pub fn selected_limb_clz(a: Word) -> usize { Limb(a).leading_zeros() }
#[no_mangle]
pub fn selected_uint_ctz(a: &U256) -> usize { a.trailing_zeros() }
#[no_mangle]
pub fn selected_p256_sub(a: &p256::Scalar, b: &p256::Scalar) -> p256::Scalar { a.sub(b) }
#[no_mangle]
pub fn selected_k256_sub(a: &k256::Scalar, b: &k256::Scalar) -> k256::Scalar { a.sub(b) }
#[no_mangle]
pub fn selected_p256_reduce(a: U256) -> p256::Scalar { <p256::Scalar as Reduce<U256>>::reduce(a) }
#[no_mangle]
pub fn selected_k256_reduce(a: U256) -> k256::Scalar { <k256::Scalar as Reduce<U256>>::reduce(a) }
#[no_mangle]
pub fn selected_p256_neg(a: p256::Scalar) -> p256::Scalar { -a }
#[no_mangle]
pub fn selected_k256_neg(a: k256::Scalar) -> k256::Scalar { -a }
#[no_mangle]
pub fn selected_p256_high(a: p256::Scalar) -> u8 { a.is_high().unwrap_u8() }
#[no_mangle]
pub fn selected_k256_high(a: k256::Scalar) -> u8 { a.is_high().unwrap_u8() }
#[no_mangle]
pub fn selected_eq(a: &U256, b: &U256) -> bool { a == b }

