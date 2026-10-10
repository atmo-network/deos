use crate::pallet::*;
use polkadot_sdk::sp_runtime::DispatchResult;

impl<T: Config> crate::BalanceTransitionIngress<T::AssetId> for Pallet<T> {
  fn note_balance_transition(asset: T::AssetId) -> DispatchResult {
    Pallet::<T>::publish_balance_dependency_event(asset)
  }
}
