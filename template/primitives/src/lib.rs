#![cfg_attr(not(feature = "std"), no_std)]

pub mod assets;
pub mod ecosystem;

pub use assets::*;
pub use ecosystem::*;
