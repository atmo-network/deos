//! AAA pallet configuration for the DEOS reference runtime.
//!
//! Wires the two adapter traits (`AssetOps`, `DexOps`) to concrete runtime pallets:
//! - Native token: `pallet-balances`
//! - Foreign assets: `pallet-assets`
//! - Swaps: Axial Router
//! - Liquidity: Asset Conversion

use super::*;
use primitives::{AssetKind, ecosystem};

use polkadot_sdk::frame_support::traits::{
  Currency, Get,
  fungible::Inspect as NativeInspect,
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
  tokens::{Fortitude, Precision, Preservation},
};
use polkadot_sdk::pallet_asset_conversion::PoolLocator;
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult, Perbill, TokenError};

use crate::{AssetConversion, RuntimeOrigin};
use pallet_aaa::{AssetOps, DexOps, FeeCollector, FundingAuthority, LiquidityDonationOps};

parameter_types! {
  // --- Identity and ownership ---

  pub const AaaPalletId: PalletId = PalletId(*ecosystem::pallet_ids::AAA_PALLET_ID);
  pub const AaaNativeAssetId: AssetKind = AssetKind::Native;
  /// User AAA slot capacity per owner; System AAA is not constrained by this limit
  pub const AaaMaxOwnerSlots: u8 = 8;

  // --- Execution-plan and task bounds ---

  pub const AaaMaxExecutionPlanSteps: u32 = 10;
  pub const AaaMaxSystemExecutionPlanSteps: u32 = 10;
  pub const AaaMaxUserExecutionPlanSteps: u32 = 3;
  pub const AaaMaxFundingTrackedAssets: u32 = 10;
  pub const AaaMaxConditionsPerStep: u32 = 4;
  pub const AaaMaxSplitTransferLegs: u32 = 8;
  pub const AaaMaxPoolScan: u32 = 64;

  // --- Trigger and schedule bounds ---

  pub const AaaMaxExecutionDelayBlocks: BlockNumber = 52_560_000;
  pub const AaaMaxTimerJitterBlocks: u32 = 64;
  pub const AaaMinWindowLength: BlockNumber = 100;
  pub const AaaMaxWhitelistSize: u32 = 16;

  // --- Scheduler controls ---

  /// Defense-in-depth count ceiling; RefTime and ProofSize admission remain primary.
  pub const AaaMaxExecutionsPerBlock: u32 = 1_000;
  pub const AaaMaxQueueLength: u32 = 10_000;
  /// Provisional paged-FIFO candidate; production selection awaits 32/64/128 Wasm comparison.
  pub const AaaQueuePageSize: u32 = 64;
  pub const AaaMaxQueueEntriesScannedPerBlock: u32 = 10_000;
  pub const AaaMaxWakeupBucketSize: u32 = 10_000;
  pub const AaaMaxWakeupsPerBlock: u32 = 512;
  pub const AaaMaxSpilloverBlocks: u32 = 8;
  pub const AaaMaxIngressEventsPerBlock: u32 = 1024;
  pub const AaaMaxIngressOverflowQueue: u32 = 8192;
  pub AaaGuaranteedOnIdleWeight: Weight =
    MIN_ON_IDLE_RESERVE_RATIO * MAXIMUM_BLOCK_WEIGHT;

  // --- Lifecycle and sweep controls ---

  pub const AaaMaxConsecutiveFailures: u32 = 10;
  pub const AaaMaxAutoCloseNonceHorizon: u64 = 10_000;
  pub const AaaMinUserBalance: Balance = 5 * ExistentialDeposit::get();
  pub const AaaMaxSweepPerBlock: u32 = 5;

  // --- Starvation safeguard controls ---

  pub const AaaMaxIdleStarvationBlocks: u32 = 25;
  /// Maximum number of active AAA instances. Bounds the BTreeSet storage.
  /// Set to 10,000 for production use cases with high automation density.
  pub const AaaMaxActiveActors: u32 = 10_000;
  // --- Economic parameters ---

  /// Per-step flat evaluation fee
  pub const AaaStepBaseFee: u128 = 2_000_000_000;
  /// Per-condition evaluation fee
  pub const AaaConditionReadFee: u128 = 500_000_000;
  /// Non-refundable opening fee routed to `FeeSink`
  pub const AaaCreationFee: Balance = ExistentialDeposit::get();
}

pub struct AaaMinUserBalanceGuard;

impl Get<Balance> for AaaMinUserBalanceGuard {
  fn get() -> Balance {
    AaaMinUserBalance::get().max(ExistentialDeposit::get())
  }
}

/// Canonical unified fee-collection boundary for AAA charges.
///
/// The collector transfers every opening, evaluation, execution, and close-tail fee in full to
/// the Fee Sink System AAA. Phase-specific allocation happens later through that actor's bounded
/// execution plan rather than inside the collection path.
pub struct TmctolFeeCollector;

impl FeeCollector<AccountId, AssetKind, Balance> for TmctolFeeCollector {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult {
    if amount == 0 {
      return Ok(());
    }
    TmctolAssetOps::transfer(payer, fee_sink, native_asset, amount)
  }
}

pub struct AaaFeeRecipient;
impl Get<crate::AccountId> for AaaFeeRecipient {
  fn get() -> crate::AccountId {
    crate::AAA::sovereign_account_id_system(ecosystem::aaa_ids::FEE_SINK_AAA_ID)
  }
}

pub struct TmctolAssetOps;

impl TmctolAssetOps {
  fn bridge_native_staking_ingress(to: &AccountId, amount: Balance) -> Result<(), DispatchError> {
    if amount == 0 {
      return Ok(());
    }
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      native_asset_id,
    ) {
      return Ok(());
    }
    let lp_farmer = crate::AAA::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
    );
    if to != &lp_farmer {
      return Ok(());
    }
    let (_, remainder) = <Balances as Currency<AccountId>>::slash(to, amount);
    if remainder > 0 {
      return Err(DispatchError::Token(TokenError::FundsUnavailable));
    }
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      native_asset_id,
      to,
      amount,
    )?;
    Ok(())
  }

  pub fn bridge_native_staking_pool_yield() -> Result<(), DispatchError> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      native_asset_id,
    ) {
      return Ok(());
    }
    let staking_pool = crate::Staking::pool_account_for(native_asset_id);
    let amount = <Balances as Currency<AccountId>>::free_balance(&staking_pool);
    if amount == 0 {
      return Ok(());
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let (_, remainder) = <Balances as Currency<AccountId>>::slash(&staking_pool, amount);
      if remainder > 0 {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          DispatchError::Token(TokenError::FundsUnavailable),
        ));
      }
      if let Err(error) = <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
        native_asset_id,
        &staking_pool,
        amount,
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }
}

impl AssetOps<AccountId, AssetKind, Balance> for TmctolAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) =
        <RuntimeAddressEventIngress as AddressEventIngress>::preflight_internal_inbound(
          to, asset, amount, from,
        )
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let result = (|| -> Result<(), DispatchError> {
        match asset {
          AssetKind::Native => {
            <Balances as Currency<AccountId>>::transfer(
              from,
              to,
              amount,
              polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
            )?;
            Self::bridge_native_staking_ingress(to, amount)?;
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::transfer(
              id,
              from,
              to,
              amount,
              Preservation::Expendable,
            )?;
          }
        }
        <RuntimeAddressEventIngress as AddressEventIngress>::on_internal_inbound(
          to, asset, amount, from,
        )?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn burn(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), DispatchError> {
    match asset {
      AssetKind::Native => {
        let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
        if remainder > 0 {
          return Err(DispatchError::Token(TokenError::FundsUnavailable));
        }
        Ok(())
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::burn_from(
          id,
          who,
          amount,
          Preservation::Expendable,
          Precision::Exact,
          Fortitude::Polite,
        )?;
        Ok(())
      }
    }
  }

  fn mint(to: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), DispatchError> {
    match asset {
      AssetKind::Native => {
        let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(id, to, amount)?;
      }
    }
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_without_source(
      to, asset, amount,
    )?;
    Ok(())
  }

  fn balance(who: &AccountId, asset: AssetKind) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::reducible_balance(
        who,
        Preservation::Expendable,
        Fortitude::Polite,
      ),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::reducible_balance(
          id,
          who,
          Preservation::Expendable,
          Fortitude::Polite,
        )
      }
    }
  }

  fn minimum_balance(asset: AssetKind) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::minimum_balance(),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::minimum_balance(id)
      }
    }
  }

  fn can_deposit(who: &AccountId, asset: AssetKind, amount: Balance) -> bool {
    if amount == 0 {
      return true;
    }
    let current = match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(who),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, who)
      }
    };
    if current != 0 {
      return true;
    }
    amount >= Self::minimum_balance(asset)
  }
}

pub struct TmctolDexOps;

pub struct TmctolLiquidityDonationOps;
impl LiquidityDonationOps<AccountId, AssetKind, Balance> for TmctolLiquidityDonationOps {
  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    amount: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), DispatchError> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    if asset_a == AssetKind::Local(native_asset_id) && asset_b == AssetKind::Local(staked_asset_id)
    {
      let donation =
        crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
          who,
          amount,
          max_ratio_error,
        )?;
      TmctolAssetOps::bridge_native_staking_pool_yield()?;
      return Ok(donation);
    }
    Err(DispatchError::Other("LiquidityDonationUnsupported"))
  }
}

impl DexOps<AccountId, AssetKind, Balance> for TmctolDexOps {
  fn swap_exact_in(
    who: &AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<Balance, DispatchError> {
    let quote = pallet_axial_router::Pallet::<Runtime>::quote_exact_input(
      who.clone(),
      asset_in,
      asset_out,
      amount_in,
    )?;
    let min_out =
      (polkadot_sdk::sp_runtime::Perbill::one() - slippage_tolerance).mul_floor(quote.amount_out);
    pallet_axial_router::Pallet::<Runtime>::execute_swap_for(
      who, asset_in, asset_out, amount_in, min_out, who,
    )
  }

  fn swap_exact_out(
    who: &AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<Balance, DispatchError> {
    if amount_out == 0 {
      return Err(DispatchError::Other("ZeroAmountOut"));
    }
    if max_amount_in == 0 {
      return Err(DispatchError::Other("ExactOutInputCapacityExceeded"));
    }
    let quote_output = |amount_in: Balance| -> Result<Balance, DispatchError> {
      if amount_in == 0 {
        return Err(DispatchError::Other("ZeroAmountIn"));
      }
      Ok(
        pallet_axial_router::Pallet::<Runtime>::quote_exact_input(
          who.clone(),
          asset_in,
          asset_out,
          amount_in,
        )?
        .amount_out,
      )
    };
    if quote_output(max_amount_in)? < amount_out {
      return Err(DispatchError::Other("ExactOutInputCapacityExceeded"));
    }
    let mut low: Balance = 1;
    let mut high = max_amount_in;
    for _ in 0..Self::MAX_EXACT_OUT_QUOTE_STEPS {
      if low >= high {
        break;
      }
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match quote_output(mid) {
        Ok(quoted) if quoted >= amount_out => high = mid,
        _ => low = mid.saturating_add(1),
      }
    }
    let required_in = high;
    let quoted_max_in = required_in.saturating_add(slippage_tolerance.mul_ceil(required_in));
    if quoted_max_in > max_amount_in {
      return Err(DispatchError::Other("ExactOutInputCapacityExceeded"));
    }
    if TmctolAssetOps::balance(who, asset_in) < required_in {
      return Err(DispatchError::Other("InsufficientInputForExactOut"));
    }
    pallet_axial_router::Pallet::<Runtime>::execute_swap_for(
      who,
      asset_in,
      asset_out,
      required_in,
      amount_out,
      who,
    )?;
    Ok(required_in)
  }

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    amount_a: Balance,
    amount_b: Balance,
  ) -> Result<(Balance, Balance, Balance), DispatchError> {
    use alloc::boxed::Box;
    // Auto-create pool if it doesn't exist yet
    if AssetConversion::get_reserves(asset_a, asset_b).is_err() {
      AssetConversion::create_pool(
        RuntimeOrigin::signed(who.clone()),
        Box::new(asset_a),
        Box::new(asset_b),
      )?;
    }
    let lp_before = Self::lp_balance(who, asset_a, asset_b);
    AssetConversion::add_liquidity(
      RuntimeOrigin::signed(who.clone()),
      Box::new(asset_a),
      Box::new(asset_b),
      amount_a,
      amount_b,
      0,
      0,
      who.clone(),
    )?;
    let lp_after = Self::lp_balance(who, asset_a, asset_b);
    let lp_minted = lp_after.saturating_sub(lp_before);
    Ok((amount_a, amount_b, lp_minted))
  }

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetKind,
    lp_amount: Balance,
  ) -> Result<(Balance, Balance), DispatchError> {
    use alloc::boxed::Box;
    let lp_id = match lp_asset {
      AssetKind::Local(id) => id,
      _ => return Err(DispatchError::Other("LP asset must be Local")),
    };
    let (asset_a, asset_b) =
      Self::pool_pair_for_lp(lp_id).ok_or(DispatchError::Other("Pool not found for LP token"))?;
    let before_a = TmctolAssetOps::balance(who, asset_a);
    let before_b = TmctolAssetOps::balance(who, asset_b);
    AssetConversion::remove_liquidity(
      RuntimeOrigin::signed(who.clone()),
      Box::new(asset_a),
      Box::new(asset_b),
      lp_amount,
      0,
      0,
      who.clone(),
    )?;
    let after_a = TmctolAssetOps::balance(who, asset_a);
    let after_b = TmctolAssetOps::balance(who, asset_b);
    Ok((
      after_a.saturating_sub(before_a),
      after_b.saturating_sub(before_b),
    ))
  }
}

impl TmctolDexOps {
  const MAX_EXACT_OUT_QUOTE_STEPS: u32 = Balance::BITS;

  fn lp_balance(who: &AccountId, asset_a: AssetKind, asset_b: AssetKind) -> Balance {
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b).ok();
    let Some(pool_id) = pool_id else {
      return 0;
    };
    let Some(pool_info) = pallet_asset_conversion::Pools::<Runtime>::get(pool_id) else {
      return 0;
    };
    <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(
      pool_info.lp_token,
      who,
    )
  }

  fn pool_pair_for_lp(lp_token_id: u32) -> Option<(AssetKind, AssetKind)> {
    let mut scanned = 0u32;
    for (pool_key, pool_info) in pallet_asset_conversion::Pools::<Runtime>::iter() {
      if scanned >= <Runtime as pallet_aaa::Config>::MaxAdapterScan::get() {
        break;
      }
      scanned = scanned.saturating_add(1);
      if pool_info.lp_token == lp_token_id {
        return Some(pool_key);
      }
    }
    None
  }
}

/// System AAA genesis initializer for the current DEOS reference runtime.
///
/// Creates well-known System actors at genesis with deterministic `aaa_id` values
/// defined in `primitives::ecosystem::aaa_ids` (including sparse ranges).
/// The sovereign accounts are derived from `(AaaPalletId, "system", aaa_id)`
/// and can be computed offline for use in other configs.
pub struct TmctolGenesisSystemAaas;

impl TmctolGenesisSystemAaas {
  pub fn resolve_zap_slippage_tolerance(foreign: AssetKind) -> Perbill {
    let Some((native_reserve, _)) = AssetConversion::get_reserves(AssetKind::Native, foreign).ok()
    else {
      return ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE;
    };
    let min_parts = u128::from(ecosystem::params::LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE.deconstruct());
    let max_parts = u128::from(ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE.deconstruct());
    let reference_depth =
      ecosystem::params::LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE.max(1);
    let scaled_parts = max_parts
      .saturating_mul(reference_depth)
      .saturating_div(native_reserve.max(1));
    let clamped_parts = scaled_parts.clamp(min_parts, max_parts);
    Perbill::from_parts(clamped_parts as u32)
  }
}

impl
  pallet_aaa::GenesisSystemAaas<
    AccountId,
    pallet_aaa::ScheduleOf<Runtime>,
    pallet_aaa::ScheduleWindow<crate::BlockNumber>,
    pallet_aaa::ExecutionPlanOf<Runtime>,
  > for TmctolGenesisSystemAaas
{
  fn system_aaas() -> alloc::vec::Vec<(
    pallet_aaa::AaaId,
    AccountId,
    pallet_aaa::Mutability,
    pallet_aaa::ScheduleOf<Runtime>,
    Option<pallet_aaa::ScheduleWindow<crate::BlockNumber>>,
    pallet_aaa::ExecutionPlanOf<Runtime>,
  )> {
    use pallet_aaa::{Mutability, Schedule, Trigger};
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = AaaPalletId::get().into_account_truncating();

    // --- Burn Actor (aaa_id = 0; legacy constant: BURNING_MANAGER_AAA_ID) ---
    // Omnivorous intake: any verified inbound value signals one bounded pass that
    // swaps configured foreign balances to native and burns available native.
    let burn_schedule = Schedule {
      trigger: Trigger::OnAddressEvent {
        source_filter: pallet_aaa::SourceFilter::Any,
        asset_filter: pallet_aaa::AssetFilter::Any,
      },
      cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
    };
    let dust = ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    // Genesis execution_plan: swap known foreign assets → native, then burn.
    // Governance adds steps for new foreign assets via `update_execution_plan`.
    let burn_execution_plan: pallet_aaa::ExecutionPlanOf<Runtime> =
      Self::build_burn_execution_plan(alloc::vec![], dust);

    // --- Fee Sink (aaa_id = 1) ---
    // Inbound-driven Phase 1 fan-out: distributes accumulated native fees/rewards
    // into staking-pool yield and native LP-donation ingress channels.
    let fee_sink_schedule = Schedule {
      trigger: Trigger::OnAddressEvent {
        source_filter: pallet_aaa::SourceFilter::Any,
        asset_filter: pallet_aaa::AssetFilter::Any,
      },
      cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
    };
    let fee_sink_execution_plan: pallet_aaa::ExecutionPlanOf<Runtime> =
      Self::build_phase1_fee_sink_execution_plan();

    alloc::vec![
      (
        ecosystem::aaa_ids::BURNING_MANAGER_AAA_ID,
        governance.clone(),
        Mutability::Mutable,
        burn_schedule,
        None,
        burn_execution_plan,
      ),
      (
        ecosystem::aaa_ids::FEE_SINK_AAA_ID,
        governance.clone(),
        Mutability::Mutable,
        fee_sink_schedule,
        None,
        fee_sink_execution_plan,
      ),
      // --- BLDR Splitter (aaa_id = 10) ---
      // Receives 66% of TMC-minted $BLDR, splits 50/50 to BLDR liquidity + treasury lanes.
      (
        ecosystem::aaa_ids::BLDR_SPLITTER_AAA_ID,
        governance,
        Mutability::Mutable,
        Schedule {
          trigger: Trigger::OnAddressEvent {
            source_filter: pallet_aaa::SourceFilter::Any,
            asset_filter: pallet_aaa::AssetFilter::Any,
          },
          cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
        },
        None,
        Self::build_bldr_splitter_execution_plan(
          AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID),
          dust,
        ),
      ),
    ]
  }

  fn system_custody_accounts() -> alloc::vec::Vec<pallet_aaa::AaaId> {
    alloc::vec![
      ecosystem::aaa_ids::TOL_BUCKET_A_AAA_ID,
      ecosystem::aaa_ids::BLDR_BUCKET_A_AAA_ID,
    ]
  }

  fn dormant_system_aaas() -> alloc::vec::Vec<(pallet_aaa::AaaId, AccountId)> {
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = AaaPalletId::get().into_account_truncating();
    alloc::vec![
      ecosystem::aaa_ids::LIQUIDITY_ACTOR_AAA_ID,
      ecosystem::aaa_ids::TOL_BUCKET_B_AAA_ID,
      ecosystem::aaa_ids::TOL_BUCKET_C_AAA_ID,
      ecosystem::aaa_ids::TOL_BUCKET_D_AAA_ID,
      ecosystem::aaa_ids::TREASURY_B_AAA_ID,
      ecosystem::aaa_ids::TREASURY_C_AAA_ID,
      ecosystem::aaa_ids::TREASURY_D_AAA_ID,
      ecosystem::aaa_ids::BLDR_ZM_AAA_ID,
      ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID,
      ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
    ]
    .into_iter()
    .map(|aaa_id| (aaa_id, governance.clone()))
    .collect()
  }
}

impl TmctolGenesisSystemAaas {
  pub fn build_phase1_fee_sink_execution_plan() -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, SplitLeg, Step, StepErrorPolicy, Task};
    alloc::vec![Step {
      conditions: Default::default(),
      task: Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::AllBalance,
        legs: alloc::vec![
          SplitLeg {
            to: crate::Staking::pool_account_for(0),
            share: Perbill::from_percent(50),
          },
          SplitLeg {
            to: crate::AAA::sovereign_account_id_system(
              ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
            ),
            share: Perbill::from_percent(50),
          },
        ]
        .try_into()
        .expect("phase1 fee-sink split legs fit"),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("phase1 fee-sink execution_plan fits")
  }

  /// Builds the Burn Actor execution_plan: for each known foreign asset, add a
  /// conditional SwapExactIn step (skip if balance < dust), then a final Burn step.
  pub fn build_burn_execution_plan(
    foreign_assets: alloc::vec::Vec<AssetKind>,
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let dust_guard = |asset: AssetKind| -> Conditions {
      alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits")
    };
    let mut steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec::Vec::new();
    for foreign in foreign_assets {
      steps.push(Step {
        conditions: dust_guard(foreign),
        task: Task::SwapExactIn {
          asset_in: foreign,
          asset_out: AssetKind::Native,
          amount_in: AmountResolution::AllBalance,
          slippage_tolerance: ecosystem::params::SYSTEM_AAA_MAX_SWAP_SLIPPAGE,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      });
    }
    // Final step: burn all accumulated native (only if above dust)
    steps.push(Step {
      conditions: dust_guard(AssetKind::Native),
      task: Task::Burn {
        asset: AssetKind::Native,
        amount: AmountResolution::AllBalance,
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    steps
      .try_into()
      .expect("burn execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the Liquidity Actor execution_plan for a specific foreign asset / LP pair.
  ///
  /// Called by governance after pool creation, since LP asset IDs are
  /// pool-specific and unknown at genesis.
  ///
  /// ExecutionPlan steps:
  /// 1. If Native > dust AND Foreign > dust → AddLiquidity (opportunistic)
  /// 2. If Foreign > dust → SwapExactIn Foreign→Native with reserve-aware slippage
  /// 3. If LP > dust → SplitTransfer LP to TOL buckets (50/16.67/16.67/16.66)
  pub fn build_zap_execution_plan(
    foreign: AssetKind,
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, SplitLeg, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let dust_guard = |asset: AssetKind| -> Conditions {
      alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits")
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| -> Conditions {
      alloc::vec![
        Condition::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Condition::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ]
      .try_into()
      .expect("two conditions fit")
    };
    let slippage_tolerance = Self::resolve_zap_slippage_tolerance(foreign);
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![
      // Step 1: Opportunistic LP provisioning — add both sides at current pool ratio
      // AllBalance for native subtracts ED at resolution layer, safe with Preserve semantics
      Step {
        conditions: dual_dust_guard(AssetKind::Native, foreign),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: foreign,
          amount_a: AmountResolution::AllBalance,
          amount_b: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 2: Patriotic accumulation — convert leftover Foreign to Native
      Step {
        conditions: dust_guard(foreign),
        task: Task::SwapExactIn {
          asset_in: foreign,
          asset_out: AssetKind::Native,
          amount_in: AmountResolution::AllBalance,
          slippage_tolerance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 3: Distribute LP tokens to TOL buckets
      Step {
        conditions: dust_guard(lp_asset),
        task: Task::SplitTransfer {
          asset: lp_asset,
          amount: AmountResolution::AllBalance,
          legs: alloc::vec![
            SplitLeg {
              to: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::aaa_ids::TOL_BUCKET_A_AAA_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_A_ALLOCATION,
            },
            SplitLeg {
              to: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::aaa_ids::TOL_BUCKET_B_AAA_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_B_ALLOCATION,
            },
            SplitLeg {
              to: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::aaa_ids::TOL_BUCKET_C_AAA_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_C_ALLOCATION,
            },
            SplitLeg {
              to: pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::aaa_ids::TOL_BUCKET_D_AAA_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_D_ALLOCATION,
            },
          ]
          .try_into()
          .expect("4 bucket legs fit"),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("Liquidity Actor execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the Bucket-side half of production-admissible LP unwind.
  ///
  /// The Bucket transfers a bounded LP fraction into the paired Treasury sovereign.
  /// The Treasury then removes liquidity in its own independently admitted cycle.
  pub fn build_bucket_lp_transfer_execution_plan(
    lp_asset: AssetKind,
    dust_threshold: Balance,
    unwind_pct: polkadot_sdk::sp_runtime::Perbill,
    treasury_aaa_id: u64,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let treasury_account =
      pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(treasury_aaa_id);
    alloc::vec![Step {
      conditions: alloc::vec![Condition::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits"),
      task: Task::Transfer {
        to: treasury_account,
        asset: lp_asset,
        amount: AmountResolution::PercentageOfCurrent(unwind_pct),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("single-step Bucket LP transfer fits")
  }

  /// Builds the Treasury-side half of production-admissible LP unwind.
  ///
  /// Removing all preservable LP leaves both underlying assets in Treasury custody.
  pub fn build_treasury_lp_unwind_execution_plan(
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    alloc::vec![Step {
      conditions: alloc::vec![Condition::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits"),
      task: Task::RemoveLiquidity {
        lp_asset,
        amount: AmountResolution::AllBalance,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("single-step Treasury LP unwind fits")
  }

  /// Builds the BLDR Splitter execution_plan.
  ///
  /// Receives the minted $BLDR liquidity share from TMC output and splits it 50/50
  /// between BLDR liquidity and treasury lanes. TMC routes collateral directly to
  /// the BLDR Liquidity Actor.
  pub fn build_bldr_splitter_execution_plan(
    bldr_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, SplitLeg, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      pallet_aaa::Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let dust_guard = |asset: AssetKind| -> Conditions {
      alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits")
    };
    let bldr_zm_account = pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::aaa_ids::BLDR_ZM_AAA_ID,
    );
    let bldr_treasury_account = pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID,
    );
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![Step {
      conditions: dust_guard(bldr_asset),
      task: Task::SplitTransfer {
        asset: bldr_asset,
        amount: AmountResolution::AllBalance,
        legs: alloc::vec![
          SplitLeg {
            to: bldr_zm_account,
            share: ecosystem::params::BLDR_SPLITTER_ZM_SHARE,
          },
          SplitLeg {
            to: bldr_treasury_account,
            share: ecosystem::params::BLDR_SPLITTER_TREASURY_SHARE,
          },
        ]
        .try_into()
        .expect("2 legs fit"),
      },
      on_error: StepErrorPolicy::AbortCycle,
    },];
    steps
      .try_into()
      .expect("BLDR splitter execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the BLDR Liquidity Actor execution_plan for NTVE-BLDR provisioning.
  ///
  /// ExecutionPlan steps:
  /// 1. AddLiquidity(NTVE, BLDR) — opportunistic at current pool ratio
  /// 2. SplitTransfer(LP → BLDR Bucket A, 100%)
  pub fn build_bldr_zm_execution_plan(
    bldr_asset: AssetKind,
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      pallet_aaa::Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let dust_guard = |asset: AssetKind| -> Conditions {
      alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }]
      .try_into()
      .expect("single condition fits")
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| -> Conditions {
      alloc::vec![
        Condition::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Condition::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ]
      .try_into()
      .expect("two conditions fit")
    };
    let bldr_bucket_a = pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::aaa_ids::BLDR_BUCKET_A_AAA_ID,
    );
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![
      Step {
        conditions: dual_dust_guard(AssetKind::Native, bldr_asset),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: bldr_asset,
          amount_a: AmountResolution::AllBalance,
          amount_b: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      Step {
        conditions: dust_guard(lp_asset),
        task: Task::Transfer {
          to: bldr_bucket_a,
          asset: lp_asset,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("BLDR Liquidity Actor execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the Native Staking LP Farmer execution_plan.
  ///
  /// ExecutionPlan steps:
  /// 1. DonateLiquidity — stake the calculated NTVE side and donate balanced reserves
  pub fn activate_native_staking_lp_farming(
    dust_threshold: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Self::ensure_native_staking_lp_farming_ready()?;
    let execution_plan = Self::build_native_staking_lp_farming_execution_plan(dust_threshold);
    crate::AAA::activate_aaa(
      RuntimeOrigin::root(),
      ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
      pallet_aaa::ProgramInput::Active {
        schedule: pallet_aaa::Schedule {
          trigger: pallet_aaa::Trigger::OnAddressEvent {
            source_filter: pallet_aaa::SourceFilter::Any,
            asset_filter: pallet_aaa::AssetFilter::Any,
          },
          cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
        },
        schedule_window: None,
        execution_plan,
        on_close_execution_plan: Default::default(),
        funding_source_policy: pallet_aaa::FundingSourcePolicy::RuntimePolicy,
      },
    )
  }

  pub fn ensure_native_staking_lp_farming_ready() -> polkadot_sdk::sp_runtime::DispatchResult {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(
      staked_asset_id,
    ) {
      return Err(DispatchError::Other("StakedAssetUnavailable"));
    }
    pallet_staking::Pools::<Runtime>::get(native_asset_id)
      .ok_or(DispatchError::Other("NativeStakingPoolUnavailable"))?;
    let actor_id = ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID;
    if crate::AAA::aaa_instances(actor_id).is_none()
      && crate::AAA::dormant_aaa_identities(actor_id).is_none()
    {
      return Err(DispatchError::Other("NativeStakingLpFarmerUnavailable"));
    }
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    AssetConversion::get_reserves(base_asset, staked_asset)
      .map_err(|_| DispatchError::Other("NativeStakingAmmUnavailable"))?;
    Ok(())
  }

  pub fn build_native_staking_lp_farming_execution_plan(
    dust_threshold: Balance,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let native_staking_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let native_asset = AssetKind::Local(native_staking_asset_id);
    let staked_asset_id = crate::Staking::staked_asset_id(native_staking_asset_id)
      .expect("native staking LP farming activation checks staked asset first");
    let staked_asset = AssetKind::Local(staked_asset_id);
    let native_dust: Conditions = alloc::vec![Condition::BalanceAbove {
      asset: native_asset,
      threshold: dust_threshold,
    }]
    .try_into()
    .expect("single condition fits");
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![Step {
      conditions: native_dust,
      task: Task::DonateLiquidity {
        asset_a: native_asset,
        asset_b: staked_asset,
        amount: AmountResolution::AllBalance,
        max_ratio_error: ecosystem::params::NATIVE_STAKING_LP_DONATION_MAX_RATIO_ERROR,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }];
    steps
      .try_into()
      .expect("native staking LP farming execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the Treasury B BLDR buyback-and-burn execution_plan.
  ///
  /// ExecutionPlan steps:
  /// 1. SwapExactIn(NTVE → target) — amount resolved as % of current NTVE balance
  /// 2. Burn(target, AllBalance) — destroy all acquired tokens
  ///
  /// Multiple small buybacks per day create smooth market pressure.
  pub fn build_treasury_b_buyback_execution_plan(
    target_asset: AssetKind,
    buyback_pct: polkadot_sdk::sp_runtime::Perbill,
    dust_threshold: Balance,
    slippage: polkadot_sdk::sp_runtime::Perbill,
  ) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    type Conditions = polkadot_sdk::frame_support::BoundedVec<
      Condition<AssetKind, Balance>,
      <Runtime as pallet_aaa::Config>::MaxConditionsPerStep,
    >;
    let native_dust: Conditions = alloc::vec![Condition::BalanceAbove {
      asset: AssetKind::Native,
      threshold: dust_threshold,
    }]
    .try_into()
    .expect("single condition fits");
    let target_dust: Conditions = alloc::vec![Condition::BalanceAbove {
      asset: target_asset,
      threshold: dust_threshold,
    }]
    .try_into()
    .expect("single condition fits");
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![
      // Step 1: Swap NTVE → target (% of current balance)
      Step {
        conditions: native_dust,
        task: Task::SwapExactIn {
          asset_in: AssetKind::Native,
          asset_out: target_asset,
          amount_in: AmountResolution::PercentageOfCurrent(buyback_pct),
          slippage_tolerance: slippage,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      // Step 2: Burn all acquired target tokens
      Step {
        conditions: target_dust,
        task: Task::Burn {
          asset: target_asset,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("Treasury B buyback execution_plan fits within MaxSystemExecutionPlanSteps")
  }
}

pub struct TmctolStakingOps;
impl TmctolStakingOps {
  fn staking_asset_id(asset: AssetKind) -> u32 {
    match asset {
      AssetKind::Native => <Runtime as pallet_staking::Config>::NativeStakingAssetId::get(),
      AssetKind::Foreign(id) | AssetKind::Local(id) => id,
    }
  }
}

impl pallet_aaa::adapters::StakingOps<AccountId, AssetKind, Balance> for TmctolStakingOps {
  fn stake(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), DispatchError> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staking_asset_id = Self::staking_asset_id(asset);
    if staking_asset_id == native_asset_id {
      let _ = crate::Staking::stake_native(RuntimeOrigin::signed(who.clone()).into(), amount)?;
      return Ok(());
    }
    let _ = crate::Staking::stake(
      RuntimeOrigin::signed(who.clone()).into(),
      staking_asset_id,
      amount,
    )?;
    Ok(())
  }

  fn unstake(who: &AccountId, asset: AssetKind, shares: Balance) -> Result<(), DispatchError> {
    let _ = crate::Staking::unstake(
      RuntimeOrigin::signed(who.clone()).into(),
      Self::staking_asset_id(asset),
      shares,
    )?;
    Ok(())
  }

  fn share_balance(who: &AccountId, asset: AssetKind) -> Balance {
    crate::Staking::effective_share_balance_for_queries(Self::staking_asset_id(asset), who)
      .unwrap_or_default()
  }

  fn share_asset(asset: AssetKind) -> Option<AssetKind> {
    crate::Staking::staked_asset_id_for_queries(Self::staking_asset_id(asset)).map(AssetKind::Local)
  }
}

pub struct DeosFundingAuthority;

impl FundingAuthority<AccountId> for DeosFundingAuthority {
  fn allows(
    _: pallet_aaa::AaaId,
    _: &AccountId,
    _: &pallet_aaa::FundingProvenance<AccountId>,
  ) -> bool {
    // The reference launch line has no source/actor authorization entries.
    // Downstream runtimes must add explicit pairs rather than inheriting trust
    // from an account-shaped signed, internal-protocol, or XCM identity.
    false
  }
}

impl pallet_aaa::Config for Runtime {
  type PalletId = AaaPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetKind;
  type NativeAssetId = AaaNativeAssetId;
  type Balance = Balance;
  type AssetOps = TmctolAssetOps;
  type FundingAuthority = DeosFundingAuthority;
  type DexOps = TmctolDexOps;
  type StakingOps = TmctolStakingOps;
  type LiquidityDonationOps = TmctolLiquidityDonationOps;
  type AaaCreationFee = AaaCreationFee;
  type AddressEventIngressHook = RuntimeAddressEventIngressHook;
  type AtomicityHook = ();
  type ConditionReadFee = AaaConditionReadFee;
  type FeeSink = AaaFeeRecipient;
  type FeeCollector = TmctolFeeCollector;
  type GenesisSystemAaas = TmctolGenesisSystemAaas;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxActiveActors = AaaMaxActiveActors;
  type MaxActorIdentities = AaaMaxActiveActors;
  type MaxAdapterScan = AaaMaxPoolScan;
  type MaxConditionsPerStep = AaaMaxConditionsPerStep;
  type MaxConsecutiveFailures = AaaMaxConsecutiveFailures;
  type MaxAutoCloseNonceHorizon = AaaMaxAutoCloseNonceHorizon;
  type MaxExecutionDelayBlocks = AaaMaxExecutionDelayBlocks;
  type MaxTimerJitterBlocks = AaaMaxTimerJitterBlocks;
  type MaxExecutionsPerBlock = AaaMaxExecutionsPerBlock;
  type MaxQueueLength = AaaMaxQueueLength;
  type QueuePageSize = AaaQueuePageSize;
  type MaxQueueEntriesScannedPerBlock = AaaMaxQueueEntriesScannedPerBlock;
  type MaxWakeupBucketSize = AaaMaxWakeupBucketSize;
  type MaxWakeupsPerBlock = AaaMaxWakeupsPerBlock;
  type MaxSpilloverBlocks = AaaMaxSpilloverBlocks;
  type MaxFundingTrackedAssets = AaaMaxFundingTrackedAssets;
  type MaxIdleStarvationBlocks = AaaMaxIdleStarvationBlocks;
  type GuaranteedOnIdleWeight = AaaGuaranteedOnIdleWeight;
  type MaxIngressOverflowQueue = AaaMaxIngressOverflowQueue;
  type MaxOwnerSlots = AaaMaxOwnerSlots;
  type MaxExecutionPlanSteps = AaaMaxExecutionPlanSteps;
  type MaxSplitTransferLegs = AaaMaxSplitTransferLegs;
  type MaxSweepPerBlock = AaaMaxSweepPerBlock;
  type MaxSystemExecutionPlanSteps = AaaMaxSystemExecutionPlanSteps;
  type MaxUserExecutionPlanSteps = AaaMaxUserExecutionPlanSteps;
  type MaxWhitelistSize = AaaMaxWhitelistSize;
  type MinUserBalance = AaaMinUserBalanceGuard;
  type MinWindowLength = AaaMinWindowLength;
  type StepBaseFee = AaaStepBaseFee;
  type WeightInfo = crate::weights::pallet_aaa::SubstrateWeight<Runtime>;
  type WeightToFee = crate::WeightToFee;
  // Runtime binds task upper bounds so fee admission stays chain-specific and auditable
  type TaskWeightInfo = pallet_aaa::weights::SubstrateTaskWeightInfo<Runtime>;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeAaaBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeAaaBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl RuntimeAaaBenchmarkHelper {
  fn ensure_local_asset(asset_id: u32, owner: &AccountId) -> Result<(), DispatchError> {
    if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(asset_id) {
      pallet_assets::Pallet::<Runtime>::force_create(
        RuntimeOrigin::root(),
        asset_id,
        polkadot_sdk::sp_runtime::MultiAddress::Id(owner.clone()),
        true,
        1,
      )?;
    }
    Ok(())
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_aaa::BenchmarkHelper<AccountId, AssetKind, Balance> for RuntimeAaaBenchmarkHelper {
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance, Balance), DispatchError> {
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    let local_asset_id = 300_000;
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(local_asset_id);
    Self::ensure_local_asset(local_asset_id, owner)?;
    let amount: Balance = 1_000_000_000_000;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(owner, amount.saturating_mul(2));
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      local_asset_id,
      owner,
      amount.saturating_add(1),
    )?;
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
    if pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id) {
      return Err(DispatchError::Other("AddLiquidityPoolAlreadyExists"));
    }
    Ok((asset_a, asset_b, amount, amount))
  }

  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance), DispatchError> {
    let asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    Self::ensure_local_asset(asset_id, owner)?;
    let liquidity: Balance = 1_000_000_000;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      owner,
      EXISTENTIAL_DEPOSIT.saturating_mul(100),
    );
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      asset_id,
      owner,
      liquidity.saturating_mul(3),
    )?;
    if !pallet_staking::Pools::<Runtime>::contains_key(asset_id) {
      crate::Staking::register_staking_asset(RuntimeOrigin::root(), asset_id)?;
    }
    crate::Staking::stake_native(RuntimeOrigin::signed(owner.clone()), liquidity)?;
    let staked_asset_id = crate::Staking::staked_asset_id(asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    let asset_a = AssetKind::Local(asset_id);
    let asset_b = AssetKind::Local(staked_asset_id);
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    AssetConversion::create_pool(
      RuntimeOrigin::signed(owner.clone()),
      alloc::boxed::Box::new(asset_a),
      alloc::boxed::Box::new(asset_b),
    )?;
    let pool_id =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
        .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
    let pool_info = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .ok_or(DispatchError::Other("PoolNotCreated"))?;
    if <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
      u32,
      AccountId,
    >>::should_touch(pool_info.lp_token, owner)
    {
      <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
        u32,
        AccountId,
      >>::touch(pool_info.lp_token, owner, owner)?;
    }
    AssetConversion::add_liquidity(
      RuntimeOrigin::signed(owner.clone()),
      alloc::boxed::Box::new(asset_a),
      alloc::boxed::Box::new(asset_b),
      liquidity / 2,
      liquidity / 2,
      0,
      0,
      owner.clone(),
    )?;
    Ok((asset_a, asset_b, liquidity / 10))
  }

  fn setup_remove_liquidity_max_k(
    owner: &AccountId,
    max_scan: u32,
  ) -> Result<(AssetKind, Balance), DispatchError> {
    if max_scan == 0 {
      return Err(DispatchError::Other("MaxAdapterScanZero"));
    }
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    let liquidity = 1_000_000_000_000u128;
    let native_seed = liquidity.saturating_mul(max_scan.saturating_add(1) as u128);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(owner, native_seed);
    let mut target_lp: Option<(AssetKind, Balance)> = None;
    for i in 0..max_scan {
      let local_asset_id = 100_000u32.saturating_add(i);
      if Self::ensure_local_asset(local_asset_id, owner).is_err() {
        return Err(DispatchError::Other("EnsureLocalAssetFailed"));
      }
      if <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
        local_asset_id,
        owner,
        liquidity.saturating_add(1),
      )
      .is_err()
      {
        return Err(DispatchError::Other("MintLocalForBenchmarkFailed"));
      }
      let asset_a = AssetKind::Native;
      let asset_b = AssetKind::Local(local_asset_id);
      if AssetConversion::create_pool(
        RuntimeOrigin::signed(owner.clone()),
        alloc::boxed::Box::new(asset_a),
        alloc::boxed::Box::new(asset_b),
      )
      .is_err()
      {
        return Err(DispatchError::Other("CreatePoolForBenchmarkFailed"));
      }
      let pool_account =
        <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(&asset_a, &asset_b)
          .map_err(|_| DispatchError::Other("PoolAddressUnavailable"))?;
      let _ =
        <Balances as Currency<AccountId>>::deposit_creating(&pool_account, EXISTENTIAL_DEPOSIT);
      let pool_id =
        <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b)
          .map_err(|_| DispatchError::Other("PoolIdUnavailable"))?;
      let pool_info = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
        .ok_or(DispatchError::Other("PoolNotCreated"))?;
      if <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
        u32,
        AccountId,
      >>::should_touch(pool_info.lp_token, owner)
        && <pallet_assets::Pallet<Runtime> as polkadot_sdk::frame_support::traits::AccountTouch<
          u32,
          AccountId,
        >>::touch(pool_info.lp_token, owner, owner)
        .is_err()
      {
        return Err(DispatchError::Other("TouchLpAccountForBenchmarkFailed"));
      }
      if AssetConversion::add_liquidity(
        RuntimeOrigin::signed(owner.clone()),
        alloc::boxed::Box::new(asset_a),
        alloc::boxed::Box::new(asset_b),
        liquidity,
        liquidity,
        0,
        0,
        owner.clone(),
      )
      .is_err()
      {
        return Err(DispatchError::Other("AddLiquidityForBenchmarkFailed"));
      }
      if i.saturating_add(1) == max_scan {
        let lp_amount = <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(
          pool_info.lp_token,
          owner,
        );
        let min_native_reserve = <Balances as NativeInspect<AccountId>>::minimum_balance();
        let benchmark_lp_amount = lp_amount.saturating_sub(min_native_reserve);
        if benchmark_lp_amount == 0 {
          return Err(DispatchError::Other("LpAmountTooSmallForBenchmark"));
        }
        target_lp = Some((AssetKind::Local(pool_info.lp_token), benchmark_lp_amount));
      }
    }
    target_lp.ok_or(DispatchError::Other("TargetLpMissing"))
  }

  fn setup_stake(owner: &AccountId) -> Result<(AssetKind, Balance), DispatchError> {
    let asset_id = 200_000;
    let amount: Balance = 1_000_000;
    Self::ensure_local_asset(asset_id, owner)?;
    <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
      asset_id,
      owner,
      amount.saturating_add(1),
    )?;
    crate::Staking::register_staking_asset(RuntimeOrigin::root(), asset_id)?;
    Ok((AssetKind::Local(asset_id), amount))
  }

  fn setup_unstake(owner: &AccountId) -> Result<(AssetKind, Balance), DispatchError> {
    let (asset, amount) = Self::setup_stake(owner)?;
    <TmctolStakingOps as pallet_aaa::adapters::StakingOps<AccountId, AssetKind, Balance>>::stake(
      owner, asset, amount,
    )?;
    let shares = <TmctolStakingOps as pallet_aaa::adapters::StakingOps<
      AccountId,
      AssetKind,
      Balance,
    >>::share_balance(owner, asset);
    if shares == 0 {
      return Err(DispatchError::Other("UnstakeSharesMissing"));
    }
    Ok((asset, shares))
  }

  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance), DispatchError> {
    let _ = Self::setup_remove_liquidity_max_k(owner, 1)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurningManagerAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    Ok((AssetKind::Native, AssetKind::Local(100_000), 1_000_000))
  }

  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance, Balance), DispatchError> {
    let _ = Self::setup_remove_liquidity_max_k(owner, 1)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurningManagerAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    Ok((
      AssetKind::Native,
      AssetKind::Local(100_000),
      100_000,
      1_000_000_000,
    ))
  }

  fn funding_assets(max: u32) -> alloc::vec::Vec<AssetKind> {
    (0..max)
      .map(|index| {
        if index == 0 {
          AssetKind::Native
        } else {
          AssetKind::Local(index)
        }
      })
      .collect()
  }

  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    let transferred = amount.max(EXISTENTIAL_DEPOSIT);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      source,
      transferred.saturating_add(EXISTENTIAL_DEPOSIT),
    );
    System::reset_events();
    Balances::transfer_allow_death(
      RuntimeOrigin::signed(source.clone()),
      polkadot_sdk::sp_runtime::MultiAddress::Id(recipient.clone()),
      transferred,
    )
  }

  fn run_address_event_ingress(_recipient: &AccountId) -> bool {
    crate::configs::address_event_ingress::RuntimeAddressEventIngressHook::submit_events_since_with_verified_source(0, true)
      .expect("benchmark ingress notification must succeed")
  }

  fn setup_xcm_asset_deposit() -> DispatchResult {
    crate::configs::xcm_config::setup_benchmark_foreign_asset()
  }

  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    crate::configs::xcm_config::benchmark_foreign_asset_deposit(recipient, source, amount)
  }

  fn clear_address_event_ingress_events() {
    System::reset_events();
  }

  fn run_compatibility_address_event_ingress() -> Weight {
    <crate::configs::address_event_ingress::RuntimeAddressEventIngressHook as pallet_aaa::AddressEventIngressHook<BlockNumber>>::ingest(
      System::block_number(),
      Weight::MAX,
    )
  }
}
