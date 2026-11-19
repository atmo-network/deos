use subtle::Choice;
use crate::Word;

/// A boolean value returned by constant-time `const fn`s.
#[derive(Debug, Copy, Clone)]
pub struct CtChoice(ctutils::Choice);

impl CtChoice {
    /// The falsy value.
    pub const FALSE: Self = Self(ctutils::Choice::FALSE);
    /// The truthy value.
    pub const TRUE: Self = Self(ctutils::Choice::TRUE);
    pub(crate) const fn from_mask(value: Word) -> Self {
        debug_assert!(value == 0 || value == Word::MAX);
        Self::from_lsb(value & 1)
    }
    pub(crate) const fn from_usize_equality(x: usize, y: usize) -> Self {
        Self::from_usize_being_nonzero(x ^ y).not()
    }
    pub(crate) const fn not(&self) -> Self { Self(self.0.not()) }
    pub(crate) const fn or(&self, other: Self) -> Self { Self(self.0.or(other.0)) }
    pub(crate) const fn and(&self, other: Self) -> Self { Self(self.0.and(other.0)) }
    pub(crate) const fn if_true(&self, x: Word) -> Word { self.select(0, x) }
    pub(crate) const fn is_true_vartime(&self) -> bool { self.0.to_bool() }
    pub(crate) const fn to_u8(self) -> u8 { self.0.to_u8() }
}

macro_rules! word_choice {
    ($from_lsb:ident, $from_nz:ident, $from_lt:ident, $select:ident) => {
        impl CtChoice {
            pub(crate) const fn from_lsb(value: Word) -> Self {
                debug_assert!(value == 0 || value == 1);
                Self(ctutils::Choice::$from_lsb(value))
            }
            pub(crate) const fn from_usize_being_nonzero(value: usize) -> Self {
                Self(ctutils::Choice::$from_nz(value as Word))
            }
            pub(crate) const fn from_usize_lt(x: usize, y: usize) -> Self {
                Self(ctutils::Choice::$from_lt(x as Word, y as Word))
            }
            pub(crate) const fn select(&self, a: Word, b: Word) -> Word {
                self.0.$select(a, b)
            }
        }
    };
}
#[cfg(target_pointer_width = "32")]
word_choice!(from_u32_lsb, from_u32_nz, from_u32_lt, select_u32);
#[cfg(target_pointer_width = "64")]
word_choice!(from_u64_lsb, from_u64_nz, from_u64_lt, select_u64);

impl From<CtChoice> for Choice {
    fn from(choice: CtChoice) -> Self { Choice::from(choice.to_u8()) }
}
impl From<CtChoice> for bool {
    fn from(choice: CtChoice) -> Self { choice.is_true_vartime() }
}

#[cfg(test)]
mod tests {
    use super::CtChoice;
    use crate::Word;
    #[test]
    fn select() {
        let a: Word = 1;
        let b: Word = 2;
        assert_eq!(CtChoice::TRUE.select(a, b), b);
        assert_eq!(CtChoice::FALSE.select(a, b), a);
    }
}
