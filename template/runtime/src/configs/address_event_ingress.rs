//! Runtime ingress adapter for AAA `OnAddressEvent` trigger.
//!
//! Ingress producers (router fees, TMC distribution, asset transfer/mint hooks)
//! call this adapter instead of touching AAA storage directly.

use super::*;
use alloc::{collections::BTreeMap, vec::Vec};
use polkadot_sdk::{pallet_assets::Event as AssetsEvent, pallet_balances::Event as BalancesEvent};
use primitives::assets::TYPE_FOREIGN;

pub trait AddressEventIngress {
  fn on_inbound_with_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  );
  fn on_inbound_without_source(recipient: &AccountId, asset: AssetKind, amount: Balance);
}

pub struct RuntimeAddressEventIngress;

impl RuntimeAddressEventIngress {
  fn resolve_aaa(recipient: &AccountId) -> Option<pallet_aaa::AaaId> {
    crate::AAA::sovereign_index(recipient)
  }

  fn notify_aaa_event(
    aaa_id: pallet_aaa::AaaId,
    asset: AssetKind,
    amount: Balance,
    source: Option<&AccountId>,
  ) {
    if amount == 0 {
      return;
    }
    if let Some(source) = source {
      crate::AAA::notify_address_event(aaa_id, asset, amount, source);
      return;
    }
    crate::AAA::notify_address_event_without_source(aaa_id, asset, amount);
  }

  fn queue_aaa_event(
    aaa_id: pallet_aaa::AaaId,
    asset: AssetKind,
    amount: Balance,
    source: Option<AccountId>,
  ) -> bool {
    crate::AAA::queue_address_event(aaa_id, asset, amount, source)
  }

  fn notify_inbound_with_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) -> bool {
    if amount == 0 {
      return false;
    }
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return false;
    };
    Self::notify_aaa_event(aaa_id, asset, amount, Some(source));
    true
  }

  fn notify_inbound_without_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> bool {
    if amount == 0 {
      return false;
    }
    let Some(aaa_id) = Self::resolve_aaa(recipient) else {
      return false;
    };
    Self::notify_aaa_event(aaa_id, asset, amount, None);
    true
  }
}

impl AddressEventIngress for RuntimeAddressEventIngress {
  fn on_inbound_with_source(
    recipient: &AccountId,
    asset: AssetKind,
    amount: Balance,
    source: &AccountId,
  ) {
    let _ = Self::notify_inbound_with_source(recipient, asset, amount, source);
  }

  fn on_inbound_without_source(recipient: &AccountId, asset: AssetKind, amount: Balance) {
    let _ = Self::notify_inbound_without_source(recipient, asset, amount);
  }
}

pub struct RuntimeAddressEventIngressHook;

impl RuntimeAddressEventIngressHook {
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
      source: Some(source.clone()),
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
      source: None,
    })
  }

  fn event_to_candidate(
    event: &crate::RuntimeEvent,
  ) -> Option<pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>>
  {
    match event {
      crate::RuntimeEvent::Balances(BalancesEvent::Transfer { from, to, amount }) => {
        Self::candidate_with_source(to, AssetKind::Native, *amount, from)
      }
      crate::RuntimeEvent::Balances(
        BalancesEvent::Deposit { who, amount }
        | BalancesEvent::Minted { who, amount }
        | BalancesEvent::Endowed {
          account: who,
          free_balance: amount,
        },
      ) => Self::candidate_without_source(who, AssetKind::Native, *amount),
      crate::RuntimeEvent::Assets(AssetsEvent::Transferred {
        asset_id,
        from,
        to,
        amount,
      }) => Self::candidate_with_source(to, Self::map_asset_id(*asset_id), *amount, from),
      crate::RuntimeEvent::Assets(
        AssetsEvent::Issued {
          asset_id,
          owner,
          amount,
        }
        | AssetsEvent::Deposited {
          asset_id,
          who: owner,
          amount,
        },
      ) => Self::candidate_without_source(owner, Self::map_asset_id(*asset_id), *amount),
      _ => None,
    }
  }

  fn aggregate_candidates(
    max_scan: usize,
  ) -> (
    u64,
    Vec<pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>>,
  ) {
    let mut scanned: u64 = 0;
    let mut aggregated: Vec<
      pallet_aaa::IngressOverflowEvent<pallet_aaa::AaaId, AssetKind, Balance, AccountId>,
    > = Vec::new();
    let mut index: BTreeMap<(pallet_aaa::AaaId, AssetKind, Option<AccountId>), usize> =
      BTreeMap::new();
    for record in crate::System::read_events_no_consensus().take(max_scan) {
      scanned = scanned.saturating_add(1);
      let Some(candidate) = Self::event_to_candidate(&record.event) else {
        continue;
      };
      let key = (candidate.aaa_id, candidate.asset, candidate.source.clone());
      if let Some(pos) = index.get(&key).copied() {
        aggregated[pos].amount = candidate.amount;
        continue;
      }
      index.insert(key, aggregated.len());
      aggregated.push(candidate);
    }
    (scanned, aggregated)
  }
}

impl pallet_aaa::AddressEventIngressHook<BlockNumber> for RuntimeAddressEventIngressHook {
  fn ingest(_now: BlockNumber) -> polkadot_sdk::frame_support::weights::Weight {
    const INGRESS_SCAN_WEIGHT_REF_TIME: u64 = 1_000;
    const INGRESS_QUEUE_DRAIN_WEIGHT_REF_TIME: u64 = 2_000;
    const INGRESS_QUEUE_ENQUEUE_WEIGHT_REF_TIME: u64 = 2_000;
    let max_admit = crate::configs::aaa_config::AaaMaxIngressEventsPerBlock::get();
    let max_scan = crate::configs::aaa_config::AaaMaxIngressScanEventsPerBlock::get() as usize;
    let drained = crate::AAA::drain_address_event_overflow(max_admit);
    let mut remaining_admit = max_admit.saturating_sub(drained);
    let (scanned, aggregated) = Self::aggregate_candidates(max_scan);
    let mut queued: u64 = 0;
    for event in aggregated {
      if remaining_admit > 0 {
        RuntimeAddressEventIngress::notify_aaa_event(
          event.aaa_id,
          event.asset,
          event.amount,
          event.source.as_ref(),
        );
        remaining_admit = remaining_admit.saturating_sub(1);
        continue;
      }
      if RuntimeAddressEventIngress::queue_aaa_event(
        event.aaa_id,
        event.asset,
        event.amount,
        event.source,
      ) {
        queued = queued.saturating_add(1);
      }
    }
    let weight_ref_time = scanned
      .saturating_mul(INGRESS_SCAN_WEIGHT_REF_TIME)
      .saturating_add(u64::from(drained).saturating_mul(INGRESS_QUEUE_DRAIN_WEIGHT_REF_TIME))
      .saturating_add(queued.saturating_mul(INGRESS_QUEUE_ENQUEUE_WEIGHT_REF_TIME));
    polkadot_sdk::frame_support::weights::Weight::from_parts(weight_ref_time, 0)
  }
}
