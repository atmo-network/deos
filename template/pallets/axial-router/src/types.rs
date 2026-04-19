use polkadot_sdk::frame_support::pallet_prelude::*;
use scale_info::prelude::vec::Vec;

// Re-export AssetKind from primitives as the single source of truth
pub use primitives::AssetKind;

/// Fee routing adapter for direct fee transfer to burning manager
pub trait FeeRoutingAdapter<AccountId, Balance> {
  /// Route fee directly from sender to burning manager account
  fn route_fee(who: &AccountId, asset: AssetKind, amount: Balance) -> DispatchResult;
}

/// Price oracle interface for manipulation-resistant pricing
pub trait PriceOracle<Balance> {
  /// Update EMA price for an asset pair
  fn update_ema_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    price: Balance,
  ) -> Result<(), DispatchError>;

  /// Get current EMA price for an asset pair
  fn get_ema_price(asset_in: AssetKind, asset_out: AssetKind) -> Option<Balance>;

  /// Validate price deviation from EMA
  fn validate_price_deviation(
    asset_in: AssetKind,
    asset_out: AssetKind,
    current_price: Balance,
  ) -> Result<(), DispatchError>;
}

/// TMC interface for Axial Router integration
pub trait TmcInterface<AccountId, Balance> {
  /// Check if TMC curve exists for asset
  fn has_curve(asset: AssetKind) -> bool;

  /// Check whether the curve accepts the provided collateral asset
  fn supports_collateral(token_asset: AssetKind, foreign_asset: AssetKind) -> bool;

  /// Calculate user receives for given foreign amount
  fn calculate_user_receives(
    token_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, DispatchError>;

  /// Mint with distribution
  fn mint_with_distribution(
    who: &AccountId,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, DispatchError>;
}

/// Asset conversion API for XYK pools
pub trait AssetConversionApi<AccountId, Balance> {
  /// Get pool ID for asset pair
  fn get_pool_id(asset_a: AssetKind, asset_b: AssetKind) -> Option<(AssetKind, AssetKind)>;

  /// Get pool reserves
  fn get_pool_reserves(pool_id: (AssetKind, AssetKind)) -> Option<(Balance, Balance)>;

  /// Quote price for exact tokens
  fn quote_price_exact_tokens_for_tokens(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    include_fee: bool,
  ) -> Option<Balance>;

  /// Execute swap
  fn swap_exact_tokens_for_tokens(
    who: AccountId,
    path: Vec<AssetKind>,
    amount_in: Balance,
    min_amount_out: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<Balance, DispatchError>;
}

/// Weight information for benchmarking
pub trait WeightInfo {
  /// Weight for swap operation
  fn swap() -> Weight;
}

/// Helper for benchmarking
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetKind, AccountId, Balance> {
  fn create_asset(asset: AssetKind) -> DispatchResult;
  fn mint_asset(asset: AssetKind, to: &AccountId, amount: Balance) -> DispatchResult;
  fn create_pool(asset1: AssetKind, asset2: AssetKind) -> DispatchResult;
  fn add_liquidity(
    who: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
    amount1: Balance,
    amount2: Balance,
  ) -> DispatchResult;
}
