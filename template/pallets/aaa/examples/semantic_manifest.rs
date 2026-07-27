use pallet_aaa::contract::{
  ActorAvailability, AdapterRequirement, AmountDataDependency, BoundedInternalAlgorithm,
  ClassifiedStepControl, ConditionInstructionContract, ConditionObservation, ConditionReadSurface,
  ContextDependency, EffectClass, ObservationWindow, RecipientSurface, RetryObservation,
  TaskAmountRole, TaskInstructionContract, TaskWeightOwner, describe_amount_resolution,
  describe_condition, describe_task,
};
use pallet_aaa::{AmountResolution, Condition, InputLimit, SplitLeg, Task};
use polkadot_sdk::frame_support::{BoundedVec, traits::ConstU32};
use polkadot_sdk::sp_runtime::Perbill;
use scale_info::{TypeDef, TypeInfo};
use serde::Serialize;
use std::{env, fs, path::Path};

type ManifestTask = Task<u32, u128, u64, ConstU32<8>>;
type ManifestCondition = Condition<u32, u128, u32, u32>;
type Contract = TaskInstructionContract<u32, u64>;
type ConditionContract = ConditionInstructionContract<u32, u32>;

const ASSET: u32 = 11;
const ASSET_IN: u32 = 12;
const ASSET_OUT: u32 = 13;
const ASSET_A: u32 = 14;
const ASSET_B: u32 = 15;
const LP_ASSET: u32 = 16;
const TO: u64 = 21;
const LEG_A: u64 = 22;
const LEG_B: u64 = 23;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticManifest {
  format: &'static str,
  format_version: u32,
  tasks: Vec<TaskManifest>,
  conditions: Vec<ConditionManifest>,
  amount_resolutions: Vec<AmountResolutionManifest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskManifest {
  task: &'static str,
  required_adapter: &'static str,
  assets_read: Vec<&'static str>,
  assets_written: Vec<&'static str>,
  reads_adapter_derived_assets: bool,
  writes_adapter_derived_assets: bool,
  recipients: Vec<RecipientManifest>,
  effects: Vec<&'static str>,
  availability: &'static str,
  committed_non_compensated_effects: bool,
  successful_control: &'static str,
  weight_owner: &'static str,
  bounded_internal_algorithm: &'static str,
  amount_surfaces: Vec<AmountSurfaceManifest>,
}

#[derive(Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
enum RecipientManifest {
  ActorSovereign,
  Explicit { path: &'static str },
  AdapterDerived,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AmountSurfaceManifest {
  role: &'static str,
  path: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConditionManifest {
  condition: &'static str,
  scale_index: u8,
  observation: &'static str,
  read_surface: ConditionReadSurfaceManifest,
  pure: bool,
  observation_window: &'static str,
  bounded_read_count: u32,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
enum ConditionReadSurfaceManifest {
  SpendableAssetBalance {
    path: &'static str,
  },
  CurrentBlockNumber,
  TypedObservation {
    feed_path: &'static str,
    max_age_blocks_path: &'static str,
  },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AmountResolutionManifest {
  resolution: &'static str,
  data_dependencies: Vec<&'static str>,
  minimum_balance_dependency: &'static str,
  fee_reserve_dependency: &'static str,
  value_observation_window: &'static str,
  retry_observation: &'static str,
}

fn fixed() -> AmountResolution<u128> {
  AmountResolution::Fixed(10)
}

fn amount_resolution_cases() -> Vec<(&'static str, AmountResolution<u128>)> {
  vec![
    ("Fixed", AmountResolution::Fixed(10)),
    (
      "PercentageOfCurrent",
      AmountResolution::PercentageOfCurrent(Perbill::one()),
    ),
    (
      "PercentageOfTrigger",
      AmountResolution::PercentageOfTrigger(Perbill::one()),
    ),
    (
      "PercentageOfLastFunding",
      AmountResolution::PercentageOfLastFunding(Perbill::one()),
    ),
    ("AllBalance", AmountResolution::AllBalance),
  ]
}

fn condition_cases() -> Vec<(&'static str, ManifestCondition)> {
  vec![
    (
      "BalanceAbove",
      Condition::BalanceAbove {
        asset: ASSET,
        threshold: 1,
      },
    ),
    (
      "BalanceBelow",
      Condition::BalanceBelow {
        asset: ASSET,
        threshold: 1,
      },
    ),
    (
      "BalanceEquals",
      Condition::BalanceEquals {
        asset: ASSET,
        threshold: 1,
      },
    ),
    (
      "BalanceNotEquals",
      Condition::BalanceNotEquals {
        asset: ASSET,
        threshold: 1,
      },
    ),
    (
      "BlockNumberAbove",
      Condition::BlockNumberAbove { threshold: 1 },
    ),
    (
      "BlockNumberBelow",
      Condition::BlockNumberBelow { threshold: 1 },
    ),
    (
      "ObservationAbove",
      Condition::ObservationAbove {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
    ),
    (
      "ObservationBelow",
      Condition::ObservationBelow {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
    ),
    (
      "ObservationEquals",
      Condition::ObservationEquals {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
    ),
    (
      "ObservationNotEquals",
      Condition::ObservationNotEquals {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      },
    ),
  ]
}

fn task_cases() -> Vec<(&'static str, ManifestTask)> {
  let legs = BoundedVec::try_from(vec![
    SplitLeg {
      to: LEG_A,
      share: Perbill::from_percent(40),
    },
    SplitLeg {
      to: LEG_B,
      share: Perbill::from_percent(60),
    },
  ])
  .expect("manifest split legs fit");
  vec![
    (
      "Transfer",
      Task::Transfer {
        to: TO,
        asset: ASSET,
        amount: fixed(),
      },
    ),
    (
      "SplitTransfer",
      Task::SplitTransfer {
        asset: ASSET,
        amount: fixed(),
        legs,
      },
    ),
    (
      "SwapIn",
      Task::SwapIn {
        asset_in: ASSET_IN,
        amount_in: fixed(),
        asset_out: ASSET_OUT,
        slippage_tolerance: Perbill::zero(),
      },
    ),
    (
      "SwapOut",
      Task::SwapOut {
        asset_out: ASSET_OUT,
        amount_out: fixed(),
        asset_in: ASSET_IN,
        input_limit: InputLimit::Absolute(100),
        slippage_tolerance: Perbill::zero(),
      },
    ),
    (
      "AddLiquidity",
      Task::AddLiquidity {
        asset_a: ASSET_A,
        asset_b: ASSET_B,
        amount_a: fixed(),
        amount_b: fixed(),
        min_lp_out: 1,
      },
    ),
    (
      "RemoveLiquidity",
      Task::RemoveLiquidity {
        lp_asset: LP_ASSET,
        amount: fixed(),
        min_amount_a: 1,
        min_amount_b: 1,
      },
    ),
    (
      "Burn",
      Task::Burn {
        asset: ASSET,
        amount: fixed(),
      },
    ),
    (
      "Mint",
      Task::Mint {
        asset: ASSET,
        amount: fixed(),
      },
    ),
    (
      "Stake",
      Task::Stake {
        asset: ASSET,
        amount: fixed(),
      },
    ),
    (
      "DonateLiquidity",
      Task::DonateLiquidity {
        asset_a: ASSET_A,
        asset_b: ASSET_B,
        amount: fixed(),
        max_ratio_error: Perbill::zero(),
      },
    ),
    (
      "Unstake",
      Task::Unstake {
        asset: ASSET,
        shares: fixed(),
      },
    ),
    ("StopCycle", Task::StopCycle),
  ]
}

fn asset_path(asset: u32) -> &'static str {
  match asset {
    ASSET => "/asset",
    ASSET_IN => "/asset_in",
    ASSET_OUT => "/asset_out",
    ASSET_A => "/asset_a",
    ASSET_B => "/asset_b",
    LP_ASSET => "/lp_asset",
    _ => panic!("unknown manifest asset sentinel {asset}"),
  }
}

fn recipient(recipient: RecipientSurface<u64>) -> RecipientManifest {
  match recipient {
    RecipientSurface::ActorSovereign => RecipientManifest::ActorSovereign,
    RecipientSurface::Explicit(TO) => RecipientManifest::Explicit { path: "/to" },
    RecipientSurface::Explicit(LEG_A | LEG_B) => RecipientManifest::Explicit { path: "/legs/*/to" },
    RecipientSurface::Explicit(account) => panic!("unknown manifest account sentinel {account}"),
    RecipientSurface::AdapterDerived => RecipientManifest::AdapterDerived,
  }
}

fn adapter(value: AdapterRequirement) -> &'static str {
  match value {
    AdapterRequirement::None => "None",
    AdapterRequirement::AssetOps => "AssetOps",
    AdapterRequirement::DexOps => "DexOps",
    AdapterRequirement::StakingOps => "StakingOps",
    AdapterRequirement::LiquidityOps => "LiquidityOps",
  }
}

fn effect(value: EffectClass) -> &'static str {
  match value {
    EffectClass::Transfer => "Transfer",
    EffectClass::SupplyBurn => "SupplyBurn",
    EffectClass::SupplyMint => "SupplyMint",
    EffectClass::LiquidityMutation => "LiquidityMutation",
    EffectClass::StakingMutation => "StakingMutation",
  }
}

fn availability(value: ActorAvailability) -> &'static str {
  match value {
    ActorAvailability::UserAndSystem => "UserAndSystem",
    ActorAvailability::SystemOnly => "SystemOnly",
  }
}

fn control(value: ClassifiedStepControl) -> &'static str {
  match value {
    ClassifiedStepControl::Advance => "Advance",
    ClassifiedStepControl::CompleteCycle => "CompleteCycle",
    ClassifiedStepControl::Terminate => "Terminate",
    ClassifiedStepControl::SuspendCurrent => "SuspendCurrent",
  }
}

fn weight_owner(value: TaskWeightOwner) -> &'static str {
  match value {
    TaskWeightOwner::Transfer => "Transfer",
    TaskWeightOwner::SplitTransfer => "SplitTransfer",
    TaskWeightOwner::Burn => "Burn",
    TaskWeightOwner::Mint => "Mint",
    TaskWeightOwner::DexSwapIn => "DexSwapIn",
    TaskWeightOwner::DexSwapOut => "DexSwapOut",
    TaskWeightOwner::AddLiquidity => "AddLiquidity",
    TaskWeightOwner::RemoveLiquidity => "RemoveLiquidity",
    TaskWeightOwner::Stake => "Stake",
    TaskWeightOwner::DonateLiquidity => "DonateLiquidity",
    TaskWeightOwner::Unstake => "Unstake",
    TaskWeightOwner::StopCycle => "StopCycle",
  }
}

fn algorithm(value: BoundedInternalAlgorithm) -> &'static str {
  match value {
    BoundedInternalAlgorithm::None => "None",
    BoundedInternalAlgorithm::PalletSplitFanout => "PalletSplitFanout",
    BoundedInternalAlgorithm::RuntimeAdapterContract => "RuntimeAdapterContract",
  }
}

fn amount_role(value: TaskAmountRole) -> (&'static str, &'static str) {
  match value {
    TaskAmountRole::Amount => ("Amount", "/amount"),
    TaskAmountRole::AmountIn => ("AmountIn", "/amount_in"),
    TaskAmountRole::AmountOut => ("AmountOut", "/amount_out"),
    TaskAmountRole::AmountA => ("AmountA", "/amount_a"),
    TaskAmountRole::AmountB => ("AmountB", "/amount_b"),
    TaskAmountRole::Shares => ("Shares", "/shares"),
  }
}

fn dependency(value: AmountDataDependency) -> &'static str {
  match value {
    AmountDataDependency::ArtifactValue => "ArtifactValue",
    AmountDataDependency::CurrentBalanceOrShares => "CurrentBalanceOrShares",
    AmountDataDependency::TriggerSnapshot => "TriggerSnapshot",
    AmountDataDependency::LastFundingSnapshot => "LastFundingSnapshot",
    AmountDataDependency::TaskPolicyCapacity => "TaskPolicyCapacity",
  }
}

fn context_dependency(value: ContextDependency) -> &'static str {
  match value {
    ContextDependency::None => "None",
    ContextDependency::TaskPolicy => "TaskPolicy",
  }
}

fn observation_window(value: ObservationWindow) -> &'static str {
  match value {
    ObservationWindow::ArtifactTime => "ArtifactTime",
    ObservationWindow::LogicalRunStart => "LogicalRunStart",
    ObservationWindow::StepAttemptTime => "StepAttemptTime",
  }
}

fn retry_observation(value: RetryObservation) -> &'static str {
  match value {
    RetryObservation::ReobserveLiveValue => "ReobserveLiveValue",
    RetryObservation::ReuseFrozenValueWithLiveCapacity => "ReuseFrozenValueWithLiveCapacity",
  }
}

fn task_manifest(task: &'static str, contract: Contract) -> TaskManifest {
  let mut recipients = Vec::new();
  for surface in contract.recipients {
    let projected = recipient(surface);
    if !recipients.contains(&projected) {
      recipients.push(projected);
    }
  }
  TaskManifest {
    task,
    required_adapter: adapter(contract.required_adapter),
    assets_read: contract.assets_read.into_iter().map(asset_path).collect(),
    assets_written: contract
      .assets_written
      .into_iter()
      .map(asset_path)
      .collect(),
    reads_adapter_derived_assets: contract.reads_adapter_derived_assets,
    writes_adapter_derived_assets: contract.writes_adapter_derived_assets,
    recipients,
    effects: contract.effects.into_iter().map(effect).collect(),
    availability: availability(contract.availability),
    committed_non_compensated_effects: contract.committed_non_compensated_effects,
    successful_control: control(contract.successful_control),
    weight_owner: weight_owner(contract.weight_owner),
    bounded_internal_algorithm: algorithm(contract.bounded_internal_algorithm),
    amount_surfaces: contract
      .amount_surfaces
      .into_iter()
      .map(|surface| {
        let (role, path) = amount_role(surface.role);
        AmountSurfaceManifest { role, path }
      })
      .collect(),
  }
}

fn condition_manifest(
  condition: &'static str,
  scale_index: u8,
  contract: ConditionContract,
) -> ConditionManifest {
  let observation = match contract.observation {
    ConditionObservation::BalanceComparison => "BalanceComparison",
    ConditionObservation::BlockNumberComparison => "BlockNumberComparison",
    ConditionObservation::ScalarObservationComparison => "ScalarObservationComparison",
  };
  let read_surface = match contract.read_surface {
    ConditionReadSurface::SpendableAssetBalance(ASSET) => {
      ConditionReadSurfaceManifest::SpendableAssetBalance { path: "/asset" }
    }
    ConditionReadSurface::SpendableAssetBalance(asset) => {
      panic!("unknown condition asset sentinel {asset}")
    }
    ConditionReadSurface::CurrentBlockNumber => ConditionReadSurfaceManifest::CurrentBlockNumber,
    ConditionReadSurface::TypedObservation { .. } => {
      ConditionReadSurfaceManifest::TypedObservation {
        feed_path: "/feed",
        max_age_blocks_path: "/max_age_blocks",
      }
    }
  };
  ConditionManifest {
    condition,
    scale_index,
    observation,
    read_surface,
    pure: contract.pure,
    observation_window: observation_window(contract.observation_window),
    bounded_read_count: contract.bounded_read_count,
  }
}

fn amount_resolution_manifest(
  resolution: &'static str,
  amount: AmountResolution<u128>,
) -> AmountResolutionManifest {
  let contract = describe_amount_resolution(&amount);
  AmountResolutionManifest {
    resolution,
    data_dependencies: contract
      .data_dependencies
      .into_iter()
      .map(dependency)
      .collect(),
    minimum_balance_dependency: context_dependency(contract.minimum_balance_dependency),
    fee_reserve_dependency: context_dependency(contract.fee_reserve_dependency),
    value_observation_window: observation_window(contract.value_observation_window),
    retry_observation: retry_observation(contract.retry_observation),
  }
}

fn manifest() -> SemanticManifest {
  let cases = task_cases();
  let TypeDef::Variant(task_type) = <ManifestTask as TypeInfo>::type_info().type_def else {
    panic!("Task metadata must remain a variant type");
  };
  let metadata_names = task_type
    .variants
    .into_iter()
    .map(|variant| variant.name)
    .collect::<Vec<_>>();
  let case_names = cases
    .as_slice()
    .into_iter()
    .map(|(name, _)| *name)
    .collect::<Vec<_>>();
  assert_eq!(
    case_names, metadata_names,
    "manifest must cover Task in SCALE order"
  );
  let condition_cases = condition_cases();
  let TypeDef::Variant(condition_type) = <ManifestCondition as TypeInfo>::type_info().type_def
  else {
    panic!("Condition metadata must remain a variant type");
  };
  let condition_metadata = condition_type.variants;
  assert_eq!(
    condition_cases
      .as_slice()
      .into_iter()
      .map(|(name, _)| *name)
      .collect::<Vec<_>>(),
    condition_metadata
      .as_slice()
      .into_iter()
      .map(|variant| variant.name)
      .collect::<Vec<_>>(),
    "manifest must cover Condition in SCALE order"
  );
  let amount_cases = amount_resolution_cases();
  let TypeDef::Variant(amount_type) = <AmountResolution<u128> as TypeInfo>::type_info().type_def
  else {
    panic!("AmountResolution metadata must remain a variant type");
  };
  assert_eq!(
    amount_cases
      .as_slice()
      .into_iter()
      .map(|(name, _)| *name)
      .collect::<Vec<_>>(),
    amount_type
      .variants
      .into_iter()
      .map(|variant| variant.name)
      .collect::<Vec<_>>(),
    "manifest must cover AmountResolution in SCALE order"
  );
  SemanticManifest {
    format: "deos.aaa.semantic-manifest",
    format_version: 2,
    tasks: cases
      .into_iter()
      .map(|(name, task)| task_manifest(name, describe_task(&task)))
      .collect(),
    conditions: condition_cases
      .into_iter()
      .zip(condition_metadata)
      .map(|((name, condition), metadata)| {
        assert_eq!(
          name, metadata.name,
          "condition case and metadata order differ"
        );
        condition_manifest(name, metadata.index, describe_condition(&condition))
      })
      .collect(),
    amount_resolutions: amount_cases
      .into_iter()
      .map(|(name, amount)| amount_resolution_manifest(name, amount))
      .collect(),
  }
}

fn main() {
  let rendered = serde_json::to_string(&manifest()).expect("manifest serializes") + "\n";
  let args = env::args().skip(1).collect::<Vec<_>>();
  match args.as_slice() {
    [] => print!("{rendered}"),
    [flag, path] if flag == "--check" => {
      let actual = fs::read_to_string(Path::new(path)).expect("manifest artifact is readable");
      assert_eq!(actual, rendered, "semantic manifest artifact is stale");
    }
    _ => {
      panic!("usage: cargo run -p pallet-deos-aaa --example semantic_manifest -- [--check PATH]")
    }
  }
}
