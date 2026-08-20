//! Transaction-extension ownership for the Asset Conversion LP reverse index.

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use polkadot_sdk::sp_runtime::{
  DispatchResult, impl_tx_ext_default,
  traits::{DispatchInfoOf, PostDispatchInfoOf, TransactionExtension},
  transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use scale_info::TypeInfo;

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct PoolIndexExtension;

impl PoolIndexExtension {
  fn index_weight() -> Weight {
    let feed_registration =
      <crate::weights::pallet_oracle::SubstrateWeight<Runtime> as pallet_oracle::WeightInfo>::register_feed_existing_producer()
        .max(
          <crate::weights::pallet_oracle::SubstrateWeight<Runtime> as pallet_oracle::WeightInfo>::register_feed_new_producer(),
        );
    <Runtime as frame_system::Config>::DbWeight::get()
      .reads_writes(13, 1)
      .saturating_add(feed_registration.saturating_mul(2))
  }

  fn pair(call: &RuntimeCall) -> Option<(AssetKind, AssetKind)> {
    match call {
      RuntimeCall::AssetConversion(
        pallet_asset_conversion::Call::create_pool { asset1, asset2 }
        | pallet_asset_conversion::Call::add_liquidity { asset1, asset2, .. },
      ) => Some((**asset1, **asset2)),
      _ => None,
    }
  }

  fn expected_lp_token(call: &RuntimeCall, pair: (AssetKind, AssetKind)) -> Option<u32> {
    match call {
      RuntimeCall::AssetConversion(pallet_asset_conversion::Call::create_pool { .. }) => {
        pallet_asset_conversion::NextPoolAssetId::<Runtime>::get()
      }
      RuntimeCall::AssetConversion(pallet_asset_conversion::Call::add_liquidity { .. }) => {
        let pool_id =
          <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&pair.0, &pair.1)
            .ok()?;
        pallet_asset_conversion::Pools::<Runtime>::get(pool_id).map(|pool| pool.lp_token)
      }
      _ => None,
    }
  }
}

impl TransactionExtension<RuntimeCall> for PoolIndexExtension {
  const IDENTIFIER: &'static str = "PoolIndex";
  type Implicit = ();
  type Val = ();
  type Pre = Option<(AssetKind, AssetKind)>;

  fn weight(&self, call: &RuntimeCall) -> Weight {
    if Self::pair(call).is_some() {
      Self::index_weight()
    } else {
      Weight::zero()
    }
  }

  fn prepare(
    self,
    _val: Self::Val,
    _origin: &<RuntimeCall as polkadot_sdk::sp_runtime::traits::Dispatchable>::RuntimeOrigin,
    call: &RuntimeCall,
    _info: &DispatchInfoOf<RuntimeCall>,
    _len: usize,
  ) -> Result<Self::Pre, TransactionValidityError> {
    let Some(pair) = Self::pair(call) else {
      return Ok(None);
    };
    if let Some(lp_token) = Self::expected_lp_token(call, pair) {
      crate::configs::assets_config::preflight_register_pool_lp_pair(pair.0, pair.1, lp_token)
        .map_err(|_| InvalidTransaction::Custom(41))?;
    }
    Ok(Some(pair))
  }

  fn post_dispatch_details(
    pre: Self::Pre,
    _info: &DispatchInfoOf<RuntimeCall>,
    _post_info: &PostDispatchInfoOf<RuntimeCall>,
    _len: usize,
    result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    let Some((asset1, asset2)) = pre else {
      return Ok(Weight::zero());
    };
    if result.is_err() {
      return Ok(Self::index_weight());
    }
    crate::configs::assets_config::register_pool_lp_pair(asset1, asset2)
      .map_err(|_| InvalidTransaction::Custom(41))?;
    Ok(Weight::zero())
  }

  impl_tx_ext_default!(RuntimeCall; validate);
}
