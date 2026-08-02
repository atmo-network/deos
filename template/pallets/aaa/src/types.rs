use frame::prelude::*;
use polkadot_sdk::sp_runtime::Perbill;

pub type AaaId = u64;
pub type QueueTicket = u64;
pub type QueuePageId = u64;
pub type WakeupPageId = u64;
pub type WakeupSlot = u32;
pub type WakeupCursorIndex = u32;
pub type ObservationRevision = u64;

/// Global cache epoch stamp shared by every executable Active actor (spec 2.1, 5.4).
pub type CacheEpoch = u32;

/// Durable global cache-revalidation progress (spec 5.4). The workset is the exact set of
/// Active actors at upgrade start; `cursor` marks the last processed workset key, and
/// `remaining` counts members not yet confirmed, shrinking on close/deactivation removal and
/// on each stamped or skipped worker visit. The state clears atomically at `remaining == 0`.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct CacheRevalidationState<Cursor> {
  pub target_epoch: CacheEpoch,
  pub cursor: Cursor,
  pub remaining: u32,
}

/// Disposition applied by the bounded revalidation worker to an Active actor whose plan no
/// longer admits under current bindings (spec 6.4). The migration-specific contract MUST name
/// one; without it a cache-affecting runtime change MUST NOT activate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevalidationDisposition {
  /// Remove the Active epoch while preserving identity, locator, nonce, and balances.
  Deactivate,
  /// Delete actor semantics while preserving sovereign balances.
  Close(CloseReason),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ObservationSubscriberPageList {
  pub head: u32,
  pub tail: u32,
  pub count: u32,
}

#[derive(
  polkadot_sdk::frame_support::CloneNoBound,
  polkadot_sdk::frame_support::DebugNoBound,
  polkadot_sdk::frame_support::PartialEqNoBound,
  polkadot_sdk::frame_support::EqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxEntries))]
pub struct ObservationSubscriberPage<MaxEntries: Get<u32>> {
  pub previous: Option<u32>,
  pub next: Option<u32>,
  pub entries: BoundedVec<Option<AaaId>, MaxEntries>,
}

impl<MaxEntries: Get<u32>> Default for ObservationSubscriberPage<MaxEntries> {
  fn default() -> Self {
    Self {
      previous: None,
      next: None,
      entries: BoundedVec::default(),
    }
  }
}

impl<MaxEntries: Get<u32>> core::ops::Deref for ObservationSubscriberPage<MaxEntries> {
  type Target = BoundedVec<Option<AaaId>, MaxEntries>;

  fn deref(&self) -> &Self::Target {
    &self.entries
  }
}

impl<MaxEntries: Get<u32>> core::ops::DerefMut for ObservationSubscriberPage<MaxEntries> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.entries
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DirtyObservationState<FeedId, BlockNumber> {
  pub latest_revision: ObservationRevision,
  pub fanout_revision: ObservationRevision,
  pub dirty_since: BlockNumber,
  pub next_subscriber_page: Option<u32>,
  pub previous_dirty_feed: Option<FeedId>,
  pub next_dirty_feed: Option<FeedId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DirtyObservationList<FeedId> {
  pub head: Option<FeedId>,
  pub tail: Option<FeedId>,
  pub cursor: Option<FeedId>,
  pub count: u32,
}

impl<FeedId> Default for DirtyObservationList<FeedId> {
  fn default() -> Self {
    Self {
      head: None,
      tail: None,
      cursor: None,
      count: 0,
    }
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum IdleStarvationPhase<BlockNumber> {
  #[default]
  Healthy,
  Starving {
    since: BlockNumber,
  },
  Alerted {
    since: BlockNumber,
  },
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPointer<BlockNumber> {
  pub block: BlockNumber,
  pub page_id: WakeupPageId,
  pub slot: WakeupSlot,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupEntry {
  pub aaa_id: AaaId,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPage<Entries> {
  pub entries: Entries,
  pub live_entries: u32,
  pub scan_slot: WakeupSlot,
  pub previous_page: Option<WakeupPageId>,
  pub next_page: Option<WakeupPageId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupBucketState {
  pub head_page: WakeupPageId,
  pub tail_page: WakeupPageId,
  pub next_page_id: WakeupPageId,
  pub live_entries: u32,
  pub cursor_index: Option<WakeupCursorIndex>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct QueueEntry {
  pub ticket: QueueTicket,
  pub aaa_id: AaaId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueDrainStats {
  pub entries_scanned: u32,
  pub tombstones_skipped: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeupDrainStats {
  pub entries_scanned: u32,
  pub ready_entries: u32,
  pub stale_entries: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AmountResolution<Balance> {
  Fixed(Balance),
  PercentageOfCurrent(Perbill),
  PercentageAtOpening(Perbill),
  PercentageOfLastFunding(Perbill),
  AllAvailable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum InputLimit<Balance> {
  LiveQuote,
  Absolute(Balance),
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
  SwapIn {
    asset_in: AssetId,
    amount_in: AmountResolution<Balance>,
    asset_out: AssetId,
    slippage_tolerance: Perbill,
  },
  SwapOut {
    asset_out: AssetId,
    amount_out: AmountResolution<Balance>,
    asset_in: AssetId,
    input_limit: InputLimit<Balance>,
    slippage_tolerance: Perbill,
  },
  AddLiquidity {
    asset_a: AssetId,
    asset_b: AssetId,
    amount_a: AmountResolution<Balance>,
    amount_b: AmountResolution<Balance>,
    min_lp_out: Balance,
  },
  RemoveLiquidity {
    lp_asset: AssetId,
    asset_a: AssetId,
    asset_b: AssetId,
    lp_amount: AmountResolution<Balance>,
    min_amount_a: Balance,
    min_amount_b: Balance,
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
    max_amount_a: AmountResolution<Balance>,
    max_ratio_error: Perbill,
  },
  Unstake {
    asset: AssetId,
    shares: AmountResolution<Balance>,
  },
  StopCycle,
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
      Self::SwapIn {
        asset_in,
        amount_in,
        asset_out,
        slippage_tolerance,
      } => Self::SwapIn {
        asset_in: asset_in.clone(),
        amount_in: amount_in.clone(),
        asset_out: asset_out.clone(),
        slippage_tolerance: *slippage_tolerance,
      },
      Self::SwapOut {
        asset_out,
        amount_out,
        asset_in,
        input_limit,
        slippage_tolerance,
      } => Self::SwapOut {
        asset_out: asset_out.clone(),
        amount_out: amount_out.clone(),
        asset_in: asset_in.clone(),
        input_limit: input_limit.clone(),
        slippage_tolerance: *slippage_tolerance,
      },
      Self::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
        min_lp_out,
      } => Self::AddLiquidity {
        asset_a: asset_a.clone(),
        asset_b: asset_b.clone(),
        amount_a: amount_a.clone(),
        amount_b: amount_b.clone(),
        min_lp_out: min_lp_out.clone(),
      },
      Self::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount,
        min_amount_a,
        min_amount_b,
      } => Self::RemoveLiquidity {
        lp_asset: lp_asset.clone(),
        asset_a: asset_a.clone(),
        asset_b: asset_b.clone(),
        lp_amount: lp_amount.clone(),
        min_amount_a: min_amount_a.clone(),
        min_amount_b: min_amount_b.clone(),
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
        max_amount_a,
        max_ratio_error,
      } => Self::DonateLiquidity {
        asset_a: asset_a.clone(),
        asset_b: asset_b.clone(),
        max_amount_a: max_amount_a.clone(),
        max_ratio_error: *max_ratio_error,
      },
      Self::Unstake { asset, shares } => Self::Unstake {
        asset: asset.clone(),
        shares: shares.clone(),
      },
      Self::StopCycle => Self::StopCycle,
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
      Self::SwapIn {
        asset_in,
        amount_in,
        asset_out,
        slippage_tolerance,
      } => f
        .debug_struct("SwapIn")
        .field("asset_in", asset_in)
        .field("amount_in", amount_in)
        .field("asset_out", asset_out)
        .field("slippage_tolerance", slippage_tolerance)
        .finish(),
      Self::SwapOut {
        asset_out,
        amount_out,
        asset_in,
        input_limit,
        slippage_tolerance,
      } => f
        .debug_struct("SwapOut")
        .field("asset_out", asset_out)
        .field("amount_out", amount_out)
        .field("asset_in", asset_in)
        .field("input_limit", input_limit)
        .field("slippage_tolerance", slippage_tolerance)
        .finish(),
      Self::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
        min_lp_out,
      } => f
        .debug_struct("AddLiquidity")
        .field("asset_a", asset_a)
        .field("asset_b", asset_b)
        .field("amount_a", amount_a)
        .field("amount_b", amount_b)
        .field("min_lp_out", min_lp_out)
        .finish(),
      Self::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount,
        min_amount_a,
        min_amount_b,
      } => f
        .debug_struct("RemoveLiquidity")
        .field("lp_asset", lp_asset)
        .field("asset_a", asset_a)
        .field("asset_b", asset_b)
        .field("lp_amount", lp_amount)
        .field("min_amount_a", min_amount_a)
        .field("min_amount_b", min_amount_b)
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
        max_amount_a,
        max_ratio_error,
      } => f
        .debug_struct("DonateLiquidity")
        .field("asset_a", asset_a)
        .field("asset_b", asset_b)
        .field("max_amount_a", max_amount_a)
        .field("max_ratio_error", max_ratio_error)
        .finish(),
      Self::Unstake { asset, shares } => f
        .debug_struct("Unstake")
        .field("asset", asset)
        .field("shares", shares)
        .finish(),
      Self::StopCycle => f.write_str("StopCycle"),
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
        Self::SwapIn {
          asset_in: left_asset_in,
          amount_in: left_amount_in,
          asset_out: left_asset_out,
          slippage_tolerance: left_slippage,
        },
        Self::SwapIn {
          asset_in: right_asset_in,
          amount_in: right_amount_in,
          asset_out: right_asset_out,
          slippage_tolerance: right_slippage,
        },
      ) => {
        left_asset_in == right_asset_in
          && left_asset_out == right_asset_out
          && left_amount_in == right_amount_in
          && left_slippage == right_slippage
      }
      (
        Self::SwapOut {
          asset_out: left_asset_out,
          amount_out: left_amount_out,
          asset_in: left_asset_in,
          input_limit: left_input_limit,
          slippage_tolerance: left_slippage,
        },
        Self::SwapOut {
          asset_out: right_asset_out,
          amount_out: right_amount_out,
          asset_in: right_asset_in,
          input_limit: right_input_limit,
          slippage_tolerance: right_slippage,
        },
      ) => {
        left_asset_in == right_asset_in
          && left_asset_out == right_asset_out
          && left_amount_out == right_amount_out
          && left_input_limit == right_input_limit
          && left_slippage == right_slippage
      }
      (
        Self::AddLiquidity {
          asset_a: left_asset_a,
          asset_b: left_asset_b,
          amount_a: left_amount_a,
          amount_b: left_amount_b,
          min_lp_out: left_min_lp_out,
        },
        Self::AddLiquidity {
          asset_a: right_asset_a,
          asset_b: right_asset_b,
          amount_a: right_amount_a,
          amount_b: right_amount_b,
          min_lp_out: right_min_lp_out,
        },
      ) => {
        left_asset_a == right_asset_a
          && left_asset_b == right_asset_b
          && left_amount_a == right_amount_a
          && left_amount_b == right_amount_b
          && left_min_lp_out == right_min_lp_out
      }
      (
        Self::RemoveLiquidity {
          lp_asset: left_lp_asset,
          asset_a: left_asset_a,
          asset_b: left_asset_b,
          lp_amount: left_lp_amount,
          min_amount_a: left_min_amount_a,
          min_amount_b: left_min_amount_b,
        },
        Self::RemoveLiquidity {
          lp_asset: right_lp_asset,
          asset_a: right_asset_a,
          asset_b: right_asset_b,
          lp_amount: right_lp_amount,
          min_amount_a: right_min_amount_a,
          min_amount_b: right_min_amount_b,
        },
      ) => {
        left_lp_asset == right_lp_asset
          && left_asset_a == right_asset_a
          && left_asset_b == right_asset_b
          && left_lp_amount == right_lp_amount
          && left_min_amount_a == right_min_amount_a
          && left_min_amount_b == right_min_amount_b
      }
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
          max_amount_a: left_max_amount_a,
          max_ratio_error: left_max_ratio_error,
        },
        Self::DonateLiquidity {
          asset_a: right_asset_a,
          asset_b: right_asset_b,
          max_amount_a: right_max_amount_a,
          max_ratio_error: right_max_ratio_error,
        },
      ) => {
        left_asset_a == right_asset_a
          && left_asset_b == right_asset_b
          && left_max_amount_a == right_max_amount_a
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
      (Self::StopCycle, Self::StopCycle) => true,
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

/// Lifecycle at the moment a fresh actor identity is created. Excludes Paused by
/// construction; a newly created actor is either Dormant or Active.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum InitialLifecycle {
  Dormant,
  Active,
}

pub type SystemSovereignId = u64;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SystemSovereignState {
  Vacant,
  Occupied(AaaId),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorClass {
  User { owner_slot: u8 },
  System { sovereign_id: SystemSovereignId },
}

impl ActorClass {
  pub fn aaa_type(self) -> AaaType {
    match self {
      Self::User { .. } => AaaType::User,
      Self::System { .. } => AaaType::System,
    }
  }

  pub fn owner_slot(self) -> Option<u8> {
    match self {
      Self::User { owner_slot } => Some(owner_slot),
      Self::System { .. } => None,
    }
  }

  pub fn system_sovereign_id(self) -> Option<SystemSovereignId> {
    match self {
      Self::User { .. } => None,
      Self::System { sovereign_id } => Some(sovereign_id),
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
  RetryAttemptsExhausted,
  ProductiveRunCompleted,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum CompletionPolicy {
  #[default]
  Persistent,
  CloseAfterProductiveRun,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum StepErrorPolicy {
  AbortCycle,
  ContinueNextStep,
  RetryLater { max_attempts: u32 },
}

impl StepErrorPolicy {
  pub fn retry_max_attempts(self) -> Option<u32> {
    match self {
      Self::RetryLater { max_attempts } => Some(max_attempts),
      Self::AbortCycle | Self::ContinueNextStep => None,
    }
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SuspensionReason {
  FundingUnavailable,
  Temporary,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum CycleResult {
  Completed,
  Failed,
  Cancelled,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum CancellationReason {
  Explicit,
  ExecutionPlanChanged,
  CompletionPolicyChanged,
  FundingPolicyChanged,
  ScheduleChanged,
  Deactivated,
  Closing(CloseReason),
  RuntimeUpgrade,
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
pub enum SimulationMode {
  FreshCurrentPlan,
  CurrentContinuation,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SimulationStatus {
  Completed,
  Aborted,
  Suspended,
  Closed(CloseReason),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SimulationStepOutcome {
  Executed,
  Skipped(StepSkippedReason),
  Failed(crate::RetryClass),
  Suspended(SuspensionReason),
  Stopped,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct SimulationStepRecord {
  pub step_index: u32,
  pub outcome: SimulationStepOutcome,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SimulationError {
  ActorNotFound,
  ProgramMismatch,
  TypeMismatch,
  MutabilityMismatch,
  ModeCycleStateMismatch,
  CacheRevalidationActive,
  GlobalCircuitBreaker,
  WindowExpired,
  Paused,
  CycleNonceExhausted,
  ConsecutiveFailures,
  NotReady,
  BalanceUnavailable,
  FeeBudgetUnavailable,
  ContinuationInvariant,
  TransactionDepthExceeded,
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for SimulationError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::TransactionDepthExceeded
  }
}

#[derive(
  polkadot_sdk::frame_support::CloneNoBound,
  polkadot_sdk::frame_support::DebugNoBound,
  polkadot_sdk::frame_support::PartialEqNoBound,
  polkadot_sdk::frame_support::EqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
)]
#[scale_info(skip_type_params(MaxExecutionPlanSteps))]
pub struct SimulationResult<MaxExecutionPlanSteps: Get<u32>> {
  pub status: SimulationStatus,
  pub cycle_nonce: u64,
  pub attempt: u32,
  pub start_cursor: u32,
  pub continuation_cursor: Option<u32>,
  pub unsuccessful_attempts_at_cursor: Option<u32>,
  pub finalized_through: Option<u32>,
  pub cumulative_outcomes: OutcomeTotals,
  pub steps: BoundedVec<SimulationStepRecord, MaxExecutionPlanSteps>,
}

/// Read-only scheduler-owned readiness phase for the eligibility projection
/// (spec 7.3). Clients read one runtime API instead of reimplementing cadence
/// phase, cooldown, window floor, retry backoff, breaker, and latch arithmetic.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AaaEligibilityPhase {
  /// No identity is registered under the id.
  NotRegistered,
  /// A dormant identity exists without an Active program.
  Dormant,
  /// The actor is temporally and trigger-ready for scheduler admission now.
  Ready,
  /// Manual pause blocks execution.
  Paused,
  /// The global circuit breaker blocks all execution.
  GlobalCircuitBreaker,
  /// The schedule window ended; window-expiry closure is due.
  WindowExpired,
  /// Cycle nonce exhausted; closure is due.
  CycleNonceExhausted,
  /// The consecutive-failure limit is reached; closure is due.
  ConsecutiveFailureLimit,
  /// The configured auto-close nonce is reached; closure precedes the next cycle.
  AutoCloseDue,
  /// The temporal gate is open but the pending-signal latch is absent.
  WaitingSignal,
  /// A suspended run waits for retry backoff or cooldown before the next attempt.
  WaitingRetry,
  /// Cooldown, window floor, or cadence has not yet opened the temporal gate.
  WaitingTemporal,
}

/// One read-only eligibility projection (spec 7.3). `ready` is the scheduler
/// verdict at the read block; `phase` explains it; `next_eligible_block` is the
/// next block at which temporal eligibility opens (`now` when `ready`), or
/// `None` when no future temporal gate is computable.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct AaaEligibilityProjection<BlockNumber> {
  pub ready: bool,
  pub phase: AaaEligibilityPhase,
  pub next_eligible_block: Option<BlockNumber>,
}

/// Why the eligibility projection could not be computed.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AaaEligibilityError {
  /// Next-eligible arithmetic overflowed the block-number domain.
  ComputationOverflow,
  /// A suspended actor lacks its mandatory Continuation state.
  ContinuationInvariant,
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

impl<AccountId: Encode, MaxWhitelistSize: Get<u32>> SourceFilter<AccountId, MaxWhitelistSize> {
  pub fn has_canonical_members(&self) -> bool {
    match self {
      Self::Any | Self::OwnerOnly => true,
      Self::Whitelist(list) => is_non_empty_strictly_scale_ordered(list),
    }
  }
}

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

impl<AssetId: Encode, MaxWhitelistSize: Get<u32>> AssetFilter<AssetId, MaxWhitelistSize> {
  pub fn has_canonical_members(&self) -> bool {
    match self {
      Self::Any => true,
      Self::Whitelist(list) => is_non_empty_strictly_scale_ordered(list),
    }
  }
}

fn is_non_empty_strictly_scale_ordered<Value: Encode, Bound: Get<u32>>(
  values: &BoundedVec<Value, Bound>,
) -> bool {
  !values.is_empty()
    && values
      .windows(2)
      .all(|pair| pair[0].encode() < pair[1].encode())
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize))]
pub enum TriggerSource<AccountId, AssetId, MaxWhitelistSize: Get<u32>, ObservationFeedId = AssetId>
{
  Manual,
  OnAddressEvent {
    source_filter: SourceFilter<AccountId, MaxWhitelistSize>,
    asset_filter: AssetFilter<AssetId, MaxWhitelistSize>,
  },
  OnObservationChange {
    feed: ObservationFeedId,
  },
}

impl<AccountId: Clone, AssetId: Clone, MaxWhitelistSize: Get<u32>, ObservationFeedId: Clone> Clone
  for TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>
{
  fn clone(&self) -> Self {
    match self {
      Self::Manual => Self::Manual,
      Self::OnAddressEvent {
        source_filter,
        asset_filter,
      } => Self::OnAddressEvent {
        source_filter: source_filter.clone(),
        asset_filter: asset_filter.clone(),
      },
      Self::OnObservationChange { feed } => Self::OnObservationChange { feed: feed.clone() },
    }
  }
}

impl<
  AccountId: core::fmt::Debug,
  AssetId: core::fmt::Debug,
  MaxWhitelistSize: Get<u32>,
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug for TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Manual => f.write_str("Manual"),
      Self::OnAddressEvent {
        source_filter,
        asset_filter,
      } => f
        .debug_struct("OnAddressEvent")
        .field("source_filter", source_filter)
        .field("asset_filter", asset_filter)
        .finish(),
      Self::OnObservationChange { feed } => f
        .debug_struct("OnObservationChange")
        .field("feed", feed)
        .finish(),
    }
  }
}

impl<
  AccountId: PartialEq,
  AssetId: PartialEq,
  MaxWhitelistSize: Get<u32>,
  ObservationFeedId: PartialEq,
> PartialEq for TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Manual, Self::Manual) => true,
      (
        Self::OnAddressEvent {
          source_filter: left_source,
          asset_filter: left_asset,
        },
        Self::OnAddressEvent {
          source_filter: right_source,
          asset_filter: right_asset,
        },
      ) => left_source == right_source && left_asset == right_asset,
      (Self::OnObservationChange { feed: left }, Self::OnObservationChange { feed: right }) => {
        left == right
      }
      _ => false,
    }
  }
}

impl<AccountId: Eq, AssetId: Eq, MaxWhitelistSize: Get<u32>, ObservationFeedId: Eq> Eq
  for TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>
{
}

impl<AccountId: Encode, AssetId: Encode, MaxWhitelistSize: Get<u32>, ObservationFeedId: Encode>
  TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>
{
  pub fn has_canonical_filters(&self) -> bool {
    match self {
      Self::Manual => true,
      Self::OnAddressEvent {
        source_filter,
        asset_filter,
      } => source_filter.has_canonical_members() && asset_filter.has_canonical_members(),
      Self::OnObservationChange { .. } => true,
    }
  }
}

pub type TriggerSources<
  AccountId,
  AssetId,
  MaxWhitelistSize,
  MaxTriggerSources,
  ObservationFeedId = AssetId,
> = BoundedVec<
  TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>,
  MaxTriggerSources,
>;

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize, MaxTriggerSources))]
pub enum CadenceMode<
  AccountId,
  AssetId,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId = AssetId,
> {
  Always,
  WhenSignalled(
    TriggerSources<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
  ),
}

impl<
  AccountId: Clone,
  AssetId: Clone,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Clone,
> Clone
  for CadenceMode<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn clone(&self) -> Self {
    match self {
      Self::Always => Self::Always,
      Self::WhenSignalled(sources) => Self::WhenSignalled(sources.clone()),
    }
  }
}

impl<
  AccountId: core::fmt::Debug,
  AssetId: core::fmt::Debug,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug
  for CadenceMode<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Always => f.write_str("Always"),
      Self::WhenSignalled(sources) => f.debug_tuple("WhenSignalled").field(sources).finish(),
    }
  }
}

impl<
  AccountId: PartialEq,
  AssetId: PartialEq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: PartialEq,
> PartialEq
  for CadenceMode<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Always, Self::Always) => true,
      (Self::WhenSignalled(left), Self::WhenSignalled(right)) => left == right,
      _ => false,
    }
  }
}

impl<
  AccountId: Eq,
  AssetId: Eq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Eq,
> Eq for CadenceMode<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize, MaxTriggerSources))]
pub enum TriggerPolicy<
  AccountId,
  AssetId,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId = AssetId,
> {
  Immediate {
    sources:
      TriggerSources<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
  },
  Cadenced {
    every_blocks: u32,
    mode: CadenceMode<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
  },
}

impl<
  AccountId: Clone,
  AssetId: Clone,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Clone,
> Clone
  for TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn clone(&self) -> Self {
    match self {
      Self::Immediate { sources } => Self::Immediate {
        sources: sources.clone(),
      },
      Self::Cadenced { every_blocks, mode } => Self::Cadenced {
        every_blocks: *every_blocks,
        mode: mode.clone(),
      },
    }
  }
}

impl<
  AccountId: core::fmt::Debug,
  AssetId: core::fmt::Debug,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug
  for TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Immediate { sources } => f
        .debug_struct("Immediate")
        .field("sources", sources)
        .finish(),
      Self::Cadenced { every_blocks, mode } => f
        .debug_struct("Cadenced")
        .field("every_blocks", every_blocks)
        .field("mode", mode)
        .finish(),
    }
  }
}

impl<
  AccountId: PartialEq,
  AssetId: PartialEq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: PartialEq,
> PartialEq
  for TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Immediate { sources: left }, Self::Immediate { sources: right }) => left == right,
      (
        Self::Cadenced {
          every_blocks: left_blocks,
          mode: left_mode,
        },
        Self::Cadenced {
          every_blocks: right_blocks,
          mode: right_mode,
        },
      ) => left_blocks == right_blocks && left_mode == right_mode,
      _ => false,
    }
  }
}

impl<
  AccountId: Eq,
  AssetId: Eq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Eq,
> Eq for TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
}

impl<
  AccountId: Encode,
  AssetId: Encode,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Encode,
> TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  pub fn immediate_manual() -> Self {
    Self::Immediate {
      sources: BoundedVec::try_from(alloc::vec![TriggerSource::Manual])
        .unwrap_or_else(|_| panic!("MaxTriggerSources must admit Manual")),
    }
  }

  pub fn immediate_address_event(
    source_filter: SourceFilter<AccountId, MaxWhitelistSize>,
    asset_filter: AssetFilter<AssetId, MaxWhitelistSize>,
  ) -> Self {
    Self::Immediate {
      sources: BoundedVec::try_from(alloc::vec![TriggerSource::OnAddressEvent {
        source_filter,
        asset_filter,
      }])
      .unwrap_or_else(|_| panic!("MaxTriggerSources must admit AddressEvent")),
    }
  }

  pub fn immediate_manual_and_address_event(
    source_filter: SourceFilter<AccountId, MaxWhitelistSize>,
    asset_filter: AssetFilter<AssetId, MaxWhitelistSize>,
  ) -> Self {
    Self::Immediate {
      sources: BoundedVec::try_from(alloc::vec![
        TriggerSource::Manual,
        TriggerSource::OnAddressEvent {
          source_filter,
          asset_filter,
        },
      ])
      .unwrap_or_else(|_| panic!("MaxTriggerSources must admit Manual and AddressEvent")),
    }
  }

  pub fn cadenced_always(every_blocks: u32) -> Self {
    Self::Cadenced {
      every_blocks,
      mode: CadenceMode::Always,
    }
  }

  pub fn cadenced_when_signalled_manual(every_blocks: u32) -> Self {
    Self::Cadenced {
      every_blocks,
      mode: CadenceMode::WhenSignalled(
        BoundedVec::try_from(alloc::vec![TriggerSource::Manual])
          .unwrap_or_else(|_| panic!("MaxTriggerSources must admit Manual")),
      ),
    }
  }

  pub fn cadenced_when_signalled_address_event(
    every_blocks: u32,
    source_filter: SourceFilter<AccountId, MaxWhitelistSize>,
    asset_filter: AssetFilter<AssetId, MaxWhitelistSize>,
  ) -> Self {
    Self::Cadenced {
      every_blocks,
      mode: CadenceMode::WhenSignalled(
        BoundedVec::try_from(alloc::vec![TriggerSource::OnAddressEvent {
          source_filter,
          asset_filter,
        }])
        .unwrap_or_else(|_| panic!("MaxTriggerSources must admit AddressEvent")),
      ),
    }
  }

  pub fn sources(
    &self,
  ) -> Option<
    &TriggerSources<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
  > {
    match self {
      Self::Immediate { sources }
      | Self::Cadenced {
        mode: CadenceMode::WhenSignalled(sources),
        ..
      } => Some(sources),
      Self::Cadenced {
        mode: CadenceMode::Always,
        ..
      } => None,
    }
  }

  pub fn has_canonical_sources(&self) -> bool {
    let Some(sources) = self.sources() else {
      return true;
    };
    let mut filters_are_canonical = true;
    for source in sources.as_slice() {
      if !source.has_canonical_filters() {
        filters_are_canonical = false;
        break;
      }
    }
    !sources.is_empty()
      && filters_are_canonical
      && sources.windows(2).all(|pair| {
        let left = pair[0].encode();
        let right = pair[1].encode();
        left < right
      })
  }

  pub fn cadence_blocks(&self) -> Option<u32> {
    match self {
      Self::Immediate { .. } => None,
      Self::Cadenced { every_blocks, .. } => Some(*every_blocks),
    }
  }

  fn source_enabled(
    &self,
    predicate: impl Fn(&TriggerSource<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>) -> bool,
  ) -> bool {
    let Some(sources) = self.sources() else {
      return false;
    };
    for source in sources.as_slice() {
      if predicate(source) {
        return true;
      }
    }
    false
  }

  pub fn manual_source_enabled(&self) -> bool {
    self.source_enabled(|source| matches!(source, TriggerSource::Manual))
  }

  pub fn address_event_source_enabled(&self) -> bool {
    self.source_enabled(|source| matches!(source, TriggerSource::OnAddressEvent { .. }))
  }

  pub fn observation_source_enabled(&self) -> bool {
    self.source_enabled(|source| matches!(source, TriggerSource::OnObservationChange { .. }))
  }
}

pub type Trigger<
  AccountId,
  AssetId,
  MaxWhitelistSize,
  MaxTriggerSources,
  ObservationFeedId = AssetId,
> = TriggerPolicy<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>;

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxWhitelistSize, MaxTriggerSources))]
pub struct Schedule<
  AccountId,
  AssetId,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId = AssetId,
> {
  pub trigger: Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
  pub cooldown_blocks: u32,
}

impl<
  AccountId: Clone,
  AssetId: Clone,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Clone,
> Clone for Schedule<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn clone(&self) -> Self {
    Self {
      trigger: self.trigger.clone(),
      cooldown_blocks: self.cooldown_blocks,
    }
  }
}

impl<
  AccountId: core::fmt::Debug,
  AssetId: core::fmt::Debug,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug
  for Schedule<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Schedule")
      .field("trigger", &self.trigger)
      .field("cooldown_blocks", &self.cooldown_blocks)
      .finish()
  }
}

impl<
  AccountId: PartialEq,
  AssetId: PartialEq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: PartialEq,
> PartialEq
  for Schedule<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn eq(&self, other: &Self) -> bool {
    self.trigger == other.trigger && self.cooldown_blocks == other.cooldown_blocks
  }
}

impl<
  AccountId: Eq,
  AssetId: Eq,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Eq,
> Eq for Schedule<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum Condition<AssetId, Balance, BlockNumber = u32, ObservationFeedId = ()> {
  BalanceAbove {
    asset: AssetId,
    threshold: Balance,
  },
  BalanceBelow {
    asset: AssetId,
    threshold: Balance,
  },
  BalanceEquals {
    asset: AssetId,
    threshold: Balance,
  },
  BalanceNotEquals {
    asset: AssetId,
    threshold: Balance,
  },
  BlockNumberAbove {
    threshold: BlockNumber,
  },
  BlockNumberBelow {
    threshold: BlockNumber,
  },
  ObservationAbove {
    feed: ObservationFeedId,
    threshold: u128,
    max_age_blocks: u32,
  },
  ObservationBelow {
    feed: ObservationFeedId,
    threshold: u128,
    max_age_blocks: u32,
  },
  ObservationEquals {
    feed: ObservationFeedId,
    threshold: u128,
    max_age_blocks: u32,
  },
  ObservationNotEquals {
    feed: ObservationFeedId,
    threshold: u128,
    max_age_blocks: u32,
  },
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxConditions))]
pub enum ConditionSet<C, MaxConditions: Get<u32>> {
  Always,
  All(BoundedVec<C, MaxConditions>),
  Any(BoundedVec<C, MaxConditions>),
}

impl<C, MaxConditions: Get<u32>> Default for ConditionSet<C, MaxConditions> {
  fn default() -> Self {
    Self::Always
  }
}

impl<C, MaxConditions: Get<u32>> ConditionSet<C, MaxConditions> {
  pub fn len(&self) -> u32 {
    match self {
      Self::Always => 0,
      Self::All(conditions) | Self::Any(conditions) => conditions.len() as u32,
    }
  }

  pub fn is_always(&self) -> bool {
    matches!(self, Self::Always)
  }
}

impl<C: Clone, MaxConditions: Get<u32>> Clone for ConditionSet<C, MaxConditions> {
  fn clone(&self) -> Self {
    match self {
      Self::Always => Self::Always,
      Self::All(conditions) => Self::All(conditions.clone()),
      Self::Any(conditions) => Self::Any(conditions.clone()),
    }
  }
}

impl<C: core::fmt::Debug, MaxConditions: Get<u32>> core::fmt::Debug
  for ConditionSet<C, MaxConditions>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Always => f.write_str("Always"),
      Self::All(conditions) => f.debug_tuple("All").field(conditions).finish(),
      Self::Any(conditions) => f.debug_tuple("Any").field(conditions).finish(),
    }
  }
}

impl<C: PartialEq, MaxConditions: Get<u32>> PartialEq for ConditionSet<C, MaxConditions> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Always, Self::Always) => true,
      (Self::All(left), Self::All(right)) | (Self::Any(left), Self::Any(right)) => left == right,
      _ => false,
    }
  }
}

impl<C: Eq, MaxConditions: Get<u32>> Eq for ConditionSet<C, MaxConditions> {}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxConditionsPerStep, MaxSplitTransferLegs))]
pub struct Step<
  AssetId,
  Balance,
  AccountId,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId = (),
> {
  pub conditions:
    ConditionSet<Condition<AssetId, Balance, u32, ObservationFeedId>, MaxConditionsPerStep>,
  pub task: Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
  pub on_error: StepErrorPolicy,
}

impl<
  AssetId: Clone,
  Balance: Clone,
  AccountId: Clone,
  MaxConditionsPerStep: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId: Clone,
> Clone
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxConditionsPerStep,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
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
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxConditionsPerStep,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
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
  ObservationFeedId: PartialEq,
> PartialEq
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxConditionsPerStep,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
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
  ObservationFeedId: Eq,
> Eq
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxConditionsPerStep,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
{
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ScheduleWindow<BlockNumber> {
  pub start: BlockNumber,
  pub end: BlockNumber,
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSignedFundingSources))]
pub enum FundingSourcePolicy<AccountId, MaxSignedFundingSources: Get<u32>> {
  OwnerOnly,
  SignedAllowlist(BoundedBTreeSet<AccountId, MaxSignedFundingSources>),
  RuntimePolicy,
  AnyVerifiedIngress,
}

impl<AccountId: Clone, MaxSignedFundingSources: Get<u32>> Clone
  for FundingSourcePolicy<AccountId, MaxSignedFundingSources>
{
  fn clone(&self) -> Self {
    match self {
      Self::OwnerOnly => Self::OwnerOnly,
      Self::SignedAllowlist(allowed) => Self::SignedAllowlist(allowed.clone()),
      Self::RuntimePolicy => Self::RuntimePolicy,
      Self::AnyVerifiedIngress => Self::AnyVerifiedIngress,
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
      Self::AnyVerifiedIngress => f.write_str("AnyVerifiedIngress"),
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
      | (Self::AnyVerifiedIngress, Self::AnyVerifiedIngress) => true,
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
pub struct ActiveProgramInput<Schedule, BlockNumber, ExecutionPlan, FundingPolicy> {
  pub schedule: Schedule,
  pub schedule_window: Option<ScheduleWindow<BlockNumber>>,
  pub execution_plan: ExecutionPlan,
  pub completion_policy: CompletionPolicy,
  pub funding_source_policy: FundingPolicy,
  pub auto_close_at_cycle_nonce: Option<u64>,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProgramInput<Schedule, BlockNumber, ExecutionPlan, FundingPolicy> {
  Dormant,
  Active(ActiveProgramInput<Schedule, BlockNumber, ExecutionPlan, FundingPolicy>),
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum CycleState {
  #[default]
  Idle,
  Suspended,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum OpeningSurface<AssetId> {
  PreservableAsset(AssetId),
  TargetAsset(AssetId),
  StakingShares(AssetId),
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub struct OutcomeTotals {
  pub executed_steps: u32,
  pub committed_effectful_tasks: u32,
  pub skipped_conditions: u32,
  pub skipped_resolution: u32,
  pub skipped_funding_unavailable: u32,
  pub failed_steps: u32,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSnapshotEntries, MaxFundingTrackedAssets))]
pub struct ContinuationState<
  AssetId,
  Balance,
  BlockNumber,
  MaxSnapshotEntries: Get<u32>,
  MaxFundingTrackedAssets: Get<u32>,
> {
  pub cursor: u32,
  pub attempt: u32,
  pub unsuccessful_attempts_at_cursor: u32,
  pub last_attempt_block: BlockNumber,
  pub opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxSnapshotEntries>,
  pub funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  pub cumulative_outcomes: OutcomeTotals,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorIdentity<AccountId> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub cycle_nonce: u64,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorHotState<BlockNumber, Balance> {
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub consecutive_failures: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
  pub terminal_at: Option<BlockNumber>,
  pub last_control_queue_mutation_block: Option<BlockNumber>,
  pub cycle_weight_upper: Weight,
  pub cycle_fee_upper: Balance,
  pub funding_tracked_count: u32,
  pub schedule_anchor: BlockNumber,
  pub last_cycle_block: Option<BlockNumber>,
  /// Current global cache epoch stamp; an Active actor is executable only while this
  /// equals `CurrentCacheEpoch` and no `CacheRevalidationState` exists (spec 2.1).
  pub cache_epoch: CacheEpoch,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorProgramState<Schedule, BlockNumber, ExecutionPlan> {
  pub schedule: Schedule,
  pub schedule_window: Option<ScheduleWindow<BlockNumber>>,
  pub execution_plan: ExecutionPlan,
  pub completion_policy: CompletionPolicy,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActiveActorView<AccountId, BlockNumber, Schedule, ExecutionPlan, Balance> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub schedule: Schedule,
  pub schedule_window: Option<ScheduleWindow<BlockNumber>>,
  pub execution_plan: ExecutionPlan,
  pub completion_policy: CompletionPolicy,
  pub cycle_nonce: u64,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub consecutive_failures: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub last_control_queue_mutation_block: Option<BlockNumber>,
  pub cycle_weight_upper: Weight,
  pub cycle_fee_upper: Balance,
  pub funding_tracked_count: u32,
  pub schedule_anchor: BlockNumber,
  pub last_cycle_block: Option<BlockNumber>,
  /// Read-only cache stamp mirroring `ActorHotState.cache_epoch` (spec 2.1).
  pub cache_epoch: CacheEpoch,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorFundingState<FundingPolicy, FundingAccumulated, FundingTrackedAssets> {
  pub funding_source_policy: FundingPolicy,
  pub funding_accumulated: FundingAccumulated,
  pub funding_tracked_assets: FundingTrackedAssets,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum FundingProvenance {
  Signed,
  InternalProtocol,
  Xcm,
}

/// One certified AddressEvent ingress movement (spec 3.1, 5.3, 6.2).
///
/// Only certified producers construct this value; the runtime adapter implements
/// `AddressEventIngress::preflight`/`notify` on it. `provenance == None` with
/// `source == None` is the source-less movement surface.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct AddressEvent<AccountId, AssetId, Balance> {
  pub destination: AccountId,
  pub source: Option<AccountId>,
  pub asset: AssetId,
  pub amount: Balance,
  pub provenance: Option<FundingProvenance>,
}
