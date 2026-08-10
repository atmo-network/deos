//! Bounded TMCTOL guarantee-state read projection.
//!
//! This module owns no storage and exposes no extrinsics. It composes live state from the
//! reference-runtime pallets into a compact read-only protocol projection.
//!
//! Boundary: this is the canonical live read model for protocol guarantees. It MUST NOT grow
//! historical metrics, dashboards, trend analysis, or alerting; those belong in external indexers,
//! operator tooling, or test-only analytical helpers that may consume this projection.

use pallet_deos_actors::{ActorType, Task};
use polkadot_sdk::frame_support::traits::fungibles::Inspect as FungiblesInspect;
use polkadot_sdk::pallet_asset_conversion::{self, PoolLocator};
use polkadot_sdk::sp_runtime::traits::Zero;
use primitives::{
  AnchorBucketState, AnchorDomain, AssetKind, BurnDomain, BurnLivenessState, GuaranteeStatus,
  PoolProjection, ReportedFloorInputs, TmctolConformanceStatus, TmctolGuaranteeState,
  ZapPostconditionState, ecosystem,
};

use crate::{AccountId, Assets, Balance, Balances, Runtime};

pub struct TmctolReadModel;

impl TmctolReadModel {
  pub fn tmctol_guarantee_state() -> TmctolGuaranteeState<AccountId, Balance> {
    let tol_anchor = Self::anchor_bucket_state(
      AnchorDomain::Tol,
      ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
    );
    let bldr_anchor = Self::anchor_bucket_state(
      AnchorDomain::Bldr,
      ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID,
    );

    let tol_counterparty =
      pallet_tmc::TokenCurves::<Runtime>::get(AssetKind::Native).map(|curve| curve.foreign_asset);
    let tol_pool = Self::pool_projection(AnchorDomain::Tol, tol_counterparty, &tol_anchor);
    let bldr_pool = Self::pool_projection(
      AnchorDomain::Bldr,
      Some(AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID)),
      &bldr_anchor,
    );
    let native_floor_inputs = Self::native_floor_inputs(&tol_pool);

    let native_burn_liveness = Self::native_burn_liveness();
    let bldr_buyback_liveness = Self::bldr_buyback_liveness();
    let zap_postconditions = Self::zap_postconditions();
    let anchor_status = Self::aggregate_status([tol_anchor.status, bldr_anchor.status]);
    let pool_status = Self::aggregate_status([tol_pool.status, bldr_pool.status]);
    let burn_liveness_status =
      Self::aggregate_status([native_burn_liveness.status, bldr_buyback_liveness.status]);
    let zap_status = zap_postconditions.status;
    let conformance_status =
      Self::conformance_status([anchor_status, pool_status, burn_liveness_status, zap_status]);

    TmctolGuaranteeState {
      tol_anchor,
      bldr_anchor,
      tol_pool,
      bldr_pool,
      native_floor_inputs,
      native_burn_liveness,
      bldr_buyback_liveness,
      zap_postconditions,
      anchor_status,
      pool_status,
      burn_liveness_status,
      zap_status,
      conformance_status,
    }
  }

  fn anchor_bucket_state(domain: AnchorDomain, actor_id: u64) -> AnchorBucketState<AccountId> {
    let sovereign_account = crate::Actors::sovereign_account_id_system(actor_id);
    let is_custody_only = matches!(
      (domain, actor_id),
      (
        AnchorDomain::Tol,
        ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID
      ) | (
        AnchorDomain::Bldr,
        ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID
      )
    );
    let actor_identity_exists =
      pallet_deos_actors::ActorIdentities::<Runtime>::contains_key(actor_id);
    let scheduler_state_exists = pallet_deos_actors::ActorHot::<Runtime>::get(actor_id)
      .is_some_and(|hot| {
        hot.pending_signal || hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some()
      });
    let status = if is_custody_only && !actor_identity_exists && !scheduler_state_exists {
      GuaranteeStatus::Satisfied
    } else {
      GuaranteeStatus::Violated
    };

    AnchorBucketState {
      domain,
      actor_id,
      status,
      sovereign_account,
      is_custody_only,
      actor_identity_exists,
      scheduler_state_exists,
    }
  }

  fn pool_projection(
    domain: AnchorDomain,
    asset_b: Option<AssetKind>,
    anchor: &AnchorBucketState<AccountId>,
  ) -> PoolProjection<Balance> {
    let asset_a = AssetKind::Native;
    let Some(asset_b_value) = asset_b else {
      return PoolProjection {
        domain,
        status: GuaranteeStatus::NotInitialized,
        asset_a,
        asset_b: None,
        lp_asset_id: None,
        reserve_a: Zero::zero(),
        reserve_b: Zero::zero(),
        lp_total_issuance: Zero::zero(),
        anchor_lp_balance: Zero::zero(),
      };
    };

    let Ok(pool_id) =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b_value)
    else {
      return Self::missing_pool_projection(domain, asset_a, asset_b_value);
    };
    let Some(pool) = pallet_asset_conversion::Pools::<Runtime>::get(pool_id) else {
      return Self::missing_pool_projection(domain, asset_a, asset_b_value);
    };
    let Ok(pool_account) = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(
      &asset_a,
      &asset_b_value,
    ) else {
      return Self::missing_pool_projection(domain, asset_a, asset_b_value);
    };

    let reserve_a = Self::asset_balance(asset_a, &pool_account);
    let reserve_b = Self::asset_balance(asset_b_value, &pool_account);
    let lp_total_issuance = Assets::total_issuance(pool.lp_token);
    let anchor_lp_balance = Assets::balance(pool.lp_token, &anchor.sovereign_account);
    let status = if reserve_a.is_zero()
      || reserve_b.is_zero()
      || lp_total_issuance.is_zero()
      || anchor_lp_balance.is_zero()
    {
      GuaranteeStatus::NotInitialized
    } else {
      GuaranteeStatus::Satisfied
    };

    PoolProjection {
      domain,
      status,
      asset_a,
      asset_b: Some(asset_b_value),
      lp_asset_id: Some(pool.lp_token),
      reserve_a,
      reserve_b,
      lp_total_issuance,
      anchor_lp_balance,
    }
  }

  fn missing_pool_projection(
    domain: AnchorDomain,
    asset_a: AssetKind,
    asset_b: AssetKind,
  ) -> PoolProjection<Balance> {
    PoolProjection {
      domain,
      status: GuaranteeStatus::NotInitialized,
      asset_a,
      asset_b: Some(asset_b),
      lp_asset_id: None,
      reserve_a: Zero::zero(),
      reserve_b: Zero::zero(),
      lp_total_issuance: Zero::zero(),
      anchor_lp_balance: Zero::zero(),
    }
  }

  fn native_floor_inputs(tol_pool: &PoolProjection<Balance>) -> ReportedFloorInputs<Balance> {
    let curve = pallet_tmc::TokenCurves::<Runtime>::get(AssetKind::Native);
    ReportedFloorInputs {
      status: if curve.is_some() {
        tol_pool.status
      } else {
        GuaranteeStatus::NotInitialized
      },
      curve_exists: curve.is_some(),
      initial_issuance: curve
        .map(|curve| curve.initial_issuance)
        .unwrap_or_default(),
      total_native_minted: pallet_tmc::TotalNativeMinted::<Runtime>::get(),
      current_native_issuance: Balances::total_issuance(),
      anchor_lp_balance: tol_pool.anchor_lp_balance,
      lp_total_issuance: tol_pool.lp_total_issuance,
      reserve_native: tol_pool.reserve_a,
      reserve_counterparty: tol_pool.reserve_b,
    }
  }

  fn native_burn_liveness() -> BurnLivenessState<AccountId, Balance> {
    let actor_id = ecosystem::actor_ids::BURN_ACTOR_ID;
    let target_asset = AssetKind::Native;
    Self::burn_liveness_state(
      BurnDomain::NativeBurnActor,
      actor_id,
      target_asset,
      false,
      |task| matches!(task, Task::Burn { asset, .. } if *asset == target_asset),
      |_task| true,
    )
  }

  fn bldr_buyback_liveness() -> BurnLivenessState<AccountId, Balance> {
    let actor_id = ecosystem::actor_ids::TREASURY_B_ACTORS_ID;
    let target_asset = AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID);
    Self::burn_liveness_state(
      BurnDomain::BldrBuyback,
      actor_id,
      target_asset,
      true,
      |task| matches!(task, Task::Burn { asset, .. } if *asset == target_asset),
      |task| {
        matches!(
          task,
          Task::SwapIn { asset_in, asset_out, .. }
            if *asset_in == AssetKind::Native && *asset_out == target_asset
        )
      },
    )
  }

  fn burn_liveness_state(
    domain: BurnDomain,
    actor_id: u64,
    target_asset: AssetKind,
    requires_swap: bool,
    burn_match: impl Fn(&pallet_deos_actors::TaskOf<Runtime>) -> bool,
    swap_match: impl Fn(&pallet_deos_actors::TaskOf<Runtime>) -> bool,
  ) -> BurnLivenessState<AccountId, Balance> {
    let sovereign_account = crate::Actors::sovereign_account_id_system(actor_id);
    let target_balance = Self::asset_balance(target_asset, &sovereign_account);
    let dust_threshold = ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let maybe_actor = pallet_deos_actors::ActorIdentities::<Runtime>::get(actor_id)
      .zip(pallet_deos_actors::ActorHot::<Runtime>::get(actor_id))
      .zip(pallet_deos_actors::ActorProgram::<Runtime>::get(actor_id));
    let (
      actor_exists,
      is_system,
      is_paused,
      has_address_event_trigger,
      has_required_burn_step,
      has_required_swap_step,
    ) = maybe_actor
      .map(|((identity, hot), program)| {
        let has_address_event_trigger = program.schedule.trigger.address_event_source_enabled();
        let has_required_burn_step = program
          .execution_plan
          .iter()
          .any(|step| burn_match(&step.task));
        let has_required_swap_step = !requires_swap
          || program
            .execution_plan
            .iter()
            .any(|step| swap_match(&step.task));
        (
          true,
          identity.actor_class.actor_type() == ActorType::System,
          hot.lifecycle.is_paused(),
          has_address_event_trigger,
          has_required_burn_step,
          has_required_swap_step,
        )
      })
      .unwrap_or((false, false, false, false, false, !requires_swap));
    let dormant = pallet_deos_actors::ActorIdentities::<Runtime>::contains_key(actor_id)
      && !pallet_deos_actors::ActorHot::<Runtime>::contains_key(actor_id);
    let status = if domain == BurnDomain::BldrBuyback && dormant {
      GuaranteeStatus::NotInitialized
    } else if !actor_exists || !is_system || is_paused || !has_address_event_trigger {
      GuaranteeStatus::Violated
    } else if has_required_burn_step && has_required_swap_step {
      GuaranteeStatus::Satisfied
    } else if domain == BurnDomain::BldrBuyback
      && !has_required_burn_step
      && !has_required_swap_step
    {
      GuaranteeStatus::NotInitialized
    } else {
      GuaranteeStatus::Violated
    };

    BurnLivenessState {
      domain,
      actor_id,
      status,
      sovereign_account,
      actor_exists,
      is_system,
      is_paused,
      has_address_event_trigger,
      requires_swap,
      has_required_swap_step,
      has_required_burn_step,
      target_asset,
      target_balance,
      dust_threshold,
      target_balance_above_dust: target_balance > dust_threshold,
    }
  }

  fn zap_postconditions() -> ZapPostconditionState<AccountId> {
    let actor_id = ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID;
    let sovereign_account = crate::Actors::sovereign_account_id_system(actor_id);
    let maybe_actor = pallet_deos_actors::ActorIdentities::<Runtime>::get(actor_id)
      .zip(pallet_deos_actors::ActorHot::<Runtime>::get(actor_id))
      .zip(pallet_deos_actors::ActorProgram::<Runtime>::get(actor_id));
    let Some(((identity, hot), program)) = maybe_actor else {
      let status = if pallet_deos_actors::ActorIdentities::<Runtime>::contains_key(actor_id) {
        GuaranteeStatus::NotInitialized
      } else {
        GuaranteeStatus::Violated
      };
      return ZapPostconditionState {
        actor_id,
        status,
        sovereign_account,
        actor_exists: false,
        is_system: false,
        is_paused: false,
        has_address_event_trigger: false,
        configured_foreign_asset: None,
        configured_lp_asset: None,
        has_add_liquidity_step: false,
        has_foreign_to_native_swap_step: false,
        has_lp_split_step: false,
        split_targets_all_buckets: false,
        split_shares_sum_to_one: false,
        split_shares_match_policy: false,
      };
    };

    let is_system = identity.actor_class.actor_type() == ActorType::System;
    let has_address_event_trigger = program.schedule.trigger.address_event_source_enabled();
    let mut foreign_from_add: Option<AssetKind> = None;
    let mut foreign_from_swap: Option<AssetKind> = None;
    let mut configured_lp_asset: Option<AssetKind> = None;
    let mut has_add_liquidity_step = false;
    let mut has_foreign_to_native_swap_step = false;
    let mut has_lp_split_step = false;
    let mut split_targets_all_buckets = false;
    let mut split_shares_sum_to_one = false;
    let mut split_shares_match_policy = false;

    for step in &program.execution_plan {
      match &step.task {
        Task::AddLiquidity {
          asset_a, asset_b, ..
        } if *asset_a == AssetKind::Native => {
          has_add_liquidity_step = true;
          foreign_from_add = Some(*asset_b);
        }
        Task::AddLiquidity {
          asset_a, asset_b, ..
        } if *asset_b == AssetKind::Native => {
          has_add_liquidity_step = true;
          foreign_from_add = Some(*asset_a);
        }
        Task::SwapIn {
          asset_in,
          asset_out,
          ..
        } if *asset_out == AssetKind::Native => {
          has_foreign_to_native_swap_step = true;
          foreign_from_swap = Some(*asset_in);
        }
        Task::SplitTransfer { asset, legs, .. } => {
          has_lp_split_step = true;
          configured_lp_asset = Some(*asset);
          let bucket_targets = [
            (
              ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
              ecosystem::params::TOL_BUCKET_A_ALLOCATION,
            ),
            (
              ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
              ecosystem::params::TOL_BUCKET_B_ALLOCATION,
            ),
            (
              ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
              ecosystem::params::TOL_BUCKET_C_ALLOCATION,
            ),
            (
              ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
              ecosystem::params::TOL_BUCKET_D_ALLOCATION,
            ),
          ];
          split_targets_all_buckets = bucket_targets.into_iter().all(|(actor_id, _)| {
            let target = crate::Actors::sovereign_account_id_system(actor_id);
            legs.iter().any(|leg| leg.to == target)
          });
          split_shares_match_policy =
            bucket_targets
              .into_iter()
              .all(|(actor_id, expected_share)| {
                let target = crate::Actors::sovereign_account_id_system(actor_id);
                legs
                  .into_iter()
                  .find(|leg| leg.to == target)
                  .map(|leg| leg.share == expected_share)
                  .unwrap_or(false)
              });
          let share_sum: u32 = legs.iter().map(|leg| leg.share.deconstruct()).sum();
          split_shares_sum_to_one =
            share_sum == polkadot_sdk::sp_runtime::Perbill::one().deconstruct();
        }
        _ => {}
      }
    }

    let configured_foreign_asset = if foreign_from_add == foreign_from_swap {
      foreign_from_add
    } else {
      None
    };
    let structure_satisfied = configured_foreign_asset.is_some()
      && configured_lp_asset.is_some()
      && has_add_liquidity_step
      && has_foreign_to_native_swap_step
      && has_lp_split_step
      && split_targets_all_buckets
      && split_shares_sum_to_one
      && split_shares_match_policy;
    let status = if !is_system || hot.lifecycle.is_paused() || !has_address_event_trigger {
      GuaranteeStatus::Violated
    } else if structure_satisfied {
      GuaranteeStatus::Satisfied
    } else {
      GuaranteeStatus::Violated
    };

    ZapPostconditionState {
      actor_id,
      status,
      sovereign_account,
      actor_exists: true,
      is_system,
      is_paused: hot.lifecycle.is_paused(),
      has_address_event_trigger,
      configured_foreign_asset,
      configured_lp_asset,
      has_add_liquidity_step,
      has_foreign_to_native_swap_step,
      has_lp_split_step,
      split_targets_all_buckets,
      split_shares_sum_to_one,
      split_shares_match_policy,
    }
  }

  fn aggregate_status<const N: usize>(statuses: [GuaranteeStatus; N]) -> GuaranteeStatus {
    if statuses
      .iter()
      .any(|status| *status == GuaranteeStatus::Violated)
    {
      GuaranteeStatus::Violated
    } else if statuses
      .iter()
      .any(|status| *status == GuaranteeStatus::NotInitialized)
    {
      GuaranteeStatus::NotInitialized
    } else if statuses
      .iter()
      .any(|status| *status == GuaranteeStatus::NotGuaranteed)
    {
      GuaranteeStatus::NotGuaranteed
    } else {
      GuaranteeStatus::Satisfied
    }
  }

  fn conformance_status<const N: usize>(statuses: [GuaranteeStatus; N]) -> TmctolConformanceStatus {
    match Self::aggregate_status(statuses) {
      GuaranteeStatus::Satisfied => TmctolConformanceStatus::Conformant,
      GuaranteeStatus::NotInitialized => TmctolConformanceStatus::NotInitialized,
      GuaranteeStatus::NotGuaranteed => TmctolConformanceStatus::NotGuaranteed,
      GuaranteeStatus::Violated => TmctolConformanceStatus::Violated,
    }
  }

  fn asset_balance(asset: AssetKind, account: &AccountId) -> Balance {
    match asset {
      AssetKind::Native => Balances::free_balance(account),
      AssetKind::Local(id) | AssetKind::Foreign(id) => Assets::balance(id, account),
    }
  }
}
