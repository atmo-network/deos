//! Runtime ingress adapter for AAA `OnAddressEvent` trigger.
//!
//! Ingress producers (router fees, TMC distribution, asset transfer/mint hooks)
//! call this adapter instead of touching AAA storage directly.

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use polkadot_sdk::sp_runtime::{
  DispatchResult, impl_tx_ext_default,
  traits::{DispatchInfoOf, PostDispatchInfoOf, StaticLookup, TransactionExtension},
  transaction_validity::{InvalidTransaction, TransactionValidityError},
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

fn map_asset_id(asset_id: u32) -> AssetKind {
  if (asset_id & TYPE_FOREIGN) == TYPE_FOREIGN {
    return AssetKind::Foreign(asset_id);
  }
  AssetKind::Local(asset_id)
}

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

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum PreparedIngressAmount {
  Fixed(Balance),
  RecipientBalanceBefore(Balance),
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct PreparedIngressCandidate {
  aaa_id: pallet_aaa::AaaId,
  recipient: AccountId,
  asset: AssetKind,
  source: Option<AccountId>,
  amount: PreparedIngressAmount,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum AddressEventIngressPre {
  Direct(Option<PreparedIngressCandidate>),
}

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

  fn is_fixed_signed_transfer(call: &RuntimeCall) -> bool {
    matches!(
      call,
      RuntimeCall::Balances(
        pallet_balances::Call::transfer_allow_death { .. }
          | pallet_balances::Call::transfer_keep_alive { .. }
          | pallet_balances::Call::transfer_all { .. }
      ) | RuntimeCall::Assets(
        pallet_assets::Call::transfer { .. }
          | pallet_assets::Call::transfer_keep_alive { .. }
          | pallet_assets::Call::transfer_all { .. }
      )
    )
  }

  fn prepare_fixed_signed_transfer(
    origin: &RuntimeOrigin,
    call: &RuntimeCall,
  ) -> Result<Option<PreparedIngressCandidate>, TransactionValidityError> {
    let source = frame_system::ensure_signed(origin.clone())
      .map_err(|_| TransactionValidityError::from(InvalidTransaction::BadSigner))?;
    let candidate = match call {
      RuntimeCall::Balances(
        pallet_balances::Call::transfer_allow_death { dest, value }
        | pallet_balances::Call::transfer_keep_alive { dest, value },
      ) => <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
        .ok()
        .map(|recipient| {
          (
            recipient,
            AssetKind::Native,
            *value,
            PreparedIngressAmount::Fixed(*value),
          )
        }),
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
          .map(|recipient| {
            let before = <Balances as polkadot_sdk::frame_support::traits::fungible::Inspect<
              AccountId,
            >>::balance(&recipient);
            (
              recipient,
              AssetKind::Native,
              amount,
              PreparedIngressAmount::RecipientBalanceBefore(before),
            )
          })
      }
      RuntimeCall::Assets(
        pallet_assets::Call::transfer { id, target, amount }
        | pallet_assets::Call::transfer_keep_alive { id, target, amount },
      ) => <Runtime as frame_system::Config>::Lookup::lookup(target.clone())
        .ok()
        .map(|recipient| {
          (
            recipient,
            map_asset_id(*id),
            *amount,
            PreparedIngressAmount::Fixed(*amount),
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
            let before =
              <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
                AccountId,
              >>::balance(*id, &recipient);
            (
              recipient,
              map_asset_id(*id),
              amount,
              PreparedIngressAmount::RecipientBalanceBefore(before),
            )
          })
      }
      _ => None,
    };
    let Some((recipient, asset, preflight_amount, amount)) = candidate else {
      return Ok(None);
    };
    let Some(aaa_id) = RuntimeAddressEventIngress::resolve_aaa(&recipient) else {
      return Ok(None);
    };
    let provenance = pallet_aaa::FundingProvenance::Signed(source.clone());
    crate::AAA::preflight_funding_event(aaa_id, asset, preflight_amount, Some(&provenance))
      .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(PreparedIngressCandidate {
      aaa_id,
      recipient,
      asset,
      source: Some(source),
      amount,
    }))
  }

  fn prepare_dynamic_producer(
    call: &RuntimeCall,
  ) -> Result<Option<PreparedIngressCandidate>, TransactionValidityError> {
    let candidate = match call {
      RuntimeCall::Assets(pallet_assets::Call::mint {
        id,
        beneficiary,
        amount,
      }) => <Runtime as frame_system::Config>::Lookup::lookup(beneficiary.clone())
        .ok()
        .map(|recipient| (recipient, map_asset_id(*id), *amount)),
      RuntimeCall::Assets(pallet_assets::Call::force_transfer {
        id, dest, amount, ..
      }) => <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
        .ok()
        .map(|recipient| (recipient, map_asset_id(*id), *amount)),
      RuntimeCall::Assets(pallet_assets::Call::transfer_approved {
        id,
        destination,
        amount,
        ..
      }) => <Runtime as frame_system::Config>::Lookup::lookup(destination.clone())
        .ok()
        .map(|recipient| (recipient, map_asset_id(*id), *amount)),
      RuntimeCall::Balances(pallet_balances::Call::force_transfer { dest, value, .. }) => {
        <Runtime as frame_system::Config>::Lookup::lookup(dest.clone())
          .ok()
          .map(|recipient| (recipient, AssetKind::Native, *value))
      }
      _ => None,
    };
    let Some((recipient, asset, amount)) = candidate else {
      return Ok(None);
    };
    let Some(aaa_id) = RuntimeAddressEventIngress::resolve_aaa(&recipient) else {
      return Ok(None);
    };
    crate::AAA::preflight_funding_event(aaa_id, asset, amount, None)
      .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(PreparedIngressCandidate {
      aaa_id,
      recipient,
      asset,
      source: None,
      amount: PreparedIngressAmount::Fixed(amount),
    }))
  }

  fn prepared_amount(candidate: &PreparedIngressCandidate) -> Balance {
    match candidate.amount {
      PreparedIngressAmount::Fixed(amount) => amount,
      PreparedIngressAmount::RecipientBalanceBefore(before) => {
        let after = match candidate.asset {
          AssetKind::Native => {
            <Balances as polkadot_sdk::frame_support::traits::fungible::Inspect<AccountId>>::balance(
              &candidate.recipient,
            )
          }
          AssetKind::Local(asset_id) | AssetKind::Foreign(asset_id) => {
            <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
              AccountId,
            >>::balance(asset_id, &candidate.recipient)
          }
        };
        after.saturating_sub(before)
      }
    }
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
  type Pre = Option<AddressEventIngressPre>;

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
    if frame_system::ensure_signed(origin.clone()).is_ok() && Self::is_fixed_signed_transfer(call) {
      return Ok(Some(AddressEventIngressPre::Direct(
        Self::prepare_fixed_signed_transfer(origin, call)?,
      )));
    }
    if Self::tracks(call) {
      return Ok(Some(AddressEventIngressPre::Direct(
        Self::prepare_dynamic_producer(call)?,
      )));
    }
    Ok(None)
  }

  fn post_dispatch_details(
    pre: Self::Pre,
    _info: &DispatchInfoOf<RuntimeCall>,
    _post_info: &PostDispatchInfoOf<RuntimeCall>,
    _len: usize,
    result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    let Some(pre) = pre else {
      return Ok(Weight::zero());
    };
    if result.is_err() {
      return Ok(Self::post_dispatch_refund(true, false));
    }
    let submitted = match pre {
      AddressEventIngressPre::Direct(Some(candidate)) => {
        let amount = Self::prepared_amount(&candidate);
        if amount == 0 {
          false
        } else {
          match candidate.source.as_ref() {
            Some(source) => {
              crate::AAA::notify_address_event(candidate.aaa_id, candidate.asset, amount, source)
            }
            None => crate::AAA::notify_address_event_without_source(
              candidate.aaa_id,
              candidate.asset,
              amount,
            ),
          }
          .map_err(|_| InvalidTransaction::Custom(40))?;
          true
        }
      }
      AddressEventIngressPre::Direct(None) => false,
    };
    Ok(Self::post_dispatch_refund(false, submitted))
  }

  impl_tx_ext_default!(RuntimeCall; validate);
}
