use super::Limb;
#[cfg(target_pointer_width = "32")]
use ctutils::primitives::{leading_zeros_u32 as leading_zeros, trailing_zeros_u32 as trailing_zeros};
#[cfg(target_pointer_width = "64")]
use ctutils::primitives::{leading_zeros_u64 as leading_zeros, trailing_zeros_u64 as trailing_zeros};

impl Limb {
    /// Calculate the number of bits needed to represent this number.
    pub const fn bits(self) -> usize { Limb::BITS - self.leading_zeros() }
    /// Calculate the number of leading zeros in the binary representation of this number.
    pub const fn leading_zeros(self) -> usize { leading_zeros(self.0) as usize }
    /// Calculate the number of trailing zeros in the binary representation of this number.
    pub const fn trailing_zeros(self) -> usize { trailing_zeros(self.0) as usize }
    /// Calculate the number of trailing ones in the binary representation of this number.
    pub const fn trailing_ones(self) -> usize { trailing_zeros(!self.0) as usize }
}
