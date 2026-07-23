//! Token Minting Curve pallet configuration for the DEOS reference runtime.
//!
//! Configures the linear price ceiling bonding curve system for the current
//! TMCTOL standard with mathematical price boundaries and treasury-owned
//! liquidity distribution.
//!
//! All account IDs and economic parameters are imported from `primitives::ecosystem`,
//! serving as the single source of truth.

use super::*;

use polkadot_sdk::frame_support::traits::Get;
use polkadot_sdk::sp_runtime::DispatchError;
use primitives::{AssetKind, ecosystem};
use sp_runtime::Perbill;

parameter_types! {
  /// Initial price for token minting (1:1 ratio for testing)
  pub const TmcInitialPrice: Balance = 1_000_000_000_000;

  /// Pallet ID for the token minting curve (from ecosystem constants)
  pub const TmcPalletId: PalletId = PalletId(*ecosystem::pallet_ids::TMC_PALLET_ID);

  /// Precision for mathematical calculations (ecosystem constant: 10^12)
  pub const TmcPrecision: Balance = ecosystem::params::PRECISION;

  /// Slope parameter for linear price ceiling (ecosystem constant: 0.000001 per token)
  pub const TmcSlopeParameter: Balance = ecosystem::params::TMC_SLOPE_PARAMETER;

  /// Distribution ratio for user allocation (ecosystem constant: 33.3%)
  pub const TmcUserAllocationRatio: Perbill = ecosystem::params::TMC_USER_ALLOCATION;

  /// Distribution ratio for liquidity actor allocation (ecosystem constant: 66.6%)
  pub const TmcZapAllocationRatio: Perbill = ecosystem::params::TMC_ZAP_ALLOCATION;
}

use super::axial_router_config::LiquidityActorAccount;

pub struct TmctolDomainGlue;
impl pallet_tmc::DomainGlueHook for TmctolDomainGlue {
  fn on_curve_created(
    _token_asset: AssetKind,
    _foreign_asset: AssetKind,
  ) -> Result<(), DispatchError> {
    Ok(())
  }
}

/// Routes collateral and minted liquidity share per curve.
///
/// L1 (Native): both outputs → Liquidity Actor.
/// L2 (BLDR): collateral → BLDR Liquidity Actor; minted share → BLDR Splitter.
pub struct TmctolMintOutput;
impl pallet_tmc::MintOutputResolver<AccountId> for TmctolMintOutput {
  fn output_accounts(minted_asset: AssetKind) -> pallet_tmc::MintOutputAccounts<AccountId> {
    match minted_asset {
      AssetKind::Local(id) if id == ecosystem::protocol_tokens::BLDR_ASSET_ID => {
        pallet_tmc::MintOutputAccounts {
          collateral: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
            ecosystem::aaa_ids::BLDR_ZM_AAA_ID,
          ),
          minted: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
            ecosystem::aaa_ids::BLDR_SPLITTER_AAA_ID,
          ),
        }
      }
      _ => {
        let liquidity_actor = LiquidityActorAccount::get();
        pallet_tmc::MintOutputAccounts {
          collateral: liquidity_actor.clone(),
          minted: liquidity_actor,
        }
      }
    }
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TmcBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_tmc::BenchmarkHelper<AccountId> for TmcBenchmarkHelper {
  fn create_asset(asset_id: u32) -> polkadot_sdk::sp_runtime::DispatchResult {
    if !<pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(asset_id) {
      let _ = pallet_assets::Pallet::<Runtime>::force_create(
        RuntimeOrigin::root(),
        asset_id,
        polkadot_sdk::sp_runtime::MultiAddress::Id(BurningManagerAccount::get()),
        true,
        1,
      );
    }
    Ok(())
  }

  fn mint_native(to: &AccountId, amount: Balance) -> polkadot_sdk::sp_runtime::DispatchResult {
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        to, amount,
      );
    Ok(())
  }

  fn mint_local(
    asset_id: u32,
    to: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::fungibles::Mutate;
    <pallet_assets::Pallet<Runtime> as Mutate<AccountId>>::mint_into(asset_id, to, amount)?;
    Ok(())
  }
}

impl pallet_tmc::pallet::Config for Runtime {
  type AdminOrigin = frame_system::EnsureRoot<AccountId>;
  type Assets = pallet_assets::Pallet<Runtime>;
  type Balance = Balance;
  type Currency = Balances;
  type InitialPrice = TmcInitialPrice;
  type PalletId = TmcPalletId;
  type Precision = TmcPrecision;
  type SlopeParameter = TmcSlopeParameter;
  type DomainGlueHook = TmctolDomainGlue;
  type TreasuryAccount = LiquidityActorAccount;
  type MintOutputResolver = TmctolMintOutput;
  type UserAllocationRatio = TmcUserAllocationRatio;
  type MintDistributionHook = ();
  type WeightInfo = crate::weights::pallet_tmc::SubstrateWeight<Runtime>;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = TmcBenchmarkHelper;
}
