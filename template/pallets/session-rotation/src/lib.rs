#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
use polkadot_sdk::frame_support::weights::Weight;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper {
  fn prepare_rotation();
  fn verify_rotation();
}

pub trait SessionRotation<BlockNumber> {
  fn should_rotate(now: BlockNumber) -> bool;
  fn rotate();
}

pub trait WeightInfo {
  fn rotate_session() -> Weight;
}

impl WeightInfo for () {
  fn rotate_session() -> Weight {
    Weight::zero()
  }
}

#[frame::pallet]
pub mod pallet {
  use super::*;
  use frame::prelude::*;

  #[pallet::config]
  pub trait Config: frame_system::Config {
    type SessionRotation: SessionRotation<BlockNumberFor<Self>>;
    type WeightInfo: WeightInfo;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: BenchmarkHelper;
  }

  #[pallet::pallet]
  pub struct Pallet<T>(_);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_initialize(now: BlockNumberFor<T>) -> Weight {
      if !T::SessionRotation::should_rotate(now) {
        return Weight::zero();
      }
      T::SessionRotation::rotate();
      T::WeightInfo::rotate_session()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::cell::RefCell;
  use polkadot_sdk::{
    frame_support::{construct_runtime, derive_impl, parameter_types, traits::Hooks},
    frame_system,
  };

  type Block = frame_system::mocking::MockBlock<Test>;

  construct_runtime!(
    pub enum Test {
      System: frame_system,
      SessionRotationPallet: crate,
    }
  );

  #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
  impl frame_system::Config for Test {
    type Block = Block;
  }

  thread_local! {
    static ROTATIONS: RefCell<u32> = const { RefCell::new(0) };
  }

  pub struct MockRotation;
  impl SessionRotation<u64> for MockRotation {
    fn should_rotate(now: u64) -> bool {
      now > 0 && now.is_multiple_of(10)
    }

    fn rotate() {
      ROTATIONS.with(|rotations| *rotations.borrow_mut() += 1);
    }
  }

  parameter_types! {
    pub const RotationWeight: Weight = Weight::from_parts(123, 45);
  }

  impl WeightInfo for RotationWeight {
    fn rotate_session() -> Weight {
      RotationWeight::get()
    }
  }

  #[cfg(feature = "runtime-benchmarks")]
  impl BenchmarkHelper for MockRotation {
    fn prepare_rotation() {}
    fn verify_rotation() {
      ROTATIONS.with(|rotations| assert_eq!(*rotations.borrow(), 1));
    }
  }

  impl Config for Test {
    type SessionRotation = MockRotation;
    type WeightInfo = RotationWeight;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = MockRotation;
  }

  #[test]
  fn rotates_only_on_schedule_and_returns_owned_weight() {
    ROTATIONS.with(|rotations| *rotations.borrow_mut() = 0);
    assert_eq!(Pallet::<Test>::on_initialize(9), Weight::zero());
    assert_eq!(Pallet::<Test>::on_initialize(10), RotationWeight::get());
    assert_eq!(Pallet::<Test>::on_initialize(11), Weight::zero());
    ROTATIONS.with(|rotations| assert_eq!(*rotations.borrow(), 1));
  }
}
