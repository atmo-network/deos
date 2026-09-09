//! Asset-related pallet configurations for the parachain runtime.
//!
//! Configures:
//! - `pallet-assets`: Fungible asset management
//! - `pallet-asset-conversion`: Uniswap V2-like DEX functionality

use alloc::{vec, vec::Vec};
use polkadot_sdk::{
  frame_support::{parameter_types, traits::*},
  pallet_asset_conversion::{self, PoolLocator},
  pallet_assets,
  sp_runtime::traits::{AccountIdConversion, TryConvert},
};

use crate::{
  AccountId, Balance, Balances, EXISTENTIAL_DEPOSIT, Runtime, RuntimeEvent, RuntimeOrigin,
};
pub use primitives::AssetKind;

/// Asset ID type used throughout the runtime
pub type AssetId = u32;

/// Ensure that privileged asset operations can only be performed by root.
pub type AssetsForceOrigin = polkadot_sdk::frame_system::EnsureRoot<AccountId>;

/// Root-only origin for creating assets, returning deterministic owner account.
pub struct AssetsCreateOrigin;
impl polkadot_sdk::frame_support::traits::EnsureOriginWithArg<RuntimeOrigin, AssetId>
  for AssetsCreateOrigin
{
  type Success = AccountId;

  fn try_origin(o: RuntimeOrigin, _: &AssetId) -> Result<Self::Success, RuntimeOrigin> {
    <AssetsForceOrigin as polkadot_sdk::frame_support::traits::EnsureOrigin<RuntimeOrigin>>::try_origin(o)
      .map(|_| AssetRegistryAccount::get())
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn try_successful_origin(_: &AssetId) -> Result<RuntimeOrigin, ()> {
    Ok(RuntimeOrigin::root())
  }
}

/// Consensus-level LP custody lock for the two immutable TMCTOL Anchor accounts.
///
/// Incoming LP remains admissible, while every debit path observes zero reducible LP balance.
/// Declaring the LP namespace frozen also prevents destruction of an LP asset class.
pub struct AnchorLpFreezer;

impl pallet_assets::FrozenBalance<AssetId, AccountId, Balance> for AnchorLpFreezer {
  fn frozen_balance(asset: AssetId, who: &AccountId) -> Option<Balance> {
    let is_lp = (asset & primitives::assets::MASK_TYPE) == primitives::assets::TYPE_LP;
    let is_anchor = [
      primitives::ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
      primitives::ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID,
    ]
    .into_iter()
    .map(pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system)
    .any(|anchor| anchor == *who);
    (is_lp && is_anchor).then_some(Balance::MAX)
  }

  fn died(_: AssetId, _: &AccountId) {}

  fn contains_freezes(asset: AssetId) -> bool {
    (asset & primitives::assets::MASK_TYPE) == primitives::assets::TYPE_LP
  }
}

/// Converter to distinguish between native and asset tokens
pub struct NativeOrAssetIdConverter;

/// Canonical DEOS pool identity boundary.
///
/// Upstream Asset Conversion compares semantic values only. DEOS additionally rejects semantic
/// aliases of the same `pallet-assets` ledger and any variant that disagrees with its ID namespace.
pub struct DeosPoolLocator;

impl PoolLocator<AccountId, AssetKind, (AssetKind, AssetKind)> for DeosPoolLocator {
  fn address(id: &(AssetKind, AssetKind)) -> Result<AccountId, ()> {
    if !id.0.is_valid_market_pair(id.1) {
      return Err(());
    }
    pallet_asset_conversion::AccountIdConverter::<
      AssetConversionPalletId,
      (AssetKind, AssetKind),
    >::try_convert(id)
    .map_err(|_| ())
  }

  fn pool_id(asset1: &AssetKind, asset2: &AssetKind) -> Result<(AssetKind, AssetKind), ()> {
    if !asset1.is_valid_market_pair(*asset2) {
      return Err(());
    }
    if asset1 < asset2 {
      Ok((*asset1, *asset2))
    } else {
      Ok((*asset2, *asset1))
    }
  }
}

impl
  polkadot_sdk::sp_runtime::traits::Convert<
    AssetKind,
    polkadot_sdk::sp_runtime::Either<(), AssetId>,
  > for NativeOrAssetIdConverter
{
  fn convert(asset_kind: AssetKind) -> polkadot_sdk::sp_runtime::Either<(), AssetId> {
    match asset_kind {
      AssetKind::Native => polkadot_sdk::sp_runtime::Either::Left(()),
      AssetKind::Local(asset_id) | AssetKind::Foreign(asset_id) => {
        polkadot_sdk::sp_runtime::Either::Right(asset_id)
      }
    }
  }
}

polkadot_sdk::frame_support::parameter_types! {
  /// Native asset ID
  pub const NativeAssetId: AssetKind = AssetKind::Native;
}

parameter_types! {
  // -- Assets Pallet Constants --
  /// Minimum balance required to approve an asset transfer
  pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
  /// Minimum balance required to keep an asset account alive
  pub const AssetAccountDeposit: Balance = EXISTENTIAL_DEPOSIT;
  /// Minimum balance required to create an asset
  pub const AssetDeposit: Balance = EXISTENTIAL_DEPOSIT;
  /// Minimum balance required to create metadata for an asset
  pub const MetadataDepositBase: Balance = EXISTENTIAL_DEPOSIT;
  /// Additional deposit required per byte of metadata
  pub const MetadataDepositPerByte: Balance = EXISTENTIAL_DEPOSIT;
  /// Maximum length of asset name
  pub const StringLimit: u32 = 50;

  // -- Asset Conversion Constants --
  pub const AssetConversionPalletId: polkadot_sdk::frame_support::PalletId = polkadot_sdk::frame_support::PalletId(*primitives::ecosystem::pallet_ids::ASSET_CONVERSION_PALLET_ID);
  /// XYK liquidity-provider swap fee (0% for the current launch line).
  pub const XykLpFee: polkadot_sdk::sp_runtime::Permill = polkadot_sdk::sp_runtime::Permill::from_percent(0);
  /// Independent liquidity withdrawal fee (0% for the current launch line).
  pub const LiquidityWithdrawalFee: polkadot_sdk::sp_runtime::Permill = polkadot_sdk::sp_runtime::Permill::from_percent(0);
  /// Minimum liquidity that must be minted when creating a pool
  pub const MintMinLiquidity: Balance = 100;
  /// Pool setup fee to prevent spam pool creation (temporarily disabled for testing)
  pub const PoolSetupFee: Balance = 0;
}

impl pallet_assets::Config for Runtime {
  type ApprovalDeposit = ApprovalDeposit;
  type AssetAccountDeposit = AssetAccountDeposit;
  type AssetDeposit = AssetDeposit;
  type AssetId = AssetId;
  type AssetIdParameter = AssetId;
  type Balance = Balance;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = ();
  // Push architecture via polling: pallet-assets lacks per-transfer callbacks,
  // only lifecycle (created/destroyed) hooks. Liquidity actors use pending-intake
  // storage to decouple scanning (on_initialize) from execution (on_idle).
  type CallbackHandle = ();
  type CreateOrigin = AssetsCreateOrigin;
  type Currency = Balances;
  type Extra = ();
  type ReserveData = ();
  type ForceOrigin = AssetsForceOrigin;
  type Freezer = AnchorLpFreezer;
  type Holder = ();
  type MetadataDepositBase = MetadataDepositBase;
  type MetadataDepositPerByte = MetadataDepositPerByte;
  type RemoveItemsLimit = ConstU32<1000>;
  type RuntimeEvent = RuntimeEvent;
  type StringLimit = StringLimit;
  type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const AssetRegistryPalletId: polkadot_sdk::frame_support::PalletId = polkadot_sdk::frame_support::PalletId(*primitives::ecosystem::pallet_ids::ASSET_REGISTRY_PALLET_ID);
}

pub struct AssetRegistryAccount;
impl polkadot_sdk::frame_support::traits::Get<AccountId> for AssetRegistryAccount {
  fn get() -> AccountId {
    AssetRegistryPalletId::get().into_account_truncating()
  }
}

pub fn genesis_protocol_assets() -> Vec<(AssetId, AccountId, bool, Balance)> {
  vec![(
    primitives::ecosystem::protocol_tokens::VETO_ASSET_ID,
    AssetRegistryAccount::get(),
    true,
    1,
  )]
}

pub fn genesis_protocol_asset_metadata() -> Vec<(AssetId, Vec<u8>, Vec<u8>, u8)> {
  let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
  let metadata = primitives::get_well_known_metadata(AssetKind::Local(asset_id))
    .expect("well-known protocol asset metadata must exist");
  vec![(asset_id, metadata.name, metadata.symbol, metadata.decimals)]
}

pub struct AssetRegistryTokenDomainHook;
impl pallet_asset_registry::TokenDomainHook for AssetRegistryTokenDomainHook {
  fn on_token_registered(_token_asset: AssetKind) -> polkadot_sdk::sp_runtime::DispatchResult {
    Ok(())
  }
}

pub struct ReservedForeignAssetLocations;
impl Contains<polkadot_sdk::staging_xcm::latest::Location> for ReservedForeignAssetLocations {
  fn contains(location: &polkadot_sdk::staging_xcm::latest::Location) -> bool {
    location == &polkadot_sdk::staging_xcm::latest::Location::here()
  }
}

impl pallet_asset_registry::Config for Runtime {
  type RegistryOrigin = AssetsForceOrigin;
  type AssetIdGenerator = crate::configs::xcm_config::LocationToAssetId;
  type AssetOwner = AssetRegistryAccount;
  type ReservedLocations = ReservedForeignAssetLocations;
  type TokenDomainHook = AssetRegistryTokenDomainHook;
  type WeightInfo = crate::weights::pallet_asset_registry::SubstrateWeight<Runtime>;
}

pub struct DeosPoolLifecycle;

#[cfg(test)]
std::thread_local! {
  static FAIL_AFTER_POOL_CREATION: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
  static FORCE_LP_IDENTITY_MISMATCH: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn set_fail_after_pool_creation(value: bool) {
  FAIL_AFTER_POOL_CREATION.with(|flag| flag.set(value));
}

#[cfg(test)]
pub(crate) fn set_force_lp_identity_mismatch(value: bool) {
  FORCE_LP_IDENTITY_MISMATCH.with(|flag| flag.set(value));
}

impl pallet_deos_router::PoolLifecycleApi<AccountId> for DeosPoolLifecycle {
  #[polkadot_sdk::frame_support::transactional]
  fn create_pool(
    who: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    let pool_id = DeosPoolLocator::pool_id(&asset1, &asset2)
      .map_err(|_| polkadot_sdk::sp_runtime::DispatchError::Other("Invalid asset pair"))?;
    polkadot_sdk::frame_support::ensure!(
      !pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id),
      polkadot_sdk::sp_runtime::DispatchError::Other("Pool already exists")
    );
    super::deos_router_config::AssetConversionAdapter::ensure_lp_asset_namespace();
    let expected_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("LP namespace unavailable"),
    )?;
    preflight_register_pool_lp_pair(pool_id.0, pool_id.1, expected_lp)?;
    crate::AssetConversion::create_pool(
      RuntimeOrigin::signed(who.clone()),
      alloc::boxed::Box::new(pool_id.0),
      alloc::boxed::Box::new(pool_id.1),
    )?;
    let actual = pallet_asset_conversion::Pools::<Runtime>::get(pool_id).ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("Created pool missing"),
    )?;
    #[cfg(test)]
    let expected_lp = if FORCE_LP_IDENTITY_MISMATCH.with(|flag| flag.get()) {
      expected_lp.saturating_add(1)
    } else {
      expected_lp
    };
    polkadot_sdk::frame_support::ensure!(
      actual.lp_token == expected_lp,
      polkadot_sdk::sp_runtime::DispatchError::Other("LP identity mismatch")
    );
    #[cfg(test)]
    if FAIL_AFTER_POOL_CREATION.with(|flag| flag.get()) {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "Injected post-pool lifecycle failure",
      ));
    }
    register_pool_lp_pair(pool_id.0, pool_id.1)
  }
}

pub(crate) fn preflight_register_pool_lp_pair(
  asset1: AssetKind,
  asset2: AssetKind,
  lp_token: u32,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  crate::DeosRouter::preflight_register_lp_pair(lp_token, (asset1, asset2))?;
  super::oracle_config::preflight_deos_router_pool_feeds(asset1, asset2)
}

#[polkadot_sdk::frame_support::transactional]
pub(crate) fn register_pool_lp_pair(
  asset1: AssetKind,
  asset2: AssetKind,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  let pool_id =
    <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset1, &asset2)
      .map_err(|_| polkadot_sdk::sp_runtime::DispatchError::Other("Invalid asset pair"))?;
  let pool = pallet_asset_conversion::Pools::<Runtime>::get(&pool_id).ok_or(
    polkadot_sdk::sp_runtime::DispatchError::Other("Pool not found"),
  )?;
  crate::DeosRouter::register_lp_pair(pool.lp_token, pool_id)?;
  super::oracle_config::ensure_deos_router_pool_feeds(pool_id.0, pool_id.1)
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AssetConversionBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_asset_conversion::BenchmarkHelper<AssetKind> for AssetConversionBenchmarkHelper {
  fn create_pair(seed1: u32, seed2: u32) -> (AssetKind, AssetKind) {
    (
      AssetKind::Local(seed1 & primitives::MASK_INDEX),
      AssetKind::Local(seed2 & primitives::MASK_INDEX),
    )
  }
}

impl pallet_asset_conversion::Config for Runtime {
  type AssetKind = AssetKind;
  type Assets = polkadot_sdk::frame_support::traits::fungible::UnionOf<
    Balances,
    pallet_assets::Pallet<Runtime>,
    NativeOrAssetIdConverter,
    AssetKind,
    AccountId,
  >;
  type Balance = Balance;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = AssetConversionBenchmarkHelper;
  type HigherPrecisionBalance = polkadot_sdk::sp_core::U256;
  type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
  type LPFee = XykLpFee;
  type MaxSwapPathLength = ConstU32<4>;
  type MintMinLiquidity = MintMinLiquidity;
  type PalletId = AssetConversionPalletId;
  type PoolAssetId = u32;
  type PoolAssets = pallet_assets::Pallet<Runtime>;
  type PoolId = (AssetKind, AssetKind);
  type PoolLocator = DeosPoolLocator;
  type PoolSetupFee = PoolSetupFee;
  type PoolSetupFeeAsset = NativeAssetId;
  type PoolSetupFeeTarget = ();
  type RuntimeEvent = RuntimeEvent;
  type WeightInfo = pallet_asset_conversion::weights::SubstrateWeight<Runtime>;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reference_runtime_xyk_and_withdrawal_fees_are_independent_zero_parameters() {
    assert_eq!(XykLpFee::get(), polkadot_sdk::sp_runtime::Permill::zero());
    assert_eq!(
      LiquidityWithdrawalFee::get(),
      polkadot_sdk::sp_runtime::Permill::zero()
    );
    assert_eq!(
      <Runtime as pallet_asset_conversion::Config>::LPFee::get(),
      XykLpFee::get()
    );
    assert_eq!(
      <Runtime as pallet_asset_conversion::Config>::LiquidityWithdrawalFee::get(),
      LiquidityWithdrawalFee::get()
    );
    assert_eq!(
      super::super::deos_router_config::DeosRouterFee::get(),
      polkadot_sdk::sp_runtime::Perbill::from_rational(5u32, 1_000u32)
    );
  }

  #[test]
  fn canonical_pool_locator_rejects_same_ledger_and_noncanonical_pairs() {
    let id = primitives::TYPE_FOREIGN | 7;
    assert!(DeosPoolLocator::pool_id(&AssetKind::Local(id), &AssetKind::Foreign(id)).is_err());
    assert!(DeosPoolLocator::pool_id(&AssetKind::Native, &AssetKind::Local(id)).is_err());
    assert!(
      DeosPoolLocator::pool_id(
        &AssetKind::Native,
        &AssetKind::Foreign(primitives::TYPE_PROTOCOL | 7),
      )
      .is_err()
    );
  }

  #[test]
  fn canonical_pool_locator_orders_distinct_physical_assets_once() {
    let foreign = AssetKind::Foreign(primitives::TYPE_FOREIGN | 7);
    let expected = (AssetKind::Native, foreign);
    assert_eq!(
      DeosPoolLocator::pool_id(&foreign, &AssetKind::Native),
      Ok(expected)
    );
    assert_eq!(
      DeosPoolLocator::pool_id(&AssetKind::Native, &foreign),
      Ok(expected)
    );
    assert!(DeosPoolLocator::address(&expected).is_ok());
  }

  #[test]
  fn genesis_protocol_asset_policy_exposes_well_known_veto_asset() {
    let assets = genesis_protocol_assets();
    assert_eq!(assets.len(), 1);
    assert_eq!(
      assets[0],
      (
        primitives::ecosystem::protocol_tokens::VETO_ASSET_ID,
        AssetRegistryAccount::get(),
        true,
        1,
      )
    );
  }

  #[test]
  fn genesis_protocol_asset_metadata_matches_well_known_veto_definition() {
    let metadata = genesis_protocol_asset_metadata();
    assert_eq!(metadata.len(), 1);
    assert_eq!(
      metadata[0].0,
      primitives::ecosystem::protocol_tokens::VETO_ASSET_ID
    );
    assert_eq!(metadata[0].1, b"Veto Governance Token".to_vec());
    assert_eq!(metadata[0].2, b"VETO".to_vec());
    assert_eq!(metadata[0].3, 12);
  }
}
