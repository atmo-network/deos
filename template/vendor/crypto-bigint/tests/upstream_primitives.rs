use crypto_bigint::{Limb, Uint, Word, U256};

#[test]
fn upstream_borrow_mask_boundary_contract() {
    let msb = 1 << (Word::BITS - 1);
    let values = [0, 1, 2, msb - 1, msb, msb + 1, Word::MAX - 1, Word::MAX];
    for lhs in values {
        for rhs in values {
            for incoming in [0, 1, msb, Word::MAX] {
                let (a, b1) = lhs.overflowing_sub(incoming >> (Word::BITS - 1));
                let (expected, b2) = a.overflowing_sub(rhs);
                assert_eq!(Limb(lhs).sbb(Limb(rhs), Limb(incoming)),
                    (Limb(expected), Limb(Word::from(b1 | b2).wrapping_neg())));
            }
        }
    }
}

#[test]
fn upstream_counts_and_mask_composition() {
    assert_eq!(U256::ZERO.leading_zeros(), 256);
    assert_eq!(U256::ZERO.trailing_zeros(), 256);
    assert_eq!(U256::MAX.trailing_ones(), 256);
    for bit in 0..256 {
        let x = U256::ONE.shl_vartime(bit);
        assert_eq!(x.leading_zeros(), 255 - bit);
        assert_eq!(x.trailing_zeros(), bit);
        assert_eq!(x.wrapping_sub(&U256::ONE).trailing_ones(), bit);
        assert_eq!(x.neg_mod(&U256::MAX), U256::MAX.wrapping_sub(&x));
    }
    assert_eq!(U256::ZERO.neg_mod(&U256::MAX), U256::ZERO);
    let _: Uint<1> = Uint::ONE;
}
