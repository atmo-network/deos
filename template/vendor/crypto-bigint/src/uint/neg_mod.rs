//! [`Uint`] negation modulus operations.

use crate::{Limb, NegMod, Uint};

impl<const LIMBS: usize> Uint<LIMBS> {
    /// Computes `-a mod p`.
    /// Assumes `self` is in `[0, p)`.
    pub const fn neg_mod(&self, p: &Self) -> Self {
        let z = self.ct_is_nonzero();
        let mut ret = p.sbb(self, Limb::ZERO).0;
        let mut i = 0;
        while i < LIMBS {
            // Set ret to 0 if the original value was 0, in which
            // case ret would be p.
            ret.limbs[i].0 = z.if_true(ret.limbs[i].0);
            i += 1;
        }
        ret
    }

    /// Computes `-a mod p` for the special modulus
    /// `p = MAX+1-c` where `c` is small enough to fit in a single [`Limb`].
    pub const fn neg_mod_special(&self, c: Limb) -> Self {
        Self::ZERO.sub_mod_special(self, c)
    }
}

impl<const LIMBS: usize> NegMod for Uint<LIMBS> {
    type Output = Self;

    fn neg_mod(&self, p: &Self) -> Self {
        debug_assert!(self < p);
        self.neg_mod(p)
    }
}

#[cfg(test)]
mod tests {
    use crate::U256;

    #[test]
    fn neg_mod_boundaries_and_const_evaluation() {
        const ZERO: U256 = U256::ZERO.neg_mod(&U256::MAX);
        const ONE: U256 = U256::ONE.neg_mod(&U256::MAX);
        assert_eq!(ZERO, U256::ZERO);
        assert_eq!(ONE, U256::MAX.wrapping_sub(&U256::ONE));
        let p = U256::MAX;
        for a in [
            U256::ZERO,
            U256::ONE,
            U256::from_u64(u64::MAX),
            U256::ONE.shl_vartime(128),
            p.wrapping_sub(&U256::ONE),
        ] {
            let neg = a.neg_mod(&p);
            assert_eq!(a.add_mod(&neg, &p), U256::ZERO);
            assert_eq!(neg.neg_mod(&p), a);
            if a != U256::ZERO {
                assert_eq!(neg, p.wrapping_sub(&a));
            }
        }
    }

    #[test]
    fn neg_mod_random() {
        let x =
            U256::from_be_hex("8d16e171674b4e6d8529edba4593802bf30b8cb161dd30aa8e550d41380007c2");
        let p =
            U256::from_be_hex("928334a4e4be0843ec225a4c9c61df34bdc7a81513e4b6f76f2bfa3148e2e1b5");

        let actual = x.neg_mod(&p);
        let expected =
            U256::from_be_hex("056c53337d72b9d666f86c9256ce5f08cabc1b63b207864ce0d6ecf010e2d9f3");

        assert_eq!(expected, actual);
    }

    #[test]
    fn neg_mod_zero() {
        let x =
            U256::from_be_hex("0000000000000000000000000000000000000000000000000000000000000000");
        let p =
            U256::from_be_hex("928334a4e4be0843ec225a4c9c61df34bdc7a81513e4b6f76f2bfa3148e2e1b5");

        let actual = x.neg_mod(&p);
        let expected =
            U256::from_be_hex("0000000000000000000000000000000000000000000000000000000000000000");

        assert_eq!(expected, actual);
    }
}
