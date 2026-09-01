//! Actors pallet configuration for the DEOS reference runtime.
//!
//! Wires the two adapter traits (`AssetOps`, `DexOps`) to concrete runtime pallets:
//! - Native token: `pallet-balances`
//! - Foreign assets: `pallet-assets`
//! - Swaps: DEOS Router
//! - Liquidity: Asset Conversion

use super::*;
use codec::Encode;
use primitives::{AssetKind, ecosystem};

use polkadot_sdk::frame_support::traits::{
  Currency, Get,
  fungible::{Inspect as NativeInspect, Mutate as NativeMutate},
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
  tokens::{DepositConsequence, Fortitude, Precision, Preservation, Provenance},
};
use polkadot_sdk::pallet_asset_conversion::PoolLocator;
use polkadot_sdk::sp_core::U256;
use polkadot_sdk::{
  sp_runtime::{DispatchError, DispatchResult, Perbill, TokenError},
  sp_weights::Weight,
};

use crate::{AssetConversion, RuntimeOrigin, Timestamp};
use pallet_deos_actors::{
  ActorPrepassContext, ActorType, AdmissionCertificateAuthority,
  AdmissionCertificateAuthorityProvider, AssetOps, DexOps, DexSwapOutcome, ExecutionContext,
  FeeCollector, FundingAuthority, LiquidityOps, StepControlExecution, StepControlOutcome,
  StepControlPhase, StepControlPlacement, StepControlWeightProvider, Task,
  TaskEffectWeightProvider, TaskFailure, WeightInfo as ActorsWeightInfo,
};

parameter_types! {
  // --- Identity and ownership ---

  pub const ActorsPalletId: PalletId = PalletId(*ecosystem::pallet_ids::ACTORS_PALLET_ID);
  pub const ActorFeeNativeAssetId: AssetKind = AssetKind::Native;
  pub const ActorCadenceTickMillis: u64 = ecosystem::params::ACTOR_CADENCE_TICK_MILLIS;
  /// User Actors slot capacity per owner; System Actors is not constrained by this limit
  pub const ActorMaxOwnerSlots: u8 = 255;

  // --- Execution-plan and task bounds ---

  pub const ActorMaxContractSteps: u32 = 12;
  pub const ActorMaxFundingTrackedAssets: u32 = 40;
  pub const ActorMaxOpeningSnapshotEntries: u32 = 24;
  pub const ActorMaxOpeningPredicateResults: u32 = 48;
  pub const ActorMaxPreconditionClauses: u32 = 4;
  pub const ActorMaxPredicatesPerClause: u32 = 4;
  pub const ActorMaxPredicatesPerStep: u32 = 4;
  pub const ActorMaxSplitTransferLegs: u32 = 8;

  // --- Trigger and schedule bounds ---

  pub const ActorTargetBlockTime: u64 = 6;
  pub const ActorMaxExecutionDelayBlocks: BlockNumber = 52_596_000;
  pub const ActorMaxTemporalDelayTicks: u64 = 631_152_000;
  pub const ActorMinWindowLength: BlockNumber = 100;
  pub const ActorMaxWhitelistSize: u32 = 16;

  // --- Scheduler controls ---

  /// Defense-in-depth count ceiling; RefTime and ProofSize admission remain primary.
  pub const ActorMaxExecutionsPerBlock: u32 = 1_000;
  pub const ActorMaxQueueLength: u32 = 10_000;
  /// Balanced production granularity selected from 32/64/128 production-Wasm evidence.
  pub const ActorQueuePageSize: u32 = 64;
  /// Production temporal page granularity selected from 32/64/128 Wasm operation evidence.
  pub const ActorWakeupPageSize: u32 = 32;
  /// Broad ObservationChange subscriber/fanout page granularity.
  pub const ActorObservationPageSize: u32 = 64;
  /// ObservationCrossing membership page granularity selected by bounded observation fanout requirements.
  pub const ActorCrossingPageSize: u32 = 128;
  pub const ActorMaxCrossingTransitionsPerFeed: u32 = 64;
  pub const ActorMaxCrossingMembersPerFeed: u32 = 10_000;
pub const ActorMaxUserCrossingMembersPerFeed: u32 = 9_000;
pub const ActorMaxCrossingTransitionsPerBlock: u32 = 8;
  pub const ActorMaxCrossingLeavesPerBlock: u32 = 64;
  pub const ActorMaxCrossingPagesPerBlock: u32 = 64;
  /// Accepted tail/preflight ceiling under Actor Control. Exact non-tail compaction is
  /// independently clamped to 64 candidates by the pallet.
  pub const ActorMaxCrossingActorsPerBlock: u32 = 128;
  pub const ActorMaxQueueEntriesScannedPerBlock: u32 = 10_000;
  pub const ActorMaxObservationFanoutPagesPerBlock: u32 = 64;
  pub const ActorMaxWakeupsPerBlock: u32 = 512;
  pub ActorObservationFanoutWeightLimit: Weight =
    Perbill::from_percent(20) * MAXIMUM_BLOCK_WEIGHT;
  pub ActorCrossingWorkerWeightLimit: Weight =
    Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT;
  /// Dedicated overdue-wakeup worker envelope: one worst-case complete wakeup unit plus cursor
  /// probe remains inside it (spec 15.2.9), and it stays below the guaranteed on_idle headroom.
  pub ActorWakeupWeightLimit: Weight = Perbill::from_percent(14) * MAXIMUM_BLOCK_WEIGHT;
  pub ActorOnIdleReserve: Weight =
    MIN_ON_IDLE_RESERVE_RATIO * MAXIMUM_BLOCK_WEIGHT;
  /// Mandatory cutoff capture is Actor-specific control work, never fixed-context work.
  pub ActorControlInitializationWeight: Weight =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::scheduler_on_initialize_cutoff();
  // --- Lifecycle and sweep controls ---

  pub const ActorMaxConsecutiveFailures: u32 = 10;
  pub const ActorMaxRetryAttempts: u32 = 10;
  pub const ActorMaxAutoCloseNonceHorizon: u64 = 10_000;
  pub const ActorMinUserBalance: Balance = 5 * ExistentialDeposit::get();
  pub const ActorMaxSweepBatch: u32 = 5;

  // --- Starvation safeguard controls ---

  pub const ActorMaxIdleStarvationBlocks: u32 = 25;
  /// Maximum number of active Actors instances. Bounds the BTreeSet storage.
  /// Set to 10,000 for production use cases with high automation density.
  pub const ActorMaxActiveActors: u32 = 10_000;
  // --- Economic parameters ---

  pub const ActorMaxSystemPriceDeviation: Perbill =
    ecosystem::params::MAX_SYSTEM_PRICE_DEVIATION;
  pub const ActorMaxSystemReferenceAgeBlocks: u32 =
    ecosystem::params::MAX_SYSTEM_REFERENCE_AGE_BLOCKS;

  /// Non-refundable opening fee routed to `FeeSink`.
  pub const ActorCreationFee: Balance = 2 * ExistentialDeposit::get();
  /// Fixed accounting owner for each present Actor-state component.
  pub const ActorStateHoldBase: Balance = ExistentialDeposit::get();
  /// Linear refundable price for each retained SCALE byte.
  pub const ActorStateHoldPerByte: Balance = MICRO_UNIT;
}

pub struct ActorMinUserBalanceGuard;

impl Get<Balance> for ActorMinUserBalanceGuard {
  fn get() -> Balance {
    ActorMinUserBalance::get().max(ExistentialDeposit::get())
  }
}

/// Canonical unified fee-collection boundary for Actors charges.
///
/// The collector transfers every opening, evaluation, and execution fee in full to the Fee Sink
/// System Actors. Phase-specific allocation happens later through that actor's bounded
/// execution plan rather than inside the collection path.
pub struct TmctolFeeCollector;

impl FeeCollector<AccountId, AssetKind, Balance> for TmctolFeeCollector {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    _native_asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult {
    if amount == 0 {
      return Ok(());
    }
    TmctolAssetOps::transfer_native_ledger_only(payer, fee_sink, amount)
      .map_err(|failure| failure.error)
  }
}

pub struct ActorFeeRecipient;
impl Get<crate::AccountId> for ActorFeeRecipient {
  fn get() -> crate::AccountId {
    crate::Actors::sovereign_account_id_system(ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
  }
}

pub struct RuntimeStepControlWeight;

impl RuntimeStepControlWeight {
  fn component_max(left: Weight, right: Weight) -> Weight {
    Weight::from_parts(
      left.ref_time().max(right.ref_time()),
      left.proof_size().max(right.proof_size()),
    )
  }

  fn nonzero_parameterized(value: u32, weight: impl FnOnce(u32) -> Weight) -> Weight {
    if value == 0 {
      Weight::zero()
    } else {
      weight(value)
    }
  }
}

impl StepControlWeightProvider<pallet_deos_actors::StepOf<Runtime>> for RuntimeStepControlWeight {
  fn production_weight_identity() -> Option<[u8; 32]> {
    type ControlWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let tail_plan = (1..=4)
      .map(ControlWeights::current_step_plan_running_tail)
      .collect::<alloc::vec::Vec<_>>();
    let maximum_opening_tail_chunks = ActorMaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
    let opening_failed = (0..=maximum_opening_tail_chunks)
      .map(|tail_chunks| {
        (
          ControlWeights::scheduler_inner_opening_failed_min(tail_chunks),
          ControlWeights::scheduler_inner_opening_failed_max(tail_chunks),
        )
      })
      .collect::<alloc::vec::Vec<_>>();
    let opening_retry = (0..=maximum_opening_tail_chunks)
      .map(|tail_chunks| {
        (
          ControlWeights::scheduler_inner_opening_retry_min(tail_chunks),
          ControlWeights::scheduler_inner_opening_retry_max(tail_chunks),
        )
      })
      .collect::<alloc::vec::Vec<_>>();
    let opening_complete = (0..=maximum_opening_tail_chunks)
      .map(|tail_chunks| {
        (
          ControlWeights::scheduler_inner_opening_complete_min(tail_chunks),
          ControlWeights::scheduler_inner_opening_complete_max(tail_chunks),
        )
      })
      .collect::<alloc::vec::Vec<_>>();
    let opening_progress = (1..=maximum_opening_tail_chunks)
      .map(|tail_chunks| {
        (
          ControlWeights::scheduler_inner_opening_progress_min(tail_chunks),
          ControlWeights::scheduler_inner_opening_progress_max(tail_chunks),
        )
      })
      .collect::<alloc::vec::Vec<_>>();
    let running_complete = (1..=4)
      .flat_map(|steps| {
        (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
          ControlWeights::scheduler_inner_running_complete(steps, predicates)
        })
      })
      .collect::<alloc::vec::Vec<_>>();
    let running_progress = (2..=4)
      .flat_map(|steps| {
        (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
          ControlWeights::scheduler_inner_running_progress(steps, predicates)
        })
      })
      .collect::<alloc::vec::Vec<_>>();
    let suspended_tail_retry = (1..=4)
      .flat_map(|steps| {
        (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
          ControlWeights::scheduler_inner_suspended_tail_retry(steps, predicates)
        })
      })
      .collect::<alloc::vec::Vec<_>>();
    let suspended_tail_complete = (1..=4)
      .flat_map(|steps| {
        (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
          ControlWeights::scheduler_inner_suspended_tail_complete(steps, predicates)
        })
      })
      .collect::<alloc::vec::Vec<_>>();
    let suspended_tail_progress = (2..=4)
      .flat_map(|steps| {
        (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
          ControlWeights::scheduler_inner_suspended_tail_progress(steps, predicates)
        })
      })
      .collect::<alloc::vec::Vec<_>>();
    let suspended_head_retry = [
      ControlWeights::scheduler_inner_suspended_head_retry(0, 0, 0, 0),
      ControlWeights::scheduler_inner_suspended_head_retry(
        ActorMaxOpeningSnapshotEntries::get(),
        0,
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_retry(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_retry(
        0,
        0,
        ActorMaxFundingTrackedAssets::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_retry(
        0,
        0,
        0,
        ActorMaxPredicatesPerStep::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_retry(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
        ActorMaxPredicatesPerStep::get(),
      ),
    ];
    let suspended_head_complete = [
      ControlWeights::scheduler_inner_suspended_head_complete(0, 0, 0, 0),
      ControlWeights::scheduler_inner_suspended_head_complete(
        ActorMaxOpeningSnapshotEntries::get(),
        0,
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_complete(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_complete(
        0,
        0,
        ActorMaxFundingTrackedAssets::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_complete(
        0,
        0,
        0,
        ActorMaxPredicatesPerStep::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_complete(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
        ActorMaxPredicatesPerStep::get(),
      ),
    ];
    let suspended_head_progress = [
      ControlWeights::scheduler_inner_suspended_head_progress(0, 0, 0, 0),
      ControlWeights::scheduler_inner_suspended_head_progress(
        ActorMaxOpeningSnapshotEntries::get(),
        0,
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_progress(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_progress(
        0,
        0,
        ActorMaxFundingTrackedAssets::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_progress(
        0,
        0,
        0,
        ActorMaxPredicatesPerStep::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_progress(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
        ActorMaxPredicatesPerStep::get(),
      ),
    ];
    let suspended_head_opening_retry = [
      ControlWeights::scheduler_inner_suspended_head_opening_retry(0, 1, 0),
      ControlWeights::scheduler_inner_suspended_head_opening_retry(
        ActorMaxOpeningSnapshotEntries::get(),
        1,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_retry(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_retry(
        0,
        1,
        ActorMaxFundingTrackedAssets::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_retry(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
      ),
    ];
    let suspended_head_opening_complete = [
      ControlWeights::scheduler_inner_suspended_head_opening_complete(0, 1, 0),
      ControlWeights::scheduler_inner_suspended_head_opening_complete(
        ActorMaxOpeningSnapshotEntries::get(),
        1,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_complete(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_complete(
        0,
        1,
        ActorMaxFundingTrackedAssets::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_complete(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
      ),
    ];
    let suspended_head_opening_progress = [
      ControlWeights::scheduler_inner_suspended_head_opening_progress(0, 1, 0),
      ControlWeights::scheduler_inner_suspended_head_opening_progress(
        ActorMaxOpeningSnapshotEntries::get(),
        1,
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_progress(
        0,
        ActorMaxOpeningPredicateResults::get(),
        0,
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_progress(
        0,
        1,
        ActorMaxFundingTrackedAssets::get(),
      ),
      ControlWeights::scheduler_inner_suspended_head_opening_progress(
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxFundingTrackedAssets::get(),
      ),
    ];
    let opening_tail_chunks = (0..=ActorMaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK))
      .map(ControlWeights::contract_geometry_reconstruct)
      .collect::<alloc::vec::Vec<_>>();
    let opening_snapshot = (1..=ActorMaxOpeningSnapshotEntries::get())
      .map(ControlWeights::opening_snapshot_capture)
      .collect::<alloc::vec::Vec<_>>();
    let opening_predicates = (1..=ActorMaxOpeningPredicateResults::get())
      .map(ControlWeights::opening_predicate_capture)
      .collect::<alloc::vec::Vec<_>>();
    let funding = (1..=ActorMaxFundingTrackedAssets::get())
      .map(ControlWeights::funding_snapshot_open)
      .collect::<alloc::vec::Vec<_>>();
    let current_predicates = (1..=ActorMaxPredicatesPerStep::get())
      .map(ControlWeights::predicate_set_evaluation)
      .collect::<alloc::vec::Vec<_>>();
    Some(
      (
        *b"DEOS_ACTOR_STEP_CONTROL_WEIGHT",
        *b"DEOS_ACTOR_CONTROL_ACTUAL_V3",
        (
          ControlWeights::scheduler_paged_tombstone_drain(1),
          ControlWeights::scheduler_actor_state_probe(),
          ControlWeights::scheduler_paged_consume_preserve_page(),
          ControlWeights::scheduler_paged_consume_delete_page(),
        ),
        (
          ControlWeights::scheduler_inner_zero_step_complete(),
          ControlWeights::scheduler_paged_execute_opening_max(),
          opening_failed,
          opening_retry,
          opening_complete,
          opening_progress,
        ),
        ControlWeights::current_step_plan_opening_head(),
        ControlWeights::current_step_plan_suspended_head(),
        tail_plan,
        (
          running_complete,
          running_progress,
          suspended_tail_retry,
          suspended_tail_complete,
          suspended_tail_progress,
          suspended_head_retry,
          suspended_head_complete,
          suspended_head_progress,
          suspended_head_opening_retry,
          suspended_head_opening_complete,
          suspended_head_opening_progress,
        ),
        opening_tail_chunks,
        opening_snapshot,
        opening_predicates,
        funding,
        current_predicates,
        ControlWeights::run_progress(),
        ControlWeights::run_suspend(),
        ControlWeights::run_complete(),
        ControlWeights::scheduler_paged_append_new_page(),
        (
          ControlWeights::scheduler_wakeup_append_new_page(),
          ControlWeights::close_actor(),
          ControlWeights::fee_collection(),
        ),
      )
        .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256),
    )
  }

  fn maximum_control_weight(
    context: pallet_deos_actors::StepControlWeightContext,
    step: &pallet_deos_actors::StepOf<Runtime>,
  ) -> Option<Weight> {
    Self::base_maximum_control_weight(context, step)?
      .checked_add(&Self::action_collection_allowance(step))
  }

  fn actual_control_weight(
    context: pallet_deos_actors::StepControlWeightContext,
    step: &pallet_deos_actors::StepOf<Runtime>,
    maximum: Weight,
    execution: StepControlExecution,
  ) -> Option<Weight> {
    let allowance = Self::action_collection_allowance(step);
    if execution.action_fee_collected && allowance == Weight::zero() {
      return None;
    }
    let base =
      Self::base_actual_control_weight(context, step, maximum.checked_sub(&allowance)?, execution)?;
    let actual = base.checked_add(&if execution.action_fee_collected {
      allowance
    } else {
      Weight::zero()
    })?;
    actual.all_lte(maximum).then_some(actual)
  }
}

impl RuntimeStepControlWeight {
  fn action_collection_allowance(step: &pallet_deos_actors::StepOf<Runtime>) -> Weight {
    if matches!(step.task, Task::StopCycle) {
      Weight::zero()
    } else {
      crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::fee_collection()
    }
  }

  fn base_maximum_control_weight(
    context: pallet_deos_actors::StepControlWeightContext,
    _: &pallet_deos_actors::StepOf<Runtime>,
  ) -> Option<Weight> {
    type ControlWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let maximum_evaluation_units = ActorMaxPredicatesPerStep::get().checked_mul(2)?;
    let maximum_opening_tail_chunks = ActorMaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
    if context.predicate_evaluation_units > maximum_evaluation_units
      || context.opening_tail_chunks > maximum_opening_tail_chunks
      || context.cursor > 0 && context.opening_tail_chunks != 0
      || context.opening_snapshot_entries > ActorMaxOpeningSnapshotEntries::get()
      || context.opening_predicate_results > ActorMaxOpeningPredicateResults::get()
      || context.funding_snapshot_entries > ActorMaxFundingTrackedAssets::get()
    {
      return None;
    }
    let maximum_opening = context.cursor == 0
      && context.steps_in_fragment == 1
      && context.opening_tail_chunks == maximum_opening_tail_chunks
      && context.predicate_evaluation_units == maximum_evaluation_units
      && context.opening_snapshot_entries == ActorMaxOpeningSnapshotEntries::get()
      && context.opening_predicate_results == ActorMaxOpeningPredicateResults::get()
      && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get();
    let plan = if context.cursor == 0 {
      if context.steps_in_fragment != 1 {
        return None;
      }
      Self::component_max(
        ControlWeights::current_step_plan_opening_head(),
        ControlWeights::current_step_plan_suspended_head(),
      )
    } else {
      if context.steps_in_fragment == 0 || context.steps_in_fragment > 4 {
        return None;
      }
      ControlWeights::current_step_plan_running_tail(context.steps_in_fragment)
    };
    let opening_tail = if context.opening_tail_chunks == 0 {
      Weight::zero()
    } else {
      ControlWeights::contract_geometry_reconstruct(context.opening_tail_chunks)
        .checked_sub(&ControlWeights::contract_geometry_reconstruct(0))?
    };
    let opening_snapshot = Self::nonzero_parameterized(
      context.opening_snapshot_entries,
      ControlWeights::opening_snapshot_capture,
    );
    let opening_predicates = Self::nonzero_parameterized(
      context.opening_predicate_results,
      ControlWeights::opening_predicate_capture,
    );
    let funding = Self::nonzero_parameterized(
      context.funding_snapshot_entries,
      ControlWeights::funding_snapshot_open,
    );
    let current_predicates = if context.predicate_evaluation_units == 0 {
      Weight::zero()
    } else {
      ControlWeights::predicate_set_evaluation(
        context
          .predicate_evaluation_units
          .min(ActorMaxPredicatesPerStep::get()),
      )
    };
    let commit = Self::component_max(
      Self::component_max(
        ControlWeights::run_progress(),
        ControlWeights::run_suspend(),
      ),
      ControlWeights::run_complete(),
    );
    let placement = Self::component_max(
      ControlWeights::scheduler_paged_append_new_page(),
      ControlWeights::scheduler_wakeup_append_new_page(),
    );
    let composed = plan
      .saturating_add(opening_tail)
      .saturating_add(opening_snapshot)
      .saturating_add(opening_predicates)
      .saturating_add(funding)
      .saturating_add(current_predicates)
      .saturating_add(commit)
      .saturating_add(placement);
    let opening_geometry_steps = 1u32
      .saturating_add(
        context
          .opening_tail_chunks
          .saturating_mul(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
      )
      .min(ActorMaxContractSteps::get());
    let minimal_opening = context.cursor == 0
      && context.predicate_evaluation_units == 0
      && context.opening_snapshot_entries == 0
      && context.opening_predicate_results == 0;
    let maximal_opening = context.cursor == 0
      && context.opening_tail_chunks > 0
      && context.predicate_evaluation_units == maximum_evaluation_units
      && context.opening_snapshot_entries
        == opening_geometry_steps
          .saturating_mul(2)
          .min(ActorMaxOpeningSnapshotEntries::get())
      && context.opening_predicate_results
        == opening_geometry_steps
          .saturating_mul(ActorMaxPredicatesPerStep::get())
          .min(ActorMaxOpeningPredicateResults::get())
      && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get();
    let maximal_opening_completion = context.cursor == 0
      && context.predicate_evaluation_units == maximum_evaluation_units
      && context.opening_snapshot_entries
        == opening_geometry_steps
          .saturating_sub(1)
          .saturating_mul(2)
          .min(ActorMaxOpeningSnapshotEntries::get())
      && context.opening_predicate_results
        == opening_geometry_steps
          .saturating_mul(ActorMaxPredicatesPerStep::get())
          .min(ActorMaxOpeningPredicateResults::get())
      && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get();
    let composed = if minimal_opening {
      let non_progress_min = Self::component_max(
        ControlWeights::scheduler_inner_opening_failed_min(context.opening_tail_chunks),
        Self::component_max(
          ControlWeights::scheduler_inner_opening_retry_min(context.opening_tail_chunks),
          ControlWeights::scheduler_inner_opening_complete_min(context.opening_tail_chunks),
        ),
      );
      let direct = if context.opening_tail_chunks == 0 {
        non_progress_min
      } else {
        Self::component_max(
          non_progress_min,
          ControlWeights::scheduler_inner_opening_progress_min(context.opening_tail_chunks),
        )
      };
      Self::component_max(composed, direct)
    } else if maximal_opening {
      Self::component_max(
        composed,
        Self::component_max(
          ControlWeights::scheduler_inner_opening_progress_max(context.opening_tail_chunks),
          if maximum_opening {
            ControlWeights::scheduler_paged_execute_opening_max()
          } else {
            Weight::zero()
          },
        ),
      )
    } else if maximal_opening_completion {
      Self::component_max(
        composed,
        Self::component_max(
          ControlWeights::scheduler_inner_opening_failed_max(context.opening_tail_chunks),
          Self::component_max(
            ControlWeights::scheduler_inner_opening_retry_max(context.opening_tail_chunks),
            ControlWeights::scheduler_inner_opening_complete_max(context.opening_tail_chunks),
          ),
        ),
      )
    } else {
      composed
    };
    if context.cursor == 0 && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      let retry = ControlWeights::scheduler_inner_suspended_head_retry(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      );
      let complete = ControlWeights::scheduler_inner_suspended_head_complete(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      );
      let progress = ControlWeights::scheduler_inner_suspended_head_progress(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      );
      return Some(Self::component_max(
        composed,
        Self::component_max(retry, Self::component_max(complete, progress)),
      ));
    }
    if context.cursor == 0
      && context.predicate_evaluation_units > ActorMaxPredicatesPerStep::get()
      && context.predicate_evaluation_units <= maximum_evaluation_units
    {
      let opening_retry = ControlWeights::scheduler_inner_suspended_head_opening_retry(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
      );
      let opening_complete = ControlWeights::scheduler_inner_suspended_head_opening_complete(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
      );
      let opening_progress = ControlWeights::scheduler_inner_suspended_head_opening_progress(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
      );
      return Some(Self::component_max(
        composed,
        Self::component_max(
          opening_retry,
          Self::component_max(opening_complete, opening_progress),
        ),
      ));
    }
    if context.cursor > 0 && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      let atomic_complete = ControlWeights::scheduler_inner_running_complete(
        context.steps_in_fragment,
        context.predicate_evaluation_units,
      );
      let atomic = if context.steps_in_fragment >= 2 {
        Self::component_max(
          atomic_complete,
          ControlWeights::scheduler_inner_running_progress(
            context.steps_in_fragment,
            context.predicate_evaluation_units,
          ),
        )
      } else {
        atomic_complete
      };
      let atomic = Self::component_max(
        atomic,
        ControlWeights::scheduler_inner_suspended_tail_retry(
          context.steps_in_fragment,
          context.predicate_evaluation_units,
        ),
      );
      let atomic = Self::component_max(
        atomic,
        ControlWeights::scheduler_inner_suspended_tail_complete(
          context.steps_in_fragment,
          context.predicate_evaluation_units,
        ),
      );
      let atomic = if context.steps_in_fragment >= 2 {
        Self::component_max(
          atomic,
          ControlWeights::scheduler_inner_suspended_tail_progress(
            context.steps_in_fragment,
            context.predicate_evaluation_units,
          ),
        )
      } else {
        atomic
      };
      return Some(Self::component_max(composed, atomic));
    }
    Some(composed)
  }

  fn base_actual_control_weight(
    context: pallet_deos_actors::StepControlWeightContext,
    step: &pallet_deos_actors::StepOf<Runtime>,
    maximum: Weight,
    execution: StepControlExecution,
  ) -> Option<Weight> {
    type ControlWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let expected_context_maximum = Self::base_maximum_control_weight(context, step)?;
    if execution.phase == StepControlPhase::Opening && expected_context_maximum != maximum {
      return None;
    }
    let maximum_evaluation_units = ActorMaxPredicatesPerStep::get().checked_mul(2)?;
    let maximum_opening_tail_chunks = ActorMaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
    let maximum_opening = context.cursor == 0
      && context.steps_in_fragment == 1
      && context.opening_tail_chunks == maximum_opening_tail_chunks
      && context.predicate_evaluation_units == maximum_evaluation_units
      && context.opening_snapshot_entries == ActorMaxOpeningSnapshotEntries::get()
      && context.opening_predicate_results == ActorMaxOpeningPredicateResults::get()
      && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get();
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Failed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.predicate_evaluation_units == 0
      && context.opening_snapshot_entries == 0
      && context.opening_predicate_results == 0
    {
      return Some(ControlWeights::scheduler_inner_opening_failed_min(
        context.opening_tail_chunks,
      ));
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Suspended
      && execution.placement == StepControlPlacement::Wakeup
      && context.cursor == 0
      && context.predicate_evaluation_units == 0
      && context.opening_snapshot_entries == 0
      && context.opening_predicate_results == 0
    {
      return Some(ControlWeights::scheduler_inner_opening_retry_min(
        context.opening_tail_chunks,
      ));
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Completed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.predicate_evaluation_units == 0
      && context.opening_snapshot_entries == 0
      && context.opening_predicate_results == 0
    {
      return Some(ControlWeights::scheduler_inner_opening_complete_min(
        context.opening_tail_chunks,
      ));
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Continued
      && execution.placement == StepControlPlacement::Queue
      && context.cursor == 0
      && context.opening_tail_chunks > 0
    {
      let opening_geometry_steps = 1u32
        .saturating_add(
          context
            .opening_tail_chunks
            .saturating_mul(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
        )
        .min(ActorMaxContractSteps::get());
      if context.predicate_evaluation_units == 0
        && context.opening_snapshot_entries == 0
        && context.opening_predicate_results == 0
      {
        return Some(ControlWeights::scheduler_inner_opening_progress_min(
          context.opening_tail_chunks,
        ));
      }
      if context.predicate_evaluation_units == maximum_evaluation_units
        && context.opening_snapshot_entries
          == opening_geometry_steps
            .saturating_mul(2)
            .min(ActorMaxOpeningSnapshotEntries::get())
        && context.opening_predicate_results
          == opening_geometry_steps
            .saturating_mul(ActorMaxPredicatesPerStep::get())
            .min(ActorMaxOpeningPredicateResults::get())
        && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get()
      {
        // The direct profile does not bound independent amount/predicate sources.
        // Retain the admitted composed envelope until complete-path coverage is regenerated.
        return Some(maximum);
      }
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Failed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.predicate_evaluation_units == maximum_evaluation_units
    {
      let opening_geometry_steps = 1u32
        .saturating_add(
          context
            .opening_tail_chunks
            .saturating_mul(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
        )
        .min(ActorMaxContractSteps::get());
      if context.opening_snapshot_entries
        == opening_geometry_steps
          .saturating_sub(1)
          .saturating_mul(2)
          .min(ActorMaxOpeningSnapshotEntries::get())
        && context.opening_predicate_results
          == opening_geometry_steps
            .saturating_mul(ActorMaxPredicatesPerStep::get())
            .min(ActorMaxOpeningPredicateResults::get())
        && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get()
      {
        return Some(ControlWeights::scheduler_inner_opening_failed_max(
          context.opening_tail_chunks,
        ));
      }
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Suspended
      && execution.placement == StepControlPlacement::Wakeup
      && context.cursor == 0
      && context.predicate_evaluation_units == maximum_evaluation_units
    {
      let opening_geometry_steps = 1u32
        .saturating_add(
          context
            .opening_tail_chunks
            .saturating_mul(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
        )
        .min(ActorMaxContractSteps::get());
      if context.opening_snapshot_entries
        == opening_geometry_steps
          .saturating_sub(1)
          .saturating_mul(2)
          .min(ActorMaxOpeningSnapshotEntries::get())
        && context.opening_predicate_results
          == opening_geometry_steps
            .saturating_mul(ActorMaxPredicatesPerStep::get())
            .min(ActorMaxOpeningPredicateResults::get())
        && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get()
      {
        return Some(ControlWeights::scheduler_inner_opening_retry_max(
          context.opening_tail_chunks,
        ));
      }
    }
    if execution.phase == StepControlPhase::Opening
      && execution.outcome == StepControlOutcome::Completed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.predicate_evaluation_units == maximum_evaluation_units
    {
      let opening_geometry_steps = 1u32
        .saturating_add(
          context
            .opening_tail_chunks
            .saturating_mul(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
        )
        .min(ActorMaxContractSteps::get());
      if context.opening_snapshot_entries
        == opening_geometry_steps
          .saturating_sub(1)
          .saturating_mul(2)
          .min(ActorMaxOpeningSnapshotEntries::get())
        && context.opening_predicate_results
          == opening_geometry_steps
            .saturating_mul(ActorMaxPredicatesPerStep::get())
            .min(ActorMaxOpeningPredicateResults::get())
        && context.funding_snapshot_entries == ActorMaxFundingTrackedAssets::get()
      {
        return Some(ControlWeights::scheduler_inner_opening_complete_max(
          context.opening_tail_chunks,
        ));
      }
    }
    if maximum_opening && execution.phase == StepControlPhase::Opening {
      return Some(maximum);
    }
    if execution.phase == StepControlPhase::Running
      && context.cursor > 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      if execution.outcome == StepControlOutcome::Completed
        && execution.placement == StepControlPlacement::None
      {
        return Some(ControlWeights::scheduler_inner_running_complete(
          context.steps_in_fragment,
          context.predicate_evaluation_units,
        ));
      }
      if execution.outcome == StepControlOutcome::Continued
        && execution.placement == StepControlPlacement::Queue
        && context.steps_in_fragment >= 2
      {
        return Some(ControlWeights::scheduler_inner_running_progress(
          context.steps_in_fragment,
          context.predicate_evaluation_units,
        ));
      }
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Continued
      && execution.placement == StepControlPlacement::Queue
      && context.cursor == 0
      && context.opening_tail_chunks > 0
      && context.predicate_evaluation_units > ActorMaxPredicatesPerStep::get()
      && context.predicate_evaluation_units <= maximum_evaluation_units
    {
      return Some(
        ControlWeights::scheduler_inner_suspended_head_opening_progress(
          context.opening_snapshot_entries,
          context.opening_predicate_results,
          context.funding_snapshot_entries,
        ),
      );
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Completed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.opening_tail_chunks == 0
      && context.predicate_evaluation_units > ActorMaxPredicatesPerStep::get()
      && context.predicate_evaluation_units <= maximum_evaluation_units
    {
      return Some(
        ControlWeights::scheduler_inner_suspended_head_opening_complete(
          context.opening_snapshot_entries,
          context.opening_predicate_results,
          context.funding_snapshot_entries,
        ),
      );
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Suspended
      && execution.placement == StepControlPlacement::Wakeup
      && context.cursor == 0
      && context.predicate_evaluation_units > ActorMaxPredicatesPerStep::get()
      && context.predicate_evaluation_units <= maximum_evaluation_units
    {
      return Some(
        ControlWeights::scheduler_inner_suspended_head_opening_retry(
          context.opening_snapshot_entries,
          context.opening_predicate_results,
          context.funding_snapshot_entries,
        ),
      );
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Continued
      && execution.placement == StepControlPlacement::Queue
      && context.cursor == 0
      && context.opening_tail_chunks > 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_head_progress(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      ));
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Completed
      && execution.placement == StepControlPlacement::None
      && context.cursor == 0
      && context.opening_tail_chunks == 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_head_complete(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      ));
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Suspended
      && execution.placement == StepControlPlacement::Wakeup
      && context.cursor == 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_head_retry(
        context.opening_snapshot_entries,
        context.opening_predicate_results,
        context.funding_snapshot_entries,
        context.predicate_evaluation_units,
      ));
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Completed
      && execution.placement == StepControlPlacement::None
      && context.cursor > 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_tail_complete(
        context.steps_in_fragment,
        context.predicate_evaluation_units,
      ));
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Continued
      && execution.placement == StepControlPlacement::Queue
      && context.cursor > 0
      && context.steps_in_fragment >= 2
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_tail_progress(
        context.steps_in_fragment,
        context.predicate_evaluation_units,
      ));
    }
    if execution.phase == StepControlPhase::Suspended
      && execution.outcome == StepControlOutcome::Suspended
      && execution.placement == StepControlPlacement::Wakeup
      && context.cursor > 0
      && context.predicate_evaluation_units <= ActorMaxPredicatesPerStep::get()
    {
      return Some(ControlWeights::scheduler_inner_suspended_tail_retry(
        context.steps_in_fragment,
        context.predicate_evaluation_units,
      ));
    }
    let plan = match execution.phase {
      StepControlPhase::Opening if context.cursor == 0 => {
        ControlWeights::current_step_plan_opening_head()
      }
      StepControlPhase::Suspended if context.cursor == 0 => {
        ControlWeights::current_step_plan_suspended_head()
      }
      StepControlPhase::Running | StepControlPhase::Suspended if context.cursor > 0 => {
        ControlWeights::current_step_plan_running_tail(context.steps_in_fragment)
      }
      _ => return None,
    };
    let opening_tail =
      if execution.phase != StepControlPhase::Opening || context.opening_tail_chunks == 0 {
        Weight::zero()
      } else {
        ControlWeights::contract_geometry_reconstruct(context.opening_tail_chunks)
          .checked_sub(&ControlWeights::contract_geometry_reconstruct(0))?
      };
    let opening_snapshot = if execution.phase == StepControlPhase::Opening {
      Self::nonzero_parameterized(
        context.opening_snapshot_entries,
        ControlWeights::opening_snapshot_capture,
      )
    } else {
      Weight::zero()
    };
    let opening_predicates = if execution.phase == StepControlPhase::Opening {
      Self::nonzero_parameterized(
        context.opening_predicate_results,
        ControlWeights::opening_predicate_capture,
      )
    } else {
      Weight::zero()
    };
    let funding = if execution.phase == StepControlPhase::Opening {
      Self::nonzero_parameterized(
        context.funding_snapshot_entries,
        ControlWeights::funding_snapshot_open,
      )
    } else {
      Weight::zero()
    };
    let current_predicates = Self::nonzero_parameterized(
      context
        .predicate_evaluation_units
        .min(ActorMaxPredicatesPerStep::get()),
      ControlWeights::predicate_set_evaluation,
    );
    let commit = match execution.outcome {
      StepControlOutcome::Continued => ControlWeights::run_progress(),
      StepControlOutcome::Suspended => ControlWeights::run_suspend(),
      StepControlOutcome::Completed | StepControlOutcome::Failed => ControlWeights::run_complete(),
    };
    let placement = match execution.placement {
      StepControlPlacement::None => Weight::zero(),
      StepControlPlacement::Queue => ControlWeights::scheduler_paged_append_new_page(),
      StepControlPlacement::Wakeup => ControlWeights::scheduler_wakeup_append_new_page(),
    };
    Some(
      plan
        .saturating_add(opening_tail)
        .saturating_add(opening_snapshot)
        .saturating_add(opening_predicates)
        .saturating_add(funding)
        .saturating_add(current_predicates)
        .saturating_add(commit)
        .saturating_add(placement),
    )
  }
}

pub struct TmctolTaskEffectWeight;

impl<AssetId, Balance, AccountId, MaxSplitTransferLegs>
  TaskEffectWeightProvider<Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>>
  for TmctolTaskEffectWeight
where
  MaxSplitTransferLegs: Get<u32>,
{
  fn production_weight_identity() -> Option<[u8; 32]> {
    type EffectWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let split_transfer = (0..=ActorMaxSplitTransferLegs::get())
      .map(EffectWeights::task_split_transfer)
      .collect::<alloc::vec::Vec<_>>();
    Some(
      (
        *b"DEOS_ACTOR_TASK_EFFECT_WEIGHT",
        *b"DEOS_ACTOR_EFFECT_ACTUAL_V1",
        EffectWeights::task_transfer(),
        split_transfer,
        EffectWeights::task_dex_exact_in(),
        EffectWeights::task_dex_exact_out(),
        EffectWeights::task_add_liquidity(),
        EffectWeights::task_remove_liquidity(),
        EffectWeights::task_donate_liquidity(),
        EffectWeights::task_burn(),
        EffectWeights::task_mint(),
        EffectWeights::task_stake(),
        EffectWeights::task_unstake(),
        Weight::zero(),
      )
        .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256),
    )
  }

  fn maximum_effect_weight(
    task: &Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
  ) -> Option<Weight> {
    type EffectWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    match task {
      Task::Transfer { .. } => Some(EffectWeights::task_transfer()),
      Task::SplitTransfer { legs, .. } => Some(EffectWeights::task_split_transfer(
        u32::try_from(legs.len()).ok()?,
      )),
      Task::SwapIn { .. } => Some(EffectWeights::task_dex_exact_in()),
      Task::SwapOut { .. } => Some(EffectWeights::task_dex_exact_out()),
      Task::AddLiquidity { .. } => Some(EffectWeights::task_add_liquidity()),
      Task::RemoveLiquidity { .. } => Some(EffectWeights::task_remove_liquidity()),
      Task::Burn { .. } => Some(EffectWeights::task_burn()),
      Task::Mint { .. } => Some(EffectWeights::task_mint()),
      Task::Stake { .. } => Some(EffectWeights::task_stake()),
      Task::DonateLiquidity { .. } => Some(EffectWeights::task_donate_liquidity()),
      Task::Unstake { .. } => Some(EffectWeights::task_unstake()),
      Task::StopCycle => Some(Weight::zero()),
    }
  }

  fn actual_effect_weight(
    task: &Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
    execution: pallet_deos_actors::TaskEffectExecution,
  ) -> Option<Weight> {
    match execution {
      pallet_deos_actors::TaskEffectExecution::NotInvoked => Some(Weight::zero()),
      pallet_deos_actors::TaskEffectExecution::Invoked => Self::maximum_effect_weight(task),
    }
  }
}

pub struct RuntimeAdmissionCertificateAuthority;

impl AdmissionCertificateAuthorityProvider for RuntimeAdmissionCertificateAuthority {
  fn current() -> Option<AdmissionCertificateAuthority> {
    type ControlWeights = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
    type EffectWeights = <Runtime as pallet_deos_actors::Config>::TaskEffectWeight;
    let control_identity = <ControlWeights as StepControlWeightProvider<
      pallet_deos_actors::StepOf<Runtime>,
    >>::production_weight_identity()?;
    let effect_identity = <EffectWeights as TaskEffectWeightProvider<
      pallet_deos_actors::TaskOf<Runtime>,
    >>::production_weight_identity()?;
    let configured_bounds_commitment = (
      *b"DEOS_ACTOR_CONFIGURED_BOUNDS",
      (
        ActorMaxContractSteps::get(),
        ActorMaxFundingTrackedAssets::get(),
        ActorMaxOpeningSnapshotEntries::get(),
        ActorMaxOpeningPredicateResults::get(),
        ActorMaxPreconditionClauses::get(),
        ActorMaxPredicatesPerClause::get(),
        ActorMaxPredicatesPerStep::get(),
        ActorMaxSplitTransferLegs::get(),
        ActorMaxWhitelistSize::get(),
      ),
      (
        ActorMaxConsecutiveFailures::get(),
        ActorMaxRetryAttempts::get(),
        ActorMaxAutoCloseNonceHorizon::get(),
        ActorMaxExecutionDelayBlocks::get(),
        ActorMaxTemporalDelayTicks::get(),
        ActorMinWindowLength::get(),
        ActorTargetBlockTime::get(),
      ),
      (
        ActorMaxExecutionsPerBlock::get(),
        ActorMaxQueueLength::get(),
        ActorMaxQueueEntriesScannedPerBlock::get(),
        ActorQueuePageSize::get(),
        ActorWakeupPageSize::get(),
        ActorObservationPageSize::get(),
        ActorOnIdleReserve::get(),
      ),
    )
      .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
    type LifecycleWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let maximum_lifecycle_weight = [
      LifecycleWeights::create_user_actor(),
      LifecycleWeights::create_user_actor_crossing_new_page(),
      LifecycleWeights::create_user_actor_at_slot(),
      LifecycleWeights::create_system_actor(),
      LifecycleWeights::create_system_actor_at_sovereign_id(),
      LifecycleWeights::create_dormant_system_actor(),
      LifecycleWeights::activate_actor(),
      LifecycleWeights::deactivate_actor(),
      LifecycleWeights::pause_actor(),
      LifecycleWeights::resume_actor(),
      LifecycleWeights::update_contract(),
      LifecycleWeights::close_actor(),
      LifecycleWeights::run_cancel(),
    ]
    .into_iter()
    .fold(Weight::zero(), |maximum, weight| {
      Weight::from_parts(
        maximum.ref_time().max(weight.ref_time()),
        maximum.proof_size().max(weight.proof_size()),
      )
    });
    Some(AdmissionCertificateAuthority {
      runtime_actor_semantics_version: 1,
      production_weight_identity: AdmissionCertificateAuthority::compose_production_weight_identity(
        control_identity,
        effect_identity,
      ),
      body_geometry_version: 1,
      configured_bounds_commitment,
      maximum_lifecycle_weight,
    })
  }
}

pub struct TmctolAssetOps;

impl TmctolAssetOps {
  fn bridge_native_staking_ingress(to: &AccountId, amount: Balance) -> Result<(), DispatchError> {
    if amount == 0 {
      return Ok(());
    }
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      native_asset_id,
    ) {
      return Ok(());
    }
    let staking_liquidity_actor = crate::Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    if to != &staking_liquidity_actor {
      return Ok(());
    }
    let (_, remainder) = <Balances as Currency<AccountId>>::slash(to, amount);
    if remainder > 0 {
      return Err(DispatchError::Token(TokenError::FundsUnavailable));
    }
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      native_asset_id,
      to,
      amount,
    )?;
    Ok(())
  }

  pub fn bridge_native_staking_pool_yield() -> Result<(), DispatchError> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      native_asset_id,
    ) {
      return Ok(());
    }
    let staking_pool = crate::Staking::pool_account_for(native_asset_id);
    let amount = <Balances as Currency<AccountId>>::free_balance(&staking_pool)
      .saturating_sub(ExistentialDeposit::get());
    if amount == 0 {
      return Ok(());
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let (_, remainder) = <Balances as Currency<AccountId>>::slash(&staking_pool, amount);
      if remainder > 0 {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          DispatchError::Token(TokenError::FundsUnavailable),
        ));
      }
      if let Err(error) = <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
        native_asset_id,
        &staking_pool,
        amount,
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  /// Private ledger-only native transfer used by `FeeCollector`. It performs one fee-native
  /// ledger movement with no Actors ingress preflight, notification, funding accumulation,
  /// transaction-extension ingress, trigger consequence, or native-staking bridge side effect.
  pub(crate) fn transfer_native_ledger_only(
    from: &AccountId,
    to: &AccountId,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> Result<(), DispatchError> {
        <Balances as Currency<AccountId>>::transfer(
          from,
          to,
          amount,
          polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
        )?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          TaskFailure::permanent(error),
        )),
      }
    })
  }
}

impl AssetOps<AccountId, AssetKind, Balance> for TmctolAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(failure) =
        RuntimeAddressEventIngress::preflight_internal_inbound(to, asset, amount, from)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          failure.into(),
        ));
      }
      let result = (|| -> Result<(), TaskFailure> {
        match asset {
          AssetKind::Native => {
            <Balances as Currency<AccountId>>::transfer(
              from,
              to,
              amount,
              polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
            )
            .map_err(TaskFailure::permanent)?;
            if from
              == &crate::Actors::sovereign_account_id_system(
                ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
              )
              && to == &crate::Staking::native_security_reward_account()
            {
              crate::Staking::certify_native_security_reward_funding(
                from,
                crate::Staking::current_security_epoch(),
                amount,
              )
              .map_err(TaskFailure::permanent)?;
            } else {
              Self::bridge_native_staking_ingress(to, amount).map_err(TaskFailure::permanent)?;
            }
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::transfer(
              id,
              from,
              to,
              amount,
              Preservation::Expendable,
            )
            .map_err(TaskFailure::permanent)?;
          }
        }
        // A certified destination ingress consequence keeps its closed retry
        // classification through TaskFailure (spec 6.1): recoverable queue/wakeup
        // capacity is Temporary, exhaustion/corruption/invariant failure is
        // Permanent, so the owning task retries rather than aborting.
        RuntimeAddressEventIngress::on_internal_inbound(to, asset, amount, from)
          .map_err(TaskFailure::from)?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(failure) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(failure))
        }
      }
    })
  }

  fn burn(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
      match asset {
        AssetKind::Native => {
          let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
          if remainder > 0 {
            return Err(DispatchError::Token(TokenError::FundsUnavailable));
          }
          Ok(())
        }
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::burn_from(
            id,
            who,
            amount,
            Preservation::Expendable,
            Precision::Exact,
            Fortitude::Polite,
          )?;
          Ok(())
        }
      }
    })()
    .map_err(TaskFailure::permanent)
  }

  fn mint(to: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(failure) =
        RuntimeAddressEventIngress::preflight_inbound_without_source(to, asset, amount)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          failure.into(),
        ));
      }
      let result = (|| -> Result<(), TaskFailure> {
        match asset {
          AssetKind::Native => {
            <Balances as NativeMutate<AccountId>>::mint_into(to, amount)
              .map_err(TaskFailure::permanent)?;
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
              id, to, amount,
            )
            .map_err(TaskFailure::permanent)?;
          }
        }
        // Source-less certified Mint keeps the placement classification through
        // TaskFailure so the owning task retries on recoverable capacity.
        RuntimeAddressEventIngress::on_inbound_without_source(to, asset, amount)
          .map_err(TaskFailure::from)?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(failure) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(failure))
        }
      }
    })
  }

  fn balance(who: &AccountId, asset: AssetKind) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::reducible_balance(
        who,
        Preservation::Expendable,
        Fortitude::Polite,
      ),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::reducible_balance(
          id,
          who,
          Preservation::Expendable,
          Fortitude::Polite,
        )
      }
    }
  }

  fn minimum_balance(asset: AssetKind) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::minimum_balance(),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::minimum_balance(id)
      }
    }
  }

  fn preflight_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    if amount == 0 {
      return Ok(());
    }
    if asset == AssetKind::Native
      && from
        == &crate::Actors::sovereign_account_id_system(ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
      && to == &crate::Staking::native_security_reward_account()
    {
      crate::Staking::preflight_native_security_reward_funding(
        from,
        crate::Staking::current_security_epoch(),
        amount,
      )
      .map_err(TaskFailure::permanent)?;
    }
    let deposit = match asset {
      AssetKind::Native => {
        <Balances as NativeInspect<AccountId>>::can_withdraw(from, amount)
          .into_result(false)
          .map_err(TaskFailure::permanent)?;
        <Balances as NativeInspect<AccountId>>::can_deposit(to, amount, Provenance::Extant)
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::can_withdraw(
          id, from, amount,
        )
        .into_result(false)
        .map_err(TaskFailure::permanent)?;
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::can_deposit(
          id,
          to,
          amount,
          Provenance::Extant,
        )
      }
    };
    match deposit {
      DepositConsequence::Success => Ok(()),
      DepositConsequence::BelowMinimum
      | DepositConsequence::CannotCreate
      | DepositConsequence::Blocked => Err(TaskFailure::temporary(
        pallet_deos_actors::Error::<Runtime>::RecipientDepositUnavailable,
      )),
      permanent => Err(TaskFailure::permanent(
        permanent
          .into_result()
          .expect_err("non-success deposit consequence has an error"),
      )),
    }
  }
}

pub struct TmctolDexOps;

pub(crate) fn validate_remove_liquidity_output(
  amount_a: Balance,
  amount_b: Balance,
  min_amount_a: Balance,
  min_amount_b: Balance,
) -> Result<(), TaskFailure> {
  if amount_a < min_amount_a || amount_b < min_amount_b {
    return Err(TaskFailure::temporary(DispatchError::Other(
      "MinimumLiquidityOutputNotMet",
    )));
  }
  Ok(())
}

pub(crate) fn classify_remove_liquidity_failure(error: DispatchError) -> TaskFailure {
  let first_minimum: DispatchError =
    pallet_asset_conversion::Error::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum.into();
  let second_minimum: DispatchError =
    pallet_asset_conversion::Error::<Runtime>::AssetTwoWithdrawalDidNotMeetMinimum.into();
  if error == first_minimum || error == second_minimum {
    TaskFailure::temporary(error)
  } else {
    TaskFailure::permanent(error)
  }
}

pub(crate) fn classify_router_failure(error: pallet_deos_router::Error<Runtime>) -> TaskFailure {
  let retry = error.retry_disposition();
  classify_router_retry(retry, error.into())
}

pub(crate) fn classify_router_execution_failure(
  error: pallet_deos_router::ExecutionError<Runtime>,
) -> TaskFailure {
  let retry = error.retry_disposition();
  classify_router_retry(retry, error.into_dispatch_error())
}

fn classify_router_retry(
  retry: pallet_deos_router::RetryDisposition,
  error: DispatchError,
) -> TaskFailure {
  match retry {
    pallet_deos_router::RetryDisposition::Permanent => TaskFailure::permanent(error),
    pallet_deos_router::RetryDisposition::RetryLater => TaskFailure::temporary(error),
  }
}

pub struct TmctolLiquidityOps;

impl DexOps<AccountId, AssetKind, Balance> for TmctolDexOps {
  fn swap_exact_in(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    let quote = pallet_deos_router::Pallet::<Runtime>::quote_exact_input(
      who.clone(),
      asset_in,
      asset_out,
      amount_in,
    )
    .map_err(classify_router_failure)?;
    let min_out =
      (polkadot_sdk::sp_runtime::Perbill::one() - slippage_tolerance).mul_floor(quote.amount_out);
    Self::ensure_system_reference_price(
      &context,
      asset_in,
      asset_out,
      quote.amount_after_fee,
      quote.amount_out,
    )?;
    pallet_deos_router::Pallet::<Runtime>::execute_swap_for(
      who, asset_in, asset_out, amount_in, min_out, who,
    )
    .map(|outcome| DexSwapOutcome {
      total_amount_in: outcome.total_amount_in,
      recipient_amount_out: outcome.recipient_amount_out,
    })
    .map_err(classify_router_execution_failure)
  }

  fn swap_exact_out(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    let quote = pallet_deos_router::Pallet::<Runtime>::quote_exact_out(
      who.clone(),
      asset_in,
      asset_out,
      amount_out,
    )
    .map_err(classify_router_failure)?;
    // Tolerance-bound cap with checked widened ceiling arithmetic: no saturation or
    // silent clamp. The ceiling is quote.amount_in + ceil(slippage * quote.amount_in),
    // computed in U256 and narrowed to the balance width; overflow fails closed.
    let quoted_max_in = U256::from(quote.amount_in)
      .checked_add(
        U256::from(quote.amount_in)
          .checked_mul(U256::from(slippage_tolerance.deconstruct()))
          .and_then(|value| value.checked_add(U256::from(1_000_000_000u64 - 1)))
          .map(|value| value / U256::from(1_000_000_000u64))
          .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?,
      )
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?;
    let quoted_max_in: Balance = quoted_max_in
      .try_into()
      .map_err(|_| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?;
    // The tolerance-bound cap, not merely the larger preservable balance, bounds the
    // Router exact-output execution boundary.
    let execution_cap = quoted_max_in.min(max_amount_in);
    if quoted_max_in > max_amount_in {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "ExactOutInputCapacityExceeded",
      )));
    }
    Self::ensure_system_reference_price(
      &context,
      asset_in,
      asset_out,
      quote.amount_after_fee,
      quote.amount_out,
    )?;
    pallet_deos_router::Pallet::<Runtime>::execute_exact_out_for(
      who,
      asset_in,
      asset_out,
      amount_out,
      execution_cap,
      who,
    )
    .map(|outcome| DexSwapOutcome {
      total_amount_in: outcome.total_amount_in,
      recipient_amount_out: outcome.recipient_amount_out,
    })
    .map_err(classify_router_execution_failure)
  }
}

impl LiquidityOps<AccountId, AssetKind, Balance> for TmctolLiquidityOps {
  fn lp_assets(lp_asset: AssetKind) -> Option<(AssetKind, AssetKind)> {
    let AssetKind::Local(lp_id) = lp_asset else {
      return None;
    };
    crate::DeosRouter::lp_pair_by_token_id(lp_id)
  }

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    amount_a: Balance,
    amount_b: Balance,
    min_lp_out: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      use alloc::boxed::Box;
      let result = (|| -> Result<(Balance, Balance, Balance), TaskFailure> {
        if AssetConversion::get_reserves(asset_a, asset_b).is_err() {
          crate::DeosRouter::create_pool(RuntimeOrigin::signed(who.clone()), asset_a, asset_b)
            .map_err(TaskFailure::permanent)?;
        }
        let lp_before = liquidity_lp_balance(who, asset_a, asset_b);
        let a_before = TmctolAssetOps::balance(who, asset_a);
        let b_before = TmctolAssetOps::balance(who, asset_b);
        AssetConversion::add_liquidity(
          RuntimeOrigin::signed(who.clone()),
          Box::new(asset_a),
          Box::new(asset_b),
          amount_a,
          amount_b,
          0,
          0,
          who.clone(),
        )
        .map_err(TaskFailure::permanent)?;
        let lp_after = liquidity_lp_balance(who, asset_a, asset_b);
        let lp_minted = lp_after.saturating_sub(lp_before);
        // Factual outcomes: measure the actual asset debits and LP output rather than
        // returning the authored caps as if fully consumed (spec 3.4).
        let used_a = a_before.saturating_sub(TmctolAssetOps::balance(who, asset_a));
        let used_b = b_before.saturating_sub(TmctolAssetOps::balance(who, asset_b));
        if lp_minted < min_lp_out {
          return Err(TaskFailure::temporary(DispatchError::Other(
            "MinimumLpOutputNotMet",
          )));
        }
        Ok((used_a, used_b, lp_minted))
      })();
      match result {
        Ok(value) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(value)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetKind,
    asset_a: AssetKind,
    asset_b: AssetKind,
    lp_amount: Balance,
    min_amount_a: Balance,
    min_amount_b: Balance,
  ) -> Result<(Balance, Balance), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      use alloc::boxed::Box;
      let result = (|| -> Result<(Balance, Balance), TaskFailure> {
        let lp_id = match lp_asset {
          AssetKind::Local(id) => id,
          _ => {
            return Err(TaskFailure::permanent(DispatchError::Other(
              "LP asset must be Local",
            )));
          }
        };
        let (registry_a, registry_b) =
          crate::DeosRouter::lp_pair_by_token_id(lp_id).ok_or_else(|| {
            TaskFailure::permanent(DispatchError::Other("Pool not found for LP token"))
          })?;
        // The expected ordered pair must match the stable registry binding; an
        // admitted LP token is never silently reinterpreted.
        if (registry_a, registry_b) != (asset_a, asset_b) {
          return Err(TaskFailure::permanent(DispatchError::Other(
            "LiquidityPairBindingMismatch",
          )));
        }
        let before_a = TmctolAssetOps::balance(who, asset_a);
        let before_b = TmctolAssetOps::balance(who, asset_b);
        AssetConversion::remove_liquidity(
          RuntimeOrigin::signed(who.clone()),
          Box::new(asset_a),
          Box::new(asset_b),
          lp_amount,
          min_amount_a,
          min_amount_b,
          who.clone(),
        )
        .map_err(classify_remove_liquidity_failure)?;
        let after_a = TmctolAssetOps::balance(who, asset_a);
        let after_b = TmctolAssetOps::balance(who, asset_b);
        let amount_a = after_a.saturating_sub(before_a);
        let amount_b = after_b.saturating_sub(before_b);
        validate_remove_liquidity_output(amount_a, amount_b, min_amount_a, min_amount_b)?;
        Ok((amount_a, amount_b))
      })();
      match result {
        Ok(value) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(value)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    max_amount_a: Balance,
    max_amount_b: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("StakedAssetUnavailable")))?;
    if asset_a == AssetKind::Local(native_asset_id) && asset_b == AssetKind::Local(staked_asset_id)
    {
      if max_amount_a.is_zero() {
        return Err(TaskFailure::permanent(DispatchError::Other(
          "DonationAmountTooSmall",
        )));
      }
      let donation =
        crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
          who,
          max_amount_a,
          max_amount_b,
          max_ratio_error,
        )?;
      TmctolAssetOps::bridge_native_staking_pool_yield().map_err(TaskFailure::permanent)?;
      return Ok(donation);
    }
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityDonationUnsupported",
    )))
  }
}

impl TmctolDexOps {
  pub(crate) fn ensure_system_reference_price(
    context: &ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    amount_out: Balance,
  ) -> Result<(), TaskFailure> {
    if context.actor_type != ActorType::System {
      return Ok(());
    }
    if amount_in == 0 || amount_out == 0 {
      return Err(TaskFailure::permanent(DispatchError::Other(
        "InvalidSystemMarketQuote",
      )));
    }
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    let ema_reference =
      crate::Oracle::observation_state(feed, ActorMaxSystemReferenceAgeBlocks::get())
        .ok()
        .and_then(|state| match state {
          pallet_oracle::ObservationState::Fresh(observation) if observation.value > 0 => {
            Some(observation.value)
          }
          _ => None,
        });
    let reference = ema_reference
      .or_else(|| {
        AssetConversion::get_reserves(asset_in, asset_out)
          .ok()
          .and_then(|(reserve_in, reserve_out)| {
            primitives::checked_scaled_ratio(reserve_out, reserve_in, ecosystem::params::PRECISION)
          })
          .filter(|price| *price > 0)
      })
      .ok_or_else(|| {
        TaskFailure::temporary(DispatchError::Other("SystemReferencePriceUnavailable"))
      })?;
    // Checked cross-multiplication deviation guard (spec 5.3): the scaled reference
    // price is ref_out/ref_in * PRECISION; comparing without division requires
    //   abs(exec_out * ref_in - ref_out * exec_in) * ACCURACY
    //     <= deviation * ref_out * exec_in
    // computed with a sufficient widened integer type (U256) and checked narrowing.
    // The products ref_out * exec_in and the deviation product use PRECISION-scaled
    // values consistently; a widening overflow fails closed as Permanent.
    let exec_in = U256::from(amount_in);
    let exec_out = U256::from(amount_out);
    let ref_in = U256::from(ecosystem::params::PRECISION);
    let ref_out = U256::from(reference);
    let a = exec_out
      .checked_mul(ref_in)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let b = exec_in
      .checked_mul(ref_out)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let abs_diff = a.max(b) - a.min(b);
    let left = abs_diff
      .checked_mul(U256::from(1_000_000_000u64))
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let right = U256::from(ActorMaxSystemPriceDeviation::get().deconstruct())
      .checked_mul(ref_out)
      .and_then(|value| value.checked_mul(exec_in))
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    if left > right {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "SystemPriceDeviationExceeded",
      )));
    }
    Ok(())
  }
}

fn liquidity_lp_balance(who: &AccountId, asset_a: AssetKind, asset_b: AssetKind) -> Balance {
  let pool_id =
    <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b).ok();
  let Some(pool_id) = pool_id else {
    return 0;
  };
  let Some(pool_info) = pallet_asset_conversion::Pools::<Runtime>::get(pool_id) else {
    return 0;
  };
  <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(pool_info.lp_token, who)
}

/// System Actors genesis initializer for the current DEOS reference runtime.
///
/// Creates well-known System actors at genesis with deterministic `actor_id` values
/// defined in `primitives::ecosystem::actor_ids` (including sparse ranges).
/// The sovereign accounts are derived from `(ActorsPalletId, "system", actor_id)`
/// and can be computed offline for use in other configs.
pub struct TmctolGenesisSystemActors;

const SYSTEM_ACTOR_TOPOLOGY_IDS: [pallet_deos_actors::ActorId; 15] = [
  ecosystem::actor_ids::BURN_ACTOR_ID,
  ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
  ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
  ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
  ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
  ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
  ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
  ecosystem::actor_ids::TREASURY_B_ACTORS_ID,
  ecosystem::actor_ids::TREASURY_C_ACTORS_ID,
  ecosystem::actor_ids::TREASURY_D_ACTORS_ID,
  ecosystem::actor_ids::BLDR_SPLITTER_ACTORS_ID,
  ecosystem::actor_ids::BLDR_LIQUIDITY_ACTOR_ID,
  ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID,
  ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
  ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
];

const SYSTEM_ACTIVATION_MANIFEST_EDGES: [(
  pallet_deos_actors::ActorId,
  pallet_deos_actors::ActorId,
); 11] = [
  (
    ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
    ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
  ),
  (
    ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
    ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
    ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
    ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
    ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
    ecosystem::actor_ids::TREASURY_B_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
    ecosystem::actor_ids::TREASURY_C_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
    ecosystem::actor_ids::TREASURY_D_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::BLDR_SPLITTER_ACTORS_ID,
    ecosystem::actor_ids::BLDR_LIQUIDITY_ACTOR_ID,
  ),
  (
    ecosystem::actor_ids::BLDR_SPLITTER_ACTORS_ID,
    ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
  ),
  (
    ecosystem::actor_ids::BLDR_LIQUIDITY_ACTOR_ID,
    ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID,
  ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemActivationEffect {
  CertifiedActorTransfer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemActivationEdge {
  pub source: pallet_deos_actors::ActorId,
  pub target: pallet_deos_actors::ActorId,
  pub effect: SystemActivationEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemActivationNode {
  pub actor_id: pallet_deos_actors::ActorId,
  pub rank: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemActivationTopology {
  pub nodes: alloc::vec::Vec<SystemActivationNode>,
  pub edges: alloc::vec::Vec<SystemActivationEdge>,
}

pub struct DeosSystemActorContractValidator;

impl DeosSystemActorContractValidator {
  fn known_target(account: &AccountId) -> Option<pallet_deos_actors::ActorId> {
    SYSTEM_ACTOR_TOPOLOGY_IDS
      .into_iter()
      .find(|actor_id| crate::Actors::sovereign_account_id_system(*actor_id) == *account)
  }

  fn declared_targets(
    contract: &pallet_deos_actors::ActorContractOf<Runtime>,
  ) -> alloc::vec::Vec<pallet_deos_actors::ActorId> {
    let mut targets = alloc::vec::Vec::new();
    for step in &contract.steps {
      match &step.task {
        pallet_deos_actors::Task::Transfer { to, .. } => {
          if let Some(target) = Self::known_target(to)
            && !targets.contains(&target)
          {
            targets.push(target);
          }
        }
        pallet_deos_actors::Task::SplitTransfer { legs, .. } => {
          for leg in legs {
            if let Some(target) = Self::known_target(&leg.to)
              && !targets.contains(&target)
            {
              targets.push(target);
            }
          }
        }
        _ => {}
      }
    }
    targets
  }

  fn contract_set(
    candidate: Option<(
      pallet_deos_actors::ActorId,
      &pallet_deos_actors::ActorContractOf<Runtime>,
    )>,
  ) -> Result<
    alloc::vec::Vec<(
      pallet_deos_actors::ActorId,
      pallet_deos_actors::ActorContractOf<Runtime>,
    )>,
    DispatchError,
  > {
    let mut contracts = alloc::vec::Vec::new();
    for actor_id in SYSTEM_ACTOR_TOPOLOGY_IDS {
      let contract = candidate
        .filter(|(candidate_id, _)| *candidate_id == actor_id)
        .map(|(_, contract)| contract.clone())
        .or_else(|| crate::Actors::actor_contract(actor_id));
      if let Some(contract) = contract {
        contracts.push((actor_id, contract));
      }
    }
    Ok(contracts)
  }

  fn target_actor(
    account: &AccountId,
    contracts: &[(
      pallet_deos_actors::ActorId,
      pallet_deos_actors::ActorContractOf<Runtime>,
    )],
  ) -> Option<pallet_deos_actors::ActorId> {
    contracts.iter().find_map(|(actor_id, contract)| {
      let target = crate::Actors::sovereign_account_id_system(*actor_id);
      (target == *account
        && matches!(
          contract.trigger,
          pallet_deos_actors::Trigger::AddressEvent { .. }
        ))
      .then_some(*actor_id)
    })
  }

  fn derive_edges(
    contracts: &[(
      pallet_deos_actors::ActorId,
      pallet_deos_actors::ActorContractOf<Runtime>,
    )],
  ) -> alloc::vec::Vec<SystemActivationEdge> {
    let mut edges = alloc::vec::Vec::new();
    for (source, contract) in contracts {
      for step in &contract.steps {
        match &step.task {
          pallet_deos_actors::Task::Transfer { to, .. } => {
            if let Some(target) = Self::target_actor(to, contracts) {
              let edge = SystemActivationEdge {
                source: *source,
                target,
                effect: SystemActivationEffect::CertifiedActorTransfer,
              };
              if !edges.contains(&edge) {
                edges.push(edge);
              }
            }
          }
          pallet_deos_actors::Task::SplitTransfer { legs, .. } => {
            for leg in legs {
              if let Some(target) = Self::target_actor(&leg.to, contracts) {
                let edge = SystemActivationEdge {
                  source: *source,
                  target,
                  effect: SystemActivationEffect::CertifiedActorTransfer,
                };
                if !edges.contains(&edge) {
                  edges.push(edge);
                }
              }
            }
          }
          _ => {}
        }
      }
    }
    edges
  }

  fn topology_from_contracts(
    contracts: alloc::vec::Vec<(
      pallet_deos_actors::ActorId,
      pallet_deos_actors::ActorContractOf<Runtime>,
    )>,
  ) -> Result<SystemActivationTopology, DispatchError> {
    let edges = Self::derive_edges(&contracts);
    let mut ranks = [0u8; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    let mut indegree = [0u8; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    let mut present = [false; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    for (actor_id, _) in &contracts {
      let index = SYSTEM_ACTOR_TOPOLOGY_IDS
        .iter()
        .position(|known| known == actor_id)
        .ok_or(DispatchError::Other("SystemActorOutsideTopologyManifest"))?;
      present[index] = true;
    }
    for edge in &edges {
      let target = SYSTEM_ACTOR_TOPOLOGY_IDS
        .iter()
        .position(|known| *known == edge.target)
        .ok_or(DispatchError::Other("SystemActivationTargetUnknown"))?;
      indegree[target] = indegree[target]
        .checked_add(1)
        .ok_or(DispatchError::Other("SystemActivationIndegreeOverflow"))?;
    }
    let mut processed = 0usize;
    let mut admitted = [false; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    loop {
      let next = (0..SYSTEM_ACTOR_TOPOLOGY_IDS.len())
        .find(|index| present[*index] && !admitted[*index] && indegree[*index] == 0);
      let Some(index) = next else { break };
      admitted[index] = true;
      processed += 1;
      let source = SYSTEM_ACTOR_TOPOLOGY_IDS[index];
      for edge in edges.iter().filter(|edge| edge.source == source) {
        let target = SYSTEM_ACTOR_TOPOLOGY_IDS
          .iter()
          .position(|known| *known == edge.target)
          .ok_or(DispatchError::Other("SystemActivationTargetUnknown"))?;
        indegree[target] = indegree[target]
          .checked_sub(1)
          .ok_or(DispatchError::Other("SystemActivationIndegreeUnderflow"))?;
        ranks[target] = ranks[target].max(
          ranks[index]
            .checked_add(1)
            .ok_or(DispatchError::Other("SystemActivationRankOverflow"))?,
        );
      }
    }
    if processed != contracts.len() {
      return Err(DispatchError::Other("SystemActivationCycle"));
    }
    let nodes = contracts
      .iter()
      .map(|(actor_id, _)| {
        let index = SYSTEM_ACTOR_TOPOLOGY_IDS
          .iter()
          .position(|known| known == actor_id)
          .expect("contract set contains only manifest actors"); // deos-bypass: panic-owner — the bounded manifest admission check above owns membership.
        SystemActivationNode {
          actor_id: *actor_id,
          rank: ranks[index],
        }
      })
      .collect();
    Ok(SystemActivationTopology { nodes, edges })
  }

  pub fn projection() -> Result<SystemActivationTopology, DispatchError> {
    Self::topology_from_contracts(Self::contract_set(None)?)
  }

  pub fn manifest() -> Result<SystemActivationTopology, DispatchError> {
    let nodes = SYSTEM_ACTOR_TOPOLOGY_IDS
      .into_iter()
      .map(|actor_id| SystemActivationNode { actor_id, rank: 0 })
      .collect::<alloc::vec::Vec<_>>();
    let edges = SYSTEM_ACTIVATION_MANIFEST_EDGES
      .into_iter()
      .map(|(source, target)| SystemActivationEdge {
        source,
        target,
        effect: SystemActivationEffect::CertifiedActorTransfer,
      })
      .collect();
    Self::rank_topology(nodes, edges)
  }

  fn rank_topology(
    nodes: alloc::vec::Vec<SystemActivationNode>,
    edges: alloc::vec::Vec<SystemActivationEdge>,
  ) -> Result<SystemActivationTopology, DispatchError> {
    let mut ranked = SystemActivationTopology { nodes, edges };
    let mut ranks = [0u8; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    let mut indegree = [0u8; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    for edge in &ranked.edges {
      let target = SYSTEM_ACTOR_TOPOLOGY_IDS
        .iter()
        .position(|id| *id == edge.target)
        .ok_or(DispatchError::Other("SystemActivationTargetUnknown"))?;
      indegree[target] = indegree[target]
        .checked_add(1)
        .ok_or(DispatchError::Other("SystemActivationIndegreeOverflow"))?;
    }
    let mut admitted = [false; SYSTEM_ACTOR_TOPOLOGY_IDS.len()];
    let mut processed = 0usize;
    while let Some(index) =
      (0..SYSTEM_ACTOR_TOPOLOGY_IDS.len()).find(|index| !admitted[*index] && indegree[*index] == 0)
    {
      admitted[index] = true;
      processed += 1;
      for edge in ranked
        .edges
        .iter()
        .filter(|edge| edge.source == SYSTEM_ACTOR_TOPOLOGY_IDS[index])
      {
        let target = SYSTEM_ACTOR_TOPOLOGY_IDS
          .iter()
          .position(|id| *id == edge.target)
          .ok_or(DispatchError::Other("SystemActivationTargetUnknown"))?;
        indegree[target] = indegree[target]
          .checked_sub(1)
          .ok_or(DispatchError::Other("SystemActivationIndegreeUnderflow"))?;
        ranks[target] = ranks[target].max(
          ranks[index]
            .checked_add(1)
            .ok_or(DispatchError::Other("SystemActivationRankOverflow"))?,
        );
      }
    }
    if processed != SYSTEM_ACTOR_TOPOLOGY_IDS.len() {
      return Err(DispatchError::Other("SystemActivationCycle"));
    }
    for node in &mut ranked.nodes {
      let index = SYSTEM_ACTOR_TOPOLOGY_IDS
        .iter()
        .position(|id| *id == node.actor_id)
        .ok_or(DispatchError::Other("SystemActorOutsideTopologyManifest"))?;
      node.rank = ranks[index];
    }
    Ok(ranked)
  }

  fn validate_candidate(
    actor_id: pallet_deos_actors::ActorId,
    contract: &pallet_deos_actors::ActorContractOf<Runtime>,
  ) -> DispatchResult {
    Self::manifest()?;
    let targets = Self::declared_targets(contract);
    if !SYSTEM_ACTOR_TOPOLOGY_IDS.contains(&actor_id) {
      return if targets.is_empty() {
        Ok(())
      } else {
        Err(DispatchError::Other("SystemActorOutsideTopologyManifest"))
      };
    }
    for target in targets {
      if !SYSTEM_ACTIVATION_MANIFEST_EDGES.contains(&(actor_id, target)) {
        return Err(DispatchError::Other("SystemActivationEdgeUndeclared"));
      }
    }
    Ok(())
  }
}

impl pallet_deos_actors::SystemActorContractValidator<pallet_deos_actors::ActorContractOf<Runtime>>
  for DeosSystemActorContractValidator
{
  fn validate(
    actor_id: pallet_deos_actors::ActorId,
    contract: &pallet_deos_actors::ActorContractOf<Runtime>,
  ) -> DispatchResult {
    Self::validate_candidate(actor_id, contract)
  }
}

impl TmctolGenesisSystemActors {
  /// Runtime-topology accounts that retain one free native ED so arbitrarily small native ingress
  /// remains admissible under `pallet-balances` semantics.
  pub fn native_flow_anchor_accounts() -> alloc::vec::Vec<AccountId> {
    let mut accounts = (ecosystem::actor_ids::BURN_ACTOR_ID
      ..=ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID)
      .map(crate::Actors::sovereign_account_id_system)
      .collect::<alloc::vec::Vec<_>>();
    accounts.push(crate::Staking::pool_account_for(
      <Runtime as pallet_staking::Config>::NativeStakingAssetId::get(),
    ));
    accounts.push(crate::Staking::native_security_reward_account());
    accounts
  }

  pub fn resolve_zap_slippage_tolerance(foreign: AssetKind) -> Perbill {
    let Some((native_reserve, _)) = AssetConversion::get_reserves(AssetKind::Native, foreign).ok()
    else {
      return ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE;
    };
    let min_parts = u128::from(ecosystem::params::LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE.deconstruct());
    let max_parts = u128::from(ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE.deconstruct());
    let reference_depth =
      ecosystem::params::LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE.max(1);
    let scaled_parts = max_parts
      .saturating_mul(reference_depth)
      .saturating_div(native_reserve.max(1));
    let clamped_parts = scaled_parts.clamp(min_parts, max_parts);
    Perbill::from_parts(clamped_parts as u32)
  }
}

impl
  pallet_deos_actors::GenesisSystemActors<AccountId, pallet_deos_actors::ActorContractOf<Runtime>>
  for TmctolGenesisSystemActors
{
  fn system_actors() -> alloc::vec::Vec<(
    pallet_deos_actors::ActorId,
    AccountId,
    pallet_deos_actors::Mutability,
    pallet_deos_actors::ActorContractOf<Runtime>,
  )> {
    use pallet_deos_actors::{ActorContract, FundingSourcePolicy, Mutability, Trigger};
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = ActorsPalletId::get().into_account_truncating();

    // --- Burn Actor (actor_id = 0) ---
    // Omnivorous intake: any verified inbound value signals one bounded pass that
    // swaps configured foreign balances to native and burns available native.
    let burn_trigger = Trigger::address_event(
      pallet_deos_actors::SourceFilter::Any,
      pallet_deos_actors::AssetFilter::Any,
    );
    let dust = ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    // Genesis contract_steps: swap known foreign assets → native, then burn.
    // Governance replaces the canonical contract when adding steps for new foreign assets.
    let burn_contract_steps: pallet_deos_actors::ContractSteps<Runtime> =
      Self::build_burn_contract_steps(alloc::vec![], dust);

    // --- Fee Sink (actor_id = 1) ---
    // Collection only grows the ledger buffer; one ordinary cadence owns allocation.
    let fee_sink_trigger = Trigger::cadenced(ecosystem::params::FEE_SINK_CADENCE_TICKS);
    let fee_sink_contract_steps: pallet_deos_actors::ContractSteps<Runtime> =
      Self::build_fee_sink_contract_steps();

    alloc::vec![
      (
        ecosystem::actor_ids::BURN_ACTOR_ID,
        governance.clone(),
        Mutability::Mutable,
        ActorContract {
          trigger: burn_trigger,
          cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
          window: None,
          steps: burn_contract_steps,
          funding: FundingSourcePolicy::RuntimePolicy,
          completion: pallet_deos_actors::CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        },
      ),
      (
        ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
        governance.clone(),
        Mutability::Mutable,
        ActorContract {
          trigger: fee_sink_trigger,
          cooldown_blocks: 0,
          window: None,
          steps: fee_sink_contract_steps,
          funding: FundingSourcePolicy::RuntimePolicy,
          completion: pallet_deos_actors::CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        },
      ),
      // --- BLDR Splitter (actor_id = 10) ---
      // Receives 66% of TMC-minted $BLDR, splits 50/50 to BLDR liquidity + treasury lanes.
      (
        ecosystem::actor_ids::BLDR_SPLITTER_ACTORS_ID,
        governance,
        Mutability::Mutable,
        ActorContract {
          trigger: Trigger::address_event(
            pallet_deos_actors::SourceFilter::Any,
            pallet_deos_actors::AssetFilter::Any,
          ),
          cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
          window: None,
          steps: Self::build_bldr_splitter_contract_steps(
            AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID),
            dust,
          ),
          funding: FundingSourcePolicy::RuntimePolicy,
          completion: pallet_deos_actors::CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        },
      ),
    ]
  }

  fn dormant_system_actors() -> alloc::vec::Vec<(
    pallet_deos_actors::ActorId,
    AccountId,
    pallet_deos_actors::Mutability,
  )> {
    use pallet_deos_actors::Mutability;
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = ActorsPalletId::get().into_account_truncating();
    alloc::vec![
      (
        ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
        Mutability::Immutable,
      ),
      (
        ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TREASURY_B_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TREASURY_C_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::TREASURY_D_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::BLDR_LIQUIDITY_ACTOR_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID,
        Mutability::Immutable,
      ),
      (
        ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
        Mutability::Mutable,
      ),
      (
        ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
        Mutability::Mutable,
      ),
    ]
    .into_iter()
    .map(|(actor_id, mutability)| (actor_id, governance.clone(), mutability))
    .collect()
  }

  fn integrity_test() {
    use pallet_deos_actors::{
      AmountResolution, CompletionPolicy, FundingSourcePolicy, Mutability, Task, Trigger,
    };

    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    DeosSystemActorContractValidator::manifest()
      .expect("System Actor activation manifest must be acyclic"); // deos-bypass: panic-owner — runtime integrity owns the bounded static catalog.
    for (actor_id, _, _, contract) in Self::system_actors() {
      DeosSystemActorContractValidator::validate_candidate(actor_id, &contract)
        .expect("genesis System Actor effects must fit the activation manifest"); // deos-bypass: panic-owner — runtime integrity owns the bounded genesis catalog.
    }
    let system_control_account: AccountId = ActorsPalletId::get().into_account_truncating();
    for (_, owner, _, _) in Self::system_actors() {
      assert_eq!(
        owner, system_control_account,
        "executable System Actor owner must be the non-signable Actors pallet account",
      );
    }
    for (_, owner, _) in Self::dormant_system_actors() {
      assert_eq!(
        owner, system_control_account,
        "dormant System Actor owner must be the non-signable Actors pallet account",
      );
    }

    let fee_sink_id = ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = Self::system_actors()
      .into_iter()
      .find(|(actor_id, _, _, _)| *actor_id == fee_sink_id);
    assert!(fee_sink.is_some(), "Fee Sink System Actor must exist");
    let Some((_, _, mutability, contract)) = fee_sink else {
      return;
    };
    assert_eq!(mutability, Mutability::Mutable, "Fee Sink must be Mutable");
    assert_eq!(
      contract.trigger,
      Trigger::cadenced(ecosystem::params::FEE_SINK_CADENCE_TICKS),
      "Fee Sink must use the canonical timestamp cadence",
    );
    assert_eq!(
      contract.funding,
      FundingSourcePolicy::RuntimePolicy,
      "Fee Sink must accept runtime-certified fee ingress",
    );
    assert_eq!(
      contract.completion,
      CompletionPolicy::Persistent,
      "Fee Sink must remain persistent",
    );
    assert_eq!(
      contract.steps.len(),
      1,
      "Fee Sink must own one allocation step"
    );

    assert!(
      Self::dormant_system_actors()
        .iter()
        .any(|(actor_id, _, _)| {
          *actor_id == ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID
        }),
      "Fee Sink liquidity ingress must retain its System sovereign locator",
    );
    let staking_pool = crate::Staking::pool_account_for(0);
    let reward_account = crate::Staking::native_security_reward_account();
    let liquidity_actor = crate::Actors::sovereign_account_id_system(
      ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let expected_legs = if crate::Staking::native_security_mode()
      == pallet_staking::NativeSecurityMode::LpBackedSelection
    {
      alloc::vec![
        (reward_account.clone(), Perbill::from_percent(34)),
        (staking_pool.clone(), Perbill::from_percent(33)),
        (liquidity_actor.clone(), Perbill::from_percent(33)),
      ]
    } else {
      alloc::vec![
        (staking_pool.clone(), Perbill::from_percent(50)),
        (liquidity_actor.clone(), Perbill::from_percent(50)),
      ]
    };
    assert!(
      matches!(contract.steps[0].task, Task::SplitTransfer { .. }),
      "Fee Sink must own one split transfer",
    );
    let Task::SplitTransfer {
      asset,
      amount,
      legs,
    } = &contract.steps[0].task
    else {
      return;
    };
    assert_eq!(
      *asset,
      AssetKind::Native,
      "Fee Sink must allocate native fees"
    );
    assert_eq!(
      *amount,
      AmountResolution::PercentageOfCurrent(ecosystem::params::FEE_SINK_BUFFER_PCT),
      "Fee Sink must process the canonical buffer share",
    );
    let actual_legs = legs
      .iter()
      .map(|leg| (leg.to.clone(), leg.share))
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(
      actual_legs, expected_legs,
      "Fee Sink mode/leg topology drifted"
    );
    assert_eq!(
      legs.iter().map(|leg| leg.share.deconstruct()).sum::<u32>(),
      Perbill::one().deconstruct(),
      "Fee Sink allocation shares must conserve the processed amount",
    );

    let fee_sink_account = crate::Actors::sovereign_account_id_system(fee_sink_id);
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::FeeSink::get(),
      fee_sink_account,
      "fee collection and the Fee Sink actor must share one custody account",
    );
    let distinct_accounts = [
      fee_sink_account.clone(),
      staking_pool.clone(),
      reward_account.clone(),
      liquidity_actor.clone(),
      crate::Staking::native_lp_lock_account(),
      crate::configs::governance_config::governance_vote_power_custody_account(),
    ]
    .into_iter()
    .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(
      distinct_accounts.len(),
      6,
      "fee, staking, security, liquidity, LP lock, and governance custody accounts must not alias",
    );
    let anchor_accounts = Self::native_flow_anchor_accounts();
    for account in [
      fee_sink_account,
      staking_pool,
      reward_account,
      liquidity_actor,
    ] {
      assert!(
        anchor_accounts.contains(&account),
        "every arbitrarily small native-flow endpoint must own a genesis ED anchor",
      );
    }
  }
}

impl TmctolGenesisSystemActors {
  fn all_conditions(
    predicates: alloc::vec::Vec<
      pallet_deos_actors::Predicate<AssetKind, Balance, u32, primitives::OracleFeedId>,
    >,
  ) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
    let clause = predicates
      .into_iter()
      .map(|predicate| pallet_deos_actors::TimedPredicate {
        timing: pallet_deos_actors::ObservationTiming::Current,
        predicate,
      })
      .collect::<alloc::vec::Vec<_>>()
      .try_into()
      .expect("runtime predicate clause fits MaxPredicatesPerClause");
    Some(pallet_deos_actors::Precondition {
      clauses: alloc::vec![clause]
        .try_into()
        .expect("runtime clause fits MaxPreconditionClauses"),
    })
  }

  fn minimum_base_for_perbill_output(minimum_output: Balance, share: Perbill) -> Balance {
    let parts = u128::from(share.deconstruct());
    assert!(parts > 0, "Fee Sink allocation shares must be nonzero");
    let accuracy = u128::from(Perbill::one().deconstruct());
    let numerator =
      sp_core::U256::from(minimum_output).saturating_mul(sp_core::U256::from(accuracy));
    let rounded = numerator.saturating_add(sp_core::U256::from(parts.saturating_sub(1)))
      / sp_core::U256::from(parts);
    rounded.try_into().unwrap_or(Balance::MAX)
  }

  pub fn build_fee_sink_contract_steps() -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, SplitLeg, Step, StepErrorPolicy, Task};
    let lp_backed = crate::Staking::native_security_mode()
      == pallet_staking::NativeSecurityMode::LpBackedSelection;
    let legs = if lp_backed {
      alloc::vec![
        SplitLeg {
          to: crate::Staking::native_security_reward_account(),
          share: Perbill::from_percent(34),
        },
        SplitLeg {
          to: crate::Staking::pool_account_for(0),
          share: Perbill::from_percent(33),
        },
        SplitLeg {
          to: crate::Actors::sovereign_account_id_system(
            ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
          ),
          share: Perbill::from_percent(33),
        },
      ]
    } else {
      alloc::vec![
        SplitLeg {
          to: crate::Staking::pool_account_for(0),
          share: Perbill::from_percent(50),
        },
        SplitLeg {
          to: crate::Actors::sovereign_account_id_system(
            ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
          ),
          share: Perbill::from_percent(50),
        },
      ]
    };
    let mut minimum_processed = 0;
    for leg in &legs {
      minimum_processed = minimum_processed.max(Self::minimum_base_for_perbill_output(
        ExistentialDeposit::get(),
        leg.share,
      ));
    }
    assert!(
      minimum_processed > 0,
      "Fee Sink has at least one allocation leg"
    );
    let required_spendable = Self::minimum_base_for_perbill_output(
      minimum_processed,
      ecosystem::params::FEE_SINK_BUFFER_PCT,
    );
    let minimum_balance = ExistentialDeposit::get()
      .saturating_add(required_spendable)
      .saturating_sub(1);
    alloc::vec![Step {
      precondition: Self::all_conditions(alloc::vec![
        pallet_deos_actors::Predicate::BalanceAbove {
          asset: AssetKind::Native,
          threshold: minimum_balance,
        },
      ]),
      task: Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::PercentageOfCurrent(ecosystem::params::FEE_SINK_BUFFER_PCT,),
        legs: legs
          .try_into()
          .expect("phase-aware fee-sink split legs fit"),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("phase-aware fee-sink contract_steps fits")
  }

  /// Builds the Burn Actor contract_steps: for each known foreign asset, add a
  /// conditional SwapIn step (skip if balance < dust), then a final Burn step.
  pub fn build_burn_contract_steps(
    foreign_assets: alloc::vec::Vec<AssetKind>,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let mut steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec::Vec::new();
    for foreign in foreign_assets {
      steps.push(Step {
        precondition: dust_guard(foreign),
        task: Task::SwapIn {
          asset_in: foreign,
          amount_in: AmountResolution::AllAvailable,
          asset_out: AssetKind::Native,
          slippage_tolerance: ecosystem::params::SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      });
    }
    // Final step: burn all accumulated native (only if above dust)
    steps.push(Step {
      precondition: dust_guard(AssetKind::Native),
      task: Task::Burn {
        asset: AssetKind::Native,
        amount: AmountResolution::AllAvailable,
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    steps
      .try_into()
      .expect("burn contract_steps fits within MaxContractSteps")
  }

  /// Builds the Liquidity Actor contract_steps for a specific foreign asset / LP pair.
  ///
  /// Called by governance after pool creation, since LP asset IDs are
  /// pool-specific and unknown at genesis.
  ///
  /// Contract steps:
  /// 1. If Native > dust AND Foreign > dust → AddLiquidity (opportunistic)
  /// 2. If Foreign > dust → SwapIn Foreign→Native with reserve-aware slippage
  /// 3. If LP > dust → SplitTransfer LP to TOL buckets (50/16.67/16.67/16.66)
  pub fn build_zap_contract_steps(
    foreign: AssetKind,
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, SplitLeg, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| {
      Self::all_conditions(alloc::vec![
        Predicate::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Predicate::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ])
    };
    let slippage_tolerance = Self::resolve_zap_slippage_tolerance(foreign);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      // Step 1: Opportunistic LP provisioning — add both sides at current pool ratio
      // AllAvailable for native subtracts ED at resolution layer, safe with Preserve semantics
      Step {
        precondition: dual_dust_guard(AssetKind::Native, foreign),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: foreign,
          amount_a: AmountResolution::AllAvailable,
          amount_b: AmountResolution::AllAvailable,
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 2: Patriotic accumulation — convert leftover Foreign to Native
      Step {
        precondition: dust_guard(foreign),
        task: Task::SwapIn {
          asset_in: foreign,
          amount_in: AmountResolution::AllAvailable,
          asset_out: AssetKind::Native,
          slippage_tolerance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 3: Distribute LP tokens to TOL buckets
      Step {
        precondition: dust_guard(lp_asset),
        task: Task::SplitTransfer {
          asset: lp_asset,
          amount: AmountResolution::AllAvailable,
          legs: alloc::vec![
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_A_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_B_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_C_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_D_ALLOCATION,
            },
          ]
          .try_into()
          .expect("4 bucket legs fit"),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("Liquidity Actor contract_steps fits within MaxContractSteps")
  }

  /// Builds the Bucket-side half of production-admissible LP unwind.
  ///
  /// The Bucket transfers a bounded LP fraction into the paired Treasury sovereign.
  /// The Treasury then removes liquidity in its own independently admitted cycle.
  pub fn build_bucket_lp_transfer_contract_steps(
    lp_asset: AssetKind,
    dust_threshold: Balance,
    unwind_pct: polkadot_sdk::sp_runtime::Perbill,
    treasury_actor_id: u64,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let treasury_account =
      pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(treasury_actor_id);
    alloc::vec![Step {
      precondition: Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]),
      task: Task::Transfer {
        to: treasury_account,
        asset: lp_asset,
        amount: AmountResolution::PercentageOfCurrent(unwind_pct),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("single-step Bucket LP transfer fits")
  }

  /// Builds the Treasury-side half of production-admissible LP unwind.
  ///
  /// Removing all preservable LP leaves both underlying assets in Treasury custody.
  pub fn build_treasury_lp_unwind_contract_steps(
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> Result<pallet_deos_actors::ContractSteps<Runtime>, DispatchError> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let AssetKind::Local(lp_id) = lp_asset else {
      return Err(DispatchError::Other("TreasuryLpUnwindRequiresLocalLpAsset"));
    };
    let (asset_a, asset_b) = crate::DeosRouter::lp_pair_by_token_id(lp_id).ok_or(
      DispatchError::Other("TreasuryLpUnwindRequiresRegisteredLpPair"),
    )?;
    alloc::vec![Step {
      precondition: Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]),
      task: Task::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount: AmountResolution::AllAvailable,
        min_amount_a: 1,
        min_amount_b: 1,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .map_err(|_| DispatchError::Other("TreasuryLpUnwindContractStepsOverflow"))
  }

  /// Builds the BLDR Splitter contract_steps.
  ///
  /// Receives the minted $BLDR liquidity share from TMC output and splits it 50/50
  /// between BLDR liquidity and treasury lanes. TMC routes collateral directly to
  /// the BLDR Liquidity Actor.
  pub fn build_bldr_splitter_contract_steps(
    bldr_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, SplitLeg, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let bldr_liquidity_account = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_LIQUIDITY_ACTOR_ID,
    );
    let bldr_treasury_account = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![Step {
      precondition: dust_guard(bldr_asset),
      task: Task::SplitTransfer {
        asset: bldr_asset,
        amount: AmountResolution::AllAvailable,
        legs: alloc::vec![
          SplitLeg {
            to: bldr_liquidity_account,
            share: ecosystem::params::BLDR_SPLITTER_LIQUIDITY_SHARE,
          },
          SplitLeg {
            to: bldr_treasury_account,
            share: ecosystem::params::BLDR_SPLITTER_TREASURY_SHARE,
          },
        ]
        .try_into()
        .expect("2 legs fit"),
      },
      on_error: StepErrorPolicy::AbortCycle,
    },];
    steps
      .try_into()
      .expect("BLDR splitter contract_steps fits within MaxContractSteps")
  }

  /// Builds the BLDR Liquidity Actor contract_steps for NTVE-BLDR provisioning.
  ///
  /// Contract steps:
  /// 1. AddLiquidity(NTVE, BLDR) — opportunistic at current pool ratio
  /// 2. Transfer(LP → BLDR Anchor, 100%)
  pub fn build_bldr_liquidity_contract_steps(
    bldr_asset: AssetKind,
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| {
      Self::all_conditions(alloc::vec![
        Predicate::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Predicate::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ])
    };
    let bldr_anchor = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID,
    );
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      Step {
        precondition: dual_dust_guard(AssetKind::Native, bldr_asset),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: bldr_asset,
          amount_a: AmountResolution::AllAvailable,
          amount_b: AmountResolution::AllAvailable,
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      Step {
        precondition: dust_guard(lp_asset),
        task: Task::Transfer {
          to: bldr_anchor,
          asset: lp_asset,
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("BLDR Liquidity Actor contract_steps fits within MaxContractSteps")
  }

  /// Builds and activates the Native Staking Liquidity Actor execution plan.
  ///
  /// Contract steps:
  /// 1. DonateLiquidity — stake the calculated NTVE side and donate balanced reserves
  pub fn activate_native_staking_liquidity_actor(
    dust_threshold: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Self::ensure_native_staking_liquidity_ready()?;
    let contract_steps = Self::build_native_staking_liquidity_contract_steps(dust_threshold);
    crate::Actors::activate_actor(
      RuntimeOrigin::root(),
      ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
      pallet_deos_actors::ActorContract {
        trigger: pallet_deos_actors::Trigger::address_event(
          pallet_deos_actors::SourceFilter::Any,
          pallet_deos_actors::AssetFilter::Any,
        ),
        cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
        window: None,
        steps: contract_steps,
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: pallet_deos_actors::FundingSourcePolicy::RuntimePolicy,
        auto_close_at_cycle_nonce: None,
      },
    )
  }

  pub fn ensure_native_staking_liquidity_ready() -> polkadot_sdk::sp_runtime::DispatchResult {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      staked_asset_id,
    ) {
      return Err(DispatchError::Other("StakedAssetUnavailable"));
    }
    pallet_staking::Pools::<Runtime>::get(native_asset_id)
      .ok_or(DispatchError::Other("NativeStakingPoolUnavailable"))?;
    let actor_id = ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID;
    if crate::Actors::active_actor_state(actor_id).is_none()
      && crate::Actors::actor_identities(actor_id).is_none()
    {
      return Err(DispatchError::Other(
        "NativeStakingLiquidityActorUnavailable",
      ));
    }
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    AssetConversion::get_reserves(base_asset, staked_asset)
      .map_err(|_| DispatchError::Other("NativeStakingAmmUnavailable"))?;
    Ok(())
  }

  pub fn build_native_staking_liquidity_contract_steps(
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let native_staking_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let native_asset = AssetKind::Local(native_staking_asset_id);
    let staked_asset_id = crate::Staking::staked_asset_id(native_staking_asset_id)
      .expect("native staking liquidity activation checks staked asset first");
    let staked_asset = AssetKind::Local(staked_asset_id);
    let native_dust = Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
      asset: native_asset,
      threshold: dust_threshold,
    }]);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![Step {
      precondition: native_dust,
      task: Task::DonateLiquidity {
        asset_a: native_asset,
        asset_b: staked_asset,
        max_amount_a: AmountResolution::AllAvailable,
        max_ratio_error: ecosystem::params::NATIVE_STAKING_LP_DONATION_MAX_RATIO_ERROR,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }];
    steps
      .try_into()
      .expect("native staking liquidity execution plan fits within MaxContractSteps")
  }

  /// Builds the Treasury B BLDR buyback-and-burn contract_steps.
  ///
  /// Contract steps:
  /// 1. SwapIn(NTVE → target) — amount resolved as % of current NTVE balance
  /// 2. Burn(target, AllAvailable) — destroy all acquired tokens
  ///
  /// Multiple small buybacks per day create smooth market pressure.
  pub fn build_treasury_b_buyback_contract_steps(
    target_asset: AssetKind,
    buyback_pct: polkadot_sdk::sp_runtime::Perbill,
    dust_threshold: Balance,
    slippage: polkadot_sdk::sp_runtime::Perbill,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    use pallet_deos_actors::{AmountResolution, Predicate, Step, StepErrorPolicy, Task};
    let native_dust = Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
      asset: AssetKind::Native,
      threshold: dust_threshold,
    }]);
    let target_dust = Self::all_conditions(alloc::vec![Predicate::BalanceAbove {
      asset: target_asset,
      threshold: dust_threshold,
    }]);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      // Step 1: Swap NTVE → target (% of current balance)
      Step {
        precondition: native_dust,
        task: Task::SwapIn {
          asset_in: AssetKind::Native,
          amount_in: AmountResolution::PercentageOfCurrent(buyback_pct),
          asset_out: target_asset,
          slippage_tolerance: slippage,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      // Step 2: Burn all acquired target tokens
      Step {
        precondition: target_dust,
        task: Task::Burn {
          asset: target_asset,
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("Treasury B buyback contract_steps fits within MaxContractSteps")
  }
}

pub struct TmctolStakingOps;
impl TmctolStakingOps {
  fn staking_asset_id(asset: AssetKind) -> u32 {
    match asset {
      AssetKind::Native => <Runtime as pallet_staking::Config>::NativeStakingAssetId::get(),
      AssetKind::Foreign(id) | AssetKind::Local(id) => id,
    }
  }
}

impl pallet_deos_actors::adapters::StakingOps<AccountId, AssetKind, Balance> for TmctolStakingOps {
  fn stake(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
      let staking_asset_id = Self::staking_asset_id(asset);
      let _ = crate::Staking::stake(
        RuntimeOrigin::signed(who.clone()).into(),
        staking_asset_id,
        amount,
      )?;
      Ok(())
    })()
    .map_err(TaskFailure::permanent)
  }

  fn unstake(who: &AccountId, asset: AssetKind, shares: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
      let _ = crate::Staking::unstake(
        RuntimeOrigin::signed(who.clone()).into(),
        Self::staking_asset_id(asset),
        shares,
      )?;
      Ok(())
    })()
    .map_err(TaskFailure::permanent)
  }

  fn share_balance(who: &AccountId, asset: AssetKind) -> Balance {
    crate::Staking::effective_share_balance_for_queries(Self::staking_asset_id(asset), who)
      .unwrap_or_default()
  }

  fn share_asset(asset: AssetKind) -> Option<AssetKind> {
    crate::Staking::staked_asset_id_for_queries(Self::staking_asset_id(asset)).map(AssetKind::Local)
  }
}

pub struct DeosFundingAuthority;

impl FundingAuthority<AccountId> for DeosFundingAuthority {
  fn permits(
    _: pallet_deos_actors::ActorId,
    _: &AccountId,
    _: Option<&AccountId>,
    _: Option<&pallet_deos_actors::FundingProvenance>,
  ) -> bool {
    // The reference launch line has no source/actor authorization entries.
    // Downstream runtimes must add explicit pairs rather than inheriting trust
    // from an account-shaped signed, internal-protocol, or XCM identity.
    false
  }
}

pub struct DeosSovereignAccountDeriver;

impl pallet_deos_actors::SovereignAccountDeriver<AccountId> for DeosSovereignAccountDeriver {
  fn user(pallet_id: PalletId, owner: &AccountId, owner_slot: u8) -> AccountId {
    AccountId::new(polkadot_sdk::sp_io::hashing::blake2_256(
      &(pallet_id, b"user", owner, owner_slot).encode(),
    ))
  }

  fn system(pallet_id: PalletId, actor_id: pallet_deos_actors::ActorId) -> AccountId {
    AccountId::new(polkadot_sdk::sp_io::hashing::blake2_256(
      &(pallet_id, b"system", actor_id).encode(),
    ))
  }
}

/// Derived sovereign accounts must never collide with host-reserved identities.
///
/// The reference runtime marks the Fee Sink and the reserved deterministic System Actors custody
/// accounts as reserved so a hashed sovereign derivation can never alias them.
pub struct DeosSovereignAccountPolicy;

impl pallet_deos_actors::adapters::SovereignAccountPolicy<AccountId>
  for DeosSovereignAccountPolicy
{
  fn is_reserved(account: &AccountId) -> bool {
    // The deterministic genesis System Actors custody accounts (including the Fee Sink) are
    // host-reserved; a hashed sovereign derivation can never alias them.
    account == &crate::configs::governance_config::governance_vote_power_custody_account()
      || (primitives::ecosystem::actor_ids::BURN_ACTOR_ID
        ..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID)
        .any(|id| {
          account == &pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(id)
        })
  }
}

pub struct TmctolObservationProvider;

impl pallet_deos_actors::ObservationProvider<primitives::OracleFeedId, crate::BlockNumber>
  for TmctolObservationProvider
{
  fn current(feed: &primitives::OracleFeedId) -> pallet_deos_actors::CanonicalObservationState {
    let Some(config) = pallet_oracle::Feeds::<Runtime>::get(*feed) else {
      return pallet_deos_actors::CanonicalObservationState::Unavailable;
    };
    if config.lifecycle == pallet_oracle::FeedLifecycle::Deactivated {
      return pallet_deos_actors::CanonicalObservationState::Unavailable;
    }
    match pallet_oracle::Observations::<Runtime>::get(*feed) {
      Some(observation) => pallet_deos_actors::CanonicalObservationState::Available {
        value: observation.value,
        revision: observation.revision,
      },
      None => pallet_deos_actors::CanonicalObservationState::Uninitialized,
    }
  }

  fn observe(
    feed: &primitives::OracleFeedId,
    _now: crate::BlockNumber,
    max_age_blocks: u32,
  ) -> pallet_deos_actors::ScalarObservationState<crate::BlockNumber> {
    match crate::Oracle::observation_state(*feed, max_age_blocks) {
      Ok(pallet_oracle::ObservationState::Fresh(observation)) => {
        pallet_deos_actors::ScalarObservationState::Fresh {
          value: observation.value,
          observed_at: observation.updated_at,
        }
      }
      Ok(pallet_oracle::ObservationState::Uninitialized) => {
        pallet_deos_actors::ScalarObservationState::Uninitialized
      }
      Ok(pallet_oracle::ObservationState::Stale(_)) => {
        pallet_deos_actors::ScalarObservationState::Stale
      }
      Ok(pallet_oracle::ObservationState::Unavailable) | Err(_) => {
        pallet_deos_actors::ScalarObservationState::Unavailable
      }
    }
  }
}

pub struct DeosActorPrepassContext;

impl ActorPrepassContext for DeosActorPrepassContext {
  fn context_ready() -> bool {
    Timestamp::get() != 0
      && polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::exists()
  }
}

impl pallet_deos_actors::Config for Runtime {
  type PalletId = ActorsPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetKind;
  type FeeNativeAssetId = ActorFeeNativeAssetId;
  type Balance = Balance;
  type AssetOps = TmctolAssetOps;
  type AdmissionCertificateAuthority = RuntimeAdmissionCertificateAuthority;
  type StepControlWeight = RuntimeStepControlWeight;
  type TaskEffectWeight = TmctolTaskEffectWeight;
  type ObservationFeedId = primitives::OracleFeedId;
  type ObservationProvider = TmctolObservationProvider;
  type FundingAuthority = DeosFundingAuthority;
  type SovereignAccountDeriver = DeosSovereignAccountDeriver;
  type SovereignAccountPolicy = DeosSovereignAccountPolicy;
  type DexOps = TmctolDexOps;
  type StakingOps = TmctolStakingOps;
  type LiquidityOps = TmctolLiquidityOps;
  type Time = Timestamp;
  type CadenceTickMillis = ActorCadenceTickMillis;
  type ActorCreationFee = ActorCreationFee;
  type RuntimeHoldReason = RuntimeHoldReason;
  type StateHoldCurrency = Balances;
  type ActorStateHoldBase = ActorStateHoldBase;
  type ActorStateHoldPerByte = ActorStateHoldPerByte;
  type FeeSink = ActorFeeRecipient;
  type FeeCollector = TmctolFeeCollector;
  type GenesisSystemActors = TmctolGenesisSystemActors;
  type SystemActorContractValidator = DeosSystemActorContractValidator;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxActiveActors = ActorMaxActiveActors;
  type MaxActorIdentities = ActorMaxActiveActors;
  type MaxSystemSovereigns = ActorMaxActiveActors;
  type MaxOpeningPredicateResults = ActorMaxOpeningPredicateResults;
  type MaxPreconditionClauses = ActorMaxPreconditionClauses;
  type MaxPredicatesPerClause = ActorMaxPredicatesPerClause;
  type MaxPredicatesPerStep = ActorMaxPredicatesPerStep;
  type MaxConsecutiveFailures = ActorMaxConsecutiveFailures;
  type MaxRetryAttempts = ActorMaxRetryAttempts;
  type MaxAutoCloseNonceHorizon = ActorMaxAutoCloseNonceHorizon;
  type TargetBlockTime = ActorTargetBlockTime;
  type MaxExecutionDelayBlocks = ActorMaxExecutionDelayBlocks;
  type MaxTemporalDelayTicks = ActorMaxTemporalDelayTicks;
  type MaxExecutionsPerBlock = ActorMaxExecutionsPerBlock;
  type MaxQueueLength = ActorMaxQueueLength;
  type QueuePageSize = ActorQueuePageSize;
  type WakeupPageSize = ActorWakeupPageSize;
  type ObservationPageSize = ActorObservationPageSize;
  type CrossingPageSize = ActorCrossingPageSize;
  type MaxCrossingTransitionsPerFeed = ActorMaxCrossingTransitionsPerFeed;
  type MaxCrossingMembersPerFeed = ActorMaxCrossingMembersPerFeed;
  type MaxUserCrossingMembersPerFeed = ActorMaxUserCrossingMembersPerFeed;
  type MaxCrossingTransitionsPerBlock = ActorMaxCrossingTransitionsPerBlock;
  type MaxCrossingLeavesPerBlock = ActorMaxCrossingLeavesPerBlock;
  type MaxCrossingPagesPerBlock = ActorMaxCrossingPagesPerBlock;
  type MaxCrossingActorsPerBlock = ActorMaxCrossingActorsPerBlock;
  type CrossingWorkerWeightLimit = ActorCrossingWorkerWeightLimit;
  type MaxQueueEntriesScannedPerBlock = ActorMaxQueueEntriesScannedPerBlock;
  type MaxObservationFanoutPagesPerBlock = ActorMaxObservationFanoutPagesPerBlock;
  type ObservationFanoutWeightLimit = ActorObservationFanoutWeightLimit;
  type WakeupWeightLimit = ActorWakeupWeightLimit;
  type MaxWakeupsPerBlock = ActorMaxWakeupsPerBlock;
  type MaxFundingTrackedAssets = ActorMaxFundingTrackedAssets;
  type MaxOpeningSnapshotEntries = ActorMaxOpeningSnapshotEntries;
  type MaxIdleStarvationBlocks = ActorMaxIdleStarvationBlocks;
  type ActorOnIdleReserve = ActorOnIdleReserve;
  type MaxOwnerSlots = ActorMaxOwnerSlots;
  type MaxContractSteps = ActorMaxContractSteps;
  type MaxSplitTransferLegs = ActorMaxSplitTransferLegs;
  type MaxSweepBatch = ActorMaxSweepBatch;
  type MaxWhitelistSize = ActorMaxWhitelistSize;
  type MinUserBalance = ActorMinUserBalanceGuard;
  type MinWindowLength = ActorMinWindowLength;
  type WeightInfo = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  type BlockResourceBudget = crate::configs::BlockResourceBudgetValue;
  type PrepassContext = DeosActorPrepassContext;
  type WeightToFee = crate::WeightToFee;
  // Runtime binds task upper bounds so fee admission stays chain-specific and auditable
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeActorsBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeActorsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl RuntimeActorsBenchmarkHelper {
  fn ensure_local_asset(asset_id: u32, owner: &AccountId) -> Result<(), DispatchError> {
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(asset_id) {
      pallet_assets::Pallet::<Runtime>::force_create(
        RuntimeOrigin::root(),
        asset_id,
        polkadot_sdk::sp_runtime::MultiAddress::Id(owner.clone()),
        true,
        1,
      )?;
    }
    Ok(())
  }

  /// Adds a thin direct pool for a non-Native pair so Router quotes both the direct XYK candidate
  /// and the Native-anchored path. Liquidity stays three orders below the Native pools, so the
  /// multi-hop route still wins execution while the extra candidate is still enumerated, which is
  /// the worst case for route selection.
  fn ensure_thin_direct_pool(
    owner: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    liquidity: Balance,
  ) -> Result<(), DispatchError> {
    crate::DeosRouter::create_pool(RuntimeOrigin::signed(owner.clone()), asset_a, asset_b)
      .map_err(|_| DispatchError::Other("CreateDirectPoolForBenchmarkFailed"))?;
    let pool_account =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("DirectPoolAddressUnavailable"))?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&pool_account, EXISTENTIAL_DEPOSIT);
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("DirectPoolIdUnavailable"))?;
    let pool_info = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .ok_or(DispatchError::Other("DirectPoolNotCreated"))?;
    if <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
      u32,
      AccountId,
    >>::should_touch(pool_info.lp_token, owner)
      && <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
        u32,
        AccountId,
      >>::touch(pool_info.lp_token, owner, owner)
      .is_err()
    {
      return Err(DispatchError::Other("TouchDirectLpAccountFailed"));
    }
    AssetConversion::add_liquidity(
      RuntimeOrigin::signed(owner.clone()),
      alloc::boxed::Box::new(asset_a),
      alloc::boxed::Box::new(asset_b),
      liquidity,
      liquidity,
      0,
      0,
      owner.clone(),
    )
    .map_err(|_| DispatchError::Other("AddDirectPoolLiquidityFailed"))?;
    Ok(())
  }

  /// Registers a TMC curve on the swap target so Router enumerates its third candidate family
  /// alongside direct XYK and the Native-anchored path. The curve is priced so minting loses to
  /// XYK, which keeps the multi-hop route executing while every candidate is still quoted.
  fn ensure_losing_tmc_curve(
    token_asset: AssetKind,
    collateral_asset: AssetKind,
  ) -> Result<(), DispatchError> {
    if pallet_tmc::Pallet::<Runtime>::has_curve(token_asset) {
      return Ok(());
    }
    pallet_tmc::Pallet::<Runtime>::create_curve(
      RuntimeOrigin::root(),
      token_asset,
      collateral_asset,
      ecosystem::params::PRECISION,
      0,
    )
  }

  /// Publishes the Router pool reference a System swap consults, so `ensure_system_reference_price`
  /// measures its full Oracle-plus-reserves read path instead of missing early. Both benchmark
  /// pools carry equal liquidity, so the pair prices 1:1 at `PRECISION` and a two-leg fee of about
  /// one percent stays well inside `MAX_SYSTEM_PRICE_DEVIATION`.
  fn publish_system_reference_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
  ) -> Result<(), DispatchError> {
    let producer = crate::DeosRouter::account_id();
    crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)?;
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    crate::Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      ecosystem::params::PRECISION,
    )?;
    Ok(())
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_deos_actors::BenchmarkHelper<AccountId, AssetKind, Balance, primitives::OracleFeedId>
  for RuntimeActorsBenchmarkHelper
{
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance, Balance), DispatchError> {
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    let local_asset_id = 300_000;
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(local_asset_id);
    Self::ensure_local_asset(local_asset_id, owner)?;
    let amount: Balance = 1_000_000_000_000;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(owner, amount.saturating_mul(2));
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      local_asset_id,
      owner,
      amount.saturating_add(1),
    )?;
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
    if pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id) {
      return Err(DispatchError::Other("AddLiquidityPoolAlreadyExists"));
    }
    Ok((asset_a, asset_b, amount, amount))
  }

  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance), DispatchError> {
    let asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    Self::ensure_local_asset(asset_id, owner)?;
    let liquidity: Balance = 1_000_000_000;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      owner,
      EXISTENTIAL_DEPOSIT.saturating_mul(100),
    );
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      asset_id,
      owner,
      liquidity.saturating_mul(3),
    )?;
    if !pallet_staking::Pools::<Runtime>::contains_key(asset_id) {
      crate::Staking::register_staking_asset(RuntimeOrigin::root(), asset_id)?;
    }
    crate::Staking::stake(RuntimeOrigin::signed(owner.clone()), asset_id, liquidity)?;
    let staked_asset_id = crate::Staking::staked_asset_id(asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    let asset_a = AssetKind::Local(asset_id);
    let asset_b = AssetKind::Local(staked_asset_id);
    crate::DeosRouter::create_pool(RuntimeOrigin::signed(owner.clone()), asset_a, asset_b)?;
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
    let pool_info = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .ok_or(DispatchError::Other("PoolNotCreated"))?;
    if <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
      u32,
      AccountId,
    >>::should_touch(pool_info.lp_token, owner)
    {
      <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
        u32,
        AccountId,
      >>::touch(pool_info.lp_token, owner, owner)?;
    }
    AssetConversion::add_liquidity(
      RuntimeOrigin::signed(owner.clone()),
      alloc::boxed::Box::new(asset_a),
      alloc::boxed::Box::new(asset_b),
      liquidity / 2,
      liquidity / 2,
      0,
      0,
      owner.clone(),
    )?;
    Ok((asset_a, asset_b, liquidity / 10))
  }

  fn setup_remove_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, AssetKind, Balance), DispatchError> {
    let pool_count = 2u32;
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    let liquidity = 1_000_000_000_000u128;
    let native_seed = liquidity.saturating_mul(pool_count.saturating_add(1) as u128);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(owner, native_seed);
    let mut target_lp: Option<(AssetKind, AssetKind, AssetKind, Balance)> = None;
    for i in 0..pool_count {
      let local_asset_id = 100_000u32.saturating_add(i);
      if Self::ensure_local_asset(local_asset_id, owner).is_err() {
        return Err(DispatchError::Other("EnsureLocalAssetFailed"));
      }
      if <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
        local_asset_id,
        owner,
        liquidity.saturating_add(1_000_000_000),
      )
      .is_err()
      {
        return Err(DispatchError::Other("MintLocalForBenchmarkFailed"));
      }
      let asset_a = AssetKind::Native;
      let asset_b = AssetKind::Local(local_asset_id);
      if crate::DeosRouter::create_pool(RuntimeOrigin::signed(owner.clone()), asset_a, asset_b)
        .is_err()
      {
        return Err(DispatchError::Other("CreatePoolForBenchmarkFailed"));
      }
      let pool_account =
        <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(&asset_a, &asset_b)
          .map_err(|_| DispatchError::Other("PoolAddressUnavailable"))?;
      let _ =
        <Balances as Currency<AccountId>>::deposit_creating(&pool_account, EXISTENTIAL_DEPOSIT);
      let pool_id =
        <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
          .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
      let pool_info = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
        .ok_or(DispatchError::Other("PoolNotCreated"))?;
      if <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
        u32,
        AccountId,
      >>::should_touch(pool_info.lp_token, owner)
        && <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
          u32,
          AccountId,
        >>::touch(pool_info.lp_token, owner, owner)
        .is_err()
      {
        return Err(DispatchError::Other("TouchLpAccountForBenchmarkFailed"));
      }
      if AssetConversion::add_liquidity(
        RuntimeOrigin::signed(owner.clone()),
        alloc::boxed::Box::new(asset_a),
        alloc::boxed::Box::new(asset_b),
        liquidity,
        liquidity,
        0,
        0,
        owner.clone(),
      )
      .is_err()
      {
        return Err(DispatchError::Other("AddLiquidityForBenchmarkFailed"));
      }
      if i.saturating_add(1) == pool_count {
        let lp_amount = <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(
          pool_info.lp_token,
          owner,
        );
        let min_native_reserve = <Balances as NativeInspect<AccountId>>::minimum_balance();
        let benchmark_lp_amount = lp_amount.saturating_sub(min_native_reserve);
        if benchmark_lp_amount == 0 {
          return Err(DispatchError::Other("LpAmountTooSmallForBenchmark"));
        }
        target_lp = Some((
          AssetKind::Local(pool_info.lp_token),
          asset_a,
          asset_b,
          benchmark_lp_amount,
        ));
      }
    }
    target_lp.ok_or(DispatchError::Other("TargetLpMissing"))
  }

  fn setup_stake(owner: &AccountId) -> Result<(AssetKind, Balance), DispatchError> {
    let asset_id = 200_000;
    let amount: Balance = 1_000_000;
    Self::ensure_local_asset(asset_id, owner)?;
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      asset_id,
      owner,
      amount.saturating_add(1),
    )?;
    crate::Staking::register_staking_asset(RuntimeOrigin::root(), asset_id)?;
    Ok((AssetKind::Local(asset_id), amount))
  }

  fn setup_unstake(owner: &AccountId) -> Result<(AssetKind, Balance), DispatchError> {
    let (asset, amount) = Self::setup_stake(owner)?;
    <TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<AccountId, AssetKind, Balance>>::stake(
      owner, asset, amount,
    )
    .map_err(|failure| failure.error)?;
    let shares = <TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<
      AccountId,
      AssetKind,
      Balance,
    >>::share_balance(owner, asset);
    if shares == 0 {
      return Err(DispatchError::Other("UnstakeSharesMissing"));
    }
    Ok((asset, shares))
  }

  fn set_asset_account_frozen(
    owner: &AccountId,
    who: &AccountId,
    asset: AssetKind,
    frozen: bool,
  ) -> DispatchResult {
    let AssetKind::Local(id) = asset else {
      return Err(DispatchError::Other("UnsupportedAccountFreezeAsset"));
    };
    let balance = crate::Assets::balance(id, who);
    let issuance =
      <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::total_issuance(id);
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> DispatchResult {
        if frozen {
          crate::Assets::freeze(RuntimeOrigin::signed(owner.clone()), id, who.clone().into())?;
        } else {
          crate::Assets::thaw(RuntimeOrigin::signed(owner.clone()), id, who.clone().into())?;
        }
        if crate::Assets::balance(id, who) != balance
          || <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::total_issuance(id)
            != issuance
        {
          return Err(DispatchError::Other("AccountFreezeChangedCustody"));
        }
        if frozen && TmctolAssetOps::balance(who, asset) != 0 {
          return Err(DispatchError::Other("FrozenAssetStillSpendable"));
        }
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn remove_empty_staking_receipt(owner: &AccountId, asset: AssetKind) -> DispatchResult {
    if !matches!(asset, AssetKind::Native | AssetKind::Local(_)) {
      return Err(DispatchError::Other("UnsupportedStakingReceiptAsset"));
    }
    let share_asset = || {
      <TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<
        AccountId,
        AssetKind,
        Balance,
      >>::share_asset(asset)
    };
    let Some(AssetKind::Local(receipt)) = share_asset() else {
      return Err(DispatchError::Other("LiveStakingReceiptMissing"));
    };
    if <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::total_issuance(receipt) != 0
    {
      return Err(DispatchError::Other("StakingReceiptNotEmpty"));
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> DispatchResult {
        crate::Assets::start_destroy(RuntimeOrigin::root(), receipt)?;
        crate::Assets::finish_destroy(RuntimeOrigin::signed(owner.clone()), receipt)?;
        if share_asset().is_some() {
          return Err(DispatchError::Other("StakingReceiptMappingRemains"));
        }
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance), DispatchError> {
    let _ = Self::setup_remove_liquidity(owner)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurnActorAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    let asset_in = AssetKind::Local(100_000);
    let asset_out = AssetKind::Local(100_001);
    Self::ensure_thin_direct_pool(owner, asset_in, asset_out, 1_000_000_00)?;
    Self::ensure_losing_tmc_curve(asset_out, asset_in)?;
    Self::publish_system_reference_price(asset_in, asset_out)?;
    Ok((asset_in, asset_out, 1_000_000))
  }

  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance, Balance), DispatchError> {
    let _ = Self::setup_remove_liquidity(owner)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurnActorAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    let asset_in = AssetKind::Local(100_000);
    let asset_out = AssetKind::Local(100_001);
    Self::ensure_thin_direct_pool(owner, asset_in, asset_out, 1_000_000_00)?;
    Self::ensure_losing_tmc_curve(asset_out, asset_in)?;
    Self::publish_system_reference_price(asset_in, asset_out)?;
    Ok((asset_in, asset_out, 100_000, 1_000_000_000))
  }

  fn funding_assets(max: u32) -> alloc::vec::Vec<AssetKind> {
    (0..max)
      .map(|index| {
        if index == 0 {
          AssetKind::Native
        } else {
          AssetKind::Local(index)
        }
      })
      .collect()
  }

  fn setup_predicate_assets(
    owner: &AccountId,
    max: u32,
  ) -> Result<alloc::vec::Vec<AssetKind>, DispatchError> {
    let assets = Self::funding_assets(max);
    for asset in &assets {
      if let AssetKind::Local(asset_id) = asset {
        Self::ensure_local_asset(*asset_id, owner)?;
      }
    }
    Ok(assets)
  }

  fn setup_observation_feeds(
    max: u32,
  ) -> Result<alloc::vec::Vec<primitives::OracleFeedId>, DispatchError> {
    let producer = crate::DeosRouter::account_id();
    let mut feeds = alloc::vec::Vec::with_capacity(max as usize);
    for index in 1..=max {
      let asset_in = AssetKind::Local(0x3000_0000u32.saturating_add(index));
      let asset_out = AssetKind::Native;
      crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)?;
      let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
      crate::Oracle::publish(RuntimeOrigin::signed(producer.clone()), feed, 1)?;
      feeds.push(feed);
    }
    Ok(feeds)
  }

  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    let transferred = amount.max(EXISTENTIAL_DEPOSIT);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      source,
      transferred.saturating_add(EXISTENTIAL_DEPOSIT),
    );
    let _ = (recipient, transferred);
    Ok(())
  }

  fn run_address_event_ingress(recipient: &AccountId, source: &AccountId, amount: Balance) -> bool {
    // The benchmark mirrors the extension's resolved-match semantics: an absent
    // sovereign is not a producer event at all.
    if crate::Actors::sovereign_index(recipient).is_none() {
      return false;
    }
    let event = pallet_deos_actors::AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset: AssetKind::Native,
      amount,
      provenance: Some(pallet_deos_actors::FundingProvenance::Signed),
    };
    <crate::configs::RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
      AccountId,
      AssetKind,
      Balance,
    >>::notify(&event)
    .is_ok()
  }

  fn setup_xcm_asset_deposit() -> DispatchResult {
    crate::configs::xcm_config::setup_benchmark_foreign_asset()
  }

  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    crate::configs::xcm_config::benchmark_foreign_asset_deposit(recipient, source, amount)
  }

  type MaximumContextInherent = (
    cumulus_pallet_parachain_system::parachain_inherent::BasicParachainInherentData,
    cumulus_pallet_parachain_system::parachain_inherent::InboundMessagesData,
  );

  fn prepare_maximum_context_inherent() -> Self::MaximumContextInherent {
    use polkadot_sdk::frame_support::traits::BuildGenesisConfig;
    parachain_info::GenesisConfig::<Runtime> {
      parachain_id: 100u32.into(),
      _config: Default::default(),
    }
    .build();
    use alloc::{collections::BTreeMap, vec};
    use codec::Decode;
    use cumulus_pallet_parachain_system::parachain_inherent::{
      InboundDownwardMessages, InboundHrmpMessages, InboundMessagesData,
    };
    use polkadot_sdk::cumulus_primitives_core::{
      InboundDownwardMessage, InboundHrmpMessage, ParaId,
    };

    let downward_messages = (0..MaxInboundDownwardMessagesPerContext::get())
      .map(|_| InboundDownwardMessage {
        sent_at: 1,
        msg: vec![0; 65_536],
      })
      .collect::<Vec<_>>();
    let mut horizontal_messages = BTreeMap::new();
    let channels = MaxInboundHorizontalChannelsPerContext::get();
    for message_index in 0..MaxInboundHorizontalMessagesPerContext::get() {
      let sender = ParaId::from(message_index % channels + 1);
      horizontal_messages
        .entry(sender)
        .or_insert_with(Vec::new)
        .push(InboundHrmpMessage {
          sent_at: 1,
          data: vec![0],
        });
    }
    let data =
      cumulus_pallet_parachain_system::parachain_inherent::BasicParachainInherentData::decode(
        &mut &include_bytes!("../../fixtures/maximum-context-basic.scale")[..],
      )
      .expect("validated maximum-context BasicParachainInherentData fixture must decode"); // deos-bypass: panic-owner — fixture hash 720062c34020ad38b5c56e7772befd24a38775a7b3dff547c4b5b28dac97ae17 passed the actual maximum-context dispatch test.
    let collection_size_limit = (RuntimeBlockWeights::get().max_block.proof_size() / 6) as usize;
    let mut size_limit = collection_size_limit;
    let downward = InboundDownwardMessages::new(downward_messages).into_abridged(&mut size_limit);
    size_limit = size_limit.saturating_add(collection_size_limit);
    let horizontal =
      InboundHrmpMessages::from_map(horizontal_messages).into_abridged(&mut size_limit);
    (data, InboundMessagesData::new(downward, horizontal))
  }

  fn execute_maximum_context_inherent(
    (data, inbound): Self::MaximumContextInherent,
  ) -> DispatchResult {
    crate::ParachainSystem::set_validation_data(
      polkadot_sdk::frame_system::RawOrigin::None.into(),
      data,
      inbound,
    )
  }

  fn verify_maximum_context_inherent() {
    assert_eq!(
      polkadot_sdk::cumulus_pallet_parachain_system::ProcessedDownwardMessages::<Runtime>::get(),
      6
    );
  }

  fn prepare_maximum_xcm_version_discovery() {
    use codec::Encode;
    use xcm::{VersionedLocation, latest::prelude::*};

    let queue = (0..<Runtime as polkadot_sdk::pallet_xcm::Config>::VERSION_DISCOVERY_QUEUE_SIZE)
      .map(|index| {
        (
          VersionedLocation::from(Location::new(0, [GeneralIndex(u128::from(index))])),
          index,
        )
      })
      .collect::<Vec<_>>();
    let queue_key = polkadot_sdk::frame_support::storage::storage_prefix(
      b"PolkadotXcm",
      b"VersionDiscoveryQueue",
    );
    let migration_key =
      polkadot_sdk::frame_support::storage::storage_prefix(b"PolkadotXcm", b"CurrentMigration");
    polkadot_sdk::sp_io::storage::set(&queue_key, &queue.encode());
    polkadot_sdk::sp_io::storage::clear(&migration_key);
  }

  fn execute_maximum_xcm_version_discovery() {
    use polkadot_sdk::frame_support::traits::Hooks;
    let _ = crate::PolkadotXcm::on_initialize(1);
  }

  fn verify_maximum_xcm_version_discovery() {
    use codec::Decode;

    let queue_key = polkadot_sdk::frame_support::storage::storage_prefix(
      b"PolkadotXcm",
      b"VersionDiscoveryQueue",
    );
    let encoded = polkadot_sdk::sp_io::storage::get(&queue_key)
      .expect("XCM discovery queue must be rewritten after traversal"); // deos-bypass: panic-owner — benchmark setup writes the exact storage key and on_initialize always rewrites the taken queue.
    let remaining = Vec::<(xcm::VersionedLocation, u32)>::decode(&mut &encoded[..])
      .expect("rewritten XCM discovery queue must decode"); // deos-bypass: panic-owner — pallet_xcm owns the same bounded queue SCALE type used by setup.
    assert!(remaining.is_empty());
  }

  fn prepare_block_resource_meter_extension() {
    use polkadot_sdk::sp_runtime::traits::One;

    let measured_block = crate::System::block_number().saturating_add(One::one());
    let mut state = pallet_deos_actors::BlockResourceState::new(measured_block);
    state
      .begin_prepass()
      .and_then(|()| state.open_external_phase())
      .expect("benchmark state must enter ExternalPhase"); // deos-bypass: panic-owner — fresh state has no reservations and follows the canonical transition.
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
  }

  fn execute_block_resource_meter_extension() {
    use polkadot_sdk::frame_support::dispatch::{GetDispatchInfo, Pays, PostDispatchInfo};
    use polkadot_sdk::sp_runtime::traits::TransactionExtension;

    let extension = crate::configs::resource_meter::BlockResourceMeterExtension;
    let call = crate::RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
    let mut info = call.get_dispatch_info();
    info.extension_weight = extension.weight(&call);
    let origin = crate::RuntimeOrigin::none();
    let reservation = extension
      .prepare((), &origin, &call, &info, 0)
      .expect("prepared benchmark resource state must admit one remark"); // deos-bypass: panic-owner — benchmark setup opens the exact current-block ExternalPhase budget.
    let post_info = PostDispatchInfo {
      actual_weight: Some(info.total_weight()),
      pays_fee: Pays::Yes,
    };
    let _ = crate::configs::resource_meter::BlockResourceMeterExtension::post_dispatch_details(
      reservation,
      &info,
      &post_info,
      0,
      &Ok(()),
    )
    .expect("valid benchmark reservation must settle"); // deos-bypass: panic-owner — actual equals the exact reserved maximum.
  }

  fn verify_block_resource_meter_extension() {
    let state = crate::Actors::block_resource_state()
      .expect("resource extension benchmark must retain current state"); // deos-bypass: panic-owner — setup creates and execution mutates the authoritative state.
    assert_eq!(state.outstanding_reservations(), 0);
    assert!(state.usage().user_dispatch_used() != Weight::zero());
  }
}
