//! Actors pallet configuration for the DEOS reference runtime.
//!
//! Wires the two adapter traits (`AssetOps`, `DexOps`) to concrete runtime pallets:
//! - Native token: `pallet-balances`
//! - Foreign assets: `pallet-assets`
//! - Swaps: DEOS Router
//! - Liquidity: Asset Conversion

use super::*;
use primitives::{AssetKind, ecosystem};

use polkadot_sdk::frame_support::traits::{
  Currency, Get,
  fungible::Inspect as NativeInspect,
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
  tokens::{DepositConsequence, Fortitude, Precision, Preservation, Provenance},
};
use polkadot_sdk::pallet_asset_conversion::PoolLocator;
use polkadot_sdk::sp_core::U256;
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult, Perbill, TokenError};

use crate::{AssetConversion, RuntimeOrigin};
use pallet_deos_actors::{
  ActorType, AssetOps, DexOps, DexSwapOutcome, ExecutionContext, FeeCollector, FundingAuthority,
  LiquidityOps, TaskFailure,
};

parameter_types! {
  // --- Identity and ownership ---

  pub const ActorsPalletId: PalletId = PalletId(*ecosystem::pallet_ids::ACTORS_PALLET_ID);
  pub const ActorFeeNativeAssetId: AssetKind = AssetKind::Native;
  /// User Actors slot capacity per owner; System Actors is not constrained by this limit
  pub const ActorMaxOwnerSlots: u8 = 255;

  // --- Execution-plan and task bounds ---

  pub const ActorMaxExecutionPlanSteps: u32 = 8;
  pub const ActorMaxFundingTrackedAssets: u32 = 10;
  pub const ActorMaxOpeningSnapshotEntries: u32 = 16;
  pub const ActorMaxConditionsPerStep: u32 = 4;
  pub const ActorMaxSplitTransferLegs: u32 = 8;

  // --- Trigger and schedule bounds ---

  pub const ActorTargetBlockTime: u64 = 6;
pub const ActorMaxExecutionDelayBlocks: BlockNumber = 52_596_000;
  pub const ActorMaxTimerJitterBlocks: u32 = 64;
  pub const ActorMinWindowLength: BlockNumber = 100;
  pub const ActorMaxWhitelistSize: u32 = 16;
  pub const ActorMaxTriggerSources: u32 = 4;

  // --- Scheduler controls ---

  /// Defense-in-depth count ceiling; RefTime and ProofSize admission remain primary.
  pub const ActorMaxExecutionsPerBlock: u32 = 1_000;
  pub const ActorMaxQueueLength: u32 = 10_000;
  /// Balanced production granularity selected from 32/64/128 production-Wasm evidence.
  pub const ActorQueuePageSize: u32 = 64;
  /// Production temporal page granularity selected from 32/64/128 Wasm operation evidence.
  pub const ActorWakeupPageSize: u32 = 32;
  /// Independent observation subscriber/fanout page granularity.
  pub const ActorObservationPageSize: u32 = 64;
  pub const ActorMaxQueueEntriesScannedPerBlock: u32 = 10_000;
  pub const ActorMaxObservationFanoutPagesPerBlock: u32 = 64;
  pub const ActorMaxWakeupsPerBlock: u32 = 512;
  pub ActorObservationFanoutWeightLimit: Weight =
    Perbill::from_percent(20) * MAXIMUM_BLOCK_WEIGHT;
  /// Dedicated overdue-wakeup worker envelope: one worst-case complete wakeup unit plus cursor
  /// probe remains inside it (spec 15.2.9), and it stays below the guaranteed on_idle headroom.
  pub ActorWakeupWeightLimit: Weight = Perbill::from_percent(20) * MAXIMUM_BLOCK_WEIGHT;
  pub ActorOnIdleReserve: Weight =
    MIN_ON_IDLE_RESERVE_RATIO * MAXIMUM_BLOCK_WEIGHT;
  // --- Lifecycle and sweep controls ---

  pub const ActorMaxConsecutiveFailures: u32 = 10;
  pub const ActorMaxRetryAttempts: u32 = 10;
  pub const ActorMaxAutoCloseNonceHorizon: u64 = 10_000;
  pub const ActorMinUserBalance: Balance = 5 * ExistentialDeposit::get();
  pub const ActorMaxSweepBatch: u32 = 5;

  // --- Starvation safeguard controls ---

  pub const ActorMaxIdleStarvationBlocks: u32 = 25;
  /// Maximum number of active Actors instances. Bounds the BTreeSet storage.
  /// Set to 10,000 for production use cases with high automation density.
  pub const ActorMaxActiveActors: u32 = 10_000;
  // --- Economic parameters ---

  pub const ActorMaxSystemPriceDeviation: Perbill =
    ecosystem::params::MAX_SYSTEM_PRICE_DEVIATION;
  pub const ActorMaxSystemReferenceAgeBlocks: u32 =
    ecosystem::params::MAX_SYSTEM_REFERENCE_AGE_BLOCKS;

  /// Non-refundable opening fee routed to `FeeSink`
  pub const ActorCreationFee: Balance = ExistentialDeposit::get();
}

pub struct ActorMinUserBalanceGuard;

impl Get<Balance> for ActorMinUserBalanceGuard {
  fn get() -> Balance {
    ActorMinUserBalance::get().max(ExistentialDeposit::get())
  }
}

/// Canonical unified fee-collection boundary for Actors charges.
///
/// The collector transfers every opening, evaluation, and execution fee in full to the Fee Sink
/// System Actors. Phase-specific allocation happens later through that actor's bounded
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
    // Fee collection is one explicit certified producer: exactly one read-only
    // ingress preflight, one fee-native ledger movement, and one post-movement
    // notification in the same transaction. The ledger-only primitive performs
    // NO generic AssetOps ingress, transaction-extension ingress, or native-
    // staking bridge, so notifying the same movement twice is impossible.
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> DispatchResult {
        TmctolAssetOps::transfer_native_ledger_only(payer, fee_sink, amount)
          .map_err(|failure| failure.error)?;
        crate::configs::RuntimeAddressEventIngress::on_internal_inbound(
          fee_sink,
          native_asset,
          amount,
          payer,
        )
        .map_err(|failure| failure.error)?;
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
}

pub struct ActorFeeRecipient;
impl Get<crate::AccountId> for ActorFeeRecipient {
  fn get() -> crate::AccountId {
    crate::Actors::sovereign_account_id_system(ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
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
    let lp_farmer = crate::Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID,
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
    let amount = <Balances as Currency<AccountId>>::free_balance(&staking_pool)
      .saturating_sub(ExistentialDeposit::get());
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

  /// Private ledger-only native transfer used by the certified `FeeCollector`
  /// producer. It performs one read-only ingress preflight and one fee-native
  /// ledger movement, but deliberately NO generic `AssetOps` ingress, NO
  /// transaction-extension ingress, and NO native-staking bridge side effect.
  /// The caller (FeeCollector) submits exactly one post-movement notification,
  /// so one charge yields exactly one preflight/one movement/one notification.
  pub(crate) fn transfer_native_ledger_only(
    from: &AccountId,
    to: &AccountId,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(failure) =
        RuntimeAddressEventIngress::preflight_internal_inbound(to, AssetKind::Native, amount, from)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          failure.into(),
        ));
      }
      let result = (|| -> Result<(), DispatchError> {
        <Balances as Currency<AccountId>>::transfer(
          from,
          to,
          amount,
          polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
        )?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          TaskFailure::permanent(error),
        )),
      }
    })
  }
}

impl AssetOps<AccountId, AssetKind, Balance> for TmctolAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(failure) =
        RuntimeAddressEventIngress::preflight_internal_inbound(to, asset, amount, from)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          failure.into(),
        ));
      }
      let result = (|| -> Result<(), TaskFailure> {
        match asset {
          AssetKind::Native => {
            <Balances as Currency<AccountId>>::transfer(
              from,
              to,
              amount,
              polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
            )
            .map_err(TaskFailure::permanent)?;
            Self::bridge_native_staking_ingress(to, amount).map_err(TaskFailure::permanent)?;
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::transfer(
              id,
              from,
              to,
              amount,
              Preservation::Expendable,
            )
            .map_err(TaskFailure::permanent)?;
          }
        }
        // A certified destination ingress consequence keeps its closed retry
        // classification through TaskFailure (spec 6.1): recoverable queue/wakeup
        // capacity is Temporary, exhaustion/corruption/invariant failure is
        // Permanent, so the owning task retries rather than aborting.
        RuntimeAddressEventIngress::on_internal_inbound(to, asset, amount, from)
          .map_err(TaskFailure::from)?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(failure) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(failure))
        }
      }
    })
  }

  fn burn(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
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
    })()
    .map_err(TaskFailure::permanent)
  }

  fn mint(to: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> Result<(), TaskFailure> {
        match asset {
          AssetKind::Native => {
            let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
              id, to, amount,
            )
            .map_err(TaskFailure::permanent)?;
          }
        }
        // Source-less certified Mint keeps the placement classification through
        // TaskFailure so the owning task retries on recoverable capacity.
        RuntimeAddressEventIngress::on_inbound_without_source(to, asset, amount)
          .map_err(TaskFailure::from)?;
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(failure) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(failure))
        }
      }
    })
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

  fn preflight_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    if amount == 0 {
      return Ok(());
    }
    let deposit = match asset {
      AssetKind::Native => {
        <Balances as NativeInspect<AccountId>>::can_withdraw(from, amount)
          .into_result(false)
          .map_err(TaskFailure::permanent)?;
        <Balances as NativeInspect<AccountId>>::can_deposit(to, amount, Provenance::Extant)
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::can_withdraw(
          id, from, amount,
        )
        .into_result(false)
        .map_err(TaskFailure::permanent)?;
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::can_deposit(
          id,
          to,
          amount,
          Provenance::Extant,
        )
      }
    };
    match deposit {
      DepositConsequence::Success => Ok(()),
      DepositConsequence::BelowMinimum
      | DepositConsequence::CannotCreate
      | DepositConsequence::Blocked => Err(TaskFailure::temporary(
        pallet_deos_actors::Error::<Runtime>::RecipientDepositUnavailable,
      )),
      permanent => Err(TaskFailure::permanent(
        permanent
          .into_result()
          .expect_err("non-success deposit consequence has an error"),
      )),
    }
  }
}

pub struct TmctolDexOps;

pub(crate) fn validate_remove_liquidity_output(
  amount_a: Balance,
  amount_b: Balance,
  min_amount_a: Balance,
  min_amount_b: Balance,
) -> Result<(), TaskFailure> {
  if amount_a < min_amount_a || amount_b < min_amount_b {
    return Err(TaskFailure::temporary(DispatchError::Other(
      "MinimumLiquidityOutputNotMet",
    )));
  }
  Ok(())
}

pub(crate) fn classify_remove_liquidity_failure(error: DispatchError) -> TaskFailure {
  let first_minimum: DispatchError =
    pallet_asset_conversion::Error::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum.into();
  let second_minimum: DispatchError =
    pallet_asset_conversion::Error::<Runtime>::AssetTwoWithdrawalDidNotMeetMinimum.into();
  if error == first_minimum || error == second_minimum {
    TaskFailure::temporary(error)
  } else {
    TaskFailure::permanent(error)
  }
}

pub(crate) fn classify_router_failure(error: pallet_axial_router::Error<Runtime>) -> TaskFailure {
  use pallet_axial_router::RouterFailureClass;
  match error.failure_class() {
    RouterFailureClass::NoViableRoute
    | RouterFailureClass::ProtectionRejected
    | RouterFailureClass::LiquidityUnavailable
    | RouterFailureClass::PublicationRejected => TaskFailure::temporary(error),
    RouterFailureClass::InvalidRequest
    | RouterFailureClass::FeeRejected
    | RouterFailureClass::IngressRejected
    | RouterFailureClass::InvariantViolation => TaskFailure::permanent(error),
  }
}

pub struct TmctolLiquidityOps;

impl DexOps<AccountId, AssetKind, Balance> for TmctolDexOps {
  fn swap_exact_in(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    let quote = pallet_axial_router::Pallet::<Runtime>::quote_exact_input(
      who.clone(),
      asset_in,
      asset_out,
      amount_in,
    )
    .map_err(classify_router_failure)?;
    let min_out =
      (polkadot_sdk::sp_runtime::Perbill::one() - slippage_tolerance).mul_floor(quote.amount_out);
    Self::ensure_system_reference_price(
      &context,
      asset_in,
      asset_out,
      quote.amount_after_fee,
      quote.amount_out,
    )?;
    pallet_axial_router::Pallet::<Runtime>::execute_swap_for(
      who, asset_in, asset_out, amount_in, min_out, who,
    )
    .map(|outcome| DexSwapOutcome {
      total_amount_in: outcome.total_amount_in,
      recipient_amount_out: outcome.recipient_amount_out,
    })
    .map_err(TaskFailure::permanent)
  }

  fn swap_exact_out(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    let quote = pallet_axial_router::Pallet::<Runtime>::quote_exact_out(
      who.clone(),
      asset_in,
      asset_out,
      amount_out,
    )
    .map_err(classify_router_failure)?;
    // Tolerance-bound cap with checked widened ceiling arithmetic: no saturation or
    // silent clamp. The ceiling is quote.amount_in + ceil(slippage * quote.amount_in),
    // computed in U256 and narrowed to the balance width; overflow fails closed.
    let quoted_max_in = U256::from(quote.amount_in)
      .checked_add(
        U256::from(quote.amount_in)
          .checked_mul(U256::from(slippage_tolerance.deconstruct()))
          .and_then(|value| value.checked_add(U256::from(1_000_000_000u64 - 1)))
          .map(|value| value / U256::from(1_000_000_000u64))
          .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?,
      )
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?;
    let quoted_max_in: Balance = quoted_max_in
      .try_into()
      .map_err(|_| TaskFailure::permanent(DispatchError::Other("ExactOutCapOverflow")))?;
    // The tolerance-bound cap, not merely the larger preservable balance, bounds the
    // Router exact-output execution boundary.
    let execution_cap = quoted_max_in.min(max_amount_in);
    if quoted_max_in > max_amount_in {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "ExactOutInputCapacityExceeded",
      )));
    }
    Self::ensure_system_reference_price(
      &context,
      asset_in,
      asset_out,
      quote.amount_after_fee,
      quote.amount_out,
    )?;
    pallet_axial_router::Pallet::<Runtime>::execute_exact_out_for(
      who,
      asset_in,
      asset_out,
      amount_out,
      execution_cap,
      who,
    )
    .map(|outcome| DexSwapOutcome {
      total_amount_in: outcome.total_amount_in,
      recipient_amount_out: outcome.recipient_amount_out,
    })
    .map_err(TaskFailure::permanent)
  }
}

impl LiquidityOps<AccountId, AssetKind, Balance> for TmctolLiquidityOps {
  fn lp_assets(lp_asset: AssetKind) -> Option<(AssetKind, AssetKind)> {
    let AssetKind::Local(lp_id) = lp_asset else {
      return None;
    };
    crate::AxialRouter::lp_pair_by_token_id(lp_id)
  }

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    amount_a: Balance,
    amount_b: Balance,
    min_lp_out: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      use alloc::boxed::Box;
      let result = (|| -> Result<(Balance, Balance, Balance), TaskFailure> {
        if AssetConversion::get_reserves(asset_a, asset_b).is_err() {
          AssetConversion::create_pool(
            RuntimeOrigin::signed(who.clone()),
            Box::new(asset_a),
            Box::new(asset_b),
          )
          .map_err(TaskFailure::permanent)?;
        }
        super::assets_config::register_pool_lp_pair(asset_a, asset_b)
          .map_err(TaskFailure::permanent)?;
        let lp_before = liquidity_lp_balance(who, asset_a, asset_b);
        let a_before = TmctolAssetOps::balance(who, asset_a);
        let b_before = TmctolAssetOps::balance(who, asset_b);
        AssetConversion::add_liquidity(
          RuntimeOrigin::signed(who.clone()),
          Box::new(asset_a),
          Box::new(asset_b),
          amount_a,
          amount_b,
          0,
          0,
          who.clone(),
        )
        .map_err(TaskFailure::permanent)?;
        let lp_after = liquidity_lp_balance(who, asset_a, asset_b);
        let lp_minted = lp_after.saturating_sub(lp_before);
        // Factual outcomes: measure the actual asset debits and LP output rather than
        // returning the authored caps as if fully consumed (spec 3.4).
        let used_a = a_before.saturating_sub(TmctolAssetOps::balance(who, asset_a));
        let used_b = b_before.saturating_sub(TmctolAssetOps::balance(who, asset_b));
        if lp_minted < min_lp_out {
          return Err(TaskFailure::temporary(DispatchError::Other(
            "MinimumLpOutputNotMet",
          )));
        }
        Ok((used_a, used_b, lp_minted))
      })();
      match result {
        Ok(value) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(value)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetKind,
    asset_a: AssetKind,
    asset_b: AssetKind,
    lp_amount: Balance,
    min_amount_a: Balance,
    min_amount_b: Balance,
  ) -> Result<(Balance, Balance), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      use alloc::boxed::Box;
      let result = (|| -> Result<(Balance, Balance), TaskFailure> {
        let lp_id = match lp_asset {
          AssetKind::Local(id) => id,
          _ => {
            return Err(TaskFailure::permanent(DispatchError::Other(
              "LP asset must be Local",
            )));
          }
        };
        let (registry_a, registry_b) =
          crate::AxialRouter::lp_pair_by_token_id(lp_id).ok_or_else(|| {
            TaskFailure::permanent(DispatchError::Other("Pool not found for LP token"))
          })?;
        // The expected ordered pair must match the stable registry binding; an
        // admitted LP token is never silently reinterpreted.
        if (registry_a, registry_b) != (asset_a, asset_b) {
          return Err(TaskFailure::permanent(DispatchError::Other(
            "LiquidityPairBindingMismatch",
          )));
        }
        let before_a = TmctolAssetOps::balance(who, asset_a);
        let before_b = TmctolAssetOps::balance(who, asset_b);
        AssetConversion::remove_liquidity(
          RuntimeOrigin::signed(who.clone()),
          Box::new(asset_a),
          Box::new(asset_b),
          lp_amount,
          min_amount_a,
          min_amount_b,
          who.clone(),
        )
        .map_err(classify_remove_liquidity_failure)?;
        let after_a = TmctolAssetOps::balance(who, asset_a);
        let after_b = TmctolAssetOps::balance(who, asset_b);
        let amount_a = after_a.saturating_sub(before_a);
        let amount_b = after_b.saturating_sub(before_b);
        validate_remove_liquidity_output(amount_a, amount_b, min_amount_a, min_amount_b)?;
        Ok((amount_a, amount_b))
      })();
      match result {
        Ok(value) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(value)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetKind,
    asset_b: AssetKind,
    max_amount_a: Balance,
    max_amount_b: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("StakedAssetUnavailable")))?;
    if asset_a == AssetKind::Local(native_asset_id) && asset_b == AssetKind::Local(staked_asset_id)
    {
      if max_amount_a.is_zero() {
        return Err(TaskFailure::permanent(DispatchError::Other(
          "DonationAmountTooSmall",
        )));
      }
      let donation =
        crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
          who,
          max_amount_a,
          max_amount_b,
          max_ratio_error,
        )?;
      TmctolAssetOps::bridge_native_staking_pool_yield().map_err(TaskFailure::permanent)?;
      return Ok(donation);
    }
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityDonationUnsupported",
    )))
  }
}

impl TmctolDexOps {
  pub(crate) fn ensure_system_reference_price(
    context: &ExecutionContext<'_, AccountId>,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    amount_out: Balance,
  ) -> Result<(), TaskFailure> {
    if context.actor_type != ActorType::System {
      return Ok(());
    }
    if amount_in == 0 || amount_out == 0 {
      return Err(TaskFailure::permanent(DispatchError::Other(
        "InvalidSystemMarketQuote",
      )));
    }
    let feed = crate::configs::oracle_config::axial_router_pool_feed(asset_in, asset_out);
    let ema_reference =
      crate::Oracle::observation_state(feed, ActorMaxSystemReferenceAgeBlocks::get())
        .ok()
        .and_then(|state| match state {
          pallet_oracle::ObservationState::Fresh(observation) if observation.value > 0 => {
            Some(observation.value)
          }
          _ => None,
        });
    let reserve_reference = AssetConversion::get_reserves(asset_in, asset_out)
      .ok()
      .and_then(|(reserve_in, reserve_out)| {
        (!reserve_in.is_zero()).then(|| {
          reserve_out
            .saturating_mul(ecosystem::params::PRECISION)
            .saturating_div(reserve_in)
        })
      })
      .filter(|price| *price > 0);
    let reference = ema_reference.or(reserve_reference).ok_or_else(|| {
      TaskFailure::temporary(DispatchError::Other("SystemReferencePriceUnavailable"))
    })?;
    // Checked cross-multiplication deviation guard (spec 5.3): the scaled reference
    // price is ref_out/ref_in * PRECISION; comparing without division requires
    //   abs(exec_out * ref_in - ref_out * exec_in) * ACCURACY
    //     <= deviation * ref_out * exec_in
    // computed with a sufficient widened integer type (U256) and checked narrowing.
    // The products ref_out * exec_in and the deviation product use PRECISION-scaled
    // values consistently; a widening overflow fails closed as Permanent.
    let exec_in = U256::from(amount_in);
    let exec_out = U256::from(amount_out);
    let ref_in = U256::from(ecosystem::params::PRECISION);
    let ref_out = U256::from(reference);
    let a = exec_out
      .checked_mul(ref_in)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let b = exec_in
      .checked_mul(ref_out)
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let abs_diff = a.max(b) - a.min(b);
    let left = abs_diff
      .checked_mul(U256::from(1_000_000_000u64))
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    let right = U256::from(ActorMaxSystemPriceDeviation::get().deconstruct())
      .checked_mul(ref_out)
      .and_then(|value| value.checked_mul(exec_in))
      .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("ReferenceGuardOverflow")))?;
    if left > right {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "SystemPriceDeviationExceeded",
      )));
    }
    Ok(())
  }
}

fn liquidity_lp_balance(who: &AccountId, asset_a: AssetKind, asset_b: AssetKind) -> Balance {
  let pool_id =
    <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(&asset_a, &asset_b).ok();
  let Some(pool_id) = pool_id else {
    return 0;
  };
  let Some(pool_info) = pallet_asset_conversion::Pools::<Runtime>::get(pool_id) else {
    return 0;
  };
  <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(pool_info.lp_token, who)
}

/// System Actors genesis initializer for the current DEOS reference runtime.
///
/// Creates well-known System actors at genesis with deterministic `actor_id` values
/// defined in `primitives::ecosystem::actor_ids` (including sparse ranges).
/// The sovereign accounts are derived from `(ActorsPalletId, "system", actor_id)`
/// and can be computed offline for use in other configs.
pub struct TmctolGenesisSystemActors;

impl TmctolGenesisSystemActors {
  /// Runtime-topology accounts that retain one free native ED so arbitrarily small native ingress
  /// remains admissible under `pallet-balances` semantics.
  pub fn native_flow_anchor_accounts() -> alloc::vec::Vec<AccountId> {
    let mut accounts = (ecosystem::actor_ids::BURNING_MANAGER_ACTORS_ID
      ..=ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID)
      .map(crate::Actors::sovereign_account_id_system)
      .collect::<alloc::vec::Vec<_>>();
    accounts.push(crate::Staking::pool_account_for(
      <Runtime as pallet_staking::Config>::NativeStakingAssetId::get(),
    ));
    accounts
  }

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
  pallet_deos_actors::GenesisSystemActors<
    AccountId,
    pallet_deos_actors::ScheduleOf<Runtime>,
    pallet_deos_actors::ScheduleWindow<crate::BlockNumber>,
    pallet_deos_actors::ExecutionPlanOf<Runtime>,
  > for TmctolGenesisSystemActors
{
  fn system_actors() -> alloc::vec::Vec<(
    pallet_deos_actors::ActorId,
    AccountId,
    pallet_deos_actors::Mutability,
    pallet_deos_actors::ScheduleOf<Runtime>,
    Option<pallet_deos_actors::ScheduleWindow<crate::BlockNumber>>,
    pallet_deos_actors::ExecutionPlanOf<Runtime>,
    pallet_deos_actors::CompletionPolicy,
  )> {
    use pallet_deos_actors::{Mutability, Schedule, Trigger};
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = ActorsPalletId::get().into_account_truncating();

    // --- Burn Actor (actor_id = 0; legacy constant: BURNING_MANAGER_ACTORS_ID) ---
    // Omnivorous intake: any verified inbound value signals one bounded pass that
    // swaps configured foreign balances to native and burns available native.
    let burn_schedule = Schedule {
      trigger: Trigger::immediate_manual_and_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      ),
      cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
    };
    let dust = ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    // Genesis execution_plan: swap known foreign assets → native, then burn.
    // Governance adds steps for new foreign assets via `update_execution_plan`.
    let burn_execution_plan: pallet_deos_actors::ExecutionPlanOf<Runtime> =
      Self::build_burn_execution_plan(alloc::vec![], dust);

    // --- Fee Sink (actor_id = 1) ---
    // Inbound-driven Phase 1 fan-out: distributes accumulated native fees/rewards
    // into staking-pool yield and native LP-donation ingress channels.
    let fee_sink_schedule = Schedule {
      trigger: Trigger::immediate_manual_and_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      ),
      cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
    };
    let fee_sink_execution_plan: pallet_deos_actors::ExecutionPlanOf<Runtime> =
      Self::build_phase1_fee_sink_execution_plan();

    alloc::vec![
      (
        ecosystem::actor_ids::BURNING_MANAGER_ACTORS_ID,
        governance.clone(),
        Mutability::Mutable,
        burn_schedule,
        None,
        burn_execution_plan,
        pallet_deos_actors::CompletionPolicy::Persistent,
      ),
      (
        ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
        governance.clone(),
        Mutability::Mutable,
        fee_sink_schedule,
        None,
        fee_sink_execution_plan,
        pallet_deos_actors::CompletionPolicy::Persistent,
      ),
      // --- BLDR Splitter (actor_id = 10) ---
      // Receives 66% of TMC-minted $BLDR, splits 50/50 to BLDR liquidity + treasury lanes.
      (
        ecosystem::actor_ids::BLDR_SPLITTER_ACTORS_ID,
        governance,
        Mutability::Mutable,
        Schedule {
          trigger: Trigger::immediate_manual_and_address_event(
            pallet_deos_actors::SourceFilter::Any,
            pallet_deos_actors::AssetFilter::Any,
          ),
          cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
        },
        None,
        Self::build_bldr_splitter_execution_plan(
          AssetKind::Local(ecosystem::protocol_tokens::BLDR_ASSET_ID),
          dust,
        ),
        pallet_deos_actors::CompletionPolicy::Persistent,
      ),
    ]
  }

  fn system_custody_accounts() -> alloc::vec::Vec<pallet_deos_actors::ActorId> {
    alloc::vec![
      ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
      ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID,
    ]
  }

  fn dormant_system_actors() -> alloc::vec::Vec<(pallet_deos_actors::ActorId, AccountId)> {
    use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
    let governance: AccountId = ActorsPalletId::get().into_account_truncating();
    alloc::vec![
      ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
      ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
      ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
      ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
      ecosystem::actor_ids::TREASURY_B_ACTORS_ID,
      ecosystem::actor_ids::TREASURY_C_ACTORS_ID,
      ecosystem::actor_ids::TREASURY_D_ACTORS_ID,
      ecosystem::actor_ids::BLDR_ZM_ACTORS_ID,
      ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
      ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID,
    ]
    .into_iter()
    .map(|actor_id| (actor_id, governance.clone()))
    .collect()
  }
}

impl TmctolGenesisSystemActors {
  fn all_conditions(
    conditions: alloc::vec::Vec<
      pallet_deos_actors::Condition<AssetKind, Balance, u32, primitives::OracleFeedId>,
    >,
  ) -> pallet_deos_actors::ConditionSetOf<Runtime> {
    pallet_deos_actors::ConditionSet::All(
      conditions
        .try_into()
        .expect("runtime condition group fits MaxConditionsPerStep"),
    )
  }

  pub fn build_phase1_fee_sink_execution_plan() -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, SplitLeg, Step, StepErrorPolicy, Task};
    alloc::vec![Step {
      conditions: Default::default(),
      task: Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::AllAvailable,
        legs: alloc::vec![
          SplitLeg {
            to: crate::Staking::pool_account_for(0),
            share: Perbill::from_percent(50),
          },
          SplitLeg {
            to: crate::Actors::sovereign_account_id_system(
              ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID,
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
  /// conditional SwapIn step (skip if balance < dust), then a final Burn step.
  pub fn build_burn_execution_plan(
    foreign_assets: alloc::vec::Vec<AssetKind>,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let mut steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec::Vec::new();
    for foreign in foreign_assets {
      steps.push(Step {
        conditions: dust_guard(foreign),
        task: Task::SwapIn {
          asset_in: foreign,
          amount_in: AmountResolution::AllAvailable,
          asset_out: AssetKind::Native,
          slippage_tolerance: ecosystem::params::SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      });
    }
    // Final step: burn all accumulated native (only if above dust)
    steps.push(Step {
      conditions: dust_guard(AssetKind::Native),
      task: Task::Burn {
        asset: AssetKind::Native,
        amount: AmountResolution::AllAvailable,
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    steps
      .try_into()
      .expect("burn execution_plan fits within MaxExecutionPlanSteps")
  }

  /// Builds the Liquidity Actor execution_plan for a specific foreign asset / LP pair.
  ///
  /// Called by governance after pool creation, since LP asset IDs are
  /// pool-specific and unknown at genesis.
  ///
  /// ExecutionPlan steps:
  /// 1. If Native > dust AND Foreign > dust → AddLiquidity (opportunistic)
  /// 2. If Foreign > dust → SwapIn Foreign→Native with reserve-aware slippage
  /// 3. If LP > dust → SplitTransfer LP to TOL buckets (50/16.67/16.67/16.66)
  pub fn build_zap_execution_plan(
    foreign: AssetKind,
    lp_asset: AssetKind,
    dust_threshold: Balance,
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, SplitLeg, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| {
      Self::all_conditions(alloc::vec![
        Condition::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Condition::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ])
    };
    let slippage_tolerance = Self::resolve_zap_slippage_tolerance(foreign);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      // Step 1: Opportunistic LP provisioning — add both sides at current pool ratio
      // AllAvailable for native subtracts ED at resolution layer, safe with Preserve semantics
      Step {
        conditions: dual_dust_guard(AssetKind::Native, foreign),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: foreign,
          amount_a: AmountResolution::AllAvailable,
          amount_b: AmountResolution::AllAvailable,
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 2: Patriotic accumulation — convert leftover Foreign to Native
      Step {
        conditions: dust_guard(foreign),
        task: Task::SwapIn {
          asset_in: foreign,
          amount_in: AmountResolution::AllAvailable,
          asset_out: AssetKind::Native,
          slippage_tolerance,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      // Step 3: Distribute LP tokens to TOL buckets
      Step {
        conditions: dust_guard(lp_asset),
        task: Task::SplitTransfer {
          asset: lp_asset,
          amount: AmountResolution::AllAvailable,
          legs: alloc::vec![
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_A_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_B_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_B_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_C_ACTORS_ID,
              ),
              share: ecosystem::params::TOL_BUCKET_C_ALLOCATION,
            },
            SplitLeg {
              to: pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
                ecosystem::actor_ids::TOL_BUCKET_D_ACTORS_ID,
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
      .expect("Liquidity Actor execution_plan fits within MaxExecutionPlanSteps")
  }

  /// Builds the Bucket-side half of production-admissible LP unwind.
  ///
  /// The Bucket transfers a bounded LP fraction into the paired Treasury sovereign.
  /// The Treasury then removes liquidity in its own independently admitted cycle.
  pub fn build_bucket_lp_transfer_execution_plan(
    lp_asset: AssetKind,
    dust_threshold: Balance,
    unwind_pct: polkadot_sdk::sp_runtime::Perbill,
    treasury_actor_id: u64,
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let treasury_account =
      pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(treasury_actor_id);
    alloc::vec![Step {
      conditions: Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]),
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
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let lp_id = match lp_asset {
      AssetKind::Local(id) => id,
      _ => panic!("Treasury LP unwind requires a Local LP asset"),
    };
    let (asset_a, asset_b) = crate::AxialRouter::lp_pair_by_token_id(lp_id)
      .expect("Treasury LP unwind requires a registered LP pair");
    alloc::vec![Step {
      conditions: Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset: lp_asset,
        threshold: dust_threshold,
      }]),
      task: Task::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount: AmountResolution::AllAvailable,
        min_amount_a: 1,
        min_amount_b: 1,
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
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, SplitLeg, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let bldr_zm_account = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_ZM_ACTORS_ID,
    );
    let bldr_treasury_account = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![Step {
      conditions: dust_guard(bldr_asset),
      task: Task::SplitTransfer {
        asset: bldr_asset,
        amount: AmountResolution::AllAvailable,
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
      .expect("BLDR splitter execution_plan fits within MaxExecutionPlanSteps")
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
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let dust_guard = |asset: AssetKind| {
      Self::all_conditions(alloc::vec![Condition::BalanceAbove {
        asset,
        threshold: dust_threshold,
      }])
    };
    let dual_dust_guard = |asset_a: AssetKind, asset_b: AssetKind| {
      Self::all_conditions(alloc::vec![
        Condition::BalanceAbove {
          asset: asset_a,
          threshold: dust_threshold,
        },
        Condition::BalanceAbove {
          asset: asset_b,
          threshold: dust_threshold,
        },
      ])
    };
    let bldr_bucket_a = pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(
      ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID,
    );
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      Step {
        conditions: dual_dust_guard(AssetKind::Native, bldr_asset),
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: bldr_asset,
          amount_a: AmountResolution::AllAvailable,
          amount_b: AmountResolution::AllAvailable,
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      Step {
        conditions: dust_guard(lp_asset),
        task: Task::Transfer {
          to: bldr_bucket_a,
          asset: lp_asset,
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("BLDR Liquidity Actor execution_plan fits within MaxExecutionPlanSteps")
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
    crate::Actors::activate_actor(
      RuntimeOrigin::root(),
      ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID,
      pallet_deos_actors::ProgramInput::Active(pallet_deos_actors::ActiveProgramInput {
        schedule: pallet_deos_actors::Schedule {
          trigger: pallet_deos_actors::Trigger::immediate_manual_and_address_event(
            pallet_deos_actors::SourceFilter::Any,
            pallet_deos_actors::AssetFilter::Any,
          ),
          cooldown_blocks: ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
        },
        schedule_window: None,
        execution_plan,
        completion_policy: pallet_deos_actors::CompletionPolicy::Persistent,
        funding_source_policy: pallet_deos_actors::FundingSourcePolicy::RuntimePolicy,
        auto_close_at_cycle_nonce: None,
      }),
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
    let actor_id = ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID;
    if crate::Actors::active_actor_view(actor_id).is_none()
      && crate::Actors::actor_identities(actor_id).is_none()
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
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let native_staking_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let native_asset = AssetKind::Local(native_staking_asset_id);
    let staked_asset_id = crate::Staking::staked_asset_id(native_staking_asset_id)
      .expect("native staking LP farming activation checks staked asset first");
    let staked_asset = AssetKind::Local(staked_asset_id);
    let native_dust = Self::all_conditions(alloc::vec![Condition::BalanceAbove {
      asset: native_asset,
      threshold: dust_threshold,
    }]);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![Step {
      conditions: native_dust,
      task: Task::DonateLiquidity {
        asset_a: native_asset,
        asset_b: staked_asset,
        max_amount_a: AmountResolution::AllAvailable,
        max_ratio_error: ecosystem::params::NATIVE_STAKING_LP_DONATION_MAX_RATIO_ERROR,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }];
    steps
      .try_into()
      .expect("native staking LP farming execution_plan fits within MaxExecutionPlanSteps")
  }

  /// Builds the Treasury B BLDR buyback-and-burn execution_plan.
  ///
  /// ExecutionPlan steps:
  /// 1. SwapIn(NTVE → target) — amount resolved as % of current NTVE balance
  /// 2. Burn(target, AllAvailable) — destroy all acquired tokens
  ///
  /// Multiple small buybacks per day create smooth market pressure.
  pub fn build_treasury_b_buyback_execution_plan(
    target_asset: AssetKind,
    buyback_pct: polkadot_sdk::sp_runtime::Perbill,
    dust_threshold: Balance,
    slippage: polkadot_sdk::sp_runtime::Perbill,
  ) -> pallet_deos_actors::ExecutionPlanOf<Runtime> {
    use pallet_deos_actors::{AmountResolution, Condition, Step, StepErrorPolicy, Task};
    let native_dust = Self::all_conditions(alloc::vec![Condition::BalanceAbove {
      asset: AssetKind::Native,
      threshold: dust_threshold,
    }]);
    let target_dust = Self::all_conditions(alloc::vec![Condition::BalanceAbove {
      asset: target_asset,
      threshold: dust_threshold,
    }]);
    let steps: alloc::vec::Vec<pallet_deos_actors::StepOf<Runtime>> = alloc::vec![
      // Step 1: Swap NTVE → target (% of current balance)
      Step {
        conditions: native_dust,
        task: Task::SwapIn {
          asset_in: AssetKind::Native,
          amount_in: AmountResolution::PercentageOfCurrent(buyback_pct),
          asset_out: target_asset,
          slippage_tolerance: slippage,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      // Step 2: Burn all acquired target tokens
      Step {
        conditions: target_dust,
        task: Task::Burn {
          asset: target_asset,
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ];
    steps
      .try_into()
      .expect("Treasury B buyback execution_plan fits within MaxExecutionPlanSteps")
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

impl pallet_deos_actors::adapters::StakingOps<AccountId, AssetKind, Balance> for TmctolStakingOps {
  fn stake(who: &AccountId, asset: AssetKind, amount: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
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
    })()
    .map_err(TaskFailure::permanent)
  }

  fn unstake(who: &AccountId, asset: AssetKind, shares: Balance) -> Result<(), TaskFailure> {
    (|| -> DispatchResult {
      let _ = crate::Staking::unstake(
        RuntimeOrigin::signed(who.clone()).into(),
        Self::staking_asset_id(asset),
        shares,
      )?;
      Ok(())
    })()
    .map_err(TaskFailure::permanent)
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
  fn permits(
    _: pallet_deos_actors::ActorId,
    _: &AccountId,
    _: Option<&AccountId>,
    _: Option<&pallet_deos_actors::FundingProvenance>,
  ) -> bool {
    // The reference launch line has no source/actor authorization entries.
    // Downstream runtimes must add explicit pairs rather than inheriting trust
    // from an account-shaped signed, internal-protocol, or XCM identity.
    false
  }
}

/// Derived sovereign accounts must never collide with host-reserved identities.
///
/// The reference runtime marks the Fee Sink and the reserved deterministic System Actors custody
/// accounts as reserved so a hashed sovereign derivation can never alias them.
pub struct DeosSovereignAccountPolicy;

impl pallet_deos_actors::adapters::SovereignAccountPolicy<AccountId>
  for DeosSovereignAccountPolicy
{
  fn is_reserved(account: &AccountId) -> bool {
    // The deterministic genesis System Actors custody accounts (including the Fee Sink) are
    // host-reserved; a hashed sovereign derivation can never alias them.
    (primitives::ecosystem::actor_ids::BURNING_MANAGER_ACTORS_ID
      ..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LP_FARMER_ACTORS_ID)
      .any(|id| account == &pallet_deos_actors::Pallet::<Runtime>::sovereign_account_id_system(id))
  }
}

pub struct TmctolObservationProvider;

impl pallet_deos_actors::ObservationProvider<primitives::OracleFeedId, crate::BlockNumber>
  for TmctolObservationProvider
{
  fn observe(
    feed: &primitives::OracleFeedId,
    _now: crate::BlockNumber,
    max_age_blocks: u32,
  ) -> pallet_deos_actors::ScalarObservationState<crate::BlockNumber> {
    match crate::Oracle::observation_state(*feed, max_age_blocks) {
      Ok(pallet_oracle::ObservationState::Fresh(observation)) => {
        pallet_deos_actors::ScalarObservationState::Fresh {
          value: observation.value,
          observed_at: observation.updated_at,
        }
      }
      Ok(pallet_oracle::ObservationState::Uninitialized) => {
        pallet_deos_actors::ScalarObservationState::Uninitialized
      }
      Ok(pallet_oracle::ObservationState::Stale(_)) => {
        pallet_deos_actors::ScalarObservationState::Stale
      }
      Ok(pallet_oracle::ObservationState::Unavailable) | Err(_) => {
        pallet_deos_actors::ScalarObservationState::Unavailable
      }
    }
  }
}

impl pallet_deos_actors::Config for Runtime {
  type PalletId = ActorsPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetKind;
  type FeeNativeAssetId = ActorFeeNativeAssetId;
  type Balance = Balance;
  type AssetOps = TmctolAssetOps;
  type ObservationFeedId = primitives::OracleFeedId;
  type ObservationProvider = TmctolObservationProvider;
  type FundingAuthority = DeosFundingAuthority;
  type SovereignAccountPolicy = DeosSovereignAccountPolicy;
  type DexOps = TmctolDexOps;
  type StakingOps = TmctolStakingOps;
  type LiquidityOps = TmctolLiquidityOps;
  type ActorCreationFee = ActorCreationFee;
  type FeeSink = ActorFeeRecipient;
  type FeeCollector = TmctolFeeCollector;
  type GenesisSystemActors = TmctolGenesisSystemActors;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxActiveActors = ActorMaxActiveActors;
  type MaxActorIdentities = ActorMaxActiveActors;
  type MaxSystemSovereigns = ActorMaxActiveActors;
  type MaxConditionsPerStep = ActorMaxConditionsPerStep;
  type MaxConsecutiveFailures = ActorMaxConsecutiveFailures;
  type MaxRetryAttempts = ActorMaxRetryAttempts;
  type MaxAutoCloseNonceHorizon = ActorMaxAutoCloseNonceHorizon;
  type TargetBlockTime = ActorTargetBlockTime;
  type MaxExecutionDelayBlocks = ActorMaxExecutionDelayBlocks;
  type MaxTimerJitterBlocks = ActorMaxTimerJitterBlocks;
  type MaxExecutionsPerBlock = ActorMaxExecutionsPerBlock;
  type MaxQueueLength = ActorMaxQueueLength;
  type QueuePageSize = ActorQueuePageSize;
  type WakeupPageSize = ActorWakeupPageSize;
  type ObservationPageSize = ActorObservationPageSize;
  type MaxQueueEntriesScannedPerBlock = ActorMaxQueueEntriesScannedPerBlock;
  type MaxObservationFanoutPagesPerBlock = ActorMaxObservationFanoutPagesPerBlock;
  type ObservationFanoutWeightLimit = ActorObservationFanoutWeightLimit;
  type WakeupWeightLimit = ActorWakeupWeightLimit;
  type MaxWakeupsPerBlock = ActorMaxWakeupsPerBlock;
  type MaxFundingTrackedAssets = ActorMaxFundingTrackedAssets;
  type MaxOpeningSnapshotEntries = ActorMaxOpeningSnapshotEntries;
  type MaxIdleStarvationBlocks = ActorMaxIdleStarvationBlocks;
  type ActorOnIdleReserve = ActorOnIdleReserve;
  type MaxOwnerSlots = ActorMaxOwnerSlots;
  type MaxExecutionPlanSteps = ActorMaxExecutionPlanSteps;
  type MaxSplitTransferLegs = ActorMaxSplitTransferLegs;
  type MaxSweepBatch = ActorMaxSweepBatch;
  type MaxWhitelistSize = ActorMaxWhitelistSize;
  type MaxTriggerSources = ActorMaxTriggerSources;
  type MinUserBalance = ActorMinUserBalanceGuard;
  type MinWindowLength = ActorMinWindowLength;
  type WeightInfo = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  type WeightToFee = crate::WeightToFee;
  // Runtime binds task upper bounds so fee admission stays chain-specific and auditable
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeActorsBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeActorsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl RuntimeActorsBenchmarkHelper {
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
impl pallet_deos_actors::BenchmarkHelper<AccountId, AssetKind, Balance, primitives::OracleFeedId>
  for RuntimeActorsBenchmarkHelper
{
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
    super::assets_config::register_pool_lp_pair(asset_a, asset_b)?;
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

  fn setup_remove_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, AssetKind, Balance), DispatchError> {
    let pool_count = 2u32;
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
    let liquidity = 1_000_000_000_000u128;
    let native_seed = liquidity.saturating_mul(pool_count.saturating_add(1) as u128);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(owner, native_seed);
    let mut target_lp: Option<(AssetKind, AssetKind, AssetKind, Balance)> = None;
    for i in 0..pool_count {
      let local_asset_id = 100_000u32.saturating_add(i);
      if Self::ensure_local_asset(local_asset_id, owner).is_err() {
        return Err(DispatchError::Other("EnsureLocalAssetFailed"));
      }
      if <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::mint_into(
        local_asset_id,
        owner,
        liquidity.saturating_add(1_000_000_000),
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
      super::assets_config::register_pool_lp_pair(asset_a, asset_b)?;
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
      if i.saturating_add(1) == pool_count {
        let lp_amount = <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(
          pool_info.lp_token,
          owner,
        );
        let min_native_reserve = <Balances as NativeInspect<AccountId>>::minimum_balance();
        let benchmark_lp_amount = lp_amount.saturating_sub(min_native_reserve);
        if benchmark_lp_amount == 0 {
          return Err(DispatchError::Other("LpAmountTooSmallForBenchmark"));
        }
        target_lp = Some((
          AssetKind::Local(pool_info.lp_token),
          asset_a,
          asset_b,
          benchmark_lp_amount,
        ));
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
    <TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<AccountId, AssetKind, Balance>>::stake(
      owner, asset, amount,
    )
    .map_err(|failure| failure.error)?;
    let shares = <TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<
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
    let _ = Self::setup_remove_liquidity(owner)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurningManagerAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    Ok((
      AssetKind::Local(100_000),
      AssetKind::Local(100_001),
      1_000_000,
    ))
  }

  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(AssetKind, AssetKind, Balance, Balance), DispatchError> {
    let _ = Self::setup_remove_liquidity(owner)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &BurningManagerAccount::get(),
      EXISTENTIAL_DEPOSIT,
    );
    Ok((
      AssetKind::Local(100_000),
      AssetKind::Local(100_001),
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

  fn setup_condition_assets(
    owner: &AccountId,
    max: u32,
  ) -> Result<alloc::vec::Vec<AssetKind>, DispatchError> {
    let assets = Self::funding_assets(max);
    for asset in &assets {
      if let AssetKind::Local(asset_id) = asset {
        Self::ensure_local_asset(*asset_id, owner)?;
      }
    }
    Ok(assets)
  }

  fn setup_observation_feeds(
    max: u32,
  ) -> Result<alloc::vec::Vec<primitives::OracleFeedId>, DispatchError> {
    let producer = crate::AxialRouter::account_id();
    let mut feeds = alloc::vec::Vec::with_capacity(max as usize);
    for index in 1..=max {
      let asset_in = AssetKind::Local(0x3000_0000u32.saturating_add(index));
      let asset_out = AssetKind::Native;
      crate::configs::oracle_config::ensure_axial_router_pool_feeds(asset_in, asset_out)?;
      let feed = crate::configs::oracle_config::axial_router_pool_feed(asset_in, asset_out);
      crate::Oracle::publish(RuntimeOrigin::signed(producer.clone()), feed, 1)?;
      feeds.push(feed);
    }
    Ok(feeds)
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
    let _ = (recipient, transferred);
    Ok(())
  }

  fn run_address_event_ingress(recipient: &AccountId, source: &AccountId, amount: Balance) -> bool {
    // The benchmark mirrors the extension's resolved-match semantics: an absent
    // sovereign is not a producer event at all.
    if crate::Actors::sovereign_index(recipient).is_none() {
      return false;
    }
    let event = pallet_deos_actors::AddressEvent {
      destination: recipient.clone(),
      source: Some(source.clone()),
      asset: AssetKind::Native,
      amount,
      provenance: Some(pallet_deos_actors::FundingProvenance::Signed),
    };
    <crate::configs::RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
      AccountId,
      AssetKind,
      Balance,
    >>::notify(&event)
    .is_ok()
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
}
