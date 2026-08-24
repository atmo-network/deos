use super::common::{
  ALICE, ASSET_A, BOB, CHARLIE, add_liquidity, create_pool, create_test_asset, deos_router_account,
  mint_tokens, seeded_test_ext, set_consensus_timestamp, update_actor_contract_partial,
};
macro_rules! update_actor_contract_partial {
  ($origin:expr, $actor:expr, $value:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, $value)
  };
  ($origin:expr, $actor:expr, $first:expr, $second:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, ($first, $second))
  };
}

use crate::{
  AccountId, Actors, Address, Assets, Balance, Balances, Executive, MessageQueue, Oracle, Runtime,
  RuntimeCall, RuntimeEvent, RuntimeHoldReason, RuntimeOrigin, Session, SessionRotation, Signature,
  Staking, System, TxExtension, UncheckedExtrinsic, XcmpQueue,
  configs::{
    AuraExtFixedWeight, AuraFixedWeight, AuthorshipFixedWeight, BlockResourceBudgetValue,
    DmpFixedWeight, FixedBlockWeight, FixedBlockWeightComponentsValue,
    MaxInboundDownwardMessagesPerContext, MaxInboundHorizontalChannelsPerContext,
    MaxInboundHorizontalMessagesPerContext, MaximumContextMeasuredWeight,
    MessageQueueServiceWeight, ReservedDmpWeight, ReservedXcmpWeight, RuntimeAddressEventIngress,
    SessionRotationFixedWeight, XcmMigrationMaximumWeight, XcmVersionDiscoveryFixedWeight,
    XcmpRegisteredFixedWeight,
    actor_config::{
      ActorControlInitializationWeight, ActorOnIdleReserve, RuntimeAdmissionCertificateAuthority,
      RuntimeStepControlWeight, TmctolAssetOps, TmctolDexOps, TmctolFeeCollector,
      TmctolGenesisSystemActors, TmctolLiquidityOps, TmctolTaskEffectWeight,
      classify_remove_liquidity_failure, classify_router_execution_failure,
      classify_router_failure, validate_remove_liquidity_output,
    },
    address_event_ingress::AddressEventIngressExtension,
    deos_router_config::{AssetConversionAdapter, market_execution_failure},
    governance_config::GovernanceFixedWeight,
    resource_meter::BlockResourceMeterExtension,
  },
};
use alloc::{
  boxed::Box,
  collections::{BTreeMap, BTreeSet},
};
use codec::Encode;
use pallet_deos_actors::adapters::SovereignAccountPolicy;
use pallet_deos_actors::{
  ActorContract, ActorId, ActorType, AdmissionCertificateAuthorityProvider, AmountResolution,
  AssetFilter, AssetFilterOf, AssetOps, AttemptDisposition, CloseReason, CompletionPolicy,
  ContextMessageGeometry, ContextMessageLimits, ContractSteps, CrossingDirection, CycleResult,
  DexOps, Error, Event, ExecutionContext, FeeCollector, FundingSourcePolicy, IdleStarvationState,
  InputLimit, LiquidityOps, Mutability, OutcomeTotals, RetryClass, ScheduleWindow, SimulationMode,
  SourceFilter, SourceFilterOf, SplitLeg, SplitTransferLegsOf, StakingOps, StepControlExecution,
  StepControlOutcome, StepControlPhase, StepControlPlacement, StepControlWeightContext,
  StepControlWeightProvider, StepErrorPolicy, StepOf, StepOutcome, StepSkippedReason, Task,
  TaskEffectExecution, TaskEffectWeightProvider, TaskOf, Trigger, TriggerFamily, WakeupKey,
  WeightInfo,
};
use pallet_deos_router::{AssetConversionApi, FeeRoutingAdapter};
use polkadot_sdk::frame_support::{
  BoundedVec, assert_noop, assert_ok,
  dispatch::{DispatchClass, GetDispatchInfo},
  traits::{
    Currency, ExistenceRequirement, Get, GetStorageVersion, Hooks, PalletInfoAccess,
    ReservableCurrency, StorageVersion,
    fungible::InspectHold,
    fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
    tokens::imbalance::{ImbalanceAccounting, UnsafeConstructorDestructor, UnsafeManualAccounting},
  },
  weights::Weight,
};
use polkadot_sdk::sp_runtime::traits::{AccountIdConversion, TransactionExtension};
use polkadot_sdk::sp_runtime::{
  DispatchError, Perbill, generic, transaction_validity::TransactionSource,
};
use polkadot_sdk::sp_weights::{WeightMeter, WeightToFee};
use polkadot_sdk::{
  cumulus_primitives_core::{InboundDownwardMessage, InboundHrmpMessage, ParaId},
  sp_core::{Pair, crypto::Ss58Codec, sr25519},
};
use polkadot_sdk::{
  staging_xcm as xcm,
  staging_xcm_executor::{AssetsInHolding, traits::TransactAsset},
};
use primitives::AssetKind;

fn conservative_actor_resource_limits() -> (Weight, Weight) {
  let schedulable = ActorOnIdleReserve::get();
  let control = Perbill::from_percent(20) * schedulable;
  let shared = schedulable
    .checked_sub(&control)
    .expect("control share fits conservative schedulable reserve");
  let actor_base_turn = Perbill::from_percent(50) * shared;
  (control, actor_base_turn)
}

#[test]
fn task_effect_weight_provider_owns_all_task_families() {
  assert!(
    <TmctolTaskEffectWeight as TaskEffectWeightProvider<TaskOf<Runtime>>>::production_weight_identity()
      .is_some()
  );
  assert!(RuntimeAdmissionCertificateAuthority::current().is_some());
  let (_, actor_base_turn) = conservative_actor_resource_limits();
  let swap = TaskOf::<Runtime>::SwapIn {
    asset_in: AssetKind::Native,
    amount_in: AmountResolution::Fixed(1),
    asset_out: AssetKind::Local(ASSET_A),
    slippage_tolerance: Perbill::zero(),
  };
  let swap_weight = TmctolTaskEffectWeight::maximum_effect_weight(&swap)
    .expect("SwapIn has a production Router envelope");
  assert_eq!(
    TmctolTaskEffectWeight::actual_effect_weight(&swap, TaskEffectExecution::NotInvoked),
    Some(Weight::zero()),
  );
  assert_eq!(
    TmctolTaskEffectWeight::actual_effect_weight(&swap, TaskEffectExecution::Invoked),
    Some(swap_weight),
  );
  assert!(swap_weight.ref_time() > 0);
  assert!(swap_weight.proof_size() > 0);
  assert!(swap_weight.all_lte(actor_base_turn));
  for task in [
    TaskOf::<Runtime>::Stake {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    TaskOf::<Runtime>::Unstake {
      asset: AssetKind::Native,
      shares: AmountResolution::Fixed(1),
    },
  ] {
    let weight = TmctolTaskEffectWeight::maximum_effect_weight(&task)
      .expect("staking Task has a production envelope");
    assert!(weight.ref_time() > 0);
    assert!(weight.proof_size() > 0);
    assert!(weight.all_lte(actor_base_turn));
  }
  assert_eq!(
    TmctolTaskEffectWeight::maximum_effect_weight(&TaskOf::<Runtime>::StopCycle),
    Some(Weight::zero())
  );
  let transfer = TaskOf::<Runtime>::Transfer {
    to: BOB,
    asset: AssetKind::Native,
    amount: AmountResolution::Fixed(1),
  };
  let transfer_weight = TmctolTaskEffectWeight::maximum_effect_weight(&transfer)
    .expect("Transfer has a measured composite effect envelope");
  assert!(transfer_weight.ref_time() > 0);
  assert!(transfer_weight.proof_size() > 0);
  assert!(transfer_weight.all_lte(actor_base_turn));

  let legs = SplitTransferLegsOf::<Runtime>::try_from(alloc::vec![
    SplitLeg {
      to: BOB,
      share: Perbill::from_percent(50),
    },
    SplitLeg {
      to: CHARLIE,
      share: Perbill::from_percent(50),
    },
  ])
  .expect("two split legs fit");
  let remaining = alloc::vec![
    TaskOf::<Runtime>::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(2),
      legs,
    },
    TaskOf::<Runtime>::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(1),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::LiveQuote,
      slippage_tolerance: Perbill::zero(),
    },
    TaskOf::<Runtime>::AddLiquidity {
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      amount_a: AmountResolution::Fixed(1),
      amount_b: AmountResolution::Fixed(1),
      min_lp_out: 1,
    },
    TaskOf::<Runtime>::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    },
    TaskOf::<Runtime>::Burn {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    TaskOf::<Runtime>::Mint {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    TaskOf::<Runtime>::DonateLiquidity {
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      max_amount_a: AmountResolution::Fixed(1),
      max_ratio_error: Perbill::zero(),
    },
  ];
  for task in remaining {
    let weight = TmctolTaskEffectWeight::maximum_effect_weight(&task)
      .expect("every runtime Task family has a production envelope");
    assert!(weight.ref_time() > 0);
    assert!(weight.proof_size() > 0);
    assert!(weight.all_lte(actor_base_turn));
  }
}

#[test]
fn runtime_step_control_weight_identity_commits_every_staged_production_branch() {
  let step = StepOf::<Runtime> {
    precondition: None,
    task: TaskOf::<Runtime>::StopCycle,
    on_error: StepErrorPolicy::AbortCycle,
  };
  assert!(RuntimeStepControlWeight::production_weight_identity().is_some());
  let simple_context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: 0,
    predicate_evaluation_units: 0,
    opening_snapshot_entries: 0,
    opening_predicate_results: 0,
    funding_snapshot_entries: 0,
  };
  let simple = RuntimeStepControlWeight::maximum_control_weight(simple_context, &step)
    .expect("the production head branch is bounded");
  let simple_actual = RuntimeStepControlWeight::actual_control_weight(
    simple_context,
    &step,
    simple,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("simple completed branch has actual control evidence");
  assert!(simple_actual.all_lte(simple));
  assert!(simple_actual.ref_time() < simple.ref_time());
  let suspended_head_retry = RuntimeStepControlWeight::actual_control_weight(
    simple_context,
    &step,
    simple,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Suspended,
      placement: StepControlPlacement::Wakeup,
    },
  )
  .expect("Suspended-head retry actual evidence exists");
  assert_eq!(
    suspended_head_retry,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_retry(
      0, 0, 0, 0,
    ),
  );
  assert!(suspended_head_retry.all_lte(simple));
  let opening_heavy_retry_context = StepControlWeightContext {
    predicate_evaluation_units: 5,
    opening_predicate_results: 1,
    ..simple_context
  };
  let opening_heavy_retry_maximum =
    RuntimeStepControlWeight::maximum_control_weight(opening_heavy_retry_context, &step)
      .expect("Opening-heavy Suspended retry maximum exists");
  let opening_heavy_retry = RuntimeStepControlWeight::actual_control_weight(
    opening_heavy_retry_context,
    &step,
    opening_heavy_retry_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Suspended,
      placement: StepControlPlacement::Wakeup,
    },
  )
  .expect("Opening-heavy Suspended retry actual evidence exists");
  assert_eq!(
    opening_heavy_retry,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_opening_retry(
      0, 1, 0,
    ),
  );
  assert!(opening_heavy_retry.all_lte(opening_heavy_retry_maximum));
  let opening_heavy_complete = RuntimeStepControlWeight::actual_control_weight(
    opening_heavy_retry_context,
    &step,
    opening_heavy_retry_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("Opening-heavy Suspended completion actual evidence exists");
  assert_eq!(
    opening_heavy_complete,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_opening_complete(
      0, 1, 0,
    ),
  );
  assert!(opening_heavy_complete.all_lte(opening_heavy_retry_maximum));
  let opening_heavy_progress_context = StepControlWeightContext {
    opening_tail_chunks: 1,
    ..opening_heavy_retry_context
  };
  let opening_heavy_progress_maximum =
    RuntimeStepControlWeight::maximum_control_weight(opening_heavy_progress_context, &step)
      .expect("Opening-heavy Suspended progress maximum exists");
  let opening_heavy_progress = RuntimeStepControlWeight::actual_control_weight(
    opening_heavy_progress_context,
    &step,
    opening_heavy_progress_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("Opening-heavy Suspended progress actual evidence exists");
  assert_eq!(
    opening_heavy_progress,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_opening_progress(
      0, 1, 0,
    ),
  );
  assert!(opening_heavy_progress.all_lte(opening_heavy_progress_maximum));
  let suspended_head_complete = RuntimeStepControlWeight::actual_control_weight(
    simple_context,
    &step,
    simple,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("Suspended-head completion actual evidence exists");
  assert_eq!(
    suspended_head_complete,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_complete(
      0, 0, 0, 0,
    ),
  );
  assert!(suspended_head_complete.all_lte(simple));
  let suspended_head_progress_context = StepControlWeightContext {
    opening_tail_chunks: 1,
    ..simple_context
  };
  let suspended_head_progress_maximum =
    RuntimeStepControlWeight::maximum_control_weight(suspended_head_progress_context, &step)
      .expect("Suspended-head progress maximum exists");
  let suspended_head_progress = RuntimeStepControlWeight::actual_control_weight(
    suspended_head_progress_context,
    &step,
    suspended_head_progress_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("Suspended-head progress actual evidence exists");
  assert_eq!(
    suspended_head_progress,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_head_progress(
      0, 0, 0, 0,
    ),
  );
  assert!(suspended_head_progress.all_lte(suspended_head_progress_maximum));
  let two_tail_chunks = RuntimeStepControlWeight::maximum_control_weight(
    StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 2,
      predicate_evaluation_units: 0,
      opening_snapshot_entries: 0,
      opening_predicate_results: 0,
      funding_snapshot_entries: 0,
    },
    &step,
  )
  .expect("authored Opening tail reconstruction is bounded");
  assert!(two_tail_chunks.ref_time() > simple.ref_time());
  assert!(two_tail_chunks.proof_size() > simple.proof_size());
  let authored = RuntimeStepControlWeight::maximum_control_weight(
    StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK),
      predicate_evaluation_units:
        <Runtime as pallet_deos_actors::Config>::MaxPredicatesPerStep::get().saturating_mul(2),
      opening_snapshot_entries:
        <Runtime as pallet_deos_actors::Config>::MaxOpeningSnapshotEntries::get(),
      opening_predicate_results:
        <Runtime as pallet_deos_actors::Config>::MaxOpeningPredicateResults::get(),
      funding_snapshot_entries:
        <Runtime as pallet_deos_actors::Config>::MaxFundingTrackedAssets::get(),
    },
    &step,
  )
  .expect("maximum authored Opening geometry is bounded");
  let maximum_opening_tail_chunks =
    <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
  let opening_progress =
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_progress_max(
      maximum_opening_tail_chunks,
    );
  let conservative_opening =
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_paged_execute_opening_max();
  assert_eq!(
    authored,
    Weight::from_parts(
      conservative_opening
        .ref_time()
        .max(opening_progress.ref_time()),
      conservative_opening
        .proof_size()
        .max(opening_progress.proof_size()),
    ),
  );
  let authored_progress_actual = RuntimeStepControlWeight::actual_control_weight(
    StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: maximum_opening_tail_chunks,
      predicate_evaluation_units:
        <Runtime as pallet_deos_actors::Config>::MaxPredicatesPerStep::get().saturating_mul(2),
      opening_snapshot_entries:
        <Runtime as pallet_deos_actors::Config>::MaxOpeningSnapshotEntries::get(),
      opening_predicate_results:
        <Runtime as pallet_deos_actors::Config>::MaxOpeningPredicateResults::get(),
      funding_snapshot_entries:
        <Runtime as pallet_deos_actors::Config>::MaxFundingTrackedAssets::get(),
    },
    &step,
    authored,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("maximum Opening progress actual evidence exists");
  assert_eq!(authored_progress_actual, opening_progress);
  assert!(authored_progress_actual.all_lte(authored));
  let minimal_opening_context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: maximum_opening_tail_chunks,
    predicate_evaluation_units: 0,
    opening_snapshot_entries: 0,
    opening_predicate_results: 0,
    funding_snapshot_entries: <Runtime as pallet_deos_actors::Config>::MaxFundingTrackedAssets::get(
    ),
  };
  let minimal_opening_maximum =
    RuntimeStepControlWeight::maximum_control_weight(minimal_opening_context, &step)
      .expect("minimal Opening progress maximum exists");
  let minimal_opening_actual = RuntimeStepControlWeight::actual_control_weight(
    minimal_opening_context,
    &step,
    minimal_opening_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("minimal Opening progress actual evidence exists");
  assert_eq!(
    minimal_opening_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_progress_min(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(minimal_opening_actual.all_lte(minimal_opening_maximum));
  let minimal_failed_actual = RuntimeStepControlWeight::actual_control_weight(
    minimal_opening_context,
    &step,
    minimal_opening_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Failed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("minimal Opening failure actual evidence exists");
  assert_eq!(
    minimal_failed_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_failed_min(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(minimal_failed_actual.all_lte(minimal_opening_maximum));
  let minimal_retry_actual = RuntimeStepControlWeight::actual_control_weight(
    minimal_opening_context,
    &step,
    minimal_opening_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Suspended,
      placement: StepControlPlacement::Wakeup,
    },
  )
  .expect("minimal Opening retry actual evidence exists");
  assert_eq!(
    minimal_retry_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_retry_min(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(minimal_retry_actual.all_lte(minimal_opening_maximum));
  let minimal_completion_actual = RuntimeStepControlWeight::actual_control_weight(
    minimal_opening_context,
    &step,
    minimal_opening_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("minimal Opening completion actual evidence exists");
  assert_eq!(
    minimal_completion_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_complete_min(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(minimal_completion_actual.all_lte(minimal_opening_maximum));
  let maximal_completion_context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: maximum_opening_tail_chunks,
    predicate_evaluation_units: <Runtime as pallet_deos_actors::Config>::MaxPredicatesPerStep::get(
    )
    .saturating_mul(2),
    opening_snapshot_entries: <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get()
      .saturating_sub(1)
      .saturating_mul(2),
    opening_predicate_results:
      <Runtime as pallet_deos_actors::Config>::MaxOpeningPredicateResults::get(),
    funding_snapshot_entries: <Runtime as pallet_deos_actors::Config>::MaxFundingTrackedAssets::get(
    ),
  };
  let maximal_completion_maximum =
    RuntimeStepControlWeight::maximum_control_weight(maximal_completion_context, &step)
      .expect("maximum realizable Opening completion is bounded");
  let maximal_failed_actual = RuntimeStepControlWeight::actual_control_weight(
    maximal_completion_context,
    &step,
    maximal_completion_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Failed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("maximum Opening failure actual evidence exists");
  assert_eq!(
    maximal_failed_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_failed_max(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(maximal_failed_actual.all_lte(maximal_completion_maximum));
  let maximal_retry_actual = RuntimeStepControlWeight::actual_control_weight(
    maximal_completion_context,
    &step,
    maximal_completion_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Suspended,
      placement: StepControlPlacement::Wakeup,
    },
  )
  .expect("maximum Opening retry actual evidence exists");
  assert_eq!(
    maximal_retry_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_retry_max(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(maximal_retry_actual.all_lte(maximal_completion_maximum));
  let maximal_completion_actual = RuntimeStepControlWeight::actual_control_weight(
    maximal_completion_context,
    &step,
    maximal_completion_maximum,
    StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("maximum Opening completion actual evidence exists");
  assert_eq!(
    maximal_completion_actual,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_opening_complete_max(
      maximum_opening_tail_chunks,
    ),
  );
  assert!(maximal_completion_actual.all_lte(maximal_completion_maximum));
  assert!(authored.ref_time() > simple.ref_time());
  assert!(authored.proof_size() > simple.proof_size());
  let (control_limit, _) = conservative_actor_resource_limits();
  assert!(
    authored.all_lte(control_limit),
    "maximum control {authored:?} exceeds conservative limit {control_limit:?}"
  );
  assert_eq!(
    RuntimeStepControlWeight::maximum_control_weight(
      StepControlWeightContext {
        cursor: 1,
        steps_in_fragment: 5,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
        opening_snapshot_entries: 0,
        opening_predicate_results: 0,
        funding_snapshot_entries: 0,
      },
      &step,
    ),
    None
  );
  let tail_context = StepControlWeightContext {
    cursor: 1,
    steps_in_fragment: 4,
    opening_tail_chunks: 0,
    predicate_evaluation_units: 0,
    opening_snapshot_entries: 0,
    opening_predicate_results: 0,
    funding_snapshot_entries: 0,
  };
  let tail_maximum = RuntimeStepControlWeight::maximum_control_weight(tail_context, &step)
    .expect("tail control maximum exists");
  let running_complete = RuntimeStepControlWeight::actual_control_weight(
    tail_context,
    &step,
    tail_maximum,
    StepControlExecution {
      phase: StepControlPhase::Running,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("Running completion actual evidence exists");
  assert_eq!(
    running_complete,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_running_complete(
      4, 0,
    ),
  );
  assert!(running_complete.all_lte(tail_maximum));
  let running_progress = RuntimeStepControlWeight::actual_control_weight(
    tail_context,
    &step,
    tail_maximum,
    StepControlExecution {
      phase: StepControlPhase::Running,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("Running progress actual evidence exists");
  assert_eq!(
    running_progress,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_running_progress(
      4, 0,
    ),
  );
  assert!(running_progress.all_lte(tail_maximum));
  let suspended_complete = RuntimeStepControlWeight::actual_control_weight(
    tail_context,
    &step,
    tail_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::None,
    },
  )
  .expect("Suspended completion actual evidence exists");
  assert_eq!(
    suspended_complete,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_tail_complete(
      4, 0,
    ),
  );
  assert!(suspended_complete.all_lte(tail_maximum));
  let suspended_progress = RuntimeStepControlWeight::actual_control_weight(
    tail_context,
    &step,
    tail_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Continued,
      placement: StepControlPlacement::Queue,
    },
  )
  .expect("Suspended progress actual evidence exists");
  assert_eq!(
    suspended_progress,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_tail_progress(
      4, 0,
    ),
  );
  assert!(suspended_progress.all_lte(tail_maximum));
  let suspended = RuntimeStepControlWeight::actual_control_weight(
    tail_context,
    &step,
    tail_maximum,
    StepControlExecution {
      phase: StepControlPhase::Suspended,
      outcome: StepControlOutcome::Suspended,
      placement: StepControlPlacement::Wakeup,
    },
  )
  .expect("Suspended branch actual evidence exists");
  assert_eq!(
    suspended,
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::scheduler_inner_suspended_tail_retry(
      4, 0,
    ),
  );
  assert!(suspended.all_lte(tail_maximum));
  assert!(RuntimeAdmissionCertificateAuthority::current().is_some());
}

fn system_transfer_steps(target_actor_id: ActorId) -> pallet_deos_actors::ContractSteps<Runtime> {
  alloc::vec![StepOf::<Runtime> {
    precondition: None,
    task: Task::Transfer {
      to: Actors::sovereign_account_id_system(target_actor_id),
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::AbortCycle,
  }]
  .try_into()
  .expect("one System transfer step fits")
}

#[test]
fn host_system_activation_manifest_is_ranked_and_rejects_undeclared_cycle_edges_before_commit() {
  seeded_test_ext().execute_with(|| {
    use crate::configs::actor_config::{
      DeosSystemActorContractValidator, SystemActivationEdge, SystemActivationEffect,
    };
    use primitives::ecosystem::actor_ids;

    let topology = DeosSystemActorContractValidator::manifest().expect("DAG manifest");
    let projection = DeosSystemActorContractValidator::projection().expect("derived topology");
    assert!(
      projection
        .edges
        .iter()
        .all(|edge| topology.edges.contains(edge))
    );
    assert!(projection.nodes.iter().all(|node| {
      topology
        .nodes
        .iter()
        .any(|manifest_node| manifest_node.actor_id == node.actor_id)
    }));
    assert!(topology.edges.contains(&SystemActivationEdge {
      source: actor_ids::FEE_SINK_ACTORS_ID,
      target: actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
      effect: SystemActivationEffect::CertifiedActorTransfer,
    }));
    let fee_sink_rank = topology
      .nodes
      .iter()
      .find(|node| node.actor_id == actor_ids::FEE_SINK_ACTORS_ID)
      .expect("Fee Sink node")
      .rank;
    let liquidity_rank = topology
      .nodes
      .iter()
      .find(|node| node.actor_id == actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID)
      .expect("Native staking liquidity node")
      .rank;
    assert!(fee_sink_rank < liquidity_rank);

    System::set_block_number(1);
    let splitter_before =
      Actors::actor_contract(actor_ids::BLDR_SPLITTER_ACTORS_ID).expect("Splitter contract");
    assert_noop!(
      update_actor_contract_partial(
        RuntimeOrigin::root(),
        actor_ids::BLDR_SPLITTER_ACTORS_ID,
        (
          system_transfer_steps(actor_ids::BURN_ACTOR_ID),
          CompletionPolicy::Persistent
        ),
      ),
      Error::<Runtime>::SystemActorTopologyInvalid
    );
    assert_eq!(
      Actors::actor_contract(actor_ids::BLDR_SPLITTER_ACTORS_ID),
      Some(splitter_before),
      "cycle rejection must precede contract mutation"
    );
  });
}

#[test]
#[ignore = "fixed T+1 runtime length-32 stress profile; run by exact test name"]
fn next_block_runtime_mixed_length_herd_includes_maximum_tail() {
  seeded_test_ext().execute_with(|| {
    let feed = crate::configs::oracle_config::deos_router_pool_feed(
      AssetKind::Native,
      AssetKind::Local(8_043),
    );
    let lengths = [0_u32, 1, 4, 8, 32];
    let actors = (0..10)
      .map(|index| {
        let steps = BoundedVec::try_from(
          (0..lengths[index % lengths.len()])
            .map(|_| {
              make_step(Task::Mint {
                asset: AssetKind::Native,
                amount: AmountResolution::Fixed(1),
              })
            })
            .collect::<Vec<_>>(),
        )
        .expect("runtime W3 contract fits MaxContractSteps");
        create_system(ALICE, observation_schedule(feed), None, steps)
      })
      .collect::<Vec<_>>();

    System::set_block_number(1);
    Actors::on_initialize(1);
    assert_ok!(Actors::note_observation_transition_with_provenance(
      feed,
      pallet_deos_actors::ObservationTransition {
        revision: 1,
        previous: None,
        current: 10,
      },
      pallet_deos_actors::TriggerCauseProvenance::ExternalPhase,
    ));
    run_idle(Weight::MAX);

    let started_actors = || {
      System::events()
        .into_iter()
        .filter_map(|record| match record.event {
          RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
          _ => None,
        })
        .collect::<Vec<_>>()
    };
    let source_started = started_actors();
    assert!(source_started.is_empty());

    let mut started = source_started;
    let mut block = 1_u32;
    while started.len() < actors.len() {
      block += 1;
      assert!(block <= 12, "runtime W3 herd failed to drain");
      System::set_block_number(block);
      Actors::on_initialize(block);
      run_idle(Weight::MAX);
      started = started_actors();
      assert_eq!(started, actors[..started.len()]);
    }
    assert_eq!(started, actors);
  });
}

fn run_next_block_runtime_herd(requested_population: Option<usize>) {
  seeded_test_ext().execute_with(|| {
    let feed = crate::configs::oracle_config::deos_router_pool_feed(
      AssetKind::Native,
      AssetKind::Local(8_044),
    );
    let active_remaining = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get()
      .saturating_sub(Actors::active_actor_count());
    let identity_remaining = <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get()
      .saturating_sub(Actors::actor_identity_count());
    let sovereign_remaining = <Runtime as pallet_deos_actors::Config>::MaxSystemSovereigns::get()
      .saturating_sub(Actors::system_sovereign_count());
    let maximum_population = active_remaining
      .min(identity_remaining)
      .min(sovereign_remaining) as usize;
    if requested_population.is_none() {
      assert_eq!(maximum_population, 9_985);
    }
    let population = requested_population.unwrap_or(maximum_population);
    assert!(population <= maximum_population);
    let lengths = [0_u32, 1, 4, 8, 32];
    let actors = (0..population)
      .map(|index| {
        let steps = BoundedVec::try_from(
          (0..lengths[index % lengths.len()])
            .map(|_| {
              make_step(Task::Mint {
                asset: AssetKind::Native,
                amount: AmountResolution::Fixed(1),
              })
            })
            .collect::<Vec<_>>(),
        )
        .expect("runtime W3 contract fits MaxContractSteps");
        create_system(ALICE, observation_schedule(feed), None, steps)
      })
      .collect::<Vec<_>>();
    let actor_set = actors
      .iter()
      .copied()
      .collect::<alloc::collections::BTreeSet<_>>();

    System::set_block_number(1);
    Actors::on_initialize(1);
    assert_ok!(Actors::note_observation_transition_with_provenance(
      feed,
      pallet_deos_actors::ObservationTransition {
        revision: 1,
        previous: None,
        current: 10,
      },
      pallet_deos_actors::TriggerCauseProvenance::ExternalPhase,
    ));

    let mut started = Vec::new();
    let mut block = 1_u32;
    while started.len() < actors.len() {
      System::reset_events();
      run_idle(Weight::MAX);
      started.extend(
        System::events()
          .into_iter()
          .filter_map(|record| match record.event {
            RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. })
              if actor_set.contains(&actor_id) =>
            {
              Some(actor_id)
            }
            _ => None,
          }),
      );
      assert_eq!(started, actors[..started.len()]);
      if started.len() == actors.len() {
        break;
      }
      block += 1;
      assert!(
        block <= population as u32 + 2,
        "runtime W3 herd failed to drain"
      );
      System::set_block_number(block);
      Actors::on_initialize(block);
    }
    assert_eq!(started, actors);
  });
}

#[test]
#[ignore = "fixed T+1 1,000-Actor runtime stress profile; run by exact test name"]
fn next_block_thousand_actor_runtime_herd_preserves_fifo_order() {
  run_next_block_runtime_herd(Some(1_000));
}

#[test]
#[ignore = "fixed T+1 maximum-admissible runtime stress profile; run by exact test name"]
fn next_block_maximum_admissible_runtime_herd_preserves_fifo_order() {
  run_next_block_runtime_herd(None);
}

#[test]
fn actor_produced_address_event_waits_one_additional_block() {
  seeded_test_ext().execute_with(|| {
    let first_account = Actors::sovereign_account_id(&ALICE, 0);
    let second_account = Actors::sovereign_account_id(&ALICE, 1);
    let first = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(second_account.clone(), AssetKind::Native, 10),
    );
    let second = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(first_account.clone(), AssetKind::Native, 10),
    );
    fund_native(first, 1_000_000_000_000);
    fund_native(second, 1_000_000_000_000);

    System::set_block_number(1);
    Actors::on_initialize(1);
    ensure_actor_prepass_context();
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &first_account,
      AssetKind::Native,
      100,
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(first)
        .expect("first Actor")
        .identity
        .cycle_nonce,
      0
    );
    assert_eq!(
      Actors::active_actor_state(second)
        .expect("second Actor")
        .identity
        .cycle_nonce,
      0
    );
    assert!(Actors::pending_signal(first));

    System::set_block_number(2);
    Actors::on_initialize(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(first)
        .expect("first Actor")
        .identity
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_state(second)
        .expect("second Actor")
        .identity
        .cycle_nonce,
      0
    );
    assert!(Actors::pending_signal(second));

    System::set_block_number(3);
    Actors::on_initialize(3);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(second)
        .expect("second Actor")
        .identity
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn paid_user_two_actor_cycle_is_fifo_bounded_and_cannot_recurse_in_one_block() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let first_account = Actors::sovereign_account_id(&ALICE, 0);
    let second_account = Actors::sovereign_account_id(&ALICE, 1);
    let first = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(second_account.clone(), AssetKind::Native, 10),
    );
    let second = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(first_account.clone(), AssetKind::Native, 10),
    );
    fund_native(first, 1_000_000_000_000);
    fund_native(second, 1_000_000_000_000);
    let fee_sink =
      Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
    let fees_before = Balances::free_balance(&fee_sink);

    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &first_account,
      AssetKind::Native,
      100,
    ));
    // A repeated detection before service owns no second ticket.
    let first_ticket = Actors::actor_hot(first)
      .expect("first Actor hot state")
      .queue_ticket;
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &first_account,
      AssetKind::Native,
      100,
    ));
    assert_eq!(
      Actors::actor_hot(first).expect("first Actor").queue_ticket,
      first_ticket
    );

    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(first)
        .expect("first Actor")
        .identity
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_state(second)
        .expect("second Actor")
        .identity
        .cycle_nonce,
      0
    );
    assert!(Actors::pending_signal(second));
    assert_eq!(Actors::combined_queue_occupancy(), 1);

    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(second)
        .expect("second Actor")
        .identity
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_state(first)
        .expect("first Actor")
        .identity
        .cycle_nonce,
      1
    );
    assert!(Actors::pending_signal(first));
    assert_eq!(Actors::combined_queue_occupancy(), 1);
    assert!(Balances::free_balance(&fee_sink) > fees_before);

    for block in 3..=8 {
      System::set_block_number(block);
      run_idle(Weight::MAX);
      assert!(Actors::combined_queue_occupancy() <= 1);
    }
    let first_nonce = Actors::active_actor_state(first)
      .expect("first Actor solvent")
      .identity
      .cycle_nonce;
    let second_nonce = Actors::active_actor_state(second)
      .expect("second Actor solvent")
      .identity
      .cycle_nonce;
    assert_eq!(first_nonce, 4);
    assert_eq!(second_nonce, 4);
  });
}

#[test]
fn externally_closed_user_self_cycle_remains_paid_and_economically_apoptotic() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let account = Actors::sovereign_account_id(&ALICE, 0);
    let reserved_before =
      <Balances as ReservableCurrency<crate::AccountId>>::reserved_balance(&ALICE);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let fee_sink =
      Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
    let fees_before = Balances::free_balance(&fee_sink);
    for block in 1..=3 {
      System::set_block_number(block);
      assert_ok!(TmctolAssetOps::transfer(
        &ALICE,
        &account,
        AssetKind::Native,
        1_000_000_000,
      ));
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("funded Actor")
          .identity
          .cycle_nonce,
        u64::from(block),
      );
    }
    assert!(Balances::free_balance(&fee_sink) > fees_before);

    let balance = Balances::free_balance(&account);
    let protected_floor = crate::configs::actor_config::ActorMinUserBalance::get();
    deplete_user_sovereign(actor_id, balance.saturating_sub(protected_floor));
    fund_native(actor_id, address_event_trigger_fee());
    System::set_block_number(4);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      1,
      &ALICE,
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_state(actor_id).is_none());
    assert_eq!(
      <Balances as ReservableCurrency<crate::AccountId>>::reserved_balance(&ALICE),
      reserved_before,
      "process cleanup never mutates owner reserve custody"
    );
    assert!(System::events().iter().any(|record| matches!(
      &record.event,
      RuntimeEvent::Actors(Event::ActorClosed {
        actor_id: closed,
        reason: CloseReason::CycleAdmissionInsufficient,
      }) if *closed == actor_id
    )));
  });
}

#[test]
fn paid_user_long_cycle_preserves_fifo_under_queue_pressure_and_closes_insolvent_member() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    const ACTOR_COUNT: u8 = 8;
    let accounts = (0..ACTOR_COUNT)
      .map(|slot| Actors::sovereign_account_id(&ALICE, slot))
      .collect::<alloc::vec::Vec<_>>();
    let mut actors = alloc::vec::Vec::new();
    for slot in 0..ACTOR_COUNT {
      let next = accounts[(usize::from(slot) + 1) % usize::from(ACTOR_COUNT)].clone();
      let actor_id = create_user(
        ALICE,
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        None,
        transfer_contract_steps(next, AssetKind::Native, 10),
      );
      assert_eq!(actor_account(actor_id), accounts[usize::from(slot)]);
      fund_native(actor_id, 2_000_000_000_000);
      actors.push(actor_id);
    }
    let fee_sink =
      Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
    let fees_before = Balances::free_balance(&fee_sink);
    for account in &accounts {
      assert_ok!(TmctolAssetOps::transfer(
        &CHARLIE,
        account,
        AssetKind::Native,
        100,
      ));
    }
    assert_eq!(Actors::combined_queue_occupancy(), u64::from(ACTOR_COUNT));

    run_idle(Weight::MAX);
    assert_eq!(Actors::combined_queue_occupancy(), 1);
    for actor_id in &actors {
      assert_eq!(
        Actors::active_actor_state(*actor_id)
          .expect("solvent ring member")
          .identity
          .cycle_nonce,
        1,
        "the initial queue-pressure cohort executes once in ticket order"
      );
    }
    for block in 2..=9 {
      System::set_block_number(block);
      run_idle(Weight::MAX);
      assert_eq!(Actors::combined_queue_occupancy(), 1);
    }
    assert!(actors.iter().all(|actor_id| {
      Actors::active_actor_state(*actor_id)
        .expect("solvent ring member")
        .identity
        .cycle_nonce
        == 2
    }));
    assert!(Balances::free_balance(&fee_sink) > fees_before);

    let insolvent = actors[3];
    let balance = Balances::free_balance(actor_account(insolvent));
    let protected_floor = crate::configs::actor_config::ActorMinUserBalance::get();
    deplete_user_sovereign(insolvent, balance.saturating_sub(protected_floor));
    fund_native(insolvent, address_event_trigger_fee());
    System::set_block_number(10);
    assert_ok!(Actors::notify_address_event(
      insolvent,
      AssetKind::Native,
      1,
      &ALICE,
    ));
    for block in 10..=13 {
      System::set_block_number(block);
      run_idle(Weight::MAX);
    }
    assert!(Actors::active_actor_state(insolvent).is_none());
    assert!(System::events().iter().any(|record| matches!(
      &record.event,
      RuntimeEvent::Actors(Event::ActorClosed {
        actor_id,
        reason: CloseReason::CycleAdmissionInsufficient,
      }) if *actor_id == insolvent
    )));
    assert!(
      actors
        .iter()
        .filter(|actor_id| **actor_id != insolvent)
        .all(|actor_id| Actors::active_actor_state(*actor_id).is_some())
    );
  });
}

#[derive(Clone)]
struct RuntimeSchedule {
  trigger: pallet_deos_actors::TriggerOf<Runtime>,
  cooldown_blocks: u32,
}
type Schedule = RuntimeSchedule;
type RuntimeSourceFilter = SourceFilterOf<Runtime>;

#[test]
fn canonical_actors_seed_derives_documented_accounts() {
  let pallet_account: AccountId =
    crate::configs::actor_config::ActorsPalletId::get().into_account_truncating();
  assert_eq!(
    pallet_account,
    AccountId::from_ss58check("5EYCAe5fiQWMqjyVakD96Nwxv8toW2XYiWaTHmnmop8X9u5J").unwrap()
  );
  let expected = [
    "5HG3S6PLHrykv65Vw8j19zRaEx2Bmb37iywfo2qK3cHosGKX",
    "5Eiik51gjANLwbjZUXnVJv8pPpoTTVVic2x5sNwy8NaoVaJ9",
    "5EL8uyEoZA3JQkhCC3ackopXhdujtKjHHRYVSM1BVrf5x6LW",
    "5DHChJzyAY9pz54d6PXLmScG5vhdiarfNY2VjhkP4pG8vqSs",
    "5F6w8Jd8mHTPphhHgBdUJdkTaT2hQ8mKYojDhzCre5TJqGPg",
    "5CMBGiT8bLjfecCBLf7jSeWXoHKwEXtF7epoFHaLSTmxPhyp",
    "5Epu2U8sJbpBH1AQhc2KW6yuPA62Hst9r3zSdEHx4vS386JW",
    "5CvGRScqAYFFZRymun1fNJogwgUZCigd2ncmxCGvpquWy4nM",
    "5FZaRybmQEh2eHXM95zB2tyty3vxBZPyrCYTekHu5YxuCKj8",
    "5CeoQfeA6zkG7yToYZm3L8g5gjR5aMikm4b1gVLK69CgYzsC",
    "5H3KvwhcEmU5QZNcXWjwwmtduXdrKTrR5WYZqjrJm23KK14u",
    "5D7ZRz4hMphgVdq9UYBA9Gtk1q2cBjKTgoDCqpBETQi6Ziq4",
    "5EoWnoVuB925BHs9UwHUfLkcm5rSbmqzrHgFZRzY5nA4M5B6",
    "5CE6WsJ12vyyjAPMuvaqf2cdSQMVzAAxVjZDvXZK99VswFGe",
    "5CX93X5agA9cbvbv4JKpXmR8RF9ywdLbyg6WR9qY15evri5L",
  ];
  for (actor_id, expected) in expected.into_iter().enumerate() {
    assert_eq!(
      Actors::sovereign_account_id_system(actor_id as u64),
      AccountId::from_ss58check(expected).unwrap()
    );
  }
}
type RuntimeAssetFilter = AssetFilterOf<Runtime>;
type RuntimeTask = TaskOf<Runtime>;
type RuntimeStep = StepOf<Runtime>;
type RuntimeContractSteps = pallet_deos_actors::ContractSteps<Runtime>;

#[test]
fn runtime_oracle_change_hook_coalesces_into_actor_dirty_feed_state() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(7));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    create_system(
      ALICE,
      observation_schedule(feed),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("one step fits"),
    );

    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    let first = Actors::dirty_observation_feeds(feed).expect("Actors hook marks the feed dirty");
    assert_eq!(first.latest_revision, 1);
    assert_eq!(first.fanout_revision, 0);
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    assert_eq!(Actors::dirty_observation_feeds(feed), Some(first));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      2_000_000_000_000,
    ));
    let latest = Actors::dirty_observation_feeds(feed).expect("dirty feed remains coalesced");
    assert_eq!(latest.previous_dirty_feed, first.previous_dirty_feed);
    assert_eq!(latest.next_dirty_feed, first.next_dirty_feed);
    assert_eq!(latest.latest_revision, 2);
    assert_eq!(Actors::dirty_observation_feed_count(), 1);
  });
}

#[test]
fn oracle_publication_rolls_back_when_actor_change_hook_rejects() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(8));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    create_system(
      ALICE,
      observation_schedule(feed),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("one step fits"),
    );
    pallet_deos_actors::DirtyObservationListState::<Runtime>::mutate(|list| {
      list.count = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    });
    let actor_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_noop!(
      Oracle::publish(
        RuntimeOrigin::signed(producer.clone()),
        feed,
        1_000_000_000_000
      ),
      Error::<Runtime>::DirtyObservationCapacityExceeded
    );
    assert!(Oracle::observations(feed).is_none());
    assert!(Actors::dirty_observation_feeds(feed).is_none());
    assert_eq!(Actors::dirty_observation_list(), actor_before);
    assert_eq!(System::events(), events_before);

    pallet_deos_actors::DirtyObservationListState::<Runtime>::kill();
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      1_000_000_000_000,
    ));
    assert_eq!(
      Oracle::observations(feed).expect("retry commits").revision,
      1
    );
    assert_eq!(
      Actors::dirty_observation_feeds(feed)
        .expect("retry reaches Actors")
        .latest_revision,
      1
    );
  });
}

#[test]
fn underfunded_crossing_fire_advances_without_readiness_or_peer_cursor_loss() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(19));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    let hold_before = actors_owner_hold(&ALICE);
    let crossing = RuntimeSchedule {
      trigger: Trigger::observation_crossing(
        feed,
        CrossingDirection::Rising,
        1_500_000_000_000,
        800_000_000_000,
      ),
      cooldown_blocks: 0,
    };
    let insolvent = create_user(
      ALICE,
      crossing.clone(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let peer = create_user(
      ALICE,
      crossing,
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let state_hold_after_creation = hold_before
      .saturating_add(actor_state_hold_total(insolvent))
      .saturating_add(actor_state_hold_total(peer));
    assert_eq!(actors_owner_hold(&ALICE), state_hold_after_creation);
    let balance = Balances::free_balance(actor_account(insolvent));
    let protected_floor = crate::configs::actor_config::ActorMinUserBalance::get();
    deplete_user_sovereign(insolvent, balance.saturating_sub(protected_floor));
    fund_native(peer, observation_crossing_trigger_fee());

    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      100_000_000_000_000,
    ));
    for block in System::block_number()..=System::block_number().saturating_add(2) {
      System::set_block_number(block);
      run_idle(Weight::MAX);
    }

    let underfunded = Actors::active_actor_state(insolvent).expect("underfunded process remains");
    assert!(!underfunded.hot.pending_signal);
    assert!(underfunded.hot.queue_ticket.is_none());
    assert!(Actors::active_actor_state(peer).is_some());
    assert_eq!(actors_owner_hold(&ALICE), state_hold_after_creation);
    assert_eq!(Actors::crossing_feed_membership_count(feed), 2);
    assert!(Actors::crossing_membership(peer).is_some());
    assert!(Actors::crossing_membership(insolvent).is_some());
    assert!(Actors::crossing_worker_fault().is_none());
    assert!(!System::events().iter().any(|record| matches!(
      &record.event,
      RuntimeEvent::Actors(Event::ActorClosed { actor_id, .. }) if *actor_id == insolvent
    )));
    assert!(!System::events().iter().any(|record| matches!(
      &record.event,
      RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
        actor_id,
        trigger_family: TriggerFamily::ObservationCrossing,
        ..
      }) if *actor_id == insolvent
    )));
  });
}

#[test]
fn oracle_publication_rolls_back_when_crossing_transition_queue_is_full() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(9));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    create_system(
      ALICE,
      RuntimeSchedule {
        trigger: Trigger::observation_crossing(
          feed,
          CrossingDirection::Rising,
          1_500_000_000_000,
          800_000_000_000,
        ),
        cooldown_blocks: 0,
      },
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("one step fits"),
    );
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      2_000_000_000_000,
    ));
    let observation = Oracle::observations(feed).expect("second publication commits");
    let maximum =
      <Runtime as pallet_deos_actors::Config>::MaxCrossingTransitionsPerFeed::get() as usize;
    let saturated = pallet_deos_actors::CrossingTransitionQueueOf::<Runtime>::try_from(vec![
      pallet_deos_actors::CrossingTransitionObligation {
        revision: observation.revision,
        previous: observation.value.saturating_sub(1),
        current: observation.value,
        cause_provenance: pallet_deos_actors::TriggerCauseProvenance::Deferred,
        cause_block: 0,
      };
      maximum
    ])
    .expect("runtime Crossing queue bound fits");
    pallet_deos_actors::CrossingTransitionQueues::<Runtime>::insert(feed, saturated);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(producer), feed, 3_000_000_000_000),
      Error::<Runtime>::CrossingTransitionCapacityExceeded
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    assert_eq!(Oracle::observations(feed), Some(observation));
  });
}

#[test]
fn native_flow_anchor_topology_is_unique_and_funded_with_one_ed() {
  super::common::new_test_ext().execute_with(|| {
    let anchors = TmctolGenesisSystemActors::native_flow_anchor_accounts();
    let unique = anchors.iter().collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), anchors.len());
    for (index, account) in anchors.into_iter().enumerate() {
      assert_eq!(
        Balances::free_balance(&account),
        crate::EXISTENTIAL_DEPOSIT,
        "native-flow anchor {index} ({account:?}) must start with one ED"
      );
    }
  });
}

#[test]
fn actor_0_7_storage_schema_is_a_fresh_genesis_baseline() {
  seeded_test_ext().execute_with(|| {
    let baseline = StorageVersion::new(15);
    assert_eq!(Actors::in_code_storage_version(), baseline);
    assert_eq!(Actors::on_chain_storage_version(), baseline);
  });
}

fn ensure_actor_prepass_context() {
  if crate::Timestamp::get() == 0 {
    set_consensus_timestamp(1);
  }
  if !polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::exists() {
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData::default(),
    );
  }
}

fn ensure_current_resource_state() {
  ensure_actor_prepass_context();
  let now = System::block_number();
  if Actors::block_resource_state().is_some_and(|state| state.ensure_block(now).is_err()) {
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::kill();
  }
  if Actors::block_resource_state().is_none() {
    let _ = Actors::on_initialize(now);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
  }
}

fn signed_extrinsic(
  signer: &sr25519::Pair,
  nonce: crate::Nonce,
  call: RuntimeCall,
) -> UncheckedExtrinsic {
  ensure_current_resource_state();
  let tx_ext = TxExtension::new((
    polkadot_sdk::frame_system::AuthorizeCall::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckNonZeroSender::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckSpecVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckTxVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckGenesis::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(nonce),
    polkadot_sdk::frame_system::CheckWeight::<Runtime>::new(),
    (BlockResourceMeterExtension, AddressEventIngressExtension),
    polkadot_sdk::pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  ));
  let payload =
    generic::SignedPayload::new(call.clone(), tx_ext.clone()).expect("signed payload must encode");
  let signature = payload.using_encoded(|encoded| signer.sign(encoded));
  let account = crate::AccountId::from(signer.public());
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(account),
    Signature::Sr25519(signature),
    tx_ext,
  )
}

fn make_step(task: RuntimeTask) -> RuntimeStep {
  StepOf::<Runtime> {
    precondition: None,
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }
}

fn all_preconditions(
  predicates: Vec<pallet_deos_actors::Predicate<AssetKind, u128, u32, primitives::OracleFeedId>>,
) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
  let clause = BoundedVec::try_from(
    predicates
      .into_iter()
      .map(|predicate| pallet_deos_actors::TimedPredicate {
        timing: pallet_deos_actors::ObservationTiming::Current,
        predicate,
      })
      .collect::<Vec<_>>(),
  )
  .expect("runtime predicates fit");
  Some(pallet_deos_actors::Precondition {
    clauses: BoundedVec::try_from(vec![clause]).expect("runtime clause fits"),
  })
}

fn any_preconditions(
  predicates: Vec<pallet_deos_actors::Predicate<AssetKind, u128, u32, primitives::OracleFeedId>>,
) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
  let clauses = predicates
    .into_iter()
    .map(|predicate| {
      BoundedVec::try_from(vec![pallet_deos_actors::TimedPredicate {
        timing: pallet_deos_actors::ObservationTiming::Current,
        predicate,
      }])
      .expect("runtime predicate fits")
    })
    .collect::<Vec<_>>();
  Some(pallet_deos_actors::Precondition {
    clauses: BoundedVec::try_from(clauses).expect("runtime clauses fit"),
  })
}

fn inert_task() -> RuntimeTask {
  Task::StopCycle
}

fn exp_0002_false_steps(step_count: u32) -> RuntimeContractSteps {
  let step = RuntimeStep {
    precondition: all_preconditions(vec![pallet_deos_actors::Predicate::BlockNumberBelow {
      threshold: 0,
    }]),
    task: inert_task(),
    on_error: StepErrorPolicy::AbortCycle,
  };
  BoundedVec::try_from(vec![step; step_count as usize]).expect("profile Contract Steps fit")
}

fn manual_schedule() -> RuntimeSchedule {
  RuntimeSchedule {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
  }
}

fn observation_schedule(feed: primitives::OracleFeedId) -> RuntimeSchedule {
  RuntimeSchedule {
    trigger: Trigger::observation_change(feed),
    cooldown_blocks: 0,
  }
}

fn on_address_event_schedule(
  source_filter: RuntimeSourceFilter,
  asset_filter: RuntimeAssetFilter,
) -> RuntimeSchedule {
  RuntimeSchedule {
    trigger: Trigger::address_event(source_filter, asset_filter),
    cooldown_blocks: 0,
  }
}

fn transfer_contract_steps(
  to: crate::AccountId,
  asset: AssetKind,
  amount: u128,
) -> RuntimeContractSteps {
  BoundedVec::try_from(vec![make_step(Task::Transfer {
    to,
    asset,
    amount: AmountResolution::Fixed(amount),
  })])
  .expect("steps fits")
}

fn user_active_contract(
  schedule: RuntimeSchedule,
  window: Option<ScheduleWindow<u32>>,
  steps: RuntimeContractSteps,
) -> Option<pallet_deos_actors::ActorContractOf<Runtime>> {
  Some(ActorContract {
    trigger: schedule.trigger,
    cooldown_blocks: schedule.cooldown_blocks,
    window,
    steps,
    completion: pallet_deos_actors::CompletionPolicy::Persistent,
    funding: pallet_deos_actors::FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  })
}

fn system_active_contract(
  schedule: RuntimeSchedule,
  window: Option<ScheduleWindow<u32>>,
  steps: RuntimeContractSteps,
) -> Option<pallet_deos_actors::ActorContractOf<Runtime>> {
  Some(ActorContract {
    trigger: schedule.trigger,
    cooldown_blocks: schedule.cooldown_blocks,
    window,
    steps,
    completion: pallet_deos_actors::CompletionPolicy::Persistent,
    funding: pallet_deos_actors::FundingSourcePolicy::RuntimePolicy,
    auto_close_at_cycle_nonce: None,
  })
}

fn create_user(
  who: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: RuntimeContractSteps,
) -> ActorId {
  prefund_active_user_creation(&who, &steps);
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_user_actor(
    RuntimeOrigin::signed(who),
    Mutability::Mutable,
    user_active_contract(schedule, schedule_window, steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn create_system(
  owner: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: RuntimeContractSteps,
) -> ActorId {
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_system_actor(
    RuntimeOrigin::root(),
    owner,
    Mutability::Mutable,
    system_active_contract(schedule, schedule_window, steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn age_fixture_control_clock(actor_id: ActorId) {
  let now = System::block_number();
  if now == 0 {
    System::set_block_number(1);
    return;
  }
  pallet_deos_actors::ActorIdentities::<Runtime>::mutate(actor_id, |maybe| {
    maybe
      .as_mut()
      .expect("fixture actor identity exists")
      .last_control_mutation_block = now.saturating_sub(1);
  });
}

fn actor_funding(actor_id: ActorId) -> pallet_deos_actors::ActorFundingStateOf<Runtime> {
  Actors::actor_funding(actor_id).expect("active actor funding exists")
}

fn actor_account(actor_id: ActorId) -> crate::AccountId {
  Actors::active_actor_state(actor_id)
    .map(|state| state.identity.sovereign_account)
    .expect("Actors must exist")
}

fn fund_native(actor_id: ActorId, amount: u128) {
  let actor_acc = actor_account(actor_id);
  let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&actor_acc, amount);
}

fn address_event_trigger_fee() -> Balance {
  let weight =
    <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::address_event_trigger_occurrence();
  <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&weight)
}

fn observation_crossing_trigger_fee() -> Balance {
  let weight =
    <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::observation_crossing_trigger_occurrence();
  <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&weight)
}

/// User Active prefunding requirement: floor plus the maximum opening-Step fee.
fn user_prefunding_requirement(plan: &RuntimeContractSteps) -> u128 {
  Actors::user_pipeline_machine_capacity_requirement(plan)
    .expect("fixture plan has a checked opening-Step fee")
}

/// Lowest free owner slot for the deterministic prospective User sovereign; mirrors the
/// pallet's `available_owner_slot(None)` lowest-free-slot scan over the public bitmap.
fn lowest_free_owner_slot(owner: &crate::AccountId) -> u8 {
  let bitmap = pallet_deos_actors::OwnerSlotBitmaps::<Runtime>::get(owner);
  let max_slots = <Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get();
  for (byte_index, byte) in bitmap.iter().enumerate() {
    let first_slot = byte_index * 8;
    if first_slot >= max_slots as usize {
      break;
    }
    let remaining = (max_slots as usize).saturating_sub(first_slot);
    let valid_bits = if remaining >= 8 {
      u8::MAX
    } else {
      (1u8 << remaining) - 1
    };
    let free_bits = !*byte & valid_bits;
    if free_bits != 0 {
      return (first_slot + free_bits.trailing_zeros() as usize) as u8;
    }
  }
  panic!("fixture owner has no free User owner slot");
}

/// Pre-funds the deterministic User sovereign so Active creation/activation admits (spec 7.1)
/// without mutating any pallet state.
fn prefund_user_sovereign(owner: &crate::AccountId, slot: u8, plan: &RuntimeContractSteps) {
  let sovereign = Actors::sovereign_account_id(owner, slot);
  let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
    &sovereign,
    user_prefunding_requirement(plan),
  );
}

/// Pre-funds the next automatically allocated User slot for a direct Active creation fixture.
fn prefund_active_user_creation(owner: &crate::AccountId, plan: &RuntimeContractSteps) {
  let slot = lowest_free_owner_slot(owner);
  prefund_user_sovereign(owner, slot, plan);
}

/// Depletes the sovereign fee-native balance after creation, restoring the historical
/// unfunded post-creation fixture state while keeping creation itself admitted.
fn deplete_user_sovereign(actor_id: ActorId, amount: u128) {
  let acc = actor_account(actor_id);
  let (_, remainder) = <Balances as Currency<crate::AccountId>>::slash(&acc, amount);
  assert_eq!(
    remainder, 0,
    "fixture depletion must not overdraw the sovereign"
  );
}

#[test]
fn zero_step_pipeline_quote_uses_generated_machine_and_cleanup_owners() {
  seeded_test_ext().execute_with(|| {
    type ActorWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let steps = RuntimeContractSteps::default();
    let machine = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
      &ActorWeights::scheduler_inner_zero_step_complete(),
    );
    let cleanup = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
      &ActorWeights::close_actor(),
    );
    let expected = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
      .saturating_add(machine)
      .saturating_add(cleanup);
    assert_eq!(
      Actors::user_pipeline_machine_capacity_requirement(&steps)
        .expect("zero-Step Pipeline quote fits"),
      expected
    );
    assert!(machine > 0);
    assert!(cleanup > 0);
  });
}

#[test]
fn deos_runtime_executes_unconditional_and_dnf_with_fixed_successors() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let transfer = |amount| Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(amount),
    };
    let plan = BoundedVec::try_from(vec![
      pallet_deos_actors::Step {
        precondition: None,
        task: transfer(7),
        on_error: StepErrorPolicy::AbortCycle,
      },
      pallet_deos_actors::Step {
        precondition: all_preconditions(vec![pallet_deos_actors::Predicate::BlockNumberAbove {
          threshold: 0,
        }]),
        task: transfer(11),
        on_error: StepErrorPolicy::AbortCycle,
      },
      pallet_deos_actors::Step {
        precondition: any_preconditions(vec![pallet_deos_actors::Predicate::BlockNumberAbove {
          threshold: 0,
        }]),
        task: transfer(13),
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .expect("three-step User plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 10_000_000_000_000);
    let bob_before = Balances::free_balance(BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(31));
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("actor remains active")
        .identity
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn genesis_anchor_buckets_are_custody_only_accounts() {
  seeded_test_ext().execute_with(|| {
    for actor_id in [
      primitives::ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
      primitives::ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID,
    ] {
      let sovereign = Actors::sovereign_account_id_system(actor_id);
      assert!(Actors::active_actor_state(actor_id).is_none());
      assert!(Actors::actor_identities(actor_id).is_none());
      assert!(Actors::sovereign_index(sovereign).is_none());
      let plan = transfer_contract_steps(BOB, AssetKind::Native, 1);
      assert_noop!(
        update_actor_contract_partial!(
          RuntimeOrigin::root(),
          actor_id,
          (plan, CompletionPolicy::Persistent,)
        ),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::pause_actor(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::manual_trigger(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::close_actor(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
    }
  });
}

fn fund_native_via_call(funder: crate::AccountId, actor_id: ActorId, amount: u128) {
  let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
  let provenance = pallet_deos_actors::FundingProvenance::Signed;
  assert_ok!(Actors::preflight_funding_event(
    actor_id,
    AssetKind::Native,
    amount,
    Some(&funder),
    Some(&provenance),
  ));
  assert_ok!(<Balances as Currency<crate::AccountId>>::transfer(
    &funder,
    &instance.identity.sovereign_account,
    amount,
    polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
  ));
  assert_ok!(Actors::notify_address_event(
    actor_id,
    AssetKind::Native,
    amount,
    &funder
  ));
}

fn native_balance(who: &crate::AccountId) -> u128 {
  Balances::free_balance(who)
}

fn actor_state_hold_total(actor_id: ActorId) -> Balance {
  let record = Actors::actor_state_hold(actor_id).expect("User Actor state hold exists");
  record
    .breakdown
    .identity
    .saturating_add(record.breakdown.contract_head)
    .saturating_add(record.breakdown.contract_body)
    .saturating_add(record.breakdown.detector)
    .saturating_add(record.breakdown.funding)
    .saturating_add(record.breakdown.run)
}

fn actors_owner_hold(owner: &AccountId) -> Balance {
  Balances::balance_on_hold(
    &RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState),
    owner,
  )
}

fn account_location(who: crate::AccountId) -> xcm::latest::Location {
  let mut id = [0u8; 32];
  id.copy_from_slice(who.as_ref());
  xcm::latest::Location::new(
    0,
    [xcm::latest::Junction::AccountId32 { network: None, id }],
  )
}

fn native_xcm_asset(amount: u128) -> xcm::latest::Asset {
  xcm::latest::Asset {
    id: xcm::latest::AssetId(xcm::latest::Location::here()),
    fun: xcm::latest::Fungibility::Fungible(amount),
  }
}

#[derive(Clone)]
struct MockCredit(u128);

impl UnsafeConstructorDestructor<u128> for MockCredit {
  fn unsafe_clone(&self) -> Box<dyn ImbalanceAccounting<u128>> {
    Box::new(Self(self.0))
  }

  fn forget_imbalance(&mut self) -> u128 {
    core::mem::take(&mut self.0)
  }
}

impl UnsafeManualAccounting<u128> for MockCredit {
  fn saturating_subsume(&mut self, mut other: Box<dyn ImbalanceAccounting<u128>>) {
    self.0 = self.0.saturating_add(other.amount());
    let _ = other.forget_imbalance();
  }
}

impl ImbalanceAccounting<u128> for MockCredit {
  fn amount(&self) -> u128 {
    self.0
  }

  fn saturating_take(&mut self, amount: u128) -> Box<dyn ImbalanceAccounting<u128>> {
    let taken = self.0.min(amount);
    self.0 -= taken;
    Box::new(Self(taken))
  }
}

fn asset_to_holding(asset: xcm::latest::Asset) -> AssetsInHolding {
  let mut holding = AssetsInHolding::new();
  match asset.fun {
    xcm::latest::Fungibility::Fungible(amount) => {
      holding
        .fungible
        .insert(asset.id, Box::new(MockCredit(amount)));
    }
    xcm::latest::Fungibility::NonFungible(instance) => {
      holding.non_fungible.insert((asset.id, instance));
    }
  }
  holding
}

fn run_next_idle(weight: Weight) {
  let now = System::block_number()
    .checked_add(1)
    .expect("test block advances");
  System::set_block_number(now);
  set_consensus_timestamp(
    u64::from(now).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
  );
  Actors::on_initialize(now);
  run_idle(weight);
}

fn run_idle(weight: Weight) {
  let mut now = System::block_number();
  let block_time =
    u64::from(now).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS);
  if crate::Timestamp::get() < block_time {
    set_consensus_timestamp(block_time);
  }
  ensure_current_resource_state();
  Actors::on_idle(now, weight);
  for _ in 1..<Runtime as pallet_deos_actors::Config>::MaxContractSteps::get().saturating_mul(2) {
    if !pallet_deos_actors::ActorHot::<Runtime>::iter_values()
      .any(|hot| hot.cycle_state == pallet_deos_actors::CycleState::Running)
    {
      break;
    }
    now = now.checked_add(1).expect("test block advances");
    System::set_block_number(now);
    set_consensus_timestamp(
      u64::from(now).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
    );
    Actors::on_initialize(now);
    ensure_current_resource_state();
    Actors::on_idle(now, weight);
  }
}

fn starvation_observation_weight() -> Weight {
  <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_on_idle_base()
}

/// Proof-limited on_idle budget that admits the wakeup cursor, queue scan, loaded-state probe, and
/// the head consume, but not the actor's full cycle admission. Materializes the only spec 8.6.3
/// starvation trigger: a live FIFO head blocked by weight with no admitted attempt.
fn starvation_blocked_budget(actor_id: ActorId) -> Weight {
  let base = starvation_observation_weight();
  let cursor = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_future();
  let scan =
    <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_tombstone_drain(1);
  let state_probe = Actors::scheduler_actor_state_probe_weight_upper();
  let consume = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_preserve_page()
    .max(<<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_delete_page());
  let instance = Actors::active_actor_state(actor_id).expect("actor exists");
  let cycle = Actors::compute_cycle_weight_upper(
    instance.identity.actor_class.actor_type(),
    &instance.contract.steps,
  );
  let full = base
    .saturating_add(cursor)
    .saturating_add(scan)
    .saturating_add(state_probe)
    .saturating_add(consume)
    .saturating_add(cycle);
  Weight::from_parts(u64::MAX, full.proof_size().saturating_sub(1))
}

fn run_idle_until_cycle_nonce(actor_id: ActorId, target_cycle_nonce: u64) {
  for _ in 0..20 {
    run_idle(Weight::MAX);
    if Actors::active_actor_state(actor_id)
      .map(|state| state.identity.cycle_nonce >= target_cycle_nonce)
      .unwrap_or(false)
    {
      return;
    }
  }
  panic!("cycle nonce did not reach target");
}

fn actor_events() -> alloc::vec::Vec<Event<Runtime>> {
  System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(event) => Some(event),
      _ => None,
    })
    .collect()
}

pub fn has_actor_event(predicate: impl Fn(&Event<Runtime>) -> bool) -> bool {
  actor_events().iter().any(predicate)
}

// --- Actors Platform: Lifecycle ---

#[test]
fn manual_trigger_executes_transfer_contract_steps() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 5_000_000_000_000u128;
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            committed_effectful_tasks: 1,
            precondition_skips: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn exp_0002_ceiling_semantics_profile_commits_exactly_one_step_per_block() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let step_count = <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get();
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      exp_0002_false_steps(step_count),
    );
    let expected_tail_chunks = step_count
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK)
      as usize;
    assert_eq!(
      pallet_deos_actors::ActorContractTailChunks::<Runtime>::iter_prefix(actor_id).count(),
      expected_tail_chunks
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("profile actor is active")
        .identity
        .cycle_nonce,
      0
    );

    let mut now = System::block_number();
    for step_index in 0..step_count {
      now = now.checked_add(1).expect("profile block advances");
      System::set_block_number(now);
      set_consensus_timestamp(
        u64::from(now).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
      );
      assert_eq!(Actors::on_initialize(now), Weight::zero());
      ensure_actor_prepass_context();
      assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
      Actors::on_idle(now, Weight::MAX);
      assert!(has_actor_event(|event| matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: index,
          reason: StepSkippedReason::PreconditionFalse,
          ..
        } if *id == actor_id && *index == step_index
      )));
      let state = Actors::active_actor_state(actor_id).expect("profile actor remains active");
      if step_index + 1 < step_count {
        let run = Actors::actor_run_state(actor_id).expect("profile run remains open");
        assert_eq!(run.cursor, step_index + 1);
        assert_eq!(run.last_committed_step_block, Some(now));
        assert_eq!(run.eligible_at, now + 1);
        assert_eq!(state.identity.cycle_nonce, 0);
      } else {
        assert!(Actors::actor_run_state(actor_id).is_none());
        assert_eq!(state.identity.cycle_nonce, 1);
      }
    }
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Completed,
        outcomes,
      } if *id == actor_id
        && outcomes.executed_steps == 0
        && outcomes.precondition_skips == step_count
        && outcomes.committed_effectful_tasks == 0
    )));
  });
}

#[test]
#[ignore = "deterministic EXP-0002 profile output; run explicitly with --nocapture"]
fn profile_exp_0002_state_footprint() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let step_count = <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get();
    let short_actor = create_system(
      ALICE,
      manual_schedule(),
      None,
      exp_0002_false_steps(1),
    );
    let max_actor = create_system(
      BOB,
      manual_schedule(),
      None,
      exp_0002_false_steps(step_count),
    );
    let geometry = |actor_id| {
      let head = pallet_deos_actors::ActorContractHeads::<Runtime>::get(actor_id)
        .expect("profile head exists");
      let certificate = pallet_deos_actors::ActorAdmissionCertificates::<Runtime>::get(actor_id)
        .expect("profile certificate exists");
      let tails = pallet_deos_actors::ActorContractTailChunks::<Runtime>::iter_prefix(actor_id)
        .collect::<Vec<_>>();
      let tail_bytes = tails
        .iter()
        .map(|(_, chunk)| chunk.encode().len())
        .sum::<usize>();
      (
        head.encode().len(),
        certificate.encode().len(),
        tails.len(),
        tail_bytes,
      )
    };
    let short = geometry(short_actor);
    let maximum = geometry(max_actor);
    assert_eq!(short.2, 0);
    assert_eq!(
      maximum.2,
      step_count
        .saturating_sub(1)
        .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK) as usize
    );
    println!(
      "EXP0002_PROFILE steps={} funding={} opening={} opening_predicates={} short_head={} short_certificate={} short_tail_chunks={} short_tail_bytes={} max_head={} max_certificate={} max_tail_chunks={} max_tail_bytes={}",
      step_count,
      <Runtime as pallet_deos_actors::Config>::MaxFundingTrackedAssets::get(),
      <Runtime as pallet_deos_actors::Config>::MaxOpeningSnapshotEntries::get(),
      <Runtime as pallet_deos_actors::Config>::MaxOpeningPredicateResults::get(),
      short.0,
      short.1,
      short.2,
      short.3,
      maximum.0,
      maximum.1,
      maximum.2,
      maximum.3,
    );
  });
}

#[test]
fn productive_run_completion_closes_runtime_actor_after_committed_transfer() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 5_000_000_000_000u128;
    let actor_id = Actors::next_actor_id();
    let schedule = manual_schedule();
    let contract = ActorContract {
      trigger: schedule.trigger,
      cooldown_blocks: schedule.cooldown_blocks,
      window: None,
      steps: transfer_contract_steps(BOB, AssetKind::Native, amount),
      completion: CompletionPolicy::CloseAfterProductiveCycle,
      funding: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    };
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(contract.clone()),
    ));
    let actor = Actors::sovereign_account_id_system(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    let simulation = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready productive contract simulates");
    assert_eq!(
      simulation.status,
      AttemptDisposition::Closed(CloseReason::ProductiveCycleCompleted)
    );
    assert!(Actors::active_actor_state(actor_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&actor), actor_before);

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(amount));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn native_staking_liquidity_actor_activation_requires_initialized_pool() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_noop!(
      TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1),
      DispatchError::Other("StakedAssetUnavailable")
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_noop!(
      TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1),
      DispatchError::Other("NativeStakingAmmUnavailable")
    );
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = AssetKind::Local(0);
    let staked_asset = AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    assert_ok!(TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1));
    let actor = Actors::active_actor_state(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    )
    .expect("Native Staking Liquidity Actor must exist");
    assert!(matches!(
      actor.contract.steps.first().map(|step| &step.task),
      Some(Task::DonateLiquidity { .. })
    ));
  });
}

#[test]
fn pool_creation_owns_an_exact_lp_reverse_index() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_001;
    System::set_block_number(1);
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1));
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    assert_eq!(
      crate::DeosRouter::lp_pair_by_token_id(pool.lp_token),
      Some(pair)
    );
    assert_noop!(
      crate::DeosRouter::register_lp_pair(
        pool.lp_token,
        (
          AssetKind::Native,
          AssetKind::Local(INDEXED_ASSET.saturating_add(1))
        ),
      ),
      pallet_deos_router::Error::<Runtime>::LpTokenPairCollision
    );
  });
}

#[test]
fn remove_liquidity_requires_and_uses_the_exact_lp_reverse_index() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_002;
    System::set_block_number(1);
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    let liquidity = 1_000_000_000_000_000u128;
    assert_ok!(mint_tokens(
      INDEXED_ASSET,
      &ALICE,
      &ALICE,
      liquidity.saturating_mul(2),
    ));
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      pair.0,
      pair.1,
      liquidity,
      liquidity,
      1,
      1,
      &ALICE,
    ));
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    let lp_before_add_bound = Assets::balance(pool.lp_token, &ALICE);
    let root_before_add_failure =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::add_liquidity(
        &ALICE,
        pair.0,
        pair.1,
        liquidity / 10,
        liquidity / 10,
        Balance::MAX,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("MinimumLpOutputNotMet")
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before_add_failure,
      "late LP-output rejection restores ledgers, pool, LP index, events, and issuance"
    );
    assert_eq!(Assets::balance(pool.lp_token, &ALICE), lp_before_add_bound);
    let lp_amount = Assets::balance(pool.lp_token, &ALICE) / 2;
    let root_before_remove_failure =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::remove_liquidity(
        &ALICE,
        AssetKind::Local(pool.lp_token),
        pair.0,
        pair.1,
        lp_amount,
        Balance::MAX,
        Balance::MAX,
      )
      .is_err()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before_remove_failure,
      "late minimum-output rejection restores ledgers, pool, LP index, events, and issuance"
    );
    pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
      pairs.remove(&pool.lp_token);
    });
    assert_noop!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::remove_liquidity(
        &ALICE,
        AssetKind::Local(pool.lp_token),
        pair.0,
        pair.1,
        lp_amount,
        1,
        1,
      ),
      DispatchError::Other("Pool not found for LP token")
    );
    assert_ok!(crate::DeosRouter::register_lp_pair(pool.lp_token, pair));
    let lp_before_bound_failure = Assets::balance(pool.lp_token, &ALICE);
    assert_eq!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::remove_liquidity(
        &ALICE,
        AssetKind::Local(pool.lp_token),
        pair.0,
        pair.1,
        lp_amount,
        Balance::MAX,
        Balance::MAX,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        crate::pallet_asset_conversion::Error::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum
      ))
    );
    assert_eq!(
      Assets::balance(pool.lp_token, &ALICE),
      lp_before_bound_failure
    );
    assert_ok!(<TmctolLiquidityOps as LiquidityOps<
      AccountId,
      AssetKind,
      Balance,
    >>::remove_liquidity(
      &ALICE,
      AssetKind::Local(pool.lp_token),
      pair.0,
      pair.1,
      lp_amount,
      1,
      1,
    ));
  });
}

#[test]
fn executive_canonical_pool_creation_commits_complete_topology() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_003;
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[43u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    let call = RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
      asset_a: pair.0,
      asset_b: pair.1,
    });
    let result = Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call));
    assert!(matches!(result, Ok(Ok(_))), "{result:?}");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    assert_eq!(
      crate::DeosRouter::lp_pair_by_token_id(pool.lp_token),
      Some(pair)
    );
  });
}

#[test]
fn executive_canonical_pool_creation_rolls_back_when_oracle_admission_fails() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    for index in 0..1_000 {
      let feed = crate::configs::oracle_config::deos_router_pool_feed(
        AssetKind::Local(20_000 + index),
        AssetKind::Native,
      );
      assert_ok!(Oracle::register_feed(
        RuntimeOrigin::root(),
        feed,
        producer.clone(),
        feed.meaning(),
        primitives::OracleProvenance::DeosRouterPreExecutionReserves,
        feed.scale,
        pallet_oracle::Aggregation::Ema {
          half_life_blocks: 100,
        },
        pallet_oracle::ZeroPolicy::Reject,
        false,
      ));
    }
    const INDEXED_ASSET: u32 = 901_004;
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    let signer = sr25519::Pair::from_seed(&[44u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    AssetConversionAdapter::ensure_lp_asset_namespace();
    let next_lp_before = polkadot_sdk::pallet_asset_conversion::NextPoolAssetId::<Runtime>::get();
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    let call = RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
      asset_a: pair.0,
      asset_b: pair.1,
    });

    let result = Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call));

    assert!(matches!(result, Ok(Err(_))), "{result:?}");
    assert!(!polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::contains_key(pair));
    assert_eq!(
      polkadot_sdk::pallet_asset_conversion::NextPoolAssetId::<Runtime>::get(),
      next_lp_before
    );
    assert_eq!(pallet_oracle::FeedIds::<Runtime>::decode_len(), Some(1_000));
  });
}

#[test]
fn system_actor_executes_native_staking_lp_donation_task() {
  seeded_test_ext().execute_with(|| {
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;
    System::set_block_number(1);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = AssetKind::Local(0);
    let staked_asset = AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .expect("NTVE/stNTVE pool id must resolve");
    let pool_account =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
        .expect("NTVE/stNTVE pool account must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    let ratio_failure =
      crate::configs::AssetConversionAdapter::donate_balanced_liquidity_classified(
        &BOB,
        base_asset,
        staked_asset,
        40,
        20,
        Perbill::from_percent(1),
      )
      .expect_err("ratio movement must fail before transfer");
    assert_eq!(
      ratio_failure.retry,
      pallet_deos_actors::RetryClass::Temporary
    );
    let lp_supply_before =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    let steps = TmctolGenesisSystemActors::build_native_staking_liquidity_contract_steps(1);
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      0,
      sovereign.clone().into(),
      81,
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let lp_supply_after =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after, lp_supply_before);
    assert_eq!(Assets::balance(0, pool_account.clone()), 440);
    assert_eq!(Assets::balance(staked_asset_id, pool_account), 440);
    assert_eq!(Assets::balance(0, sovereign.clone()), 1);
    assert_eq!(Assets::balance(staked_asset_id, sovereign), 0);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::LiquidityDonated {
          actor_id: id,
          asset_a: AssetKind::Local(0),
          asset_b,
          max_amount_a: 80,
          amount_a: 40,
          amount_b: 40,
          ..
        } if *id == actor_id && *asset_b == AssetKind::Local(staked_asset_id)
      )
    }));
  });
}

#[test]
fn create_user_charges_creation_fee_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let sink_before = native_balance(&fee_sink);
    let alice_before = native_balance(&ALICE);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(
      native_balance(&ALICE),
      alice_before
        .saturating_sub(fee)
        .saturating_sub(actor_state_hold_total(actor_id))
    );
  });
}

#[test]
fn actor_fee_collector_routes_the_full_amount_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let payer = BOB;
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let payer_before = native_balance(&payer);
    let fee_sink_before = native_balance(&fee_sink);
    assert_ok!(TmctolFeeCollector::collect_fee(
      &payer,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));
    assert_eq!(native_balance(&payer), payer_before.saturating_sub(amount));
    assert_eq!(
      native_balance(&fee_sink),
      fee_sink_before.saturating_add(amount)
    );
    // Collection changes ledger custody only; one cadence remains the sole trigger.
    assert!(!Actors::pending_signal(fee_sink_id));
    let hot = Actors::actor_hot(fee_sink_id).expect("Fee Sink hot state");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(hot.trigger_wakeup_pointer.is_some());
    assert_eq!(
      Actors::active_actor_state(fee_sink_id)
        .expect("Fee Sink remains active")
        .identity
        .cycle_nonce,
      0
    );
    // The default-deny RuntimePolicy accumulates no authoritative funding from fees.
    let funding = Actors::actor_funding(fee_sink_id).expect("Fee Sink funding state");
    assert!(funding.funding_accumulated.is_empty());
  });
}

#[test]
fn actor_fee_collector_ignores_malformed_actor_and_scheduler_state() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let payer = BOB;
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let payer_before = native_balance(&payer);
    let sink_before = native_balance(&fee_sink);
    let hot_before = Actors::actor_hot(fee_sink_id).expect("Fee Sink hot state");
    pallet_deos_actors::ActorFunding::<Runtime>::remove(fee_sink_id);
    pallet_deos_actors::QueueTail::<Runtime>::put(1);
    pallet_deos_actors::QueueOccupancy::<Runtime>::put(0);
    let wakeup_len = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    pallet_deos_actors::WakeupCursorLen::<Runtime>::insert(
      pallet_deos_actors::WakeupClock::Block,
      wakeup_len,
    );
    System::reset_events();

    assert_ok!(TmctolFeeCollector::collect_fee(
      &payer,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));

    assert_eq!(native_balance(&payer), payer_before - amount);
    assert_eq!(native_balance(&fee_sink), sink_before + amount);
    assert_eq!(Actors::actor_funding(fee_sink_id), None);
    assert_eq!(Actors::actor_hot(fee_sink_id), Some(hot_before));
    assert_eq!(pallet_deos_actors::QueueTail::<Runtime>::get(), 1);
    assert_eq!(pallet_deos_actors::QueueOccupancy::<Runtime>::get(), 0);
    assert_eq!(
      pallet_deos_actors::WakeupCursorLen::<Runtime>::get(pallet_deos_actors::WakeupClock::Block),
      wakeup_len
    );
    assert!(
      System::events()
        .iter()
        .all(|record| !matches!(record.event, RuntimeEvent::Actors(..)))
    );
  });
}

#[test]
fn actor_fee_collector_rolls_back_completely_on_ledger_failure() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let payer = BOB;
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    System::reset_events();
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert!(
      TmctolFeeCollector::collect_fee(
        &payer,
        &fee_sink,
        AssetKind::Native,
        native_balance(&payer).saturating_add(1),
      )
      .is_err()
    );

    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn fee_sink_processes_ten_percent_and_splits_it_fifty_fifty() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    // Genesis seeds the Fee Sink with an initial balance; add a fresh inflow so the split is
    // observable on top of the seeded anchor.
    let inflow = 2_000_000_000_000_000u128;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&fee_sink, inflow);
    let total = native_balance(&fee_sink);
    let staking_pool = crate::Staking::pool_account_for(0);
    let staking_liquidity_actor = crate::Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let pool_before = native_balance(&staking_pool);
    let liquidity_before = native_balance(&staking_liquidity_actor);

    set_consensus_timestamp(1_000);
    System::set_block_number(2);
    Actors::on_initialize(2);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&staking_pool), pool_before);
    assert_eq!(native_balance(&staking_liquidity_actor), liquidity_before);

    let cadence_tick = Actors::actor_hot(fee_sink_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("Fee Sink cadence is armed from the first consensus timestamp")
      .tick;
    set_consensus_timestamp(
      cadence_tick.saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
    );
    System::set_block_number(3);
    Actors::on_initialize(3);
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);

    // Each cycle processes 10% of the spendable buffer and splits that amount 50/50.
    let pool_delta = native_balance(&staking_pool).saturating_sub(pool_before);
    let liquidity_delta = native_balance(&staking_liquidity_actor).saturating_sub(liquidity_before);
    let distributed = pool_delta.saturating_add(liquidity_delta);
    assert_eq!(
      pool_delta, liquidity_delta,
      "Fee Sink must split its native balance exactly 50/50 between staking ingress and liquidity"
    );
    let expected = primitives::ecosystem::params::FEE_SINK_BUFFER_PCT
      .mul_floor(total.saturating_sub(crate::EXISTENTIAL_DEPOSIT));
    assert_eq!(distributed, expected);
    assert_eq!(
      native_balance(&fee_sink),
      total.saturating_sub(expected),
      "the unprocessed Fee Sink buffer and free-balance anchor remain"
    );
  });
}

#[test]
fn permissionless_sweep_many_does_not_predict_pipeline_affordability() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user_a_prefunded =
      user_prefunding_requirement(&transfer_contract_steps(BOB, AssetKind::Native, 1));
    let user_b_prefunded =
      user_prefunding_requirement(&transfer_contract_steps(ALICE, AssetKind::Native, 1));
    let user_a = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let user_b = create_user(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, AssetKind::Native, 1),
    );
    deplete_user_sovereign(user_a, user_a_prefunded);
    deplete_user_sovereign(user_b, user_b_prefunded);
    let system_alive = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let sweep_ids: BoundedVec<ActorId, <Runtime as pallet_deos_actors::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b, system_alive]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_state(user_a).is_some());
    assert!(Actors::active_actor_state(user_b).is_some());
    assert!(Actors::active_actor_state(system_alive).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 3,
          closed: 0,
          alive: 3,
          missing: 0,
        }
      )
    }));
  });
}

#[test]
fn zombie_spam_attack_cost_dominates_batch_cleanup_cost() {
  seeded_test_ext().execute_with(|| {
    let active_cap = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    let creation_fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let create_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::create_user_actor();
    let create_tx_fee = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&create_weight);
    let attacker_cost_per_actor = creation_fee.saturating_add(create_tx_fee);
    let attacker_total_cost = attacker_cost_per_actor.saturating_mul(active_cap as u128);
    let sweep_batch_size = <Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get().max(1);
    let sweep_calls = active_cap.div_ceil(sweep_batch_size);
    let batch_sweep_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::permissionless_sweep_many(
        sweep_batch_size,
      );
    let batch_sweep_tx_fee =
      <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&batch_sweep_weight);
    let cleanup_total_cost = batch_sweep_tx_fee.saturating_mul(sweep_calls as u128);
    assert!(cleanup_total_cost > 0, "Cleanup fee floor must be non-zero");
    assert!(
      attacker_total_cost >= cleanup_total_cost.saturating_mul(100),
      "Creation-cost floor must dominate bounded cleanup cost by >=100x"
    );
    let cost_ratio_bp = attacker_total_cost.saturating_mul(10_000) / cleanup_total_cost;
    println!(
      "Actors zombie economics: active_cap={}, creation_fee={}, create_tx_fee={}, attacker_total_cost={}, sweep_batch_size={}, sweep_calls={}, batch_sweep_tx_fee={}, cleanup_total_cost={}, cost_ratio={:.2}x",
      active_cap,
      creation_fee,
      create_tx_fee,
      attacker_total_cost,
      sweep_batch_size,
      sweep_calls,
      batch_sweep_tx_fee,
      cleanup_total_cost,
      (cost_ratio_bp as f64) / 10_000.0,
    );
  });
}

#[test]
fn min_user_balance_is_not_below_native_existential_deposit() {
  seeded_test_ext().execute_with(|| {
    let configured_min_user_balance = crate::configs::actor_config::ActorMinUserBalance::get();
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let native_ed = <Balances as Currency<crate::AccountId>>::minimum_balance();
    assert_eq!(
      min_user_balance,
      configured_min_user_balance.max(native_ed),
      "Runtime MinUserBalance guard must clamp below-ED configurations"
    );
    assert!(
      min_user_balance >= native_ed,
      "MinUserBalance must be >= native ExistentialDeposit"
    );
  });
}

#[test]
fn paged_queue_limits_are_independent_runtime_controls() {
  seeded_test_ext().execute_with(|| {
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::QueuePageSize::get(),
      64,
      "64 is the balanced production choice from the 32/64/128 production-Wasm comparison"
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get(),
      10_000
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get(),
      64
    );
    let fanout_limit = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
    assert!(fanout_limit.ref_time() > 0 && fanout_limit.proof_size() > 0);
    assert!(fanout_limit.all_lte(
      <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get()
    ));
    assert!(
      crate::Actors::observation_change_ingress_weight().all_lte(fanout_limit)
    );
    assert!(
      <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_page()
        .all_lte(fanout_limit),
      "one maximum-density fanout page must fit the dedicated two-dimensional runtime budget"
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
      1_000,
      "the execution count is a safety ceiling; WeightMeter remains primary"
    );
    assert_ne!(
      <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get(),
      <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
      "physical inspection and successful execution must remain independent controls"
    );
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 0);
  });
}

#[test]
fn active_trigger_lifecycle_reconciles_only_the_dedicated_state_hold() {
  seeded_test_ext().execute_with(|| {
    let hold_before = actors_owner_hold(&ALICE);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    assert_eq!(
      actors_owner_hold(&ALICE),
      hold_before.saturating_add(actor_state_hold_total(actor_id))
    );

    assert_ok!(update_actor_contract_partial(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      (Trigger::cadenced(1), 0, None),
    ));
    assert_eq!(
      actors_owner_hold(&ALICE),
      hold_before.saturating_add(actor_state_hold_total(actor_id))
    );

    age_fixture_control_clock(actor_id);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let dormant = Actors::actor_state_hold(actor_id).expect("Dormant identity hold remains");
    assert!(dormant.breakdown.identity > 0);
    assert_eq!(dormant.breakdown.contract_head, 0);
    assert_eq!(
      actors_owner_hold(&ALICE),
      hold_before.saturating_add(dormant.breakdown.identity)
    );
  });
}

#[test]
fn crossing_capacity_policy_is_bound_to_measured_minimum_progress_and_explicit_horizons() {
  use pallet_deos_actors::WeightInfo as _;

  type ActorsWeight = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let tail_refill = ActorsWeight::crossing_tail_refill_probe();
  assert!(tail_refill.ref_time() > 0);
  assert!(tail_refill.proof_size() > 0);
  for (one, four) in [
    (
      ActorsWeight::crossing_fire_cohort_preflight(1),
      ActorsWeight::crossing_fire_cohort_preflight(4),
    ),
    (
      ActorsWeight::crossing_coalesced_cohort_preflight(1),
      ActorsWeight::crossing_coalesced_cohort_preflight(4),
    ),
    (
      ActorsWeight::crossing_terminal_cohort_preflight(1),
      ActorsWeight::crossing_terminal_cohort_preflight(4),
    ),
    (
      ActorsWeight::crossing_skip_cohort_preflight(1),
      ActorsWeight::crossing_skip_cohort_preflight(4),
    ),
    (
      ActorsWeight::crossing_rearm_cohort_preflight(1),
      ActorsWeight::crossing_rearm_cohort_preflight(4),
    ),
  ] {
    assert!(four.ref_time() > one.ref_time());
    assert!(four.proof_size() > one.proof_size());
  }

  let limit = <Runtime as pallet_deos_actors::Config>::CrossingWorkerWeightLimit::get();
  let base = crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::crossing_worker_base();
  let pair_unit =
    crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::crossing_work_probe()
      .saturating_add(
        crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::crossing_fire_pair_probe(),
      )
      .saturating_add(
        crate::weights::pallet_deos_actors::SubstrateWeight::<Runtime>::crossing_placed_pair_unit(),
      );
  let available = limit.saturating_sub(base);
  let admitted_pairs = (available.ref_time() / pair_unit.ref_time())
    .min(available.proof_size() / pair_unit.proof_size());
  let candidates_per_block = admitted_pairs.saturating_mul(2).min(u64::from(
    <Runtime as pallet_deos_actors::Config>::MaxCrossingActorsPerBlock::get(),
  ));
  assert_eq!(candidates_per_block, 4);

  let user_cap = crate::configs::actor_config::ActorMaxUserCrossingMembersPerFeed::get();
  let total_cap = crate::configs::actor_config::ActorMaxCrossingMembersPerFeed::get();
  let user_blocks = u64::from(user_cap).div_ceil(candidates_per_block);
  let total_blocks = u64::from(total_cap).div_ceil(candidates_per_block);
  assert_eq!((user_cap, total_cap), (9_000, 10_000));
  assert_eq!((user_blocks, total_blocks), (2_250, 2_500));
  assert_eq!(
    total_blocks * 6,
    15_000,
    "maximum herd is 4h10m at six-second blocks"
  );
  assert_eq!(
    <Runtime as pallet_deos_actors::Config>::MaxQueueLength::get(),
    total_cap
  );
  assert_eq!(
    <Runtime as pallet_deos_actors::Config>::MaxCrossingTransitionsPerFeed::get(),
    64
  );
}

#[test]
fn reactive_delivery_envelopes_follow_production_weights_and_topology_bounds() {
  let base =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_base();
  let branch_probe =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_branch_probe();
  let queue =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_page();
  let wakeup =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_wakeup_page();
  let coalesced =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_coalesced_page();
  let blocked =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_blocked_page();
  let unit = Weight::from_parts(
    queue
      .ref_time()
      .max(wakeup.ref_time())
      .max(coalesced.ref_time())
      .max(blocked.ref_time()),
    queue
      .proof_size()
      .max(wakeup.proof_size())
      .max(coalesced.proof_size())
      .max(blocked.proof_size()),
  );
  let fault =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::record_observation_fanout_worker_fault();
  let limit = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
  let configured_units =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get());
  let available = limit.saturating_sub(base);
  let admitted_unit = branch_probe.saturating_add(unit).saturating_add(fault);
  let units_per_block = configured_units
    .min(available.ref_time() / admitted_unit.ref_time())
    .min(available.proof_size() / admitted_unit.proof_size());

  assert_eq!(base, Weight::from_parts(56_285_000, 1_629));
  assert_eq!(branch_probe, Weight::from_parts(63_340_000, 3_587));
  assert_eq!(unit, Weight::from_parts(150_994_631_000, 304_734));
  assert_eq!(fault, Weight::from_parts(195_464_000, 4_106));
  assert_eq!(limit, Weight::from_parts(400_000_000_000, 1_000_000));
  assert_eq!(
    units_per_block, 2,
    "fee-charged blocked-fallback RefTime is the active ordinary fanout service limit"
  );

  let max_actors = u64::from(<Runtime as pallet_deos_actors::Config>::MaxActiveActors::get());
  let page_size = u64::from(<Runtime as pallet_deos_actors::Config>::QueuePageSize::get());
  let max_sources = 1u64;
  let subscription_pages = max_actors.div_ceil(page_size);
  let dense_single_feed_units = subscription_pages;
  let sparse_high_slot_units = 1u64;
  let compact_four_feed_units = subscription_pages.saturating_mul(max_sources);
  let quiescent_revision_race_units = subscription_pages.saturating_mul(2);

  assert_eq!((max_actors, page_size, max_sources), (10_000, 64, 1));
  assert_eq!(subscription_pages, 157);
  assert_eq!(dense_single_feed_units.div_ceil(units_per_block), 79);
  assert_eq!(sparse_high_slot_units.div_ceil(units_per_block), 1);
  assert_eq!(compact_four_feed_units.div_ceil(units_per_block), 79);
  assert_eq!(quiescent_revision_race_units.div_ceil(units_per_block), 157);
}

#[test]
fn sched_workers_static_envelope_leaves_one_actor_unit_inside_guaranteed_budget() {
  use crate::weights::pallet_deos_actors::SubstrateWeight;
  type W = SubstrateWeight<Runtime>;
  let base = W::scheduler_on_idle_base();
  let coordinator = W::materialization_coordinator_base();
  let mandatory_cleanup = W::scheduler_paged_tombstone_drain(1);
  // Maximum wakeup worker envelope: cursor probe plus one worst-case complete wakeup unit per
  // `MaxWakeupsPerBlock` slot, capped by the dedicated two-dimensional `WakeupWeightLimit`.
  let cursor_probe = W::scheduler_wakeup_cursor_worker_future();
  let wakeup_unit = crate::Actors::wakeup_cursor_drain_unit_weight_upper(true);
  let wakeup_ceiling = <Runtime as pallet_deos_actors::Config>::WakeupWeightLimit::get();
  let wakeup_per_block =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxWakeupsPerBlock::get());
  let wakeup_envelope = cursor_probe
    .saturating_add(wakeup_unit.saturating_mul(wakeup_per_block))
    .min(wakeup_ceiling);
  assert!(wakeup_envelope.ref_time() > 0 && wakeup_envelope.proof_size() > 0);
  assert!(
    wakeup_envelope.all_lte(wakeup_ceiling),
    "worker stays in its own envelope"
  );

  // Maximum fanout worker envelope: base plus one page per configured slot, capped by the limit.
  let fanout_base = W::observation_fanout_base();
  let fanout_page = Actors::observation_fanout_ordinary_weight_upper()
    .saturating_add(W::record_observation_fanout_worker_fault());
  let fanout_ceiling = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
  let fanout_per_block =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get());
  let fanout_envelope = fanout_base
    .saturating_add(fanout_page.saturating_mul(fanout_per_block))
    .min(fanout_ceiling);
  assert!(
    fanout_envelope.all_lte(fanout_ceiling),
    "fanout stays in its own envelope"
  );

  let crossing_base = W::crossing_worker_base();
  let crossing_unit = W::crossing_transition_unit()
    .saturating_add(W::crossing_leaf_unit())
    .saturating_add(W::crossing_page_unit())
    .saturating_add(W::crossing_actor_unit());
  let crossing_ceiling = <Runtime as pallet_deos_actors::Config>::CrossingWorkerWeightLimit::get();
  let crossing_envelope = crossing_base
    .saturating_add(crossing_unit)
    .saturating_add(W::record_crossing_worker_fault());
  assert!(
    crossing_envelope.all_lte(crossing_ceiling),
    "one complete maximum Crossing worker unit must fit its two-dimensional envelope: base={crossing_base:?}, unit={crossing_unit:?}, combined={crossing_envelope:?}, ceiling={crossing_ceiling:?}"
  );

  // One maximum actor unit: admission overhead plus one full cycle admission plus pure cleanup.
  let actor_unit = crate::Actors::scheduler_admission_overhead()
    .saturating_add(crate::Actors::close_dispatch_weight_upper());
  let reserve = <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get();
  let shared_materialization = crate::Actors::materialization_weight_limit();
  assert_eq!(
    shared_materialization,
    wakeup_ceiling
      .saturating_add(crossing_ceiling)
      .saturating_add(fanout_ceiling),
    "one shared materialization envelope must own all three bounded family ceilings"
  );
  let minimum_quanta = crate::Actors::materialization_family_minimum(0)
    .saturating_add(crate::Actors::materialization_family_minimum(1))
    .saturating_add(crate::Actors::materialization_family_minimum(2));
  assert!(
    minimum_quanta.all_lte(shared_materialization),
    "the shared envelope must admit one maximum unit from every family"
  );
  assert!(
    minimum_quanta.ref_time() < shared_materialization.ref_time()
      && minimum_quanta.proof_size() < shared_materialization.proof_size(),
    "the production envelope must retain lendable capacity after all minimum quanta"
  );
  let actor_service = crate::Actors::guaranteed_actor_service_weight()
    .expect("configured housekeeping must fit the runtime reserve");
  assert_eq!(
    actor_service,
    reserve
      .saturating_sub(base)
      .saturating_sub(coordinator)
      .saturating_sub(mandatory_cleanup)
      .saturating_sub(shared_materialization),
    "the Actor floor must be the exact reserve remainder after shared materialization ownership"
  );
  let close_cleanup = crate::Actors::close_cleanup_weight_upper();
  assert!(
    close_cleanup.all_lte(actor_service),
    "bond-aware terminal cleanup across Crossing and pending queues must fit the Actor floor: cleanup={close_cleanup:?}, floor={actor_service:?}"
  );
  assert!(
    actor_unit.all_lte(actor_service),
    "dense materialization must retain one maximum Actor service unit: actor={actor_unit:?}, floor={actor_service:?}"
  );
  let combined = base
    .saturating_add(coordinator)
    .saturating_add(mandatory_cleanup)
    .saturating_add(wakeup_envelope)
    .saturating_add(crossing_envelope)
    .saturating_add(fanout_envelope)
    .saturating_add(actor_unit);
  assert!(
    combined.all_lte(reserve),
    "fixed base + coordinator + cleanup + max wakeup worker + one Crossing unit + max fanout worker + one max actor unit must fit ActorOnIdleReserve: base={base:?}, coordinator={coordinator:?}, cleanup={mandatory_cleanup:?}, wakeup={wakeup_envelope:?}, crossing={crossing_envelope:?}, fanout={fanout_envelope:?}, actor={actor_unit:?}, combined={combined:?}, reserve={reserve:?}"
  );
  println!(
    "SCHED-WORKERS: base={base:?}, coordinator={coordinator:?}, cleanup={mandatory_cleanup:?}, wakeup={wakeup_envelope:?}, crossing={crossing_envelope:?}, fanout={fanout_envelope:?}, actor={actor_unit:?}, floor={actor_service:?}, combined={combined:?}, reserve={reserve:?}"
  );
}

#[test]
fn queue_length_covers_active_actor_capacity() {
  seeded_test_ext().execute_with(|| {
    let queue_cap = <Runtime as pallet_deos_actors::Config>::MaxQueueLength::get();
    let active_cap = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    assert!(
      queue_cap >= active_cap,
      "MaxQueueLength must be >= MaxActiveActors to avoid scheduler actor loss under full activation"
    );
  });
}

#[test]
fn close_actor_emits_owner_initiated_reason() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let fee_sink_before = native_balance(&fee_sink);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_state(actor_id).is_none());
    assert_eq!(native_balance(&fee_sink), fee_sink_before);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::OwnerInitiated,
        } if *id == actor_id
      )
    }));
  });
}

// --- Actors Platform: Amount Resolution ---

#[test]
fn percentage_of_last_funding_keeps_system_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    fund_native_via_call(ALICE, actor_id, 10_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    System::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    System::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 3);
    let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
    assert_eq!(
      instance.hot.lifecycle,
      pallet_deos_actors::ActiveLifecycle::Active
    );
    fund_native_via_call(CHARLIE, actor_id, 8_000_000_000_000);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&8_000_000_000_000)
    );
  });
}

#[test]
fn cycle_summary_reports_funding_unavailable_skip() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 0,
            committed_effectful_tasks: 0,
            precondition_skips: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 1,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn percentage_of_last_funding_keeps_user_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    })])
    .expect("steps fits");
    let prefunded = user_prefunding_requirement(&steps);
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    deplete_user_sovereign(actor_id, prefunded);
    fund_native_via_call(ALICE, actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_state(actor_id).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn swap_exact_in_zero_tolerance_matches_caller_aware_router_quote() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let amount_in = crate::EXISTENTIAL_DEPOSIT.saturating_mul(10);
    let quote = crate::DeosRouter::quote_exact_input(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount_in,
    )
    .expect("caller-aware route is quotable");
    let amount_out = crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount_in,
      Perbill::zero(),
    )
    .expect("zero-tolerance exact-input swap succeeds at its executable quote");
    assert_eq!(amount_out.recipient_amount_out, quote.amount_out);
  });
}

#[test]
fn exact_out_nonzero_tolerance_requires_capacity_for_adjusted_bound() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let required_in = crate::DeosRouter::quote_exact_out(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    let balance_before = native_balance(&ALICE);
    assert_eq!(
      crate::configs::actor_config::TmctolDexOps::swap_exact_out(
        ExecutionContext::new(&ALICE, ActorType::User),
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        target_out,
        required_in,
        Perbill::from_percent(1),
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("ExactOutInputCapacityExceeded",)
      ))
    );
    assert_eq!(native_balance(&ALICE), balance_before);
  });
}

#[test]
fn exact_out_execution_is_bounded_by_the_tolerance_cap_not_the_preservable_balance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let required_in = crate::DeosRouter::quote_exact_out(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    // The tolerance-bound cap is required_in + ceil(1% * required_in). The supplied
    // preservable cap is larger than that, so the execution must be bounded by the
    // tolerance cap, not the preservable balance.
    let tolerance_cap = required_in + (required_in * 10_000_000 / 1_000_000_000) + 1;
    let preservable = tolerance_cap.saturating_mul(2);
    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_out(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
      preservable,
      Perbill::from_percent(1),
    ));
  });
}

#[test]
fn user_exact_out_zero_tolerance_preserves_floor_and_later_step_fees() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![
      make_step(Task::SwapOut {
        asset_out: AssetKind::Local(ASSET_A),
        amount_out: AmountResolution::Fixed(target_out),
        asset_in: AssetKind::Native,
        input_limit: InputLimit::Absolute(100_000_000_000_000),
        slippage_tolerance: Perbill::zero(),
      }),
      make_step(Task::Stake {
        asset: AssetKind::Local(999),
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      }),
    ])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let required_in = crate::DeosRouter::quote_exact_out(
      sovereign.clone(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
    let fee_reserve = Actors::attempt_fee_envelope(
      instance.identity.actor_class.actor_type(),
      &instance.contract.steps,
      0,
    )
    .expect("admitted plan has a checked fee envelope")
    .total;
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    fund_native(
      actor_id,
      required_in
        .saturating_add(fee_reserve)
        .saturating_add(min_user_balance),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SwapExecuted { actor_id: id, amount_in, amount_out, .. }
          if *id == actor_id && *amount_in == required_in && *amount_out == target_out
      )
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 1, skipped_resolution: 1, failed_steps: 0, .. },
          ..
        } if *id == actor_id
      )
    }));
    assert!(native_balance(&sovereign) >= min_user_balance);
  });
}

#[test]
fn swap_out_rounding_boundary_uses_minimal_input_for_target_output() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let out_before = Assets::balance(ASSET_A, sovereign.clone());
    let effective_quote = |gross_in: u128| -> Option<u128> {
      if gross_in == 0 {
        return None;
      }
      let fee = if crate::DeosRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::DeosRouter::calculate_router_fee(gross_in)
      };
      let net_in = gross_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      AssetConversionAdapter::quote_single_pool_exact_input(
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        net_in,
        true,
      )
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match effective_quote(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match effective_quote(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let expected_required_in = high;
    if expected_required_in > 1 {
      let prev_quote = effective_quote(expected_required_in.saturating_sub(1)).unwrap_or_default();
      assert!(
        prev_quote < target_out,
        "selected input must be minimal at rounding boundary"
      );
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let events = actor_events();
    let (amount_in, amount_out) = events
      .iter()
      .find_map(|event| match event {
        Event::SwapExecuted {
          actor_id: id,
          asset_in,
          asset_out,
          amount_in,
          amount_out,
          ..
        } if *id == actor_id
          && *asset_in == AssetKind::Native
          && *asset_out == AssetKind::Local(ASSET_A) =>
        {
          Some((*amount_in, *amount_out))
        }
        _ => None,
      })
      .unwrap_or_else(|| panic!("SwapExecuted must be emitted, events={events:?}"));
    assert_eq!(amount_out, target_out);
    assert_eq!(amount_in, expected_required_in);
    let out_after = Assets::balance(ASSET_A, sovereign.clone());
    assert!(out_after >= out_before.saturating_add(target_out));
  });
}

#[test]
fn swap_exact_out_liquidity_boundary_fails_without_partial_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let impossible_out = super::common::LIQUIDITY_AMOUNT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(impossible_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn swap_out_fails_when_required_input_exceeds_actor_balance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let quote_output = |amount_in: u128| -> Option<u128> {
      if amount_in == 0 {
        return None;
      }
      let fee = if crate::DeosRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::DeosRouter::calculate_router_fee(amount_in)
      };
      let net_in = amount_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      AssetConversionAdapter::quote_single_pool_exact_input(
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        net_in,
        true,
      )
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match quote_output(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match quote_output(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let required_in = high;
    fund_native(actor_id, required_in.saturating_sub(1));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn dex_exact_out_adapter_retries_unfunded_input_with_explicit_error() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let unfunded = crate::AccountId::new([99u8; 32]);
    let result = <crate::configs::actor_config::TmctolDexOps as DexOps<
      crate::AccountId,
      AssetKind,
      u128,
    >>::swap_exact_out(
      ExecutionContext::new(&unfunded, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      crate::EXISTENTIAL_DEPOSIT,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(100),
      Perbill::zero(),
    );
    assert_eq!(
      result,
      Err(pallet_deos_actors::TaskFailure::temporary(
        pallet_deos_router::Error::<Runtime>::InsufficientInputBalance
      ))
    );
  });
}

#[test]
fn remove_liquidity_failure_classifier_is_explicit_and_typed() {
  use crate::pallet_asset_conversion::Error as AssetConversionError;
  use pallet_deos_actors::RetryClass;

  for error in [
    AssetConversionError::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum,
    AssetConversionError::<Runtime>::AssetTwoWithdrawalDidNotMeetMinimum,
  ] {
    assert_eq!(
      classify_remove_liquidity_failure(error.into()).retry,
      RetryClass::Temporary
    );
  }
  for error in [
    AssetConversionError::<Runtime>::InvalidAssetPair,
    AssetConversionError::<Runtime>::PoolNotFound,
    AssetConversionError::<Runtime>::ZeroLiquidity,
  ] {
    assert_eq!(
      classify_remove_liquidity_failure(error.into()).retry,
      RetryClass::Permanent
    );
  }
}

#[test]
fn remove_liquidity_post_delta_guard_rejects_each_adversarial_mismatch() {
  assert_ok!(validate_remove_liquidity_output(10, 20, 10, 20));
  for result in [
    validate_remove_liquidity_output(9, 20, 10, 20),
    validate_remove_liquidity_output(10, 19, 10, 20),
  ] {
    assert_eq!(
      result,
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("MinimumLiquidityOutputNotMet")
      ))
    );
  }
}

#[test]
fn remove_liquidity_passes_each_minimum_to_asset_conversion_before_mutation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, AssetKind::Local(ASSET_A));
    let AssetKind::Local(lp_id) = lp_asset else {
      panic!("pool LP asset must be local");
    };
    let lp_before = Assets::balance(lp_id, &ALICE);
    let native_before = Balances::free_balance(&ALICE);
    let asset_before = Assets::balance(ASSET_A, &ALICE);
    let events_before = System::event_count();
    assert!(lp_before > 1);

    for (min_amount_a, min_amount_b) in [(u128::MAX, 1), (1, u128::MAX)] {
      let failure = TmctolLiquidityOps::remove_liquidity(
        &ALICE,
        lp_asset,
        pair.0,
        pair.1,
        lp_before / 2,
        min_amount_a,
        min_amount_b,
      )
      .expect_err("downstream authored minimum must reject before mutation");
      assert_eq!(failure.retry, RetryClass::Temporary);
      assert_eq!(Assets::balance(lp_id, &ALICE), lp_before);
      assert_eq!(Balances::free_balance(&ALICE), native_before);
      assert_eq!(Assets::balance(ASSET_A, &ALICE), asset_before);
      assert_eq!(System::event_count(), events_before);
    }
  });
}

#[test]
fn remove_liquidity_minimum_failure_preserves_each_error_policy_path() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, AssetKind::Local(ASSET_A));
    let AssetKind::Local(lp_id) = lp_asset else {
      panic!("pool LP asset must be local");
    };
    let lp_amount = Assets::minimum_balance(lp_id).max(10);

    for policy in [
      StepErrorPolicy::AbortCycle,
      StepErrorPolicy::ContinueNextStep,
      StepErrorPolicy::RetryLater { max_attempts: 3 },
    ] {
      let plan = alloc::vec![
        pallet_deos_actors::Step {
          precondition: None,
          task: Task::RemoveLiquidity {
            lp_asset,
            asset_a: pair.0,
            asset_b: pair.1,
            lp_amount: AmountResolution::Fixed(lp_amount),
            min_amount_a: Balance::MAX,
            min_amount_b: Balance::MAX,
          },
          on_error: policy,
        },
        pallet_deos_actors::Step {
          precondition: None,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        },
      ]
      .try_into()
      .expect("two-step plan fits");
      let actor_id = create_system(ALICE, manual_schedule(), None, plan);
      let actor = actor_account(actor_id);
      fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(2));
      assert_ok!(<Assets as FungiblesMutate<AccountId>>::mint_into(
        lp_id,
        &actor,
        lp_amount.saturating_mul(2)
      ));
      let lp_before = Assets::balance(lp_id, &actor);
      let native_before = Balances::free_balance(&actor);
      System::reset_events();

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_next_idle(Weight::MAX);

      assert_eq!(Assets::balance(lp_id, &actor), lp_before);
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::LiquidityRemoved { actor_id: id, .. } if *id == actor_id
      )));
      assert!(
        has_actor_event(|event| matches!(
          event,
          Event::StepFailed { actor_id: id, .. } if *id == actor_id
        )),
        "minimum failure must emit StepFailed: {:?}",
        System::events()
      );
      match policy {
        StepErrorPolicy::AbortCycle => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(!has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, .. } if *id == actor_id
          )));
          assert!(Actors::actor_run_state(actor_id).is_none());
        }
        StepErrorPolicy::ContinueNextStep => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, step_index: 1, .. } if *id == actor_id
          )));
          assert!(Actors::actor_run_state(actor_id).is_none());
        }
        StepErrorPolicy::RetryLater { .. } => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(!has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, .. } if *id == actor_id
          )));
          let continuation =
            Actors::actor_run_state(actor_id).expect("temporary minimum failure suspends");
          assert_eq!(continuation.cursor, 0);
          assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);
        }
      }
    }
  });
}

#[test]
fn router_failure_classifier_is_exhaustive_and_typed() {
  use pallet_deos_actors::RetryClass;
  use pallet_deos_router::Error as RouterError;

  for error in [
    RouterError::<Runtime>::SlippageExceeded,
    RouterError::<Runtime>::PriceDeviationExceeded,
    RouterError::<Runtime>::NoRouteFound,
    RouterError::<Runtime>::InsufficientLiquidity,
    RouterError::<Runtime>::InvalidOracleData,
    RouterError::<Runtime>::InsufficientInputBalance,
  ] {
    assert_eq!(classify_router_failure(error).retry, RetryClass::Temporary);
  }

  let temporary_adapter =
    pallet_deos_router::ExecutionError::<Runtime>::from(pallet_deos_router::AdapterFailure::new(
      DispatchError::Other("PublicationCapacity"),
      pallet_deos_router::RouterFailureClass::PublicationRejected,
      pallet_deos_router::RetryDisposition::RetryLater,
    ));
  assert_eq!(
    classify_router_execution_failure(temporary_adapter).retry,
    RetryClass::Temporary,
  );
  let unknown_adapter = pallet_deos_router::ExecutionError::<Runtime>::from(
    pallet_deos_router::AdapterFailure::unknown(DispatchError::Other("UnknownAdapterFailure")),
  );
  assert_eq!(
    classify_router_execution_failure(unknown_adapter).retry,
    RetryClass::Permanent,
  );

  for error in [
    RouterError::<Runtime>::IdenticalAssets,
    RouterError::<Runtime>::ZeroAmount,
    RouterError::<Runtime>::AmountTooLow,
    RouterError::<Runtime>::DeadlinePassed,
    RouterError::<Runtime>::FeeRoutingFailed,
    RouterError::<Runtime>::RouterFeeTooHigh,
    RouterError::<Runtime>::LpTokenPairCollision,
    RouterError::<Runtime>::LpPairCapacityExceeded,
    RouterError::<Runtime>::InvalidPoolPair,
    RouterError::<Runtime>::PreparedRouteMismatch,
  ] {
    assert_eq!(classify_router_failure(error).retry, RetryClass::Permanent);
  }
}

#[test]
fn market_execution_classifier_uses_the_concrete_cause() {
  use pallet_deos_actors::RetryClass as ActorRetryClass;
  use pallet_deos_router::{RetryDisposition as RouterRetryClass, RouterFailureClass};

  let recoverable = market_execution_failure(
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::PoolEmpty.into(),
  );
  assert_eq!(
    recoverable.failure_class(),
    RouterFailureClass::LiquidityUnavailable
  );
  assert_eq!(
    recoverable.retry_disposition(),
    RouterRetryClass::RetryLater
  );
  assert_eq!(
    classify_router_execution_failure(recoverable.into()).retry,
    ActorRetryClass::Temporary
  );

  for error in [
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::InvalidPath,
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::InvalidAssetPair,
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::PoolNotFound,
  ] {
    let permanent = market_execution_failure(error.into());
    assert_eq!(
      permanent.failure_class(),
      RouterFailureClass::InvariantViolation
    );
    assert_eq!(permanent.retry_disposition(), RouterRetryClass::Permanent);
    assert_eq!(
      classify_router_execution_failure(permanent.into()).retry,
      ActorRetryClass::Permanent
    );
  }
}

#[test]
fn system_actor_preserves_task_local_swap_amounts_without_fifo_priority() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let amount = 200 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    let quote = crate::DeosRouter::quote_exact_input(
      actor.clone(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
    )
    .expect("large System quote exists");
    let reference = quote
      .amount_out
      .saturating_mul(PRECISION)
      .saturating_div(quote.amount_after_fee);
    publish_deos_router_observation(AssetKind::Native, AssetKind::Local(ASSET_A), reference);
    let before = native_balance(&actor);
    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&actor, ActorType::System),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
      Perbill::one(),
    ));
    assert_eq!(before.saturating_sub(native_balance(&actor)), amount);
  });
}

#[test]
fn typed_system_swap_uses_stricter_reference_deviation_than_user_swap() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let amount = 10 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      PRECISION.saturating_mul(110).saturating_div(100),
    );
    let actor_before = native_balance(&actor);
    assert_eq!(
      crate::configs::actor_config::TmctolDexOps::swap_exact_in(
        ExecutionContext::new(&actor, ActorType::System),
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        amount,
        Perbill::one(),
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
    assert_eq!(native_balance(&actor), actor_before);

    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
      Perbill::one(),
    ));
  });
}

#[test]
fn missing_or_uninitialized_pool_feed_does_not_block_a_valid_user_swap() {
  use primitives::ecosystem::params::PRECISION;

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(ASSET_A);
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    assert_eq!(
      Oracle::observation_state(feed, 1).expect("maximum age is valid"),
      pallet_oracle::ObservationState::Uninitialized
    );
    assert_ok!(TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      asset_in,
      asset_out,
      10 * PRECISION,
      Perbill::one(),
    ));

    pallet_oracle::Feeds::<Runtime>::remove(feed);
    assert_eq!(
      Oracle::observation_state(feed, 1).expect("maximum age is valid"),
      pallet_oracle::ObservationState::Unavailable
    );
    assert_ok!(TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      asset_in,
      asset_out,
      10 * PRECISION,
      Perbill::one(),
    ));
  });
}

fn publish_deos_router_observation(asset_in: AssetKind, asset_out: AssetKind, value: Balance) {
  crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)
    .expect("test pair feed admission succeeds");
  Oracle::publish(
    RuntimeOrigin::signed(deos_router_account()),
    crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out),
    value,
  )
  .expect("DEOS Router producer publishes the observation");
}

#[test]
fn system_reference_guard_enforces_freshness_boundary_and_reserve_fallback() {
  seeded_test_ext().execute_with(|| {
    use crate::configs::actor_config::ActorMaxSystemReferenceAgeBlocks;
    use primitives::ecosystem::params::PRECISION;

    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(999_999);
    let max_age = ActorMaxSystemReferenceAgeBlocks::get();
    System::set_block_number(1);
    publish_deos_router_observation(asset_in, asset_out, PRECISION);
    System::set_block_number(max_age.saturating_add(1));
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      asset_out,
      PRECISION,
      PRECISION,
    ));

    System::set_block_number(max_age.saturating_add(2));
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        PRECISION,
        PRECISION,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemReferencePriceUnavailable")
      ))
    );

    let uninitialized_out = AssetKind::Local(999_998);
    assert_ok!(
      crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, uninitialized_out,)
    );
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        uninitialized_out,
        PRECISION,
        PRECISION,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemReferencePriceUnavailable")
      ))
    );

    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pooled_out = AssetKind::Local(ASSET_A);
    publish_deos_router_observation(asset_in, pooled_out, PRECISION.saturating_mul(10));
    System::set_block_number(
      System::block_number()
        .saturating_add(max_age)
        .saturating_add(1),
    );
    let (reserve_in, reserve_out) =
      crate::AssetConversion::get_reserves(asset_in, pooled_out).expect("pool reserves exist");
    let reserve_reference = primitives::checked_scaled_ratio(reserve_out, reserve_in, PRECISION)
      .expect("reference pool ratio is representable");
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      pooled_out,
      PRECISION,
      reserve_reference,
    ));
  });
}

#[test]
fn checked_reference_guard_is_exact_at_the_deviation_boundary_and_rejects_above() {
  seeded_test_ext().execute_with(|| {
    use primitives::ecosystem::params::PRECISION;
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(999_997);
    System::set_block_number(1);
    // Reference price 1.0 (scaled PRECISION).
    publish_deos_router_observation(asset_in, asset_out, PRECISION);
    let max_dev = crate::configs::actor_config::ActorMaxSystemPriceDeviation::get().deconstruct();
    // Exactly at the deviation limit: |exec_out * ref_in - ref_out * exec_in| * ACCURACY
    // == max_dev * ref_out * exec_in passes; one part above fails. With ref price 1.0
    // and exec_in = PRECISION, the exact margin is max_dev * PRECISION / ACCURACY.
    let margin = (max_dev as u128).saturating_mul(PRECISION) / 1_000_000_000u128;
    let exec_in = PRECISION;
    let exec_out = PRECISION.saturating_add(margin);
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      asset_out,
      exec_in,
      exec_out,
    ));
    let exec_out_above = PRECISION.saturating_add(margin).saturating_add(1);
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        exec_in,
        exec_out_above,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
    // Orientation reversal: a swapped quote (exec_out below reference by the same
    // margin) is rejected symmetrically by the absolute-value cross-multiplication.
    let exec_out_low = PRECISION.saturating_sub(margin).saturating_sub(1);
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        exec_in,
        exec_out_low,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
  });
}

#[test]
fn excessive_system_reference_deviation_suspends_without_fill_and_backs_off() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor_id = actor_ids::TREASURY_B_ACTORS_ID;
    let actor = Actors::sovereign_account_id_system(actor_id);
    let amount = 10 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(ASSET_A),
        amount_in: AmountResolution::Fixed(amount),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    }])
    .expect("single-step deviation retry plan fits");
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_id,
      ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
        window: None,
        steps: plan,
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: FundingSourcePolicy::RuntimePolicy,
        auto_close_at_cycle_nonce: None,
      },
    ));
    publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      PRECISION.saturating_mul(110).saturating_div(100),
    );
    let before = native_balance(&actor);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("deviation suspends");
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(continuation.cursor, 0);
    assert_eq!(native_balance(&actor), before);
    let first_retry = Actors::actor_hot(actor_id).expect("actor stays hot");
    assert!(first_retry.queue_ticket.is_some());
    assert!(first_retry.wakeup_pointer.is_none());

    System::set_block_number(2);
    run_idle(Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("deviation resuspends");
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(continuation.cursor, 0);
    assert_eq!(native_balance(&actor), before);
    let second_retry = Actors::actor_hot(actor_id).expect("actor stays hot");
    assert!(second_retry.queue_ticket.is_none());
    assert_eq!(
      second_retry.wakeup_pointer.map(|pointer| pointer.block),
      Some(WakeupKey::Block(4))
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        reason: pallet_deos_actors::SuspensionReason::Temporary,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_market_failure_opens_the_single_retry_continuation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::SwapOut {
        asset_out: AssetKind::Local(ASSET_A),
        amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
        asset_in: AssetKind::Native,
        input_limit: InputLimit::Absolute(1),
        slippage_tolerance: Perbill::zero(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    }])
    .expect("single-step retry plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let continuation = Actors::actor_run_state(actor_id).expect("Temporary failure suspends");
    assert_eq!(continuation.cursor, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        cursor: 0,
        reason: pallet_deos_actors::SuspensionReason::Temporary,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_oracle_capacity_failure_rolls_back_economics_and_has_one_retry_owner() {
  use primitives::ecosystem::params::PRECISION;

  for exact_output in [false, true] {
    seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(super::common::setup_deos_router_infrastructure());
      for block in 2..=19 {
        System::set_block_number(block);
        Actors::on_initialize(block);
        run_idle(Weight::MAX);
      }
      let asset_in = AssetKind::Native;
      let asset_out = AssetKind::Local(ASSET_A);
      let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
      crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)
        .expect("directional pool feeds fit");
      create_system(
        ALICE,
        observation_schedule(feed),
        None,
        BoundedVec::try_from(vec![make_step(inert_task())]).expect("one inert step fits"),
      );
      let task = if exact_output {
        Task::SwapOut {
          asset_out,
          amount_out: AmountResolution::Fixed(PRECISION),
          asset_in,
          input_limit: InputLimit::Absolute(100 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      } else {
        Task::SwapIn {
          asset_in,
          asset_out,
          amount_in: AmountResolution::Fixed(10 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      };
      let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
        precondition: None,
        task,
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      }])
      .expect("single-step publication retry plan fits");
      let actor_id = create_user(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 1_000 * PRECISION);
      let actor = actor_account(actor_id);
      let input_before = native_balance(&actor);
      let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
      let burn_actor = super::common::burn_actor_account();
      let router_fee_before = native_balance(&burn_actor);
      let burn_cycle_before = Actors::active_actor_state(burn_actor_id)
        .expect("Burn Actor exists")
        .identity
        .cycle_nonce;
      let output_before = Assets::balance(ASSET_A, &actor);
      let pool_before =
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool exists");
      let reward_liability_before = Staking::native_security_reward_liability();
      let reward_account = Staking::native_security_reward_account();
      let reward_custody_before = native_balance(&reward_account);
      let dirty_capacity = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
      pallet_deos_actors::DirtyObservationListState::<Runtime>::mutate(|list| {
        list.count = dirty_capacity;
      });

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_next_idle(Weight::MAX);

      let input_after_failure = native_balance(&actor);
      assert!(input_after_failure < input_before);
      assert_eq!(native_balance(&burn_actor), router_fee_before);
      assert_eq!(
        Actors::active_actor_state(burn_actor_id)
          .expect("Burn Actor remains active")
          .identity
          .cycle_nonce,
        burn_cycle_before,
      );
      assert_eq!(Assets::balance(ASSET_A, &actor), output_before);
      assert_eq!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert!(Oracle::observations(feed).is_none());
      assert!(Actors::dirty_observation_feeds(feed).is_none());
      assert_eq!(Actors::dirty_observation_feed_count(), dirty_capacity);
      assert_eq!(
        Staking::native_security_reward_liability(),
        reward_liability_before
      );
      assert_eq!(native_balance(&reward_account), reward_custody_before);
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(|event| matches!(
            event,
            Event::CycleSuspended {
              actor_id: id,
              reason: pallet_deos_actors::SuspensionReason::Temporary,
              ..
            } if *id == actor_id
          ))
          .count(),
        1,
      );
      let continuation = Actors::actor_run_state(actor_id).expect("publication retry suspends");
      assert_eq!(continuation.cursor, 0);
      let hot = Actors::actor_hot(actor_id).expect("suspended Actor stays hot");
      assert!(hot.queue_ticket.is_some());
      assert!(hot.wakeup_pointer.is_none());

      pallet_deos_actors::DirtyObservationListState::<Runtime>::kill();
      System::set_block_number(21);
      run_idle(Weight::MAX);

      assert!(Actors::actor_run_state(actor_id).is_none());
      assert!(native_balance(&actor) < input_after_failure);
      assert!(Assets::balance(ASSET_A, &actor) > output_before);
      assert_ne!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert_eq!(
        Oracle::observations(feed)
          .expect("retry publishes")
          .revision,
        1
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        1
      );

      System::set_block_number(22);
      run_idle(Weight::MAX);
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        1
      );
    });
  }
}

#[test]
fn permanent_publication_invariant_terminates_without_cross_system_mutation_or_retry() {
  use primitives::ecosystem::params::PRECISION;

  for exact_output in [false, true] {
    seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(super::common::setup_deos_router_infrastructure());
      for block in 2..=19 {
        System::set_block_number(block);
        Actors::on_initialize(block);
        run_idle(Weight::MAX);
      }
      let asset_in = AssetKind::Native;
      let asset_out = AssetKind::Local(ASSET_A);
      let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
      pallet_oracle::Feeds::<Runtime>::mutate(feed, |maybe| {
        maybe.as_mut().expect("pool feed is registered").producer = ALICE;
      });
      let task = if exact_output {
        Task::SwapOut {
          asset_out,
          amount_out: AmountResolution::Fixed(PRECISION),
          asset_in,
          input_limit: InputLimit::Absolute(100 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      } else {
        Task::SwapIn {
          asset_in,
          asset_out,
          amount_in: AmountResolution::Fixed(10 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      };
      let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
        precondition: None,
        task,
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      }])
      .expect("single-step permanent publication plan fits");
      let actor_id = create_user(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 1_000 * PRECISION);
      let actor = actor_account(actor_id);
      let actor_input_before = native_balance(&actor);
      let actor_contract_before = Actors::actor_contract(actor_id).expect("Actor Contract exists");
      let output_before = Assets::balance(ASSET_A, &actor);
      let pool_before =
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool exists");
      let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
      let burn_actor = super::common::burn_actor_account();
      let burn_balance_before = native_balance(&burn_actor);
      let burn_cycle_before = Actors::active_actor_state(burn_actor_id)
        .expect("Burn Actor exists")
        .identity
        .cycle_nonce;
      let reward_liability_before = Staking::native_security_reward_liability();
      let reward_account = Staking::native_security_reward_account();
      let reward_custody_before = native_balance(&reward_account);
      let staking_participants_before = Staking::native_security_participants();
      let governance_coefficient_before = Staking::governance_participation_coefficient(0, &actor);

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_next_idle(Weight::MAX);

      assert!(
        native_balance(&actor) < actor_input_before,
        "the bounded attempt remains paid"
      );
      assert_eq!(
        Actors::actor_contract(actor_id).expect("Actor Contract remains"),
        actor_contract_before,
      );
      assert!(Actors::actor_run_state(actor_id).is_none());
      let hot = Actors::actor_hot(actor_id).expect("Actor hot state remains");
      assert!(hot.queue_ticket.is_none());
      assert!(hot.wakeup_pointer.is_none());
      assert_eq!(Assets::balance(ASSET_A, &actor), output_before);
      assert_eq!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert_eq!(native_balance(&burn_actor), burn_balance_before);
      assert_eq!(
        Actors::active_actor_state(burn_actor_id)
          .expect("Burn Actor remains active")
          .identity
          .cycle_nonce,
        burn_cycle_before,
      );
      assert!(Oracle::observations(feed).is_none());
      assert!(Actors::dirty_observation_feeds(feed).is_none());
      assert_eq!(
        Staking::native_security_reward_liability(),
        reward_liability_before
      );
      assert_eq!(native_balance(&reward_account), reward_custody_before);
      assert_eq!(
        Staking::native_security_participants(),
        staking_participants_before
      );
      assert_eq!(
        Staking::governance_participation_coefficient(0, &actor),
        governance_coefficient_before,
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );

      System::set_block_number(21);
      run_idle(Weight::MAX);
      assert!(Actors::actor_run_state(actor_id).is_none());
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );

      if !exact_output {
        assert_noop!(
          crate::DeosRouter::swap(
            RuntimeOrigin::signed(ALICE),
            asset_in,
            asset_out,
            10 * PRECISION,
            0,
            BOB,
            u32::MAX,
          ),
          pallet_deos_router::Error::<Runtime>::InvalidOracleData
        );
      }
    });
  }
}

#[test]
fn staking_adapter_supports_liquid_native_stake_without_operator_context() {
  seeded_test_ext().execute_with(|| {
    let who = crate::AccountId::new([77u8; 32]);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &who,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10)
    ));
    let result =
      <crate::configs::actor_config::TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<
        crate::AccountId,
        AssetKind,
        u128,
      >>::stake(&who, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT);
    assert_ok!(result);
    assert!(crate::Staking::live_native_staked_receipt_balance(&who).unwrap_or_default() > 0);
  });
}

#[test]
fn actor_unstake_percentage_current_resolves_live_staking_shares() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let steps = BoundedVec::try_from(vec![make_step(Task::Unstake {
      asset: AssetKind::Native,
      shares: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_user(BOB, manual_schedule(), None, steps);
    let actor = actor_account(actor_id);
    let stake_amount = crate::EXISTENTIAL_DEPOSIT.saturating_mul(10);
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &actor,
      stake_amount.saturating_add(crate::EXISTENTIAL_DEPOSIT),
    ));
    assert_ok!(crate::configs::actor_config::TmctolStakingOps::stake(
      &actor,
      AssetKind::Native,
      stake_amount,
    ));
    fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(10));
    let shares_before =
      crate::configs::actor_config::TmctolStakingOps::share_balance(&actor, AssetKind::Native);
    assert!(shares_before > 0);
    assert_eq!(
      crate::configs::actor_config::TmctolStakingOps::share_asset(AssetKind::Native),
      crate::Staking::staked_asset_id_for_queries(0).map(AssetKind::Local)
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      crate::configs::actor_config::TmctolStakingOps::share_balance(&actor, AssetKind::Native),
      shares_before.saturating_sub(Perbill::from_percent(50).mul_floor(shares_before))
    );
  });
}

#[test]
fn actor_native_stake_task_mints_liquid_stntve_without_binding() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    crate::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let steps = BoundedVec::try_from(vec![make_step(Task::Stake {
      asset: AssetKind::Local(0),
      amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
    })])
    .expect("steps fits");
    let actor_id = create_user(BOB, manual_schedule(), None, steps);
    let actor_acc = actor_account(actor_id);
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &actor_acc,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10),
    ));
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), actor_id));
    run_idle(Weight::MAX);
    assert!(
      crate::Staking::live_native_staked_receipt_balance(&actor_acc).unwrap_or_default() > 0,
      "Actors sovereign must receive stNTVE after native stake"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StakeExecuted {
          actor_id: id,
          asset: AssetKind::Local(0),
          amount,
          ..
        } if *id == actor_id && *amount == crate::EXISTENTIAL_DEPOSIT
      )
    }));
  });
}

// --- Actors Platform: SplitTransfer ---

#[test]
fn split_transfer_uses_perbill_and_keeps_remainder_on_actor() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let total = 101u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let steps = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    let actor_acc = actor_account(actor_id);
    let actor_before = native_balance(&actor_acc);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    let spent = actor_before.saturating_sub(native_balance(&actor_acc));
    assert!(
      spent >= 100,
      "Actors must spend at least distributed amount"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          actor_id: id,
          total: amount,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == actor_id
          && *amount == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn native_preflight_requires_a_free_ed_anchor_for_sub_ed_ingress() {
  seeded_test_ext().execute_with(|| {
    let provider_only = AccountId::new([70u8; 32]);
    let reserved_anchor = AccountId::new([71u8; 32]);
    let free_anchor = AccountId::new([72u8; 32]);
    let existential_deposit = crate::EXISTENTIAL_DEPOSIT;
    let amount = existential_deposit / 2;
    let unavailable = || {
      Err(pallet_deos_actors::TaskFailure::temporary(
        Error::<Runtime>::RecipientDepositUnavailable,
      ))
    };

    System::inc_providers(&provider_only);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &provider_only, AssetKind::Native, amount,),
      unavailable()
    );

    System::inc_providers(&reserved_anchor);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &reserved_anchor,
      existential_deposit,
      ExistenceRequirement::AllowDeath,
    ));
    assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(
      &reserved_anchor,
      existential_deposit,
    ));
    assert_eq!(Balances::free_balance(&reserved_anchor), 0);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &reserved_anchor, AssetKind::Native, amount,),
      unavailable()
    );

    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &free_anchor,
      existential_deposit,
      ExistenceRequirement::AllowDeath,
    ));
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE,
      &free_anchor,
      AssetKind::Native,
      amount,
    ));
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &free_anchor,
      AssetKind::Native,
      amount,
    ));
    assert_eq!(
      Balances::free_balance(&free_anchor),
      existential_deposit.saturating_add(amount)
    );
  });
}

#[test]
fn anchored_split_transfer_rolls_back_when_a_later_recipient_is_unavailable() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let anchored = AccountId::new([73u8; 32]);
    let provider_only = AccountId::new([74u8; 32]);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &anchored,
      crate::EXISTENTIAL_DEPOSIT,
      ExistenceRequirement::AllowDeath,
    ));
    System::inc_providers(&provider_only);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: anchored.clone(),
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: provider_only.clone(),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("two split legs fit");
    let plan = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(2),
      legs,
    })])
    .expect("split plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(10));
    let anchored_before = native_balance(&anchored);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&anchored), anchored_before);
    assert_eq!(native_balance(&provider_only), 0);
  });
}

#[test]
fn foreign_asset_preflight_enforces_exact_minimum_boundary() {
  seeded_test_ext().execute_with(|| {
    const ASSET_ID: u32 = 77_707;
    let below = AccountId::new([71u8; 32]);
    let equal = AccountId::new([72u8; 32]);
    let above = AccountId::new([73u8; 32]);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      ASSET_ID,
      ALICE.clone().into(),
      true,
      100,
    ));
    assert_ok!(mint_tokens(ASSET_ID, &ALICE, &ALICE, 1_000));
    let asset = AssetKind::Foreign(ASSET_ID);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &below, asset, 99),
      Err(pallet_deos_actors::TaskFailure::temporary(
        Error::<Runtime>::RecipientDepositUnavailable,
      ))
    );
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE, &equal, asset, 100
    ));
    assert_ok!(TmctolAssetOps::transfer(&ALICE, &equal, asset, 100));
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE, &above, asset, 101
    ));
    assert_ok!(TmctolAssetOps::transfer(&ALICE, &above, asset, 101));
    assert_eq!(
      <Assets as FungiblesInspect<AccountId>>::balance(ASSET_ID, &equal),
      100
    );
    assert_eq!(
      <Assets as FungiblesInspect<AccountId>>::balance(ASSET_ID, &above),
      101
    );
  });
}

#[test]
fn split_transfer_rejects_ed_ineligible_recipient_then_retries_atomically() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let unknown = crate::AccountId::new([9u8; 32]);
    let total = 100u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: unknown.clone(),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let mut step = make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let steps = BoundedVec::try_from(vec![step]).expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    let actor = actor_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&unknown), 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, error, .. }
        if *id == actor_id
          && *error == Error::<Runtime>::RecipientDepositUnavailable.into()
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id
    )));
    let continuation = Actors::actor_run_state(actor_id).expect("temporary rejection suspends");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);

    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&unknown, crate::EXISTENTIAL_DEPOSIT);
    let unknown_before = native_balance(&unknown);
    System::set_block_number(2);
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), actor_before - total);
    assert_eq!(native_balance(&BOB), bob_before + 50);
    assert_eq!(native_balance(&unknown), unknown_before + 50);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id: id,
        total: 100,
        distributed: 100,
        retained: 0,
        legs: 2,
        effective_legs: 2,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn create_rejects_split_transfer_share_sum_above_one() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(60),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let steps = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    })])
    .expect("steps fits");
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, steps),
      ),
      Error::<Runtime>::InvalidSplitTransfer
    );
  });
}

// --- Actors Platform: Bounds & Validation ---

#[test]
fn split_transfer_leg_count_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_legs =
      <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get() as usize;
    let within_limit = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let above_limit = (0..max_legs.saturating_add(1))
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    assert!(SplitTransferLegsOf::<Runtime>::try_from(within_limit).is_ok());
    assert!(SplitTransferLegsOf::<Runtime>::try_from(above_limit).is_err());
  });
}

#[test]
fn whitelist_size_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_whitelist =
      <<Runtime as pallet_deos_actors::Config>::MaxWhitelistSize as Get<u32>>::get() as usize;
    let within_limit = (0..max_whitelist)
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    let above_limit = (0..max_whitelist.saturating_add(1))
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize>::try_from(
        within_limit
      )
      .is_ok()
    );
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize>::try_from(
        above_limit
      )
      .is_err()
    );
  });
}

#[test]
fn typed_ten_julian_year_horizons_match_runtime_clocks() {
  const JULIAN_YEAR_MILLIS: u64 = 36525 * 24 * 60 * 60 * 10;
  const TEN_JULIAN_YEARS_MILLIS: u64 = JULIAN_YEAR_MILLIS * 10;
  let block_horizon = TEN_JULIAN_YEARS_MILLIS.div_ceil(crate::SLOT_DURATION);
  let cadence_horizon =
    TEN_JULIAN_YEARS_MILLIS.div_ceil(crate::configs::actor_config::ActorCadenceTickMillis::get());
  assert_eq!(crate::SLOT_DURATION, 6_000);
  assert_eq!(block_horizon, 52_596_000);
  assert_eq!(cadence_horizon, 631_152_000);
  assert_eq!(
    u64::from(crate::configs::actor_config::ActorMaxExecutionDelayBlocks::get()),
    block_horizon
  );
  assert_eq!(
    crate::configs::actor_config::ActorMaxTemporalDelayTicks::get(),
    cadence_horizon
  );
}

#[test]
fn timer_horizon_validation_accepts_exact_runtime_bound() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let largest_valid_temporal_delay =
      <Runtime as pallet_deos_actors::Config>::MaxTemporalDelayTicks::get();
    assert!(
      largest_valid_temporal_delay
        > u64::from(<Runtime as pallet_deos_actors::Config>::MaxExecutionDelayBlocks::get())
    );
    for trigger in [
      Trigger::at_time(largest_valid_temporal_delay),
      Trigger::cadenced(largest_valid_temporal_delay),
    ] {
      let valid_plan = transfer_contract_steps(BOB, AssetKind::Native, 1);
      prefund_active_user_creation(&ALICE, &valid_plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          Schedule {
            trigger,
            cooldown_blocks: 0,
          },
          None,
          valid_plan,
        ),
      ));
    }
    for trigger in [
      Trigger::at_time(largest_valid_temporal_delay.saturating_add(1)),
      Trigger::cadenced(largest_valid_temporal_delay.saturating_add(1)),
    ] {
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(
            Schedule {
              trigger,
              cooldown_blocks: 0,
            },
            None,
            transfer_contract_steps(BOB, AssetKind::Native, 1),
          ),
        ),
        Error::<Runtime>::ExecutionDelayTooLong
      );
    }
  });
}

// --- Actors Platform: Trigger & Source Filter ---

#[test]
fn on_address_event_owner_only_respects_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &BOB
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    System::set_block_number(System::block_number().saturating_add(1));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_asset_filter_is_enforced() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let asset_whitelist = BoundedVec::try_from(vec![AssetKind::Local(ASSET_A)]).expect("fits");
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(asset_whitelist)),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Local(ASSET_A),
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    System::set_block_number(System::block_number().saturating_add(1));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_without_source_is_ignored_for_filtered_trigger() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      AssetKind::Native,
      100
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn internal_asset_transfer_rolls_back_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let alice_before = native_balance(&ALICE);
    let sovereign_before = native_balance(&sovereign);
    assert_eq!(
      <TmctolAssetOps as AssetOps<AccountId, AssetKind, Balance>>::transfer(
        &ALICE,
        &sovereign,
        AssetKind::Native,
        1,
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        Error::<Runtime>::FundingAccumulatorOverflow,
      ))
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn asset_ops_transfer_notifies_on_address_event_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_amount = 1_000u128;
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, receiver_amount),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_contract_steps(receiver_sovereign, AssetKind::Native, 5_000),
    );
    let sender_sovereign = actor_account(sender_id);
    let sender_whitelist = BoundedVec::try_from(vec![sender_sovereign]).expect("fits");
    let schedule =
      on_address_event_schedule(SourceFilter::Whitelist(sender_whitelist), AssetFilter::Any);
    assert_ok!(update_actor_contract_partial(
      RuntimeOrigin::signed(ALICE),
      receiver_id,
      (schedule.trigger, schedule.cooldown_blocks, None),
    ));
    fund_native(sender_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(receiver_id)
        .expect("receiver exists")
        .identity
        .cycle_nonce,
      0
    );
    assert!(
      Actors::actor_hot(receiver_id)
        .expect("receiver hot state")
        .queue_ticket
        .is_some(),
      "an address event created during on_idle must survive as next-block work"
    );
    assert!(Actors::pending_signal(receiver_id));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before.saturating_add(receiver_amount)
    );
  });
}

#[test]
fn repeated_same_block_transfers_coalesce_to_one_actor_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    fund_native_via_call(ALICE, actor_id, 100_000_000_000_000);
    fund_native_via_call(ALICE, actor_id, 50_000_000_000_000);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .cycle_nonce,
      1,
      "multiple same-block funding events must coalesce into one execution"
    );
  });
}

#[test]
fn split_transfer_legs_to_actor_sovereigns_notify_through_certified_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_a = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_b = create_user(
      BOB,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_a_sovereign = actor_account(receiver_a);
    let receiver_b_sovereign = actor_account(receiver_b);
    let legs = SplitTransferLegsOf::<Runtime>::try_from(vec![
      SplitLeg {
        to: receiver_a_sovereign.clone(),
        share: Perbill::from_percent(40),
      },
      SplitLeg {
        to: receiver_b_sovereign,
        share: Perbill::from_percent(40),
      },
    ])
    .expect("two legs fit");
    let sender = create_system(
      CHARLIE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(10_000),
        legs,
      })])
      .expect("plan fits"),
    );
    fund_native(sender, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), sender));
    run_idle(Weight::MAX);
    assert!(
      Actors::pending_signal(receiver_a),
      "first SplitTransfer leg to an Actors sovereign must latch readiness"
    );
    assert!(
      Actors::pending_signal(receiver_b),
      "second SplitTransfer leg to an Actors sovereign must latch readiness"
    );
  });
}

#[test]
fn mint_to_actor_sovereign_notifies_source_less_certified_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_sovereign = actor_account(receiver);
    let before = native_balance(&receiver_sovereign);
    // A certified Mint to an Actors sovereign destination (the Actors Mint task calls
    // the same adapter) must create the value and notify source-less ingress.
    assert_ok!(TmctolAssetOps::mint(
      &receiver_sovereign,
      AssetKind::Native,
      10_000,
    ));
    assert_eq!(
      native_balance(&receiver_sovereign),
      before
        .saturating_add(10_000)
        .saturating_sub(address_event_trigger_fee()),
      "mint creates value and the sovereign independently pays Trigger materialization"
    );
    assert!(
      Actors::pending_signal(receiver),
      "Mint to an Actors sovereign must notify source-less certified ingress"
    );
  });
}

#[test]
fn same_block_funding_signals_coalesce_to_one_actor_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    fund_native(actor_id, 100_000_000_000_000);
    fund_native_via_call(ALICE, actor_id, 50_000_000_000_000);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .cycle_nonce,
      1,
      "manual and funding readiness must share one live queue membership"
    );
  });
}

#[test]
fn runtime_rejects_self_transfer_before_contract_replacement() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let before = Actors::active_actor_state(actor_id).expect("actor exists");
    let before_encoded = before.encode();
    assert_noop!(
      update_actor_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        transfer_contract_steps(
          before.identity.sovereign_account.clone(),
          AssetKind::Native,
          1_000,
        ),
        CompletionPolicy::Persistent,
      ),
      Error::<Runtime>::SelfTransferNotAllowed
    );
    assert_eq!(
      Actors::active_actor_state(actor_id).map(|state| state.encode()),
      Some(before_encoded)
    );
  });
}

#[test]
fn circular_actor_graph_cannot_reexecute_an_actor_in_the_same_block() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let event_schedule = on_address_event_schedule(SourceFilter::Any, AssetFilter::Any);
    let actor_a = create_user(
      ALICE,
      event_schedule.clone(),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let actor_a_account = actor_account(actor_a);
    let actor_b = create_user(
      CHARLIE,
      event_schedule,
      None,
      transfer_contract_steps(actor_a_account, AssetKind::Native, 1_000),
    );
    let actor_b_account = actor_account(actor_b);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_a,
      transfer_contract_steps(actor_b_account, AssetKind::Native, 1_000),
      CompletionPolicy::Persistent,
    ));
    System::set_block_number(2);
    for (owner, actor_id) in [(ALICE, actor_a), (CHARLIE, actor_b)] {
      assert_ok!(update_actor_contract_partial!(
        RuntimeOrigin::signed(owner.clone()),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress,
      ));
      fund_native_via_call(owner, actor_id, 100_000_000_000_000);
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(actor_a)
        .expect("actor A exists")
        .identity
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_state(actor_b)
        .expect("actor B exists")
        .identity
        .cycle_nonce,
      1
    );
    assert!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot state")
        .queue_ticket
        .is_some(),
      "B triggering already-executed A must create next-block work"
    );
    System::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(actor_a)
        .expect("actor A exists")
        .identity
        .cycle_nonce,
      2
    );
    assert_eq!(
      Actors::active_actor_state(actor_b)
        .expect("actor B exists")
        .identity
        .cycle_nonce,
      1,
      "A-triggered recursive work for B must remain beyond the next block's cutoff"
    );
  });
}

#[test]
fn actor_observation_provider_maps_oracle_state_without_concrete_pallet_dependency() {
  seeded_test_ext().execute_with(|| {
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(ASSET_A);
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 0, 10),
      pallet_deos_actors::ScalarObservationState::Unavailable
    );
    assert_ok!(crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out,));
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 0, 10),
      pallet_deos_actors::ScalarObservationState::Uninitialized
    );
    System::set_block_number(1);
    publish_deos_router_observation(asset_in, asset_out, 50);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 1, 10),
      pallet_deos_actors::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      }
    );
    System::set_block_number(12);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 12, 10),
      pallet_deos_actors::ScalarObservationState::Stale
    );
  });
}

#[test]
fn router_fee_routing_notifies_burn_actor_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
    let schedule = on_address_event_schedule(SourceFilter::Any, AssetFilter::Any);
    assert_ok!(update_actor_contract_partial(
      RuntimeOrigin::root(),
      burn_actor_id,
      (schedule.trigger, schedule.cooldown_blocks, None),
    ));
    System::set_block_number(2);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      transfer_contract_steps(BOB, AssetKind::Native, 777),
      CompletionPolicy::Persistent,
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(
      crate::configs::deos_router_config::FeeManagerImpl::<Runtime>::route_fee(
        &ALICE,
        AssetKind::Native,
        10_000,
      )
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(777));
  });
}

#[test]
fn router_fee_transfer_rolls_back_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
    let funding_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      (funding_plan, CompletionPolicy::Persistent,)
    ));
    System::set_block_number(2);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(burn_actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(burn_actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Burn Actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let alice_before = native_balance(&ALICE);
    let sovereign_before = native_balance(&sovereign);
    assert_noop!(
      crate::configs::deos_router_config::FeeManagerImpl::<Runtime>::route_fee(
        &ALICE,
        AssetKind::Native,
        10_000,
      ),
      pallet_deos_router::AdapterFailure::new(
        Error::<Runtime>::FundingAccumulatorOverflow.into(),
        pallet_deos_router::RouterFailureClass::IngressRejected,
        pallet_deos_router::RetryDisposition::Permanent,
      )
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn deos_sovereign_account_policy_reserves_genesis_custody_accounts() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // Every genesis System Actors custody account (including the Fee Sink) is host-reserved by
    // DeosSovereignAccountPolicy; a hashed sovereign derivation can never alias them.
    let ids = primitives::ecosystem::actor_ids::BURN_ACTOR_ID
      ..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID;
    for id in ids {
      let sovereign = crate::Actors::sovereign_account_id_system(id);
      assert!(
        crate::configs::actor_config::DeosSovereignAccountPolicy::is_reserved(&sovereign),
        "genesis System Actors id {id} sovereign must be reserved"
      );
    }
    // A User-slot derived sovereign is not reserved, so ordinary creation remains admissible.
    let slot = 200u8;
    let user_sovereign = crate::Actors::sovereign_account_id(&ALICE, slot);
    assert!(
      !crate::configs::actor_config::DeosSovereignAccountPolicy::is_reserved(&user_sovereign)
    );
  });
}

#[test]
fn genesis_system_locator_is_recoverable_after_close_through_reattachment() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // A genesis System locator (the Fee Sink) is host-reserved for fresh derivation
    // but MUST be recoverable by reattaching a fresh actor to its exact registered
    // Vacant locator after close (spec 5.4): context-aware reservation.
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let sovereign = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let original_sovereign_balance_before = Balances::free_balance(&sovereign);
    let preserved = crate::EXISTENTIAL_DEPOSIT.saturating_mul(777);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, preserved);
    let original_identity = Actors::actor_identities(fee_sink_id).expect("genesis identity");
    let _original_nonce = original_identity.cycle_nonce;

    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), fee_sink_id));
    assert_eq!(
      crate::Actors::system_sovereigns(fee_sink_id),
      Some(pallet_deos_actors::SystemSovereignState::Vacant)
    );

    // Reattachment to the exact registered Vacant locator is allowed even though the
    // account belongs to the genesis System custody range.
    let fresh_id = crate::Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      fee_sink_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, AssetKind::Native, 1),
      ),
    ));
    let fresh = Actors::active_actor_state(fresh_id).expect("fresh Fee Sink identity");
    assert_ne!(fresh_id, fee_sink_id);
    assert_eq!(fresh.identity.sovereign_account, sovereign);
    // Reattachment mints a fresh identity with a fresh nonce sequence (zero), never
    // inheriting the closed actor's nonce or run state.
    assert_eq!(
      fresh.identity.cycle_nonce, 0,
      "reattachment resets the nonce"
    );
    assert_eq!(
      Balances::free_balance(&sovereign),
      preserved + original_sovereign_balance_before,
      "reattachment preserves residual custody balances"
    );
  });
}

#[test]
fn user_exact_slot_recovery_reuses_residual_runtime_custody() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let slot = 200u8;
    let steps = transfer_contract_steps(BOB, AssetKind::Native, 333);
    prefund_user_sovereign(&ALICE, slot, &steps);
    let first_id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      slot,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, steps.clone()),
    ));
    let sovereign = Actors::sovereign_account_id(&ALICE, slot);
    let residual = 10_000_000_000_000u128;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, residual);
    let preserved = Balances::free_balance(&sovereign);

    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first_id));
    assert_eq!(Balances::free_balance(&sovereign), preserved);

    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      slot,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, steps),
    ));
    let fresh = Actors::active_actor_state(fresh_id).expect("fresh User identity exists");
    assert_ne!(fresh_id, first_id);
    assert_eq!(fresh.identity.sovereign_account, sovereign);
    assert_eq!(fresh.identity.cycle_nonce, 0);
    assert_eq!(Balances::free_balance(&sovereign), preserved);

    let bob_before = Balances::free_balance(BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      fresh_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(333));
    assert!(Balances::free_balance(&sovereign) < preserved);
  });
}

#[test]
fn ingress_adapter_without_source_matches_any_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(RuntimeAddressEventIngress::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(333));
  });
}

#[test]
fn ingress_adapter_without_source_is_ignored_by_owner_only_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(RuntimeAddressEventIngress::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn transfer_ingress_updates_system_snapshot_without_pause_resume() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let target_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      target_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    fund_native_via_call(ALICE, target_id, 10_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 1);
    System::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 2);
    System::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 3);
    let instance = Actors::active_actor_state(target_id).expect("Actors exists");
    assert_eq!(
      instance.hot.lifecycle,
      pallet_deos_actors::ActiveLifecycle::Active
    );
    let target_sovereign = actor_account(target_id);
    let refill_amount = 8_000_000_000_000u128;
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_contract_steps(target_sovereign, AssetKind::Native, refill_amount),
    );
    fund_native(sender_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    System::set_block_number(System::block_number().saturating_add(1));
    run_idle(Weight::MAX);
    assert_eq!(
      actor_funding(target_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&refill_amount)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == target_id)
    }));
  });
}

#[test]
fn xcm_ingress_with_source_triggers_owner_only_on_address_event() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign.clone());
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [7u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn system_runtime_policy_defaults_deny_for_signed_internal_and_xcm_provenance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let recipient = account_location(sovereign.clone());
    let sourced_amount = 10_000_000_000_000;
    let context = xcm::latest::XcmContext {
      origin: Some(account_location(ALICE)),
      message_id: [6u8; 32],
      topic: None,
    };
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(native_xcm_asset(sourced_amount)),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    let source_less_amount = 7_000_000_000_000;
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(native_xcm_asset(source_less_amount)),
        &recipient,
        None,
      )
      .is_ok()
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      3_000,
      &ALICE
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      AssetKind::Native,
      4_000,
      &ALICE
    ));
    assert_eq!(
      native_balance(&sovereign),
      sourced_amount.saturating_add(source_less_amount)
    );
    let funding = actor_funding(actor_id);
    assert!(
      funding
        .funding_accumulated
        .get(&AssetKind::Native)
        .is_none()
    );
  });
}

#[test]
fn xcm_deposit_rejects_before_value_movement_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let recipient = account_location(sovereign.clone());
    let context = xcm::latest::XcmContext {
      origin: Some(account_location(ALICE)),
      message_id: [8u8; 32],
      topic: None,
    };
    let sovereign_before = native_balance(&sovereign);
    let result = <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
      asset_to_holding(native_xcm_asset(5_000)),
      &recipient,
      Some(&context),
    );
    assert!(matches!(
      result,
      Err((_, xcm::latest::Error::FailedToTransactAsset(_)))
    ));
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn xcm_source_less_scheduler_exhaustion_closes_actor_and_preserves_deposit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let sovereign = actor_account(actor_id);
    let recipient = account_location(sovereign.clone());
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let before = native_balance(&sovereign);
    let asset = native_xcm_asset(5_000);
    assert_ok!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset.clone()),
        &recipient,
        None,
      )
    );
    assert!(Actors::active_actor_state(actor_id).is_none());

    assert_ok!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset_with_surplus(
        asset_to_holding(asset.clone()),
        &recipient,
        None,
      )
    );
    assert_eq!(native_balance(&sovereign), before.saturating_add(10_000));
  });
}

#[test]
fn xcm_deposit_failure_rolls_back_precommit_events_and_exact_holding() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let sovereign = actor_account(actor_id);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      polkadot_sdk::sp_runtime::MultiAddress::Id(sovereign.clone()),
      u128::MAX,
    ));
    let recipient = account_location(sovereign);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let asset = native_xcm_asset(1);
    let result = <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
      asset_to_holding(asset.clone()),
      &recipient,
      None,
    );
    let Err((returned, _)) = result else {
      panic!("ledger overflow must reject after the Actors precommit");
    };
    assert_eq!(returned.assets_iter().collect::<Vec<_>>(), vec![asset]);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "deposit failure restores the precommit, events, ledger, and holding"
    );
  });
}

#[test]
fn xcm_ingress_without_source_is_ignored_for_owner_only() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        None,
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn xcm_mixed_ingress_single_deposit_triggers_single_cycle() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [9u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
    assert_eq!(instance.identity.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

// --- Actors Platform: Scheduling & Budget ---

#[test]
fn mandatory_base_pass_executes_independently_of_later_idle_budget() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::set_block_number(2);
    run_idle(Weight::zero());
    let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
    assert_eq!(instance.identity.cycle_nonce, 1);
    assert!(!instance.hot.pending_signal);
  });
}

#[test]
fn underfunded_manual_trigger_preserves_runtime_user_process() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    let step = make_step(heavy_task.clone());
    let steps = BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("steps fits");
    let fee_envelope = Actors::attempt_fee_envelope(ActorType::User, &steps, 0)
      .expect("runtime plan has a checked fee envelope");
    let min_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    assert!(
      fee_envelope.total < min_balance,
      "reference Weight-derived fee should remain below MinUserBalance"
    );
    let prefunded = user_prefunding_requirement(&steps);
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, min_balance.saturating_sub(1));
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Runtime>::InsufficientFee
    );
    let state = Actors::active_actor_state(actor_id).expect("process remains live");
    assert!(!state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn scheduler_fifo_order_is_deterministic_across_actor_types() {
  let cases = [(2u32, 2u32), (3u32, 3u32), (4u32, 2u32)];
  for (system_count, user_count) in cases {
    let run_case = || -> (alloc::vec::Vec<ActorId>, alloc::vec::Vec<ActorId>) {
      seeded_test_ext().execute_with(|| {
        System::set_block_number(1);
        let schedule = Schedule {
          trigger: Trigger::cadenced(1),
          cooldown_blocks: 0,
        };
        let steps = BoundedVec::try_from(vec![make_step(inert_task())]).expect("steps fits");
        let mut tracked: alloc::vec::Vec<ActorId> = alloc::vec::Vec::new();
        for _ in 0..system_count {
          tracked.push(create_system(ALICE, schedule.clone(), None, steps.clone()));
        }
        for _ in 0..user_count {
          let user_id = create_user(ALICE, schedule.clone(), None, steps.clone());
          fund_native(user_id, 100_000_000_000);
          tracked.push(user_id);
        }
        System::set_block_number(2);
        run_idle(Weight::MAX);
        System::set_block_number(3);
        run_idle(Weight::MAX);
        System::set_block_number(4);
        run_idle(Weight::MAX);
        let mut actual: Vec<_> = actor_events()
          .into_iter()
          .filter_map(|event| match event {
            Event::CycleStarted { actor_id, .. } if tracked.contains(&actor_id) => Some(actor_id),
            _ => None,
          })
          .collect();
        actual.dedup();
        (tracked, actual)
      })
    };
    let first = run_case();
    let second = run_case();
    assert_eq!(first.1, first.0, "scheduler must preserve FIFO order");
    assert_eq!(
      first, second,
      "FIFO order must be deterministic for system_count={}, user_count={}",
      system_count, user_count
    );
  }
}

#[test]
fn mandatory_base_and_drain_preserve_strict_heavy_head_order() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // Heavy head first: three transfer steps make its cycle envelope strictly larger than the
    // single-step followers behind it, while the constrained remainder admits the head's probes
    // and consume but not its full cycle admission.
    let step = |amount| {
      make_step(Task::Transfer {
        to: BOB,
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(amount),
      })
    };
    let heavy_plan =
      BoundedVec::try_from(vec![step(1), step(2), step(3)]).expect("plan fits runtime bound");
    let head = create_system(ALICE, manual_schedule(), None, heavy_plan);
    fund_native(head, 1_000_000_000_000_000);
    let light_a = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    fund_native(light_a, 1_000_000_000_000_000);
    let light_b = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    fund_native(light_b, 1_000_000_000_000_000);
    for actor_id in [head, light_a, light_b] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    let tickets: Vec<_> = [head, light_a, light_b]
      .into_iter()
      .map(|id| {
        Actors::actor_hot(id)
          .and_then(|hot| hot.queue_ticket)
          .expect("triggered actor is queued")
      })
      .collect();
    assert_eq!(
      tickets,
      vec![0, 1, 2],
      "physical FIFO order is head, light A, light B"
    );

    System::set_block_number(2);
    System::reset_events();
    run_idle(starvation_blocked_budget(head));

    let mut started: Vec<_> = System::events()
      .iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started.first(), Some(&head));

    System::set_block_number(3);
    System::reset_events();
    run_idle(Weight::MAX);
    started.extend(
      System::events()
        .into_iter()
        .filter_map(|record| match record.event {
          RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
          _ => None,
        }),
    );
    started.dedup();
    assert_eq!(started, vec![head, light_a, light_b]);
    for id in [head, light_a, light_b] {
      assert_eq!(
        Actors::active_actor_state(id)
          .expect("actor executed")
          .identity
          .cycle_nonce,
        1
      );
    }
  });
}

#[test]
fn exact_input_task_uses_measured_caller_aware_router_weight() {
  seeded_test_ext().execute_with(|| {
    let task = Task::SwapIn {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let actor_upper = Actors::weight_upper_bound(&task);
    let measured =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_in();
    assert_eq!(actor_upper, measured);
  });
}

#[test]
fn exact_output_task_uses_generated_native_router_weight() {
  seeded_test_ext().execute_with(|| {
    let exact_in = Task::SwapIn {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let exact_out = Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(1),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(10),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let exact_out_upper = Actors::weight_upper_bound(&exact_out);
    let measured =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_out();
    assert_eq!(exact_out_upper, measured);
    assert_eq!(
      Actors::weight_upper_bound(&exact_in),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_in()
    );
  });
}

#[test]
fn staking_tasks_use_separate_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let stake = Task::Stake {
      asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    let unstake = Task::Unstake {
      asset: AssetKind::Local(ASSET_A),
      shares: AmountResolution::Fixed(1),
    };
    assert_eq!(
      Actors::weight_upper_bound(&stake),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_stake()
    );
    assert_eq!(
      Actors::weight_upper_bound(&unstake),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_unstake()
    );
    assert!(
      Actors::weight_upper_bound(&unstake).ref_time()
        > Actors::weight_upper_bound(&stake).ref_time()
    );
  });
}

#[test]
fn liquidity_tasks_use_separate_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let add = Task::AddLiquidity {
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      amount_a: AmountResolution::Fixed(1),
      amount_b: AmountResolution::Fixed(1),
      min_lp_out: 1,
    };
    let donation = Task::DonateLiquidity {
      asset_a: AssetKind::Local(0),
      asset_b: AssetKind::Local(ASSET_A),
      max_amount_a: AmountResolution::Fixed(1),
      max_ratio_error: Perbill::zero(),
    };
    let remove = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    assert_eq!(
      Actors::weight_upper_bound(&add),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_add_liquidity()
    );
    assert_eq!(
      Actors::weight_upper_bound(&donation),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_donate_liquidity()
    );
    assert_eq!(
      Actors::weight_upper_bound(&remove),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_remove_liquidity()
    );
    assert_ne!(
      Actors::weight_upper_bound(&remove),
      Actors::weight_upper_bound(&add)
    );
  });
}

#[test]
fn wakeup_registration_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let expected =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_append_new_page()
        .saturating_add(
          <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_insert(),
        )
        .saturating_add(
          <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_remove_exact(),
        );
    assert_eq!(Actors::wakeup_registration_weight_upper(), expected);
  });
}

#[test]
fn scheduler_actor_probe_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let state =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_state_probe();
    assert_eq!(Actors::scheduler_actor_state_probe_weight_upper(), state);
    assert_eq!(Actors::scheduler_actor_probe_weight_upper(), state);
  });
}

#[test]
fn scheduler_paged_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let scan = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_tombstone_drain(1);
    let consume = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_delete_page();
    let state = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_state_probe();
    assert!(scan.ref_time() > 0 && scan.proof_size() > 0);
    assert!(consume.ref_time() > 0 && consume.proof_size() > 0);
    assert_eq!(Actors::scheduler_actor_state_probe_weight_upper(), state);
  });
}

#[test]
fn wakeup_drain_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let close =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::close_actor();
    let fault =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::record_wakeup_worker_fault();
    let at_time =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::at_time_trigger_occurrence();
    let cadence =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::cadenced_trigger_occurrence();
    let temporal = Weight::from_parts(
      at_time.ref_time().max(cadence.ref_time()),
      at_time.proof_size().max(cadence.proof_size()),
    );
    assert_eq!(
      Actors::wakeup_cursor_drain_unit_weight_upper(false),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_partial()
        .saturating_add(temporal)
        .saturating_add(close)
        .saturating_add(fault)
    );
    assert_eq!(
      Actors::wakeup_cursor_drain_unit_weight_upper(true),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_remove()
        .saturating_add(temporal)
        .saturating_add(close)
        .saturating_add(fault)
    );
  });
}

#[test]
fn transaction_extension_ingress_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: polkadot_sdk::sp_runtime::MultiAddress::Id(BOB),
      value: 1,
    });
    let measured_notify = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_notify();
    let base = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_base();
    let notify = Weight::from_parts(
      base.ref_time().max(measured_notify.ref_time()),
      base.proof_size().max(measured_notify.proof_size()),
    );
    assert_eq!(AddressEventIngressExtension.weight(&call), notify);
    assert!(base.all_lte(notify));
    assert!(base.proof_size() > 0);
    let unmatched_refund = AddressEventIngressExtension::post_dispatch_refund(false, false);
    assert_eq!(notify.saturating_sub(unmatched_refund), base);
    assert_eq!(
      AddressEventIngressExtension::post_dispatch_refund(false, true),
      Weight::zero()
    );
    assert_eq!(
      AddressEventIngressExtension::post_dispatch_refund(true, false),
      notify
    );
  });
}

#[test]
fn certified_ingress_inventory_is_closed_and_typed() {
  seeded_test_ext().execute_with(|| {
    let inventory = RuntimeAddressEventIngress::certified_producer_inventory();
    assert!(
      !inventory.is_empty(),
      "inventory must name every producer path"
    );
    let mut ids = alloc::vec::Vec::new();
    for producer in inventory {
      assert!(!producer.id.is_empty());
      assert!(!producer.credited_surface.is_empty());
      assert!(!producer.source_provenance.is_empty());
      assert!(!producer.preflight_owner.is_empty());
      assert!(!producer.consequence_owner.is_empty());
      assert!(!producer.rollback_owner.is_empty());
      assert!(matches!(
        producer.protocol,
        crate::configs::address_event_ingress::CertifiedMovementProtocol::PostMovementNotify
          | crate::configs::address_event_ingress::CertifiedMovementProtocol::BlockAtomicPostDispatch
          | crate::configs::address_event_ingress::CertifiedMovementProtocol::XcmTransactionalPrecommit
      ));
      assert!(!producer.weight_owner.is_empty());
      ids.push(producer.id);
    }
    let unique = ids
      .iter()
      .collect::<alloc::collections::BTreeSet<_>>()
      .len();
    assert_eq!(ids.len(), unique, "producer ids must be unique");
    // The runtime adapter implements the typed boundary: absent destinations are
    // balance-only no-ops for both preflight and notify.
    let event = pallet_deos_actors::AddressEvent {
      destination: BOB,
      source: Some(ALICE),
      asset: AssetKind::Native,
      amount: 1,
      provenance: Some(pallet_deos_actors::FundingProvenance::Signed),
    };
    assert_ok!(
      <RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
        AccountId,
        AssetKind,
        Balance,
      >>::preflight(&event)
    );
    assert_ok!(
      <RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
        AccountId,
        AssetKind,
        Balance,
      >>::notify(&event)
    );
  });
}

#[test]
fn certified_extension_scheduler_exhaustion_closes_actor_without_rejecting_value() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer_pair = sr25519::Pair::from_seed(&[53u8; 32]);
    let signer = crate::AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer,
      1_000_000_000_000_000_000,
    );
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    // Monotonic ticket namespace at the ceiling closes the destination Actor
    // through the unified sink without rejecting the certified movement.
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let sovereign_before = native_balance(&sovereign);
    let signer_before = native_balance(&signer);
    let transfer_amount = 25_000_000_000_000u128;
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: transfer_amount,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer_pair, 0, call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      native_balance(&sovereign),
      sovereign_before.saturating_add(transfer_amount),
      "certified value movement survives scheduler-exhaustion closure"
    );
    assert!(native_balance(&signer) < signer_before);
    assert!(Actors::active_actor_state(actor_id).is_none());
  });
}

#[test]
fn asset_ops_transfer_preserves_value_while_closing_scheduler_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&ALICE, 100_000_000_000_000);
    // Monotonic ticket exhaustion closes the destination Actor while preserving
    // the already-certified movement (spec 6.1).
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let actor_before = native_balance(&sovereign);
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &sovereign,
      AssetKind::Native,
      5_000,
    ));
    assert_eq!(
      native_balance(&sovereign),
      actor_before.saturating_add(5_000),
      "certified transfer survives terminal scheduler closure"
    );
    assert!(Actors::active_actor_state(actor_id).is_none());
    // An absent sovereign destination is balance-only: the same transfer succeeds
    // and performs no Actors work.
    let bob_before = native_balance(&BOB);
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &BOB,
      AssetKind::Native,
      3_000,
    ));
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(3_000));
  });
}

#[test]
fn asset_ops_source_less_mint_closes_scheduler_exhaustion_after_movement() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let before = native_balance(&sovereign);
    assert_ok!(TmctolAssetOps::mint(&sovereign, AssetKind::Native, 5_000));
    assert_eq!(native_balance(&sovereign), before.saturating_add(5_000));
    assert!(Actors::active_actor_state(actor_id).is_none());
  });
}

#[test]
fn asset_ops_native_mint_ledger_failure_precedes_notification() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      polkadot_sdk::sp_runtime::MultiAddress::Id(sovereign.clone()),
      u128::MAX,
    ));
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let failure = TmctolAssetOps::mint(&sovereign, AssetKind::Native, 1)
      .expect_err("exact native mint must reject ledger overflow");
    assert_eq!(failure.retry, RetryClass::Permanent);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "failed value movement cannot leave a certified notification"
    );
  });
}

#[test]
fn signed_balance_deposit_credits_rejected_donor_but_only_owner_activates_funding() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[45u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[46u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    for account in [&owner, &donor] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(owner.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let donor_amount = 9_000_000_000_000;
    let donor_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: donor_amount,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, donor_call)),
      Ok(Ok(_))
    ));
    let dust_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: 1,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 1, dust_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      native_balance(&sovereign),
      sovereign_before
        .saturating_add(donor_amount)
        .saturating_add(1)
    );
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    let owner_amount = 11_000_000_000_000;
    let owner_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: owner_amount,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, owner_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&owner_amount)
    );
  });
}

#[test]
fn signed_asset_deposit_keeps_rejected_donor_balance_only_and_owner_authoritative() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[47u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[48u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    for account in [&owner, &donor] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let asset_id = 4_242u32;
    assert_ok!(create_test_asset(asset_id, &owner));
    assert_ok!(mint_tokens(asset_id, &owner, &owner, 100_000));
    assert_ok!(mint_tokens(asset_id, &owner, &donor, 100_000));
    let tracked_asset = AssetKind::Local(asset_id);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: tracked_asset,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(owner.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let donor_amount = 9_000;
    let donor_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
      id: asset_id,
      target: Address::Id(sovereign.clone()),
      amount: donor_amount,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, donor_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, sovereign.clone()), donor_amount);
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    let owner_amount = 11_000;
    let owner_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
      id: asset_id,
      target: Address::Id(sovereign.clone()),
      amount: owner_amount,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, owner_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      Assets::balance(asset_id, sovereign),
      donor_amount.saturating_add(owner_amount)
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&tracked_asset),
      Some(&owner_amount)
    );
  });
}

#[test]
fn dynamic_asset_producers_notify_directly_with_balance_only_provenance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[50u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[51u8; 32]);
    let delegate_pair = sr25519::Pair::from_seed(&[52u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    let delegate = crate::AccountId::from(delegate_pair.public());
    for account in [&owner, &donor, &delegate] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let asset_id = 4_243u32;
    assert_ok!(create_test_asset(asset_id, &owner));
    assert_ok!(mint_tokens(asset_id, &owner, &donor, 100_000));
    let tracked_asset = AssetKind::Local(asset_id);
    let make_actor = || {
      create_user(
        owner.clone(),
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        None,
        BoundedVec::try_from(vec![make_step(Task::Transfer {
          to: BOB,
          asset: tracked_asset,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
        })])
        .expect("execution plan fits"),
      )
    };

    let mint_actor = make_actor();
    let mint_sovereign = actor_account(mint_actor);
    let mint_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::mint {
      id: asset_id,
      beneficiary: Address::Id(mint_sovereign.clone()),
      amount: 7_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, mint_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, mint_sovereign), 7_000);
    assert!(
      Actors::actor_hot(mint_actor)
        .expect("mint actor")
        .pending_signal
    );
    assert!(actor_funding(mint_actor).funding_accumulated.is_empty());

    let force_actor = make_actor();
    let force_sovereign = actor_account(force_actor);
    let force_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::force_transfer {
      id: asset_id,
      source: Address::Id(donor.clone()),
      dest: Address::Id(force_sovereign.clone()),
      amount: 8_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 1, force_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, force_sovereign), 8_000);
    assert!(
      Actors::actor_hot(force_actor)
        .expect("force actor")
        .pending_signal
    );
    assert!(actor_funding(force_actor).funding_accumulated.is_empty());

    let approved_actor = make_actor();
    let approved_sovereign = actor_account(approved_actor);
    let approve_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::approve_transfer {
      id: asset_id,
      delegate: Address::Id(delegate.clone()),
      amount: 9_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, approve_call)),
      Ok(Ok(_))
    ));
    let approved_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer_approved {
      id: asset_id,
      owner: Address::Id(donor),
      destination: Address::Id(approved_sovereign.clone()),
      amount: 9_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&delegate_pair, 0, approved_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, approved_sovereign), 9_000);
    assert!(
      Actors::actor_hot(approved_actor)
        .expect("approved actor")
        .pending_signal
    );
    assert!(actor_funding(approved_actor).funding_accumulated.is_empty());
  });
}

#[test]
fn signed_fixed_transfer_is_rejected_before_dispatch_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[43u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(signer_account.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: 1,
    });
    assert!(Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call)).is_err());
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn signed_transfer_all_is_rejected_before_dispatch_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[44u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000,
    );
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(signer_account.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_all {
      dest: Address::Id(sovereign.clone()),
      keep_alive: true,
    });
    assert!(Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call)).is_err());
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn signed_transfer_all_records_actual_post_fee_movement_without_event_scan() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_signer = sr25519::Pair::from_seed(&[49u8; 32]);
    let owner = crate::AccountId::from(owner_signer.public());
    let donor_signer = sr25519::Pair::from_seed(&[50u8; 32]);
    let donor = crate::AccountId::from(donor_signer.public());
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&owner, 1_000_000_000_000_000_000);
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&donor, 1_000_000_000_000_000_000);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(owner.clone(), manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial(
      RuntimeOrigin::signed(owner),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let sovereign = actor_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_all {
      dest: Address::Id(sovereign.clone()),
      keep_alive: true,
    });

    let result = Executive::apply_extrinsic(signed_extrinsic(&donor_signer, 0, call));
    assert!(
      matches!(result, Ok(Ok(_))),
      "transfer_all result: {result:?}"
    );
    let actual = native_balance(&sovereign).saturating_sub(sovereign_before);
    assert!(actual > 0);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&actual)
    );
  });
}

#[test]
fn executive_pipeline_covers_transaction_extension_ingress_and_refunds() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[42u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    let actor_id = create_user(
      signer_account.clone(),
      Schedule {
        trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
        cooldown_blocks: 0,
      },
      None,
      BoundedVec::try_from(vec![make_step(Task::Transfer {
        to: BOB,
        asset: AssetKind::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })])
      .expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    let notify_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_notify();
    let base_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_base();
    let transfer_amount = 10_000_000_000_000;
    let matched = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: transfer_amount,
    });
    let balance_before_matched = native_balance(&signer_account);
    let matched_result = Executive::apply_extrinsic(signed_extrinsic(&signer, 0, matched));
    assert!(matches!(matched_result, Ok(Ok(_))), "{matched_result:?}");
    let matched_fee = balance_before_matched
      .saturating_sub(native_balance(&signer_account))
      .saturating_sub(transfer_amount);
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&transfer_amount)
    );
    let unmatched = RuntimeCall::Balances(
      polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(BOB),
        value: transfer_amount,
      },
    );
    let balance_before_unmatched = native_balance(&signer_account);
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer, 1, unmatched)),
      Ok(Ok(_))
    ));
    let unmatched_fee = balance_before_unmatched
      .saturating_sub(native_balance(&signer_account))
      .saturating_sub(transfer_amount);
    assert!(
      unmatched_fee < matched_fee,
      "successful tracked calls without an Actors recipient must refund the unused notification envelope"
    );
    assert!(notify_weight.saturating_sub(base_weight) != Weight::zero());
    assert!(Actors::pending_signal(actor_id));
    let untracked = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
      remark: b"untracked ingress call".to_vec(),
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer, 2, untracked)),
      Ok(Ok(_))
    ));
    assert!(Actors::pending_signal(actor_id));
    let failed_value = native_balance(&signer_account).saturating_add(1);
    let failed = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign),
      value: failed_value,
    });
    let failed_extrinsic = signed_extrinsic(&signer, 3, failed);
    let declared_failed_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Runtime>::compute_fee(
      failed_extrinsic.encoded_size() as u32,
      &failed_extrinsic.get_dispatch_info(),
      0,
    );
    let balance_before_failed = native_balance(&signer_account);
    assert!(matches!(
      Executive::apply_extrinsic(failed_extrinsic),
      Ok(Err(_))
    ));
    let failed_fee = balance_before_failed.saturating_sub(native_balance(&signer_account));
    assert!(Actors::pending_signal(actor_id));
    assert!(
      failed_fee < declared_failed_fee,
      "failed tracked calls must pay less than their declared envelope after post-dispatch refund"
    );
  });
}

#[test]
fn split_transfer_task_uses_the_single_runtime_weight_authority() {
  seeded_test_ext().execute_with(|| {
    let max_legs = <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get();
    let legs = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let task = Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs: SplitTransferLegsOf::<Runtime>::try_from(legs).expect("maximum legs fit"),
    };
    assert_eq!(
      Actors::weight_upper_bound(&task),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as pallet_deos_actors::WeightInfo>::task_split_transfer(
        max_legs,
      )
    );
  });
}

#[test]
fn maximum_single_task_attempt_and_cleanup_fit_derived_service_envelope() {
  seeded_test_ext().execute_with(|| {
    let max_legs =
      <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get();
    let legs = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let task = Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs: SplitTransferLegsOf::<Runtime>::try_from(legs).expect("maximum legs fit"),
    };
    let plan: ContractSteps<Runtime> =
      BoundedVec::try_from(vec![make_step(task)]).expect("one maximum task fits");
    let service = Actors::guaranteed_actor_service_weight().expect("runtime envelope is valid");

    let maximum_attempt = Actors::contract_steps_admission_weight_upper(ActorType::System, &plan);
    assert!(
      maximum_attempt.all_lte(service),
      "maximum_attempt={maximum_attempt:?}, service={service:?}"
    );
    assert!(Actors::close_dispatch_weight_upper().all_lte(service));
  });
}

#[test]
fn maximum_user_creation_and_replacement_pass_real_transaction_extensions() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer_pair = sr25519::Pair::from_seed(&[55u8; 32]);
    let signer = AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&signer, 1_000_000_000_000_000_000);
    let steps = transfer_contract_steps(BOB, AssetKind::Native, 1);
    prefund_active_user_creation(&signer, &steps);
    let actor_id = Actors::next_actor_id();
    let create = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
      mutability: Mutability::Mutable,
      contract: user_active_contract(manual_schedule(), None, steps.clone()),
    });
    let create_info = create.get_dispatch_info();
    let expected_create = <Runtime as pallet_deos_actors::Config>::WeightInfo::create_user_actor()
      .max(
        <Runtime as pallet_deos_actors::Config>::WeightInfo::create_user_actor_crossing_new_page(),
      );
    assert_eq!(create_info.call_weight, expected_create);
    let create_extrinsic = signed_extrinsic(&signer_pair, 0, create);
    assert_ok!(Executive::validate_transaction(
      TransactionSource::External,
      create_extrinsic.clone(),
      System::block_hash(0),
    ));
    ensure_current_resource_state();
    assert_ok!(Executive::apply_extrinsic(create_extrinsic));
    assert!(Actors::active_actor_state(actor_id).is_some());

    age_fixture_control_clock(actor_id);
    let replacement = RuntimeCall::Actors(pallet_deos_actors::Call::update_contract {
      actor_id,
      contract: user_active_contract(
        RuntimeSchedule {
          trigger: Trigger::manual(),
          cooldown_blocks: 1,
        },
        None,
        steps,
      )
      .expect("active replacement contract"),
    });
    let replacement_info = replacement.get_dispatch_info();
    assert_eq!(
      replacement_info.call_weight,
      <Runtime as pallet_deos_actors::Config>::WeightInfo::update_contract()
        .saturating_add(Actors::close_dispatch_weight_upper())
    );
    let replacement_extrinsic = signed_extrinsic(&signer_pair, 1, replacement);
    assert_ok!(Executive::validate_transaction(
      TransactionSource::External,
      replacement_extrinsic.clone(),
      System::block_hash(0),
    ));
    ensure_current_resource_state();
    assert_ok!(Executive::apply_extrinsic(replacement_extrinsic));
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("updated actor remains active")
        .contract
        .cooldown_blocks,
      1
    );
  });
}

#[test]
fn actor_resource_runtime_api_backing_projects_budget_current_state_and_finalized_telemetry() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    ensure_actor_prepass_context();
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));

    let budget = crate::configs::BlockResourceBudgetValue::get();
    assert_eq!(budget, crate::configs::BlockResourceBudgetValue::get());
    let current = Actors::block_resource_state().expect("current resource state projects");
    assert_eq!(current.block_number(), 1);
    assert_eq!(
      current.phase(),
      pallet_deos_actors::BlockResourcePhase::ExternalPhase
    );

    Actors::on_idle(1, Weight::MAX);
    let finalized =
      Actors::finalized_block_resource_telemetry().expect("finalized telemetry projects");
    assert_eq!(finalized.block_number(), 1);
  });
}

#[test]
fn production_full_block_resource_harness_records_actor_and_user_contention() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    System::set_block_number(2);
    ensure_actor_prepass_context();
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));

    let signer_pair = sr25519::Pair::from_seed(&[57u8; 32]);
    let signer = AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&signer, 1_000_000_000_000_000);
    let user_call = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
      remark: vec![1, 2, 3],
    });
    let user_extrinsic = signed_extrinsic(&signer_pair, 0, user_call);
    assert_ok!(Executive::apply_extrinsic(user_extrinsic));

    let current = Actors::block_resource_state().expect("resource state remains authoritative");
    let usage = current.usage();
    let limits = crate::configs::BlockResourceBudgetValue::get().limits();
    assert_ne!(usage.actor_effect_used(), Weight::zero());
    assert_ne!(usage.user_dispatch_used(), Weight::zero());
    assert!(usage.actor_effect_used().all_lte(limits.actor_base_turn()));
    assert!(
      usage
        .shared_used()
        .expect("shared usage reconciles")
        .all_lte(limits.shared_economic())
    );

    Actors::on_idle(2, Weight::MAX);
    let finalized = Actors::finalized_block_resource_telemetry()
      .expect("mixed Actor/user block finalizes telemetry");
    let finalized_usage = finalized.usage();
    assert_eq!(
      finalized_usage.actor_effect_used(),
      usage.actor_effect_used()
    );
    assert_eq!(
      finalized_usage.user_dispatch_used(),
      usage.user_dispatch_used()
    );
    assert!(
      usage
        .actor_control_used()
        .all_lte(finalized_usage.actor_control_used())
    );
  });
}

#[test]
fn actor_prepass_requires_timestamp_and_parachain_context_before_mutation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::kill();
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::kill();
    assert_noop!(
      Actors::actor_prepass(RuntimeOrigin::none()),
      pallet_deos_actors::Error::<Runtime>::PrepassContextIncomplete
    );
    assert!(Actors::block_resource_state().is_none());

    set_consensus_timestamp(1);
    assert_noop!(
      Actors::actor_prepass(RuntimeOrigin::none()),
      pallet_deos_actors::Error::<Runtime>::PrepassContextIncomplete
    );
    assert!(Actors::block_resource_state().is_none());

    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData::default(),
    );
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    assert_eq!(
      Actors::block_resource_state().map(|state| state.phase()),
      Some(pallet_deos_actors::BlockResourcePhase::ExternalPhase)
    );
  });
}

#[test]
fn on_initialize_freezes_cutoff_and_executes_mandatory_base_work() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::set_block_number(2);
    assert_eq!(Actors::on_initialize(2), Weight::zero());
    ensure_actor_prepass_context();
    let consumed = Actors::actor_prepass(RuntimeOrigin::none())
      .expect("mandatory prepass succeeds")
      .actual_weight
      .expect("mandatory prepass reports actual Weight");
    assert!(
      <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as WeightInfo>::scheduler_on_initialize_cutoff()
        .all_lte(consumed)
    );
    assert_eq!(
      Actors::prepass_execution_cutoff(),
      Some((2, Actors::next_queue_ticket()))
    );
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn resource_meter_extension_uses_generated_production_owner() {
  use polkadot_sdk::sp_runtime::traits::TransactionExtension;

  let extension = crate::configs::resource_meter::BlockResourceMeterExtension;
  let call = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark { remark: Vec::new() });
  assert_eq!(
    extension.weight(&call),
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::block_resource_meter_extension()
  );
}

#[test]
fn actor_control_limit_admits_cutoff_and_one_rotating_materialization_family() {
  let family_minimum = Actors::materialization_family_minimum(0)
    .max(Actors::materialization_family_minimum(1))
    .max(Actors::materialization_family_minimum(2));
  let minimum_control = ActorControlInitializationWeight::get()
    .saturating_add(<Runtime as pallet_deos_actors::Config>::WeightInfo::scheduler_on_idle_base())
    .saturating_add(
      <Runtime as pallet_deos_actors::Config>::WeightInfo::materialization_coordinator_base(),
    )
    .saturating_add(family_minimum);
  assert!(
    minimum_control.all_lte(BlockResourceBudgetValue::get().limits().actor_control()),
    "minimum Actor Control {minimum_control:?} rotating families={:?}/{:?}/{:?} must fit {:?}",
    Actors::materialization_family_minimum(0),
    Actors::materialization_family_minimum(1),
    Actors::materialization_family_minimum(2),
    BlockResourceBudgetValue::get().limits().actor_control()
  );
}

#[test]
fn actor_cutoff_capture_is_owned_by_actor_control() {
  assert_eq!(
    ActorControlInitializationWeight::get(),
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::scheduler_on_initialize_cutoff()
  );
  assert_ne!(ActorControlInitializationWeight::get(), Weight::zero());
}

#[test]
fn session_rotation_hook_uses_the_generated_bounded_owner() {
  seeded_test_ext().execute_with(|| {
    let period = crate::configs::Period::get();
    System::set_block_number(period);
    assert_eq!(Session::on_initialize(period), Weight::zero());
    let generated = <crate::weights::pallet_session_rotation::SubstrateWeight<Runtime> as pallet_session_rotation::WeightInfo>::rotate_session();
    assert_eq!(SessionRotation::on_initialize(period), generated);
    assert!(generated.all_lte(crate::configs::RuntimeBlockWeights::get().max_block));
  });
}

#[test]
fn message_queue_service_is_fixed_initialize_work_not_external_idle_work() {
  assert!(MessageQueue::index() < Actors::index());
  assert_eq!(
    <<Runtime as polkadot_sdk::pallet_message_queue::Config>::ServiceWeight as Get<
      Option<Weight>,
    >>::get(),
    Some(MessageQueueServiceWeight::get())
  );
  assert_eq!(
    <<Runtime as polkadot_sdk::pallet_message_queue::Config>::IdleMaxServiceWeight as Get<
      Option<Weight>,
    >>::get(),
    None
  );
  assert!(XcmpQueue::index() < Actors::index());
  assert_ne!(XcmpQueue::on_idle_weight(), Weight::zero());
}

#[test]
fn shared_parachain_inherent_exposes_complete_context_geometry() {
  use polkadot_sdk::cumulus_pallet_parachain_system::parachain_inherent::{
    InboundDownwardMessages, InboundHrmpMessages, InboundMessagesData,
  };

  let downward = InboundDownwardMessages::new(vec![
    InboundDownwardMessage {
      sent_at: 1,
      msg: vec![1],
    },
    InboundDownwardMessage {
      sent_at: 2,
      msg: vec![2],
    },
  ]);
  let mut downward_full_bytes = 1usize;
  let downward = downward.into_abridged(&mut downward_full_bytes);

  let mut horizontal = BTreeMap::new();
  horizontal.insert(
    ParaId::from(2_001u32),
    vec![InboundHrmpMessage {
      sent_at: 1,
      data: vec![1],
    }],
  );
  horizontal.insert(
    ParaId::from(2_002u32),
    vec![InboundHrmpMessage {
      sent_at: 2,
      data: vec![2],
    }],
  );
  let mut horizontal_full_bytes = 1usize;
  let horizontal =
    InboundHrmpMessages::from_map(horizontal).into_abridged(&mut horizontal_full_bytes);
  let data = InboundMessagesData::new(downward, horizontal);

  let (downward_full, downward_hashed) = data.downward_messages.messages();
  let (horizontal_full, horizontal_hashed) = data.horizontal_messages.messages();
  let channels = horizontal_full
    .iter()
    .map(|(sender, _)| *sender)
    .chain(horizontal_hashed.iter().map(|(sender, _)| *sender))
    .collect::<BTreeSet<_>>();
  let geometry = ContextMessageGeometry::new(
    u32::try_from(downward_full.len()).unwrap_or(u32::MAX),
    u32::try_from(downward_hashed.len()).unwrap_or(u32::MAX),
    u32::try_from(horizontal_full.len()).unwrap_or(u32::MAX),
    u32::try_from(horizontal_hashed.len()).unwrap_or(u32::MAX),
    u32::try_from(channels.len()).unwrap_or(u32::MAX),
  );
  let limits = ContextMessageLimits::new(2, 2, 2);
  assert_eq!(limits.validate(geometry), Ok(()));
  assert_eq!(
    crate::apis::validate_inbound_messages_geometry(&data),
    Ok(())
  );
}

#[test]
fn runtime_context_validator_accepts_exact_dmp_and_rejects_full_mixed_and_channel_overflow() {
  use polkadot_sdk::cumulus_pallet_parachain_system::parachain_inherent::{
    InboundDownwardMessages, InboundHrmpMessages, InboundMessagesData,
  };

  let make_downward = |count: u32, size_limit: &mut usize| {
    InboundDownwardMessages::new(
      (0..count)
        .map(|sent_at| InboundDownwardMessage {
          sent_at,
          msg: vec![1],
        })
        .collect(),
    )
    .into_abridged(size_limit)
  };
  let empty_horizontal = || {
    let mut bytes = usize::MAX;
    InboundHrmpMessages::from_map(BTreeMap::new()).into_abridged(&mut bytes)
  };
  let mut exact_bytes = usize::MAX;
  let exact = InboundMessagesData::new(make_downward(512, &mut exact_bytes), empty_horizontal());
  assert_eq!(
    crate::apis::validate_inbound_messages_geometry(&exact),
    Ok(())
  );

  let mut full_bytes = usize::MAX;
  let full_over = InboundMessagesData::new(make_downward(513, &mut full_bytes), empty_horizontal());
  assert_eq!(
    crate::apis::validate_inbound_messages_geometry(&full_over),
    Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded)
  );

  let mut mixed_bytes = 1usize;
  let mixed_over =
    InboundMessagesData::new(make_downward(513, &mut mixed_bytes), empty_horizontal());
  assert_eq!(
    crate::apis::validate_inbound_messages_geometry(&mixed_over),
    Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded)
  );

  let mut channels = BTreeMap::new();
  for sender in 1..=129u32 {
    channels.insert(
      ParaId::from(sender),
      vec![InboundHrmpMessage {
        sent_at: sender,
        data: vec![1],
      }],
    );
  }
  let mut channel_bytes = usize::MAX;
  let mut downward_bytes = usize::MAX;
  let channel_over = InboundMessagesData::new(
    InboundDownwardMessages::new(Vec::new()).into_abridged(&mut downward_bytes),
    InboundHrmpMessages::from_map(channels).into_abridged(&mut channel_bytes),
  );
  assert_eq!(
    crate::apis::validate_inbound_messages_geometry(&channel_over),
    Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded)
  );
}

#[test]
fn context_provider_and_checker_agree_at_and_above_the_dmp_bound() {
  use cumulus_primitives_parachain_inherent::{
    INHERENT_IDENTIFIER, MessageQueueChain, ParachainInherentData,
  };
  use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
  use polkadot_sdk::{
    cumulus_primitives_core::PersistedValidationData, sp_inherents::InherentData,
  };

  seeded_test_ext().execute_with(|| {
    for (count, expected) in [
      (512, Ok(())),
      (
        513,
        Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded),
      ),
    ] {
      let downward_messages = (0..count)
        .map(|_| InboundDownwardMessage {
          sent_at: 1,
          msg: vec![1],
        })
        .collect::<Vec<_>>();
      let mut queue = MessageQueueChain::default();
      for message in &downward_messages {
        queue.extend_downward(message);
      }
      let mut proof_builder = RelayStateSproofBuilder::default();
      proof_builder.dmq_mqc_head = Some(queue.head());
      let (relay_parent_storage_root, relay_chain_state) =
        proof_builder.into_state_root_and_proof();
      let mut data = InherentData::new();
      data
        .put_data(
          INHERENT_IDENTIFIER,
          &ParachainInherentData {
            validation_data: PersistedValidationData {
              relay_parent_number: 1,
              relay_parent_storage_root,
              max_pov_size: 5_000_000,
              ..Default::default()
            },
            relay_chain_state,
            downward_messages,
            horizontal_messages: Default::default(),
            relay_parent_descendants: Default::default(),
            collator_peer_id: None,
          },
        )
        .expect("bounded relay-proof fixture encodes"); // deos-bypass: panic-owner — fixed 512/513-message fixtures and upstream proof builder own deterministic encoding.
      assert_eq!(
        crate::apis::validate_context_inherent_geometry(&data),
        expected
      );
    }
  });
}

#[test]
fn context_provider_and_checker_agree_at_and_above_the_hrmp_channel_bound() {
  use cumulus_primitives_parachain_inherent::{
    INHERENT_IDENTIFIER, MessageQueueChain, ParachainInherentData,
  };
  use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
  use polkadot_sdk::{
    cumulus_primitives_core::PersistedValidationData, sp_inherents::InherentData,
  };

  seeded_test_ext().execute_with(|| {
    for (count, max_pov_size, expected) in [
      (128, 5_000_000, Ok(())),
      (
        129,
        5_000_000,
        Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded),
      ),
      (128, 1, Ok(())),
      (
        129,
        1,
        Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded),
      ),
    ] {
      let mut horizontal_messages = BTreeMap::new();
      let mut proof_builder = RelayStateSproofBuilder::default();
      for raw_sender in 1..=count {
        let sender = ParaId::from(raw_sender);
        let message = InboundHrmpMessage {
          sent_at: 1,
          data: vec![1],
        };
        horizontal_messages.insert(sender, vec![message.clone()]);
        let channel = proof_builder.upsert_inbound_channel(sender);
        channel.max_message_size = 1024;
        channel.mqc_head = Some(MessageQueueChain::default().extend_hrmp(&message).head());
      }
      let (relay_parent_storage_root, relay_chain_state) =
        proof_builder.into_state_root_and_proof();
      let mut data = InherentData::new();
      data
        .put_data(
          INHERENT_IDENTIFIER,
          &ParachainInherentData {
            validation_data: PersistedValidationData {
              relay_parent_number: 1,
              relay_parent_storage_root,
              max_pov_size,
              ..Default::default()
            },
            relay_chain_state,
            downward_messages: Default::default(),
            horizontal_messages,
            relay_parent_descendants: Default::default(),
            collator_peer_id: None,
          },
        )
        .expect("bounded HRMP relay-proof fixture encodes"); // deos-bypass: panic-owner — fixed 128/129-channel fixtures and upstream proof builder own deterministic encoding.
      assert_eq!(
        crate::apis::validate_context_inherent_geometry(&data),
        expected
      );
    }
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
#[ignore = "expensive maximum-context production evidence"]
fn maximum_context_static_fixture_dispatches() {
  use codec::Encode;
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system::parachain_inherent::{
      BasicParachainInherentData, InboundDownwardMessages, InboundHrmpMessages, InboundMessagesData,
    },
    cumulus_primitives_core::{
      InboundDownwardMessage, InboundHrmpMessage, ParaId, PersistedValidationData,
      relay_chain::HeadData,
    },
    frame_system::pallet_prelude::HeaderFor,
    sp_runtime::traits::Header as HeaderT,
  };

  seeded_test_ext().execute_with(|| {
    let downward_messages = (0..MaxInboundDownwardMessagesPerContext::get())
      .map(|_| InboundDownwardMessage {
        sent_at: 1,
        msg: vec![0; 65_536],
      })
      .collect::<Vec<_>>();
    let mut downward_queue = MessageQueueChain::default();
    for message in &downward_messages {
      downward_queue.extend_downward(message);
    }
    let mut horizontal_messages = BTreeMap::new();
    let mut proof_builder = RelayStateSproofBuilder::default();
    proof_builder.para_id = crate::ParachainInfo::parachain_id();
    proof_builder.dmq_mqc_head = Some(downward_queue.head());
    proof_builder.included_para_head = HeadData(
      HeaderFor::<Runtime>::new(
        0,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
      )
      .encode(),
    )
    .into();
    let channels = MaxInboundHorizontalChannelsPerContext::get();
    for message_index in 0..MaxInboundHorizontalMessagesPerContext::get() {
      let sender = ParaId::from(message_index % channels + 1);
      let message = InboundHrmpMessage {
        sent_at: 1,
        data: vec![0],
      };
      horizontal_messages
        .entry(sender)
        .or_insert_with(Vec::new)
        .push(message.clone());
      let channel = proof_builder.upsert_inbound_channel(sender);
      channel.max_message_size = 65_536;
      let mut queue = MessageQueueChain::new(channel.mqc_head.unwrap_or_default());
      queue.extend_hrmp(&message);
      channel.mqc_head = Some(queue.head());
    }
    let (relay_parent_storage_root, relay_chain_state) = proof_builder.into_state_root_and_proof();
    let basic = BasicParachainInherentData {
      validation_data: PersistedValidationData {
        relay_parent_number: 1,
        relay_parent_storage_root,
        max_pov_size: 12,
        ..Default::default()
      },
      relay_chain_state,
      relay_parent_descendants: Vec::new(),
      collator_peer_id: None,
    };
    let collection_size_limit = (crate::configs::RuntimeBlockWeights::get()
      .max_block
      .proof_size()
      / 6) as usize;
    let mut size_limit = collection_size_limit;
    let downward = InboundDownwardMessages::new(downward_messages).into_abridged(&mut size_limit);
    size_limit = size_limit.saturating_add(collection_size_limit);
    let horizontal =
      InboundHrmpMessages::from_map(horizontal_messages).into_abridged(&mut size_limit);
    let inbound = InboundMessagesData::new(downward, horizontal);
    let expected_processed = inbound.downward_messages.messages().0.len() as u32;
    assert_ok!(crate::ParachainSystem::set_validation_data(
      polkadot_sdk::frame_system::RawOrigin::None.into(),
      basic.clone(),
      inbound,
    ));
    assert_eq!(
      polkadot_sdk::cumulus_pallet_parachain_system::ProcessedDownwardMessages::<Runtime>::get(),
      expected_processed
    );
  });
}

#[test]
fn context_geometry_failure_is_fatal_and_has_one_owned_identifier() {
  let result = crate::apis::context_geometry_fatal_result();
  assert!(!result.ok());
  assert!(result.fatal_error());
  assert!(matches!(
    result.get_error::<()>(&crate::apis::CONTEXT_GEOMETRY_INHERENT_IDENTIFIER),
    Ok(Some(()))
  ));
  assert_eq!(result.into_errors().count(), 1);
}

#[test]
fn declared_context_geometry_covers_dmp_and_bounded_hrmp_dimensions() {
  let limits = ContextMessageLimits::new(
    MaxInboundDownwardMessagesPerContext::get(),
    MaxInboundHorizontalMessagesPerContext::get(),
    MaxInboundHorizontalChannelsPerContext::get(),
  );
  assert_eq!(MaxInboundDownwardMessagesPerContext::get(), 512);
  assert_eq!(MaxInboundHorizontalChannelsPerContext::get(), 128);
  assert_eq!(MaxInboundHorizontalMessagesPerContext::get(), 131_072);
  assert_eq!(
    limits.validate(ContextMessageGeometry::new(256, 256, 65_536, 65_536, 128)),
    Ok(())
  );
}

#[test]
fn configured_dmp_compatibility_reserve_equals_the_measured_owner() {
  let reserve = ReservedDmpWeight::get();
  assert_eq!(MaxInboundDownwardMessagesPerContext::get(), 512);
  let declared_maximum = <polkadot_sdk::cumulus_pallet_parachain_system::weights::SubstrateWeight<Runtime> as polkadot_sdk::cumulus_pallet_parachain_system::WeightInfo>::enqueue_inbound_downward_messages(512);
  assert_eq!(DmpFixedWeight::get(), declared_maximum);
  assert_eq!(reserve, declared_maximum);
}

#[test]
fn fresh_genesis_has_no_active_xcm_lazy_migration() {
  seeded_test_ext().execute_with(|| {
    let key =
      polkadot_sdk::frame_support::storage::storage_prefix(b"PolkadotXcm", b"CurrentMigration");
    assert!(polkadot_sdk::sp_io::storage::get(&key).is_none());
  });
}

#[test]
fn configured_inbound_owners_require_complete_fixed_classification() {
  assert_eq!(
    XcmMigrationMaximumWeight::get(),
    crate::configs::RuntimeBlockWeights::get().max_block / 10
  );
  assert_eq!(
    <Runtime as polkadot_sdk::pallet_xcm::Config>::VERSION_DISCOVERY_QUEUE_SIZE,
    100
  );
  let downward = ReservedDmpWeight::get();
  let horizontal = ReservedXcmpWeight::get();
  assert_eq!(downward, DmpFixedWeight::get());
  assert_eq!(
    horizontal,
    MaximumContextMeasuredWeight::get().saturating_sub(DmpFixedWeight::get())
  );
  assert_eq!(
    XcmpRegisteredFixedWeight::get(),
    horizontal.saturating_add(
      <Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get().reads_writes(2, 3)
    )
  );

  let block_weights = crate::configs::RuntimeBlockWeights::get();
  let mandatory_base = block_weights.get(DispatchClass::Mandatory).base_extrinsic;
  let three_mandatory_inherent_bases = mandatory_base
    .checked_add(&mandatory_base)
    .and_then(|weight| weight.checked_add(&mandatory_base))
    .expect("three Mandatory inherent bases must fit"); // deos-bypass: panic-owner — RuntimeBlockWeights construction and the three fixed inherents prove bounded addition.
  let frame_base = block_weights
    .base_block
    .checked_add(&three_mandatory_inherent_bases)
    .expect("configured FRAME base weights must fit"); // deos-bypass: panic-owner — RuntimeBlockWeights construction proves bounded addition.
  let timestamp = <polkadot_sdk::pallet_timestamp::weights::SubstrateWeight<Runtime> as polkadot_sdk::pallet_timestamp::WeightInfo>::set()
    .saturating_add(
      <polkadot_sdk::pallet_timestamp::weights::SubstrateWeight<Runtime> as polkadot_sdk::pallet_timestamp::WeightInfo>::on_finalize(),
    );
  let classified_candidate = frame_base
    .checked_add(&timestamp)
    .and_then(|weight| weight.checked_add(&downward))
    .and_then(|weight| weight.checked_add(&XcmpRegisteredFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&MessageQueueServiceWeight::get()))
    .and_then(|weight| weight.checked_add(&XcmpQueue::on_idle_weight()))
    .and_then(|weight| weight.checked_add(&GovernanceFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&SessionRotationFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&AuthorshipFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&AuraFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&AuraExtFixedWeight::get()))
    .and_then(|weight| weight.checked_add(&XcmVersionDiscoveryFixedWeight::get()));
  assert!(classified_candidate.is_some());
  let classified_candidate = classified_candidate.unwrap_or(Weight::MAX);
  assert_eq!(FixedBlockWeight::get(), classified_candidate);
  assert_eq!(
    FixedBlockWeightComponentsValue::get().total(),
    Ok(classified_candidate)
  );
  assert_eq!(
    BlockResourceBudgetValue::get().fixed_envelope(),
    classified_candidate
  );
  assert!(
    classified_candidate.all_lte(crate::MAXIMUM_BLOCK_WEIGHT),
    "classified fixed owners must fit before validation loops and remaining hooks are added"
  );
  assert!(
    horizontal.all_lte(classified_candidate),
    "known fixed owners cannot reduce the XCMP envelope"
  );
  let registered_context_owner = downward.saturating_add(XcmpRegisteredFixedWeight::get());
  assert!(
    MaximumContextMeasuredWeight::get().all_lte(registered_context_owner),
    "registered DMP/XCMP owners must dominate complete measured context execution without additive double accounting"
  );
  let unclassified_headroom = crate::MAXIMUM_BLOCK_WEIGHT.checked_sub(&classified_candidate);
  assert!(unclassified_headroom.is_some());
  let unclassified_headroom = unclassified_headroom.unwrap_or(Weight::zero());
  assert!(unclassified_headroom.ref_time() > 0);
  assert!(unclassified_headroom.proof_size() > 0);
}

#[test]
fn block_weight_partition_is_50_dispatch_50_on_idle_without_operational_reserve() {
  let maximum = crate::MAXIMUM_BLOCK_WEIGHT;
  let normal = crate::NORMAL_DISPATCH_RATIO * maximum;
  let on_idle = crate::MIN_ON_IDLE_RESERVE_RATIO * maximum;
  let dispatchable = crate::configs::MaxDispatchableExtrinsicWeight::get();
  let operational = dispatchable.saturating_sub(normal);

  assert_eq!(normal, Perbill::from_percent(50) * maximum);
  assert_eq!(operational, Weight::zero());
  assert_eq!(on_idle, Perbill::from_percent(50) * maximum);
  assert_eq!(
    crate::configs::RuntimeBlockWeights::get()
      .get(DispatchClass::Operational)
      .reserved,
    None
  );
  assert_eq!(
    normal.saturating_add(operational).saturating_add(on_idle),
    maximum
  );
}

#[test]
fn configured_on_idle_reserve_admits_every_genesis_actor_with_pure_cleanup() {
  seeded_test_ext().execute_with(|| {
    let reserve = <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    assert_eq!(
      reserve,
      crate::MIN_ON_IDLE_RESERVE_RATIO * crate::MAXIMUM_BLOCK_WEIGHT
    );
    let mut actor_count = 0u32;
    let mut max_ref_time = (0u64, 0u64);
    let mut max_proof_size = (0u64, 0u64);
    for actor_id in pallet_deos_actors::ActorHot::<Runtime>::iter_keys() {
      let instance = Actors::active_actor_state(actor_id).expect("split active actor exists");
      let required = Actors::contract_steps_admission_weight_upper(
        instance.identity.actor_class.actor_type(),
        &instance.contract.steps,
      );
      assert!(
        required.all_lte(reserve),
        "actor_id={actor_id}, required={required:?}, reserve={reserve:?}",
      );
      if required.ref_time() > max_ref_time.1 {
        max_ref_time = (actor_id, required.ref_time());
      }
      if required.proof_size() > max_proof_size.1 {
        max_proof_size = (actor_id, required.proof_size());
      }
      actor_count = actor_count.saturating_add(1);
    }
    assert!(
      actor_count > 0,
      "reference genesis must contain System Actors"
    );
    println!(
      "Actors admission: actors={actor_count}, reserve={reserve:?}, max_ref_time={max_ref_time:?}, max_proof_size={max_proof_size:?}"
    );
  });
}

#[test]
fn configured_on_idle_reserve_admits_one_scheduler_actor_probe() {
  let required = Actors::scheduler_admission_overhead();
  let reserve = crate::MIN_ON_IDLE_RESERVE_RATIO * crate::MAXIMUM_BLOCK_WEIGHT;
  assert!(
    required.all_lte(reserve),
    "required={required:?}, reserve={reserve:?}"
  );
}

#[test]
fn mandatory_base_progress_prevents_false_idle_starvation() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    System::set_block_number(1);
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 10),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(starvation_blocked_budget(actor_id));
    assert!(!IdleStarvationState::<Runtime>::exists());
    for block in 2..=(threshold + 2) {
      System::set_block_number(block);
      run_idle(starvation_blocked_budget(actor_id));
    }
    let detections = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationDetected { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert!(detections.is_empty());
    assert!(!IdleStarvationState::<Runtime>::exists());
  });
}

#[test]
fn starvation_requires_live_fifo_work() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    assert!(!IdleStarvationState::<Runtime>::exists());
    // An empty queue with an exhausted budget must never starve: no live FIFO work exists.
    for block in 1..=(threshold + 2) {
      System::set_block_number(block);
      run_idle(starvation_observation_weight());
    }
    assert!(!IdleStarvationState::<Runtime>::exists());
  });
}

#[test]
fn starvation_recovery_is_observable_and_healthy_idle_stays_sparse() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    assert!(!IdleStarvationState::<Runtime>::exists());
    System::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 10),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    for block in 2..=(threshold + 1) {
      System::set_block_number(block);
      run_idle(starvation_blocked_budget(actor_id));
    }
    assert!(!IdleStarvationState::<Runtime>::exists());
    System::set_block_number(threshold.saturating_add(2));
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationRecovered { .. }
    )));
    let recovery_count = System::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        )
      })
      .count();
    System::set_block_number(threshold.saturating_add(3));
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    assert_eq!(
      System::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      recovery_count
    );
  });
}

// --- Actors Platform: Owner Slots ---

#[test]
fn system_actor_count_is_not_limited_by_owner_slots() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let attempts =
      <<Runtime as pallet_deos_actors::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<crate::AccountId> = Vec::new();
    for _ in 0..attempts {
      let actor_id = create_system(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, AssetKind::Native, 1),
      );
      let inst = Actors::active_actor_state(actor_id).expect("Actors exists");
      assert_eq!(
        inst.identity.actor_class,
        pallet_deos_actors::ActorClass::System {
          sovereign_id: actor_id,
        }
      );
      sovereign_accounts.push(inst.identity.sovereign_account);
    }
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    for i in 0..sovereign_accounts.len() {
      for j in i + 1..sovereign_accounts.len() {
        assert_ne!(sovereign_accounts[i], sovereign_accounts[j]);
      }
    }
  });
}

#[test]
fn governance_can_update_active_actor_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_limit = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    assert_ok!(Actors::set_active_actor_limit(
      RuntimeOrigin::root(),
      max_limit - 1,
    ));
    assert_eq!(
      pallet_deos_actors::ActiveActorLimit::<Runtime>::get(),
      max_limit - 1
    );
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    assert!(Actors::active_actor_state(actor_id).is_some());
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 0),
      pallet_deos_actors::Error::<Runtime>::ActiveActorLimitTooLow
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      pallet_deos_actors::Error::<Runtime>::ActiveActorLimitTooHigh
    );
  });
}

#[test]
fn owner_slot_reuses_freed_slot_after_close() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let id0 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let id1 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let slot0 = Actors::active_actor_state(id0)
      .expect("id0 exists")
      .identity
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    let slot1 = Actors::active_actor_state(id1)
      .expect("id1 exists")
      .identity
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let slot2 = Actors::active_actor_state(id2)
      .expect("id2 exists")
      .identity
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot2, slot0);
  });
}

// --- User DCA Lifecycle ---

#[test]
fn user_dca_e2e_lifecycle_with_explicit_close() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let create_fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let initial_alice_balance = Balances::free_balance(&ALICE);
    let schedule = Schedule {
      trigger: Trigger::cadenced(5),
      cooldown_blocks: 0,
    };
    let foreign = AssetKind::Local(ASSET_A);
    let swap_amount = primitives::ecosystem::params::PRECISION;
    let steps = BoundedVec::try_from(vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: foreign,
        amount_in: AmountResolution::Fixed(swap_amount),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .unwrap();
    let id = create_user(ALICE, schedule, None, steps.clone());
    assert!(has_actor_event(
      |e| matches!(e, Event::ActorCreated { actor_id, .. } if *actor_id == id)
    ));
    assert_eq!(
      Balances::free_balance(&ALICE),
      initial_alice_balance - create_fee - actor_state_hold_total(id)
    );
    let sov = Actors::sovereign_account_id(&ALICE, 0);
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let inst = Actors::active_actor_state(id).unwrap();
    let per_cycle_fee = Actors::attempt_fee_envelope(
      inst.identity.actor_class.actor_type(),
      &inst.contract.steps,
      0,
    )
    .expect("admitted plan has a checked fee envelope")
    .total;
    let native_funding = min_user_balance + (per_cycle_fee + swap_amount) * 3;
    let _ = <Balances as Currency<crate::AccountId>>::transfer(
      &ALICE,
      &sov,
      native_funding,
      polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
    );
    let mut max_nonce = 0;
    for block in 2..=20 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      run_idle(Weight::MAX);
      for event in System::events() {
        if let RuntimeEvent::Actors(Event::CycleSummary {
          actor_id: ev_id,
          cycle_nonce,
          ..
        }) = event.event
          && ev_id == id
          && cycle_nonce > max_nonce
        {
          max_nonce = cycle_nonce;
        }
      }
      System::reset_events();
    }
    assert!(max_nonce >= 2, "Should have executed at least 2 cycles");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
    assert!(Actors::active_actor_state(id).is_none());
    let id_new = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let slot_new = Actors::active_actor_state(id_new)
      .expect("id_new exists")
      .identity
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot_new, 0);
  });
}

// --- Circular Transfer Chain Stress Tests ---

/// Creates `n` System Actors with explicit StopCycle contracts for scheduler stress testing.
fn inert_timer_contract() -> Option<pallet_deos_actors::ActorContractOf<Runtime>> {
  system_active_contract(
    Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    },
    None,
    alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: inert_task(),
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits"),
  )
}

fn inert_manual_window_contract(
  start: u32,
  end: u32,
) -> Option<pallet_deos_actors::ActorContractOf<Runtime>> {
  system_active_contract(
    manual_schedule(),
    Some(ScheduleWindow { start, end }),
    alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: inert_task(),
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits"),
  )
}

fn setup_inert_actors(n: u64, initial_balance: u128) -> alloc::vec::Vec<u64> {
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
  }
  actor_ids
}

fn setup_mixed_inert_actors(n: u64, initial_balance: u128) -> alloc::vec::Vec<u64> {
  let mut actor_ids = alloc::vec::Vec::new();
  let inert_plan: RuntimeContractSteps = alloc::vec![pallet_deos_actors::Step {
    precondition: None,
    task: inert_task(),
    on_error: StepErrorPolicy::AbortCycle,
  }]
  .try_into()
  .expect("fits");
  for index in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    if index % 2 == 0 {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        inert_timer_contract(),
      ));
    } else {
      let mut owner_bytes = [0u8; 32];
      owner_bytes[..8].copy_from_slice(&index.to_le_bytes());
      owner_bytes[31] = 0xA7;
      let owner = crate::AccountId::from(owner_bytes);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&owner, initial_balance);
      prefund_active_user_creation(&owner, &inert_plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(owner),
        Mutability::Mutable,
        inert_timer_contract(),
      ));
    }
    age_fixture_control_clock(actor_id);
    let sovereign = Actors::active_actor_state(actor_id)
      .expect("mixed stress actor exists")
      .identity
      .sovereign_account;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, initial_balance);
    actor_ids.push(actor_id);
  }
  actor_ids
}

fn setup_inert_actors_sparse(n: u64, initial_balance: u128, stride: u64) -> alloc::vec::Vec<u64> {
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let effective_stride = stride.max(2);
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_id);
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    let bumped_next = actor_id.saturating_add(effective_stride);
    pallet_deos_actors::NextActorId::<Runtime>::put(bumped_next);
  }
  actor_ids
}

/// Helper: creates `n` System Actors in a circular transfer chain.
/// Returns (actor_ids, sovereign_accounts).
fn setup_circular_chain(
  n: u64,
  initial_balance: u128,
) -> (alloc::vec::Vec<u64>, alloc::vec::Vec<crate::AccountId>) {
  let transfer_pct = Perbill::from_percent(1);
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let mut sovereign_accounts = alloc::vec::Vec::new();
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_id);
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    sovereign_accounts.push(sov);
  }
  for i in 0..n {
    let next_sov = sovereign_accounts[((i + 1) % n) as usize].clone();
    let steps: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
      precondition: all_preconditions(alloc::vec![pallet_deos_actors::Predicate::BalanceAbove {
        asset: primitives::AssetKind::Native,
        threshold: crate::EXISTENTIAL_DEPOSIT,
      },]),
      task: Task::Transfer {
        to: next_sov,
        asset: primitives::AssetKind::Native,
        amount: AmountResolution::PercentageOfCurrent(transfer_pct),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_ids[i as usize],
      (steps, CompletionPolicy::Persistent,)
    ));
  }
  (actor_ids, sovereign_accounts)
}

/// Per-block diagnostic counters collected during stress run.
struct StressDiagnostics {
  actor_cycle_counts: alloc::collections::BTreeMap<u64, u32>,
  total_failed_steps: u32,
  min_per_block: u32,
  max_per_block: u32,
}

struct QueuePressureDiagnostics {
  max_queue_occupancy: u32,
  max_wakeup_backlog: u32,
  max_wakeup_buckets: u32,
}

/// Runs `num_blocks` blocks with on_initialize + on_idle, collecting per-block diagnostics.
fn run_blocks_with_diagnostics(
  actor_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> StressDiagnostics {
  let (diag, _) = run_blocks_with_queue_diagnostics(actor_ids, num_blocks, weight);
  diag
}

fn run_blocks_with_queue_diagnostics(
  actor_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> (StressDiagnostics, QueuePressureDiagnostics) {
  let mut diag = StressDiagnostics {
    actor_cycle_counts: actor_ids.iter().map(|&id| (id, 0u32)).collect(),
    total_failed_steps: 0,
    min_per_block: u32::MAX,
    max_per_block: 0,
  };
  let mut queue_diag = QueuePressureDiagnostics {
    max_queue_occupancy: 0,
    max_wakeup_backlog: 0,
    max_wakeup_buckets: 0,
  };
  for block in 2..=(num_blocks + 1) {
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    run_idle(weight);
    let mut block_executions = 0u32;
    for evt in System::events() {
      match &evt.event {
        RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, outcomes, ..
        }) => {
          if let Some(count) = diag.actor_cycle_counts.get_mut(actor_id) {
            *count += 1;
          }
          block_executions += 1;
          diag.total_failed_steps += outcomes.failed_steps;
        }
        _ => {}
      }
    }
    let queue_occupancy = Actors::queue_tail()
      .saturating_sub(Actors::queue_head())
      .min(u64::from(u32::MAX)) as u32;
    let mut wakeup_backlog = 0u32;
    let mut wakeup_buckets = 0u32;
    for (_, bucket) in pallet_deos_actors::WakeupBuckets::<Runtime>::iter() {
      wakeup_backlog = wakeup_backlog.saturating_add(bucket.live_entries);
      wakeup_buckets = wakeup_buckets.saturating_add(1);
    }
    queue_diag.max_queue_occupancy = queue_diag.max_queue_occupancy.max(queue_occupancy);
    queue_diag.max_wakeup_backlog = queue_diag.max_wakeup_backlog.max(wakeup_backlog);
    queue_diag.max_wakeup_buckets = queue_diag.max_wakeup_buckets.max(wakeup_buckets);
    if block > 2 {
      diag.min_per_block = diag.min_per_block.min(block_executions);
    }
    diag.max_per_block = diag.max_per_block.max(block_executions);
  }
  (diag, queue_diag)
}

/// Asserts stability invariants that apply regardless of capacity scenario.
fn assert_core_stability(actor_ids: &[u64], diag: &StressDiagnostics) {
  assert_eq!(
    diag.total_failed_steps, 0,
    "All transfer steps must succeed (got {} failures)",
    diag.total_failed_steps,
  );
  for &id in actor_ids {
    let inst = Actors::active_actor_state(id).expect("actor must still exist");
    assert_eq!(
      inst.hot.unsuccessful_attempt_streak, 0,
      "Actor {} has unsuccessful-attempt streak {}",
      id, inst.hot.unsuccessful_attempt_streak,
    );
  }
}

/// Under-capacity: 45 chain actors plus active genesis work remain inside the
/// configurable execution ceiling and receive recurring service through the measured reserve.
/// Dormant and custody-only genesis addresses never compete for scheduler capacity.
///
/// Asserts exact balance conservation, complete traversal, bounded fairness,
/// zero failures, and zero unsuccessful-attempt streak.
#[test]
fn circular_chain_under_capacity_preserves_progress_and_fairness() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 45u64;
    let num_blocks = 50u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (actor_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag = run_blocks_with_diagnostics(
      &actor_ids,
      num_blocks,
      Weight::from_parts(u64::MAX, u64::MAX),
    );
    // Balance conservation (exact: System Actors pay no fees)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Every chain actor must receive service within the bounded run.
    for &id in &actor_ids {
      let count = diag.actor_cycle_counts[&id];
      assert!(count > 0, "Actor {id} never received service");
    }
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    assert!(
      u64::from(total_executions) >= chain_len,
      "The run must complete at least one full traversal",
    );
    // Independent cadence detection remains bounded but may phase-shift Actors when due
    // materialization spans blocks; FIFO still orders every readiness that reaches service.
    let nonces: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter_map(|&id| Actors::active_actor_state(id).map(|state| state.identity.cycle_nonce))
      .collect();
    let (min_n, max_n) = (*nonces.iter().min().unwrap(), *nonces.iter().max().unwrap());
    assert!(
      max_n.saturating_sub(min_n) <= 10,
      "Cadenced detection/service spread exceeds 10 (min={min_n}, max={max_n})",
    );
    assert!(min_n > 0, "Every actor must complete at least one cycle");
    assert_core_stability(&actor_ids, &diag);
  });
}

/// Diagnostic test: trace first 5 blocks in detail (execute_cycle only, no emergency)
#[test]
fn diagnose_over_capacity_first_blocks() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (_actor_ids, _sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    println!("\n=== Initial state ===");
    let active_count = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    println!("Active instances len: {}", active_count);
    for block in 2..=6 {
      System::set_block_number(block);
      System::reset_events();
      run_idle(Weight::from_parts(u64::MAX, u64::MAX));
      let executions: alloc::vec::Vec<u64> = System::events()
        .iter()
        .filter_map(|evt| {
          if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
            Some(*actor_id)
          } else {
            None
          }
        })
        .collect();
      let min_id = executions.iter().min().copied();
      let max_id = executions.iter().max().copied();
      println!("\n=== Block {} ===", block);
      println!(
        "Executions: {} (IDs: {:?}..{:?})",
        executions.len(),
        min_id,
        max_id
      );
      // Check zero actors (2006-2020)
      let zero_actors: alloc::vec::Vec<u64> = (2006..=2020).collect();
      let zero_executed: alloc::vec::Vec<u64> = executions
        .iter()
        .filter(|id| zero_actors.contains(id))
        .cloned()
        .collect();
      println!(
        "Zero actors (2006-2020) executed: {} {:?}",
        zero_executed.len(),
        zero_executed
      );
    }
    // After 5 blocks, check nonce of zero actors
    println!("\n=== After 5 blocks ===");
    for id in 2006..=2010 {
      if let Some(inst) = Actors::active_actor_state(id) {
        println!(
          "Actors {}: cycle_nonce={}, last_cycle_block={}",
          id,
          inst.identity.cycle_nonce,
          inst
            .hot
            .last_cycle_block
            .map(|b| b.to_string())
            .unwrap_or_else(|| String::from("None"))
        );
      }
    }
    for id in 2006..=2010 {
      println!(
        "Actors {} present: {}",
        id,
        pallet_deos_actors::ActorHot::<Runtime>::contains_key(id)
      );
    }
  });
}

/// A 100-actor chain remains fair while the configurable count ceiling and
/// WeightMeter independently bound per-block execution.
#[test]
fn circular_chain_respects_execution_ceiling_and_remains_fair() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let num_blocks = 100u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (actor_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag = run_blocks_with_diagnostics(
      &actor_ids,
      num_blocks,
      Weight::from_parts(u64::MAX, u64::MAX),
    );
    // Balance conservation (exact)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Per-block execution cap respected
    let execution_ceiling = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get();
    assert!(
      diag.max_per_block <= execution_ceiling,
      "Per-block throughput must not exceed MaxExecutionsPerBlock={execution_ceiling} (got {})",
      diag.max_per_block,
    );
    // No starvation: every chain actor must have executed multiple times
    let min_count = *diag.actor_cycle_counts.values().min().unwrap();
    let zero_actors: alloc::vec::Vec<u64> = diag
      .actor_cycle_counts
      .iter()
      .filter(|(_id, count)| **count == 0)
      .map(|(id, _)| *id)
      .collect();
    assert!(
      min_count > 0,
      "No starvation: every actor must execute at least once (min_count={}, \
       zero_actors={:?}, active_actors_len={})",
      min_count,
      &zero_actors[..zero_actors.len().min(10)],
      pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count(),
    );
    // Fairness: examine cycle_nonce spread across chain actors.
    // With identical periodic actors, the queue scheduler should keep nonce spread minimal (≤ 2).
    let nonces: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter_map(|&id| Actors::active_actor_state(id).map(|state| state.identity.cycle_nonce))
      .collect();
    let min_nonce = *nonces.iter().min().unwrap();
    let max_nonce = *nonces.iter().max().unwrap();
    let nonce_spread = max_nonce - min_nonce;
    assert!(
      nonce_spread <= 2,
      "Fairness: nonce spread {} exceeds 2 (min={}, max={})",
      nonce_spread,
      min_nonce,
      max_nonce,
    );
    // Complete traversal is the liveness floor; count ceilings do not promise throughput.
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    assert!(
      total_executions >= actor_ids.len() as u32,
      "Total executions {total_executions} must cover all {} actors",
      actor_ids.len(),
    );
    assert_core_stability(&actor_ids, &diag);
  });
}

fn clear_genesis_system_actors_for_stress_fixture() {
  let actors: alloc::vec::Vec<_> = pallet_deos_actors::ActorHot::<Runtime>::iter().collect();
  for (actor_id, _hot) in actors {
    pallet_deos_actors::ActorHot::<Runtime>::remove(actor_id);
    pallet_deos_actors::ActorContractHeads::<Runtime>::remove(actor_id);
    pallet_deos_actors::ActorAdmissionCertificates::<Runtime>::remove(actor_id);
    let _ = pallet_deos_actors::ActorContractTailChunks::<Runtime>::clear_prefix(
      actor_id,
      u32::MAX,
      None,
    );
    pallet_deos_actors::ActorFunding::<Runtime>::remove(actor_id);
    let identity = Actors::actor_identities(actor_id).expect("actor identity exists");
    pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
  }
  let identities: alloc::vec::Vec<_> =
    pallet_deos_actors::ActorIdentities::<Runtime>::iter().collect();
  for (actor_id, identity) in identities {
    pallet_deos_actors::ActorIdentities::<Runtime>::remove(actor_id);
    pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
  }
  // Isolate the synthetic active-capacity profile from retained genesis locators.
  // Production close preserves those locators for deterministic reattachment.
  let _ = pallet_deos_actors::SystemSovereigns::<Runtime>::clear(u32::MAX, None);
  pallet_deos_actors::SystemSovereignCount::<Runtime>::put(0);
  let _ = pallet_deos_actors::WakeupPages::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::WakeupBuckets::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::WakeupCursorPages::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::WakeupCursorLen::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::QueuePages::<Runtime>::clear(u32::MAX, None);
  pallet_deos_actors::QueueHead::<Runtime>::put(0);
  pallet_deos_actors::QueueTail::<Runtime>::put(0);
  pallet_deos_actors::QueueOccupancy::<Runtime>::put(0);
  pallet_deos_actors::NextQueueTicket::<Runtime>::put(0);
  pallet_deos_actors::ActiveActorCount::<Runtime>::put(0);
  pallet_deos_actors::ActorIdentityCount::<Runtime>::put(0);
}

fn close_genesis_system_actors() {
  clear_genesis_system_actors_for_stress_fixture();
}

fn run_fairness_matrix_case(total_actors: u64, num_blocks: u32) -> StressDiagnostics {
  System::set_block_number(1);
  close_genesis_system_actors();
  assert_eq!(
    pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count(),
    0,
    "Genesis actors must be removed for isolated fairness matrix",
  );
  let initial_balance = 10_000u128;
  let actor_ids = setup_inert_actors(total_actors, initial_balance);
  let active_count = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
  assert_eq!(
    active_count, total_actors,
    "Scenario must start with exact actor count (expected={}, got={})",
    total_actors, active_count,
  );
  let diag = run_blocks_with_diagnostics(&actor_ids, num_blocks, Weight::MAX);
  let budget = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() as u64;
  assert!(
    diag.max_per_block as u64 <= budget,
    "Per-block throughput must not exceed MaxExecutionsPerBlock={} (got {})",
    budget,
    diag.max_per_block,
  );
  let min_count = *diag.actor_cycle_counts.values().min().unwrap() as u64;
  let max_count = *diag.actor_cycle_counts.values().max().unwrap() as u64;
  let spread = max_count.saturating_sub(min_count);
  assert!(
    spread <= 4,
    "Fairness: nonce spread {} exceeds 4 (min={}, max={}, actors={}, blocks={})",
    spread,
    min_count,
    max_count,
    total_actors,
    num_blocks,
  );
  // Actual measured throughput, rather than the configured count ceiling, must still
  // cover every actor. The bounded spread assertion above owns relative fairness.
  let total_served: u64 = diag
    .actor_cycle_counts
    .values()
    .map(|&c| u64::from(c))
    .sum();
  assert!(
    total_served >= total_actors,
    "Scenario must serve every actor at least once (actors={}, served={})",
    total_actors,
    total_served,
  );
  let full_rotation_blocks = total_actors.div_ceil(budget);
  assert!(
    num_blocks as u64 >= full_rotation_blocks,
    "Scenario blocks {} must cover at least one full rotation {}",
    num_blocks,
    full_rotation_blocks,
  );
  assert_core_stability(&actor_ids, &diag);
  diag
}

// --- Scheduler Fast FIFO Stress (CI) ---

#[test]
fn scheduler_fast_fifo_dense_vs_sparse_topology_smoke() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 2] = [(64, 96, 8), (256, 128, 16)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert!(
      dense_total.abs_diff(sparse_total) <= 1,
      "Finite-horizon topology throughput may differ by at most one tail admission (actors={}, blocks={}, stride={}, dense={}, sparse={})",
      actors,
      blocks,
      stride,
      dense_total,
      sparse_total,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 8,
      "Dense cadence/service spread exceeded bound=8 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 8,
      "Sparse cadence/service spread exceeded bound=8 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
fn scheduler_fast_fifo_sparse_topology_liveness_smoke() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 256u64;
    let blocks = 192u32;
    let stride = 32u64;
    let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Sparse topology smoke must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

#[test]
fn reference_idle_budget_admits_mixed_tasks_without_starvation() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let mut actor_ids = setup_inert_actors(32, 10_000u128);
    let (transfer_ids, _) = setup_circular_chain(32, 10_000u128);
    actor_ids.extend(transfer_ids);
    let budget =
      <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    let diag = run_blocks_with_diagnostics(&actor_ids, 40, budget);
    let counts: alloc::vec::Vec<u32> = actor_ids
      .iter()
      .map(|id| diag.actor_cycle_counts[id])
      .collect();
    let min_cycles = *counts.iter().min().expect("actors exist");
    let max_cycles = *counts.iter().max().expect("actors exist");
    assert!(min_cycles > 0, "every admitted actor must make progress");
    assert!(
      max_cycles.saturating_sub(min_cycles) <= 1,
      "FIFO carry-over must keep mixed-task nonce spread <= 1: {counts:?}"
    );
    assert_eq!(diag.total_failed_steps, 0);
  });
}

#[test]
fn reference_idle_budget_converges_paged_wakeup_and_pure_close_pressure() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let retry_ids = setup_inert_actors(
      u64::from(<Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get()),
      10_000u128,
    );
    let expired_count = <Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get();
    let mut expired_ids = alloc::vec::Vec::new();
    for _ in 0..expired_count {
      expired_ids.push(create_system(
        ALICE,
        manual_schedule(),
        Some(ScheduleWindow { start: 1, end: 101 }),
        BoundedVec::try_from(vec![make_step(inert_task())]).expect("steps fits"),
      ));
    }
    let asset_id = 90u32;
    assert_ok!(create_test_asset(asset_id, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      asset_id,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let close_id = expired_ids[0];
    let close_account = actor_account(close_id);
    assert_ok!(mint_tokens(asset_id, &ALICE, &close_account, 500));
    let budget =
      <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    for block in 102..=150 {
      System::set_block_number(block);
      set_consensus_timestamp(u64::from(block).saturating_mul(6_000));
      Actors::on_initialize(block);
      run_idle(budget);
      let retries_done = retry_ids.iter().all(|id| {
        Actors::actor_hot(*id)
          .is_some_and(|hot| hot.wakeup_pointer.is_none() && hot.trigger_wakeup_pointer.is_some())
      });
      let closes_done = expired_ids
        .iter()
        .all(|id| Actors::active_actor_state(*id).is_none());
      let live_progress = retry_ids.iter().all(|id| {
        Actors::active_actor_state(*id).is_some_and(|state| state.identity.cycle_nonce > 0)
      });
      if retries_done && closes_done && live_progress {
        break;
      }
    }

    assert!(
      retry_ids.iter().all(|id| {
        Actors::actor_hot(*id)
          .is_some_and(|hot| hot.wakeup_pointer.is_none() && hot.trigger_wakeup_pointer.is_some())
      }),
      "overdue block wakeups must converge back to cadence ownership"
    );
    assert!(
      retry_ids.iter().all(|id| {
        Actors::active_actor_state(*id).is_some_and(|state| state.identity.cycle_nonce > 0)
      }),
      "live actors must progress while cleanup converges"
    );
    let repair_batch = BoundedVec::try_from(expired_ids.clone()).expect("repair batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(ALICE),
      repair_batch,
    ));
    assert!(
      expired_ids
        .iter()
        .all(|id| Actors::active_actor_state(*id).is_none()),
      "explicit bounded repair must close externally stranded actors"
    );
    assert_eq!(
      Assets::balance(asset_id, close_account),
      500,
      "pure terminal cleanup must preserve sovereign balances"
    );
  });
}

// --- Scheduler Stress FIFO (scheduled/nightly) ---

#[test]
#[ignore] // Heavy: run in the scheduled nightly FIFO stress job (release mode)
fn scheduler_stress_fifo_over_capacity_fairness_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 1_300)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let _ = run_fairness_matrix_case(actors, blocks);
    });
  }
}

#[test]
#[ignore] // Heavy topology matrix, run in the scheduled nightly FIFO stress job
fn scheduler_stress_fifo_dense_vs_sparse_topology_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 3] = [(100, 200, 8), (1000, 300, 16), (5000, 700, 32)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert_eq!(
      dense_total, sparse_total,
      "Topology must not change total execution throughput (actors={}, blocks={}, stride={})",
      actors, blocks, stride,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 8,
      "Dense cadence/service spread exceeded bound=8 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 8,
      "Sparse cadence/service spread exceeded bound=8 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
#[ignore] // Heavy long-run sparse-liveness check, run in the scheduled nightly FIFO stress job
fn scheduler_stress_fifo_sparse_topology_long_run_liveness() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 2000u64;
    let blocks = 1024u32;
    let stride = 32u64;
    let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Long-run sparse topology must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Long-run sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

#[test]
#[ignore] // Checkpoint A capacity acceptance; run through scripts/actors-assurance.sh.
fn checkpoint_a_s6_dense_10k_wakeups_converge_without_drops() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actor_count = 10_000u32;
    let wakeup_block = 10;
    let actor_ids = setup_inert_actors(actor_count.into(), 10_000u128);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(actor_ids.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id).is_some_and(|hot| hot.queue_ticket.is_none())
    }));
    for actor_id in &actor_ids {
      assert!(Actors::wakeup_substrate_schedule(*actor_id, wakeup_block));
    }

    let bucket = Actors::wakeup_buckets(wakeup_block).expect("dense wakeup bucket");
    assert_eq!(bucket.live_entries, actor_count);
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(wakeup_block));

    let mut scanned = 0u32;
    let mut passes = 0u32;
    while Actors::wakeup_cursor_len() > 0 {
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      let stats = Actors::drain_overdue_wakeups_cursor(wakeup_block, &mut meter);
      assert!(stats.entries_scanned > 0, "each pass must make progress");
      scanned = scanned.saturating_add(stats.entries_scanned);
      passes = passes.saturating_add(1);
      assert!(passes <= actor_count, "dense drain must remain bounded");
    }

    assert_eq!(scanned, actor_count);
    assert!(Actors::wakeup_buckets(wakeup_block).is_none());
    assert!(actor_ids.iter().all(|actor_id| {
      let hot = Actors::actor_hot(*actor_id).expect("active actor");
      hot.wakeup_pointer.is_none() && hot.queue_ticket.is_some()
    }));
  });
}

#[test]
fn scheduler_512_mixed_clocks_survives_delayed_timestamp() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let tick_millis = primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS;
    set_consensus_timestamp(tick_millis);

    let tick_ids = setup_inert_actors(256, 10_000u128);
    let mut block_ids = alloc::vec::Vec::new();
    for _ in 0..256 {
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        inert_manual_window_contract(10, 1_000),
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id,
      ));
      let hot = Actors::actor_hot(actor_id).expect("manual actor is active");
      assert_eq!(
        hot.wakeup_pointer.map(|pointer| pointer.block),
        Some(WakeupKey::Block(10)),
      );
      block_ids.push(actor_id);
    }
    assert!(tick_ids.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id).is_some_and(|hot| {
        hot
          .trigger_wakeup_pointer
          .is_some_and(|pointer| pointer.tick == 2)
      })
    }));

    System::set_block_number(10);
    set_consensus_timestamp(tick_millis.saturating_mul(100));
    let mut counts = tick_ids
      .iter()
      .chain(block_ids.iter())
      .map(|actor_id| (*actor_id, 0u32))
      .collect::<alloc::collections::BTreeMap<_, _>>();
    let mut tick_first = None;
    let mut tick_last = None;
    let mut block_first = None;
    let mut block_last = None;
    let mut previous_tick_actor = None;
    let mut previous_block_actor = None;

    for block in 10..=80 {
      System::set_block_number(block);
      System::reset_events();
      Actors::on_initialize(block);
      run_idle(Weight::MAX);
      let mut served_this_block = alloc::collections::BTreeSet::new();
      for record in System::events() {
        let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = record.event else {
          continue;
        };
        let Some(count) = counts.get_mut(&actor_id) else {
          continue;
        };
        assert!(
          served_this_block.insert(actor_id),
          "actor {actor_id} executed twice in block {block}",
        );
        *count = count.saturating_add(1);
        assert_eq!(*count, 1, "delayed cadence must not catch up in a burst");
        if tick_ids.contains(&actor_id) {
          assert!(
            previous_tick_actor.is_none_or(|previous| actor_id > previous),
            "tick-clock FIFO order regressed at actor {actor_id}",
          );
          previous_tick_actor = Some(actor_id);
          tick_first.get_or_insert(block);
          tick_last = Some(block);
        } else {
          assert!(
            previous_block_actor.is_none_or(|previous| actor_id > previous),
            "block-clock FIFO order regressed at actor {actor_id}",
          );
          previous_block_actor = Some(actor_id);
          block_first.get_or_insert(block);
          block_last = Some(block);
        }
      }
      if counts.values().all(|count| *count == 1) {
        break;
      }
    }

    assert!(counts.values().all(|count| *count == 1));
    let tick_first = tick_first.expect("tick clock makes progress");
    let block_first = block_first.expect("block clock makes progress");
    let tick_last = tick_last.expect("tick clock completes");
    let block_last = block_last.expect("block clock completes");
    assert!(
      tick_first.abs_diff(block_first) <= 1,
      "mixed clocks must begin service within one block: tick={tick_first}, block={block_first}",
    );
    assert!(
      tick_last.abs_diff(block_last) <= 1,
      "mixed clocks must finish service within one block: tick={tick_last}, block={block_last}",
    );
    for actor_id in &tick_ids {
      let hot = Actors::actor_hot(*actor_id).expect("Cadenced stress actor remains active");
      assert!(hot.queue_ticket.is_none());
      assert!(hot.wakeup_pointer.is_none());
      assert!(hot.trigger_wakeup_pointer.is_some());
    }
    for actor_id in &block_ids {
      let hot = Actors::actor_hot(*actor_id).expect("block-clock stress actor remains active");
      assert!(hot.queue_ticket.is_none());
      assert!(hot.wakeup_pointer.is_some());
      assert!(hot.trigger_wakeup_pointer.is_none());
    }
  });
}

#[test]
#[ignore] // Queue/wakeup occupancy diagnostics for over-capacity stress scenario
fn profile_scheduler_queue_wakeup_occupancy_10k() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 10_000u64;
    let blocks = 1_300u32;
    let actor_ids = setup_inert_actors(actors, 10_000u128);
    let (diag, queue_diag) = run_blocks_with_queue_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let spread = max_count.saturating_sub(min_count);
    println!(
      "Actors queue profile: actors={}, blocks={}, min_cycle_nonce={}, max_cycle_nonce={}, spread={}, max_queue_occupancy={}, max_wakeup_backlog={}, max_wakeup_buckets={}",
      actors,
      blocks,
      min_count,
      max_count,
      spread,
      queue_diag.max_queue_occupancy,
      queue_diag.max_wakeup_backlog,
      queue_diag.max_wakeup_buckets,
    );
    assert!(min_count > 0, "10k stress profile must remain starvation-free");
    assert!(
      spread <= 4,
      "10k stress profile nonce spread {} exceeds release bound 4 (min={}, max={})",
      spread,
      min_count,
      max_count,
    );
  });
}

// Profiling utility: run manually in release mode for wall-clock matrix
#[test]
#[ignore]
fn profile_scheduler_wallclock_matrix() {
  use super::common::new_test_ext;
  use std::time::Instant;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 1_300)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let started = Instant::now();
      let diag = run_fairness_matrix_case(actors, blocks);
      let elapsed = started.elapsed();
      let total_executions: u32 = diag.actor_cycle_counts.values().sum();
      let ms_per_block = (elapsed.as_secs_f64() * 1_000.0) / (blocks as f64);
      println!(
        "Actors scheduler profile: actors={}, blocks={}, elapsed_ms={:.3}, ms_per_block={:.4}, total_executions={}",
        actors,
        blocks,
        elapsed.as_secs_f64() * 1_000.0,
        ms_per_block,
        total_executions,
      );
    });
  }
}

#[test]
fn genesis_sparse_id_space_executes_only_active_actors() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // Genesis reserves IDs 0-14 as three active actors, ten dormant identities,
    // and two custody-only accounts. The gap after ID 14 stays empty until a
    // new actor is created.
    //
    // Ringless scheduler iterates ActiveActors BTreeSet directly,
    // so sparse IDs are handled efficiently — no scanning over empty slots.
    //
    // Direct test funding bypasses ingress notification. The three genesis
    // contracts must therefore remain idle while the explicit timer fixture runs.
    assert_eq!(Actors::active_actor_count(), 3);
    assert_eq!(Actors::actor_identity_count(), 13);
    let genesis_ids_all: alloc::vec::Vec<u64> =
      alloc::vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,];
    for &id in &genesis_ids_all {
      let sov = Actors::sovereign_account_id_system(id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    }
    // Dormant and custody identities own no executable contract.
    for id in [2, 4, 5, 6, 7, 8, 9, 11, 13, 14] {
      assert!(Actors::actor_identities(id).is_some());
      assert!(Actors::active_actor_state(id).is_none());
    }
    for id in [3, 12] {
      assert!(Actors::actor_identities(id).is_none());
      assert!(Actors::active_actor_state(id).is_none());
    }
    // Create a fresh actor at the current high end to extend the sparse space.
    let fresh_id = crate::Actors::next_actor_id();
    assert_eq!(fresh_id, 15);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    let sov_fresh = Actors::sovereign_account_id_system(fresh_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_fresh, initial_balance);
    let all_ids: alloc::vec::Vec<u64> = alloc::vec![fresh_id];
    // Block 2: only the explicit timer fixture fires.
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    run_idle(Weight::from_parts(u64::MAX, u64::MAX));
    System::set_block_number(3);
    System::reset_events();
    Actors::on_initialize(3);
    run_idle(Weight::from_parts(u64::MAX, u64::MAX));
    let mut executed_block_2: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    executed_block_2.dedup();
    for &id in &all_ids {
      assert!(
        executed_block_2.contains(&id),
        "Actors {} must execute in the first eligible block despite sparse ID gaps \
         (total_actors={}, id_space=0..{}, executed={:?})",
        id,
        all_ids.len(),
        crate::Actors::next_actor_id(),
        executed_block_2,
      );
    }
    for id in [0, 1, 10] {
      assert!(!executed_block_2.contains(&id));
    }
    // The fresh timer actor continues without causing work for ingress-driven
    // genesis contracts. Advance to block 13 to verify sparse-ID stability.
    let block = 13u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    run_idle(Weight::from_parts(u64::MAX, u64::MAX));
    run_next_idle(Weight::from_parts(u64::MAX, u64::MAX));
    let mut executed_block_13: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    executed_block_13.dedup();
    assert_eq!(executed_block_13, all_ids);
  });
}

#[test]
fn execution_order_lower_id_executes_before_higher_id() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // Actors-A (lower ID): transfers 10% of current NTVE to Actors-B sovereign
    let actor_a_id = crate::Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_a_id);
    let sov_a = Actors::sovereign_account_id_system(actor_a_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_a, initial_balance);
    // Actors-B (higher ID): transfers 10% of current NTVE to CHARLIE
    let actor_b_id = crate::Actors::next_actor_id();
    assert!(actor_b_id > actor_a_id, "B must have higher ID than A");
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_b_id);
    let sov_b = Actors::sovereign_account_id_system(actor_b_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_b, initial_balance);
    // Update Actors-A steps: Transfer 10% NTVE → Actors-B sovereign
    let pct = Perbill::from_percent(10);
    let contract_steps_a: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: sov_b.clone(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_a_id,
      (contract_steps_a, CompletionPolicy::Persistent,)
    ));
    // Update Actors-B steps: Transfer 10% NTVE → CHARLIE
    let contract_steps_b: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: CHARLIE,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_b_id,
      (contract_steps_b, CompletionPolicy::Persistent,)
    ));
    let charlie_before = Balances::free_balance(CHARLIE);
    // Materialize in block 2 and execute in the first eligible block.
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    run_idle(Weight::from_parts(u64::MAX, u64::MAX));
    System::set_block_number(3);
    System::reset_events();
    Actors::on_initialize(3);
    run_idle(Weight::from_parts(u64::MAX, u64::MAX));
    // If A executed before B: A transferred 10% to B, then B has initial + A's transfer,
    // and B transfers 10% of that total to CHARLIE.
    // If B executed before A: B transfers 10% of initial only, then A transfers to B.
    // We can distinguish by checking CHARLIE's balance.
    let minimum = crate::EXISTENTIAL_DEPOSIT;
    let a_transfer = pct.mul_floor(initial_balance.saturating_sub(minimum));
    let b_balance_after_a = initial_balance + a_transfer;
    let b_transfer_correct_order = pct.mul_floor(b_balance_after_a.saturating_sub(minimum));
    let b_transfer_wrong_order = pct.mul_floor(initial_balance.saturating_sub(minimum));
    let charlie_after = Balances::free_balance(CHARLIE);
    let charlie_received = charlie_after.saturating_sub(charlie_before);
    assert_eq!(
      charlie_received, b_transfer_correct_order,
      "Actors-A (id={}) must execute before Actors-B (id={}): \
       correct_order_transfer={}, wrong_order_transfer={}, actual={}",
      actor_a_id, actor_b_id, b_transfer_correct_order, b_transfer_wrong_order, charlie_received,
    );
    assert_ne!(
      b_transfer_correct_order, b_transfer_wrong_order,
      "Test must distinguish between execution orders"
    );
  });
}

// --- 10K Actors Stress Test ---

/// Validates the queue scheduler at production scale (10,000 active actors).
///
/// Runtime starts with genesis System Actors already occupying part of the active set.
/// This test fills the remaining capacity so ActiveActors reaches exactly 10,000,
/// then verifies starvation-freedom and fairness for newly added stress actors.
///
/// The configured execution ceiling and FIFO size determine the count-limited
/// rotation horizon; WeightMeter remains an independent limiter under finite budgets.
/// Nonce spread (max - min) must remain ≤ 3 for near-perfect fairness.
///
/// Acceptance criteria:
/// - ActiveActors reaches exactly 10,000
/// - Every stress actor executes at least once
/// - Nonce spread ≤ 2
/// - Zero deferrals (System Actors, Weight::MAX budget)
/// - Zero failed steps
#[test]
fn runtime_simulation_core_rolls_back_deos_adapter_effects() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = transfer_contract_steps(BOB, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT);
    let expected_contract = system_active_contract(manual_schedule(), None, steps.clone())
      .expect("system actor contract exists");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000 * crate::EXISTENTIAL_DEPOSIT);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    let actor_before = Actors::active_actor_state(actor_id)
      .expect("actor exists")
      .encode();
    let actor_balance_before = Balances::free_balance(&actor_account(actor_id));
    let bob_before = Balances::free_balance(BOB);
    let events_before = System::event_count();

    let result = Actors::simulate_current_contract(
      actor_id,
      pallet_deos_actors::ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready DEOS actor simulates");

    assert_eq!(result.status, AttemptDisposition::Completed);
    assert_eq!(result.cycle_nonce, 1);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].outcome, StepOutcome::Executed);
    assert_eq!(
      Actors::active_actor_state(actor_id).map(|state| state.encode()),
      Some(actor_before)
    );
    assert_eq!(Balances::free_balance(BOB), bob_before);
    assert_eq!(
      Balances::free_balance(&actor_account(actor_id)),
      actor_balance_before
    );
    assert_eq!(System::event_count(), events_before);
  });
}

#[test]
#[ignore = "10,000 effectful Actor production profile; run through actors-assurance"]
fn transfer_10k_manual_and_reactive_first_traversal() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actor_count = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    let initial_balance = 1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT);
    let mut actor_ids = Vec::with_capacity(actor_count as usize);
    for index in 0..actor_count {
      let trigger = if index % 2 == 0 {
        Trigger::manual()
      } else {
        Trigger::cadenced(1)
      };
      let actor_id = create_system(
        ALICE,
        RuntimeSchedule {
          trigger,
          cooldown_blocks: 0,
        },
        None,
        transfer_contract_steps(BOB, AssetKind::Native, 1),
      );
      fund_native(actor_id, initial_balance);
      if index % 2 == 0 {
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
      }
      actor_ids.push(actor_id);
    }
    assert_eq!(
      pallet_deos_actors::ActiveActorCount::<Runtime>::get(),
      actor_count
    );

    let mut progressed = alloc::collections::BTreeSet::new();
    let mut completion_block = None;
    for block in 2..=1_301u32 {
      System::set_block_number(block);
      System::reset_events();
      System::set_block_consumed_resources(Weight::zero(), 0);
      set_consensus_timestamp(
        u64::from(block).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
      );
      assert_eq!(Actors::on_initialize(block), Weight::zero());
      ensure_current_resource_state();
      Actors::on_idle(block, Weight::MAX);
      for event in System::events() {
        if let RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, outcomes, ..
        }) = event.event
        {
          assert_eq!(outcomes.failed_steps, 0);
          progressed.insert(actor_id);
        }
      }
      if progressed.len() == actor_ids.len() {
        completion_block = Some(block);
        break;
      }
    }
    assert_eq!(progressed.len(), actor_ids.len());
    let limits = crate::configs::BlockResourceBudgetValue::get().limits();
    println!(
      "CONTROL_SWEEP_W1 control={:?} shared={:?} actor_base={:?} user_base={:?} completion_block={:?}",
      limits.actor_control(),
      limits.shared_economic(),
      limits.actor_base_turn(),
      limits.user_base_turn(),
      completion_block,
    );
  });
}

#[test]
#[ignore = "10,000 effectful Actors with continuous user demand; run through actors-assurance"]
fn transfer_10k_first_traversal_with_continuous_user_dispatch() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actor_count = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    let initial_balance = 1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT);
    let mut actor_ids = Vec::with_capacity(actor_count as usize);
    for index in 0..actor_count {
      let actor_id = create_system(
        ALICE,
        RuntimeSchedule {
          trigger: if index % 2 == 0 {
            Trigger::manual()
          } else {
            Trigger::cadenced(1)
          },
          cooldown_blocks: 0,
        },
        None,
        transfer_contract_steps(BOB, AssetKind::Native, 1),
      );
      fund_native(actor_id, initial_balance);
      if index % 2 == 0 {
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
      }
      actor_ids.push(actor_id);
    }

    let signer_pair = sr25519::Pair::from_seed(&[58u8; 32]);
    let signer = AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&signer, 1_000_000_000_000_000_000);
    let mut progressed = alloc::collections::BTreeSet::new();
    let mut completion_block = None;
    for block in 2..=1_301u32 {
      System::set_block_number(block);
      System::reset_events();
      System::set_block_consumed_resources(Weight::zero(), 0);
      set_consensus_timestamp(
        u64::from(block).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
      );
      assert_eq!(Actors::on_initialize(block), Weight::zero());
      ensure_current_resource_state();
      let call = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
        remark: block.to_le_bytes().to_vec(),
      });
      let extrinsic = signed_extrinsic(&signer_pair, crate::Nonce::from(block - 2), call);
      let before_user_state = Actors::block_resource_state().expect("prepass state exists");
      let before_user_phase = before_user_state.phase();
      let before_user_outstanding = before_user_state.outstanding_reservations();
      let before_user_halted = before_user_state.optional_actor_work_halted();
      let before_user = before_user_state.usage();
      let declared = extrinsic.get_dispatch_info().total_weight();
      let applied = Executive::apply_extrinsic(extrinsic);
      assert!(
        applied.is_ok(),
        "user dispatch rejected in block {block}: phase={before_user_phase:?}, outstanding={before_user_outstanding}, halted={before_user_halted}, before={before_user:?}, declared={declared:?}, user_base={:?}, result={applied:?}",
        crate::configs::BlockResourceBudgetValue::get().limits().user_base_turn(),
      );
      let after_user = Actors::block_resource_state().expect("mixed block state exists");
      assert_ne!(after_user.usage().user_dispatch_used(), Weight::zero());
      Actors::on_idle(block, Weight::MAX);
      for event in System::events() {
        if let RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, outcomes, ..
        }) = event.event
        {
          assert_eq!(outcomes.failed_steps, 0);
          progressed.insert(actor_id);
        }
      }
      if progressed.len() == actor_ids.len() {
        completion_block = Some(block);
        break;
      }
    }
    let missing_manual = actor_ids
      .iter()
      .step_by(2)
      .filter(|actor_id| !progressed.contains(actor_id))
      .count();
    let missing_reactive = actor_ids
      .iter()
      .skip(1)
      .step_by(2)
      .filter(|actor_id| !progressed.contains(actor_id))
      .count();
    assert_eq!(
      progressed.len(),
      actor_ids.len(),
      "missing Manual={missing_manual}, reactive={missing_reactive}"
    );
    let limits = crate::configs::BlockResourceBudgetValue::get().limits();
    println!(
      "CONTROL_SWEEP control={:?} shared={:?} actor_base={:?} user_base={:?} completion_block={:?}",
      limits.actor_control(),
      limits.shared_economic(),
      limits.actor_base_turn(),
      limits.user_base_turn(),
      completion_block,
    );
  });
}

#[test]
#[ignore = "mixed 10,000-Actor production throughput profile"]
fn mixed_9500_transfer_400_swapout_100_control_first_traversal() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let dormant = pallet_deos_actors::ActorIdentities::<Runtime>::iter()
      .filter(|(actor_id, _)| !pallet_deos_actors::ActorHot::<Runtime>::contains_key(actor_id))
      .collect::<Vec<_>>();
    for (actor_id, identity) in &dormant {
      pallet_deos_actors::ActorIdentities::<Runtime>::remove(actor_id);
      pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
    }
    pallet_deos_actors::ActorIdentityCount::<Runtime>::mutate(|count| {
      *count = count.saturating_sub(dormant.len() as u32);
    });
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor_count = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    let initial_balance = 10_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT);
    let swap_steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(initial_balance),
      slippage_tolerance: Perbill::from_percent(5),
    })])
    .expect("one SwapOut Step fits");
    let control_steps = BoundedVec::try_from(vec![make_step(Task::StopCycle)])
      .expect("one control Step fits");
    let mut actor_ids = Vec::with_capacity(actor_count as usize);
    for index in 0..actor_count {
      let steps = if index < 9_500 {
        transfer_contract_steps(BOB, AssetKind::Native, 1)
      } else if index < 9_900 {
        swap_steps.clone()
      } else {
        control_steps.clone()
      };
      let actor_id = create_system(
        ALICE,
        RuntimeSchedule {
          trigger: if index % 2 == 0 {
            Trigger::manual()
          } else {
            Trigger::cadenced(1)
          },
          cooldown_blocks: 0,
        },
        None,
        steps,
      );
      fund_native(actor_id, initial_balance);
      if index % 2 == 0 {
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
      }
      actor_ids.push(actor_id);
    }

    let signer_pair = sr25519::Pair::from_seed(&[59u8; 32]);
    let signer = AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &signer,
      1_000_000_000_000_000_000,
    );
    let mut progressed = alloc::collections::BTreeSet::new();
    let mut failed_steps = 0u32;
    let mut completion_block = None;
    for block in 2..=1_301u32 {
      System::set_block_number(block);
      System::reset_events();
      System::set_block_consumed_resources(Weight::zero(), 0);
      set_consensus_timestamp(
        u64::from(block).saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
      );
      assert_eq!(Actors::on_initialize(block), Weight::zero());
      ensure_current_resource_state();
      let call = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
        remark: block.to_le_bytes().to_vec(),
      });
      let extrinsic = signed_extrinsic(&signer_pair, crate::Nonce::from(block - 2), call);
      assert!(Executive::apply_extrinsic(extrinsic).is_ok());
      Actors::on_idle(block, Weight::MAX);
      for event in System::events() {
        if let RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, outcomes, ..
        }) = event.event
        {
          failed_steps = failed_steps.saturating_add(outcomes.failed_steps);
          progressed.insert(actor_id);
        }
      }
      if progressed.len() == actor_ids.len() {
        completion_block = Some(block);
        break;
      }
    }
    assert_eq!(progressed.len(), actor_ids.len());
    assert!(
      completion_block.is_some_and(|block| block <= 1_301),
      "all 10,000 Actors must complete first traversal within the measured horizon"
    );
    assert_eq!(failed_steps, 0, "mixed traversal must lose no Step effect");
    let limits = crate::configs::BlockResourceBudgetValue::get().limits();
    println!(
      "CONTROL_SWEEP_W3 control={:?} shared={:?} actor_base={:?} user_base={:?} completion_block={:?} failed_steps={failed_steps}",
      limits.actor_control(),
      limits.shared_economic(),
      limits.actor_base_turn(),
      limits.user_base_turn(),
      completion_block,
    );
  });
}

#[test]
#[ignore] // ~30s wall-clock; run manually: cargo test --release stress_10k_actors_queue_scheduler -- --ignored
fn stress_10k_actors_queue_scheduler() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let num_blocks = 1_300u32;
    let initial_balance: u128 = 1_000 * crate::EXISTENTIAL_DEPOSIT;
    let max_active = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get() as u64;
    // Retain paused active genesis actors to validate mixed ready/non-ready fairness.
    // Remove dormant genesis identities so the identity cap does not prevent saturating
    // the independently asserted active-actor cap.
    let genesis_ids: alloc::vec::Vec<u64> = alloc::vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    for &id in &genesis_ids {
      let _ = Actors::pause_actor(RuntimeOrigin::root(), id);
    }
    let dormant: alloc::vec::Vec<_> = pallet_deos_actors::ActorIdentities::<Runtime>::iter()
      .filter(|(actor_id, _)| !pallet_deos_actors::ActorHot::<Runtime>::contains_key(actor_id))
      .collect();
    for (actor_id, identity) in &dormant {
      pallet_deos_actors::ActorIdentities::<Runtime>::remove(actor_id);
      pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
    }
    pallet_deos_actors::ActorIdentityCount::<Runtime>::mutate(|count| {
      *count = count.saturating_sub(dormant.len() as u32);
    });
    let active_before = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
    assert!(
      active_before < max_active,
      "Test precondition failed: active_before={} must be < max_active={}",
      active_before,
      max_active,
    );
    let actor_count = max_active - active_before;
    let actor_ids = setup_mixed_inert_actors(actor_count, initial_balance);
    assert_eq!(actor_ids.len(), actor_count as usize);
    let active_after = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
    assert_eq!(
      active_after, max_active,
      "ActiveActors must be saturated to max capacity",
    );
    let diag = run_blocks_with_diagnostics(&actor_ids, num_blocks, Weight::MAX);
    // All stress actors must execute at least once
    let zero_actors: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter(|&&id| *diag.actor_cycle_counts.get(&id).unwrap_or(&0) == 0)
      .copied()
      .collect();
    assert!(
      zero_actors.is_empty(),
      "Starvation: {} stress actors never executed (first 10: {:?})",
      zero_actors.len(),
      &zero_actors[..zero_actors.len().min(10)],
    );
    // User and System actors have materially different fee envelopes, so fairness is
    // assessed within each equally funded class rather than by comparing their throughput.
    for (parity, class_name) in [(0usize, "System"), (1usize, "User")] {
      let nonces: alloc::vec::Vec<u32> = actor_ids
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 2 == parity)
        .map(|(_, id)| *diag.actor_cycle_counts.get(id).unwrap_or(&0))
        .collect();
      let min_nonce = *nonces.iter().min().expect("class is nonempty");
      let max_nonce = *nonces.iter().max().expect("class is nonempty");
      let nonce_spread = max_nonce - min_nonce;
      assert!(
        nonce_spread <= 3,
        "{class_name} FIFO fairness spread {nonce_spread} exceeds 3 (min={min_nonce}, max={max_nonce})",
      );
    }
    // MaxExecutionsPerBlock is a count ceiling, not a throughput promise. The measured
    // two-dimensional Weight envelope controls actual service; saturation evidence therefore
    // requires bounded execution, complete first traversal, and fairness rather than utilization
    // against an unreachable count-only maximum.
    let execution_ceiling = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get();
    assert!(
      diag.max_per_block <= execution_ceiling,
      "Per-block executions {} exceeds MaxExecutionsPerBlock={execution_ceiling}",
      diag.max_per_block,
    );
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    assert!(
      u64::from(total_executions) >= actor_count,
      "Total executions {total_executions} must cover the complete {actor_count}-actor traversal",
    );
    assert_core_stability(&actor_ids, &diag);
  });
}

#[test]
fn dust_attack_min_balance_actors_preserve_scheduler_stability() {
  seeded_test_ext().execute_with(|| {
    let min_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let actor_count = 96u32;
    let baseline_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    let mut actor_ids = Vec::new();
    for i in 0..actor_count {
      let mut owner_bytes = [0u8; 32];
      owner_bytes[0] = (i & 0xFF) as u8;
      owner_bytes[31] = ((i + 17) & 0xFF) as u8;
      let owner = crate::AccountId::from(owner_bytes);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &owner,
        min_balance.saturating_mul(20),
      );
      let schedule = Schedule {
        trigger: Trigger::cadenced(1),
        cooldown_blocks: 0,
      };
      let actor_id = create_user(
        owner.clone(),
        schedule,
        None,
        transfer_contract_steps(owner, AssetKind::Native, 1),
      );
      let sovereign = actor_account(actor_id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &sovereign,
        min_balance.saturating_mul(10),
      );
      actor_ids.push(actor_id);
    }
    let initial_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    assert_eq!(initial_active, baseline_active + actor_count as usize);
    for block in 1..=32u32 {
      System::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let final_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    let progressed = actor_ids
      .iter()
      .filter(|id| {
        Actors::active_actor_state(**id)
          .map(|state| state.identity.cycle_nonce > 0)
          .unwrap_or(true)
      })
      .count();
    assert!(
      progressed > 0,
      "Scheduler should execute or terminally close at least some dust actors"
    );
    assert!(
      final_active > 0,
      "Dust load must not collapse scheduler to zero active actors"
    );
    assert!(
      final_active <= initial_active,
      "Active actors cannot increase without new creations"
    );
  });
}

#[test]
fn fee_ingress_accumulates_exactly_amount_never_double() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // A Mutable System actor with an accepting funding policy and a
    // Native-tracking plan: exactly one certified ingress notification must
    // accumulate exactly `amount`, never `2 * amount` from a duplicate
    // submission of the same movement.
    let tracking_plan =
      pallet_deos_actors::ContractSteps::<Runtime>::try_from(vec![pallet_deos_actors::Step {
        precondition: None,
        task: pallet_deos_actors::Task::Transfer {
          to: BOB,
          asset: AssetKind::Native,
          amount: pallet_deos_actors::AmountResolution::PercentageOfLastFunding(
            polkadot_sdk::sp_runtime::Perbill::from_percent(100),
          ),
        },
        on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
      }])
      .expect("tracking plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, tracking_plan);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let amount = crate::EXISTENTIAL_DEPOSIT.saturating_mul(3);
    let payer = BOB;
    let instance = Actors::active_actor_state(actor_id).expect("Actors exists");
    assert_ok!(<Balances as Currency<crate::AccountId>>::transfer(
      &payer,
      &instance.identity.sovereign_account,
      amount,
      polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
    ));
    // One certified notification (the exact ingress the FeeCollector emits).
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      amount,
      &payer,
    ));
    let funding = actor_funding(actor_id);
    let accumulated = funding
      .funding_accumulated
      .iter()
      .find(|(asset, _)| **asset == AssetKind::Native)
      .map(|(_, v)| *v)
      .unwrap_or(0);
    assert_eq!(
      accumulated, amount,
      "one certified ingress must accumulate exactly amount, never 2 * amount"
    );
  });
}

#[test]
fn fee_collector_charge_leaves_the_single_cadence_placement_unchanged() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let payer = BOB;
    assert_ok!(TmctolFeeCollector::collect_fee(
      &payer,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));
    let hot = Actors::actor_hot(fee_sink_id).expect("Fee Sink hot state");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(
      hot.trigger_wakeup_pointer.is_some(),
      "cadence remains the sole Trigger placement"
    );
  });
}

#[test]
fn fee_collector_noop_zero_emits_no_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let events_before = System::event_count();
    assert_ok!(TmctolFeeCollector::collect_fee(
      &BOB,
      &fee_sink,
      AssetKind::Native,
      0,
    ));
    assert_eq!(
      System::event_count(),
      events_before,
      "zero/no-op collection must emit no ingress events"
    );
    let hot = Actors::actor_hot(fee_sink_id).expect("Fee Sink remains active");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(
      hot.trigger_wakeup_pointer.is_some(),
      "cadence remains armed"
    );
  });
}

#[test]
fn user_actor_state_uses_the_dedicated_runtime_hold_reason_and_releases_exactly() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let record = Actors::actor_state_hold(actor_id).expect("User state hold exists");
    let expected = record
      .breakdown
      .identity
      .saturating_add(record.breakdown.contract_head)
      .saturating_add(record.breakdown.contract_body)
      .saturating_add(record.breakdown.detector)
      .saturating_add(record.breakdown.funding)
      .saturating_add(record.breakdown.run);
    let reason = RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState);
    assert_eq!(Balances::balance_on_hold(&reason, &ALICE), expected);

    System::set_block_number(2);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::actor_state_hold(actor_id).is_none());
    assert_eq!(Balances::balance_on_hold(&reason, &ALICE), 0);
  });
}

#[test]
fn actor_cost_runtime_api_exposes_named_deos_fee_and_hold_provenance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, AssetKind::Native, 1),
    );
    let quote = Actors::actor_cost_quote(actor_id).expect("runtime cost quote exists");
    assert_eq!(quote.actor_type, pallet_deos_actors::ActorType::User);
    assert_eq!(
      quote.creation_fee,
      <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get()
    );
    let trigger = quote
      .prospective_trigger_fee
      .expect("Manual Trigger quote exists");
    assert_eq!(
      trigger.trigger_family,
      pallet_deos_actors::TriggerFamily::Manual
    );
    assert!(trigger.maximum_weight.ref_time() > 0);
    assert!(trigger.fee > 0);
    let pipeline = quote
      .prospective_pipeline_fee
      .expect("Pipeline quote exists");
    assert_eq!(
      pipeline.strategy,
      pallet_deos_actors::PipelineMachineFeeStrategy::UpfrontBounded
    );
    assert_eq!(
      pipeline.total_fee,
      pipeline.pipeline_machine_fee + pipeline.cleanup_fee
    );
    assert!(quote.maximum_next_action_fee.maximum_effect_fee > 0);
    assert!(!quote.actor_state_hold.exempt);
    assert_eq!(
      quote.actor_state_hold.base_per_component,
      <Runtime as pallet_deos_actors::Config>::ActorStateHoldBase::get()
    );
    assert_eq!(
      quote.actor_state_hold.per_encoded_byte,
      <Runtime as pallet_deos_actors::Config>::ActorStateHoldPerByte::get()
    );
    assert_eq!(
      quote.actor_state_hold.total,
      actor_state_hold_total(actor_id)
    );
  });
}

#[test]
fn eligibility_projection_binds_genesis_actors_and_signal_readiness() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;

    let missing = Actors::actor_eligibility(primitives::ecosystem::actor_ids::BURN_ACTOR_ID + 1000)
      .expect("projection computes");
    assert_eq!(missing, pallet_deos_actors::ActorEligibility::NotRegistered);

    let idle = Actors::actor_eligibility(fee_sink_id).expect("projection computes");
    assert!(matches!(
      idle,
      pallet_deos_actors::ActorEligibility::Active(pallet_deos_actors::ActiveActorActivation {
        eligibility: pallet_deos_actors::ActorClassification {
          terminal_reason: None,
          execution_phase: pallet_deos_actors::ActorExecutionPhase::WaitingCadenceTick(_),
        },
        ..
      })
    ));

    fund_native_via_call(BOB, fee_sink_id, 1_000);
    let funded = Actors::actor_eligibility(fee_sink_id).expect("projection computes");
    assert!(matches!(
      funded,
      pallet_deos_actors::ActorEligibility::Active(pallet_deos_actors::ActiveActorActivation {
        eligibility: pallet_deos_actors::ActorClassification {
          terminal_reason: None,
          execution_phase: pallet_deos_actors::ActorExecutionPhase::WaitingCadenceTick(_),
        },
        ..
      })
    ));
  });
}
