use super::lifecycle::{CompletionPolicy, StepErrorPolicy};
use frame::prelude::*;
use polkadot_sdk::sp_runtime::Perbill;

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
pub enum Trigger<
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
    sources: Option<
      TriggerSources<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>,
    >,
  },
}

impl<
  AccountId: Clone,
  AssetId: Clone,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Clone,
> Clone for Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn clone(&self) -> Self {
    match self {
      Self::Immediate { sources } => Self::Immediate {
        sources: sources.clone(),
      },
      Self::Cadenced {
        every_blocks,
        sources,
      } => Self::Cadenced {
        every_blocks: *every_blocks,
        sources: sources.clone(),
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
  for Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Immediate { sources } => f
        .debug_struct("Immediate")
        .field("sources", sources)
        .finish(),
      Self::Cadenced {
        every_blocks,
        sources,
      } => f
        .debug_struct("Cadenced")
        .field("every_blocks", every_blocks)
        .field("sources", sources)
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
  for Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Immediate { sources: left }, Self::Immediate { sources: right }) => left == right,
      (
        Self::Cadenced {
          every_blocks: left_blocks,
          sources: left_sources,
        },
        Self::Cadenced {
          every_blocks: right_blocks,
          sources: right_sources,
        },
      ) => left_blocks == right_blocks && left_sources == right_sources,
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
> Eq for Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
{
}

impl<
  AccountId: Encode,
  AssetId: Encode,
  MaxWhitelistSize: Get<u32>,
  MaxTriggerSources: Get<u32>,
  ObservationFeedId: Encode,
> Trigger<AccountId, AssetId, MaxWhitelistSize, MaxTriggerSources, ObservationFeedId>
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
      sources: None,
    }
  }

  pub fn cadenced_when_signalled_manual(every_blocks: u32) -> Self {
    Self::Cadenced {
      every_blocks,
      sources: Some(
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
      sources: Some(
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
      Self::Immediate { sources } => Some(sources),
      Self::Cadenced { sources, .. } => sources.as_ref(),
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
pub enum Predicate<AssetId, Balance, BlockNumber = u32, ObservationFeedId = ()> {
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
pub enum ObservationTiming {
  Opening,
  Current,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum PredicateError {
  InvalidObservation,
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
pub struct TimedPredicate<P> {
  pub timing: ObservationTiming,
  pub predicate: P,
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxClauses, MaxPerClause))]
pub struct Precondition<P, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> {
  pub clauses: BoundedVec<BoundedVec<TimedPredicate<P>, MaxPerClause>, MaxClauses>,
}

impl<P, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> Precondition<P, MaxClauses, MaxPerClause> {
  pub fn predicate_count(&self) -> u32 {
    self
      .clauses
      .iter() // deos-bypass: bounded-iter — MaxClauses bounds the complete visit.
      .map(|clause| clause.len() as u32)
      .sum()
  }

  pub fn opening_predicate_count(&self) -> u32 {
    self
      .clauses
      .iter() // deos-bypass: bounded-iter — MaxClauses bounds the outer visit.
      .flat_map(|clause| {
        clause.iter() // deos-bypass: bounded-iter — MaxPerClause bounds each inner visit.
      })
      .filter(|timed| timed.timing == ObservationTiming::Opening)
      .count() as u32
  }

  pub fn evaluation_units(&self) -> u32 {
    self
      .predicate_count()
      .saturating_add(self.opening_predicate_count())
  }
}

impl<P: Clone, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> Clone
  for Precondition<P, MaxClauses, MaxPerClause>
{
  fn clone(&self) -> Self {
    Self {
      clauses: self.clauses.clone(),
    }
  }
}

impl<P: core::fmt::Debug, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> core::fmt::Debug
  for Precondition<P, MaxClauses, MaxPerClause>
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Precondition")
      .field("clauses", &self.clauses)
      .finish()
  }
}

impl<P: PartialEq, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> PartialEq
  for Precondition<P, MaxClauses, MaxPerClause>
{
  fn eq(&self, other: &Self) -> bool {
    self.clauses == other.clauses
  }
}

impl<P: Eq, MaxClauses: Get<u32>, MaxPerClause: Get<u32>> Eq
  for Precondition<P, MaxClauses, MaxPerClause>
{
}

#[derive(Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(
  MaxPreconditionClauses,
  MaxPredicatesPerClause,
  MaxSplitTransferLegs
))]
pub struct Step<
  AssetId,
  Balance,
  AccountId,
  MaxPreconditionClauses: Get<u32>,
  MaxPredicatesPerClause: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId = (),
> {
  pub precondition: Option<
    Precondition<
      Predicate<AssetId, Balance, u32, ObservationFeedId>,
      MaxPreconditionClauses,
      MaxPredicatesPerClause,
    >,
  >,
  pub task: Task<AssetId, Balance, AccountId, MaxSplitTransferLegs>,
  pub on_error: StepErrorPolicy,
}

impl<
  AssetId: Clone,
  Balance: Clone,
  AccountId: Clone,
  MaxPreconditionClauses: Get<u32>,
  MaxPredicatesPerClause: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId: Clone,
> Clone
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxPreconditionClauses,
    MaxPredicatesPerClause,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
{
  fn clone(&self) -> Self {
    Self {
      precondition: self.precondition.clone(),
      task: self.task.clone(),
      on_error: self.on_error,
    }
  }
}

impl<
  AssetId: core::fmt::Debug,
  Balance: core::fmt::Debug,
  AccountId: core::fmt::Debug,
  MaxPreconditionClauses: Get<u32>,
  MaxPredicatesPerClause: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId: core::fmt::Debug,
> core::fmt::Debug
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxPreconditionClauses,
    MaxPredicatesPerClause,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Step")
      .field("precondition", &self.precondition)
      .field("task", &self.task)
      .field("on_error", &self.on_error)
      .finish()
  }
}

impl<
  AssetId: PartialEq,
  Balance: PartialEq,
  AccountId: PartialEq,
  MaxPreconditionClauses: Get<u32>,
  MaxPredicatesPerClause: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId: PartialEq,
> PartialEq
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxPreconditionClauses,
    MaxPredicatesPerClause,
    MaxSplitTransferLegs,
    ObservationFeedId,
  >
{
  fn eq(&self, other: &Self) -> bool {
    self.precondition == other.precondition
      && self.task == other.task
      && self.on_error == other.on_error
  }
}

impl<
  AssetId: Eq,
  Balance: Eq,
  AccountId: Eq,
  MaxPreconditionClauses: Get<u32>,
  MaxPredicatesPerClause: Get<u32>,
  MaxSplitTransferLegs: Get<u32>,
  ObservationFeedId: Eq,
> Eq
  for Step<
    AssetId,
    Balance,
    AccountId,
    MaxPreconditionClauses,
    MaxPredicatesPerClause,
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
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy> {
  pub trigger: Trigger,
  pub cooldown_blocks: u32,
  pub window: Option<ScheduleWindow<BlockNumber>>,
  pub steps: Steps,
  pub funding: FundingPolicy,
  pub completion: CompletionPolicy,
  pub auto_close_at_cycle_nonce: Option<u64>,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorFundingState<FundingAccumulated, FundingTrackedAssets> {
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
