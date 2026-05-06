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
use pallet_aaa::{AssetOps, DexOps, FeeRouter, LiquidityDonationOps};

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

  pub const AaaMaxExecutionsPerBlock: u32 = 48;
  pub const AaaMaxQueueLength: u32 = 10_000;
  pub const AaaMaxWakeupBucketSize: u32 = 10_000;
  pub const AaaMaxWakeupsPerBlock: u32 = 512;
  pub const AaaMaxQueueInsertionsPerBlock: u32 = 512;
  pub const AaaFairnessWeightSystem: u32 = 1;
  pub const AaaFairnessWeightUser: u32 = 3;
  pub const AaaMaxIngressEventsPerBlock: u32 = 1024;
  pub const AaaMaxIngressScanEventsPerBlock: u32 = 4096;
  pub const AaaMaxIngressOverflowQueue: u32 = 8192;

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
  /// Current simplified policy: probabilistic timers may fall back to deterministic previous-block
  /// hash sampling while the runtime stays on a trusted collator set. A relay-beacon replacement
  /// is only acceptable if a future parachain-consumable per-block protocol beacon exists;
  /// existing epoch-scale relay randomness items are not treated as that replacement. Secure
  /// external entropy remains a future upgrade, not a launch requirement.
  pub const AaaRequireSecureEntropyForProbabilisticTasks: bool = false;

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

/// Fee sink — canonical unified fee-collection address.
///
/// AAA evaluation and execution fees route here today. The address stays derived from
/// `aaa_id = FEE_SINK_AAA_ID` so the fee collector remains stable while System AAA #1
/// owns the Phase 1 redistribution execution plan. The broader economic contract is
/// a unified 20% collator / 80% Fee Sink collection rule, followed by phase-aware Fee Sink
/// redistribution into staking-pool yield, native LP donation, and later claimable
/// LP-nomination rewards.
pub struct TmctolFeeRouter;
impl FeeRouter<AccountId, AssetKind, Balance> for TmctolFeeRouter {
  fn route_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetKind,
    amount: Balance,
  ) -> DispatchResult {
    if amount == 0 {
      return Ok(());
    }
    let Some(author) = Authorship::author() else {
      return TmctolAssetOps::transfer(payer, fee_sink, native_asset, amount);
    };
    let author_share = Perbill::from_percent(20) * amount;
    let fee_sink_share = amount.saturating_sub(author_share);
    if author_share > 0 {
      TmctolAssetOps::transfer(payer, &author, native_asset, author_share)?;
    }
    if fee_sink_share > 0 {
      TmctolAssetOps::transfer(payer, fee_sink, native_asset, fee_sink_share)?;
    }
    Ok(())
  }
}

pub struct AaaFeeRecipient;
impl Get<crate::AccountId> for AaaFeeRecipient {
  fn get() -> crate::AccountId {
    crate::AAA::sovereign_account_id_system(ecosystem::aaa_ids::FEE_SINK_AAA_ID)
  }
}

pub struct TmctolAssetOps;

impl AssetOps<AccountId, AssetKind, Balance> for TmctolAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), DispatchError> {
    match asset {
      AssetKind::Native => {
        <Balances as Currency<AccountId>>::transfer(
          from,
          to,
          amount,
          polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
        )?;
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
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_with_source(
      to, asset, amount, from,
    );
    Ok(())
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
    );
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

  fn total_issuance(asset: AssetKind) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::total_issuance(),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::total_issuance(id)
      }
    }
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
      return crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
        who,
        amount,
        max_ratio_error,
      );
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
    let quote =
      pallet_axial_router::Pallet::<Runtime>::quote_price(asset_in, asset_out, amount_in)?;
    let min_out = (polkadot_sdk::sp_runtime::Perbill::one() - slippage_tolerance).mul_floor(quote);
    pallet_axial_router::Pallet::<Runtime>::execute_swap_for(
      who, asset_in, asset_out, amount_in, min_out, who,
    )
  }

  fn swap_exact_out(
    who: &AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<Balance, DispatchError> {
    if amount_out == 0 {
      return Err(DispatchError::Other("ZeroAmountOut"));
    }
    let quote_output = |amount_in: Balance| -> Result<Balance, DispatchError> {
      if amount_in == 0 {
        return Err(DispatchError::Other("ZeroAmountIn"));
      }
      let fee = if pallet_axial_router::Pallet::<Runtime>::is_fee_exempt(who) {
        0
      } else {
        pallet_axial_router::Pallet::<Runtime>::calculate_router_fee(amount_in)
      };
      let net_in = amount_in.saturating_sub(fee);
      if net_in == 0 {
        return Err(DispatchError::Other("AmountInBelowRouterFee"));
      }
      pallet_axial_router::Pallet::<Runtime>::quote_price(asset_in, asset_out, net_in)
    };
    let mut high: Balance = 1;
    let mut found = false;
    for _ in 0..128 {
      match quote_output(high) {
        Ok(quoted) if quoted >= amount_out => {
          found = true;
          break;
        }
        _ => {
          high = high
            .checked_mul(2)
            .ok_or(DispatchError::Other("AmountInOverflow"))?;
        }
      }
    }
    if !found {
      return Err(DispatchError::Other("UnableToQuoteExactOut"));
    }
    let mut low: Balance = 1;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match quote_output(mid) {
        Ok(quoted) if quoted >= amount_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let _ = slippage_tolerance;
    let required_in = high;
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

  fn get_pool_reserves(asset_a: AssetKind, asset_b: AssetKind) -> Option<(Balance, Balance)> {
    AssetConversion::get_reserves(asset_a, asset_b).ok()
  }
}

impl TmctolDexOps {
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
    let Some((native_reserve, _)) = TmctolDexOps::get_pool_reserves(AssetKind::Native, foreign)
    else {
      return ecosystem::params::ZAP_MANAGER_MAX_SWAP_SLIPPAGE;
    };
    let min_parts = u128::from(ecosystem::params::ZAP_MANAGER_MIN_SWAP_SLIPPAGE.deconstruct());
    let max_parts = u128::from(ecosystem::params::ZAP_MANAGER_MAX_SWAP_SLIPPAGE.deconstruct());
    let reference_depth = ecosystem::params::ZAP_MANAGER_SLIPPAGE_REFERENCE_NATIVE_RESERVE.max(1);
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
    pallet_aaa::ScheduleOf<Runtime>,
    Option<pallet_aaa::ScheduleWindow<crate::BlockNumber>>,
    pallet_aaa::ExecutionPlanOf<Runtime>,
  )> {
    use pallet_aaa::{Schedule, Step, StepErrorPolicy, Task, Trigger};
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = AaaPalletId::get().into_account_truncating();

    // --- Burning Manager (aaa_id = 0) ---
    // Timer-driven: polls every N blocks, swaps any accumulated foreign tokens
    // to native, then burns all native. No explicit coupling with fee source —
    // it just processes whatever balance is on its sovereign account.
    let burn_schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: ecosystem::params::BURNING_MANAGER_POLL_BLOCKS,
        probability: None,
      },
      cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
    };
    let dust = ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    // Genesis execution_plan: swap known foreign assets → native, then burn.
    // Governance adds steps for new foreign assets via `update_execution_plan`.
    let burn_execution_plan: pallet_aaa::ExecutionPlanOf<Runtime> =
      Self::build_burn_execution_plan(alloc::vec![], dust);

    // --- Fee Sink (aaa_id = 1) ---
    // Timer-driven Phase 1 fan-out: distributes accumulated native fees/rewards
    // into staking-pool yield and native LP-donation ingress channels.
    let fee_sink_schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 1,
        probability: None,
      },
      cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
    };
    let fee_sink_execution_plan: pallet_aaa::ExecutionPlanOf<Runtime> =
      Self::build_phase1_fee_sink_execution_plan();

    // --- Zap Manager (aaa_id = 2) ---
    // Timer-driven skeleton; real LP provisioning steps are added by governance
    // after TMC curves and pools are created (LP token IDs are pool-specific).
    let zap_schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 1,
        probability: None,
      },
      cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
    };
    let zap_execution_plan: pallet_aaa::ExecutionPlanOf<Runtime> = alloc::vec![Step {
      conditions: Default::default(),
      task: Task::Noop,
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("zap execution_plan fits");

    alloc::vec![
      (
        ecosystem::aaa_ids::BURNING_MANAGER_AAA_ID,
        governance.clone(),
        burn_schedule,
        None,
        burn_execution_plan,
      ),
      (
        ecosystem::aaa_ids::FEE_SINK_AAA_ID,
        governance.clone(),
        fee_sink_schedule,
        None,
        fee_sink_execution_plan,
      ),
      (
        ecosystem::aaa_ids::ZAP_MANAGER_AAA_ID,
        governance.clone(),
        zap_schedule,
        None,
        zap_execution_plan,
      ),
      // --- TOL Bucket A: Anchor (aaa_id = 3) ---
      (
        ecosystem::aaa_ids::TOL_BUCKET_A_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- TOL Bucket B: Building (aaa_id = 4) ---
      (
        ecosystem::aaa_ids::TOL_BUCKET_B_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- TOL Bucket C: Capital (aaa_id = 5) ---
      (
        ecosystem::aaa_ids::TOL_BUCKET_C_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- TOL Bucket D: Dormant (aaa_id = 6) ---
      (
        ecosystem::aaa_ids::TOL_BUCKET_D_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- Treasury B: Building Treasury (aaa_id = 7) ---
      // Receives Native + Foreign from Bucket B unwind; Noop until governance activates spending
      (
        ecosystem::aaa_ids::TREASURY_B_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- Treasury C: Capital Treasury (aaa_id = 8) ---
      // Receives Native + Foreign from Bucket C unwind; Noop until governance activates spending
      (
        ecosystem::aaa_ids::TREASURY_C_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- Treasury D: Dormant Treasury (aaa_id = 9) ---
      // Receives Native + Foreign from Bucket D unwind; Noop until governance activates spending
      (
        ecosystem::aaa_ids::TREASURY_D_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- BLDR Splitter (aaa_id = 10) ---
      // Receives 66% of TMC-minted $BLDR, splits 50/50 to BLDR ZM + BLDR Treasury
      (
        ecosystem::aaa_ids::BLDR_SPLITTER_AAA_ID,
        governance.clone(),
        Schedule {
          trigger: Trigger::Timer {
            every_blocks: 1,
            probability: None,
          },
          cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
        },
        None,
        Self::build_bldr_splitter_execution_plan(
          AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID),
          dust,
        ),
      ),
      // --- BLDR Zap Manager (aaa_id = 11) ---
      // Timer-driven skeleton; LP provisioning steps added by governance
      // after NTVE-BLDR pool is created (LP token ID is pool-specific).
      // DexOps::add_liquidity auto-creates the pool on first execution.
      (
        ecosystem::aaa_ids::BLDR_ZM_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- BLDR Bucket A (aaa_id = 12) ---
      // Permanent LP accumulator for NTVE-BLDR pair
      (
        ecosystem::aaa_ids::BLDR_BUCKET_A_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- BLDR Treasury (aaa_id = 13) ---
      // Receives 50% of minted $BLDR from Splitter; Noop until governance activates spending
      (
        ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID,
        governance.clone(),
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
      // --- Native Staking LP Farmer (aaa_id = 14) ---
      // Timer-driven skeleton; governance activates donation after the NTVE/stNTVE pool exists.
      (
        ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
        governance,
        noop_timer_schedule(),
        None,
        noop_execution_plan(),
      ),
    ]
  }
}

fn noop_timer_schedule() -> pallet_aaa::ScheduleOf<Runtime> {
  pallet_aaa::Schedule {
    trigger: pallet_aaa::Trigger::Timer {
      every_blocks: 1,
      probability: None,
    },
    cooldown_blocks: ecosystem::params::SYSTEM_AAA_COOLDOWN_BLOCKS,
  }
}

fn noop_execution_plan() -> pallet_aaa::ExecutionPlanOf<Runtime> {
  use pallet_aaa::{Step, StepErrorPolicy, Task};
  alloc::vec![Step {
    conditions: Default::default(),
    task: Task::Noop,
    on_error: StepErrorPolicy::AbortCycle,
  }]
  .try_into()
  .expect("noop execution_plan fits")
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
            to: crate::Staking::lp_reward_account_for(0),
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

  pub fn build_phase2_fee_sink_execution_plan() -> pallet_aaa::ExecutionPlanOf<Runtime> {
    use pallet_aaa::{AmountResolution, SplitLeg, Step, StepErrorPolicy, Task};
    // Phase 2 1:1:4 fee-sink distribution.
    // Not activated at Phase 1 launch; reserved for a later runtime-upgrade boundary
    // when permissionless collators and GovXP-weighted LP-nomination rewards ship.
    alloc::vec![Step {
      conditions: Default::default(),
      task: Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::AllBalance,
        legs: alloc::vec![
          SplitLeg {
            to: crate::Staking::pool_account_for(0),
            share: Perbill::from_parts(166_666_667),
          },
          SplitLeg {
            to: crate::Staking::lp_reward_account_for(0),
            share: Perbill::from_parts(166_666_667),
          },
          SplitLeg {
            to: crate::Staking::reward_account_for(0),
            share: Perbill::from_parts(666_666_666),
          },
        ]
        .try_into()
        .expect("phase2 fee-sink split legs fit"),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("phase2 fee-sink execution_plan fits")
  }

  /// Builds the Burning Manager execution_plan: for each known foreign asset, add a
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

  /// Builds the Zap Manager execution_plan for a specific foreign asset / LP pair.
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
      .expect("zap execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds a gradual LP unwind execution_plan for a TOL Bucket (B, C, or D).
  ///
  /// Each cycle removes a small percentage of LP, then transfers the resulting
  /// Native and Foreign tokens to the paired Treasury AAA sovereign account.
  ///
  /// Called by governance after pool creation and Treasury AAA activation.
  ///
  /// ExecutionPlan steps:
  /// 1. If LP > dust → RemoveLiquidity (percentage of current LP holdings)
  /// 2. If Native > dust → Transfer Native to Treasury sovereign
  /// 3. If Foreign > dust → Transfer Foreign to Treasury sovereign
  pub fn build_bucket_unwind_execution_plan(
    lp_asset: AssetKind,
    foreign: AssetKind,
    dust_threshold: Balance,
    unwind_pct: polkadot_sdk::sp_runtime::Perbill,
    treasury_aaa_id: u64,
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
    let treasury_account =
      pallet_aaa::Pallet::<Runtime>::sovereign_account_id_system(treasury_aaa_id);
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![
      // Step 1: Remove a small fraction of LP holdings
      Step {
        conditions: dust_guard(lp_asset),
        task: Task::RemoveLiquidity {
          lp_asset,
          amount: AmountResolution::PercentageOfCurrent(unwind_pct),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      // Step 2: Send reclaimed Native to paired Treasury
      Step {
        conditions: dust_guard(AssetKind::Native),
        task: Task::Transfer {
          to: treasury_account.clone(),
          asset: AssetKind::Native,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 3: Send reclaimed Foreign to paired Treasury
      Step {
        conditions: dust_guard(foreign),
        task: Task::Transfer {
          to: treasury_account,
          asset: foreign,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ];
    steps
      .try_into()
      .expect("bucket unwind execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the BLDR Splitter execution_plan.
  ///
  /// Receives both $NTVE (collateral) and $BLDR (minted zap share) from TMC output:
  /// 1. Transfer 100% NTVE → BLDR ZM
  /// 2. SplitTransfer BLDR 50/50 → BLDR ZM + BLDR Treasury
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
    let steps: alloc::vec::Vec<pallet_aaa::StepOf<Runtime>> = alloc::vec![
      // Step 1: Forward all NTVE collateral to BLDR ZM
      Step {
        conditions: dust_guard(AssetKind::Native),
        task: Task::Transfer {
          to: bldr_zm_account.clone(),
          asset: AssetKind::Native,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 2: Split BLDR 50/50 to BLDR ZM + BLDR Treasury
      Step {
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
      },
    ];
    steps
      .try_into()
      .expect("BLDR splitter execution_plan fits within MaxSystemExecutionPlanSteps")
  }

  /// Builds the BLDR ZM execution_plan for NTVE-BLDR liquidity provisioning.
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
      .expect("BLDR ZM execution_plan fits within MaxSystemExecutionPlanSteps")
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
    crate::AAA::update_execution_plan(
      RuntimeOrigin::root(),
      ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
      execution_plan,
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
    if crate::AAA::aaa_instances(ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID).is_none() {
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
impl pallet_aaa::adapters::StakingOps<AccountId, AssetKind, Balance> for TmctolStakingOps {
  fn stake(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), DispatchError> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staking_asset_id = match asset {
      primitives::AssetKind::Native => native_asset_id,
      primitives::AssetKind::Foreign(id) => id,
      primitives::AssetKind::Local(id) => id,
    };
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
    let staking_asset_id = match asset {
      primitives::AssetKind::Native => 0,
      primitives::AssetKind::Foreign(id) => id,
      primitives::AssetKind::Local(id) => id,
    };
    let _ = crate::Staking::unstake(
      RuntimeOrigin::signed(who.clone()).into(),
      staking_asset_id,
      shares,
    )?;
    Ok(())
  }
}

impl pallet_aaa::Config for Runtime {
  type PalletId = AaaPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetKind;
  type NativeAssetId = AaaNativeAssetId;
  type Balance = Balance;
  type AssetOps = TmctolAssetOps;
  type DexOps = TmctolDexOps;
  type StakingOps = TmctolStakingOps;
  type LiquidityDonationOps = TmctolLiquidityDonationOps;
  type AaaCreationFee = AaaCreationFee;
  type AddressEventIngressHook = RuntimeAddressEventIngressHook;
  type AtomicityHook = ();
  type ConditionReadFee = AaaConditionReadFee;
  type EntropyProvider = pallet_aaa::NoEntropyProvider;
  type FairnessWeightSystem = AaaFairnessWeightSystem;
  type FairnessWeightUser = AaaFairnessWeightUser;
  type FeeSink = AaaFeeRecipient;
  type FeeRouter = TmctolFeeRouter;
  type GenesisSystemAaas = TmctolGenesisSystemAaas;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxActiveActors = AaaMaxActiveActors;
  type MaxAdapterScan = AaaMaxPoolScan;
  type MaxConditionsPerStep = AaaMaxConditionsPerStep;
  type MaxConsecutiveFailures = AaaMaxConsecutiveFailures;
  type MaxAutoCloseNonceHorizon = AaaMaxAutoCloseNonceHorizon;
  type MaxExecutionDelayBlocks = AaaMaxExecutionDelayBlocks;
  type MaxTimerJitterBlocks = AaaMaxTimerJitterBlocks;
  type MaxExecutionsPerBlock = AaaMaxExecutionsPerBlock;
  type MaxQueueLength = AaaMaxQueueLength;
  type MaxWakeupBucketSize = AaaMaxWakeupBucketSize;
  type MaxWakeupsPerBlock = AaaMaxWakeupsPerBlock;
  type MaxQueueInsertionsPerBlock = AaaMaxQueueInsertionsPerBlock;
  type MaxFundingTrackedAssets = AaaMaxFundingTrackedAssets;
  type MaxIdleStarvationBlocks = AaaMaxIdleStarvationBlocks;
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
  type RequireSecureEntropyForProbabilisticTasks = AaaRequireSecureEntropyForProbabilisticTasks;
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
}
