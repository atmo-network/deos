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

#[cfg(test)]
mod tests {
    use super::*;
    const CONST_ZERO: U256 = U256::ZERO.neg_mod(&U256::MAX);
    const CONST_ONE: U256 = U256::ONE.neg_mod(&U256::MAX);
    #[test]
    fn const_and_modular_negation_boundaries() {
        assert_eq!(CONST_ZERO, U256::ZERO);
        assert_eq!(CONST_ONE, U256::MAX.wrapping_sub(&U256::ONE));
        let p = U256::MAX;
        for a in [U256::ZERO, U256::ONE, U256::from_u64(u64::MAX), U256::ONE.shl_vartime(128), p.wrapping_sub(&U256::ONE)] {
            let neg = a.neg_mod(&p);
            assert_eq!(a.add_mod(&neg, &p), U256::ZERO);
            assert_eq!(neg.neg_mod(&p), a);
            if a != U256::ZERO { assert_eq!(neg, p.wrapping_sub(&a)); }
        }
    }
    #[test]
    fn borrow_and_carry_boundaries() {
        for a in [0, 1, Word::MAX / 2, Word::MAX / 2 + 1, Word::MAX] {
            for b in [0, 1, Word::MAX / 2, Word::MAX / 2 + 1, Word::MAX] {
                for borrow in [0, 1, Word::MAX / 2, Word::MAX / 2 + 1, Word::MAX - 1, Word::MAX] {
                    let (x, b1) = a.overflowing_sub(b);
                    // The public API consumes only the incoming mask's most significant bit.
                    let (x, b2) = x.overflowing_sub(borrow >> (Word::BITS - 1));
                    assert_eq!(selected_sbb(a, b, borrow), (Limb(x), Limb(if b1 || b2 { Word::MAX } else { 0 })));
                }
            }
        }
    }
    #[test]
    fn scalar_negation_and_group_boundaries() {
        use k256::elliptic_curve::ops::LinearCombination;
        for k in [k256::Scalar::ZERO, k256::Scalar::ONE, -k256::Scalar::ONE, k256::Scalar::from(u64::MAX)] {
            assert_eq!(k + selected_k256_neg(k), k256::Scalar::ZERO);
            let g = k256::ProjectivePoint::GENERATOR;
            assert_eq!(k256::ProjectivePoint::lincomb(&g, &k, &g, &(-k)), k256::ProjectivePoint::IDENTITY);
        }
        for p in [p256::Scalar::ZERO, p256::Scalar::ONE, -p256::Scalar::ONE, p256::Scalar::from(u64::MAX)] {
            assert_eq!(p + selected_p256_neg(p), p256::Scalar::ZERO);
        }
    }
}

