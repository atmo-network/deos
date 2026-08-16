//! DEOS Router pallet
//!
//! Minimalist multi-token routing system optimized for TMC ecosystems.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod types;
pub use types::{AssetKind, *};

#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

use frame::prelude::*;
use polkadot_sdk::{frame_support::traits::ConstU32, sp_runtime::Perbill};
use scale_info::prelude::vec::Vec;

/// Maximum asset count for the accepted direct and Native-anchored route families.
pub type MaxRouteAssets = ConstU32<3>;

/// Canonical bounded asset path. A path contains one more asset than market legs.
pub type RoutePath = BoundedVec<AssetKind, MaxRouteAssets>;

/// Maximum market-leg count for the accepted route families.
pub type MaxRouteLegs = ConstU32<2>;

#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum PreparedLeg {
  Xyk {
    pool_id: (AssetKind, AssetKind),
    asset_in: AssetKind,
    asset_out: AssetKind,
    quoted_amount_in: Balance,
    quoted_amount_out: Balance,
  },
  TmcMint {
    token_asset: AssetKind,
    collateral_asset: AssetKind,
    quoted_collateral_in: Balance,
    quoted_recipient_out: Balance,
  },
}

pub type PreparedLegs = BoundedVec<PreparedLeg, MaxRouteLegs>;

#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct ExactOutputExecution {
  pub amount_in: Balance,
  pub recipient_amount_out: Balance,
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum RouteFamily {
  DirectXyk,
  DirectMint,
  NativeAnchoredXyk,
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum RouteWeightClass {
  ExactInputDirectXyk,
  ExactInputDirectMint,
  ExactInputNativeAnchoredXyk,
  ExactOutputDirectXyk,
  ExactOutputNativeAnchoredXyk,
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum RouterFailureClass {
  InvalidRequest,
  NoViableRoute,
  ProtectionRejected,
  LiquidityUnavailable,
  FeeRejected,
  PublicationRejected,
  IngressRejected,
  InvariantViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDisposition {
  Permanent,
  RetryLater,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdapterFailure {
  dispatch_error: DispatchError,
  failure_class: RouterFailureClass,
  retry_disposition: RetryDisposition,
}

impl AdapterFailure {
  pub const fn new(
    dispatch_error: DispatchError,
    failure_class: RouterFailureClass,
    retry_disposition: RetryDisposition,
  ) -> Self {
    Self {
      dispatch_error,
      failure_class,
      retry_disposition,
    }
  }

  pub const fn unknown(dispatch_error: DispatchError) -> Self {
    Self::new(
      dispatch_error,
      RouterFailureClass::InvariantViolation,
      RetryDisposition::Permanent,
    )
  }

  pub const fn failure_class(&self) -> RouterFailureClass {
    self.failure_class
  }

  pub const fn retry_disposition(&self) -> RetryDisposition {
    self.retry_disposition
  }

  pub fn into_dispatch_error(self) -> DispatchError {
    self.dispatch_error
  }
}

impl From<DispatchError> for AdapterFailure {
  fn from(error: DispatchError) -> Self {
    Self::unknown(error)
  }
}

pub enum ExecutionError<T: Config> {
  Router(Error<T>),
  Adapter(AdapterFailure),
}

impl<T: Config> core::fmt::Debug for ExecutionError<T> {
  fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    formatter
      .debug_struct("ExecutionError")
      .field("failure_class", &self.failure_class())
      .field("retry_disposition", &self.retry_disposition())
      .finish()
  }
}

impl<T: Config> PartialEq for ExecutionError<T> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Router(left), Self::Router(right)) => left.encode() == right.encode(),
      (Self::Adapter(left), Self::Adapter(right)) => left == right,
      _ => false,
    }
  }
}

impl<T: Config> Eq for ExecutionError<T> {}

impl<T: Config> ExecutionError<T> {
  pub fn failure_class(&self) -> RouterFailureClass {
    match self {
      Self::Router(error) => error.failure_class(),
      Self::Adapter(failure) => failure.failure_class(),
    }
  }

  pub fn retry_disposition(&self) -> RetryDisposition {
    match self {
      Self::Router(error) => error.retry_disposition(),
      Self::Adapter(failure) => failure.retry_disposition(),
    }
  }

  pub fn into_dispatch_error(self) -> DispatchError {
    match self {
      Self::Router(error) => error.into(),
      Self::Adapter(failure) => match failure.failure_class() {
        RouterFailureClass::NoViableRoute => Error::<T>::NoRouteFound.into(),
        RouterFailureClass::ProtectionRejected => Error::<T>::PriceDeviationExceeded.into(),
        RouterFailureClass::LiquidityUnavailable => Error::<T>::InsufficientLiquidity.into(),
        RouterFailureClass::FeeRejected => Error::<T>::FeeRoutingFailed.into(),
        RouterFailureClass::PublicationRejected | RouterFailureClass::IngressRejected => {
          Error::<T>::InvalidOracleData.into()
        }
        RouterFailureClass::InvalidRequest | RouterFailureClass::InvariantViolation => {
          failure.into_dispatch_error()
        }
      },
    }
  }
}

impl<T: Config> From<Error<T>> for ExecutionError<T> {
  fn from(error: Error<T>) -> Self {
    Self::Router(error)
  }
}

impl<T: Config> From<AdapterFailure> for ExecutionError<T> {
  fn from(failure: AdapterFailure) -> Self {
    Self::Adapter(failure)
  }
}

impl<T: Config> From<DispatchError> for ExecutionError<T> {
  fn from(error: DispatchError) -> Self {
    Self::Adapter(AdapterFailure::unknown(error))
  }
}

impl<T: Config> From<ExecutionError<T>> for DispatchError {
  fn from(error: ExecutionError<T>) -> Self {
    error.into_dispatch_error()
  }
}

#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct RouterOutcome {
  pub family: RouteFamily,
  pub legs: PreparedLegs,
  pub total_amount_in: Balance,
  pub router_fee: Balance,
  pub routed_amount_in: Balance,
  pub recipient_amount_out: Balance,
  pub weight_class: RouteWeightClass,
}

/// Prepared route selected from current runtime state.
#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct PreparedRoute {
  pub total_amount_in: Balance,
  pub router_fee: Balance,
  pub routed_amount_in: Balance,
  pub recipient_amount_out: Balance,
  pub weight_class: RouteWeightClass,
  pub legs: PreparedLegs,
  pub family: RouteFamily,
}

impl RouteFamily {
  const fn rank(self) -> u8 {
    match self {
      Self::DirectXyk => 0,
      Self::DirectMint => 1,
      Self::NativeAnchoredXyk => 2,
    }
  }
}

/// Authoritative router quote surface for exact-input previews.
#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct RouterQuote {
  pub amount_in: Balance,
  pub router_fee: Balance,
  pub amount_after_fee: Balance,
  pub amount_out: Balance,
  pub family: RouteFamily,
  pub path: RoutePath,
  pub legs: PreparedLegs,
  pub price_impact: Perbill,
  pub total_fees: Balance,
}

/// Authoritative caller-aware exact-output quote.
#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct ExactOutputQuote {
  pub amount_in: Balance,
  pub router_fee: Balance,
  pub amount_after_fee: Balance,
  pub amount_out: Balance,
  pub family: RouteFamily,
  pub path: RoutePath,
  pub legs: PreparedLegs,
  pub price_impact: Perbill,
  pub total_fees: Balance,
}

impl PreparedRoute {
  fn path(&self) -> Option<RoutePath> {
    let mut path = RoutePath::default();
    let first = self.legs.first()?;
    path
      .try_push(match first {
        PreparedLeg::Xyk { asset_in, .. } => *asset_in,
        PreparedLeg::TmcMint {
          collateral_asset, ..
        } => *collateral_asset,
      })
      .ok()?;
    for leg in &self.legs {
      path
        .try_push(match leg {
          PreparedLeg::Xyk { asset_out, .. } => *asset_out,
          PreparedLeg::TmcMint { token_asset, .. } => *token_asset,
        })
        .ok()?;
    }
    Some(path)
  }

  fn with_ingress(mut self, total_amount_in: Balance, router_fee: Balance) -> Self {
    self.total_amount_in = total_amount_in;
    self.router_fee = router_fee;
    self
  }

  fn compare_exact_input(&self, other: &Self) -> core::cmp::Ordering {
    self
      .recipient_amount_out
      .cmp(&other.recipient_amount_out)
      .then_with(|| other.family.rank().cmp(&self.family.rank()))
      .then_with(|| other.path().cmp(&self.path()))
  }

  fn compare_exact_output(&self, other: &Self) -> core::cmp::Ordering {
    self
      .routed_amount_in
      .cmp(&other.routed_amount_in)
      .then_with(|| other.recipient_amount_out.cmp(&self.recipient_amount_out))
      .then_with(|| self.family.rank().cmp(&other.family.rank()))
      .then_with(|| self.path().cmp(&other.path()))
  }

  fn into_router_quote(
    self,
    amount_in: Balance,
    router_fee: Balance,
    price_impact: Perbill,
  ) -> RouterQuote {
    RouterQuote {
      amount_in,
      router_fee,
      amount_after_fee: self.routed_amount_in,
      amount_out: self.recipient_amount_out,
      family: self.family,
      path: self.path().unwrap_or_default(),
      legs: self.legs,
      price_impact,
      total_fees: router_fee,
    }
  }
}

#[frame::pallet]
pub mod pallet {
  use super::*;
  use crate::types::{AssetConversionApi, AssetKind, FeeRoutingAdapter, PriceOracle, TmcInterface};
  use polkadot_sdk::{
    frame_support::{
      PalletId,
      traits::{
        Currency, EnsureOrigin,
        fungible::Inspect as NativeInspect,
        fungibles::{Inspect as FungiblesInspect, Mutate},
        tokens::{Fortitude, Preservation},
      },
      transactional,
    },
    sp_runtime::traits::AccountIdConversion,
  };
  use scale_info::prelude::vec;

  #[pallet::config]
  pub trait Config: frame_system::Config {
    /// Native currency interface for native token transfers
    type Currency: Currency<Self::AccountId> + NativeInspect<Self::AccountId, Balance = Balance>;

    /// Asset management interface
    type Assets: FungiblesInspect<Self::AccountId, AssetId = u32, Balance = Balance>
      + Mutate<Self::AccountId>;

    /// TMC pallet interface
    type TmcPallet: crate::types::TmcInterface<Self::AccountId, Balance>;

    /// Asset conversion API for XYK pools
    type AssetConversion: crate::types::AssetConversionApi<Self::AccountId, Balance>;

    /// Origin that can perform governance operations
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    /// Pallet ID for account derivation
    #[pallet::constant]
    type PalletId: Get<PalletId>;

    /// Native asset (AssetKind)
    #[pallet::constant]
    type NativeAsset: Get<AssetKind>;

    /// Default router fee as Perbill (default: 0.5%)
    #[pallet::constant]
    type DefaultRouterFee: Get<Perbill>;

    /// Maximum number of canonical LP reverse-index entries.
    #[pallet::constant]
    type MaxLpPairs: Get<u32>;

    /// Maximum router fee allowed for governance updates.
    #[pallet::constant]
    type MaxRouterFee: Get<Perbill>;

    /// Precision constant for all calculations (10^12)
    #[pallet::constant]
    type Precision: Get<Balance>;

    /// EMA oracle half-life in blocks (100 blocks ~ 10 minutes at 6s/block)
    #[pallet::constant]
    type EmaHalfLife: Get<u32>;

    /// Maximum price deviation allowed (20%)
    #[pallet::constant]
    type MaxPriceDeviation: Get<Perbill>;

    /// Fee manager interface
    type FeeAdapter: FeeRoutingAdapter<Self::AccountId, Balance>;

    /// Burn Actor account for fee processing
    #[pallet::constant]
    type BurnActorAccount: Get<Self::AccountId>;

    /// Liquidity Actor account (fee-exempt System Actor)
    #[pallet::constant]
    type LiquidityActorAccount: Get<Self::AccountId>;

    /// Price-observation source for local deviation checks
    type PriceOracle: PriceOracle<Balance>;

    /// Minimum foreign amount for swapping (threshold for buffer processing)
    #[pallet::constant]
    type MinSwapForeign: Get<Balance>;

    /// Weight information
    type WeightInfo: WeightInfo;

    /// Helper for benchmarking
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::types::BenchmarkHelper<crate::types::AssetKind, Self::AccountId, u128>;
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(PhantomData<T>);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  /// Balance type
  pub type Balance = u128;

  /// Bounded reverse index from Asset Conversion LP tokens to canonical pool pairs.
  #[pallet::storage]
  pub type LpPairByTokenId<T: Config> =
    StorageValue<_, BoundedBTreeMap<u32, (AssetKind, AssetKind), T::MaxLpPairs>, ValueQuery>;

  /// Current router fee (can be updated by governance)
  #[pallet::storage]
  #[pallet::getter(fn router_fee)]
  pub type RouterFee<T: Config> = StorageValue<_, Perbill, ValueQuery, T::DefaultRouterFee>;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    /// Swap successfully executed
    SwapExecuted {
      who: T::AccountId,
      from: AssetKind,
      to: AssetKind,
      outcome: RouterOutcome,
    },
    /// Fee collected and routed
    FeeCollected {
      asset: AssetKind,
      amount: Balance,
      source: T::AccountId,
      collector: T::AccountId,
    },
    /// Router fee updated
    RouterFeeUpdated { old_fee: Perbill, new_fee: Perbill },
  }

  #[pallet::error]
  pub enum Error<T> {
    /// No viable route found between tokens
    NoRouteFound,
    /// Identical source and target assets
    IdenticalAssets,
    /// Amount is zero
    ZeroAmount,
    /// Amount below minimum swap threshold
    AmountTooLow,
    /// Insufficient liquidity in pools
    InsufficientLiquidity,
    /// Output amount below minimum acceptable
    SlippageExceeded,
    /// Transaction deadline passed
    DeadlinePassed,
    /// Fee processing failed
    FeeRoutingFailed,
    /// Account cannot pay the full swap input under the selected preservation policy
    InsufficientInputBalance,
    /// Price deviation exceeds maximum allowed
    PriceDeviationExceeded,
    /// Invalid price oracle data
    InvalidOracleData,
    /// Router fee exceeds the configured governance mutation bound
    RouterFeeTooHigh,
    /// An LP token is already indexed to a different pool pair
    LpTokenPairCollision,
    /// The bounded LP reverse index reached its configured capacity
    LpPairCapacityExceeded,
    /// A pool pair repeats an endpoint
    InvalidPoolPair,
    /// Prepared family and leg identity disagree
    PreparedRouteMismatch,
  }

  impl<T: Config> Error<T> {
    pub const fn failure_class(&self) -> RouterFailureClass {
      match self {
        Self::IdenticalAssets
        | Self::ZeroAmount
        | Self::AmountTooLow
        | Self::DeadlinePassed
        | Self::RouterFeeTooHigh => RouterFailureClass::InvalidRequest,
        Self::NoRouteFound => RouterFailureClass::NoViableRoute,
        Self::SlippageExceeded | Self::PriceDeviationExceeded => {
          RouterFailureClass::ProtectionRejected
        }
        Self::InsufficientLiquidity => RouterFailureClass::LiquidityUnavailable,
        Self::FeeRoutingFailed | Self::InsufficientInputBalance => RouterFailureClass::FeeRejected,
        Self::InvalidOracleData => RouterFailureClass::PublicationRejected,
        Self::LpTokenPairCollision
        | Self::LpPairCapacityExceeded
        | Self::InvalidPoolPair
        | Self::PreparedRouteMismatch => RouterFailureClass::InvariantViolation,
        Self::__Ignore(_, never) => match *never {},
      }
    }

    pub const fn retry_disposition(&self) -> RetryDisposition {
      match self {
        Self::NoRouteFound
        | Self::InsufficientLiquidity
        | Self::SlippageExceeded
        | Self::PriceDeviationExceeded
        | Self::InsufficientInputBalance
        | Self::InvalidOracleData => RetryDisposition::RetryLater,
        Self::IdenticalAssets
        | Self::ZeroAmount
        | Self::AmountTooLow
        | Self::DeadlinePassed
        | Self::FeeRoutingFailed
        | Self::RouterFeeTooHigh
        | Self::LpTokenPairCollision
        | Self::LpPairCapacityExceeded
        | Self::InvalidPoolPair
        | Self::PreparedRouteMismatch => RetryDisposition::Permanent,
        Self::__Ignore(_, never) => match *never {},
      }
    }
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    /// Execute a token swap through the router
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::swap())]
    pub fn swap(
      origin: OriginFor<T>,
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
      min_amount_out: Balance,
      recipient: T::AccountId,
      deadline: BlockNumberFor<T>,
    ) -> DispatchResult {
      let who = ensure_signed(origin)?;
      ensure!(
        amount_in >= T::MinSwapForeign::get(),
        Error::<T>::AmountTooLow
      );
      ensure!(
        frame_system::Pallet::<T>::block_number() <= deadline,
        Error::<T>::DeadlinePassed
      );
      Self::execute_swap_for(&who, from, to, amount_in, min_amount_out, &recipient)
        .map_err(DispatchError::from)?;
      Ok(())
    }

    /// Update router fee (governance only)
    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::update_router_fee())]
    pub fn update_router_fee(origin: OriginFor<T>, new_fee: Perbill) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::apply_router_fee_update(new_fee)
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn lp_pair_by_token_id(lp_token_id: u32) -> Option<(AssetKind, AssetKind)> {
      LpPairByTokenId::<T>::get().get(&lp_token_id).copied()
    }

    fn canonical_lp_pair(pair: (AssetKind, AssetKind)) -> Option<(AssetKind, AssetKind)> {
      if pair.0 == pair.1 {
        None
      } else if pair.0 < pair.1 {
        Some(pair)
      } else {
        Some((pair.1, pair.0))
      }
    }

    pub fn register_lp_pair(lp_token_id: u32, pair: (AssetKind, AssetKind)) -> DispatchResult {
      let canonical_pair = Self::canonical_lp_pair(pair).ok_or(Error::<T>::InvalidPoolPair)?;
      LpPairByTokenId::<T>::try_mutate(|pairs| {
        if let Some(existing) = pairs.get(&lp_token_id) {
          ensure!(
            *existing == canonical_pair,
            Error::<T>::LpTokenPairCollision
          );
          return Ok(());
        }
        ensure!(
          !pairs.values().any(|existing| *existing == canonical_pair),
          Error::<T>::LpTokenPairCollision
        );
        pairs
          .try_insert(lp_token_id, canonical_pair)
          .map_err(|_| Error::<T>::LpPairCapacityExceeded)?;
        Ok(())
      })
    }

    pub fn apply_router_fee_update(new_fee: Perbill) -> DispatchResult {
      ensure!(
        new_fee <= T::MaxRouterFee::get(),
        Error::<T>::RouterFeeTooHigh
      );
      let old_fee = RouterFee::<T>::get();
      RouterFee::<T>::put(new_fee);
      Self::deposit_event(Event::RouterFeeUpdated { old_fee, new_fee });
      Ok(())
    }

    /// Execute direct swap through asset conversion
    fn execute_direct_swap(
      who: &T::AccountId,
      path: &RoutePath,
      amount_in: Balance,
      min_amount_out: Balance,
      recipient: &T::AccountId,
      keep_alive: bool,
    ) -> Result<Balance, ExecutionError<T>> {
      if path.len() != 2 {
        return Err(Error::<T>::NoRouteFound.into());
      }
      T::AssetConversion::execute_single_pool_exact_input(
        who.clone(),
        path[0],
        path[1],
        amount_in,
        min_amount_out.max(1), // pallet_asset_conversion rejects zero
        recipient.clone(),
        keep_alive,
      )
      .map_err(Into::into)
    }

    /// Plan the optimal route and validate its protection bounds before execution
    fn prepare_optimal_route(
      from: AssetKind,
      to: AssetKind,
      total_amount_in: Balance,
      router_fee: Balance,
      min_amount_out: Balance,
    ) -> Result<PreparedRoute, ExecutionError<T>> {
      let routed_amount_in = total_amount_in.saturating_sub(router_fee);
      let prepared = Self::find_optimal_route(from, to, routed_amount_in)
        .ok_or(Error::<T>::NoRouteFound)?
        .with_ingress(total_amount_in, router_fee);
      ensure!(
        prepared.recipient_amount_out >= min_amount_out,
        Error::<T>::SlippageExceeded
      );
      Self::validate_prepared_identity(&prepared)?;
      Self::validate_prepared_legs(&prepared.legs)?;
      Ok(prepared)
    }

    /// Execute a route that was already selected and validated.
    fn execute_prepared_route(
      who: &T::AccountId,
      to: AssetKind,
      min_amount_out: Balance,
      recipient: &T::AccountId,
      keep_alive: bool,
      prepared: &PreparedRoute,
    ) -> Result<Balance, ExecutionError<T>> {
      if prepared.family == RouteFamily::DirectMint {
        let Some(PreparedLeg::TmcMint {
          token_asset,
          collateral_asset,
          ..
        }) = prepared.legs.first()
        else {
          return Err(Error::<T>::PreparedRouteMismatch.into());
        };
        ensure!(*token_asset == to, Error::<T>::PreparedRouteMismatch);
        let amount_out = T::TmcPallet::mint_with_distribution(
          who,
          recipient,
          *token_asset,
          *collateral_asset,
          prepared.routed_amount_in,
        )?;
        return Ok(amount_out);
      }

      let mut amount_in = prepared.routed_amount_in;
      let mut amount_out = 0;
      let last_leg = prepared.legs.len().saturating_sub(1);
      for (index, leg) in prepared.legs.iter().enumerate() {
        let PreparedLeg::Xyk {
          asset_in,
          asset_out,
          quoted_amount_out,
          ..
        } = leg
        else {
          return Err(Error::<T>::NoRouteFound.into());
        };
        Self::update_oracle_from_reserves(*asset_in, *asset_out)?;
        let is_last = index == last_leg;
        let leg_recipient = if is_last { recipient } else { who };
        let leg_floor = if is_last {
          min_amount_out
        } else {
          *quoted_amount_out
        };
        let path = vec![*asset_in, *asset_out]
          .try_into()
          .map_err(|_| Error::<T>::NoRouteFound)?;
        amount_out =
          Self::execute_direct_swap(who, &path, amount_in, leg_floor, leg_recipient, keep_alive)?;
        amount_in = amount_out;
      }
      Ok(amount_out)
    }

    fn execute_prepared_exact_output_legs(
      who: &T::AccountId,
      legs: &PreparedLegs,
      recipient: &T::AccountId,
      keep_alive: bool,
    ) -> Result<ExactOutputExecution, ExecutionError<T>> {
      let last_leg = legs.len().saturating_sub(1);
      let mut external_amount_in = None;
      let mut recipient_amount_out = 0;
      for (index, leg) in legs.iter().enumerate() {
        let PreparedLeg::Xyk {
          asset_in,
          asset_out,
          quoted_amount_in,
          quoted_amount_out,
          ..
        } = leg
        else {
          return Err(Error::<T>::NoRouteFound.into());
        };
        Self::update_oracle_from_reserves(*asset_in, *asset_out)?;
        let is_last = index == last_leg;
        let leg_recipient = if is_last { recipient } else { who };
        let execution = T::AssetConversion::execute_single_pool_exact_output(
          who.clone(),
          *asset_in,
          *asset_out,
          *quoted_amount_out,
          *quoted_amount_in,
          leg_recipient.clone(),
          keep_alive,
        )?;
        ensure!(
          execution.amount_in <= *quoted_amount_in,
          Error::<T>::SlippageExceeded
        );
        ensure!(
          execution.recipient_amount_out >= *quoted_amount_out,
          Error::<T>::SlippageExceeded
        );
        if external_amount_in.is_none() {
          external_amount_in = Some(execution.amount_in);
        }
        if is_last {
          recipient_amount_out = execution.recipient_amount_out;
        }
      }
      Ok(ExactOutputExecution {
        amount_in: external_amount_in.ok_or(Error::<T>::NoRouteFound)?,
        recipient_amount_out,
      })
    }

    pub(crate) fn validate_prepared_identity(prepared: &PreparedRoute) -> Result<(), Error<T>> {
      let path = prepared.path().ok_or(Error::<T>::PreparedRouteMismatch)?;
      let valid = match (prepared.family, prepared.legs.as_slice(), path.as_slice()) {
        (
          RouteFamily::DirectXyk,
          [
            PreparedLeg::Xyk {
              pool_id,
              asset_in,
              asset_out,
              ..
            },
          ],
          [path_in, path_out],
        ) => {
          asset_in == path_in
            && asset_out == path_out
            && T::AssetConversion::single_pool_id(*asset_in, *asset_out) == Some(*pool_id)
        }
        (
          RouteFamily::DirectMint,
          [
            PreparedLeg::TmcMint {
              token_asset,
              collateral_asset,
              ..
            },
          ],
          [path_in, path_out],
        ) => collateral_asset == path_in && token_asset == path_out,
        (
          RouteFamily::NativeAnchoredXyk,
          [
            PreparedLeg::Xyk {
              pool_id: first_pool,
              asset_in: first_in,
              asset_out: first_out,
              ..
            },
            PreparedLeg::Xyk {
              pool_id: second_pool,
              asset_in: second_in,
              asset_out: second_out,
              ..
            },
          ],
          [path_in, path_middle, path_out],
        ) => {
          first_in == path_in
            && first_out == path_middle
            && second_in == path_middle
            && second_out == path_out
            && *path_middle == T::NativeAsset::get()
            && T::AssetConversion::single_pool_id(*first_in, *first_out) == Some(*first_pool)
            && T::AssetConversion::single_pool_id(*second_in, *second_out) == Some(*second_pool)
        }
        _ => false,
      };
      ensure!(valid, Error::<T>::PreparedRouteMismatch);
      Ok(())
    }

    fn validate_prepared_legs(legs: &PreparedLegs) -> Result<(), ExecutionError<T>> {
      for leg in legs {
        if let PreparedLeg::Xyk {
          asset_in,
          asset_out,
          quoted_amount_in,
          quoted_amount_out,
          ..
        } = leg
        {
          ensure!(!quoted_amount_in.is_zero(), Error::<T>::InvalidOracleData);
          let current_price = quoted_amount_out
            .saturating_mul(T::Precision::get())
            .saturating_div(*quoted_amount_in);
          T::PriceOracle::validate_price_deviation(*asset_in, *asset_out, current_price)?;
        }
      }
      Ok(())
    }

    /// Update the local EMA from pre-execution pool reserves
    fn update_oracle_from_reserves(
      from: AssetKind,
      to: AssetKind,
    ) -> Result<(), ExecutionError<T>> {
      if let Some(pool_id) = T::AssetConversion::single_pool_id(from, to) {
        if let Some((res_a, res_b)) = T::AssetConversion::single_pool_reserves(pool_id) {
          // CORRECT: Identify which reserve matches the 'from' asset
          let (reserve_in, reserve_out) = if pool_id.0 == from {
            (res_a, res_b)
          } else {
            (res_b, res_a) // Flip reserves if pool is sorted differently
          };
          if !reserve_in.is_zero() {
            let spot_price = reserve_out
              .saturating_mul(T::Precision::get())
              .saturating_div(reserve_in);
            T::PriceOracle::update_ema_price(from, to, spot_price)?;
          }
        }
      }
      Ok(())
    }

    fn ensure_can_debit_input(
      who: &T::AccountId,
      asset: AssetKind,
      amount: Balance,
      keep_alive: bool,
    ) -> Result<(), Error<T>> {
      let preservation = if keep_alive {
        Preservation::Protect
      } else {
        Preservation::Expendable
      };
      let reducible = match asset {
        AssetKind::Native => T::Currency::reducible_balance(who, preservation, Fortitude::Polite),
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          T::Assets::reducible_balance(id, who, preservation, Fortitude::Polite)
        }
      };
      ensure!(reducible >= amount, Error::<T>::InsufficientInputBalance);
      Ok(())
    }

    /// Collect router fee with advanced accumulated balance processing
    fn collect_router_fee(
      fee_asset: AssetKind,
      fee_amount: Balance,
      who: &T::AccountId,
    ) -> Result<(), ExecutionError<T>> {
      if fee_amount == 0 {
        return Ok(());
      }
      if Self::is_fee_exempt(who) {
        return Ok(());
      }
      T::FeeAdapter::route_fee(who, fee_asset, fee_amount)?;
      Self::deposit_event(Event::<T>::FeeCollected {
        asset: fee_asset,
        amount: fee_amount,
        source: who.clone(),
        collector: T::BurnActorAccount::get(),
      });
      Ok(())
    }

    /// Get pallet account ID
    pub fn account_id() -> T::AccountId {
      T::PalletId::get().into_account_truncating()
    }

    /// Public entry point for system-level swaps (Burn Actor, Liquidity Actor, and other pallets).
    /// Handles fee exemption for system accounts, gross-input affordability, and max-output routing.
    #[transactional]
    pub fn execute_swap_for(
      who: &T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
      min_amount_out: Balance,
      recipient: &T::AccountId,
    ) -> Result<RouterOutcome, ExecutionError<T>> {
      ensure!(from != to, Error::<T>::IdenticalAssets);
      ensure!(amount_in > 0, Error::<T>::ZeroAmount);
      let system_account = Self::is_fee_exempt(who);
      let fee = if system_account {
        0
      } else {
        Self::calculate_router_fee(amount_in)
      };
      let keep_alive = !system_account;
      Self::ensure_can_debit_input(who, from, amount_in, keep_alive)?;
      let route_comparison = Self::prepare_optimal_route(from, to, amount_in, fee, min_amount_out)?;
      Self::collect_router_fee(from, fee, who)?;
      let amount_out = Self::execute_prepared_route(
        who,
        to,
        min_amount_out,
        recipient,
        keep_alive,
        &route_comparison,
      )?;
      ensure!(amount_out >= min_amount_out, Error::<T>::SlippageExceeded);
      let family = route_comparison.family;
      let outcome = RouterOutcome {
        family,
        legs: route_comparison.legs,
        total_amount_in: route_comparison.total_amount_in,
        router_fee: route_comparison.router_fee,
        routed_amount_in: route_comparison.routed_amount_in,
        recipient_amount_out: amount_out,
        weight_class: route_comparison.weight_class,
      };
      Self::deposit_event(Event::SwapExecuted {
        who: who.clone(),
        from,
        to,
        outcome: outcome.clone(),
      });
      Ok(outcome)
    }

    /// Execute a caller-aware native exact-output XYK route under a total input cap.
    #[transactional]
    pub fn execute_exact_out_for(
      who: &T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_out: Balance,
      max_amount_in: Balance,
      recipient: &T::AccountId,
    ) -> Result<RouterOutcome, ExecutionError<T>> {
      ensure!(!max_amount_in.is_zero(), Error::<T>::ZeroAmount);
      let (prepared, router_fee, prepared_amount_in) =
        Self::prepare_exact_output_route(who, from, to, amount_out)?;
      ensure!(
        prepared_amount_in <= max_amount_in,
        Error::<T>::SlippageExceeded
      );
      let keep_alive = !Self::is_fee_exempt(who);
      Self::ensure_can_debit_input(who, from, prepared_amount_in, keep_alive)?;
      Self::validate_prepared_legs(&prepared.legs)?;
      Self::collect_router_fee(from, router_fee, who)?;
      let execution =
        Self::execute_prepared_exact_output_legs(who, &prepared.legs, recipient, keep_alive)?;
      ensure!(
        execution.amount_in <= prepared.routed_amount_in,
        Error::<T>::SlippageExceeded
      );
      ensure!(
        execution.recipient_amount_out >= amount_out,
        Error::<T>::SlippageExceeded
      );
      let amount_in = execution.amount_in.saturating_add(router_fee);
      ensure!(amount_in <= max_amount_in, Error::<T>::SlippageExceeded);
      let family = prepared.family;
      let outcome = RouterOutcome {
        family,
        legs: prepared.legs,
        total_amount_in: amount_in,
        router_fee: prepared.router_fee,
        routed_amount_in: execution.amount_in,
        recipient_amount_out: execution.recipient_amount_out,
        weight_class: prepared.weight_class,
      };
      Self::deposit_event(Event::SwapExecuted {
        who: who.clone(),
        from,
        to,
        outcome: outcome.clone(),
      });
      Ok(outcome)
    }

    /// Check whether an account is exempt from router fees (system actors)
    pub fn is_fee_exempt(who: &T::AccountId) -> bool {
      who == &Self::account_id()
        || who == &T::BurnActorAccount::get()
        || who == &T::LiquidityActorAccount::get()
    }

    /// Get quote for swapping from asset_from to asset_to with amount_in
    /// Raw XYK quote for `amount_in` of `asset_from` -> `asset_to`, without the
    /// router fee. For a caller-aware preview that mirrors actual swap execution
    /// (including the router fee and optimal mechanism), use `quote_exact_input`.
    pub fn quote_price(
      asset_from: AssetKind,
      asset_to: AssetKind,
      amount_in: Balance,
    ) -> Result<Balance, DispatchError> {
      if asset_from == asset_to {
        return Err(Error::<T>::IdenticalAssets.into());
      }
      if amount_in.is_zero() {
        return Err(Error::<T>::ZeroAmount.into());
      }
      // Get quote from asset conversion pallet
      T::AssetConversion::quote_single_pool_exact_input(asset_from, asset_to, amount_in, true)
        .ok_or_else(|| Error::<T>::NoRouteFound.into())
    }

    /// Get oracle price for asset pair
    pub fn get_oracle_price(asset_from: AssetKind, asset_to: AssetKind) -> Option<Balance> {
      T::PriceOracle::get_ema_price(asset_from, asset_to)
    }

    /// Advanced route selection with TMC integration
    fn find_optimal_route(
      from: AssetKind,
      to: AssetKind,
      amount_after_fee: Balance,
    ) -> Option<PreparedRoute> {
      let native_asset = T::NativeAsset::get();
      let mut candidate_routes = Vec::new();
      // 1. Direct XYK route
      if let Some(direct_output) =
        T::AssetConversion::quote_single_pool_exact_input(from, to, amount_after_fee, true)
      {
        let final_output = direct_output;
        let pool_id = T::AssetConversion::single_pool_id(from, to)?;
        candidate_routes.push(PreparedRoute {
          total_amount_in: amount_after_fee,
          router_fee: 0,
          routed_amount_in: amount_after_fee,
          recipient_amount_out: final_output,
          weight_class: RouteWeightClass::ExactInputDirectXyk,
          legs: vec![PreparedLeg::Xyk {
            pool_id,
            asset_in: from,
            asset_out: to,
            quoted_amount_in: amount_after_fee,
            quoted_amount_out: direct_output,
          }]
          .try_into()
          .ok()?,
          family: RouteFamily::DirectXyk,
        });
      }
      // 2. Direct mint route (if applicable)
      // TMC mints the `to` token using `from` as collateral.
      // Supported: any pair where a curve exists for `to` and `from` is its collateral.
      if T::TmcPallet::has_curve(to) && T::TmcPallet::supports_collateral(to, from) {
        if let Ok(tmc_output) = T::TmcPallet::calculate_recipient_receives(to, amount_after_fee) {
          let final_output = tmc_output;
          candidate_routes.push(PreparedRoute {
            total_amount_in: amount_after_fee,
            router_fee: 0,
            routed_amount_in: amount_after_fee,
            recipient_amount_out: final_output,
            weight_class: RouteWeightClass::ExactInputDirectMint,
            legs: vec![PreparedLeg::TmcMint {
              token_asset: to,
              collateral_asset: from,
              quoted_collateral_in: amount_after_fee,
              quoted_recipient_out: tmc_output,
            }]
            .try_into()
            .ok()?,
            family: RouteFamily::DirectMint,
          });
        }
      }
      // 3. Multi-hop Native route
      if from != native_asset && to != native_asset {
        let multi_hop_path: RoutePath = vec![from, native_asset, to].try_into().ok()?;
        if let Some(legs) = Self::prepare_exact_input_xyk_legs(&multi_hop_path, amount_after_fee) {
          let final_output = match legs.last()? {
            PreparedLeg::Xyk {
              quoted_amount_out, ..
            } => *quoted_amount_out,
            PreparedLeg::TmcMint { .. } => return None,
          };
          candidate_routes.push(PreparedRoute {
            total_amount_in: amount_after_fee,
            router_fee: 0,
            routed_amount_in: amount_after_fee,
            recipient_amount_out: final_output,
            weight_class: RouteWeightClass::ExactInputNativeAnchoredXyk,
            legs,
            family: RouteFamily::NativeAnchoredXyk,
          });
        }
      }
      // Mechanism selection: the router is a pure execution mechanism and always
      // picks the route that delivers the most output to the swap recipient.
      // Price impact and known market fees remain informational. Economic ties
      // resolve by the specification's stable family rank and bounded path identity.
      candidate_routes
        .into_iter()
        .max_by(PreparedRoute::compare_exact_input)
    }

    /// Select the exact-output XYK route requiring the least post-fee input.
    /// Direct TMC minting remains exact-input only because its adapter cannot
    /// promise an exact recipient amount without an inverse execution contract.
    fn find_optimal_exact_output_route(
      from: AssetKind,
      to: AssetKind,
      amount_out: Balance,
    ) -> Option<PreparedRoute> {
      let native_asset = T::NativeAsset::get();
      let mut candidates = Vec::new();
      if let Some(required_input) =
        T::AssetConversion::quote_single_pool_exact_output(from, to, amount_out, true)
      {
        let pool_id = T::AssetConversion::single_pool_id(from, to)?;
        candidates.push(PreparedRoute {
          total_amount_in: required_input,
          router_fee: 0,
          routed_amount_in: required_input,
          recipient_amount_out: amount_out,
          weight_class: RouteWeightClass::ExactOutputDirectXyk,
          legs: vec![PreparedLeg::Xyk {
            pool_id,
            asset_in: from,
            asset_out: to,
            quoted_amount_in: required_input,
            quoted_amount_out: amount_out,
          }]
          .try_into()
          .ok()?,
          family: RouteFamily::DirectXyk,
        });
      }
      if from != native_asset && to != native_asset {
        let native_required =
          T::AssetConversion::quote_single_pool_exact_output(native_asset, to, amount_out, true);
        if let Some(native_required) = native_required {
          if let Some(required_input) = T::AssetConversion::quote_single_pool_exact_output(
            from,
            native_asset,
            native_required,
            true,
          ) {
            let first_pool = T::AssetConversion::single_pool_id(from, native_asset)?;
            let second_pool = T::AssetConversion::single_pool_id(native_asset, to)?;
            candidates.push(PreparedRoute {
              total_amount_in: required_input,
              router_fee: 0,
              routed_amount_in: required_input,
              recipient_amount_out: amount_out,
              weight_class: RouteWeightClass::ExactOutputNativeAnchoredXyk,
              legs: vec![
                PreparedLeg::Xyk {
                  pool_id: first_pool,
                  asset_in: from,
                  asset_out: native_asset,
                  quoted_amount_in: required_input,
                  quoted_amount_out: native_required,
                },
                PreparedLeg::Xyk {
                  pool_id: second_pool,
                  asset_in: native_asset,
                  asset_out: to,
                  quoted_amount_in: native_required,
                  quoted_amount_out: amount_out,
                },
              ]
              .try_into()
              .ok()?,
              family: RouteFamily::NativeAnchoredXyk,
            });
          }
        }
      }
      candidates
        .into_iter()
        .min_by(PreparedRoute::compare_exact_output)
    }

    fn exact_output_router_fee(who: &T::AccountId, required_input: Balance) -> Balance {
      if Self::is_fee_exempt(who) {
        return 0;
      }
      let retained = Perbill::one() - RouterFee::<T>::get();
      let gross_input = retained.saturating_reciprocal_mul_ceil(required_input);
      Self::calculate_router_fee(gross_input)
    }

    fn prepare_exact_output_route(
      who: &T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_out: Balance,
    ) -> Result<(PreparedRoute, Balance, Balance), Error<T>> {
      ensure!(from != to, Error::<T>::IdenticalAssets);
      ensure!(!amount_out.is_zero(), Error::<T>::ZeroAmount);
      let route = Self::find_optimal_exact_output_route(from, to, amount_out)
        .ok_or(Error::<T>::NoRouteFound)?;
      Self::validate_prepared_identity(&route)?;
      let router_fee = Self::exact_output_router_fee(who, route.routed_amount_in);
      let total_amount_in = route.routed_amount_in.saturating_add(router_fee);
      Ok((
        route.with_ingress(total_amount_in, router_fee),
        router_fee,
        total_amount_in,
      ))
    }

    fn projected_price_impact(route: &PreparedRoute) -> Perbill {
      let Some(path) = route.path() else {
        return Perbill::zero();
      };
      match route.family {
        RouteFamily::DirectMint => Perbill::zero(),
        RouteFamily::DirectXyk => Self::calculate_price_impact(
          path[0],
          path[path.len() - 1],
          route.routed_amount_in,
          route.recipient_amount_out,
        ),
        RouteFamily::NativeAnchoredXyk => Self::calculate_multi_hop_price_impact(
          &path,
          route.routed_amount_in,
          route.recipient_amount_out,
        ),
      }
    }

    fn prepare_exact_output_quote(
      who: &T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_out: Balance,
    ) -> Result<ExactOutputQuote, Error<T>> {
      let (route, router_fee, amount_in) =
        Self::prepare_exact_output_route(who, from, to, amount_out)?;
      let price_impact = Self::projected_price_impact(&route);
      let path = route.path().ok_or(Error::<T>::PreparedRouteMismatch)?;
      Ok(ExactOutputQuote {
        amount_in,
        router_fee,
        amount_after_fee: route.routed_amount_in,
        amount_out: route.recipient_amount_out,
        family: route.family,
        path,
        legs: route.legs,
        price_impact,
        total_fees: router_fee,
      })
    }

    fn prepare_exact_input_xyk_legs(path: &RoutePath, amount_in: Balance) -> Option<PreparedLegs> {
      let mut current_amount = amount_in;
      let mut legs = PreparedLegs::default();
      for window in path.windows(2) {
        let asset_in = window[0];
        let asset_out = window[1];
        let pool_id = T::AssetConversion::single_pool_id(asset_in, asset_out)?;
        let quoted_amount_out = T::AssetConversion::quote_single_pool_exact_input(
          asset_in,
          asset_out,
          current_amount,
          true,
        )?;
        legs
          .try_push(PreparedLeg::Xyk {
            pool_id,
            asset_in,
            asset_out,
            quoted_amount_in: current_amount,
            quoted_amount_out,
          })
          .ok()?;
        current_amount = quoted_amount_out;
      }
      Some(legs)
    }

    /// Calculate price impact for direct route
    fn calculate_price_impact(
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
      amount_out: Balance,
    ) -> Perbill {
      // Simplified price impact calculation
      // In production, this would use pool reserves and more sophisticated math
      if let Some(ema_price) = T::PriceOracle::get_ema_price(from, to) {
        if ema_price > 0 {
          let expected_out = amount_in.saturating_mul(ema_price) / T::Precision::get();
          if expected_out > amount_out {
            return Perbill::from_rational(expected_out - amount_out, expected_out);
          }
        }
      }
      Perbill::zero()
    }

    /// Calculate informational endpoint impact without performing another market quote.
    fn calculate_multi_hop_price_impact(
      path: &[AssetKind],
      amount_in: Balance,
      amount_out: Balance,
    ) -> Perbill {
      Self::calculate_price_impact(path[0], path[path.len() - 1], amount_in, amount_out)
    }

    /// Calculate router fee for a given amount
    pub fn calculate_router_fee(amount: Balance) -> Balance {
      RouterFee::<T>::get().mul_floor(amount)
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::BTreeSet;
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let fee = RouterFee::<T>::get();
      if fee > T::MaxRouterFee::get() {
        return Err(TryRuntimeError::Other(
          "RouterFee exceeds configured maximum",
        ));
      }
      let mut indexed_pairs = BTreeSet::new();
      for (_, pair) in LpPairByTokenId::<T>::get() {
        if Self::canonical_lp_pair(pair) != Some(pair) {
          return Err(TryRuntimeError::Other(
            "LP reverse index contains a non-canonical pair",
          ));
        }
        if !indexed_pairs.insert(pair) {
          return Err(TryRuntimeError::Other(
            "LP reverse index maps one pair to multiple LP tokens",
          ));
        }
      }
      Ok(())
    }
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    /// Returns the authoritative exact-input router quote for a specific caller
    pub fn quote_exact_input(
      who: T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
    ) -> Result<RouterQuote, Error<T>> {
      if from == to {
        return Err(Error::<T>::IdenticalAssets);
      }
      if amount_in.is_zero() {
        return Err(Error::<T>::ZeroAmount);
      }
      let router_fee = if Self::is_fee_exempt(&who) {
        0
      } else {
        Self::calculate_router_fee(amount_in)
      };
      let amount_after_fee = amount_in.saturating_sub(router_fee);
      let route =
        Self::find_optimal_route(from, to, amount_after_fee).ok_or(Error::<T>::NoRouteFound)?;
      Self::validate_prepared_identity(&route)?;
      let price_impact = Self::projected_price_impact(&route);
      Ok(route.with_ingress(amount_in, router_fee).into_router_quote(
        amount_in,
        router_fee,
        price_impact,
      ))
    }

    /// Returns the least-input native XYK route for an exact recipient output.
    pub fn quote_exact_out(
      who: T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_out: Balance,
    ) -> Result<ExactOutputQuote, Error<T>> {
      Self::prepare_exact_output_quote(&who, from, to, amount_out)
    }
  }

  /// Genesis configuration
  #[pallet::genesis_config]
  pub struct GenesisConfig<T: Config> {
    pub _marker: core::marker::PhantomData<T>,
  }

  impl<T: Config> Default for GenesisConfig<T> {
    fn default() -> Self {
      Self {
        _marker: Default::default(),
      }
    }
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      // Ensure pallet account survives zero native balance (ED-free)
      frame_system::Pallet::<T>::inc_providers(&Pallet::<T>::account_id());
    }
  }
}
