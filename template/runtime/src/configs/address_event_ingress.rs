//! Runtime ingress adapter for Actors `OnAddressEvent` trigger.
//!
//! Ingress producers (router fees, TMC distribution, asset transfer/mint hooks)
//! call this adapter instead of touching Actors storage directly. Every movement path
//! that claims Actors ingress must be registered in `ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY`;
//! movement outside that inventory is balance-only.

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use pallet_deos_actors::{AddressEvent, FundingProvenance, IngressFailure};
use polkadot_sdk::sp_runtime::{
  DispatchResult, impl_tx_ext_default,
  traits::{DispatchInfoOf, PostDispatchInfoOf, StaticLookup, TransactionExtension},
  transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use primitives::assets::TYPE_FOREIGN;
use scale_info::TypeInfo;

pub struct RuntimeAddressEventIngress;

fn map_asset_id(asset_id: u32) -> AssetKind {
  if (asset_id & TYPE_FOREIGN) == TYPE_FOREIGN {
    return AssetKind::Foreign(asset_id);
  }
  AssetKind::Local(asset_id)
}

impl RuntimeAddressEventIngress {
  fn resolve_actor(recipient: &AccountId) -> Option<pallet_deos_actors::ActorId> {
    crate::Actors::sovereign_index(recipient)
  }

  /// Sole certified-producer inventory accessor (spec 5.3). The generated ingress
  /// evidence parses the same constant; the runtime test binds both.
  #[allow(dead_code)] // evidence surface consumed by runtime tests and generated drift checks
  pub const fn certified_producer_inventory() -> &'static [AddressEventProducer] {
    ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY
  }

  /// Provenance-specific certified-ingress helpers. Each constructs one typed
  /// `AddressEvent` and routes it through the single typed boundary, so every
  /// producer movement shares one preflight/notify surface and one error class.
  pub fn preflight_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> Result<(), IngressFailure> {
    crate::Actors::preflight_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset,
      amount,
      provenance: Some(FundingProvenance::InternalProtocol),
    })
  }

  pub fn on_internal_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> Result<(), IngressFailure> {
    crate::Actors::notify_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset,
      amount,
      provenance: Some(FundingProvenance::InternalProtocol),
    })
  }

  pub fn preflight_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> Result<(), IngressFailure> {
    crate::Actors::preflight_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset,
      amount,
      provenance: Some(FundingProvenance::Xcm),
    })
  }

  pub fn on_xcm_inbound(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> Result<(), IngressFailure> {
    crate::Actors::notify_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset,
      amount,
      provenance: Some(FundingProvenance::Xcm),
    })
  }

  pub fn preflight_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), IngressFailure> {
    crate::Actors::preflight_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: None,
      asset,
      amount,
      provenance: None,
    })
  }

  pub fn on_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), IngressFailure> {
    crate::Actors::notify_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: None,
      asset,
      amount,
      provenance: None,
    })
  }
}

/// Certified movement ordering at the owning atomicity boundary (spec 5.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // evidence surface consumed by runtime tests and generated drift checks
pub enum CertifiedMovementProtocol {
  PostMovementNotify,
  BlockAtomicPostDispatch,
  XcmTransactionalPrecommit,
}

/// One named certified AddressEvent movement path (spec 5.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // evidence surface consumed by runtime tests and generated drift checks
pub struct AddressEventProducer {
  pub id: &'static str,
  pub protocol: CertifiedMovementProtocol,
  pub credited_surface: &'static str,
  pub source_provenance: &'static str,
  pub preflight_owner: &'static str,
  pub consequence_owner: &'static str,
  pub rollback_owner: &'static str,
  pub weight_owner: &'static str,
}

#[allow(dead_code)] // evidence surface consumed by runtime tests and generated drift checks
pub const ACTORS_ADDRESS_EVENT_PRODUCER_INVENTORY: &[AddressEventProducer] = &[
  AddressEventProducer {
    id: "AddressEventIngressExtension::signed_transfer",
    protocol: CertifiedMovementProtocol::BlockAtomicPostDispatch,
    credited_surface: "Recipient sovereign",
    source_provenance: "Signer / Signed",
    preflight_owner: "TransactionExtension::prepare",
    consequence_owner: "TransactionExtension::post_dispatch_details",
    rollback_owner: "Block author/import state transaction",
    weight_owner: "transaction_extension_ingress_base/_notify",
  },
  AddressEventProducer {
    id: "AddressEventIngressExtension::transfer_all",
    protocol: CertifiedMovementProtocol::BlockAtomicPostDispatch,
    credited_surface: "Recipient sovereign",
    source_provenance: "Signer / Signed, actual recipient delta",
    preflight_owner: "TransactionExtension::prepare",
    consequence_owner: "TransactionExtension::post_dispatch_details",
    rollback_owner: "Block author/import state transaction",
    weight_owner: "transaction_extension_ingress_base/_notify",
  },
  AddressEventProducer {
    id: "AddressEventIngressExtension::privileged_or_delegated",
    protocol: CertifiedMovementProtocol::BlockAtomicPostDispatch,
    credited_surface: "Recipient sovereign",
    source_provenance: "Source-less / none",
    preflight_owner: "TransactionExtension::prepare_dynamic_producer",
    consequence_owner: "TransactionExtension::post_dispatch_details",
    rollback_owner: "Block author/import state transaction",
    weight_owner: "transaction_extension_ingress_base/_notify",
  },
  AddressEventProducer {
    id: "TmctolAssetOps::transfer",
    protocol: CertifiedMovementProtocol::PostMovementNotify,
    credited_surface: "Task `to` sovereign",
    source_provenance: "Sender / InternalProtocol",
    preflight_owner: "TmctolAssetOps::transfer preflight",
    consequence_owner: "RuntimeAddressEventIngress::on_internal_inbound",
    rollback_owner: "Asset ops storage transaction",
    weight_owner: "task_transfer/task_split_transfer generated weights",
  },
  AddressEventProducer {
    id: "TmctolAssetOps::mint",
    protocol: CertifiedMovementProtocol::PostMovementNotify,
    credited_surface: "Task `to` sovereign",
    source_provenance: "Source-less / none",
    preflight_owner: "TmctolAssetOps::mint preflight",
    consequence_owner: "RuntimeAddressEventIngress::on_inbound_without_source",
    rollback_owner: "Asset ops storage transaction",
    weight_owner: "task_mint generated weight",
  },
  AddressEventProducer {
    id: "TmctolMintDistributionIngress",
    protocol: CertifiedMovementProtocol::PostMovementNotify,
    credited_surface: "Collateral/minted recipients",
    source_provenance: "Mint source / InternalProtocol",
    preflight_owner: "before_collateral_transfer/before_sink_mint",
    consequence_owner: "after_distribution",
    rollback_owner: "TMC distribution transaction",
    weight_owner: "TMC distribution generated weights",
  },
  AddressEventProducer {
    id: "DeosRouter::route_fee",
    protocol: CertifiedMovementProtocol::PostMovementNotify,
    credited_surface: "Burn Actor sovereign",
    source_provenance: "Fee payer / InternalProtocol",
    preflight_owner: "FeeManagerImpl::route_fee preflight",
    consequence_owner: "FeeManagerImpl::route_fee notify",
    rollback_owner: "Router fee transaction",
    weight_owner: "Router fee routing generated weights",
  },
  AddressEventProducer {
    id: "XCM asset deposit",
    protocol: CertifiedMovementProtocol::XcmTransactionalPrecommit,
    credited_surface: "Recipient sovereign",
    source_provenance: "XCM origin / Xcm",
    preflight_owner: "ActorAwareAssetTransactor::preflight_ingress",
    consequence_owner: "ActorAwareAssetTransactor::precommit_ingress",
    rollback_owner: "ActorAwareAssetTransactor storage transaction",
    weight_owner: "One-asset deposit generated weight",
  },
  AddressEventProducer {
    id: "XCM deposit without origin",
    protocol: CertifiedMovementProtocol::XcmTransactionalPrecommit,
    credited_surface: "Recipient sovereign",
    source_provenance: "Source-less / none",
    preflight_owner: "ActorAwareAssetTransactor::preflight_ingress",
    consequence_owner: "ActorAwareAssetTransactor::precommit_ingress",
    rollback_owner: "ActorAwareAssetTransactor storage transaction",
    weight_owner: "One-asset deposit generated weight",
  },
];

impl pallet_deos_actors::AddressEventIngress<AccountId, AssetKind, Balance>
  for RuntimeAddressEventIngress
{
  fn preflight(event: &AddressEvent<AccountId, AssetKind, Balance>) -> Result<(), IngressFailure> {
    crate::Actors::preflight_ingress(event)
  }

  fn notify(event: &AddressEvent<AccountId, AssetKind, Balance>) -> Result<(), IngressFailure> {
    crate::Actors::notify_ingress(event)
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
    <<Runtime as pallet_deos_actors::Config>::WeightInfo as pallet_deos_actors::WeightInfo>::transaction_extension_ingress_base()
  }

  fn notify_weight() -> Weight {
    let base = Self::base_weight();
    let notify = <<Runtime as pallet_deos_actors::Config>::WeightInfo as pallet_deos_actors::WeightInfo>::transaction_extension_ingress_notify();
    Weight::from_parts(
      base.ref_time().max(notify.ref_time()),
      base.proof_size().max(notify.proof_size()),
    )
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
    // Only movements to an Actors sovereign are certified: a non-sovereign recipient
    // is balance-only and must not carry the notification envelope.
    if RuntimeAddressEventIngress::resolve_actor(&recipient).is_none() {
      return Ok(None);
    }
    let provenance = pallet_deos_actors::FundingProvenance::Signed;
    crate::Actors::preflight_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset,
      amount: preflight_amount,
      provenance: Some(provenance),
    })
    .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(PreparedIngressCandidate {
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
    // Only movements to an Actors sovereign are certified: a non-sovereign recipient
    // is balance-only and must not carry the notification envelope.
    if RuntimeAddressEventIngress::resolve_actor(&recipient).is_none() {
      return Ok(None);
    }
    crate::Actors::preflight_ingress(&AddressEvent {
      destination: recipient.clone(),
      source: None,
      asset,
      amount,
      provenance: None,
    })
    .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(PreparedIngressCandidate {
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
          crate::Actors::notify_ingress(&AddressEvent {
            destination: candidate.recipient.clone(),
            source: candidate.source.clone(),
            asset: candidate.asset,
            amount,
            provenance: candidate
              .source
              .as_ref()
              .map(|_| pallet_deos_actors::FundingProvenance::Signed),
          })
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
