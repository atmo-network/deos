use crate::AdapterFailure;
#[cfg(feature = "runtime-benchmarks")]
use polkadot_sdk::frame_support::pallet_prelude::DispatchResult;

// Re-export AssetKind from primitives as the single source of truth
pub use primitives::AssetKind;

/// Fee routing adapter for direct fee transfer to the Burn Actor.
pub trait FeeRoutingAdapter<AccountId, Balance> {
  /// Route fee directly from sender to the Burn Actor account.
  fn route_fee(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), AdapterFailure>;
}

/// Price-observation interface for local deviation checks
pub trait PriceOracle<Balance> {
  /// Update EMA price for an asset pair
  fn update_ema_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    price: Balance,
  ) -> Result<(), AdapterFailure>;

  /// Get current EMA price for an asset pair
  fn get_ema_price(asset_in: AssetKind, asset_out: AssetKind) -> Option<Balance>;

  /// Validate price deviation from EMA
  fn validate_price_deviation(
    asset_in: AssetKind,
    asset_out: AssetKind,
    current_price: Balance,
  ) -> Result<(), AdapterFailure>;
}

/// TMC interface for DEOS Router integration
pub trait TmcInterface<AccountId, Balance> {
  /// Check if TMC curve exists for asset
  fn has_curve(asset: AssetKind) -> bool;

  /// Check whether the curve accepts the provided collateral asset
  fn supports_collateral(token_asset: AssetKind, foreign_asset: AssetKind) -> bool;

  /// Calculate the amount delivered to the swap recipient for a direct TMC mint.
  /// Protocol/sink allocation is excluded so router route selection and slippage
  /// compare the user's actual output, not total curve emission.
  fn calculate_recipient_receives(
    token_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, AdapterFailure>;

  /// Mint with distribution. Collateral is taken from `who` while the freshly
  /// minted user allocation is delivered to `recipient`; the zap allocation
  /// always lands in the protocol sink. Returns the amount delivered to
  /// `recipient`, not the total curve emission.
  fn mint_with_distribution(
    who: &AccountId,
    recipient: &AccountId,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, AdapterFailure>;
}

/// Asset conversion API for XYK pools
pub trait AssetConversionApi<AccountId, Balance> {
  /// Get pool ID for asset pair
  fn single_pool_id(asset_a: AssetKind, asset_b: AssetKind) -> Option<(AssetKind, AssetKind)>;

  /// Get pool reserves
  fn single_pool_reserves(pool_id: (AssetKind, AssetKind)) -> Option<(Balance, Balance)>;

  /// Quote the output received from an exact input to one XYK pool.
  fn quote_single_pool_exact_input(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    include_fee: bool,
  ) -> Option<Balance>;

  /// Quote the input required to receive an exact output from one XYK pool.
  fn quote_single_pool_exact_output(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    include_fee: bool,
  ) -> Option<Balance>;

  /// Execute one identified XYK pool leg under an exact-input floor.
  fn execute_single_pool_exact_input(
    who: AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    min_amount_out: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<Balance, AdapterFailure>;

  /// Execute one identified XYK pool leg under an exact-output ceiling.
  fn execute_single_pool_exact_output(
    who: AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    max_amount_in: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<crate::ExactOutputExecution, AdapterFailure>;
}

/// Helper for benchmarking
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetKind, AccountId, Balance> {
  fn create_asset(asset: AssetKind) -> DispatchResult;
  fn mint_asset(asset: AssetKind, to: &AccountId, amount: Balance) -> DispatchResult;
  fn create_pool(asset1: AssetKind, asset2: AssetKind) -> DispatchResult;
  fn create_tmc_curve(token_asset: AssetKind, collateral_asset: AssetKind) -> DispatchResult;
  fn add_liquidity(
    who: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
    amount1: Balance,
    amount2: Balance,
  ) -> DispatchResult;
}
