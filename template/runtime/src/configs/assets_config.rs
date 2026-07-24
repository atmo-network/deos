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
  sp_runtime::traits::AccountIdConversion,
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

/// Converter to distinguish between native and asset tokens
pub struct NativeOrAssetIdConverter;

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
  /// Liquidity withdrawal fee (0%)
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
  type Freezer = ();
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

impl pallet_asset_registry::Config for Runtime {
  type RegistryOrigin = AssetsForceOrigin;
  type AssetIdGenerator = crate::configs::xcm_config::LocationToAssetId;
  type AssetOwner = AssetRegistryAccount;
  type TokenDomainHook = AssetRegistryTokenDomainHook;
  type WeightInfo = crate::weights::pallet_asset_registry::SubstrateWeight<Runtime>;
}

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
  crate::AxialRouter::register_lp_pair(pool.lp_token, pool_id)
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
  type BenchmarkHelper = ();
  type HigherPrecisionBalance = polkadot_sdk::sp_core::U256;
  type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
  type LPFee = LiquidityWithdrawalFee;
  type MaxSwapPathLength = ConstU32<4>;
  type MintMinLiquidity = MintMinLiquidity;
  type PalletId = AssetConversionPalletId;
  type PoolAssetId = u32;
  type PoolAssets = pallet_assets::Pallet<Runtime>;
  type PoolId = (AssetKind, AssetKind);
  type PoolLocator = pallet_asset_conversion::Chain<
    pallet_asset_conversion::WithFirstAsset<
      NativeAssetId,
      AccountId,
      AssetKind,
      pallet_asset_conversion::AccountIdConverter<AssetConversionPalletId, (AssetKind, AssetKind)>,
    >,
    pallet_asset_conversion::Ascending<
      AccountId,
      AssetKind,
      pallet_asset_conversion::AccountIdConverter<AssetConversionPalletId, (AssetKind, AssetKind)>,
    >,
  >;
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
