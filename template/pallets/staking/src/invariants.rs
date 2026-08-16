#![cfg(feature = "try-runtime")]

use crate::pallet::{Config, Pallet};
use polkadot_sdk::frame_support::traits::Currency;
use polkadot_sdk::sp_runtime::{
  TryRuntimeError,
  traits::{CheckedSub, Zero},
};

impl<T: Config> Pallet<T> {
  pub(crate) fn ensure_native_security_reward_custody(
    expected_liability: T::Balance,
  ) -> Result<(), TryRuntimeError> {
    let reward_spendable = T::NativeCurrency::free_balance(&Self::native_security_reward_account())
      .checked_sub(&T::NativeCurrency::minimum_balance())
      .unwrap_or_else(T::Balance::zero);
    if reward_spendable < expected_liability {
      return Err(TryRuntimeError::Other(
        "Native security reward custody is below outstanding liability",
      ));
    }
    Ok(())
  }
}
