use crate::types::{AmountResolution, Condition, ConditionSet, Mutability, StepErrorPolicy, Task};
use crate::{RetryClass, TaskWeightInfo};
use alloc::vec::Vec;
use frame::prelude::*;

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum AdapterRequirement {
  None,
  AssetOps,
  DexOps,
  StakingOps,
  LiquidityOps,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum EffectClass {
  Transfer,
  SupplyBurn,
  SupplyMint,
  LiquidityMutation,
  StakingMutation,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ActorAvailability {
  UserAndSystem,
  SystemOnly,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum TaskWeightOwner {
  Transfer,
  SplitTransfer,
  Burn,
  Mint,
  DexSwapIn,
  DexSwapOut,
  AddLiquidity,
  RemoveLiquidity,
  Stake,
  DonateLiquidity,
  Unstake,
  StopCycle,
}

impl TaskWeightOwner {
  pub fn weight<W: TaskWeightInfo>(self, split_legs: u32) -> polkadot_sdk::sp_weights::Weight {
    match self {
      Self::Transfer => W::transfer(),
      Self::SplitTransfer => W::split_transfer(split_legs),
      Self::Burn => W::burn(),
      Self::Mint => W::mint(),
      Self::DexSwapIn => W::dex_exact_in(),
      Self::DexSwapOut => W::dex_exact_out(),
      Self::AddLiquidity => W::add_liquidity(),
      Self::RemoveLiquidity => W::remove_liquidity(),
      Self::Stake => W::stake(),
      Self::DonateLiquidity => W::donate_liquidity(),
      Self::Unstake => W::unstake(),
      Self::StopCycle => W::stop_cycle(),
    }
  }
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum BoundedInternalAlgorithm {
  None,
  PalletSplitFanout,
  RuntimeAdapterContract,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum RecipientSurface<AccountId> {
  ActorSovereign,
  Explicit(AccountId),
  AdapterDerived,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum TaskAmountRole {
  Amount,
  AmountIn,
  AmountOut,
  AmountA,
  AmountB,
  LpAmount,
  MaxAmountA,
  Shares,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct TaskAmountSurface {
  pub role: TaskAmountRole,
  pub contract: AmountInstructionContract,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct TaskInstructionContract<AssetId, AccountId> {
  pub required_adapter: AdapterRequirement,
  pub assets_read: Vec<AssetId>,
  pub assets_written: Vec<AssetId>,
  pub reads_adapter_derived_assets: bool,
  pub writes_adapter_derived_assets: bool,
  pub recipients: Vec<RecipientSurface<AccountId>>,
  pub effects: Vec<EffectClass>,
  pub availability: ActorAvailability,
  pub committed_non_compensated_effects: bool,
  pub successful_control: ClassifiedStepControl,
  pub weight_owner: TaskWeightOwner,
  pub bounded_internal_algorithm: BoundedInternalAlgorithm,
  pub amount_surfaces: Vec<TaskAmountSurface>,
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
  if !values.contains(&value) {
    values.push(value);
  }
}

fn task_amount_surfaces<AssetId, Balance, AccountId, MaxSplitTransferLegs>(
  task: &Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
) -> Vec<TaskAmountSurface>
where
  MaxSplitTransferLegs: Get<u32>,
{
  let surface = |role, amount| TaskAmountSurface {
    role,
    contract: describe_amount_resolution(amount),
  };
  match task {
    Task::Transfer { amount, .. }
    | Task::SplitTransfer { amount, .. }
    | Task::Burn { amount, .. }
    | Task::Mint { amount, .. }
    | Task::Stake { amount, .. } => {
      alloc::vec![surface(TaskAmountRole::Amount, amount)]
    }
    Task::DonateLiquidity { max_amount_a, .. } => {
      alloc::vec![surface(TaskAmountRole::MaxAmountA, max_amount_a)]
    }
    Task::RemoveLiquidity { lp_amount, .. } => {
      alloc::vec![surface(TaskAmountRole::LpAmount, lp_amount)]
    }
    Task::SwapIn { amount_in, .. } => {
      alloc::vec![surface(TaskAmountRole::AmountIn, amount_in)]
    }
    Task::SwapOut { amount_out, .. } => {
      alloc::vec![surface(TaskAmountRole::AmountOut, amount_out)]
    }
    Task::AddLiquidity {
      amount_a, amount_b, ..
    } => alloc::vec![
      surface(TaskAmountRole::AmountA, amount_a),
      surface(TaskAmountRole::AmountB, amount_b),
    ],
    Task::Unstake { shares, .. } => alloc::vec![surface(TaskAmountRole::Shares, shares)],
    Task::StopCycle => alloc::vec![],
  }
}

pub fn describe_task<AssetId, Balance, AccountId, MaxSplitTransferLegs>(
  task: &Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
) -> TaskInstructionContract<AssetId, AccountId>
where
  AssetId: Clone + PartialEq,
  AccountId: Clone,
  MaxSplitTransferLegs: Get<u32>,
{
  let mut assets_read = Vec::new();
  let mut assets_written = Vec::new();
  let mut recipients = Vec::new();
  let (
    required_adapter,
    effects,
    availability,
    weight_owner,
    bounded_internal_algorithm,
    reads_adapter_derived_assets,
    writes_adapter_derived_assets,
  ) = match task {
    Task::Transfer { to, asset, .. } => {
      push_unique(&mut assets_read, asset.clone());
      push_unique(&mut assets_written, asset.clone());
      recipients.push(RecipientSurface::Explicit(to.clone()));
      (
        AdapterRequirement::AssetOps,
        alloc::vec![EffectClass::Transfer],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::Transfer,
        BoundedInternalAlgorithm::None,
        false,
        false,
      )
    }
    Task::SplitTransfer { asset, legs, .. } => {
      push_unique(&mut assets_read, asset.clone());
      push_unique(&mut assets_written, asset.clone());
      for leg in legs {
        recipients.push(RecipientSurface::Explicit(leg.to.clone()));
      }
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::AssetOps,
        alloc::vec![EffectClass::Transfer],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::SplitTransfer,
        BoundedInternalAlgorithm::PalletSplitFanout,
        false,
        false,
      )
    }
    Task::SwapIn {
      asset_in,
      asset_out,
      ..
    } => {
      push_unique(&mut assets_read, asset_in.clone());
      push_unique(&mut assets_read, asset_out.clone());
      push_unique(&mut assets_written, asset_in.clone());
      push_unique(&mut assets_written, asset_out.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::DexOps,
        alloc::vec![EffectClass::Transfer, EffectClass::LiquidityMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::DexSwapIn,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        false,
      )
    }
    Task::SwapOut {
      asset_in,
      asset_out,
      ..
    } => {
      push_unique(&mut assets_read, asset_in.clone());
      push_unique(&mut assets_read, asset_out.clone());
      push_unique(&mut assets_written, asset_in.clone());
      push_unique(&mut assets_written, asset_out.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::DexOps,
        alloc::vec![EffectClass::Transfer, EffectClass::LiquidityMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::DexSwapOut,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        false,
      )
    }
    Task::AddLiquidity {
      asset_a, asset_b, ..
    } => {
      push_unique(&mut assets_read, asset_a.clone());
      push_unique(&mut assets_read, asset_b.clone());
      push_unique(&mut assets_written, asset_a.clone());
      push_unique(&mut assets_written, asset_b.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::LiquidityOps,
        alloc::vec![EffectClass::LiquidityMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::AddLiquidity,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        true,
      )
    }
    Task::RemoveLiquidity { lp_asset, .. } => {
      push_unique(&mut assets_read, lp_asset.clone());
      push_unique(&mut assets_written, lp_asset.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::LiquidityOps,
        alloc::vec![EffectClass::LiquidityMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::RemoveLiquidity,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        true,
      )
    }
    Task::Burn { asset, .. } => {
      push_unique(&mut assets_read, asset.clone());
      push_unique(&mut assets_written, asset.clone());
      (
        AdapterRequirement::AssetOps,
        alloc::vec![EffectClass::SupplyBurn],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::Burn,
        BoundedInternalAlgorithm::None,
        false,
        false,
      )
    }
    Task::Mint { asset, .. } => {
      push_unique(&mut assets_written, asset.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::AssetOps,
        alloc::vec![EffectClass::SupplyMint],
        ActorAvailability::SystemOnly,
        TaskWeightOwner::Mint,
        BoundedInternalAlgorithm::None,
        false,
        false,
      )
    }
    Task::Stake { asset, .. } => {
      push_unique(&mut assets_read, asset.clone());
      push_unique(&mut assets_written, asset.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::StakingOps,
        alloc::vec![EffectClass::StakingMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::Stake,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        true,
      )
    }
    Task::DonateLiquidity {
      asset_a, asset_b, ..
    } => {
      push_unique(&mut assets_read, asset_a.clone());
      push_unique(&mut assets_read, asset_b.clone());
      push_unique(&mut assets_written, asset_a.clone());
      push_unique(&mut assets_written, asset_b.clone());
      recipients.push(RecipientSurface::AdapterDerived);
      (
        AdapterRequirement::LiquidityOps,
        alloc::vec![EffectClass::LiquidityMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::DonateLiquidity,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        false,
        false,
      )
    }
    Task::Unstake { asset, .. } => {
      push_unique(&mut assets_read, asset.clone());
      push_unique(&mut assets_written, asset.clone());
      recipients.push(RecipientSurface::ActorSovereign);
      (
        AdapterRequirement::StakingOps,
        alloc::vec![EffectClass::StakingMutation],
        ActorAvailability::UserAndSystem,
        TaskWeightOwner::Unstake,
        BoundedInternalAlgorithm::RuntimeAdapterContract,
        true,
        true,
      )
    }
    Task::StopCycle => (
      AdapterRequirement::None,
      alloc::vec![],
      ActorAvailability::UserAndSystem,
      TaskWeightOwner::StopCycle,
      BoundedInternalAlgorithm::None,
      false,
      false,
    ),
  };
  TaskInstructionContract {
    required_adapter,
    assets_read,
    assets_written,
    reads_adapter_derived_assets,
    writes_adapter_derived_assets,
    recipients,
    effects,
    availability,
    committed_non_compensated_effects: !matches!(task, Task::StopCycle),
    successful_control: if matches!(task, Task::StopCycle) {
      ClassifiedStepControl::CompleteCycle
    } else {
      ClassifiedStepControl::Advance
    },
    weight_owner,
    bounded_internal_algorithm,
    amount_surfaces: task_amount_surfaces(task),
  }
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ConditionObservation {
  BalanceComparison,
  BlockNumberComparison,
  ScalarObservationComparison,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ConditionReadSurface<AssetId, ObservationFeedId = ()> {
  SpendableAssetBalance(AssetId),
  CurrentBlockNumber,
  TypedObservation {
    feed: ObservationFeedId,
    max_age_blocks: u32,
  },
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ObservationWindow {
  ArtifactTime,
  LogicalRunStart,
  StepAttemptTime,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ConditionInstructionContract<AssetId, ObservationFeedId = ()> {
  pub observation: ConditionObservation,
  pub read_surface: ConditionReadSurface<AssetId, ObservationFeedId>,
  pub pure: bool,
  pub observation_window: ObservationWindow,
  pub bounded_read_count: u32,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ConditionAggregateMode {
  Always,
  All,
  Any,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ConditionSetInstructionContract {
  pub mode: ConditionAggregateMode,
  pub atomic_count: u32,
  pub canonical_non_empty: bool,
  pub evaluates_all_atoms: bool,
  pub atomic_error_fails_group: bool,
  pub false_control: ClassifiedStepControl,
  pub admitted_task_count: u32,
  pub nested_groups: bool,
}

pub fn describe_condition_set<C, MaxConditions: Get<u32>>(
  condition_set: &ConditionSet<C, MaxConditions>,
) -> ConditionSetInstructionContract {
  let (mode, atomic_count, canonical_non_empty) = match condition_set {
    ConditionSet::Always => (ConditionAggregateMode::Always, 0, true),
    ConditionSet::All(conditions) => (
      ConditionAggregateMode::All,
      conditions.len() as u32,
      !conditions.is_empty(),
    ),
    ConditionSet::Any(conditions) => (
      ConditionAggregateMode::Any,
      conditions.len() as u32,
      !conditions.is_empty(),
    ),
  };
  ConditionSetInstructionContract {
    mode,
    atomic_count,
    canonical_non_empty,
    evaluates_all_atoms: true,
    atomic_error_fails_group: true,
    false_control: ClassifiedStepControl::Advance,
    admitted_task_count: 1,
    nested_groups: false,
  }
}

pub fn describe_condition<AssetId: Clone, Balance, BlockNumber, ObservationFeedId: Clone>(
  condition: &Condition<AssetId, Balance, BlockNumber, ObservationFeedId>,
) -> ConditionInstructionContract<AssetId, ObservationFeedId> {
  let (observation, read_surface) = match condition {
    Condition::BalanceAbove { asset, .. }
    | Condition::BalanceBelow { asset, .. }
    | Condition::BalanceEquals { asset, .. }
    | Condition::BalanceNotEquals { asset, .. } => (
      ConditionObservation::BalanceComparison,
      ConditionReadSurface::SpendableAssetBalance(asset.clone()),
    ),
    Condition::BlockNumberAbove { .. } | Condition::BlockNumberBelow { .. } => (
      ConditionObservation::BlockNumberComparison,
      ConditionReadSurface::CurrentBlockNumber,
    ),
    Condition::ObservationAbove {
      feed,
      max_age_blocks,
      ..
    }
    | Condition::ObservationBelow {
      feed,
      max_age_blocks,
      ..
    }
    | Condition::ObservationEquals {
      feed,
      max_age_blocks,
      ..
    }
    | Condition::ObservationNotEquals {
      feed,
      max_age_blocks,
      ..
    } => (
      ConditionObservation::ScalarObservationComparison,
      ConditionReadSurface::TypedObservation {
        feed: feed.clone(),
        max_age_blocks: *max_age_blocks,
      },
    ),
  };
  ConditionInstructionContract {
    observation,
    read_surface,
    pure: true,
    observation_window: ObservationWindow::StepAttemptTime,
    bounded_read_count: 1,
  }
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum AmountDataDependency {
  ArtifactValue,
  CurrentBalanceOrShares,
  TriggerSnapshot,
  LastFundingSnapshot,
  TaskPolicyCapacity,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ContextDependency {
  None,
  TaskPolicy,
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum RetryObservation {
  ReobserveLiveValue,
  ReuseFrozenValueWithLiveCapacity,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct AmountInstructionContract {
  pub data_dependencies: Vec<AmountDataDependency>,
  pub minimum_balance_dependency: ContextDependency,
  pub fee_reserve_dependency: ContextDependency,
  pub value_observation_window: ObservationWindow,
  pub retry_observation: RetryObservation,
}

pub fn describe_amount_resolution<Balance>(
  amount: &AmountResolution<Balance>,
) -> AmountInstructionContract {
  let (primary_dependency, value_observation_window, retry_observation) = match amount {
    AmountResolution::Fixed(_) => (
      AmountDataDependency::ArtifactValue,
      ObservationWindow::ArtifactTime,
      RetryObservation::ReuseFrozenValueWithLiveCapacity,
    ),
    AmountResolution::PercentageOfCurrent(_) | AmountResolution::AllBalance => (
      AmountDataDependency::CurrentBalanceOrShares,
      ObservationWindow::StepAttemptTime,
      RetryObservation::ReobserveLiveValue,
    ),
    AmountResolution::PercentageOfTrigger(_) => (
      AmountDataDependency::TriggerSnapshot,
      ObservationWindow::LogicalRunStart,
      RetryObservation::ReuseFrozenValueWithLiveCapacity,
    ),
    AmountResolution::PercentageOfLastFunding(_) => (
      AmountDataDependency::LastFundingSnapshot,
      ObservationWindow::LogicalRunStart,
      RetryObservation::ReuseFrozenValueWithLiveCapacity,
    ),
  };
  AmountInstructionContract {
    data_dependencies: alloc::vec![primary_dependency, AmountDataDependency::TaskPolicyCapacity],
    minimum_balance_dependency: ContextDependency::TaskPolicy,
    fee_reserve_dependency: ContextDependency::TaskPolicy,
    value_observation_window,
    retry_observation,
  }
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub enum ClassifiedStepControl {
  Advance,
  CompleteCycle,
  Terminate,
  SuspendCurrent,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ErrorPolicyInstructionContract {
  pub possible_controls: Vec<ClassifiedStepControl>,
  pub suspension_mutability_requirement: Option<Mutability>,
  pub suspension_failure_requirement: Option<RetryClass>,
}

pub fn describe_error_policy(policy: StepErrorPolicy) -> ErrorPolicyInstructionContract {
  match policy {
    StepErrorPolicy::ContinueNextStep => ErrorPolicyInstructionContract {
      possible_controls: alloc::vec![ClassifiedStepControl::Advance],
      suspension_mutability_requirement: None,
      suspension_failure_requirement: None,
    },
    StepErrorPolicy::AbortCycle => ErrorPolicyInstructionContract {
      possible_controls: alloc::vec![
        ClassifiedStepControl::Advance,
        ClassifiedStepControl::Terminate,
      ],
      suspension_mutability_requirement: None,
      suspension_failure_requirement: None,
    },
    StepErrorPolicy::RetryLater { .. } => ErrorPolicyInstructionContract {
      possible_controls: alloc::vec![
        ClassifiedStepControl::Advance,
        ClassifiedStepControl::Terminate,
        ClassifiedStepControl::SuspendCurrent,
      ],
      suspension_mutability_requirement: Some(Mutability::Mutable),
      suspension_failure_requirement: Some(RetryClass::Temporary),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::SplitLeg;
  use polkadot_sdk::frame_support::traits::ConstU32;
  use polkadot_sdk::sp_runtime::Perbill;

  type TestTask = Task<u32, u128, u64, ConstU32<8>>;

  fn fixed() -> AmountResolution<u128> {
    AmountResolution::Fixed(10)
  }

  #[test]
  fn every_task_has_one_exhaustive_semantic_contract() {
    let legs = BoundedVec::try_from(alloc::vec![
      SplitLeg {
        to: 7,
        share: Perbill::from_percent(40),
      },
      SplitLeg {
        to: 8,
        share: Perbill::from_percent(60),
      },
    ])
    .expect("two legs fit");
    let cases: Vec<(TestTask, AdapterRequirement, TaskWeightOwner)> = alloc::vec![
      (
        Task::Transfer {
          to: 7,
          asset: 1,
          amount: fixed(),
        },
        AdapterRequirement::AssetOps,
        TaskWeightOwner::Transfer,
      ),
      (
        Task::SplitTransfer {
          asset: 1,
          amount: fixed(),
          legs,
        },
        AdapterRequirement::AssetOps,
        TaskWeightOwner::SplitTransfer,
      ),
      (
        Task::SwapIn {
          asset_in: 1,
          amount_in: fixed(),
          asset_out: 2,
          slippage_tolerance: Perbill::zero(),
        },
        AdapterRequirement::DexOps,
        TaskWeightOwner::DexSwapIn,
      ),
      (
        Task::SwapOut {
          asset_out: 2,
          amount_out: fixed(),
          asset_in: 1,
          input_limit: crate::types::InputLimit::Absolute(100),
          slippage_tolerance: Perbill::zero(),
        },
        AdapterRequirement::DexOps,
        TaskWeightOwner::DexSwapOut,
      ),
      (
        Task::AddLiquidity {
          asset_a: 1,
          asset_b: 2,
          amount_a: fixed(),
          amount_b: fixed(),
          min_lp_out: 1,
        },
        AdapterRequirement::LiquidityOps,
        TaskWeightOwner::AddLiquidity,
      ),
      (
        Task::RemoveLiquidity {
          lp_asset: 3,
          asset_a: 1,
          asset_b: 2,
          lp_amount: fixed(),
          min_amount_a: 1,
          min_amount_b: 1,
        },
        AdapterRequirement::LiquidityOps,
        TaskWeightOwner::RemoveLiquidity,
      ),
      (
        Task::Burn {
          asset: 1,
          amount: fixed(),
        },
        AdapterRequirement::AssetOps,
        TaskWeightOwner::Burn,
      ),
      (
        Task::Mint {
          asset: 1,
          amount: fixed(),
        },
        AdapterRequirement::AssetOps,
        TaskWeightOwner::Mint,
      ),
      (
        Task::Stake {
          asset: 1,
          amount: fixed(),
        },
        AdapterRequirement::StakingOps,
        TaskWeightOwner::Stake,
      ),
      (
        Task::DonateLiquidity {
          asset_a: 1,
          asset_b: 2,
          max_amount_a: fixed(),
          max_ratio_error: Perbill::zero(),
        },
        AdapterRequirement::LiquidityOps,
        TaskWeightOwner::DonateLiquidity,
      ),
      (
        Task::Unstake {
          asset: 1,
          shares: fixed(),
        },
        AdapterRequirement::StakingOps,
        TaskWeightOwner::Unstake,
      ),
      (
        Task::StopCycle,
        AdapterRequirement::None,
        TaskWeightOwner::StopCycle,
      ),
    ];
    assert_eq!(cases.len(), 12);
    for (task, adapter, weight_owner) in cases {
      let is_stop = matches!(&task, Task::StopCycle);
      let contract = describe_task(&task);
      assert_eq!(contract.required_adapter, adapter);
      assert_eq!(contract.weight_owner, weight_owner);
      assert_eq!(contract.effects.is_empty(), is_stop);
      assert_eq!(contract.committed_non_compensated_effects, !is_stop);
      let expected_amount_roles = match weight_owner {
        TaskWeightOwner::Transfer
        | TaskWeightOwner::SplitTransfer
        | TaskWeightOwner::Burn
        | TaskWeightOwner::Mint
        | TaskWeightOwner::Stake => alloc::vec![TaskAmountRole::Amount],
        TaskWeightOwner::DonateLiquidity => alloc::vec![TaskAmountRole::MaxAmountA],
        TaskWeightOwner::RemoveLiquidity => alloc::vec![TaskAmountRole::LpAmount],
        TaskWeightOwner::DexSwapIn => alloc::vec![TaskAmountRole::AmountIn],
        TaskWeightOwner::DexSwapOut => alloc::vec![TaskAmountRole::AmountOut],
        TaskWeightOwner::AddLiquidity => {
          alloc::vec![TaskAmountRole::AmountA, TaskAmountRole::AmountB]
        }
        TaskWeightOwner::Unstake => alloc::vec![TaskAmountRole::Shares],
        TaskWeightOwner::StopCycle => alloc::vec![],
      };
      assert_eq!(
        contract
          .amount_surfaces
          .into_iter()
          .map(|surface| surface.role)
          .collect::<Vec<_>>(),
        expected_amount_roles,
      );
      assert_eq!(
        contract.successful_control,
        if is_stop {
          ClassifiedStepControl::CompleteCycle
        } else {
          ClassifiedStepControl::Advance
        }
      );
    }
  }

  #[test]
  fn condition_set_contract_forbids_nested_or_dynamic_control() {
    type TestCondition = Condition<u32, u128, u32>;
    type TestConditionSet = ConditionSet<TestCondition, ConstU32<4>>;
    let atom = Condition::BlockNumberAbove { threshold: 1 };
    let grouped = BoundedVec::try_from(alloc::vec![atom]).expect("one atom fits");
    let cases: alloc::vec::Vec<TestConditionSet> = alloc::vec![
      ConditionSet::Always,
      ConditionSet::All(grouped.clone()),
      ConditionSet::Any(grouped),
    ];
    for (expected_mode, condition_set) in [
      ConditionAggregateMode::Always,
      ConditionAggregateMode::All,
      ConditionAggregateMode::Any,
    ]
    .into_iter()
    .zip(cases)
    {
      let contract = describe_condition_set(&condition_set);
      assert_eq!(contract.mode, expected_mode);
      assert!(contract.canonical_non_empty);
      assert!(contract.evaluates_all_atoms);
      assert!(contract.atomic_error_fails_group);
      assert_eq!(contract.false_control, ClassifiedStepControl::Advance);
      assert_eq!(contract.admitted_task_count, 1);
      assert!(!contract.nested_groups);
    }
    let empty = TestConditionSet::Any(BoundedVec::default());
    assert!(!describe_condition_set(&empty).canonical_non_empty);
  }

  #[test]
  fn every_condition_is_pure_and_bounded() {
    let conditions: [Condition<u32, u128, u32, u32>; 10] = [
      Condition::BalanceAbove {
        asset: 1u32,
        threshold: 1u128,
      },
      Condition::BalanceBelow {
        asset: 1,
        threshold: 1,
      },
      Condition::BalanceEquals {
        asset: 1,
        threshold: 1,
      },
      Condition::BalanceNotEquals {
        asset: 1,
        threshold: 1,
      },
      Condition::BlockNumberAbove { threshold: 1u32 },
      Condition::BlockNumberBelow { threshold: 1u32 },
      Condition::ObservationAbove {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
      Condition::ObservationBelow {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
      Condition::ObservationEquals {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
      Condition::ObservationNotEquals {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
    ];
    for condition in conditions {
      let contract = describe_condition(&condition);
      assert!(contract.pure);
      assert_eq!(
        contract.observation_window,
        ObservationWindow::StepAttemptTime
      );
      assert_eq!(contract.bounded_read_count, 1);
    }
  }

  #[test]
  fn every_amount_resolution_classifies_retry_observation() {
    let cases = [
      AmountResolution::Fixed(1u128),
      AmountResolution::PercentageOfCurrent(Perbill::one()),
      AmountResolution::PercentageOfTrigger(Perbill::one()),
      AmountResolution::PercentageOfLastFunding(Perbill::one()),
      AmountResolution::AllBalance,
    ];
    for amount in cases {
      let contract = describe_amount_resolution(&amount);
      assert!(
        contract
          .data_dependencies
          .contains(&AmountDataDependency::TaskPolicyCapacity)
      );
      assert_eq!(
        contract.minimum_balance_dependency,
        ContextDependency::TaskPolicy
      );
      assert_eq!(
        contract.fee_reserve_dependency,
        ContextDependency::TaskPolicy
      );
    }
  }

  #[test]
  fn every_error_policy_classifies_possible_control() {
    let continue_contract = describe_error_policy(StepErrorPolicy::ContinueNextStep);
    assert_eq!(
      continue_contract.possible_controls,
      alloc::vec![ClassifiedStepControl::Advance],
    );
    let abort_contract = describe_error_policy(StepErrorPolicy::AbortCycle);
    assert_eq!(
      abort_contract.possible_controls,
      alloc::vec![
        ClassifiedStepControl::Advance,
        ClassifiedStepControl::Terminate,
      ],
    );
    let retry_contract = describe_error_policy(StepErrorPolicy::RetryLater { max_attempts: 3 });
    assert_eq!(
      retry_contract.suspension_mutability_requirement,
      Some(Mutability::Mutable),
    );
    assert_eq!(
      retry_contract.suspension_failure_requirement,
      Some(RetryClass::Temporary),
    );
  }
}
