use frame::prelude::*;
use polkadot_sdk::sp_runtime::Perbill;

pub type AaaId = u64;
pub const SYSTEM_OWNER_SLOT_SENTINEL: u8 = 0;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AmountResolution<Balance> {
  Fixed(Balance),
  PercentageOfCurrent(Perbill),
  PercentageOfTrigger(Perbill),
  PercentageOfLastFunding(Perbill),
  AllBalance,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct SplitLeg<AccountId> {
  pub to: AccountId,
  pub share: Perbill,
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSplitTransferLegs))]
pub enum Task<AssetId, Balance, AccountId, MaxSplitTransferLegs: Get<u32>> {
  Transfer {
    to: AccountId,
    asset: AssetId,
    amount: AmountResolution<Balance>,
  },
  SplitTransfer {
    asset: AssetId,
    amount: AmountResolution<Balance>,
    legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs>,
  },
  SwapExactIn {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
  },
  SwapExactOut {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
  },
  AddLiquidity {
    asset_a: AssetId,
    asset_b: AssetId,
    amount_a: AmountResolution<Balance>,
    amount_b: AmountResolution<Balance>,
  },
  RemoveLiquidity {
    lp_asset: AssetId,
    amount: AmountResolution<Balance>,
  },
  Burn {
    asset: AssetId,
    amount: AmountResolution<Balance>,
  },
  Mint {
    asset: AssetId,
    amount: AmountResolution<Balance>,
  },
  Stake {
    asset: AssetId,
    amount: AmountResolution<Balance>,
  },
  DonateLiquidity {
    asset_a: AssetId,
    asset_b: AssetId,
    amount: AmountResolution<Balance>,
    max_ratio_error: Perbill,
  },
  Unstake {
    asset: AssetId,
    shares: AmountResolution<Balance>,
  },
}

impl<AssetId: Clone, Balance: Clone, AccountId: Clone, MaxSplitTransferLegs: Get<u32>> Clone
  for Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>
{
  fn clone(&self) -> Self {
    match self {
      Self::Transfer { to, asset, amount } => Self::Transfer {
        to: to.clone(),
        asset: asset.clone(),
        amount: amount.clone(),
      },
      Self::SplitTransfer {
        asset,
        amount,
        legs,
      } => Self::SplitTransfer {
        asset: asset.clone(),
        amount: amount.clone(),
        legs: legs.clone(),
      },
      Self::SwapExactIn {
        asset_in,
        asset_out,
        amount_in,
        slippage_tolerance,
      } => Self::SwapExactIn {
        asset_in: asset_in.clone(),
        asset_out: asset_out.clone(),
        amount_in: amount_in.clone(),
        slippage_tolerance: *slippage_tolerance,
      },
      Self::SwapExactOut {
        asset_in,
        asset_out,
        amount_out,
        slippage_tolerance,
      } => Self::SwapExactOut {
        asset_in: asset_in.clone(),
        asset_out: asset_out.clone(),
        amount_out: amount_out.clone(),
        slippage_tolerance: *slippage_tolerance,
      },
      Self::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
      } => Self::AddLiquidity {
        asset_a: asset_a.clone(),
        asset_b: asset_b.clone(),
        amount_a: amount_a.clone(),
        amount_b: amount_b.clone(),
      },
      Self::RemoveLiquidity { lp_asset, amount } => Self::RemoveLiquidity {
        lp_asset: lp_asset.clone(),
        amount: amount.clone(),
      },
      Self::Burn { asset, amount } => Self::Burn {
        asset: asset.clone(),
        amount: amount.clone(),
      },
      Self::Mint { asset, amount } => Self::Mint {
        asset: asset.clone(),
        amount: amount.clone(),
      },
      Self::Stake { asset, amount } => Self::Stake {
        asset: asset.clone(),
        amount: amount.clone(),
      },
      Self::DonateLiquidity {
        asset_a,
        asset_b,
        amount,
        max_ratio_error,
      } => Self::DonateLiquidity {
        asset_a: asset_a.clone(),
        asset_b: asset_b.clone(),
        amount: amount.clone(),
        max_ratio_error: *max_ratio_error,
      },
      Self::Unstake { asset, shares } => Self::Unstake {
        asset: asset.clone(),
        shares: shares.clone(),
      },
    }
  }
}

impl<
  AssetId: core::fmt::Debug,
  Balance: core::fmt::Debug,
  AccountId: core::fmt::Debug,
  MaxSplitTransferLegs: Get<u32>,
> core::fmt::Debug for Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Transfer { to, asset, amount } => f
        .debug_struct("Transfer")
        .field("to", to)
        .field("asset", asset)
        .field("amount", amount)
        .finish(),
      Self::SplitTransfer {
        asset,
        amount,
        legs,
      } => f
        .debug_struct("SplitTransfer")
        .field("asset", asset)
        .field("amount", amount)
        .field("legs", legs)
        .finish(),
      Self::SwapExactIn {
        asset_in,
        asset_out,
        amount_in,
        slippage_tolerance,
      } => f
        .debug_struct("SwapExactIn")
        .field("asset_in", asset_in)
        .field("asset_out", asset_out)
        .field("amount_in", amount_in)
        .field("slippage_tolerance", slippage_tolerance)
        .finish(),
      Self::SwapExactOut {
        asset_in,
        asset_out,
        amount_out,
        slippage_tolerance,
      } => f
        .debug_struct("SwapExactOut")
        .field("asset_in", asset_in)
        .field("asset_out", asset_out)
        .field("amount_out", amount_out)
        .field("slippage_tolerance", slippage_tolerance)
        .finish(),
      Self::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
      } => f
        .debug_struct("AddLiquidity")
        .field("asset_a", asset_a)
        .field("asset_b", asset_b)
        .field("amount_a", amount_a)
        .field("amount_b", amount_b)
        .finish(),
      Self::RemoveLiquidity { lp_asset, amount } => f
        .debug_struct("RemoveLiquidity")
        .field("lp_asset", lp_asset)
        .field("amount", amount)
        .finish(),
      Self::Burn { asset, amount } => f
        .debug_struct("Burn")
        .field("asset", asset)
        .field("amount", amount)
        .finish(),
      Self::Mint { asset, amount } => f
        .debug_struct("Mint")
        .field("asset", asset)
        .field("amount", amount)
        .finish(),
      Self::Stake { asset, amount } => f
        .debug_struct("Stake")
        .field("asset", asset)
        .field("amount", amount)
        .finish(),
      Self::DonateLiquidity {
        asset_a,
        asset_b,
        amount,
        max_ratio_error,
      } => f
        .debug_struct("DonateLiquidity")
        .field("asset_a", asset_a)
        .field("asset_b", asset_b)
        .field("amount", amount)
        .field("max_ratio_error", max_ratio_error)
        .finish(),
      Self::Unstake { asset, shares } => f
        .debug_struct("Unstake")
        .field("asset", asset)
        .field("shares", shares)
        .finish(),
    }
  }
}

impl<AssetId: PartialEq, Balance: PartialEq, AccountId: PartialEq, MaxSplitTransferLegs: Get<u32>>
  PartialEq for Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (
        Self::Transfer {
          to: left_to,
          asset: left_asset,
          amount: left_amount,
        },
        Self::Transfer {
          to: right_to,
          asset: right_asset,
          amount: right_amount,
        },
      ) => left_to == right_to && left_asset == right_asset && left_amount == right_amount,
      (
        Self::SplitTransfer {
          asset: left_asset,
          amount: left_amount,
          legs: left_legs,
        },
        Self::SplitTransfer {
          asset: right_asset,
          amount: right_amount,
          legs: right_legs,
        },
      ) => left_asset == right_asset && left_amount == right_amount && left_legs == right_legs,
      (
        Self::SwapExactIn {
          asset_in: left_asset_in,
          asset_out: left_asset_out,
          amount_in: left_amount_in,
          slippage_tolerance: left_slippage,
        },
        Self::SwapExactIn {
          asset_in: right_asset_in,
          asset_out: right_asset_out,
          amount_in: right_amount_in,
          slippage_tolerance: right_slippage,
        },
      ) => {
        left_asset_in == right_asset_in
          && left_asset_out == right_asset_out
          && left_amount_in == right_amount_in
          && left_slippage == right_slippage
      }
      (
        Self::SwapExactOut {
          asset_in: left_asset_in,
          asset_out: left_asset_out,
          amount_out: left_amount_out,
          slippage_tolerance: left_slippage,
        },
        Self::SwapExactOut {
          asset_in: right_asset_in,
          asset_out: right_asset_out,
          amount_out: right_amount_out,
          slippage_tolerance: right_slippage,
        },
      ) => {
        left_asset_in == right_asset_in
          && left_asset_out == right_asset_out
          && left_amount_out == right_amount_out
          && left_slippage == right_slippage
      }
      (
        Self::AddLiquidity {
          asset_a: left_asset_a,
          asset_b: left_asset_b,
          amount_a: left_amount_a,
          amount_b: left_amount_b,
        },
        Self::AddLiquidity {
          asset_a: right_asset_a,
          asset_b: right_asset_b,
          amount_a: right_amount_a,
          amount_b: right_amount_b,
        },
      ) => {
        left_asset_a == right_asset_a
          && left_asset_b == right_asset_b
          && left_amount_a == right_amount_a
          && left_amount_b == right_amount_b
      }
      (
        Self::RemoveLiquidity {
          lp_asset: left_lp_asset,
          amount: left_amount,
        },
        Self::RemoveLiquidity {
          lp_asset: right_lp_asset,
          amount: right_amount,
        },
      ) => left_lp_asset == right_lp_asset && left_amount == right_amount,
      (
        Self::Burn {
          asset: left_asset,
          amount: left_amount,
        },
        Self::Burn {
          asset: right_asset,
          amount: right_amount,
        },
      ) => left_asset == right_asset && left_amount == right_amount,
      (
        Self::Mint {
          asset: left_asset,
          amount: left_amount,
        },
        Self::Mint {
          asset: right_asset,
          amount: right_amount,
        },
      ) => left_asset == right_asset && left_amount == right_amount,
      (
        Self::Stake {
          asset: left_asset,
          amount: left_amount,
        },
        Self::Stake {
          asset: right_asset,
          amount: right_amount,
        },
      ) => left_asset == right_asset && left_amount == right_amount,
      (
        Self::DonateLiquidity {
          asset_a: left_asset_a,
          asset_b: left_asset_b,
          amount: left_amount,
          max_ratio_error: left_max_ratio_error,
        },
        Self::DonateLiquidity {
          asset_a: right_asset_a,
          asset_b: right_asset_b,
          amount: right_amount,
          max_ratio_error: right_max_ratio_error,
        },
      ) => {
        left_asset_a == right_asset_a
          && left_asset_b == right_asset_b
          && left_amount == right_amount
          && left_max_ratio_error == right_max_ratio_error
      }
      (
        Self::Unstake {
          asset: left_asset,
          shares: left_shares,
        },
        Self::Unstake {
          asset: right_asset,
          shares: right_shares,
        },
      ) => left_asset == right_asset && left_shares == right_shares,
      _ => false,
    }
  }
}

impl<AssetId: Eq, Balance: Eq, AccountId: Eq, MaxSplitTransferLegs: Get<u32>> Eq
  for Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>
{
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AaaType {
  User,
  System,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorClass {
  User { owner_slot: u8 },
  System,
}

impl ActorClass {
  pub fn aaa_type(self) -> AaaType {
    match self {
      Self::User { .. } => AaaType::User,
      Self::System => AaaType::System,
    }
  }

  pub fn owner_slot(self) -> Option<u8> {
    match self {
      Self::User { owner_slot } => Some(owner_slot),
      Self::System => None,
    }
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Default,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum Mutability {
  #[default]
  Mutable,
  Immutable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum PauseReason {
  Manual,
  CycleNonceExhausted,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActiveLifecycle {
  Active,
  Paused(PauseReason),
}

impl ActiveLifecycle {
  pub fn is_paused(self) -> bool {
    matches!(self, Self::Paused(_))
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum CloseReason {
  OwnerInitiated,
  BalanceExhausted,
  ConsecutiveFailures,
  WindowExpired,
  CycleNonceExhausted,
  FeeBudgetExhausted,
  AutoCloseNonceReached,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum StepErrorPolicy {
  AbortCycle,
  ContinueNextStep,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum DeferReason {
  InsufficientWeightBudget,
  CloseTransitionFailed,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum StepSkippedReason {
  ConditionsNotMet,
  ResolutionSkipped,
  FundingUnavailable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum OnCloseStepFailureKind {
  EvaluationFee,
  ExecutionFee,
  Condition,
  Resolution,
  Adapter,
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize))]
pub enum SourceFilter<AccountId, MaxWhitelistSize: Get<u32>> {
  Any,
  OwnerOnly,
  Whitelist(BoundedVec<AccountId, MaxWhitelistSize>),
}

impl<AccountId: Clone, MaxWhitelistSize: Get<u32>> Clone
  for SourceFilter<AccountId, MaxWhitelistSize>
{
  fn clone(&self) -> Self {
    match self {
      Self::Any => Self::Any,
      Self::OwnerOnly => Self::OwnerOnly,
      Self::Whitelist(list) => Self::Whitelist(list.clone()),
    }
  }
}

impl<AccountId: core::fmt::Debug, MaxWhitelistSize: Get<u32>> core::fmt::Debug
  for SourceFilter<AccountId, MaxWhitelistSize>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Any => f.write_str("Any"),
      Self::OwnerOnly => f.write_str("OwnerOnly"),
      Self::Whitelist(list) => f.debug_tuple("Whitelist").field(list).finish(),
    }
  }
}

impl<AccountId: PartialEq, MaxWhitelistSize: Get<u32>> PartialEq
  for SourceFilter<AccountId, MaxWhitelistSize>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Any, Self::Any) => true,
      (Self::OwnerOnly, Self::OwnerOnly) => true,
      (Self::Whitelist(left), Self::Whitelist(right)) => left == right,
      _ => false,
    }
  }
}

impl<AccountId: Eq, MaxWhitelistSize: Get<u32>> Eq for SourceFilter<AccountId, MaxWhitelistSize> {}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize))]
pub enum AssetFilter<AssetId, MaxWhitelistSize: Get<u32>> {
  Any,
  Whitelist(BoundedVec<AssetId, MaxWhitelistSize>),
}

impl<AssetId: Clone, MaxWhitelistSize: Get<u32>> Clone for AssetFilter<AssetId, MaxWhitelistSize> {
  fn clone(&self) -> Self {
    match self {
      Self::Any => Self::Any,
      Self::Whitelist(list) => Self::Whitelist(list.clone()),
    }
  }
}

impl<AssetId: core::fmt::Debug, MaxWhitelistSize: Get<u32>> core::fmt::Debug
  for AssetFilter<AssetId, MaxWhitelistSize>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Any => f.write_str("Any"),
      Self::Whitelist(list) => f.debug_tuple("Whitelist").field(list).finish(),
    }
  }
}

impl<AssetId: PartialEq, MaxWhitelistSize: Get<u32>> PartialEq
  for AssetFilter<AssetId, MaxWhitelistSize>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Any, Self::Any) => true,
      (Self::Whitelist(left), Self::Whitelist(right)) => left == right,
      _ => false,
    }
  }
}

impl<AssetId: Eq, MaxWhitelistSize: Get<u32>> Eq for AssetFilter<AssetId, MaxWhitelistSize> {}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize))]
pub enum Trigger<AccountId, AssetId, MaxWhitelistSize: Get<u32>> {
  Timer {
    every_blocks: u32,
  },
  OnAddressEvent {
    source_filter: SourceFilter<AccountId, MaxWhitelistSize>,
    asset_filter: AssetFilter<AssetId, MaxWhitelistSize>,
  },
  Manual,
}

impl<AccountId: Clone, AssetId: Clone, MaxWhitelistSize: Get<u32>> Clone
  for Trigger<AccountId, AssetId, MaxWhitelistSize>
{
  fn clone(&self) -> Self {
    match self {
      Self::Timer { every_blocks } => Self::Timer {
        every_blocks: *every_blocks,
      },
      Self::OnAddressEvent {
        source_filter,
        asset_filter,
      } => Self::OnAddressEvent {
        source_filter: source_filter.clone(),
        asset_filter: asset_filter.clone(),
      },
      Self::Manual => Self::Manual,
    }
  }
}

impl<AccountId: core::fmt::Debug, AssetId: core::fmt::Debug, MaxWhitelistSize: Get<u32>>
  core::fmt::Debug for Trigger<AccountId, AssetId, MaxWhitelistSize>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Timer { every_blocks } => f
        .debug_struct("Timer")
        .field("every_blocks", every_blocks)
        .finish(),
      Self::OnAddressEvent {
        source_filter,
        asset_filter,
      } => f
        .debug_struct("OnAddressEvent")
        .field("source_filter", source_filter)
        .field("asset_filter", asset_filter)
        .finish(),
      Self::Manual => f.write_str("Manual"),
    }
  }
}

impl<AccountId: PartialEq, AssetId: PartialEq, MaxWhitelistSize: Get<u32>> PartialEq
  for Trigger<AccountId, AssetId, MaxWhitelistSize>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (
        Self::Timer {
          every_blocks: left_every_blocks,
        },
        Self::Timer {
          every_blocks: right_every_blocks,
        },
      ) => left_every_blocks == right_every_blocks,
      (
        Self::OnAddressEvent {
          source_filter: left_source_filter,
          asset_filter: left_asset_filter,
        },
        Self::OnAddressEvent {
          source_filter: right_source_filter,
          asset_filter: right_asset_filter,
        },
      ) => left_source_filter == right_source_filter && left_asset_filter == right_asset_filter,
      (Self::Manual, Self::Manual) => true,
      _ => false,
    }
  }
}

impl<AccountId: Eq, AssetId: Eq, MaxWhitelistSize: Get<u32>> Eq
  for Trigger<AccountId, AssetId, MaxWhitelistSize>
{
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize))]
pub struct Schedule<AccountId, AssetId, MaxWhitelistSize: Get<u32>> {
  pub trigger: Trigger<AccountId, AssetId, MaxWhitelistSize>,
  pub cooldown_blocks: u32,
}

impl<AccountId: Clone, AssetId: Clone, MaxWhitelistSize: Get<u32>> Clone
  for Schedule<AccountId, AssetId, MaxWhitelistSize>
{
  fn clone(&self) -> Self {
    Self {
      trigger: self.trigger.clone(),
      cooldown_blocks: self.cooldown_blocks,
    }
  }
}

impl<AccountId: core::fmt::Debug, AssetId: core::fmt::Debug, MaxWhitelistSize: Get<u32>>
  core::fmt::Debug for Schedule<AccountId, AssetId, MaxWhitelistSize>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Schedule")
      .field("trigger", &self.trigger)
      .field("cooldown_blocks", &self.cooldown_blocks)
      .finish()
  }
}

impl<AccountId: PartialEq, AssetId: PartialEq, MaxWhitelistSize: Get<u32>> PartialEq
  for Schedule<AccountId, AssetId, MaxWhitelistSize>
{
  fn eq(&self, other: &Self) -> bool {
    self.trigger == other.trigger && self.cooldown_blocks == other.cooldown_blocks
  }
}

impl<AccountId: Eq, AssetId: Eq, MaxWhitelistSize: Get<u32>> Eq
  for Schedule<AccountId, AssetId, MaxWhitelistSize>
{
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum Condition<AssetId, Balance, BlockNumber = u32> {
  BalanceAbove { asset: AssetId, threshold: Balance },
  BalanceBelow { asset: AssetId, threshold: Balance },
  BalanceEquals { asset: AssetId, threshold: Balance },
  BalanceNotEquals { asset: AssetId, threshold: Balance },
  BlockNumberAbove { threshold: BlockNumber },
  BlockNumberBelow { threshold: BlockNumber },
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxConditionsPerStep, MaxSplitTransferLegs))]
pub struct Step<
  AssetId,
  Balance,
  AccountId,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
> {
  pub conditions: BoundedVec<Condition<AssetId, Balance>, MaxConditionsPerStep>,
  pub task: Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
  pub on_error: StepErrorPolicy,
}

impl<
  AssetId: Clone,
  Balance: Clone,
  AccountId: Clone,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
> Clone for Step<AssetId, Balance, AccountId, MaxConditionsPerStep, MaxSplitTransferLegs>
{
  fn clone(&self) -> Self {
    Self {
      conditions: self.conditions.clone(),
      task: self.task.clone(),
      on_error: self.on_error,
    }
  }
}

impl<
  AssetId: core::fmt::Debug,
  Balance: core::fmt::Debug,
  AccountId: core::fmt::Debug,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
> core::fmt::Debug
  for Step<AssetId, Balance, AccountId, MaxConditionsPerStep, MaxSplitTransferLegs>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Step")
      .field("conditions", &self.conditions)
      .field("task", &self.task)
      .field("on_error", &self.on_error)
      .finish()
  }
}

impl<
  AssetId: PartialEq,
  Balance: PartialEq,
  AccountId: PartialEq,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
> PartialEq for Step<AssetId, Balance, AccountId, MaxConditionsPerStep, MaxSplitTransferLegs>
{
  fn eq(&self, other: &Self) -> bool {
    self.conditions == other.conditions
      && self.task == other.task
      && self.on_error == other.on_error
  }
}

impl<
  AssetId: Eq,
  Balance: Eq,
  AccountId: Eq,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
> Eq for Step<AssetId, Balance, AccountId, MaxConditionsPerStep, MaxSplitTransferLegs>
{
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ScheduleWindow<BlockNumber> {
  pub start: BlockNumber,
  pub end: BlockNumber,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FundingBatch<Balance> {
  pub amount: Balance,
  pub pending_amount: Balance,
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSignedFundingSources))]
pub enum FundingSourcePolicy<AccountId, MaxSignedFundingSources: Get<u32>> {
  OwnerOnly,
  SignedAllowlist(BoundedBTreeSet<AccountId, MaxSignedFundingSources>),
  RuntimePolicy,
  AnySource,
}

impl<AccountId: Clone, MaxSignedFundingSources: Get<u32>> Clone
  for FundingSourcePolicy<AccountId, MaxSignedFundingSources>
{
  fn clone(&self) -> Self {
    match self {
      Self::OwnerOnly => Self::OwnerOnly,
      Self::SignedAllowlist(allowed) => Self::SignedAllowlist(allowed.clone()),
      Self::RuntimePolicy => Self::RuntimePolicy,
      Self::AnySource => Self::AnySource,
    }
  }
}

impl<AccountId: core::fmt::Debug, MaxSignedFundingSources: Get<u32>> core::fmt::Debug
  for FundingSourcePolicy<AccountId, MaxSignedFundingSources>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::OwnerOnly => f.write_str("OwnerOnly"),
      Self::SignedAllowlist(allowed) => f.debug_tuple("SignedAllowlist").field(allowed).finish(),
      Self::RuntimePolicy => f.write_str("RuntimePolicy"),
      Self::AnySource => f.write_str("AnySource"),
    }
  }
}

impl<AccountId: PartialEq, MaxSignedFundingSources: Get<u32>> PartialEq
  for FundingSourcePolicy<AccountId, MaxSignedFundingSources>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::OwnerOnly, Self::OwnerOnly)
      | (Self::RuntimePolicy, Self::RuntimePolicy)
      | (Self::AnySource, Self::AnySource) => true,
      (Self::SignedAllowlist(left), Self::SignedAllowlist(right)) => left == right,
      _ => false,
    }
  }
}

impl<AccountId: Eq, MaxSignedFundingSources: Get<u32>> Eq
  for FundingSourcePolicy<AccountId, MaxSignedFundingSources>
{
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProgramInput<Schedule, BlockNumber, ExecutionPlan, FundingPolicy> {
  Dormant,
  Active {
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow<BlockNumber>>,
    execution_plan: ExecutionPlan,
    on_close_execution_plan: ExecutionPlan,
    funding_source_policy: FundingPolicy,
  },
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DormantAaaIdentity<AccountId> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct AaaInstance<AccountId, BlockNumber, Schedule, ExecutionPlan, Balance> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub lifecycle: ActiveLifecycle,
  pub schedule: Schedule,
  pub schedule_window: Option<ScheduleWindow<BlockNumber>>,
  pub execution_plan: ExecutionPlan,
  pub on_close_execution_plan: ExecutionPlan,
  pub cycle_nonce: u64,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub consecutive_failures: u32,
  pub manual_trigger_pending: bool,
  pub cycle_weight_upper: Weight,
  pub cycle_fee_upper: Balance,
  pub first_eligible_at: BlockNumber,
  pub last_cycle_block: BlockNumber,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorFundingState<FundingPolicy, FundingSnapshots, FundingTrackedAssets> {
  pub funding_source_policy: FundingPolicy,
  pub funding_snapshots: FundingSnapshots,
  pub funding_tracked_assets: FundingTrackedAssets,
  pub has_pending_funding: bool,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum FundingProvenance<AccountId> {
  Signed(AccountId),
  InternalProtocol(AccountId),
  Xcm(AccountId),
}

impl<AccountId> FundingProvenance<AccountId> {
  pub fn account(&self) -> &AccountId {
    match self {
      Self::Signed(account) | Self::InternalProtocol(account) | Self::Xcm(account) => account,
    }
  }
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct IngressOverflowEvent<AaaId, AssetId, Balance, AccountId> {
  pub aaa_id: AaaId,
  pub asset: AssetId,
  pub amount: Balance,
  pub provenance: Option<FundingProvenance<AccountId>>,
}
