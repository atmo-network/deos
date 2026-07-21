//! Runtime ingress adapter for AAA `OnAddressEvent` trigger.
//!
//! Ingress producers (router fees, TMC distribution, asset transfer/mint hooks)
//! call this adapter instead of touching AAA storage directly.

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use polkadot_sdk::{
  pallet_assets::Event as AssetsEvent,
  pallet_balances::Event as BalancesEvent,
  sp_runtime::{
    DispatchError, DispatchResult, impl_tx_ext_default,
    traits::{DispatchInfoOf, PostDispatchInfoOf, StaticLookup, TransactionExtension},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
  },
};
use primitives::assets::TYPE_FOREIGN;
use scale_info::TypeInfo;

pub trait AddressEventIngress {
  fn preflight_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult;
  fn on_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult;
  fn preflight_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult;
  fn on_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult;
  fn on_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult;
}

pub struct RuntimeAddressEventIngress;

impl RuntimeAddressEventIngress {
  fn resolve_aaa(recipient: &AccountId) -> Option<pallet_aaa::AaaId> {
    crate::AAA::sovereign_index(recipient)
  }

  fn notify_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult {
    if amount == 0 {
      return Ok(());
    }
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return Ok(());
    };
    crate::AAA::notify_address_event_without_source(aaa_id, asset, amount)
  }
}

impl AddressEventIngress for RuntimeAddressEventIngress {
  fn preflight_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult {
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return Ok(());
    };
    let provenance = pallet_aaa::FundingProvenance::InternalProtocol(source.clone());
    crate::AAA::preflight_funding_event(aaa_id, asset, amount, Some(&provenance))
  }

  fn on_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult {
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return Ok(());
    };
    crate::AAA::notify_internal_address_event(aaa_id, asset, amount, source)
  }

  fn preflight_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult {
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return Ok(());
    };
    let provenance = pallet_aaa::FundingProvenance::Xcm(source.clone());
    crate::AAA::preflight_funding_event(aaa_id, asset, amount, Some(&provenance))
  }

  fn on_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> DispatchResult {
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return Ok(());
    };
    crate::AAA::notify_xcm_address_event(aaa_id, asset, amount, source)
  }

  fn on_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult {
    Self::notify_inbound_without_source(recipient, asset, amount)
  }
}

/// Charges and submits ingress for successful top-level balance/asset producer calls.
///
/// FRAME's generic asset pallets do not expose transfer callbacks. This transaction extension
/// turns their bounded transfer/mint calls into producer-owned ingress rather than relying on a
/// lossy prefix scan of the block event vector.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct AddressEventIngressExtension;

impl AddressEventIngressExtension {
  fn base_weight() -> Weight {
    <<Runtime as pallet_aaa::Config>::WeightInfo as pallet_aaa::WeightInfo>::transaction_extension_ingress_base()
  }

  fn notify_weight() -> Weight {
    <<Runtime as pallet_aaa::Config>::WeightInfo as pallet_aaa::WeightInfo>::transaction_extension_ingress_notify()
  }

  pub(crate) fn post_dispatch_refund(result_is_err: bool, submitted: bool) -> Weight {
    if result_is_err {
      Self::notify_weight()
    } else if submitted {
      Weight::zero()
    } else {
      Self::notify_weight().saturating_sub(Self::base_weight())
    }
  }

  fn preflight_fixed_signed_transfer(
    origin: &RuntimeOrigin,
    call: &RuntimeCall,
  ) -> Result<(), TransactionValidityError> {
    let Ok(source) = frame_system::ensure_signed(origin.clone()) else {
      return Ok(());
    };
    let candidate = match call {
      RuntimeCall::Balances(
        pallet_balances::Call::transfer_allow_death { dest, value }
        | pallet_balances::Call::transfer_keep_alive { dest, value },
      ) => <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
        .ok()
        .map(|recipient| (recipient, AssetKind::Native, *value)),
      RuntimeCall::Balances(pallet_balances::Call::transfer_all { dest, keep_alive }) => {
        let preservation = if *keep_alive {
          polkadot_sdk::frame_support::traits::tokens::Preservation::Preserve
        } else {
          polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable
        };
        let amount = <Balances as polkadot_sdk::frame_support::traits::fungible::Inspect<
          AccountId,
        >>::reducible_balance(
          &source,
          preservation,
          polkadot_sdk::frame_support::traits::tokens::Fortitude::Polite,
        );
        <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
          .ok()
          .map(|recipient| (recipient, AssetKind::Native, amount))
      }
      RuntimeCall::Assets(
        pallet_assets::Call::transfer { id, target, amount }
        | pallet_assets::Call::transfer_keep_alive { id, target, amount },
      ) => <Runtime as frame_system::Config>::Lookup::lookup(target.clone())
        .ok()
        .map(|recipient| {
          (
            recipient,
            RuntimeAddressEventIngressHook::map_asset_id(*id),
            *amount,
          )
        }),
      RuntimeCall::Assets(pallet_assets::Call::transfer_all {
        id,
        dest,
        keep_alive,
      }) => {
        let preservation = if *keep_alive {
          polkadot_sdk::frame_support::traits::tokens::Preservation::Preserve
        } else {
          polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable
        };
        let amount = <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
          AccountId,
        >>::reducible_balance(
          *id,
          &source,
          preservation,
          polkadot_sdk::frame_support::traits::tokens::Fortitude::Polite,
        );
        <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
          .ok()
          .map(|recipient| {
            (
              recipient,
              RuntimeAddressEventIngressHook::map_asset_id(*id),
              amount,
            )
          })
      }
      _ => None,
    };
    let Some((recipient, asset, amount)) = candidate else {
      return Ok(());
    };
    let Some(aaa_id) = RuntimeAddressEventIngress::resolve_aaa(&recipient) else {
      return Ok(());
    };
    let provenance = pallet_aaa::FundingProvenance::Signed(source);
    crate::AAA::preflight_funding_event(aaa_id, asset, amount, Some(&provenance))
      .map_err(|_| InvalidTransaction::Custom(40).into())
  }

  fn tracks(call: &RuntimeCall) -> bool {
    matches!(
      call,
      RuntimeCall::Assets(
        pallet_assets::Call::mint { .. }
          | pallet_assets::Call::transfer { .. }
          | pallet_assets::Call::transfer_keep_alive { .. }
          | pallet_assets::Call::force_transfer { .. }
          | pallet_assets::Call::transfer_approved { .. }
          | pallet_assets::Call::transfer_all { .. }
      ) | RuntimeCall::Balances(
        pallet_balances::Call::transfer_allow_death { .. }
          | pallet_balances::Call::transfer_keep_alive { .. }
          | pallet_balances::Call::force_transfer { .. }
          | pallet_balances::Call::transfer_all { .. }
      )
    )
  }
}

impl TransactionExtension<RuntimeCall> for AddressEventIngressExtension {
  const IDENTIFIER: &'static str = "AddressEventIngress";
  type Implicit = ();
  type Val = ();
  type Pre = Option<(u32, bool)>;

  fn weight(&self, call: &RuntimeCall) -> Weight {
    if Self::tracks(call) {
      Self::notify_weight()
    } else {
      Weight::zero()
    }
  }

  fn prepare(
    self,
    _val: Self::Val,
    origin: &<RuntimeCall as polkadot_sdk::sp_runtime::traits::Dispatchable>::RuntimeOrigin,
    call: &RuntimeCall,
    _info: &DispatchInfoOf<RuntimeCall>,
    _len: usize,
  ) -> Result<Self::Pre, TransactionValidityError> {
    Self::preflight_fixed_signed_transfer(origin, call)?;
    let event_source_is_verified_signer = frame_system::ensure_signed(origin.clone()).is_ok()
      && matches!(
        call,
        RuntimeCall::Assets(
          pallet_assets::Call::transfer { .. }
            | pallet_assets::Call::transfer_keep_alive { .. }
            | pallet_assets::Call::transfer_all { .. }
        ) | RuntimeCall::Balances(
          pallet_balances::Call::transfer_allow_death { .. }
            | pallet_balances::Call::transfer_keep_alive { .. }
            | pallet_balances::Call::transfer_all { .. }
        )
      );
    Ok(Self::tracks(call).then(|| (System::event_count(), event_source_is_verified_signer)))
  }

  fn post_dispatch_details(
    pre: Self::Pre,
    _info: &DispatchInfoOf<RuntimeCall>,
    _post_info: &PostDispatchInfoOf<RuntimeCall>,
    _len: usize,
    result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    let Some((start, event_source_is_verified_signer)) = pre else {
      return Ok(Weight::zero());
    };
    if result.is_err() {
      return Ok(Self::post_dispatch_refund(true, false));
    }
    let submitted = RuntimeAddressEventIngressHook::submit_events_since_with_verified_source(
      start,
      event_source_is_verified_signer,
    )
    .map_err(|_| InvalidTransaction::Custom(40))?;
    Ok(Self::post_dispatch_refund(false, submitted))
  }

  impl_tx_ext_default!(RuntimeCall; validate);
}

pub struct RuntimeAddressEventIngressHook;

impl RuntimeAddressEventIngressHook {
  pub(crate) fn probe_weight() -> Weight {
    <<Runtime as pallet_aaa::Config>::WeightInfo as pallet_aaa::WeightInfo>::compatibility_ingress_probe()
  }

  pub(crate) fn drain_unit_weight() -> Weight {
    <<Runtime as pallet_aaa::Config>::WeightInfo as pallet_aaa::WeightInfo>::compatibility_ingress_drain()
  }

  #[cfg(test)]
  pub(crate) fn submit_events_since(start: u32) -> bool {
    Self::submit_events_since_with_verified_source(start, false)
      .expect("test ingress notification must succeed")
  }

  pub(crate) fn submit_events_since_with_verified_source(
    start: u32,
    event_source_is_verified_signer: bool,
  ) -> Result<bool, DispatchError> {
    let mut submitted = false;
    for record in System::read_events_no_consensus().skip(start as usize) {
      submitted |= Self::submit_primary_event(&record.event, event_source_is_verified_signer)?;
    }
    Ok(submitted)
  }

  fn map_asset_id(asset_id: u32) -> AssetKind {
    if (asset_id & TYPE_FOREIGN) == TYPE_FOREIGN {
      return AssetKind::Foreign(asset_id);
    }
    AssetKind::Local(asset_id)
  }

  fn candidate_with_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> Option<pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>>
  {
    if amount == 0 {
      return None;
    }
    let aaa_id = RuntimeAddressEventIngress::resolve_aaa(recipient)?;
    Some(pallet_aaa::IngressOverflowEvent {
      aaa_id,
      asset,
      amount,
      provenance: Some(pallet_aaa::FundingProvenance::Signed(source.clone())),
    })
  }

  fn candidate_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Option<pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>>
  {
    if amount == 0 {
      return None;
    }
    let aaa_id = RuntimeAddressEventIngress::resolve_aaa(recipient)?;
    Some(pallet_aaa::IngressOverflowEvent {
      aaa_id,
      asset,
      amount,
      provenance: None,
    })
  }

  fn submit_primary_event(
    event: &crate::RuntimeEvent,
    event_source_is_verified_signer: bool,
  ) -> Result<bool, DispatchError> {
    let candidate = match event {
      crate::RuntimeEvent::Balances(BalancesEvent::Transfer { from, to, amount }) => {
        if event_source_is_verified_signer {
          Self::candidate_with_source(to, AssetKind::Native, *amount, from)
        } else {
          Self::candidate_without_source(to, AssetKind::Native, *amount)
        }
      }
      crate::RuntimeEvent::Assets(AssetsEvent::Transferred {
        asset_id,
        from,
        to,
        amount,
      }) => {
        let asset = Self::map_asset_id(*asset_id);
        if event_source_is_verified_signer {
          Self::candidate_with_source(to, asset, *amount, from)
        } else {
          Self::candidate_without_source(to, asset, *amount)
        }
      }
      crate::RuntimeEvent::Assets(AssetsEvent::Issued {
        asset_id,
        owner,
        amount,
      }) => Self::candidate_without_source(owner, Self::map_asset_id(*asset_id), *amount),
      _ => None,
    };
    let Some(candidate) = candidate else {
      return Ok(false);
    };
    Self::notify_candidate(candidate)?;
    Ok(true)
  }

  fn notify_candidate(
    event: pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>,
  ) -> DispatchResult {
    match event.provenance.as_ref() {
      Some(pallet_aaa::FundingProvenance::Signed(source)) => {
        crate::AAA::notify_address_event(event.aaa_id, event.asset, event.amount, source)
      }
      Some(pallet_aaa::FundingProvenance::InternalProtocol(source)) => {
        crate::AAA::notify_internal_address_event(event.aaa_id, event.asset, event.amount, source)
      }
      Some(pallet_aaa::FundingProvenance::Xcm(source)) => {
        crate::AAA::notify_xcm_address_event(event.aaa_id, event.asset, event.amount, source)
      }
      None => {
        crate::AAA::notify_address_event_without_source(event.aaa_id, event.asset, event.amount)
      }
    }
  }
}

impl pallet_aaa::AddressEventIngressHook<BlockNumber> for RuntimeAddressEventIngressHook {
  fn ingest(
    _now: BlockNumber,
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> polkadot_sdk::frame_support::weights::Weight {
    let ingress_probe = Self::probe_weight();
    let ingress_drain_unit = Self::drain_unit_weight();
    let max_admit = crate::configs::aaa_config::AaaMaxIngressEventsPerBlock::get();
    if max_admit == 0 {
      return Weight::zero();
    }
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(remaining_weight);
    if !meter.can_consume(ingress_probe) {
      return Weight::zero();
    }
    meter.consume(ingress_probe);
    let queued = crate::AAA::ingress_overflow_len();
    let drain_target = max_admit.min(queued);
    let mut drain_limit = 0u32;
    while drain_limit < drain_target && meter.can_consume(ingress_drain_unit) {
      meter.consume(ingress_drain_unit);
      drain_limit = drain_limit.saturating_add(1);
    }
    let _ = crate::AAA::drain_address_event_overflow(drain_limit);
    meter.consumed()
  }
}
