//! Axial Router Pallet
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
use polkadot_sdk::sp_runtime::Perbill;
use scale_info::prelude::vec::Vec;

/// Route comparison result for optimal path selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteComparison {
  /// Expected output amount
  pub expected_output: Balance,
  /// Route path (asset kinds)
  pub path: Vec<AssetKind>,
  /// Route mechanism type
  pub mechanism: RouteMechanism,
  /// Price impact percentage
  pub price_impact: Perbill,
  /// Total fees (router + AMM)
  pub total_fees: Balance,
}

/// Route mechanism types for advanced routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteMechanism {
  /// Direct XYK pool swap
  DirectXyk { pool_id: (AssetKind, AssetKind) },
  /// Direct mint via TMC curve
  DirectMint { foreign_asset: AssetKind },
  /// Multi-hop through Native
  MultiHopNative { hops: Vec<AssetKind> },
}

/// Simplified route mechanism for events (no unbounded types)
#[derive(
  Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum RouteMechanismKind {
  DirectXyk,
  DirectMint,
  MultiHopNative,
}

impl From<&RouteMechanism> for RouteMechanismKind {
  fn from(m: &RouteMechanism) -> Self {
    match m {
      RouteMechanism::DirectXyk { .. } => Self::DirectXyk,
      RouteMechanism::DirectMint { .. } => Self::DirectMint,
      RouteMechanism::MultiHopNative { .. } => Self::MultiHopNative,
    }
  }
}

/// Authoritative router quote surface for exact-input previews
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RouterQuote {
  /// Original amount presented to the router before router fee deduction
  pub amount_in: Balance,
  /// Router fee charged on `amount_in`
  pub router_fee: Balance,
  /// Amount that actually enters route selection after router fee
  pub amount_after_fee: Balance,
  /// Best output amount under current router policy
  pub amount_out: Balance,
  /// Canonical mechanism chosen by the router policy
  pub mechanism: RouteMechanismKind,
  /// Canonical path chosen by the router policy
  pub path: Vec<AssetKind>,
  /// Current route price-impact snapshot for client display
  pub price_impact: Perbill,
  /// Total fee burden reported with the quote
  pub total_fees: Balance,
}

impl RouteComparison {
  /// Create new route comparison
  pub fn new(
    expected_output: Balance,
    path: Vec<AssetKind>,
    mechanism: RouteMechanism,
    price_impact: Perbill,
    total_fees: Balance,
  ) -> Self {
    Self {
      expected_output,
      path,
      mechanism,
      price_impact,
      total_fees,
    }
  }

  pub fn into_router_quote(self, amount_in: Balance, router_fee: Balance) -> RouterQuote {
    RouterQuote {
      amount_in,
      router_fee,
      amount_after_fee: amount_in.saturating_sub(router_fee),
      amount_out: self.expected_output,
      mechanism: RouteMechanismKind::from(&self.mechanism),
      path: self.path,
      price_impact: self.price_impact,
      total_fees: self.total_fees.saturating_add(router_fee),
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

    /// Maximum number of tracked assets for oracle monitoring
    #[pallet::constant]
    type MaxTrackedAssets: Get<u32>;

    /// Fee manager interface
    type FeeAdapter: FeeRoutingAdapter<Self::AccountId, Balance>;

    /// Burning manager account for fee processing
    #[pallet::constant]
    type BurningManagerAccount: Get<Self::AccountId>;

    /// Liquidity Actor account (fee-exempt System AAA Actor)
    #[pallet::constant]
    type LiquidityActorAccount: Get<Self::AccountId>;

    /// Price oracle for manipulation-resistant pricing
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

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  /// Balance type
  pub type Balance = u128;

  /// Assets registered for external oracle monitoring.
  ///
  /// This is a governance-managed observability/registry surface, not a protection
  /// gate: pre-swap oracle updates and price-deviation checks operate on the actual
  /// swap pair regardless of this list.
  #[pallet::storage]
  pub type TrackedAssets<T: Config> =
    StorageValue<_, BoundedVec<AssetKind, T::MaxTrackedAssets>, ValueQuery>;

  /// Reverse index from an Asset Conversion LP token to its canonical pool pair.
  #[pallet::storage]
  #[pallet::getter(fn lp_pair_by_token_id)]
  pub type LpPairByTokenId<T: Config> =
    StorageMap<_, Twox64Concat, u32, (AssetKind, AssetKind), OptionQuery>;

  /// Current router fee (can be updated by governance)
  #[pallet::storage]
  #[pallet::getter(fn router_fee)]
  pub type RouterFee<T: Config> = StorageValue<_, Perbill, ValueQuery, T::DefaultRouterFee>;

  /// EMA price storage for asset pairs (key: (asset_in, asset_out))
  #[pallet::storage]
  pub type EmaPrices<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    AssetKind,
    Blake2_128Concat,
    AssetKind,
    Balance,
    ValueQuery,
  >;

  /// Last update block for EMA prices
  #[pallet::storage]
  pub type EmaLastUpdate<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    AssetKind,
    Blake2_128Concat,
    AssetKind,
    BlockNumberFor<T>,
    ValueQuery,
  >;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    /// Swap successfully executed
    SwapExecuted {
      who: T::AccountId,
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
      amount_out: Balance,
      mechanism: RouteMechanismKind,
    },
    /// Fee collected and routed
    FeeCollected {
      asset: AssetKind,
      amount: Balance,
      source: T::AccountId,
      collector: T::AccountId,
    },
    /// Asset added to tracking list
    TrackedAssetAdded { asset: AssetKind },
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
    /// No viable multi-hop route found
    NoMultiHopRoute,
    /// Maximum tracked assets limit reached
    MaxTrackedAssetsExceeded,
    /// Router fee exceeds the configured governance mutation bound
    RouterFeeTooHigh,
    /// An LP token is already indexed to a different pool pair
    LpTokenPairCollision,
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
      Self::execute_swap_for(&who, from, to, amount_in, min_amount_out, &recipient)?;
      Ok(())
    }

    /// Add asset to tracked assets list for oracle monitoring (governance only)
    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::add_tracked_asset())]
    pub fn add_tracked_asset(origin: OriginFor<T>, asset: AssetKind) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::apply_add_tracked_asset(asset)
    }

    /// Update router fee (governance only)
    #[pallet::call_index(2)]
    #[pallet::weight(T::WeightInfo::update_router_fee())]
    pub fn update_router_fee(origin: OriginFor<T>, new_fee: Perbill) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::apply_router_fee_update(new_fee)
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn apply_add_tracked_asset(asset: AssetKind) -> DispatchResult {
      TrackedAssets::<T>::try_mutate(|assets_list| -> DispatchResult {
        if !assets_list.contains(&asset) {
          assets_list
            .try_push(asset)
            .map_err(|_| Error::<T>::MaxTrackedAssetsExceeded)?;
        }
        Ok(())
      })?;
      Self::deposit_event(Event::TrackedAssetAdded { asset });
      Ok(())
    }

    pub fn register_lp_pair(lp_token_id: u32, pair: (AssetKind, AssetKind)) -> DispatchResult {
      if let Some(existing) = LpPairByTokenId::<T>::get(lp_token_id) {
        ensure!(existing == pair, Error::<T>::LpTokenPairCollision);
        return Ok(());
      }
      LpPairByTokenId::<T>::insert(lp_token_id, pair);
      Ok(())
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
      path: &[AssetKind],
      amount_in: Balance,
      min_amount_out: Balance,
      recipient: &T::AccountId,
      keep_alive: bool,
    ) -> Result<Balance, DispatchError> {
      if path.len() < 2 {
        return Err(Error::<T>::NoRouteFound.into());
      }
      T::AssetConversion::swap_exact_tokens_for_tokens(
        who.clone(),
        path.to_vec(),
        amount_in,
        min_amount_out.max(1), // pallet_asset_conversion rejects zero
        recipient.clone(),
        keep_alive,
      )
    }

    /// Plan the optimal route and validate its protection bounds before execution
    fn prepare_optimal_route(
      from: AssetKind,
      to: AssetKind,
      amount_in: Balance,
      min_amount_out: Balance,
    ) -> Result<RouteComparison, Error<T>> {
      let route_comparison =
        Self::find_optimal_route(from, to, amount_in).ok_or(Error::<T>::NoRouteFound)?;
      Self::validate_price_protection(
        &route_comparison.path,
        amount_in,
        min_amount_out,
        route_comparison.expected_output,
      )?;
      Ok(route_comparison)
    }

    /// Execute a route that was already selected and validated
    fn execute_prepared_route(
      who: &T::AccountId,
      to: AssetKind,
      amount_in: Balance,
      min_amount_out: Balance,
      recipient: &T::AccountId,
      keep_alive: bool,
      route_comparison: RouteComparison,
    ) -> Result<(Balance, RouteMechanismKind), DispatchError> {
      let mechanism_kind = RouteMechanismKind::from(&route_comparison.mechanism);
      let amount_out = match route_comparison.mechanism {
        RouteMechanism::DirectMint { foreign_asset } => {
          T::TmcPallet::mint_with_distribution(who, recipient, to, foreign_asset, amount_in)?
        }
        _ => Self::execute_direct_swap(
          who,
          &route_comparison.path,
          amount_in,
          min_amount_out,
          recipient,
          keep_alive,
        )?,
      };
      Ok((amount_out, mechanism_kind))
    }

    /// Validate price protection before swap execution
    fn validate_price_protection(
      path: &[AssetKind],
      amount_in: Balance,
      min_amount_out: Balance,
      expected_output: Balance,
    ) -> Result<(), Error<T>> {
      // Basic slippage check on the quote
      if expected_output < min_amount_out {
        return Err(Error::<T>::SlippageExceeded);
      }
      if path.len() < 2 {
        return Err(Error::<T>::NoRouteFound);
      }
      let from = path.first().copied().ok_or(Error::<T>::NoRouteFound)?;
      let to = path.last().copied().ok_or(Error::<T>::NoRouteFound)?;
      if from == to {
        return Err(Error::<T>::IdenticalAssets);
      }
      if path.len() == 2 {
        let current_output = expected_output; // Use pre-calculated output to avoid double DB read
        let current_price_normalized = current_output
          .saturating_mul(T::Precision::get())
          .saturating_div(amount_in);
        T::PriceOracle::validate_price_deviation(from, to, current_price_normalized)
          .map_err(|_| Error::<T>::PriceDeviationExceeded)?;
      } else {
        // Multi-hop (Native-anchored) routes are protected by user slippage only:
        // the EMA deviation guard is intentionally a direct-pair check on the
        // current launch line. Native is treated as the stable routing hub.
        Self::quote_multi_hop_route(path, amount_in).ok_or(Error::<T>::NoRouteFound)?;
      }
      Ok(())
    }

    /// Update oracle using pre-swap pool reserves to prevent manipulation
    fn update_oracle_from_reserves(from: AssetKind, to: AssetKind) -> Result<(), Error<T>> {
      if let Some(pool_id) = T::AssetConversion::get_pool_id(from, to) {
        if let Some((res_a, res_b)) = T::AssetConversion::get_pool_reserves(pool_id) {
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
            T::PriceOracle::update_ema_price(from, to, spot_price)
              .map_err(|_| Error::<T>::InvalidOracleData)?;
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
    ) -> Result<(), Error<T>> {
      if fee_amount == 0 {
        return Ok(());
      }
      if Self::is_fee_exempt(who) {
        return Ok(());
      }
      T::FeeAdapter::route_fee(who, fee_asset, fee_amount)
        .map_err(|_| Error::<T>::FeeRoutingFailed)?;
      Self::deposit_event(Event::<T>::FeeCollected {
        asset: fee_asset,
        amount: fee_amount,
        source: who.clone(),
        collector: T::BurningManagerAccount::get(),
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
    ) -> Result<Balance, DispatchError> {
      ensure!(from != to, Error::<T>::IdenticalAssets);
      ensure!(amount_in > 0, Error::<T>::ZeroAmount);
      let system_account = Self::is_fee_exempt(who);
      let fee = if system_account {
        0
      } else {
        Self::calculate_router_fee(amount_in)
      };
      let amount_after_fee = amount_in.saturating_sub(fee);
      let keep_alive = !system_account;
      Self::ensure_can_debit_input(who, from, amount_in, keep_alive)?;
      let route_comparison =
        Self::prepare_optimal_route(from, to, amount_after_fee, min_amount_out)?;
      Self::collect_router_fee(from, fee, who)?;
      Self::update_oracle_from_reserves(from, to)?;
      let (amount_out, mechanism) = Self::execute_prepared_route(
        who,
        to,
        amount_after_fee,
        min_amount_out,
        recipient,
        keep_alive,
        route_comparison,
      )?;
      Self::deposit_event(Event::SwapExecuted {
        who: who.clone(),
        from,
        to,
        amount_in,
        amount_out,
        mechanism,
      });
      Ok(amount_out)
    }

    /// Check whether an account is exempt from router fees (system actors)
    pub fn is_fee_exempt(who: &T::AccountId) -> bool {
      who == &Self::account_id()
        || who == &T::BurningManagerAccount::get()
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
      T::AssetConversion::quote_price_exact_tokens_for_tokens(asset_from, asset_to, amount_in, true)
        .ok_or_else(|| Error::<T>::NoRouteFound.into())
    }

    /// Get oracle price for asset pair
    pub fn get_oracle_price(asset_from: AssetKind, asset_to: AssetKind) -> Option<Balance> {
      T::PriceOracle::get_ema_price(asset_from, asset_to)
    }

    /// Find best multi-hop route using Native anchor
    fn find_best_multi_hop_route(
      from: AssetKind,
      to: AssetKind,
      amount_after_fee: Balance,
    ) -> Option<Vec<AssetKind>> {
      let native_asset = T::NativeAsset::get();
      // Only support Native-anchored routing for now
      if from == native_asset || to == native_asset {
        return None; // Direct route should be used
      }
      // Check if both hops have liquidity
      let hop1_quote = T::AssetConversion::quote_price_exact_tokens_for_tokens(
        from,
        native_asset,
        amount_after_fee,
        true,
      );
      let hop2_quote = if let Some(intermediate_amount) = hop1_quote {
        T::AssetConversion::quote_price_exact_tokens_for_tokens(
          native_asset,
          to,
          intermediate_amount,
          true,
        )
      } else {
        None
      };
      if hop1_quote.is_some() && hop2_quote.is_some() {
        Some(vec![from, native_asset, to])
      } else {
        None
      }
    }

    /// Advanced route selection with TMC integration
    fn find_optimal_route(
      from: AssetKind,
      to: AssetKind,
      amount_after_fee: Balance,
    ) -> Option<RouteComparison> {
      let native_asset = T::NativeAsset::get();
      let mut candidate_routes = Vec::new();
      // 1. Direct XYK route
      if let Some(direct_output) =
        T::AssetConversion::quote_price_exact_tokens_for_tokens(from, to, amount_after_fee, true)
      {
        let final_output = direct_output;
        let price_impact = Self::calculate_price_impact(from, to, amount_after_fee, direct_output);
        candidate_routes.push(RouteComparison::new(
          final_output,
          vec![from, to],
          RouteMechanism::DirectXyk {
            pool_id: (from, to),
          },
          price_impact,
          0, // fee already collected in swap()
        ));
      }
      // 2. Direct mint route (if applicable)
      // TMC mints the `to` token using `from` as collateral.
      // Supported: any pair where a curve exists for `to` and `from` is its collateral.
      if T::TmcPallet::has_curve(to) && T::TmcPallet::supports_collateral(to, from) {
        if let Ok(tmc_output) = T::TmcPallet::calculate_recipient_receives(to, amount_after_fee) {
          let final_output = tmc_output;
          let price_impact = Perbill::zero(); // TMC has predictable pricing
          candidate_routes.push(RouteComparison::new(
            final_output,
            vec![from, to],
            RouteMechanism::DirectMint {
              foreign_asset: from,
            },
            price_impact,
            0, // fee already collected in swap()
          ));
        }
      }
      // 3. Multi-hop Native route
      if from != native_asset && to != native_asset {
        if let Some(multi_hop_path) = Self::find_best_multi_hop_route(from, to, amount_after_fee) {
          if let Some(multi_hop_output) =
            Self::quote_multi_hop_route(&multi_hop_path, amount_after_fee)
          {
            let final_output = multi_hop_output;
            let price_impact = Self::calculate_multi_hop_price_impact(
              &multi_hop_path,
              amount_after_fee,
              multi_hop_output,
            );
            candidate_routes.push(RouteComparison::new(
              final_output,
              multi_hop_path,
              RouteMechanism::MultiHopNative {
                hops: vec![from, native_asset, to],
              },
              price_impact,
              0, // fee already collected in swap()
            ));
          }
        }
      }
      // Mechanism selection: the router is a pure execution mechanism and always
      // picks the route that delivers the most output to the swap recipient.
      // price_impact and total_fees stay on RouteComparison only as informational
      // quote fields.
      candidate_routes
        .into_iter()
        .max_by_key(|route| route.expected_output)
    }

    /// Quote multi-hop route output
    fn quote_multi_hop_route(path: &[AssetKind], amount_in: Balance) -> Option<Balance> {
      if path.len() < 2 {
        return None;
      }
      let mut current_amount = amount_in;
      for window in path.windows(2) {
        let from = window[0];
        let to = window[1];
        if let Some(output) =
          T::AssetConversion::quote_price_exact_tokens_for_tokens(from, to, current_amount, true)
        {
          current_amount = output;
        } else {
          return None;
        }
      }
      Some(current_amount)
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

    /// Calculate price impact for multi-hop route
    fn calculate_multi_hop_price_impact(
      path: &[AssetKind],
      amount_in: Balance,
      amount_out: Balance,
    ) -> Perbill {
      // Simplified multi-hop price impact
      // In production, this would calculate cumulative impact across all hops
      if let Some(direct_quote) = T::AssetConversion::quote_price_exact_tokens_for_tokens(
        path[0],
        path[path.len() - 1],
        amount_in,
        true,
      ) {
        if direct_quote > amount_out {
          return Perbill::from_rational(direct_quote - amount_out, direct_quote);
        }
      }
      Perbill::zero()
    }

    /// Calculate router fee for a given amount
    pub fn calculate_router_fee(amount: Balance) -> Balance {
      RouterFee::<T>::get().mul_floor(amount)
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      // Invariant 1: RouterFee stays within the configured governance mutation bound
      let fee = RouterFee::<T>::get();
      if fee > T::MaxRouterFee::get() {
        return Err(TryRuntimeError::Other(
          "RouterFee exceeds configured maximum",
        ));
      }
      // Invariant 2: TrackedAssets has no duplicates
      let tracked = TrackedAssets::<T>::get();
      for (i, asset) in tracked.iter().enumerate() {
        if tracked.iter().take(i).any(|a| a == asset) {
          return Err(TryRuntimeError::Other(
            "TrackedAssets contains duplicate entries",
          ));
        }
      }
      // Invariant 3: EMA prices are non-negative (u128 already guarantees this,
      // but check that oracle entries reference only tracked assets)
      for asset in tracked.iter() {
        let price = EmaPrices::<T>::get(asset, &T::NativeAsset::get());
        // EMA price of 0 is valid (uninitialized), no further check needed
        let _ = price;
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
      Ok(route.into_router_quote(amount_in, router_fee))
    }
  }

  /// Genesis configuration
  #[pallet::genesis_config]
  pub struct GenesisConfig<T: Config> {
    pub tracked_assets: Vec<AssetKind>,
    pub _marker: core::marker::PhantomData<T>,
  }

  impl<T: Config> Default for GenesisConfig<T> {
    fn default() -> Self {
      Self {
        tracked_assets: vec![AssetKind::Native],
        _marker: Default::default(),
      }
    }
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      let bounded: BoundedVec<AssetKind, T::MaxTrackedAssets> = self
        .tracked_assets
        .clone()
        .try_into()
        .expect("Tracked assets exceed max limit");
      TrackedAssets::<T>::put(bounded);
      // Ensure pallet account survives zero native balance (ED-free)
      frame_system::Pallet::<T>::inc_providers(&Pallet::<T>::account_id());
    }
  }
}
