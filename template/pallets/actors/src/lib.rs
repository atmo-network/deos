#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "512"]

extern crate alloc;

use polkadot_sdk::{
  frame_support::{BoundedVec, traits::Get},
  sp_runtime::traits::{CheckedAdd, CheckedSub, Zero},
};

pub use pallet::*;

/// Keeps configured guarantees separate from the one block-bound allocation consumed by Prepass.
/// Dynamic hosts must use complete, non-overlapping settled-prefix and outstanding-tail owners.
/// The default explicitly retains the configured conservative fixed envelope; errors never fall back.
pub trait BlockResourceBudgetProvider<BlockNumber>: Get<BlockResourceBudget> {
  fn for_prepass(
    _now: BlockNumber,
    configured: BlockResourceBudget,
  ) -> Result<BlockResourceBudget, BlockResourceError> {
    Ok(configured)
  }
}

pub trait ActorPrepassContext {
  fn context_ready() -> bool;
}

impl ActorPrepassContext for () {
  fn context_ready() -> bool {
    true
  }
}

pub const ACTOR_PREPASS_INHERENT_VERSION: u8 = 1;

#[derive(codec::Decode, codec::Encode)]
pub struct ActorPrepassInherentData {
  version: u8,
}

struct CanonicalActorPrepassInherentData(ActorPrepassInherentData);

impl codec::Decode for CanonicalActorPrepassInherentData {
  fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
    let data = <ActorPrepassInherentData as codec::Decode>::decode(input)?;
    // InherentData supplies the marker's complete byte slice, not an enclosing codec stream.
    if input.remaining_len()? != Some(0) {
      return Err("Actor Prepass data must contain only its version".into());
    }
    Ok(Self(data))
  }
}

pub fn provide_actor_prepass_inherent_data(
  data: &mut polkadot_sdk::sp_inherents::InherentData,
) -> Result<(), polkadot_sdk::sp_inherents::Error> {
  data.put_data(
    ACTOR_PREPASS_INHERENT_IDENTIFIER,
    &ActorPrepassInherentData {
      version: ACTOR_PREPASS_INHERENT_VERSION,
    },
  )
}

pub mod contract;

pub mod types;

mod execution;
mod reactions;
mod scheduler;

pub use scheduler::EnqueueOutcome;

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub mod tariff;

pub mod adapters;
pub use adapters::{
  AddressEventIngress, AdmissionCertificateAuthority, AdmissionCertificateAuthorityProvider,
  AssetOps, BalanceTransitionIngress, BehaviorControlWeightContext,
  BehaviorServiceInspectionWeightContext, CanonicalObservationState, DexOps, DexSwapOutcome,
  ExecutionContext, FinancialWaitBackingProvider, FundingAuthority, IngressFailure, LiquidityOps,
  ObservationProvider, PipelineControlWeightProvider, RetryClass, ScalarObservationState,
  SovereignAccountDeriver, StakingOps, StepControlExecution, StepControlOutcome, StepControlPhase,
  StepControlPlacement, StepControlWeightContext, StepControlWeightProvider,
  SystemActorContractValidator, TaskEffectExecution, TaskEffectWeightProvider, TaskFailure,
};
pub use types::{
  ActorStepResourceReservation, AddressEvent, BlockResourceBudget, BlockResourceDomain,
  BlockResourceLimits, BlockResourcePhase, BlockResourceReservation, BlockResourceState,
  FinalizedBlockResourceSnapshot, FixedBlockWeightComponents, InputLimit, MAX_STEPS_PER_TAIL_CHUNK,
  Task, WakeupBucketState, WakeupCursorIndex,
};

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

/// Host read branches used by observation capture diagnostics.
#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkObservationReadState {
  Missing,
  Deactivated,
  Uninitialized,
  Stale,
  Fresh,
}

/// Bounded host-ledger observation used only outside recorded benchmark execution.
#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkLiquiditySnapshot<AssetId, Balance> {
  pub input_balances: (Balance, Balance),
  pub native_free: Balance,
  pub native_reserved: Balance,
  pub pool: Option<BenchmarkLiquidityPool<AssetId, Balance>>,
}

#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkLiquidityPool<AssetId, Balance> {
  pub lp_asset: AssetId,
  pub lp_balance: Balance,
  pub reserves: (Balance, Balance),
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance, ObservationFeedId> {
  /// Unsupported observers fail explicitly; absent pool is a supported snapshot with None.
  fn liquidity_snapshot(
    _who: &AccountId,
    _asset_a: AssetId,
    _asset_b: AssetId,
  ) -> Result<BenchmarkLiquiditySnapshot<AssetId, Balance>, polkadot_sdk::sp_runtime::DispatchError>
  {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkLiquiditySnapshotUnsupported",
    ))
  }

  /// Optional public producer of a non-Actor native hold, before Actor owner holds exist.
  fn setup_close_currency_peer(
    _owner: &AccountId,
  ) -> Result<bool, polkadot_sdk::sp_runtime::DispatchError> {
    Ok(false)
  }
  /// Verify the unrelated hold and its producer authority survived Actor Close.
  fn assert_close_currency_peer(_owner: &AccountId) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkCloseCurrencyPeerUnsupported",
    ))
  }
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Optional absent-pool fixture whose account deposits consume the creator's native balance.
  fn setup_add_liquidity_deposit_accounts(
    _owner: &AccountId,
  ) -> Result<Option<(AssetId, AssetId, Balance, Balance)>, polkadot_sdk::sp_runtime::DispatchError>
  {
    Ok(None)
  }
  /// Optional System pool-creation fixture: inputs, LP asset and expected LP receipt.
  fn setup_system_add_liquidity_deposits(
    _owner: &AccountId,
    _anchor_backed: bool,
  ) -> Result<
    Option<(AssetId, AssetId, Balance, Balance, AssetId, Balance)>,
    polkadot_sdk::sp_runtime::DispatchError,
  > {
    Ok(None)
  }
  /// Optional existing-pool fixture with no Native funding and an externally backed LP account.
  fn setup_system_add_liquidity_existing_pool(
    _owner: &AccountId,
  ) -> Result<
    Option<(AssetId, AssetId, Balance, Balance, AssetId, Balance)>,
    polkadot_sdk::sp_runtime::DispatchError,
  > {
    Ok(None)
  }
  /// Read-only fixture evidence for ledger-record presence, independent of spendable balance.
  /// None means the host does not support this asset's record topology.
  fn asset_account_exists(_who: &AccountId, _asset: AssetId) -> Option<bool> {
    None
  }

  /// Change host account spendability through its authorized lifecycle without moving custody.
  fn set_asset_account_frozen(
    _owner: &AccountId,
    _who: &AccountId,
    _asset: AssetId,
    _frozen: bool,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkAssetFreezeUnsupported",
    ))
  }
  /// Donation pair, input cap, and the two expected donated outputs for the host fixture.
  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Optional Fee Sink fixture: pool, recipient, converted asset, ID, amount and refusal.
  /// Refusal must occur in conversion after the first leg has executed.
  fn setup_native_conversion_ingress(
    _owner: &AccountId,
    _refusal: bool,
  ) -> Result<
    Option<(AccountId, AccountId, AssetId, ActorId, Balance, TaskFailure)>,
    polkadot_sdk::sp_runtime::DispatchError,
  > {
    Ok(None)
  }
  fn setup_remove_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Optional absent nonsufficient outputs, with third-party-deposited LP custody and no consumer.
  fn setup_remove_liquidity_consumer_outputs(
    _owner: &AccountId,
  ) -> Result<Option<(AssetId, AssetId, AssetId, Balance)>, polkadot_sdk::sp_runtime::DispatchError>
  {
    Ok(None)
  }
  fn setup_stake(
    owner: &AccountId,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_unstake(
    owner: &AccountId,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Optional first nonsufficient receipt fixture, with no pre-existing native consumer.
  fn setup_stake_consumer_receipt(
    _owner: &AccountId,
  ) -> Result<Option<(AssetId, Balance)>, polkadot_sdk::sp_runtime::DispatchError> {
    Ok(None)
  }
  /// Optional first nonsufficient base output fixture, with no pre-existing native consumer.
  fn setup_unstake_consumer_output(
    _owner: &AccountId,
  ) -> Result<Option<(AssetId, Balance)>, polkadot_sdk::sp_runtime::DispatchError> {
    Ok(None)
  }
  /// Remove an empty receipt through the host lifecycle after an Unstake Contract is admitted.
  fn remove_empty_staking_receipt(
    _owner: &AccountId,
    _asset: AssetId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkReceiptRemovalUnsupported",
    ))
  }
  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Optional first nonsufficient output from fee-native input; returns input cap and output.
  fn setup_swap_consumer_output(
    _owner: &AccountId,
  ) -> Result<Option<(AssetId, AssetId, Balance, Balance)>, polkadot_sdk::sp_runtime::DispatchError>
  {
    Ok(None)
  }
  fn funding_assets(max: u32) -> alloc::vec::Vec<AssetId>;
  fn setup_predicate_assets(
    owner: &AccountId,
    max: u32,
  ) -> Result<alloc::vec::Vec<AssetId>, polkadot_sdk::sp_runtime::DispatchError>;
  /// Fund a Transfer and return (maximum-width asset, total, empty recipient).
  /// The positive transfer amount must fall below the empty recipient's deposit minimum.
  fn setup_temporary_transfer(
    _owner: &AccountId,
    _actor: &AccountId,
  ) -> Result<(AssetId, Balance, AccountId), polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkTemporaryTransferUnsupported",
    ))
  }
  /// Prepare distinct assets with maximum encoded host balance-account state.
  fn setup_max_encoded_predicate_assets(
    _owner: &AccountId,
    _max: u32,
  ) -> Result<alloc::vec::Vec<AssetId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkMaximumAssetEncodingUnsupported",
    ))
  }
  fn setup_observation_feeds(
    max: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError>;
  /// Prepare published feeds with maximum encoded host key, configuration and observation widths.
  fn setup_max_encoded_observation_feeds(
    _max: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkMaximumObservationEncodingUnsupported",
    ))
  }
  /// Prepare maximum-width observation reads, keeping populated neighbors for absent values.
  fn setup_observation_read_state(
    _max: u32,
    _state: BenchmarkObservationReadState,
    _max_age_blocks: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationReadStateUnsupported",
    ))
  }
  fn enable_asset_ops_ingress() {}
  /// Publish a sample through the host observation owner, including revision and Actor ingress.
  fn publish_observation(
    _feed: ObservationFeedId,
    _sample: u128,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationPublishUnsupported",
    ))
  }
  /// Make a feed unavailable through the host observation owner's lifecycle.
  fn deactivate_observation_feed(
    _feed: ObservationFeedId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationDeactivateUnsupported",
    ))
  }
  /// Begin the next fixture block and advance its authoritative clock to the requested tick.
  fn advance_to_scheduler_tick(_tick: u64) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkClockAdvanceUnsupported",
    ))
  }
  /// Supply quiet host context after clock setup for a real Mandatory Prepass fixture.
  /// This does not authenticate a complete consensus block.
  fn prepare_actor_prepass_context() -> polkadot_sdk::sp_runtime::DispatchResult {
    Ok(())
  }
  /// Finalize the host clock before a fixture advances to another block.
  fn finalize_scheduler_clock() -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkClockFinalizeUnsupported",
    ))
  }
  /// Encoded Contract-head witness only; collection does not require a published observation.
  fn contract_head_observation_feed()
  -> Result<ObservationFeedId, polkadot_sdk::sp_runtime::DispatchError> {
    Self::setup_observation_feeds(1)?.into_iter().next().ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("BenchmarkContractHeadFeedUnsupported"),
    )
  }
  /// Move custody through the host's signed-transfer producer, including certified ingress.
  fn transfer_signed(
    _source: &AccountId,
    _recipient: &AccountId,
    _asset: AssetId,
    _amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkSignedTransferUnsupported",
    ))
  }
  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_address_event_ingress(recipient: &AccountId, source: &AccountId, amount: Balance) -> bool;
  fn setup_xcm_asset_deposit() -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  type MaximumContextInherent;
  fn prepare_maximum_context_inherent() -> Self::MaximumContextInherent;
  fn execute_maximum_context_inherent(
    inherent: Self::MaximumContextInherent,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn verify_maximum_context_inherent();
  fn prepare_maximum_xcm_version_discovery();
  fn execute_maximum_xcm_version_discovery();
  fn verify_maximum_xcm_version_discovery();
  fn prepare_block_resource_meter_extension();
  fn execute_block_resource_meter_extension();
  fn verify_block_resource_meter_extension();
}

pub trait FeeCollector<AccountId, AssetId, Balance> {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeEnvelopeInput<Balance> {
  pub evaluation: Balance,
  pub execution: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepFeeEnvelope<Balance> {
  pub evaluation: Balance,
  pub execution: Balance,
  pub total: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerFeeBreakdown<Balance> {
  pub trigger_family: types::TriggerFamily,
  pub trigger_fee: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineFeeBreakdown<Balance> {
  pub pipeline_machine_fee: Balance,
  pub total_fee: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepFeeBreakdown<Balance> {
  pub control_fee: Balance,
  pub effect_fee: Balance,
  pub total_fee: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeChargeKind {
  EvaluationOnly,
  Attempted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeStepSettlement<Balance> {
  pub charged: Balance,
  pub reservation_remaining: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptFeeEnvelope<Balance, MaxSteps: Get<u32>> {
  pub steps: BoundedVec<StepFeeEnvelope<Balance>, MaxSteps>,
  pub total: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeEnvelopeError {
  CursorOutOfBounds,
  Overflow,
  ReservationUnderflow,
}

pub fn compose_attempt_fee_envelope<Balance, MaxSteps>(
  actor_type: types::ActorType,
  inputs: &BoundedVec<FeeEnvelopeInput<Balance>, MaxSteps>,
  start_cursor: usize,
) -> Result<AttemptFeeEnvelope<Balance, MaxSteps>, FeeEnvelopeError>
where
  Balance: Copy + CheckedAdd + Zero,
  MaxSteps: Get<u32>,
{
  if start_cursor > inputs.len() {
    return Err(FeeEnvelopeError::CursorOutOfBounds);
  }
  let mut steps = BoundedVec::default();
  let mut total = Balance::zero();
  for index in start_cursor..inputs.len() {
    let input = &inputs[index];
    let evaluation = if actor_type == types::ActorType::User {
      input.evaluation
    } else {
      Balance::zero()
    };
    let execution = if actor_type == types::ActorType::User {
      input.execution
    } else {
      Balance::zero()
    };
    let step_total = evaluation
      .checked_add(&execution)
      .ok_or(FeeEnvelopeError::Overflow)?;
    total = total
      .checked_add(&step_total)
      .ok_or(FeeEnvelopeError::Overflow)?;
    steps
      .try_push(StepFeeEnvelope {
        evaluation,
        execution,
        total: step_total,
      })
      .map_err(|_| FeeEnvelopeError::Overflow)?;
  }
  Ok(AttemptFeeEnvelope { steps, total })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeAssetClass {
  FeeNative,
  Other,
}

/// Returns the preserve-spend floor for a direct or adapter-reported debit surface.
pub fn fee_native_protected_minimum<Balance: Ord>(
  actor_type: types::ActorType,
  asset_class: FeeAssetClass,
  asset_minimum: Balance,
  min_user_balance: Balance,
) -> Balance {
  if actor_type == types::ActorType::User && asset_class == FeeAssetClass::FeeNative {
    min_user_balance
  } else {
    asset_minimum
  }
}

/// Settles one admitted fee-envelope step without touching host balances.
///
/// User reservation always releases the step's full upper bound before charging either the
/// evaluation-only or attempted-step amount. System Actors remains fee-exempt.
pub fn settle_attempt_fee_step<Balance>(
  actor_type: types::ActorType,
  reservation: Balance,
  step: &StepFeeEnvelope<Balance>,
  charge_kind: FeeChargeKind,
) -> Result<FeeStepSettlement<Balance>, FeeEnvelopeError>
where
  Balance: Copy + CheckedSub + Zero,
{
  if actor_type == types::ActorType::System {
    return Ok(FeeStepSettlement {
      charged: Balance::zero(),
      reservation_remaining: Balance::zero(),
    });
  }
  let reservation_remaining = reservation
    .checked_sub(&step.total)
    .ok_or(FeeEnvelopeError::ReservationUnderflow)?;
  let charged = match charge_kind {
    FeeChargeKind::EvaluationOnly => step.evaluation,
    FeeChargeKind::Attempted => step.total,
  };
  Ok(FeeStepSettlement {
    charged,
    reservation_remaining,
  })
}

pub(crate) const MAX_CONTRACT_STEPS_HARD_LIMIT: u32 = u8::MAX as u32;

pub(crate) const fn contract_steps_bound_is_valid(bound: u32) -> bool {
  bound > 0 && bound <= MAX_CONTRACT_STEPS_HARD_LIMIT
}

sp_api::decl_runtime_apis! {
  #[api_version(2)]
  pub trait ActorSimulationApi<Contract, Simulation>
  where
    Contract: codec::Codec,
    Simulation: codec::Codec,
  {
    fn simulate_current_contract(
      actor_id: types::ActorId,
      expected_type: types::ActorType,
      expected_mutability: types::Mutability,
      expected_contract: Contract,
      mode: types::SimulationMode,
      budget: types::SimulationBudget,
    ) -> Result<Simulation, types::SimulationError>;
  }

  /// Read-only named Actors cost projection with independent fee and hold provenance.
  pub trait ActorCostApi<Balance>
  where
    Balance: codec::Codec,
  {
    fn actor_cost_quote(
      actor_id: types::ActorId,
    ) -> Result<types::ActorCostQuote<Balance>, types::ActorCostQuoteError>;
  }

  /// Bounded machine-control flags, current resources and finalized resource projection.
  #[api_version(2)]
  pub trait ActorResourceApi<BlockNumber>
  where
    BlockNumber: codec::Codec,
  {
    fn machine_control_flags() -> types::ActorMachineControlFlags;

    fn block_resource_budget() -> types::BlockResourceBudget;

    fn current_block_resource_state() -> Option<types::BlockResourceState<BlockNumber>>;

    fn finalized_block_resource_snapshot(
    ) -> Option<types::FinalizedBlockResourceSnapshot<BlockNumber>>;
  }

  /// Read-only eligibility projection for one Actor, including its canonical residence (§3.1).
  ///
  /// Returns absence/dormancy or the canonical Active classification, reusing
  /// the same pure owners as
  /// admission so clients do not reimplement cadence phase, cooldown, window
  /// floor, retry backoff, breaker, or latch arithmetic.
  #[api_version(8)]
  pub trait ActorEligibilityApi<BlockNumber, Balance>
  where
    BlockNumber: codec::Codec,
    Balance: codec::Codec,
  {
    fn actor_eligibility(
      actor_id: types::ActorId,
    ) -> Result<types::ActorEligibility<BlockNumber, Balance>, types::ActorClassificationError>;
  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    ACTOR_PREPASS_INHERENT_VERSION, ActorPrepassContext, AdmissionCertificateAuthorityProvider,
    AssetOps, AttemptFeeEnvelope, BehaviorControlWeightContext,
    BehaviorServiceInspectionWeightContext, BlockResourceBudgetProvider,
    CanonicalActorPrepassInherentData, DexOps, FeeCollector, FeeEnvelopeError, FeeEnvelopeInput,
    FundingAuthority, LiquidityOps, ObservationProvider, PipelineControlWeightProvider,
    PipelineFeeBreakdown, StepControlWeightContext, StepControlWeightProvider, StepFeeBreakdown,
    TaskEffectWeightProvider, TriggerFeeBreakdown, WeightInfo, compose_attempt_fee_envelope,
    contract_steps_bound_is_valid,
  };
  use crate::adapters::{
    FinancialWaitBackingProvider as _, RetryClass, SovereignAccountDeriver as _,
    SovereignAccountPolicy, SystemActorContractValidator as _,
  };
  use crate::scheduler::{CyclePass, ServiceCutoff};
  use alloc::vec::Vec;
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{
      PalletId,
      traits::{
        EnsureOrigin, Time,
        fungible::{InspectHold, MutateHold},
        tokens::Precision,
      },
    },
    sp_inherents::{InherentData, InherentIdentifier, IsFatalError},
    sp_runtime::traits::{
      CheckedAdd, CheckedMul, CheckedSub, One, SaturatedConversion, Saturating, Zero,
    },
    sp_weights::{WeightMeter, WeightToFee as _},
  };

  pub const ACTOR_PREPASS_INHERENT_IDENTIFIER: InherentIdentifier = *b"deosact0";

  #[derive(codec::Encode, Debug)]
  pub enum ActorPrepassInherentError {
    MissingData,
    MissingCall,
    UnsupportedVersion,
  }

  impl IsFatalError for ActorPrepassInherentError {
    fn is_fatal_error(&self) -> bool {
      true
    }
  }

  use super::types::Task as ActorTask;
  pub use super::types::*;

  #[pallet::composite_enum]
  pub enum HoldReason {
    /// Refundable backing for retained User Actor process state.
    #[codec(index = 0)]
    ActorState,
  }

  #[pallet::config]
  pub trait Config: frame_system::Config {
    type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen + Ord;

    type Balance: Parameter
      + Member
      + AtLeast32BitUnsigned
      + Default
      + Copy
      + MaybeSerializeDeserialize
      + MaxEncodedLen;

    #[pallet::constant]
    type FeeNativeAssetId: Get<Self::AssetId>;

    type AssetOps: AssetOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type AdmissionCertificateAuthority: AdmissionCertificateAuthorityProvider;
    type StepControlWeight: StepControlWeightProvider<StepOf<Self>>;
    /// Prepared tariff declaration only; live fee compilation and authority do not consume it yet.
    type PipelineControlWeight: PipelineControlWeightProvider<ActorContractOf<Self>>;
    type TaskEffectWeight: TaskEffectWeightProvider<TaskOf<Self>>;
    type ObservationFeedId: Parameter + Member + Copy + MaxEncodedLen + Ord;
    type ObservationProvider: ObservationProvider<Self::ObservationFeedId, BlockNumberFor<Self>>;
    type FundingAuthority: FundingAuthority<Self::AccountId>;
    type SovereignAccountDeriver: crate::adapters::SovereignAccountDeriver<Self::AccountId>;
    type SovereignAccountPolicy: crate::adapters::SovereignAccountPolicy<Self::AccountId>;
    type DexOps: DexOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type StakingOps: crate::adapters::StakingOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type LiquidityOps: LiquidityOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type Time: Time<Moment = u64>;

    #[pallet::constant]
    type CadenceTickMillis: Get<u64>;
    #[pallet::constant]
    type MinWindowLength: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type PalletId: Get<PalletId>;

    type SystemOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type GlobalBreakerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    #[pallet::constant]
    type MaxContractSteps: Get<u32>;
    /// Maximum exact asset balances in one parked-balance activation and dependency plan.
    #[pallet::constant]
    type MaxParkedBalanceWatches: Get<u32>;
    #[pallet::constant]
    type MaxBehaviorClauses: Get<u32>;
    #[pallet::constant]
    type MaxBehaviorAtomsPerClause: Get<u32>;
    #[pallet::constant]
    type MaxFundingTrackedAssets: Get<u32>;
    #[pallet::constant]
    type MaxPreconditionClauses: Get<u32>;
    #[pallet::constant]
    type MaxPredicatesPerClause: Get<u32>;
    #[pallet::constant]
    type MaxPredicatesPerStep: Get<u32>;
    #[pallet::constant]
    type MaxOwnerSlots: Get<u8>;
    #[pallet::constant]
    type MaxExecutionsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxQueueLength: Get<u32>;
    /// Physical I/O granularity for the monotonic active FIFO.
    #[pallet::constant]
    type QueuePageSize: Get<u32>;
    /// Physical I/O granularity for the paged temporal wakeup index.
    #[pallet::constant]
    type WakeupPageSize: Get<u32>;
    /// Independent ceiling for physical queue-entry inspection per scheduler pass.
    #[pallet::constant]
    type MaxQueueEntriesScannedPerBlock: Get<u32>;
    /// Hard two-dimensional ceiling for the overdue wakeup worker. The worker also remains
    /// bounded by the actual on_idle budget left after fixed base and saturated queue cleanup,
    /// then leaves the remainder for actor service.
    #[pallet::constant]
    type WakeupWeightLimit: Get<Weight>;
    #[pallet::constant]
    type MaxWakeupsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxSweepBatch: Get<u32>;
    #[pallet::constant]
    type MaxWhitelistSize: Get<u32>;
    /// Target block duration in whole seconds.
    #[pallet::constant]
    type TargetBlockTime: Get<u64>;
    #[pallet::constant]
    type MaxExecutionDelayBlocks: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type MaxTemporalDelayTicks: Get<SchedulerTick>;
    #[pallet::constant]
    type MaxIdleStarvationBlocks: Get<u32>;
    /// Gross two-dimensional `on_idle` weight guaranteed by the embedding runtime.
    #[pallet::constant]
    type ActorOnIdleReserve: Get<Weight>;
    #[pallet::constant]
    type MaxAutoCloseNonceHorizon: Get<u64>;
    /// Maximum number of active Actors instances. Bounds the BTreeSet storage.
    /// Set to 10,000 for production use cases.
    #[pallet::constant]
    type MaxActiveActors: Get<u32>;
    /// Hard cap across active and dormant actor identities.
    #[pallet::constant]
    type MaxActorIdentities: Get<u32>;
    /// Lifetime cap on allocated System custody locators, including vacant locators.
    #[pallet::constant]
    type MaxSystemSovereigns: Get<u32>;

    #[pallet::constant]
    type ActorCreationFee: Get<Self::Balance>;
    type RuntimeHoldReason: Parameter + Member + MaxEncodedLen + Copy + From<HoldReason>;
    type StateHoldCurrency: InspectHold<Self::AccountId, Balance = Self::Balance, Reason = Self::RuntimeHoldReason>
      + MutateHold<Self::AccountId>;
    /// Fixed accounting price for each present retained-state component.
    #[pallet::constant]
    type ActorStateHoldBase: Get<Self::Balance>;
    /// Linear accounting price for each SCALE-encoded retained byte.
    #[pallet::constant]
    type ActorStateHoldPerByte: Get<Self::Balance>;
    /// Converts weight to fee for execution cost calculation
    type WeightToFee: polkadot_sdk::sp_weights::WeightToFee<Balance = Self::Balance>;
    /// Runtime-bound upper weights for every Actors task variant
    type FeeSink: Get<Self::AccountId>;
    type FeeCollector: FeeCollector<Self::AccountId, Self::AssetId, Self::Balance>;
    /// Installation-paid backing for a User's one potentially unpaid AtTime occurrence.
    type FinancialWaitBacking: crate::FinancialWaitBackingProvider<Self::AccountId, Self::Balance>;
    #[pallet::constant]
    type MaxConsecutiveFailures: Get<u32>;
    #[pallet::constant]
    type MaxRetryAttempts: Get<u32>;
    #[pallet::constant]
    type MinUserBalance: Get<Self::Balance>;

    type WeightInfo: WeightInfo;

    /// Configured guarantees plus the one host allocation requested after Prepass context validation.
    type BlockResourceBudget: BlockResourceBudgetProvider<BlockNumberFor<Self>>;

    type PrepassContext: ActorPrepassContext;

    /// Provides System Actors specs to initialize at genesis.
    /// Use `()` for no genesis System Actors (default).
    type GenesisSystemActors: GenesisSystemActors<Self::AccountId, ActorContractOf<Self>>;
    /// Host policy for bounded System Actor effect topology. User Actor
    /// contracts intentionally remain outside this reference-runtime DAG.
    type SystemActorContractValidator: crate::SystemActorContractValidator<ActorContractOf<Self>>;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance, Self::ObservationFeedId>;
  }

  pub type BalanceOf<T> = <T as Config>::Balance;
  pub type AssetIdOf<T> = <T as Config>::AssetId;

  pub type SourceFilterOf<T> =
    SourceFilter<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type AssetFilterOf<T> = AssetFilter<<T as Config>::AssetId, <T as Config>::MaxWhitelistSize>;

  #[derive(Clone, Copy)]
  pub(crate) enum TriggerTransitionIntent {
    GenesisInstallation,
    CreateActive,
    ActivateDormant,
  }

  pub type TriggerOf<T> = Trigger<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
  >;

  pub type ParkedBalanceActivationOf<T> = ParkedBalanceActivation<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxParkedBalanceWatches,
  >;

  pub type ParkedBalanceWatchesOf<T> = BoundedVec<
    ParkedBalanceWatch<<T as Config>::AssetId, <T as Config>::Balance>,
    <T as Config>::MaxParkedBalanceWatches,
  >;

  pub type ParkedBalanceEpisodeOf<T> = ParkedBalanceEpisode<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxParkedBalanceWatches,
  >;

  pub type PreconditionOf<T> = Precondition<
    Predicate<
      <T as Config>::AssetId,
      <T as Config>::Balance,
      u32,
      <T as Config>::ObservationFeedId,
    >,
    <T as Config>::MaxPreconditionClauses,
    <T as Config>::MaxPredicatesPerClause,
  >;

  pub type TaskOf<T> = super::types::Task<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
  >;

  pub type StepOf<T> = Step<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as Config>::MaxPreconditionClauses,
    <T as Config>::MaxPredicatesPerClause,
    <T as Config>::ObservationFeedId,
  >;

  pub type ContractSteps<T> = BoundedVec<StepOf<T>, <T as Config>::MaxContractSteps>;

  pub type AttemptFeeEnvelopeOf<T> =
    AttemptFeeEnvelope<BalanceOf<T>, <T as Config>::MaxContractSteps>;

  pub type FundingSourcePolicyOf<T> =
    FundingSourcePolicy<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type ActorContractOf<T> = super::types::ActorContract<
    TriggerOf<T>,
    BlockNumberFor<T>,
    ContractSteps<T>,
    FundingSourcePolicyOf<T>,
    ParkedBalanceActivationOf<T>,
    CloseWhenOf<T>,
  >;

  pub type CloseWhenOf<T> = super::types::CloseWhen<
    <T as Config>::MaxBehaviorClauses,
    <T as Config>::MaxBehaviorAtomsPerClause,
  >;

  pub type ActorContractHeaderOf<T> = super::types::ActorContractHeader<
    TriggerOf<T>,
    BlockNumberFor<T>,
    FundingSourcePolicyOf<T>,
    ParkedBalanceActivationOf<T>,
    <T as Config>::Balance,
    [u8; 32],
  >;

  pub type ActorBehaviorPolicyOf<T> = super::types::ActorBehaviorPolicy<
    <T as Config>::MaxBehaviorClauses,
    <T as Config>::MaxBehaviorAtomsPerClause,
  >;

  type PreparedBehaviorStateOf<T> = (
    Option<ActorBehaviorPolicyOf<T>>,
    Option<ActorBehaviorMonitor>,
    bool,
  );

  pub(crate) type LoadedBehaviorPolicyOf<T> = LoadedBehaviorPolicy<ActorBehaviorPolicyOf<T>>;
  pub(crate) type BehaviorPolicyCandidateOf<T> = BehaviorPolicyCandidate<ActorBehaviorPolicyOf<T>>;

  pub type ActorContractHeadOf<T> =
    super::types::ActorContractHead<ActorContractHeaderOf<T>, StepOf<T>>;

  pub type ActorStepChunkOf<T> = super::types::ActorStepChunk<
    ActorId,
    [u8; 32],
    BoundedVec<StepOf<T>, ConstU32<{ super::types::MAX_STEPS_PER_TAIL_CHUNK }>>,
    BoundedVec<ActorStepResourceEnvelope, ConstU32<{ super::types::MAX_STEPS_PER_TAIL_CHUNK }>>,
  >;

  pub type ActorAdmissionResourcesOf<T> =
    BoundedVec<ActorStepResourceEnvelope, <T as Config>::MaxContractSteps>;

  pub type ActorAdmissionCertificateOf<T> = ActorAdmissionCertificate<ActorAdmissionResourcesOf<T>>;

  pub type ActorStepTicketOf<T> =
    ActorStepTicket<BlockNumberFor<T>, ActorContractCommitment<[u8; 32]>>;

  pub type ActorStepAuthorityOf<T> =
    ActorStepAuthority<BlockNumberFor<T>, ActorContractCommitment<[u8; 32]>>;

  pub type LoadedActorStepOf<T> = LoadedActorStep<StepOf<T>>;

  pub type CurrentStepPlanOf<T> = StepExecutionPlan<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorRunStateOf<T>,
    ActorAdmissionCertificateOf<T>,
    ActorStepAuthorityOf<T>,
    LoadedActorStepOf<T>,
    StepFeeBreakdown<<T as Config>::Balance>,
  >;

  pub type ActorRunStateOf<T> = ActorRunState<BlockNumberFor<T>, AssetIdOf<T>, BalanceOf<T>>;

  pub type QueuePageOf<T> = BoundedVec<QueueEntry<BlockNumberFor<T>>, <T as Config>::QueuePageSize>;

  /// Canonical movable identity authority. Sovereign account is derived from owner/class and is not
  /// duplicated in every control cell.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlIdentity<AccountId, BlockNumber> {
    pub owner: AccountId,
    pub actor_class: ActorClass,
    pub mutability: Mutability,
    pub cycle_nonce: u64,
    pub last_control_mutation_block: BlockNumber,
  }

  /// Canonical mutable control authority. Ready ticket is represented by physical location; both
  /// wakeup pointers remain explicit because process and Trigger temporal memberships can coexist.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlHotState<BlockNumber> {
    pub lifecycle: ActiveLifecycle,
    pub cycle_state: CycleState,
    pub trigger_runtime_state: TriggerRuntimeState,
    pub unsuccessful_attempt_streak: u32,
    pub pending_signal: bool,
    pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
    pub trigger_wakeup_pointer: Option<TriggerWakeupPointer>,
    pub terminal_at: Option<BlockNumber>,
    pub schedule_anchor: BlockNumber,
    pub last_cycle_block: Option<BlockNumber>,
  }

  /// Non-placement semantic authority prepared for the atomic scheduler cutover. Cursor,
  /// eligibility, and current-Step resources are intentionally absent: the run record owns the
  /// first two while contract geometry at that cursor owns the last.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorSemanticRecord<Identity, Hot, Admission> {
    pub identity: Identity,
    /// Stable Contract-generation authority used by every generation-bound carrier.
    pub generation: u64,
    pub hot: Hot,
    pub admission: Admission,
  }

  /// Dormant semantic authority preserves the last published Contract generation. Generation zero
  /// means that this identity has never published a Contract.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct DormantActorSemanticRecord<Identity> {
    pub identity: Identity,
    pub generation: u64,
  }

  /// Complete lifecycle shape for the canonical actor-keyed semantic owner. Active zero-Step Actors
  /// still use `Active` because they retain generation, hot, and admission semantics even though
  /// execution projection has no current Step.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub enum ActorSemanticState<Identity, Hot, Admission> {
    Dormant(DormantActorSemanticRecord<Identity>),
    Active(ActorSemanticRecord<Identity, Hot, Admission>),
  }

  pub fn next_actor_generation(current: u64) -> Option<u64> {
    current.checked_add(1).filter(|generation| *generation != 0)
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticLoadError {
    Corrupt,
  }

  /// Complete storage-neutral operation set for the canonical actor-keyed semantic owner. Every
  /// update is a compare-and-replace of the whole bounded record, so independently authored field
  /// patches cannot silently overwrite one another. Placement-only transitions need no operation.
  #[derive(Clone, Debug, Eq, PartialEq)]
  pub enum ActorSemanticMutation<Record> {
    Publish(Record),
    Replace {
      expected: Record,
      replacement: Record,
    },
    Remove {
      expected: Record,
    },
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticMutationError {
    AlreadyPublished,
    Missing,
    Stale,
  }

  pub fn apply_actor_semantic_mutation<Record: Clone + Eq>(
    current: Option<&Record>,
    mutation: &ActorSemanticMutation<Record>,
  ) -> Result<Option<Record>, ActorSemanticMutationError> {
    match (current, mutation) {
      (None, ActorSemanticMutation::Publish(record)) => Ok(Some(record.clone())),
      (Some(_), ActorSemanticMutation::Publish(_)) => {
        Err(ActorSemanticMutationError::AlreadyPublished)
      }
      (
        Some(current),
        ActorSemanticMutation::Replace {
          expected,
          replacement,
        },
      ) if current == expected => Ok(Some(replacement.clone())),
      (Some(current), ActorSemanticMutation::Remove { expected }) if current == expected => {
        Ok(None)
      }
      (None, ActorSemanticMutation::Replace { .. } | ActorSemanticMutation::Remove { .. }) => {
        Err(ActorSemanticMutationError::Missing)
      }
      (Some(_), ActorSemanticMutation::Replace { .. } | ActorSemanticMutation::Remove { .. }) => {
        Err(ActorSemanticMutationError::Stale)
      }
    }
  }

  /// Storage-free projection of fields currently duplicated by placement cells.
  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub struct ActorSemanticExecutionProjection<BlockNumber> {
    pub cursor: u32,
    pub eligible_at: Option<BlockNumber>,
    pub resources: ActorStepResourceEnvelope,
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticProjectionError {
    RunStateMismatch,
    CurrentStepMissing,
  }

  /// Projects execution fields from their existing canonical owners without making the semantic
  /// record a second cursor or resource authority.
  pub fn project_actor_semantic_execution<BlockNumber: Copy>(
    cycle_state: CycleState,
    run: Option<(u32, BlockNumber)>,
    current_step_resources: Option<ActorStepResourceEnvelope>,
  ) -> Result<ActorSemanticExecutionProjection<BlockNumber>, ActorSemanticProjectionError> {
    let (cursor, eligible_at) = match (cycle_state, run) {
      (CycleState::Idle, None) => (0, None),
      (CycleState::Running | CycleState::Suspended, Some((cursor, eligible_at))) => {
        (cursor, Some(eligible_at))
      }
      _ => return Err(ActorSemanticProjectionError::RunStateMismatch),
    };
    let resources =
      current_step_resources.ok_or(ActorSemanticProjectionError::CurrentStepMissing)?;
    Ok(ActorSemanticExecutionProjection {
      cursor,
      eligible_at,
      resources,
    })
  }

  /// Canonical single-owner control cell.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlCell<AccountId, BlockNumber, Admission> {
    pub actor_id: ActorId,
    pub identity: ActorControlIdentity<AccountId, BlockNumber>,
    pub hot: ActorControlHotState<BlockNumber>,
    pub pipeline_service_identity: [u8; 32],
    pub cursor: u32,
    pub eligible_at: Option<BlockNumber>,
    pub admission: Admission,
    pub resources: ActorStepResourceEnvelope,
  }

  pub type DormantActorSemanticRecordOf<T> = DormantActorSemanticRecord<ActorIdentityOf<T>>;
  pub type ActorSemanticRecordOf<T> =
    ActorSemanticRecord<ActorIdentityOf<T>, ActorHotStateOf<T>, ActorAdmissionCertificateOf<T>>;
  pub type ActorSemanticStateOf<T> =
    ActorSemanticState<ActorIdentityOf<T>, ActorHotStateOf<T>, ActorAdmissionCertificateOf<T>>;

  pub type ActorControlCellOf<T> = ActorControlCell<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    ActorAdmissionCertificateOf<T>,
  >;
  pub type ActorControlChunkOf<T> = BoundedVec<Option<ActorControlCellOf<T>>, ConstU32<32>>;

  /// A deadline membership points at the single control owner without copying mutable authority.
  /// Its clock is the containing Waiting key; the primary must carry the exact matching pointer.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorWakeupReference {
    pub actor_id: ActorId,
    pub admission_identity: [u8; 32],
  }

  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub enum ActorWaitingEntry<Cell> {
    Primary(Cell),
    Reference(ActorWakeupReference),
  }

  impl<Cell> ActorWaitingEntry<Cell> {
    pub fn primary(&self) -> Option<&Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }

    pub fn primary_mut(&mut self) -> Option<&mut Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }

    pub fn into_primary(self) -> Option<Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }
  }

  pub type ActorWaitingChunkOf<T> =
    BoundedVec<Option<ActorWaitingEntry<ActorControlCellOf<T>>>, ConstU32<32>>;
  pub type ActorWaitingPageOf<T> = WakeupPage<ActorWaitingChunkOf<T>>;
  pub type DeadlineHandleOf<T> = DeadlineHandle<BlockNumberFor<T>>;
  pub type DeadlineIndexPageOf<T> = BoundedVec<WakeupKey<BlockNumberFor<T>>, ConstU32<32>>;

  /// External-boundary location; execution writes but never reads this index.
  #[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    Eq,
    MaxEncodedLen,
    PartialEq,
    TypeInfo,
  )]
  pub enum ActorControlLocation<BlockNumber> {
    Unsignaled,
    Waiting {
      key: WakeupKey<BlockNumber>,
      page: u64,
      slot: u8,
    },
    Ready {
      ticket: QueueTicket,
    },
  }

  // Temporary source aliases remain only while the atomic cutover is assembled in this
  // diagnostic worktree; the retained production patch removes them after caller conversion.

  pub type WakeupPageEntriesOf<T> = BoundedVec<Option<WakeupEntry>, <T as Config>::WakeupPageSize>;
  pub type WakeupPageOf<T> = WakeupPage<WakeupPageEntriesOf<T>>;
  pub type WakeupCursorPageOf<T> =
    BoundedVec<WakeupKey<BlockNumberFor<T>>, <T as Config>::WakeupPageSize>;

  pub(crate) type ActiveActorViewOf<T> = ActiveActorView<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    TriggerOf<T>,
    ContractSteps<T>,
  >;

  pub type ActorHotStateOf<T> = ActorHotState<BlockNumberFor<T>>;
  pub type ActorProcessOf<T> = ActorProcess<BlockNumberFor<T>>;

  pub type ActorIdentityOf<T> =
    ActorIdentity<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

  pub type ActorStateHoldBreakdownOf<T> = ActorStateHoldBreakdown<<T as Config>::Balance>;
  pub type ActorStateHoldRecordOf<T> =
    ActorStateHoldRecord<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

  pub type ActiveActorStateOf<T> = ActiveActorState<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorContractOf<T>,
    ActorRunStateOf<T>,
  >;

  pub(crate) enum LoadedActorStateOf<T: Config> {
    NotRegistered,
    Dormant(ActorIdentityOf<T>),
    Active(ActiveActorStateOf<T>),
    Corrupt,
  }

  pub(crate) enum CheckedCloseAuthorityOf<T: Config> {
    Active(ActiveActorViewOf<T>),
    Dormant(ActorIdentityOf<T>),
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(15);

  /// FRAME map-population metadata derived from an existing consensus capacity owner.
  pub struct StorageValueCountLimit<Limit>(core::marker::PhantomData<Limit>);

  impl<Limit: Get<u32>> Get<Option<u32>> for StorageValueCountLimit<Limit> {
    fn get() -> Option<u32> {
      Some(Limit::get())
    }
  }

  /// One bounded tail per chunk for each active Contract; unrepresentable counts stay unknown.
  pub struct ContractTailStorageValues<ActorLimit, StepLimit>(
    core::marker::PhantomData<(ActorLimit, StepLimit)>,
  );

  impl<ActorLimit: Get<u32>, StepLimit: Get<u32>> Get<Option<u32>>
    for ContractTailStorageValues<ActorLimit, StepLimit>
  {
    fn get() -> Option<u32> {
      ActorLimit::get().checked_mul(
        StepLimit::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
      )
    }
  }

  /// Current parked registrations bound sources/pages; each retained page has at most 31 holes.
  pub struct DependencyStorageValues<ActorLimit, WatchLimit, const FACTOR: u32>(
    core::marker::PhantomData<(ActorLimit, WatchLimit)>,
  );

  impl<ActorLimit: Get<u32>, WatchLimit: Get<u32>, const FACTOR: u32> Get<Option<u32>>
    for DependencyStorageValues<ActorLimit, WatchLimit, FACTOR>
  {
    fn get() -> Option<u32> {
      ActorLimit::get()
        .checked_mul(WatchLimit::get())?
        .checked_mul(FACTOR)
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn next_actor_id)]
  pub type NextActorId<T> = StorageValue<_, ActorId, ValueQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorContractHead"]
  pub type ActorContractHeads<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorContractHeadOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  #[pallet::storage]
  pub type ActorBehaviorPolicies<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorBehaviorPolicyOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  #[pallet::storage]
  pub type ActorBehaviorMonitors<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorBehaviorMonitor,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  /// One immutable episode owned by an AwaitingFunding process in Deadline or tentative Service.
  #[pallet::storage]
  pub type ActorFinancialWaits<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    FinancialWaitEpisode<T::Balance>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorContractTailChunk"]
  pub type ActorContractTailChunks<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    ActorId,
    Blake2_128Concat,
    u32,
    ActorStepChunkOf<T>,
    OptionQuery,
    GetDefault,
    ContractTailStorageValues<T::MaxActiveActors, T::MaxContractSteps>,
  >;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorRunState"]
  pub type ActorRunStateStore<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorRunStateOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  impl<T: Config> Pallet<T> {
    fn financial_wait_geometry_qualified(policy: OnUnfunded) -> bool {
      policy == OnUnfunded::Close
        || Self::financial_wait_episode_bytes().is_some_and(|bytes| {
          T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes).is_some()
            && T::AdmissionCertificateAuthority::financial_wait_close_additional_weight(bytes)
              .is_some()
        })
    }

    fn financial_wait_episode_bytes() -> Option<u32> {
      u32::try_from(FinancialWaitEpisode::<T::Balance>::max_encoded_len()).ok()
    }

    /// Authenticates the owed boundary against current source authority, without changing the Run,
    /// charging its unpaid service or deriving a new Cycle.
    fn financial_wait_source(
      actor: ActorRef,
      semantic: &ActorSemanticRecordOf<T>,
      unpaid: UnpaidActorService,
    ) -> Option<(FinancialWaitScope, T::Balance)> {
      Self::financial_wait_source_with_retained_liability(actor, semantic, unpaid, None)
    }

    /// Retained liability authenticates an existing stale carrier for inspection/retirement;
    /// it never authorizes entry, recovery or an Attempt under old runtime prices.
    fn financial_wait_source_with_retained_liability(
      actor: ActorRef,
      semantic: &ActorSemanticRecordOf<T>,
      unpaid: UnpaidActorService,
      retained_liability: Option<T::Balance>,
    ) -> Option<(FinancialWaitScope, T::Balance)> {
      let head = ActorContractHeads::<T>::get(actor.actor_id)?;
      let admission = &semantic.admission;
      if semantic.generation != actor.generation
        || semantic.identity.actor_class.actor_type() != ActorType::User
        || semantic.hot.lifecycle.is_paused()
        || head.header.on_unfunded != OnUnfunded::AwaitFunding
        || (retained_liability.is_none()
          && !Self::financial_wait_geometry_qualified(head.header.on_unfunded))
        || !admission.has_valid_identity()
        || admission.semantic_contract_id != head.header.semantic_contract_id
        || admission.body_commitment != head.header.body_commitment
        || admission.admission_identity != head.header.admission_identity
        || (retained_liability.is_none() && !Self::admission_matches_current_authority(admission))
      {
        return None;
      }
      let behavior_episode =
        match Self::load_contract_behavior_policy(actor.actor_id, &head.header)? {
          Some(policy) => {
            Self::load_contract_behavior_monitor(actor.actor_id, &head.header, &policy)?;
            Some(Self::behavior_episode_identity(
              actor.actor_id,
              &policy,
              head.header.semantic_contract_id,
            ))
          }
          None => None,
        };
      let scope = FinancialWaitScope {
        actor,
        actor_type: ActorType::User,
        contract_authority: ActorRunAuthority {
          semantic_contract_id: admission.semantic_contract_id,
          body_commitment: admission.body_commitment,
          admission_identity: admission.admission_identity,
          pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        },
        behavior_episode,
      };
      let run = ActorRunStateStore::<T>::get(actor.actor_id);
      let next_nonce = semantic.identity.cycle_nonce.checked_add(1)?;
      let liability = match unpaid {
        UnpaidActorService::AtTimeTrigger { due_tick } => {
          let Trigger::AtTime { after_ticks } = head.header.trigger else {
            return None;
          };
          let TriggerRuntimeState::AtTime {
            anchor_tick: Some(anchor),
            consumed: false,
          } = semantic.hot.trigger_runtime_state
          else {
            return None;
          };
          if semantic.hot.cycle_state != CycleState::Idle
            || semantic.hot.pending_signal
            || run.is_some()
            || anchor.checked_add(after_ticks)? != due_tick
          {
            return None;
          }
          retained_liability.or_else(|| {
            Self::trigger_fee_for_head(
              ActorType::User,
              &head,
              TriggerFamily::AtTime,
              T::WeightInfo::at_time_trigger_occurrence(),
            )
            .ok()
            .map(|fee| fee.trigger_fee)
          })?
        }
        UnpaidActorService::PipelineOpening { cycle_nonce } => {
          if semantic.hot.cycle_state != CycleState::Idle
            || !semantic.hot.pending_signal
            || run.is_some()
            || cycle_nonce != next_nonce
          {
            return None;
          }
          retained_liability.unwrap_or_else(|| {
            Self::pipeline_fee_breakdown(ActorType::User, head.header.pipeline_machine_envelope)
              .total_fee
          })
        }
        UnpaidActorService::RunningAction {
          cycle_nonce,
          cursor,
        }
        | UnpaidActorService::SuspendedAction {
          cycle_nonce,
          cursor,
          ..
        } => {
          let run = run?;
          let state_matches = match unpaid {
            UnpaidActorService::RunningAction { .. } => {
              semantic.hot.cycle_state == CycleState::Running && run.running_is_coherent()
            }
            UnpaidActorService::SuspendedAction {
              unsuccessful_attempts_at_cursor,
              ..
            } => {
              semantic.hot.cycle_state == CycleState::Suspended
                && semantic.identity.mutability == Mutability::Mutable
                && run.suspension_is_coherent()
                && run.unsuccessful_attempts_at_cursor == unsuccessful_attempts_at_cursor
            }
            _ => false,
          };
          if !state_matches
            || run.contract_authority != scope.contract_authority
            || cycle_nonce != next_nonce
            || run.cycle_nonce != cycle_nonce
            || run.cursor != cursor
          {
            return None;
          }
          let (step, resources) = if retained_liability.is_some() {
            let contract = Self::load_contract_geometry_with_admission(actor.actor_id, admission)?;
            (contract.steps.get(cursor as usize)?.clone(), None)
          } else {
            let loaded = Self::load_current_step_with_admission(actor.actor_id, cursor, admission)?;
            (loaded.step, Some(loaded.resources))
          };
          if matches!(unpaid, UnpaidActorService::SuspendedAction { .. }) {
            let maximum = step.on_error.retry_max_attempts()?;
            if run.unsuccessful_attempts_at_cursor == 0
              || run.unsuccessful_attempts_at_cursor >= maximum
            {
              return None;
            }
          }
          retained_liability.or_else(|| {
            Self::maximum_current_action_fee(ActorType::User, &step, resources?)
              .ok()
              .map(|fee| fee.total_fee)
          })?
        }
      };
      (!liability.is_zero()).then_some((scope, liability))
    }

    fn financial_wait_service_kind(unpaid: UnpaidActorService) -> ServiceResidenceKind {
      match unpaid {
        UnpaidActorService::PipelineOpening { .. } => ServiceResidenceKind::Pending,
        _ => ServiceResidenceKind::Live,
      }
    }

    /// Both due inspection and certified funded hints use one tentative B+1 return owner.
    /// The caller authenticates the original episode and current solvency in its transaction.
    pub(crate) fn return_financial_wait_to_service(
      actor: ActorRef,
      mut process: ActorProcessOf<T>,
      unpaid: UnpaidActorService,
    ) -> Result<(), FinancialWaitDueError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional()
        || process.generation != actor.generation
        || process.status != ProcessStatus::AwaitingFunding
        || !matches!(process.residence, Some(ProcessResidence::Deadline { .. }))
      {
        return Err(FinancialWaitDueError::InvalidSource);
      }
      Self::remove_deadline_member(actor).map_err(FinancialWaitDueError::Deadline)?;
      let kind = Self::financial_wait_service_kind(unpaid);
      process.residence = Some(ProcessResidence::Service(kind));
      ActorProcesses::<T>::insert(actor.actor_id, process);
      Self::insert_service_member(actor, kind, frame_system::Pallet::<T>::block_number())
        .map_err(FinancialWaitDueError::Service)
    }

    /// Additional certified producer bound, excluding its ordinary movement and ingress owners.
    /// Availability is not proof that a concrete producer has admitted or paid this maximum.
    pub fn financial_wait_ingress_weight_upper() -> Option<Weight> {
      let (ingress, cleanup) = Self::financial_wait_ingress_weights()?;
      ingress.checked_add(&cleanup)
    }

    pub(crate) fn financial_wait_ingress_weights() -> Option<(Weight, Weight)> {
      let ingress = T::AdmissionCertificateAuthority::financial_wait_ingress_weight(
        Self::financial_wait_episode_bytes()?,
      )?;
      let cleanup = T::AdmissionCertificateAuthority::current()?.maximum_lifecycle_weight;
      ingress.checked_add(&cleanup)?;
      Some((ingress, cleanup))
    }

    /// One producer return prepays its future financial inspection/repark, not another Action.
    pub(crate) fn financial_wait_return_fee(
      contract: &ActorContractOf<T>,
    ) -> Result<T::Balance, Error<T>> {
      let additional = T::AdmissionCertificateAuthority::financial_wait_service_weight(
        Self::financial_wait_episode_bytes().ok_or(Error::<T>::AdmissionBoundOverflow)?,
      )
      .ok_or(Error::<T>::AwaitFundingUnavailable)?;
      let weight = T::WeightInfo::service_round_begin_populated()
        .checked_add(&T::WeightInfo::service_round_probe_eligible())
        .and_then(|weight| weight.checked_add(&T::WeightInfo::scheduler_actor_state_probe()))
        .and_then(|weight| weight.checked_add(&additional))
        .and_then(|weight| weight.checked_add(&Self::policy_service_inspection_weight(contract)?))
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(if weight == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&weight)
      })
    }

    fn repark_financial_service(
      actor: ActorRef,
      episode: &FinancialWaitEpisode<T::Balance>,
    ) -> Result<(), FinancialWaitDueError> {
      let destination =
        Self::plan_deadline_destination(actor, WakeupKey::Tick(episode.record().expiry_tick()))
          .map_err(FinancialWaitDueError::Deadline)?;
      Self::remove_service_member(actor).map_err(FinancialWaitDueError::Service)?;
      ActorProcesses::<T>::try_mutate(actor.actor_id, |process| {
        let process = process
          .as_mut()
          .ok_or(FinancialWaitDueError::InvalidSource)?;
        process.residence = Some(ProcessResidence::Deadline {
          key: destination.key,
          page: destination.page,
          slot: destination.slot,
        });
        Ok::<_, FinancialWaitDueError>(())
      })?;
      Self::insert_deadline_member(destination).map_err(FinancialWaitDueError::Deadline)
    }

    #[cfg(test)]
    pub(crate) fn test_repark_financial_service(
      actor: ActorRef,
      episode: &FinancialWaitEpisode<T::Balance>,
    ) -> Result<(), FinancialWaitDueError> {
      Self::repark_financial_service(actor, episode)
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_repark_financial_service(
      actor: ActorRef,
      episode: &FinancialWaitEpisode<T::Balance>,
    ) -> Result<(), FinancialWaitDueError> {
      Self::repark_financial_service(actor, episode)
    }

    /// Provisional retirement belongs to the owning service transaction. Its commit proves the owed
    /// boundary committed or retired; any later refusal restores the original episode and source.
    pub(crate) fn prepare_financial_service_recovery(
      actor: ActorRef,
      semantic: &ActorSemanticRecordOf<T>,
    ) -> Result<Option<FinancialWaitReceipt>, crate::scheduler::AttemptTransactionError> {
      use crate::scheduler::AttemptTransactionError;
      let mut process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(AttemptTransactionError::Invariant)?;
      if process.status == ProcessStatus::Serving {
        return if ActorFinancialWaits::<T>::contains_key(actor.actor_id) {
          Err(AttemptTransactionError::Invariant)
        } else {
          Ok(None)
        };
      }
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional()
        || !matches!(process.residence, Some(ProcessResidence::Service(_)))
        || !Self::admission_matches_current_authority(&semantic.admission)
        || !Self::financial_wait_matches_process(actor, semantic, &process)
      {
        return Err(AttemptTransactionError::Invariant);
      }
      let episode =
        ActorFinancialWaits::<T>::get(actor.actor_id).ok_or(AttemptTransactionError::Invariant)?;
      let record = episode.record();
      if T::AssetOps::balance(
        &semantic.identity.sovereign_account,
        T::FeeNativeAssetId::get(),
      )
      .saturating_sub(T::MinUserBalance::get())
        < record.unpaid_liability()
      {
        return Err(AttemptTransactionError::Invariant);
      }
      let receipt = FinancialWaitReceipt {
        actor,
        episode_identity: episode.original_identity(),
        unpaid_service: record.unpaid_service(),
      };
      process.status = ProcessStatus::Serving;
      ActorProcesses::<T>::insert(actor.actor_id, process);
      ActorFinancialWaits::<T>::remove(actor.actor_id);
      Ok(Some(receipt))
    }

    fn financial_wait_matches_process(
      actor: ActorRef,
      semantic: &ActorSemanticRecordOf<T>,
      process: &ActorProcessOf<T>,
    ) -> bool {
      let Some(episode) = ActorFinancialWaits::<T>::get(actor.actor_id) else {
        return false;
      };
      let record = episode.record();
      let retained_liability = (!Self::admission_matches_current_authority(&semantic.admission))
        .then_some(record.unpaid_liability());
      let Some((scope, liability)) = Self::financial_wait_source_with_retained_liability(
        actor,
        semantic,
        record.unpaid_service(),
        retained_liability,
      ) else {
        return false;
      };
      process.status == ProcessStatus::AwaitingFunding
        && match process.residence {
          Some(ProcessResidence::Deadline {
            key: WakeupKey::Tick(tick),
            ..
          }) => tick == record.expiry_tick(),
          Some(ProcessResidence::Service(kind)) => {
            kind == Self::financial_wait_service_kind(record.unpaid_service())
          }
          _ => false,
        }
        && semantic.hot.trigger_wakeup_pointer.is_none()
        && !TriggerDeadlineHandles::<T>::contains_key(actor.actor_id)
        && episode
          .bind(
            scope,
            record.unpaid_service(),
            liability,
            T::CadenceTickMillis::get(),
            T::MaxTemporalDelayTicks::get(),
          )
          .is_ok()
    }

    /// Ordinary Service keeps its existing carrier checks; only the optional waiting reason
    /// authenticates the retained financial companion before carrier mutation.
    fn process_has_service_authority(
      actor: ActorRef,
      process: &ActorProcessOf<T>,
      kind: ServiceResidenceKind,
    ) -> bool {
      if process.generation != actor.generation
        || process.residence != Some(ProcessResidence::Service(kind))
      {
        return false;
      }
      match process.status {
        ProcessStatus::Serving => true,
        ProcessStatus::AwaitingFunding => {
          matches!(ActorSemanticStates::<T>::get(actor.actor_id),
            Some(ActorSemanticState::Active(record))
              if Self::financial_wait_matches_process(actor, &record, process))
        }
        _ => false,
      }
    }

    /// Due control and every reachable cleanup branch are admitted before source inspection.
    /// These host bounds are optional; public financial-wait admission remains unavailable.
    pub(crate) fn financial_wait_due_weights() -> Option<(Weight, Weight)> {
      let due = T::AdmissionCertificateAuthority::financial_wait_due_weight(
        Self::financial_wait_episode_bytes()?,
      )?;
      let cleanup = T::AdmissionCertificateAuthority::current()?.maximum_lifecycle_weight;
      due.checked_add(&cleanup)?;
      Some((due, cleanup))
    }

    fn process_due_financial_wait_outcome(
      actor: ActorRef,
      now_tick: SchedulerTick,
      cleanup: Weight,
    ) -> Result<(DueTickDeadlineMutation, bool), FinancialWaitDueError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::ensure_machine_unfrozen().map_err(|_| FinancialWaitDueError::MachineFrozen)?;
          let (semantic, process) = Self::load_canonical_actor_semantic_state(actor)
            .map_err(|_| FinancialWaitDueError::InvalidSource)?;
          let episode = ActorFinancialWaits::<T>::get(actor.actor_id)
            .ok_or(FinancialWaitDueError::InvalidSource)?;
          let record = episode.record();
          if process.status != ProcessStatus::AwaitingFunding
            || !matches!(process.residence,
              Some(ProcessResidence::Deadline { key: WakeupKey::Tick(tick), .. })
                if tick == record.expiry_tick())
          {
            return Err(FinancialWaitDueError::InvalidSource);
          }
          let timestamp = T::Time::now();
          let actual_tick = scheduler_tick_floor(timestamp, T::CadenceTickMillis::get())
            .ok_or(FinancialWaitDueError::InvalidSource)?;
          if record.expiry_tick() > now_tick || record.expiry_tick() > actual_tick {
            return Err(FinancialWaitDueError::NotDue);
          }
          let (scope, liability) =
            Self::financial_wait_source(actor, &semantic, record.unpaid_service())
              .ok_or(FinancialWaitDueError::InvalidSource)?;
          let bound = episode
            .bind(
              scope,
              record.unpaid_service(),
              liability,
              T::CadenceTickMillis::get(),
              T::MaxTemporalDelayTicks::get(),
            )
            .map_err(FinancialWaitDueError::Record)?;
          let receipt = FinancialWaitReceipt {
            actor,
            episode_identity: episode.original_identity(),
            unpaid_service: record.unpaid_service(),
          };
          let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor.actor_id) else {
            return Err(FinancialWaitDueError::InvalidSource);
          };
          let instance = Self::derive_active_actor_view(
            state.identity.clone(),
            state.hot.clone(),
            state.contract.clone(),
          );
          let unpaid_trigger_due = match record.unpaid_service() {
            UnpaidActorService::AtTimeTrigger { due_tick } => Some(due_tick),
            _ => None,
          };
          let terminal_reason = Self::classify_actor_loaded_with_unpaid_trigger(
            &instance,
            state.run_state.as_ref(),
            unpaid_trigger_due,
          )
          .map_err(|_| FinancialWaitDueError::InvalidSource)?
          .terminal_reason;
          // Financial disposition never replaces an independently owned lifetime/safety terminal.
          let close_reason = if let Some(reason) = terminal_reason {
            reason
          } else {
            let spendable = T::AssetOps::balance(
              &semantic.identity.sovereign_account,
              T::FeeNativeAssetId::get(),
            )
            .saturating_sub(T::MinUserBalance::get());
            match bound
              .classify(spendable, timestamp)
              .map_err(FinancialWaitDueError::Record)?
            {
              FinancialWaitDecision::Funded => {
                Self::return_financial_wait_to_service(actor, process, record.unpaid_service())?;
                // Placement is tentative: the original boundary, debt and expiry remain unchanged.
                // No service fee or Action is executed by the Deadline worker.
                return Ok((
                  DueTickDeadlineMutation::FinancialWaitReturned(receipt),
                  false,
                ));
              }
              FinancialWaitDecision::GraceExpired => CloseReason::FundingGraceExpired,
              FinancialWaitDecision::Awaiting => return Err(FinancialWaitDueError::NotDue),
            }
          };
          if !Self::close_control_weight_for_contract(&state.contract)
            .is_some_and(|weight| weight.all_lte(cleanup))
          {
            return Err(FinancialWaitDueError::Unqualified);
          }
          let run = state.run_state.clone();
          Self::remove_actor_publication_and_finalize(actor, state, run.as_ref(), close_reason)
            .map_err(|_| FinancialWaitDueError::TerminalCleanup)?;
          Ok((DueTickDeadlineMutation::FinancialWaitClosed(receipt), true))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn process_financial_wait_due(
      actor: ActorRef,
      now_tick: SchedulerTick,
      meter: &mut WeightMeter,
    ) -> Result<DueTickDeadlineMutation, FinancialWaitDueError> {
      let (due, cleanup) =
        Self::financial_wait_due_weights().ok_or(FinancialWaitDueError::Unqualified)?;
      if !meter.can_consume(due.saturating_add(cleanup)) {
        return Err(FinancialWaitDueError::InsufficientWeight);
      }
      meter.consume(due);
      let result = Self::process_due_financial_wait_outcome(actor, now_tick, cleanup);
      if !matches!(&result, Ok((_, false))) {
        meter.consume(cleanup);
      }
      result.map(|(mutation, _)| mutation)
    }

    /// Native preparation of the canonical entry transaction. Public admission remains closed:
    /// physical entry/cleanup prices do not establish finite episode backing or recovery service.
    #[cfg(test)]
    pub(crate) fn publish_prepared_financial_wait(
      actor: ActorRef,
      unpaid: UnpaidActorService,
      meter: &mut WeightMeter,
    ) -> Result<FinancialWaitEpisode<T::Balance>, FinancialWaitPublicationError> {
      let bytes =
        Self::financial_wait_episode_bytes().ok_or(FinancialWaitPublicationError::Unqualified)?;
      let weight = T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes)
        .filter(|_| Self::financial_wait_geometry_qualified(OnUnfunded::AwaitFunding))
        .ok_or(FinancialWaitPublicationError::Unqualified)?;
      if !meter.can_consume(weight) {
        return Err(FinancialWaitPublicationError::InsufficientWeight);
      }
      // Rollback restores state, never already-incurred inspection/publication work.
      meter.consume(weight);
      Self::publish_financial_wait(actor, unpaid)
    }

    /// The caller admits entry and every reachable cleanup branch before this atomic publication.
    pub(crate) fn publish_financial_wait(
      actor: ActorRef,
      unpaid: UnpaidActorService,
    ) -> Result<FinancialWaitEpisode<T::Balance>, FinancialWaitPublicationError> {
      let bytes =
        Self::financial_wait_episode_bytes().ok_or(FinancialWaitPublicationError::Unqualified)?;
      if T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes).is_none()
        || !Self::financial_wait_geometry_qualified(OnUnfunded::AwaitFunding)
      {
        return Err(FinancialWaitPublicationError::Unqualified);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::ensure_machine_unfrozen()
            .map_err(|_| FinancialWaitPublicationError::MachineFrozen)?;
          if ActorFinancialWaits::<T>::contains_key(actor.actor_id) {
            return Err(FinancialWaitPublicationError::AlreadyWaiting);
          }
          let (mut semantic, mut process) = Self::load_canonical_actor_semantic_state(actor)
            .map_err(|_| FinancialWaitPublicationError::InvalidSource)?;
          if !matches!(
            process.status,
            ProcessStatus::Serving | ProcessStatus::Disabled(_)
          ) {
            return Err(FinancialWaitPublicationError::InvalidSource);
          }
          let (scope, liability) = Self::financial_wait_source(actor, &semantic, unpaid)
            .ok_or(FinancialWaitPublicationError::InvalidSource)?;
          let required = T::MinUserBalance::get()
            .checked_add(&liability)
            .ok_or(FinancialWaitPublicationError::InvalidSource)?;
          if T::AssetOps::balance(
            &semantic.identity.sovereign_account,
            T::FeeNativeAssetId::get(),
          ) >= required
          {
            return Err(FinancialWaitPublicationError::Solvent);
          }
          let record = FinancialWaitRecord::new(
            scope,
            unpaid,
            liability,
            T::Time::now(),
            T::CadenceTickMillis::get(),
            T::MaxTemporalDelayTicks::get(),
          )
          .map_err(FinancialWaitPublicationError::Record)?;
          let handle =
            Self::plan_deadline_destination(actor, WakeupKey::Tick(record.expiry_tick()))
              .map_err(FinancialWaitPublicationError::Deadline)?;
          if let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid {
            if !TriggerDeadlineHandles::<T>::get(actor.actor_id).is_some_and(|source| {
              source.actor == actor && source.key == WakeupKey::Tick(due_tick)
            }) {
              return Err(FinancialWaitPublicationError::InvalidSource);
            }
          }
          if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
            Self::remove_trigger_deadline_member(actor)
              .map_err(FinancialWaitPublicationError::Deadline)?;
          } else if semantic.hot.trigger_wakeup_pointer.is_some() {
            return Err(FinancialWaitPublicationError::InvalidSource);
          }
          match process.residence {
            Some(ProcessResidence::Service(_)) => {
              Self::remove_service_member(actor).map_err(FinancialWaitPublicationError::Service)?;
            }
            Some(ProcessResidence::Deadline { .. }) => {
              Self::remove_deadline_member(actor)
                .map_err(FinancialWaitPublicationError::Deadline)?;
            }
            None
              if matches!(unpaid, UnpaidActorService::AtTimeTrigger { .. })
                && matches!(process.status, ProcessStatus::Disabled(_)) => {}
            _ => return Err(FinancialWaitPublicationError::InvalidSource),
          }
          semantic.hot.trigger_wakeup_pointer = None;
          let episode = FinancialWaitEpisode::new(record);
          process.status = ProcessStatus::AwaitingFunding;
          process.residence = Some(ProcessResidence::Deadline {
            key: handle.key,
            page: handle.page,
            slot: handle.slot,
          });
          ActorSemanticStates::<T>::insert(actor.actor_id, ActorSemanticState::Active(semantic));
          ActorFinancialWaits::<T>::insert(actor.actor_id, episode);
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(handle).map_err(FinancialWaitPublicationError::Deadline)?;
          Self::reconcile_actor_state_hold_with_authority(actor.actor_id)
            .map_err(|_| FinancialWaitPublicationError::StateHold)?;
          Self::deposit_event(Event::FinancialWaitEntered {
            receipt: FinancialWaitReceipt {
              actor,
              episode_identity: episode.original_identity(),
              unpaid_service: unpaid,
            },
          });
          Ok(episode)
        })();
        match result {
          Ok(episode) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(episode))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub fn actor_run_state(actor_id: ActorId) -> Option<ActorRunStateOf<T>> {
      ActorRunStateStore::<T>::get(actor_id)
    }
    pub fn actor_contract(actor_id: ActorId) -> Option<ActorContractOf<T>> {
      Self::load_actor_contract(actor_id)
    }

    pub fn actor_cost_quote(
      actor_id: ActorId,
    ) -> Result<ActorCostQuote<T::Balance>, ActorCostQuoteError> {
      let (state, admission) = Self::load_actor_state_with_admission(actor_id);
      let (identity, active) = match state {
        LoadedActorStateOf::NotRegistered => return Err(ActorCostQuoteError::ActorNotFound),
        LoadedActorStateOf::Dormant(identity) => (identity, None),
        LoadedActorStateOf::Active(state) => (state.identity.clone(), Some(state)),
        LoadedActorStateOf::Corrupt => return Err(ActorCostQuoteError::ActorInvariant),
      };
      let actor_type = identity.actor_class.actor_type();
      let creation_fee = if actor_type == ActorType::System {
        T::Balance::zero()
      } else {
        T::ActorCreationFee::get()
      };
      let effect_identity = T::TaskEffectWeight::production_weight_identity()
        .ok_or(ActorCostQuoteError::WeightAuthorityUnavailable)?;
      let zero_action = ActorActionFeeQuote {
        maximum_effect_weight: Weight::zero(),
        maximum_effect_fee: T::Balance::zero(),
        production_weight_identity: effect_identity,
      };
      let (prospective_trigger_fee, prospective_pipeline_fee, maximum_next_action_fee) =
        match active {
          None => (None, None, zero_action),
          Some(state) => {
            let admission = admission.ok_or(ActorCostQuoteError::ActorInvariant)?;
            let trigger_family = if state.contract.parked_balance_activation.is_some() {
              TriggerFamily::ParkedBalance
            } else {
              state.contract.trigger.family()
            };
            let maximum_weight = Self::trigger_occurrence_weight(trigger_family);
            let trigger_fee = Self::trigger_fee_for_actor(
              actor_id,
              actor_type,
              state.contract.on_unfunded,
              trigger_family,
              maximum_weight,
            )
            .map_err(|_| ActorCostQuoteError::WeightAuthorityUnavailable)?;
            let machine_envelope = ActorContractHeads::<T>::get(actor_id)
              .ok_or(ActorCostQuoteError::ActorInvariant)?
              .header
              .pipeline_machine_envelope;
            let pipeline_fee = Self::pipeline_fee_breakdown(actor_type, machine_envelope);
            let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
            let action = if state.contract.steps.is_empty() {
              zero_action
            } else {
              let loaded = Self::load_current_step_from_storage(actor_id, cursor)
                .ok_or(ActorCostQuoteError::ActorInvariant)?;
              let maximum =
                Self::maximum_current_action_fee(actor_type, &loaded.step, loaded.resources)
                  .map_err(|_| ActorCostQuoteError::ComputationOverflow)?;
              ActorActionFeeQuote {
                maximum_effect_weight: if matches!(loaded.step.task, ActorTask::StopCycle) {
                  Weight::zero()
                } else {
                  loaded.resources.effect
                },
                maximum_effect_fee: maximum.effect_fee,
                production_weight_identity: effect_identity,
              }
            };
            (
              Some(ActorTriggerFeeQuote {
                trigger_family,
                maximum_weight,
                fee: trigger_fee.trigger_fee,
                production_weight_identity: Self::trigger_weight_identity(),
              }),
              Some(ActorPipelineFeeQuote {
                pipeline_machine_fee: pipeline_fee.pipeline_machine_fee,
                total_fee: pipeline_fee.total_fee,
                strategy: PipelineMachineFeeStrategy::UpfrontBounded,
                admission_identity: admission.admission_identity,
                production_weight_identity: admission.production_weight_identity,
              }),
              action,
            )
          }
        };
      let actor_state_hold = Self::actor_state_hold_quote(actor_id, actor_type)?;
      Ok(ActorCostQuote {
        actor_type,
        creation_fee,
        prospective_trigger_fee,
        prospective_pipeline_fee,
        maximum_next_action_fee,
        actor_state_hold,
      })
    }

    pub(crate) fn load_actor_contract(actor_id: ActorId) -> Option<ActorContractOf<T>> {
      Self::load_admitted_contract_geometry(actor_id).map(|(contract, _)| contract)
    }

    /// Captures one complete fixed-anchor episode from authoritative host total ownership. The
    /// authored plan is already canonical and bounded; failure returns no partial certificate.
    #[allow(
      dead_code,
      reason = "parked-balance certification is staged behind Actor Contract admission"
    )]
    pub(crate) fn certify_parked_balance_activation(
      sovereign_account: &T::AccountId,
      activation: &ParkedBalanceActivationOf<T>,
      acknowledged_revision: DependencyRevision,
    ) -> Result<ParkedBalanceWatchesOf<T>, ParkedBalanceCertificationError> {
      let mut watches = ParkedBalanceWatchesOf::<T>::default();
      for rule in activation.watches.iter(
        /* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the canonical authored plan. */
      ) {
        let minimum_balance = T::AssetOps::minimum_balance(rule.asset);
        let anchor = T::AssetOps::total_balance(sovereign_account, rule.asset);
        let watch = certify_parked_balance_watch(
          rule.asset,
          rule.authored_min_delta,
          minimum_balance,
          anchor,
          acknowledged_revision,
        )?;
        watches
          .try_push(watch)
          .map_err(|_| ParkedBalanceCertificationError::PlanCapacityExceeded)?;
      }
      Ok(watches)
    }

    pub(crate) fn store_actor_contract(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> DispatchResult {
      let certificate =
        Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Self::store_actor_contract_with_admission(actor_id, contract, &certificate)
    }

    pub(crate) fn store_actor_contract_with_admission(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> DispatchResult {
      if contract.close_when.is_some() {
        Self::ensure_current_contract_admission(&contract, certificate)?;
      }
      let replacing = ActorContractHeads::<T>::contains_key(actor_id);
      // A canonically published Actor owns no legacy primary or unsignaled cell. Its Contract
      // replacement must rotate the generation-bound process/residence carriers together with
      // the geometry instead of mirroring a physical primary.
      let canonical_replace = replacing
        && !ActorControlLocators::<T>::contains_key(actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor_id);
      if canonical_replace && contract.on_unfunded == OnUnfunded::AwaitFunding {
        let current = Self::load_control_admission(actor_id).ok_or(Error::<T>::ActorInvariant)?;
        if Self::wait_recertification_needs_preservation(actor_id, &current, certificate) {
          return Self::recertify_canonical_financial_wait(actor_id, &contract, certificate);
        }
      }
      let stored = if replacing {
        if canonical_replace {
          Self::replace_canonical_contract_geometry(actor_id, &contract, certificate)
        } else {
          Self::replace_admitted_contract_geometry(actor_id, &contract, certificate)
        }
      } else {
        let actor_type = Self::load_frame_control_authority(actor_id)
          .map(|(_, identity, _, _)| identity.actor_class.actor_type())
          .or_else(|| match ActorSemanticStates::<T>::get(actor_id) {
            Some(ActorSemanticState::Active(record)) => {
              Some(record.identity.actor_class.actor_type())
            }
            _ => None,
          })
          .ok_or(Error::<T>::ActorInvariant)?;
        Self::insert_admitted_contract_geometry_with_actor_type(
          actor_id,
          actor_type,
          &contract,
          certificate,
        )
      };
      ensure!(stored, Error::<T>::ActorInvariant);
      if canonical_replace {
        Self::republish_canonical_contract(actor_id, &contract, certificate)
          .map_err(Self::placement_error)?;
      }
      Ok(())
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn remove_actor_contract(actor_id: ActorId) -> DispatchResult {
      ensure!(
        Self::remove_admitted_contract_geometry(actor_id).is_some(),
        Error::<T>::ActorInvariant
      );
      Ok(())
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn insert_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      // Deliberately malformed storage fixtures may have no identity owner. They retain System's
      // zero-fee projection solely as a package-test corruption seam.
      let actor_type = Self::load_control_identity(actor_id)
        .map(|identity| identity.actor_class.actor_type())
        .unwrap_or(ActorType::System);
      Self::insert_admitted_contract_geometry_with_actor_type(
        actor_id,
        actor_type,
        contract,
        certificate,
      )
    }

    fn insert_admitted_contract_geometry_with_actor_type(
      actor_id: ActorId,
      actor_type: ActorType,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      if ActorContractHeads::<T>::contains_key(actor_id) {
        return false;
      }
      let Some(policy) =
        Self::prepare_contract_behavior_state(actor_id, contract, certificate, None)
      else {
        return false;
      };
      if (policy.0.is_some() && ActorBehaviorPolicies::<T>::contains_key(actor_id))
        || (policy.1.is_some() && ActorBehaviorMonitors::<T>::contains_key(actor_id))
      {
        return false;
      }
      let Some((head, chunks)) =
        Self::decompose_admitted_contract_geometry(actor_id, actor_type, contract, certificate)
      else {
        return false;
      };
      if chunks
        .iter(/* deos-bypass: bounded-iter */)
        .any(|(chunk_index, _)| {
          ActorContractTailChunks::<T>::contains_key(actor_id, chunk_index)
        })
      {
        return false;
      }
      ActorContractHeads::<T>::insert(actor_id, head);
      Self::replace_contract_behavior_policy(actor_id, false, policy);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      true
    }

    pub(crate) fn load_contract_geometry_with_admission(
      actor_id: ActorId,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      if head.header.step_count > T::MaxContractSteps::get() {
        return None;
      }
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != head.header.semantic_contract_id
        || certificate.body_commitment != head.header.body_commitment
        || certificate.admission_identity != head.header.admission_identity
      {
        return None;
      }
      let chunk_count = head
        .header
        .step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let chunks = (0..chunk_count)
        .map(|chunk_index| {
          Some((
            chunk_index,
            ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
          ))
        })
        .collect::<Option<Vec<_>>>()?;
      let mut resource_count = usize::from(head.header.step_count > 0);
      for (_, chunk) in &chunks {
        if chunk.steps.len() != chunk.step_resources.len() {
          return None;
        }
        resource_count = resource_count.checked_add(chunk.step_resources.len())?;
      }
      if resource_count != head.header.step_count as usize {
        return None;
      }
      Self::reconstruct_contract_geometry(actor_id, head, &chunks)
    }

    pub(crate) fn load_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<(ActorContractOf<T>, ActorAdmissionCertificateOf<T>)> {
      let certificate = Self::load_control_admission(actor_id)?;
      let contract = Self::load_contract_geometry_with_admission(actor_id, &certificate)?;
      Some((contract, certificate))
    }

    pub(crate) fn load_current_step_from_storage(
      actor_id: ActorId,
      cursor: u32,
    ) -> Option<LoadedActorStepOf<T>> {
      let certificate = Self::load_control_admission(actor_id)?;
      Self::load_current_step_with_admission(actor_id, cursor, &certificate)
    }

    pub(crate) fn load_current_step_with_admission(
      actor_id: ActorId,
      cursor: u32,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<LoadedActorStepOf<T>> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      let tail_chunk = if cursor == 0 {
        None
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        Some((
          chunk_index,
          ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
        ))
      };
      Self::load_current_step_from_geometry(
        actor_id,
        &head,
        certificate,
        cursor,
        tail_chunk.as_ref().map(|(index, chunk)| (*index, chunk)),
      )
    }

    pub(crate) fn replace_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let Some((_, identity, _, admission)) = Self::load_frame_control_authority(actor_id) else {
        return false;
      };
      let Some(current_contract) =
        Self::load_contract_geometry_with_admission(actor_id, &admission)
      else {
        return false;
      };
      let actor_type = identity.actor_class.actor_type();
      let previous_header =
        if current_contract.close_when.is_some() || contract.close_when.is_some() {
          let Some(head) = ActorContractHeads::<T>::get(actor_id) else {
            return false;
          };
          Some(head.header)
        } else {
          None
        };
      let Some(policy) = Self::prepare_contract_behavior_state(
        actor_id,
        contract,
        certificate,
        previous_header.as_ref(),
      ) else {
        return false;
      };
      let Some((head, chunks)) =
        Self::decompose_admitted_contract_geometry(actor_id, actor_type, contract, certificate)
      else {
        return false;
      };
      let Ok(old_step_count) = u32::try_from(current_contract.steps.len()) else {
        return false;
      };
      let old_chunk_count = old_step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let Ok(new_chunk_count) = u32::try_from(chunks.len()) else {
        return false;
      };
      if !Self::replace_control_admission_for_transition(actor_id, certificate, contract) {
        return false;
      }
      Self::replace_contract_behavior_policy(
        actor_id,
        current_contract.close_when.is_some(),
        policy,
      );
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      for chunk_index in new_chunk_count..old_chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      true
    }

    /// Replaces Contract geometry and admission for a canonically published Actor, rotating the
    /// generation-bound carriers instead of mirroring a legacy primary. The current process
    /// residence and any independent temporal Trigger deadline are released before the new
    /// geometry commits; the caller republishes one complete canonical successor afterwards.
    pub(crate) fn replace_canonical_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return false;
      };
      if contract.on_unfunded == OnUnfunded::AwaitFunding
        && Self::wait_recertification_needs_preservation(actor_id, &record.admission, certificate)
      {
        return false;
      }
      let Some(next_generation) = next_actor_generation(record.generation) else {
        return false;
      };
      let Some(process) = ActorProcesses::<T>::get(actor_id)
        .filter(|process| process.generation == record.generation)
      else {
        return false;
      };
      let actor = ActorRef {
        actor_id,
        generation: record.generation,
      };
      let Some(current_contract) =
        Self::load_contract_geometry_with_admission(actor_id, &record.admission)
      else {
        return false;
      };
      if current_contract.on_unfunded == OnUnfunded::AwaitFunding
        && Self::load_canonical_actor_semantic_state(actor).is_err()
      {
        return false;
      }
      let actor_type = record.identity.actor_class.actor_type();
      let previous_header =
        if current_contract.close_when.is_some() || contract.close_when.is_some() {
          let Some(head) = ActorContractHeads::<T>::get(actor_id) else {
            return false;
          };
          Some(head.header)
        } else {
          None
        };
      let Some(policy) = Self::prepare_contract_behavior_state(
        actor_id,
        contract,
        certificate,
        previous_header.as_ref(),
      ) else {
        return false;
      };
      let Some((head, chunks)) =
        Self::decompose_admitted_contract_geometry(actor_id, actor_type, contract, certificate)
      else {
        return false;
      };
      let Ok(old_step_count) = u32::try_from(current_contract.steps.len()) else {
        return false;
      };
      let old_chunk_count = old_step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let Ok(new_chunk_count) = u32::try_from(chunks.len()) else {
        return false;
      };
      if TriggerDeadlineHandles::<T>::contains_key(actor_id)
        && Self::remove_trigger_deadline_member(actor).is_err()
      {
        return false;
      }
      match process.residence {
        Some(ProcessResidence::Service(_)) => {
          if Self::remove_service_member(actor).is_err() {
            return false;
          }
        }
        Some(ProcessResidence::Deadline { .. }) => {
          if Self::remove_deadline_member(actor).is_err() {
            return false;
          }
        }
        Some(ProcessResidence::Parked(evidence)) => {
          if Self::release_parked_dependency_authority(actor, evidence).is_err() {
            return false;
          }
        }
        None if matches!(process.status, ProcessStatus::Disabled(_)) => {}
        _ => return false,
      }
      ActorProcesses::<T>::remove(actor_id);
      if current_contract.on_unfunded == OnUnfunded::AwaitFunding {
        ActorFinancialWaits::<T>::remove(actor_id);
      }
      Self::replace_contract_behavior_policy(
        actor_id,
        current_contract.close_when.is_some(),
        policy,
      );
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      for chunk_index in new_chunk_count..old_chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      let mut updated = record.clone();
      // The release above removed the superseded `TriggerDeadlineHandles` member. A schedule
      // replacement must also drop the semantic pointer so the successor publication re-derives
      // its temporal deadline from the replacement anchor instead of re-using the replaced
      // cadence/AtTime tick; an unchanged schedule keeps the exact outstanding tick.
      if current_contract.trigger != contract.trigger
        || current_contract.cooldown_blocks != contract.cooldown_blocks
        || current_contract.window != contract.window
      {
        updated.hot.trigger_wakeup_pointer = None;
      }
      updated.admission = certificate.clone();
      updated.generation = next_generation;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Active(record),
          replacement: ActorSemanticState::Active(updated),
        },
      )
      .is_ok()
    }

    /// Technical certificate renewal preserves the same owed boundary and physical residence.
    /// Incompatible resources require a separately qualified transition, not ordinary replacement.
    fn recertify_canonical_financial_wait(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> DispatchResult {
      ensure!(
        polkadot_sdk::frame_support::storage::transactional::is_transactional()
          && certificate.has_valid_identity()
          && Self::admission_matches_current_authority(certificate),
        Error::<T>::AwaitFundingUnavailable
      );
      let actor = Self::load_actor_ref(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      let (record, process) =
        Self::load_canonical_actor_semantic_state(actor).map_err(|_| Error::<T>::ActorInvariant)?;
      ensure!(
        process.status == ProcessStatus::AwaitingFunding
          && record.admission.body_geometry_version == certificate.body_geometry_version
          && record.admission.maximum_lifecycle_weight == certificate.maximum_lifecycle_weight
          && record.admission.semantic_contract_id == certificate.semantic_contract_id
          && record.admission.body_commitment == certificate.body_commitment
          && certificate.wake_qualification
            == contract.trigger.wake_qualification(&contract.window)
          && Self::load_contract_geometry_with_admission(actor_id, &record.admission).as_ref()
            == Some(contract),
        Error::<T>::AwaitFundingUnavailable
      );
      let mut old_head =
        ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      let old_header = old_head.header.clone();
      let (head, chunks) = Self::decompose_admitted_contract_geometry(
        actor_id,
        ActorType::User,
        contract,
        certificate,
      )
      .ok_or(Error::<T>::AwaitFundingUnavailable)?;
      old_head.header.admission_identity = certificate.admission_identity;
      ensure!(old_head == head, Error::<T>::AwaitFundingUnavailable);
      for (index, chunk) in &chunks {
        let mut old_chunk =
          ActorContractTailChunks::<T>::get(actor_id, index).ok_or(Error::<T>::ActorInvariant)?;
        old_chunk.authority.admission_identity = certificate.admission_identity;
        ensure!(&old_chunk == chunk, Error::<T>::AwaitFundingUnavailable);
      }
      let policy =
        Self::prepare_contract_behavior_state(actor_id, contract, certificate, Some(&old_header))
          .ok_or(Error::<T>::AwaitFundingUnavailable)?;
      let episode = ActorFinancialWaits::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      let mut scope = episode.record().scope();
      scope.contract_authority.admission_identity = certificate.admission_identity;
      scope.contract_authority.pipeline_service_identity =
        pipeline_service_identity(certificate.admission_identity);
      let rebound = episode
        .rebind(
          scope,
          T::CadenceTickMillis::get(),
          T::MaxTemporalDelayTicks::get(),
        )
        .map_err(|_| Error::<T>::ActorInvariant)?;
      let mut run = ActorRunStateStore::<T>::get(actor_id);
      if let Some(run) = &mut run {
        run.contract_authority = scope.contract_authority;
      }
      let mut updated = record.clone();
      updated.admission = certificate.clone();
      ActorContractHeads::<T>::insert(actor_id, head);
      for (index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, index, chunk);
      }
      Self::replace_contract_behavior_policy(actor_id, contract.close_when.is_some(), policy);
      if let Some(run) = run {
        ActorRunStateStore::<T>::insert(actor_id, run);
      }
      ActorFinancialWaits::<T>::insert(actor_id, rebound);
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Active(record),
          replacement: ActorSemanticState::Active(updated.clone()),
        },
      )
      .map_err(|_| Error::<T>::ActorInvariant)?;
      ensure!(
        Self::financial_wait_matches_process(actor, &updated, &process),
        Error::<T>::AwaitFundingUnavailable
      );
      Self::reconcile_actor_state_hold_with_authority(actor_id)
    }

    fn wait_recertification_needs_preservation(
      actor_id: ActorId,
      current: &ActorAdmissionCertificateOf<T>,
      replacement: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      current.semantic_contract_id == replacement.semantic_contract_id
        && ActorFinancialWaits::<T>::contains_key(actor_id)
    }

    /// Republishes one complete canonical process/residence carrier for a freshly replaced
    /// Contract. The replacement already released every old carrier and rotated the semantic
    /// generation, so this seam plans and commits the successor under the new `ActorRef`.
    fn republish_canonical_contract(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      };
      if record.admission != *certificate || ActorRunStateStore::<T>::contains_key(actor_id) {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      }
      let resources = if contract.steps.is_empty() {
        Self::zero_step_resource_envelope(contract)
          .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?
      } else {
        Self::derive_step_resource_envelopes(contract)
          .and_then(|envelopes| envelopes.first().copied())
          .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?
      };
      let state = ActiveActorState {
        identity: record.identity,
        hot: record.hot,
        contract: contract.clone(),
        run_state: None,
      };
      Self::publish_actor_publication(
        ActorRef {
          actor_id,
          generation: record.generation,
        },
        &state,
        None,
        resources,
        frame_system::Pallet::<T>::block_number(),
        ServiceCutoff::Open,
      )
    }

    pub(crate) fn remove_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<ActorContractOf<T>> {
      let (contract, _) = Self::load_admitted_contract_geometry(actor_id)?;
      Self::remove_loaded_contract_geometry(actor_id, contract)
    }

    pub(crate) fn remove_admitted_contract_geometry_with_admission(
      actor_id: ActorId,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let contract = Self::load_contract_geometry_with_admission(actor_id, admission)?;
      Self::remove_loaded_contract_geometry(actor_id, contract)
    }

    fn remove_loaded_contract_geometry(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let policy = Self::load_behavior_cleanup_source(actor_id, contract.close_when.is_some())?;
      let chunk_count = u32::try_from(contract.steps.len())
        .ok()?
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      ActorContractHeads::<T>::remove(actor_id);
      Self::remove_loaded_behavior_state(actor_id, policy);
      if contract.on_unfunded == OnUnfunded::AwaitFunding {
        ActorFinancialWaits::<T>::remove(actor_id);
      }
      for chunk_index in 0..chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      Some(contract)
    }

    fn load_behavior_cleanup_source(
      actor_id: ActorId,
      has_policy: bool,
    ) -> Option<Option<ActorBehaviorPolicyOf<T>>> {
      if has_policy {
        Some(Some(ActorBehaviorPolicies::<T>::get(actor_id)?))
      } else {
        Some(None)
      }
    }

    fn remove_loaded_behavior_state(actor_id: ActorId, policy: Option<ActorBehaviorPolicyOf<T>>) {
      if let Some(policy) = policy {
        ActorBehaviorPolicies::<T>::remove(actor_id);
        if Self::behavior_monitor_required(&policy.plan) {
          ActorBehaviorMonitors::<T>::remove(actor_id);
        }
      }
    }

    /// Optional-state fragments only, without the ordinary head removal between them.
    /// This component is not a standalone production transition or an enclosing transaction.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn benchmark_remove_behavior_state(
      actor_id: ActorId,
      has_policy: bool,
    ) -> Option<()> {
      let policy = Self::load_behavior_cleanup_source(actor_id, has_policy)?;
      Self::remove_loaded_behavior_state(actor_id, policy);
      Some(())
    }

    /// Measures the loaded geometry-removal owner independently of admission and source loading.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn benchmark_remove_loaded_contract_geometry(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> Option<ActorContractOf<T>> {
      Self::remove_loaded_contract_geometry(actor_id, contract)
    }

    #[cfg(test)]
    pub(crate) fn current_step_resources_fit(
      control_meter: &WeightMeter,
      effect_meter: &WeightMeter,
      resources: ActorStepResourceEnvelope,
    ) -> bool {
      control_meter.can_consume(resources.control) && effect_meter.can_consume(resources.effect)
    }

    fn trigger_occurrence_weight(trigger_family: TriggerFamily) -> Weight {
      match trigger_family {
        TriggerFamily::Manual => T::WeightInfo::manual_trigger(),
        TriggerFamily::AddressEvent => T::WeightInfo::address_event_trigger_occurrence(),
        TriggerFamily::AtTime => T::WeightInfo::at_time_trigger_occurrence(),
        TriggerFamily::Cadenced => T::WeightInfo::cadenced_trigger_occurrence(),
        TriggerFamily::ParkedBalance => Self::dependency_review_weight_upper(),
      }
    }

    fn trigger_weight_identity() -> [u8; 32] {
      (
        b"DEOS_ACTOR_TRIGGER_WEIGHT_V1",
        Self::trigger_occurrence_weight(TriggerFamily::Manual),
        Self::trigger_occurrence_weight(TriggerFamily::AddressEvent),
        Self::trigger_occurrence_weight(TriggerFamily::AtTime),
        Self::trigger_occurrence_weight(TriggerFamily::Cadenced),
        Self::trigger_occurrence_weight(TriggerFamily::ParkedBalance),
      )
        .using_encoded(frame::hashing::blake2_256)
    }

    pub(crate) fn trigger_fee_for_weight(
      actor_type: ActorType,
      trigger_family: TriggerFamily,
      weight: Weight,
    ) -> TriggerFeeBreakdown<T::Balance> {
      let trigger_fee = if actor_type == ActorType::System || weight == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&weight)
      };
      TriggerFeeBreakdown {
        trigger_family,
        trigger_fee,
      }
    }

    fn financial_wait_service_fee() -> Result<T::Balance, Error<T>> {
      let bytes = Self::financial_wait_episode_bytes().ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let weight = T::AdmissionCertificateAuthority::financial_wait_service_weight(bytes)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(if weight == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&weight)
      })
    }

    fn fixed_financial_wait_fee(
      additional_control_fee: T::Balance,
    ) -> Result<T::Balance, Error<T>> {
      let bytes = Self::financial_wait_episode_bytes().ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let weights = [
        T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes),
        T::AdmissionCertificateAuthority::financial_wait_due_weight(bytes),
        T::AdmissionCertificateAuthority::financial_wait_service_weight(bytes),
      ];
      weights.into_iter(/* deos-bypass: bounded-iter */).try_fold(additional_control_fee, |fee, weight| {
        let weight = weight.ok_or(Error::<T>::AdmissionBoundOverflow)?;
        let component = if weight == Weight::zero() {
          Zero::zero()
        } else {
          T::WeightToFee::weight_to_fee(&weight)
        };
        fee.checked_add(&component).ok_or(Error::<T>::AdmissionBoundOverflow)
      })
    }

    /// The enclosing installation transaction rolls back host funding on any later refusal.
    pub(crate) fn secure_at_time_installation_backing(
      owner: &T::AccountId,
      actor_type: ActorType,
      actor: ActorRef,
      contract: &ActorContractOf<T>,
    ) -> DispatchResult {
      if actor_type != ActorType::User
        || contract.on_unfunded != OnUnfunded::AwaitFunding
        || !matches!(contract.trigger, Trigger::AtTime { .. })
      {
        return Ok(());
      }
      let occurrence = T::WeightInfo::at_time_trigger_occurrence();
      let occurrence_fee = if occurrence == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&occurrence)
      };
      let minimum_fee = Self::fixed_financial_wait_fee(occurrence_fee)?;
      T::FinancialWaitBacking::secure_at_time_installation(
        owner,
        actor,
        contract.semantic_contract_id(),
        minimum_fee,
      )
    }

    pub(crate) fn trigger_fee_for_head(
      actor_type: ActorType,
      head: &ActorContractHeadOf<T>,
      trigger_family: TriggerFamily,
      weight: Weight,
    ) -> Result<TriggerFeeBreakdown<T::Balance>, Error<T>> {
      let mut breakdown = Self::trigger_fee_for_weight(actor_type, trigger_family, weight);
      if actor_type == ActorType::System || head.header.on_unfunded == OnUnfunded::Close {
        return Ok(breakdown);
      }
      ensure!(
        head.header.trigger.family() == trigger_family
          || (trigger_family == TriggerFamily::ParkedBalance
            && head.header.parked_balance_activation.is_some()),
        Error::<T>::ActorInvariant
      );
      let additional_control_fee = match (&head.first_step, head.first_step_resources) {
        (None, None) if head.header.step_count == 0 => {
          head
            .header
            .pipeline_machine_envelope
            .pipeline_machine_fee_upper
        }
        (Some(step), Some(resources)) if head.header.step_count > 0 => {
          let mut control = if matches!(step.task, super::types::Task::StopCycle) {
            resources
              .control
              .checked_add(&resources.effect)
              .ok_or(Error::<T>::AdmissionBoundOverflow)?
          } else {
            resources.control
          };
          if head.header.has_close_when {
            control = control
              .checked_add(
                &T::StepControlWeight::maximum_behavior_service_inspection_weight(
                  Self::behavior_service_inspection_bounds()
                    .ok_or(Error::<T>::AdmissionBoundOverflow)?,
                )
                .ok_or(Error::<T>::AdmissionBoundOverflow)?,
              )
              .ok_or(Error::<T>::AdmissionBoundOverflow)?;
          }
          if control == Weight::zero() {
            Zero::zero()
          } else {
            T::WeightToFee::weight_to_fee(&control)
          }
        }
        _ => return Err(Error::<T>::ActorInvariant),
      };
      // One paid readiness owns one Opening wait. Its original machine liability remains unpaid;
      // this additional reserve covers only fixed wait work, never an unbounded return loop.
      breakdown.trigger_fee = breakdown
        .trigger_fee
        .checked_add(&Self::financial_wait_service_fee()?)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?
        .checked_add(&Self::fixed_financial_wait_fee(additional_control_fee)?)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(breakdown)
    }

    pub(crate) fn trigger_fee_for_actor(
      actor_id: ActorId,
      actor_type: ActorType,
      on_unfunded: OnUnfunded,
      trigger_family: TriggerFamily,
      weight: Weight,
    ) -> Result<TriggerFeeBreakdown<T::Balance>, Error<T>> {
      if actor_type == ActorType::System || on_unfunded == OnUnfunded::Close {
        return Ok(Self::trigger_fee_for_weight(
          actor_type,
          trigger_family,
          weight,
        ));
      }
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      ensure!(
        head.header.on_unfunded == on_unfunded,
        Error::<T>::ActorInvariant
      );
      Self::trigger_fee_for_head(actor_type, &head, trigger_family, weight)
    }

    pub(crate) fn pipeline_fee_breakdown(
      actor_type: ActorType,
      envelope: PipelineMachineEnvelope<T::Balance>,
    ) -> PipelineFeeBreakdown<T::Balance> {
      let fee = if actor_type == ActorType::System {
        Zero::zero()
      } else {
        envelope.pipeline_machine_fee_upper
      };
      PipelineFeeBreakdown {
        pipeline_machine_fee: fee,
        total_fee: fee,
      }
    }

    pub(crate) fn step_fee_for_resources(
      actor_type: ActorType,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(StepFeeBreakdown {
          control_fee: Zero::zero(),
          effect_fee: Zero::zero(),
          total_fee: Zero::zero(),
        });
      }
      let control_fee: T::Balance = Zero::zero();
      let effect_fee = if resources.effect == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&resources.effect)
      };
      let total_fee = control_fee
        .checked_add(&effect_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(StepFeeBreakdown {
        control_fee,
        effect_fee,
        total_fee,
      })
    }

    pub(crate) fn maximum_current_action_fee(
      actor_type: ActorType,
      step: &StepOf<T>,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      if matches!(step.task, super::types::Task::StopCycle) {
        return Ok(StepFeeBreakdown {
          control_fee: Zero::zero(),
          effect_fee: Zero::zero(),
          total_fee: Zero::zero(),
        });
      }
      Self::step_fee_for_resources(actor_type, resources)
    }

    pub(crate) fn maximum_current_step_fee(
      actor_type: ActorType,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      Self::step_fee_for_resources(actor_type, resources)
    }

    /// Maximize one nonnegative cost component under shared and cursor-local repeat ceilings.
    /// Callers supply at most one choice per admitted Step; no loop depends on the global cutoff.
    pub(crate) fn maximum_pipeline_repeat_cost<Amount>(
      mut choices: Vec<(Amount, u32)>,
      mut remaining: u32,
    ) -> Option<Amount>
    where
      Amount: Copy + Ord + Zero + CheckedAdd + CheckedMul + From<u32>,
    {
      choices.sort_unstable_by(|left, right| right.0.cmp(&left.0));
      let mut total = Amount::zero();
      for (cost, capacity) in choices {
        if remaining == 0 {
          break;
        }
        let count = capacity.min(remaining);
        total = total.checked_add(&cost.checked_mul(&Amount::from(count))?)?;
        remaining -= count;
      }
      Some(total)
    }

    /// Read-only candidate over a structurally validated source; never installs fee authority.
    /// Explicit model/converter injection is confined to test/benchmark builds. Their catalog must
    /// commit the selected converter. Public availability gates and live consumers are unchanged.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub fn benchmark_pipeline_tariff_candidate<Provider, Converter>(
      actor_type: ActorType,
      mutability: Mutability,
      contract: &ActorContractOf<T>,
    ) -> Result<
      crate::tariff::PipelineTariffCandidate<BalanceOf<T>>,
      crate::tariff::PipelineTariffCandidateError,
    >
    where
      Provider: PipelineControlWeightProvider<ActorContractOf<T>>,
      Converter: polkadot_sdk::sp_weights::WeightToFee<Balance = BalanceOf<T>>,
    {
      use crate::tariff::{
        PipelineTariffCandidate, PipelineTariffCandidateError as CandidateError,
      };
      if contract.on_unfunded != OnUnfunded::Close {
        return Err(CandidateError::UnsupportedUnfundedPolicy);
      }
      let mut canonical_steps = contract.steps.clone();
      Self::canonicalize_preconditions(&mut canonical_steps)
        .map_err(CandidateError::InvalidContract)?;
      if canonical_steps.encode() != contract.steps.encode() {
        return Err(CandidateError::NonCanonicalSteps);
      }
      let validate = || -> DispatchResult {
        ensure!(
          contract.steps.len() <= T::MaxContractSteps::get() as usize,
          Error::<T>::TooManyContractSteps
        );
        if actor_type == ActorType::User {
          ensure!(
            !Self::contract_steps_contains_mint(&contract.steps),
            Error::<T>::MintNotAllowedForUserActor
          );
        }
        Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
        if let Some(window) = &contract.window {
          Self::validate_schedule_window(window)?;
        }
        Self::validate_future_schedule_targets(contract)?;
        Self::validate_contract_steps_shape(actor_type, &contract.steps)?;
        Self::validate_parked_balance_activation(contract)?;
        if let Some(expression) = &contract.close_when {
          expression
            .validate()
            .map_err(|_| Error::<T>::InvalidCloseWhen)?;
        }
        Self::ensure_retry_later_allowed(mutability, &contract.steps)?;
        if let Some(target) = contract.auto_close_at_cycle_nonce {
          Self::ensure_auto_close_target(0, target)?;
        }
        if actor_type == ActorType::System && mutability == Mutability::Immutable {
          ensure!(
            !contract.trigger.manual_source_enabled(),
            Error::<T>::InvalidTriggerConfiguration
          );
        }
        Ok(())
      };
      validate().map_err(CandidateError::InvalidContract)?;
      let authority =
        T::AdmissionCertificateAuthority::current().ok_or(CandidateError::AuthorityUnavailable)?;
      let (control_weight, pipeline_machine_fee, tariff_identity) =
        if actor_type == ActorType::System {
          (Weight::zero(), Zero::zero(), None)
        } else {
          let weight = Provider::maximum_pipeline_control_weight(contract)
            .ok_or(CandidateError::TariffUnavailable)?;
          (
            weight,
            Converter::weight_to_fee(&weight),
            Some(Provider::production_weight_identity()),
          )
        };
      let candidate_identity = (
        *b"DEOS_ACTOR_PIPELINE_TARIFF_CANDIDATE_V1",
        actor_type,
        mutability,
        contract.semantic_contract_id(),
        authority,
        tariff_identity,
        (control_weight, pipeline_machine_fee),
      )
        .using_encoded(frame::hashing::blake2_256);
      Ok(PipelineTariffCandidate {
        control_weight,
        pipeline_machine_fee,
        candidate_identity,
      })
    }

    pub(crate) fn derive_pipeline_machine_envelope(
      actor_type: ActorType,
      on_unfunded: OnUnfunded,
      contract_steps: &ContractSteps<T>,
      resources: &ActorAdmissionResourcesOf<T>,
      zero_step_control: Weight,
      policy_inspection: Weight,
    ) -> Result<PipelineMachineEnvelope<T::Balance>, Error<T>> {
      ensure!(
        contract_steps.len() == resources.len(),
        Error::<T>::ActorRunInvariant
      );
      if actor_type == ActorType::System {
        return Ok(PipelineMachineEnvelope {
          pipeline_machine_fee_upper: Zero::zero(),
        });
      }
      let service_control =
        Self::service_turn_control_weight_upper().ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let mut pipeline_control = if contract_steps.is_empty() {
        zero_step_control
          .checked_add(&policy_inspection)
          .and_then(|weight| weight.checked_add(&service_control))
          .ok_or(Error::<T>::AdmissionBoundOverflow)?
      } else {
        Weight::zero()
      };
      let mut additional_wait_fee: T::Balance = Zero::zero();
      let mut repeat_costs = Vec::new();
      let maximum_repeats = T::MaxConsecutiveFailures::get()
        .checked_sub(1)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      for (cursor, (step, resource)) in contract_steps
        .iter(/* deos-bypass: bounded-iter — admitted Contract Steps bound the complete visit. */)
        .zip(
          resources
            .iter(/* deos-bypass: bounded-iter — admitted resource count equals Step count. */),
        )
        .enumerate()
      {
        let is_stop_cycle = matches!(step.task, super::types::Task::StopCycle);
        let machine_weight = if is_stop_cycle {
          resource
            .control
            .checked_add(&resource.effect)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?
        } else {
          resource.control
        }
        .checked_add(&policy_inspection)
        .and_then(|weight| weight.checked_add(&service_control))
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        let (financial_inspection_fee, action_wait_fee) = if on_unfunded == OnUnfunded::AwaitFunding
        {
          // Readiness backs the first Opening inspection. Pipeline owns later Step/retry
          // inspections; each Action also backs its separate wait, excluding producer repeats.
          let action_wait_fee = if is_stop_cycle {
            Zero::zero()
          } else {
            let recovery_control_fee = if machine_weight == Weight::zero() {
              Zero::zero()
            } else {
              T::WeightToFee::weight_to_fee(&machine_weight)
            };
            Self::fixed_financial_wait_fee(recovery_control_fee)?
          };
          (Self::financial_wait_service_fee()?, action_wait_fee)
        } else {
          (Zero::zero(), Zero::zero())
        };
        pipeline_control = pipeline_control
          .checked_add(&machine_weight)
          .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        let first_wait_fee = if cursor == 0 {
          action_wait_fee
        } else {
          action_wait_fee
            .checked_add(&financial_inspection_fee)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?
        };
        additional_wait_fee = additional_wait_fee
          .checked_add(&first_wait_fee)
          .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        // StopCycle has no Task-failure or Action-liability path; predicate errors are Permanent.
        let repeat_capacity = if is_stop_cycle {
          0
        } else {
          step
            .on_error
            .retry_max_attempts()
            .unwrap_or(1)
            .checked_sub(1)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?
        };
        if repeat_capacity > 0 {
          let repeat_wait_fee = action_wait_fee
            .checked_add(&financial_inspection_fee)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?;
          repeat_costs.push((machine_weight, repeat_wait_fee, repeat_capacity));
        }
        // The authored suffix remains in Contract geometry, but an unconditional return cannot
        // execute it. Its loading, holds and destruction keep their independent cost owners.
        if is_stop_cycle && step.precondition.is_none() {
          break;
        }
      }
      // Dimension-wise maxima may select different paths. Dedicated wait backing is independent
      // of ordinary Control, and retains its own maximum under the same shared visit domain.
      let repeat_control = Weight::from_parts(
        Self::maximum_pipeline_repeat_cost(
          repeat_costs
            .iter(/* deos-bypass: bounded-iter — At most one repeat choice per bounded Contract Step. */)
            .map(|(weight, _, count)| (weight.ref_time(), *count))
            .collect(),
          maximum_repeats,
        )
        .ok_or(Error::<T>::AdmissionBoundOverflow)?,
        Self::maximum_pipeline_repeat_cost(
          repeat_costs
            .iter(/* deos-bypass: bounded-iter — At most one repeat choice per bounded Contract Step. */)
            .map(|(weight, _, count)| (weight.proof_size(), *count))
            .collect(),
          maximum_repeats,
        )
        .ok_or(Error::<T>::AdmissionBoundOverflow)?,
      );
      pipeline_control = pipeline_control
        .checked_add(&repeat_control)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      additional_wait_fee = additional_wait_fee
        .checked_add(
          &Self::maximum_pipeline_repeat_cost(
            repeat_costs
              .iter(/* deos-bypass: bounded-iter — At most one repeat choice per bounded Contract Step. */)
              .map(|(_, fee, count)| (*fee, *count))
              .collect(),
            maximum_repeats,
          )
          .ok_or(Error::<T>::AdmissionBoundOverflow)?,
        )
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let pipeline_machine_fee_upper = T::WeightToFee::weight_to_fee(&pipeline_control)
        .checked_add(&additional_wait_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(PipelineMachineEnvelope {
        pipeline_machine_fee_upper,
      })
    }

    pub(crate) fn step_control_weight_context(
      step_count: u32,
      cursor: u32,
      predicate_evaluation_units: u32,
    ) -> Option<StepControlWeightContext> {
      if step_count == 0 || step_count > T::MaxContractSteps::get() || cursor >= step_count {
        return None;
      }
      if cursor == 0 {
        return Some(StepControlWeightContext {
          cursor,
          steps_in_fragment: 1,
          opening_tail_chunks: step_count
            .saturating_sub(1)
            .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
          predicate_evaluation_units,
        });
      }
      let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
      let first_step_index =
        1u32.checked_add(chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
      Some(StepControlWeightContext {
        cursor,
        steps_in_fragment: step_count
          .checked_sub(first_step_index)?
          .min(MAX_STEPS_PER_TAIL_CHUNK),
        opening_tail_chunks: 0,
        predicate_evaluation_units,
      })
    }

    pub(crate) fn execution_step_control_weight_context(
      instance: &ActiveActorViewOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<StepControlWeightContext> {
      let step_count = u32::try_from(instance.steps.len()).ok()?;
      let cursor = loaded_step.cursor;
      if instance.cycle_state == CycleState::Idle && cursor != 0 {
        return None;
      }
      let predicate_evaluation_units = loaded_step
        .step
        .precondition
        .as_ref()
        .map_or(0, Precondition::evaluation_units);
      Self::step_control_weight_context(step_count, cursor, predicate_evaluation_units)
    }

    fn contract_behavior_control_weight_context(
      contract: &ActorContractOf<T>,
    ) -> Option<Option<BehaviorControlWeightContext>> {
      Some(match &contract.close_when {
        None => None,
        Some(expression) => {
          let plan = expression
            .validate()
            .ok()?
            .compile(BehaviorPolicyBinding {
              semantic_contract_id: contract.semantic_contract_id(),
              admission_identity: [0; 32],
            })
            .ok()?;
          Some(Self::behavior_control_weight_context(
            &ActorBehaviorPolicy {
              expression: expression.clone(),
              plan,
              episode_started_at_generation: 1,
            },
          )?)
        }
      })
    }

    /// Measures one Contract-side compiler invocation, excluding storage and host price selection.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_contract_behavior_control_weight_context(
      contract: &ActorContractOf<T>,
    ) -> Option<Option<BehaviorControlWeightContext>> {
      Self::contract_behavior_control_weight_context(contract)
    }

    pub(crate) fn zero_step_resource_envelope(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorStepResourceEnvelope> {
      Self::try_zero_step_resource_envelope(contract).ok()
    }

    fn try_zero_step_resource_envelope(
      contract: &ActorContractOf<T>,
    ) -> Result<ActorStepResourceEnvelope, Error<T>> {
      ensure!(
        contract.steps.is_empty(),
        Error::<T>::AdmissionBoundOverflow
      );
      let behavior = Self::contract_behavior_control_weight_context(contract)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let control = match behavior {
        Some(behavior) => T::StepControlWeight::maximum_behavior_zero_step_control_weight(behavior)
          .ok_or(Error::<T>::BehaviorPolicyUnavailable)?,
        None => T::WeightInfo::scheduler_inner_zero_step_complete(),
      };
      Ok(ActorStepResourceEnvelope {
        control: control
          .checked_add(&Self::idle_window_placement_weight(contract))
          .ok_or(Error::<T>::AdmissionBoundOverflow)?,
        effect: Weight::zero(),
      })
    }

    pub(crate) fn derive_step_resource_envelopes(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionResourcesOf<T>> {
      Self::try_derive_step_resource_envelopes(contract).ok()
    }

    // Internal loaders retain fail-closed Option semantics; admission preserves the refusing owner.
    fn try_derive_step_resource_envelopes(
      contract: &ActorContractOf<T>,
    ) -> Result<ActorAdmissionResourcesOf<T>, Error<T>> {
      let step_count =
        u32::try_from(contract.steps.len()).map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      let behavior = Self::contract_behavior_control_weight_context(contract)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      contract
        .steps
        .iter(/* deos-bypass: bounded-iter */)
        .enumerate()
        .map(|(cursor, step)| {
          let cursor = u32::try_from(cursor).map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
          let predicate_evaluation_units = step
            .precondition
            .as_ref()
            .map_or(0, Precondition::evaluation_units);
          let context = Self::step_control_weight_context(step_count, cursor, predicate_evaluation_units)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?;
          let completion = contract
            .parked_balance_activation
            .as_ref()
            .map_or(Weight::zero(), |_| T::WeightInfo::complete_cycle_to_parked_balance());
          let control = match behavior {
            Some(behavior) => T::StepControlWeight::maximum_behavior_control_weight(context, step, behavior)
              .ok_or(Error::<T>::BehaviorPolicyUnavailable)?,
            None => T::StepControlWeight::maximum_control_weight(context, step)
              .ok_or(Error::<T>::AdmissionBoundOverflow)?,
          };
          Ok(ActorStepResourceEnvelope {
            control: control.checked_add(&completion).ok_or(Error::<T>::AdmissionBoundOverflow)?,
            effect: T::TaskEffectWeight::maximum_effect_weight(&step.task)
              .ok_or(Error::<T>::AdmissionBoundOverflow)?,
          })
        })
        .collect::<Result<Vec<_>, Error<T>>>()?
        .try_into()
        .map_err(|_| Error::<T>::AdmissionBoundOverflow)
    }

    /// Builds the admission certificate for any Contract shape the host can price. A policy-bearing
    /// Contract additionally requires an available Close bound. The certificate copies the
    /// authority's lifecycle maximum verbatim, which already includes the measured policy-bearing
    /// Close owner, so the certificate stays consistent under
    /// `admission_matches_current_authority`. An unavailable bound still returns None.
    pub(crate) fn build_admission_certificate(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionCertificateOf<T>> {
      if contract.on_unfunded != OnUnfunded::Close
        || Self::validate_parked_balance_activation(contract).is_err()
      {
        return None;
      }
      let authority = T::AdmissionCertificateAuthority::current()?;
      if contract.close_when.is_some() {
        Self::close_control_weight_for_contract(contract)?;
      }
      Some(ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment()?,
        contract.trigger.wake_qualification(&contract.window),
        authority.runtime_actor_semantics_version,
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight,
      ))
    }

    pub(crate) fn decompose_admitted_contract_geometry(
      actor_id: ActorId,
      actor_type: ActorType,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<(ActorContractHeadOf<T>, Vec<(u32, ActorStepChunkOf<T>)>)> {
      let semantic_contract_id = contract.semantic_contract_id();
      let body_commitment = contract.body_commitment()?;
      let step_resources = Self::derive_step_resource_envelopes(contract)?;
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != semantic_contract_id
        || certificate.body_commitment != body_commitment
      {
        return None;
      }
      Self::compile_contract_behavior_policy(contract, certificate)?;
      let zero_step_control = if contract.steps.is_empty() {
        Self::zero_step_resource_envelope(contract)?.control
      } else {
        Weight::zero()
      };
      let pipeline_machine_envelope = Self::derive_pipeline_machine_envelope(
        actor_type,
        contract.on_unfunded,
        &contract.steps,
        &step_resources,
        zero_step_control,
        Self::policy_service_inspection_weight(contract)?,
      )
      .ok()?;
      Self::decompose_contract_geometry(
        actor_id,
        contract,
        certificate.admission_identity,
        pipeline_machine_envelope,
        &step_resources,
      )
    }

    fn compile_contract_behavior_policy(
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<Option<BehaviorPolicyPlan>> {
      let Some(expression) = &contract.close_when else {
        return Some(None);
      };
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != contract.semantic_contract_id()
        || certificate.body_commitment != contract.body_commitment()?
      {
        return None;
      }
      let binding = BehaviorPolicyBinding {
        semantic_contract_id: certificate.semantic_contract_id,
        admission_identity: certificate.admission_identity,
      };
      let plan = expression.validate().ok()?.compile(binding).ok()?;
      Some(Some(plan))
    }

    fn load_contract_behavior_policy(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
    ) -> Option<Option<ActorBehaviorPolicyOf<T>>> {
      if !header.has_close_when {
        return Some(None);
      }
      let policy = ActorBehaviorPolicies::<T>::get(actor_id)?;
      let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
        return None;
      };
      if record.generation == 0
        || policy.episode_started_at_generation == 0
        || policy.episode_started_at_generation > record.generation
        || !record.admission.has_valid_identity()
        || record.admission.semantic_contract_id != header.semantic_contract_id
        || record.admission.body_commitment != header.body_commitment
        || record.admission.admission_identity != header.admission_identity
      {
        return None;
      }
      policy
        .plan
        .verify(
          &policy.expression,
          BehaviorPolicyBinding {
            semantic_contract_id: header.semantic_contract_id,
            admission_identity: header.admission_identity,
          },
        )
        .ok()?;
      Some(Some(policy))
    }

    /// Raw cold-policy authentication/context component; excludes host prices and Service admission.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_behavior_service_policy_context(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
    ) -> Option<Option<BehaviorControlWeightContext>> {
      let Some(policy) = Self::load_contract_behavior_policy(actor_id, header)? else {
        return Some(None);
      };
      Some(Some(Self::behavior_control_weight_context(&policy)?))
    }

    fn behavior_episode_identity(
      actor_id: ActorId,
      policy: &ActorBehaviorPolicyOf<T>,
      semantic_contract_id: [u8; 32],
    ) -> [u8; 32] {
      (
        b"DEOS_ACTOR_BEHAVIOR_EPISODE",
        actor_id,
        policy.episode_started_at_generation,
        semantic_contract_id,
      )
        .using_encoded(frame::hashing::blake2_256)
    }

    fn behavior_monitor_required(plan: &BehaviorPolicyPlan) -> bool {
      plan.mode() == BehaviorTrackingMode::Accumulated || plan.requires_clean_cycle()
    }

    pub(crate) fn behavior_control_weight_context(
      policy: &ActorBehaviorPolicyOf<T>,
    ) -> Option<BehaviorControlWeightContext> {
      let counter_slots = if policy.plan.mode() == BehaviorTrackingMode::Accumulated {
        u32::try_from(policy.plan.metrics().len()).ok()?
      } else {
        0
      };
      let requires_clean_cycle = policy.plan.requires_clean_cycle();
      // Reserve the required busy clean carrier even while the current record is Idle.
      let monitor_bytes = if Self::behavior_monitor_required(&policy.plan) {
        u32::try_from(
          <[u8; 32]>::max_encoded_len()
            .checked_mul(2)?
            .checked_add(codec::Compact(counter_slots).encoded_size())?
            .checked_add(
              usize::try_from(counter_slots)
                .ok()?
                .checked_mul(u32::max_encoded_len())?,
            )?
            .checked_add(Option::<BehaviorCleanCycle>::None.encoded_size())?
            .checked_add(if requires_clean_cycle {
              BehaviorCleanCycle::max_encoded_len()
            } else {
              0
            })?,
        )
        .ok()?
      } else {
        0
      };
      let atoms = policy.expression.clauses.iter(/* deos-bypass: bounded-iter */)
        .try_fold(0u32, |total, clause| total.checked_add(u32::try_from(clause.len()).ok()?))?;
      Some(BehaviorControlWeightContext {
        policy_bytes: u32::try_from(policy.encoded_size()).ok()?,
        monitor_bytes,
        clauses: u32::try_from(policy.expression.clauses.len()).ok()?,
        atoms,
        metrics: u32::try_from(policy.plan.metrics().len()).ok()?,
        counter_slots,
        requires_clean_cycle,
      })
    }

    pub(crate) fn policy_service_inspection_weight(
      contract: &ActorContractOf<T>,
    ) -> Option<Weight> {
      if contract.close_when.is_none() {
        return Some(Weight::zero());
      }
      T::StepControlWeight::maximum_behavior_service_inspection_weight(
        Self::behavior_service_inspection_bounds()?,
      )
    }

    pub(crate) fn behavior_service_inspection_bounds()
    -> Option<BehaviorServiceInspectionWeightContext> {
      let maximum_clauses = T::MaxBehaviorClauses::get();
      Some(BehaviorServiceInspectionWeightContext {
        maximum_policy_bytes: u32::try_from(ActorBehaviorPolicyOf::<T>::max_encoded_len()).ok()?,
        maximum_monitor_bytes: u32::try_from(ActorBehaviorMonitor::max_encoded_len()).ok()?,
        maximum_clauses,
        maximum_atoms: maximum_clauses.checked_mul(T::MaxBehaviorAtomsPerClause::get())?,
      })
    }

    fn load_service_behavior_policy_with_price(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorBehaviorPolicyOf<T>> {
      if !header.has_close_when
        || !Self::financial_wait_geometry_qualified(header.on_unfunded)
        || header.parked_balance_activation.is_some()
        || !certificate.has_valid_identity()
        || certificate.semantic_contract_id != header.semantic_contract_id
        || certificate.body_commitment != header.body_commitment
        || certificate.admission_identity != header.admission_identity
        || !Self::admission_matches_current_authority(certificate)
      {
        return None;
      }
      if header.on_unfunded == OnUnfunded::AwaitFunding {
        let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
          return None;
        };
        if record.identity.actor_class.actor_type() != ActorType::User {
          return None;
        }
      }
      T::StepControlWeight::maximum_behavior_service_inspection_weight(
        Self::behavior_service_inspection_bounds()?,
      )?;
      let policy = Self::load_contract_behavior_policy(actor_id, header)??;
      // The policy-bearing Close is priced by the measured whole-call owner instead of an additive
      // optional-state supplement, so this validation only needs the policy geometry to be
      // coherent; availability of the Close bound is enforced where the Contract is admitted.
      Self::behavior_control_weight_context(&policy)?;
      Some(policy)
    }

    pub(crate) fn load_step_behavior_policy(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
    ) -> Option<Option<LoadedBehaviorPolicyOf<T>>> {
      let Some(policy) = Self::load_contract_behavior_policy(actor_id, header)? else {
        return Some(None);
      };
      let monitor = Self::load_contract_behavior_monitor(actor_id, header, &policy)?;
      let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
        return None;
      };
      Some(Some(LoadedBehaviorPolicy {
        actor: ActorRef {
          actor_id,
          generation: record.generation,
        },
        binding: BehaviorPolicyBinding {
          semantic_contract_id: header.semantic_contract_id,
          admission_identity: header.admission_identity,
        },
        policy,
        monitor,
      }))
    }

    pub(crate) fn reduce_step_behavior_policy(
      source: LoadedBehaviorPolicyOf<T>,
      source_cycle: CycleState,
      successor_cycle: CycleState,
      cycle_nonce: u64,
      mut observation: BehaviorObservation,
      clean_step: CleanCycleStep,
      completion: Option<NormalCycleCompletion>,
    ) -> Option<BehaviorPolicyCandidateOf<T>> {
      let bound = source
        .policy
        .plan
        .bind(&source.policy.expression, source.binding)
        .ok()?;
      let episode = Self::behavior_episode_identity(
        source.actor.actor_id,
        &source.policy,
        source.binding.semantic_contract_id,
      );
      let mut monitor = source.monitor.clone();
      observation.completion = completion.map(|_| false);
      if source.policy.plan.requires_clean_cycle() {
        let current = monitor.as_mut()?;
        let flags = match (source_cycle, current.clean_cycle) {
          (CycleState::Idle, None) => CleanCycleCandidate::default(),
          // The authenticated source loader permits absent flags only for a pristine paid Run.
          (CycleState::Running, None) => CleanCycleCandidate::default(),
          (CycleState::Running | CycleState::Suspended, Some(clean))
            if clean.cycle_nonce == cycle_nonce =>
          {
            clean.candidate
          }
          _ => return None,
        }
        .observe(clean_step);
        observation.completion = flags.completion(completion);
        current.clean_cycle = match successor_cycle {
          CycleState::Idle => None,
          CycleState::Running | CycleState::Suspended => Some(BehaviorCleanCycle {
            cycle_nonce,
            candidate: flags,
          }),
        };
      }
      let transient;
      let state = match source.monitor.as_ref() {
        Some(monitor) => &monitor.state,
        None => {
          transient = bound.begin_episode(episode).ok()?;
          &transient
        }
      };
      let reduced = bound.reduce(state, episode, observation).ok()?;
      if let Some(monitor) = monitor.as_mut() {
        monitor.state = reduced.candidate;
      }
      Some(BehaviorPolicyCandidate {
        source,
        monitor,
        satisfied: reduced.satisfied,
      })
    }

    pub(crate) fn validate_step_behavior_source(source: &LoadedBehaviorPolicyOf<T>) -> bool {
      let Some(ActorSemanticState::Active(record)) =
        ActorSemanticStates::<T>::get(source.actor.actor_id)
      else {
        return false;
      };
      record.generation == source.actor.generation
        && record.admission.has_valid_identity()
        && record.admission.semantic_contract_id == source.binding.semantic_contract_id
        && record.admission.admission_identity == source.binding.admission_identity
        && ActorBehaviorPolicies::<T>::get(source.actor.actor_id).as_ref() == Some(&source.policy)
        && source.monitor.as_ref().is_none_or(|monitor| {
          ActorBehaviorMonitors::<T>::get(source.actor.actor_id).as_ref() == Some(monitor)
        })
    }

    pub(crate) fn commit_step_behavior_candidate(candidate: &BehaviorPolicyCandidateOf<T>) -> bool {
      if !Self::validate_step_behavior_source(&candidate.source) {
        return false;
      }
      if let Some(monitor) = &candidate.monitor {
        if candidate.source.monitor.as_ref() != Some(monitor) {
          ActorBehaviorMonitors::<T>::insert(candidate.source.actor.actor_id, monitor);
        }
      }
      true
    }

    pub(crate) fn clear_behavior_cycle_for_cancellation(
      actor_id: ActorId,
      admission: &ActorAdmissionCertificateOf<T>,
      identity: &ActorIdentityOf<T>,
      hot: &ActorHotStateOf<T>,
      closing: bool,
    ) -> DispatchResult {
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      ensure!(
        head.header.admission_identity == admission.admission_identity,
        Error::<T>::ActorInvariant
      );
      let policy = Self::load_contract_behavior_policy(actor_id, &head.header)
        .flatten()
        .ok_or(Error::<T>::ActorInvariant)?;
      if !policy.plan.requires_clean_cycle() {
        return Ok(());
      }
      let mut monitor = Self::load_contract_behavior_monitor(actor_id, &head.header, &policy)
        .flatten()
        .ok_or(Error::<T>::ActorInvariant)?;
      monitor.clean_cycle = None;
      ActorBehaviorMonitors::<T>::insert(actor_id, monitor);
      if closing {
        let Some(ActorSemanticState::Active(mut record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        record.identity = identity.clone();
        record.hot = hot.clone();
        ActorSemanticStates::<T>::insert(actor_id, ActorSemanticState::Active(record));
      }
      Ok(())
    }

    fn load_contract_behavior_monitor(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
      policy: &ActorBehaviorPolicyOf<T>,
    ) -> Option<Option<ActorBehaviorMonitor>> {
      if !Self::behavior_monitor_required(&policy.plan) {
        return Some(None);
      }
      let monitor = ActorBehaviorMonitors::<T>::get(actor_id)?;
      let bound = policy
        .plan
        .bind(
          &policy.expression,
          BehaviorPolicyBinding {
            semantic_contract_id: header.semantic_contract_id,
            admission_identity: header.admission_identity,
          },
        )
        .ok()?;
      let episode = Self::behavior_episode_identity(actor_id, policy, header.semantic_contract_id);
      bound.validate_state(&monitor.state, episode).ok()?;
      if policy.plan.requires_clean_cycle() {
        let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
          return None;
        };
        match (record.hot.cycle_state, monitor.clean_cycle.as_ref()) {
          (CycleState::Idle, None) => {}
          (CycleState::Running, None) if header.on_unfunded == OnUnfunded::AwaitFunding => {
            let run = ActorRunStateStore::<T>::get(actor_id)?;
            if run.cursor != 0
              || !run.running_is_coherent()
              || record.identity.cycle_nonce.checked_add(1) != Some(run.cycle_nonce)
              || !run.has_contract_authority(
                header.semantic_contract_id,
                header.body_commitment,
                header.admission_identity,
              )
            {
              return None;
            }
          }
          (CycleState::Running | CycleState::Suspended, Some(clean)) => {
            let run = ActorRunStateStore::<T>::get(actor_id)?;
            if clean.cycle_nonce != run.cycle_nonce
              || record.identity.cycle_nonce.checked_add(1) != Some(clean.cycle_nonce)
              || !run.has_contract_authority(
                header.semantic_contract_id,
                header.body_commitment,
                header.admission_identity,
              )
              || !(if record.hot.cycle_state == CycleState::Running {
                run.running_is_coherent()
              } else {
                run.suspension_is_coherent()
              })
            {
              return None;
            }
          }
          _ => return None,
        }
      } else if monitor.clean_cycle.is_some() {
        return None;
      }
      Some(Some(monitor))
    }

    /// Projects only authenticated current episode facts; it creates no monitor or observations.
    pub(crate) fn project_behavior_episode(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
    ) -> Result<Option<BehaviorEpisodeProjection>, ActorClassificationError> {
      if contract.close_when.is_none() {
        return Ok(None);
      }
      let invalid = ActorClassificationError::ActorInvariant;
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(invalid)?;
      let policy = Self::load_contract_behavior_policy(actor_id, &head.header)
        .flatten()
        .ok_or(invalid)?;
      let monitor =
        Self::load_contract_behavior_monitor(actor_id, &head.header, &policy).ok_or(invalid)?;
      let bound = policy
        .plan
        .bind(
          &policy.expression,
          BehaviorPolicyBinding {
            semantic_contract_id: head.header.semantic_contract_id,
            admission_identity: head.header.admission_identity,
          },
        )
        .map_err(|_| invalid)?;
      let episode =
        Self::behavior_episode_identity(actor_id, &policy, head.header.semantic_contract_id);
      let monitor = match monitor {
        None => None,
        Some(monitor) => {
          let mut capped_values = BoundedVec::default();
          for requirement in policy.plan.metrics() {
            if let Some(value) = bound
              .value(&monitor.state, episode, requirement.metric)
              .map_err(|_| invalid)?
            {
              capped_values
                .try_push(BehaviorMetricValue {
                  metric: requirement.metric,
                  value,
                })
                .map_err(|_| invalid)?;
            }
          }
          Some(BehaviorMonitorProjection {
            capped_values,
            clean_cycle: monitor.clean_cycle,
          })
        }
      };
      Ok(Some(BehaviorEpisodeProjection {
        episode_started_at_generation: policy.episode_started_at_generation,
        tracking_mode: policy.plan.mode(),
        requirements: policy
          .plan
          .metrics()
          .to_vec()
          .try_into()
          .map_err(|_| invalid)?,
        monitor,
      }))
    }

    fn prepare_contract_behavior_state(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
      previous: Option<&ActorContractHeaderOf<T>>,
    ) -> Option<PreparedBehaviorStateOf<T>> {
      let plan = Self::compile_contract_behavior_policy(contract, certificate)?;
      let old = match previous {
        Some(header) => Self::load_contract_behavior_policy(actor_id, header)?,
        None => None,
      };
      let remove_monitor = old
        .as_ref()
        .is_some_and(|policy| Self::behavior_monitor_required(&policy.plan));
      let Some(plan) = plan else {
        return Some((None, None, remove_monitor));
      };
      let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
        return None;
      };
      let generation = if previous.is_some() {
        if previous
          .is_some_and(|header| header.semantic_contract_id == certificate.semantic_contract_id)
        {
          record.generation
        } else {
          next_actor_generation(record.generation)?
        }
      } else {
        if record.admission != *certificate {
          return None;
        }
        record.generation
      };
      if generation == 0 {
        return None;
      }
      let mut policy = ActorBehaviorPolicy {
        expression: contract.close_when.as_ref()?.clone(),
        plan,
        episode_started_at_generation: generation,
      };
      let binding = BehaviorPolicyBinding {
        semantic_contract_id: certificate.semantic_contract_id,
        admission_identity: certificate.admission_identity,
      };
      let monitor = if let (Some(old), Some(header)) = (old, previous) {
        let old_monitor = Self::load_contract_behavior_monitor(actor_id, header, &old)?;
        if header.semantic_contract_id == certificate.semantic_contract_id {
          policy.episode_started_at_generation = old.episode_started_at_generation;
          let old_bound = old
            .plan
            .bind(
              &old.expression,
              BehaviorPolicyBinding {
                semantic_contract_id: header.semantic_contract_id,
                admission_identity: header.admission_identity,
              },
            )
            .ok()?;
          let bound = policy.plan.bind(&policy.expression, binding).ok()?;
          let episode =
            Self::behavior_episode_identity(actor_id, &policy, certificate.semantic_contract_id);
          match old_monitor {
            Some(mut monitor) => {
              monitor.state = bound
                .rebind_episode(&old_bound, &monitor.state, episode)
                .ok()?;
              Some(monitor)
            }
            None if !Self::behavior_monitor_required(&policy.plan) => None,
            None => return None,
          }
        } else {
          Self::new_contract_behavior_monitor(actor_id, &policy, binding)?
        }
      } else {
        Self::new_contract_behavior_monitor(actor_id, &policy, binding)?
      };
      Some((Some(policy), monitor, remove_monitor))
    }

    /// First-installation policy owner only; excludes caller admission, ordinary geometry and holds.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_install_contract_behavior_state(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<()> {
      let prepared = Self::prepare_contract_behavior_state(actor_id, contract, certificate, None)?;
      Self::replace_contract_behavior_policy(actor_id, false, prepared);
      Some(())
    }

    fn new_contract_behavior_monitor(
      actor_id: ActorId,
      policy: &ActorBehaviorPolicyOf<T>,
      binding: BehaviorPolicyBinding,
    ) -> Option<Option<ActorBehaviorMonitor>> {
      if !Self::behavior_monitor_required(&policy.plan) {
        return Some(None);
      }
      let episode = Self::behavior_episode_identity(actor_id, policy, binding.semantic_contract_id);
      let state = policy
        .plan
        .bind(&policy.expression, binding)
        .ok()?
        .begin_episode(episode)
        .ok()?;
      Some(Some(ActorBehaviorMonitor {
        state,
        clean_cycle: None,
      }))
    }

    fn replace_contract_behavior_policy(
      actor_id: ActorId,
      had_policy: bool,
      prepared: PreparedBehaviorStateOf<T>,
    ) {
      let (policy, monitor, remove_monitor) = prepared;
      if let Some(policy) = policy {
        ActorBehaviorPolicies::<T>::insert(actor_id, policy);
      } else if had_policy {
        ActorBehaviorPolicies::<T>::remove(actor_id);
      }
      if let Some(monitor) = monitor {
        ActorBehaviorMonitors::<T>::insert(actor_id, monitor);
      } else if remove_monitor {
        ActorBehaviorMonitors::<T>::remove(actor_id);
      }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_actor_step_ticket(
      actor_id: ActorId,
      queue_ticket: QueueTicket,
      eligible_at: BlockNumberFor<T>,
      identity: &ActorIdentityOf<T>,
      hot: &ActorHotStateOf<T>,
      run: Option<&ActorRunStateOf<T>>,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorStepTicketOf<T>> {
      if hot.queue_ticket != Some(queue_ticket) || !admission.has_valid_identity() {
        return None;
      }
      let (cycle_nonce, cursor, eligible_at) = match (hot.cycle_state, run) {
        (CycleState::Idle, None) => (identity.cycle_nonce.checked_add(1)?, 0, eligible_at),
        (CycleState::Running, Some(run))
          if run.running_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          if run.eligible_at != eligible_at {
            return None;
          }
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        (CycleState::Suspended, Some(run))
          if run.suspension_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          if run.eligible_at != eligible_at {
            return None;
          }
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        _ => return None,
      };
      Some(ActorStepTicket {
        actor_id,
        cycle_nonce,
        cursor,
        ticket: queue_ticket,
        eligible_at,
        contract_commitment: ActorContractCommitment {
          semantic_contract_id: admission.semantic_contract_id,
          body_commitment: admission.body_commitment,
        },
      })
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_current_step_plan(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      run: Option<ActorRunStateOf<T>>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: ActorStepTicketOf<T>,
      loaded_step: LoadedActorStepOf<T>,
      maximum_fee: StepFeeBreakdown<T::Balance>,
    ) -> Option<CurrentStepPlanOf<T>> {
      let queue_ticket = hot.queue_ticket?;
      if !Self::validate_loaded_step_authority(
        actor_id,
        queue_ticket,
        &admission,
        &ticket,
        &loaded_step,
      ) {
        return None;
      }
      match (hot.cycle_state, run.as_ref()) {
        (CycleState::Idle, None) => {
          if ticket.cursor != 0 || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)? {
            return None;
          }
        }
        (CycleState::Running, Some(run)) => {
          if !run.running_is_coherent()
            || !run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            )
            || ticket.cycle_nonce != run.cycle_nonce
            || ticket.cursor != run.cursor
            || ticket.eligible_at != run.eligible_at
          {
            return None;
          }
        }
        (CycleState::Suspended, Some(run)) => {
          if !run.suspension_is_coherent()
            || !run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            )
            || ticket.cycle_nonce != run.cycle_nonce
            || ticket.cursor != run.cursor
            || ticket.eligible_at != run.eligible_at
          {
            return None;
          }
        }
        _ => return None,
      }
      Some(StepExecutionPlan {
        identity,
        hot,
        run,
        admission,
        ticket: ActorStepAuthority {
          actor_id: ticket.actor_id,
          cycle_nonce: ticket.cycle_nonce,
          cursor: ticket.cursor,
          eligible_at: ticket.eligible_at,
          contract_commitment: ticket.contract_commitment,
        },
        loaded_step,
        maximum_fee,
        last_step_outcome: None,
        financial_wait: None,
        behavior_observation: Default::default(),
        normal_cycle_completion: None,
        clean_cycle_step: Default::default(),
      })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_canonical_current_step_plan(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      run: Option<ActorRunStateOf<T>>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: LoadedActorStepOf<T>,
      maximum_fee: StepFeeBreakdown<T::Balance>,
    ) -> Option<CurrentStepPlanOf<T>> {
      if !admission.has_valid_identity() {
        return None;
      }
      let (cycle_nonce, cursor, eligible_at) = match (hot.cycle_state, run.as_ref()) {
        (CycleState::Idle, None) => (
          identity.cycle_nonce.checked_add(1)?,
          0,
          frame_system::Pallet::<T>::block_number(),
        ),
        (CycleState::Running, Some(run)) if run.running_is_coherent() => {
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        (CycleState::Suspended, Some(run)) if run.suspension_is_coherent() => {
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        _ => return None,
      };
      if loaded_step.cursor != cursor
        || run.as_ref().is_some_and(|run| {
          !run.has_contract_authority(
            admission.semantic_contract_id,
            admission.body_commitment,
            admission.admission_identity,
          )
        })
      {
        return None;
      }
      Some(StepExecutionPlan {
        identity,
        hot,
        run,
        admission: admission.clone(),
        ticket: ActorStepAuthority {
          actor_id,
          cycle_nonce,
          cursor,
          eligible_at,
          contract_commitment: ActorContractCommitment {
            semantic_contract_id: admission.semantic_contract_id,
            body_commitment: admission.body_commitment,
          },
        },
        loaded_step,
        maximum_fee,
        last_step_outcome: None,
        financial_wait: None,
        behavior_observation: Default::default(),
        normal_cycle_completion: None,
        clean_cycle_step: Default::default(),
      })
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    #[allow(
      dead_code,
      reason = "legacy paged-FIFO Opening planner is reachable only from runtime-benchmark fixtures after the canonical Service cutover"
    )]
    pub(crate) fn load_current_step_plan_from_storage(
      ticket: ActorStepTicketOf<T>,
    ) -> Option<CurrentStepPlanOf<T>> {
      if ticket.eligible_at > frame_system::Pallet::<T>::block_number() {
        return None;
      }
      let actor_id = ticket.actor_id;
      let identity = Self::load_control_identity(actor_id)?;
      let hot = Self::load_control_hot(actor_id)?;
      let run = ActorRunStateStore::<T>::get(actor_id);
      let admission = Self::load_control_admission(actor_id)?;
      let loaded_step = Self::load_current_step_from_storage(actor_id, ticket.cursor)?;
      let maximum_fee =
        Self::maximum_current_step_fee(identity.actor_class.actor_type(), loaded_step.resources)
          .ok()?;
      if hot.queue_ticket.is_some() {
        return Self::build_current_step_plan(
          actor_id,
          identity,
          hot,
          run,
          admission,
          ticket,
          loaded_step,
          maximum_fee,
        );
      }
      // A canonically published Actor owns no legacy queue ticket, so its current-Step plan is the
      // generation-bound canonical successor rather than a paged-FIFO entry.
      Self::build_canonical_current_step_plan(
        actor_id,
        identity,
        hot,
        run,
        admission,
        loaded_step,
        maximum_fee,
      )
    }

    pub(crate) fn control_identity_from_scalar(
      identity: ActorIdentityOf<T>,
    ) -> Option<ActorControlIdentity<T::AccountId, BlockNumberFor<T>>> {
      let sovereign_account = match identity.actor_class {
        ActorClass::User { owner_slot } => Self::sovereign_account_id(&identity.owner, owner_slot),
        ActorClass::System { sovereign_id } => Self::sovereign_account_id_system(sovereign_id),
      };
      if sovereign_account != identity.sovereign_account {
        return None;
      }
      Some(ActorControlIdentity {
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        cycle_nonce: identity.cycle_nonce,
        last_control_mutation_block: identity.last_control_mutation_block,
      })
    }

    pub(crate) fn control_hot_from_scalar(
      hot: ActorHotStateOf<T>,
    ) -> ActorControlHotState<BlockNumberFor<T>> {
      ActorControlHotState {
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger_runtime_state: hot.trigger_runtime_state,
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        terminal_at: hot.terminal_at,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub(crate) fn control_identity_to_scalar(
      identity: &ActorControlIdentity<T::AccountId, BlockNumberFor<T>>,
    ) -> ActorIdentityOf<T> {
      let sovereign_account = match identity.actor_class {
        ActorClass::User { owner_slot } => Self::sovereign_account_id(&identity.owner, owner_slot),
        ActorClass::System { sovereign_id } => Self::sovereign_account_id_system(sovereign_id),
      };
      ActorIdentity {
        sovereign_account,
        owner: identity.owner.clone(),
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        cycle_nonce: identity.cycle_nonce,
        last_control_mutation_block: identity.last_control_mutation_block,
      }
    }

    pub(crate) fn control_hot_to_scalar(
      hot: &ActorControlHotState<BlockNumberFor<T>>,
      queue_ticket: Option<QueueTicket>,
    ) -> ActorHotStateOf<T> {
      ActorHotState {
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger_runtime_state: hot.trigger_runtime_state.clone(),
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        queue_ticket,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        terminal_at: hot.terminal_at,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_cell_from_parts(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
      eligible_at: Option<BlockNumberFor<T>>,
    ) -> Option<ActorControlCellOf<T>> {
      if !admission.has_valid_identity() {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: loaded_step.cursor,
        eligible_at,
        admission,
        resources: loaded_step.resources,
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_opening_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.queue_ticket != Some(ticket.ticket)
        || ticket.actor_id != actor_id
        || ticket.cursor != 0
        || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)?
        || ticket.eligible_at > frame_system::Pallet::<T>::block_number()
        || !Self::validate_loaded_step_authority(
          actor_id,
          ticket.ticket,
          &admission,
          ticket,
          loaded_step,
        )
      {
        return None;
      }
      Self::control_cell_from_parts(
        actor_id,
        identity,
        hot,
        admission,
        loaded_step,
        Some(ticket.eligible_at),
      )
    }

    #[cfg(feature = "runtime-benchmarks")]
    #[allow(
      dead_code,
      reason = "zero-Step frame projection remains test-only until the atomic production cutover"
    )]
    pub(crate) fn control_zero_step_opening_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.queue_ticket != Some(ticket.ticket)
        || ticket.actor_id != actor_id
        || ticket.cursor != 0
        || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)?
        || ticket.eligible_at > frame_system::Pallet::<T>::block_number()
        || !admission.has_valid_identity()
      {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: 0,
        eligible_at: Some(ticket.eligible_at),
        admission,
        resources: ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        },
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    #[allow(
      dead_code,
      reason = "zero-Step frame projection remains test-only until the atomic production cutover"
    )]
    pub(crate) fn control_zero_step_unsignaled_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.pending_signal
        || hot.queue_ticket.is_some()
        || hot.wakeup_pointer.is_some()
        || hot.trigger_wakeup_pointer.is_some()
        || !admission.has_valid_identity()
      {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: 0,
        eligible_at: None,
        admission,
        resources: ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        },
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_unsignaled_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.pending_signal
        || hot.queue_ticket.is_some()
        || hot.wakeup_pointer.is_some()
        || hot.trigger_wakeup_pointer.is_some()
        || loaded_step.cursor != 0
      {
        return None;
      }
      Self::control_cell_from_parts(actor_id, identity, hot, admission, loaded_step, None)
    }

    pub(crate) fn admission_matches_current_authority(
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      T::AdmissionCertificateAuthority::current().is_some_and(|authority| {
        admission.runtime_actor_semantics_version == authority.runtime_actor_semantics_version
          && admission.production_weight_identity == authority.production_weight_identity
          && admission.body_geometry_version == authority.body_geometry_version
          && admission.configured_bounds_commitment == authority.configured_bounds_commitment
          && admission.maximum_lifecycle_weight == authority.maximum_lifecycle_weight
      })
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    #[allow(
      dead_code,
      reason = "qualified host wake projection remains candidate-only until the atomic control cutover"
    )]
    pub(crate) fn project_control_cell_for_wake(
      cell: &ActorControlCellOf<T>,
      location: ActorControlLocation<BlockNumberFor<T>>,
      qualification: ActorWakeQualification,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      cell
        .admission
        .authorizes_wake(qualification)
        .then(|| Self::project_control_cell(cell, location))?
    }

    pub(crate) fn project_control_cell(
      cell: &ActorControlCellOf<T>,
      location: ActorControlLocation<BlockNumberFor<T>>,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      if !cell.admission.has_valid_identity()
        || !Self::admission_matches_current_authority(&cell.admission)
        || cell.pipeline_service_identity
          != pipeline_service_identity(cell.admission.admission_identity)
      {
        return None;
      }
      let queue_ticket = match location {
        ActorControlLocation::Unsignaled => {
          if cell.cursor != 0
            || cell.eligible_at.is_some()
            || (cell.hot.pending_signal && !cell.hot.lifecycle.is_paused())
            || cell.hot.cycle_state != CycleState::Idle
          {
            return None;
          }
          if let Some(pointer) = cell.hot.wakeup_pointer {
            let WakeupKey::Block(terminal_at) = pointer.block else {
              return None;
            };
            if cell.hot.terminal_at != Some(terminal_at)
              || !Self::wakeup_page_entry_matches(pointer, cell.actor_id)
            {
              return None;
            }
          }
          if let Some(pointer) = cell.hot.trigger_wakeup_pointer {
            if ActorControlLocators::<T>::get(cell.actor_id)
              != Some(ActorControlLocation::Unsignaled)
            {
              return None;
            }
            let reference = WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            };
            if !Self::wakeup_page_entry_matches(reference, cell.actor_id) {
              return None;
            }
          }
          None
        }
        ActorControlLocation::Ready { ticket } => {
          let terminal_ready = cell.hot.cycle_state == CycleState::Idle
            && !cell.hot.pending_signal
            && cell
              .hot
              .terminal_at
              .zip(cell.eligible_at)
              .is_some_and(|(terminal_at, eligible_at)| eligible_at >= terminal_at);
          if cell.eligible_at.is_none()
            || !(matches!(
              cell.hot.cycle_state,
              CycleState::Running | CycleState::Suspended
            ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
              || terminal_ready)
          {
            return None;
          }
          Some(ticket)
        }
        ActorControlLocation::Waiting { key, .. } => {
          if let Some(pointer) = cell.hot.wakeup_pointer {
            let terminal_waiting = cell.hot.cycle_state == CycleState::Idle
              && !cell.hot.pending_signal
              && cell
                .hot
                .terminal_at
                .zip(cell.eligible_at)
                .is_some_and(|(terminal_at, eligible_at)| eligible_at >= terminal_at);
            if pointer.block != key
              || cell.eligible_at.is_none()
              || !(matches!(
                cell.hot.cycle_state,
                CycleState::Running | CycleState::Suspended
              ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
                || terminal_waiting)
            {
              return None;
            }
          } else {
            let pointer = cell.hot.trigger_wakeup_pointer?;
            let reference = WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            };
            if key != reference.block
              || cell.eligible_at.is_some()
              || cell.hot.pending_signal
              || cell.hot.cycle_state != CycleState::Idle
            {
              return None;
            }
          }
          None
        }
      };
      Some((
        Self::control_identity_to_scalar(&cell.identity),
        Self::control_hot_to_scalar(&cell.hot, queue_ticket),
        cell.admission.clone(),
      ))
    }

    /// Publishes one already-inventoried legacy transition only inside its caller's transaction.
    /// The legacy locator must have been removed first, so failure rolls the whole authority move
    /// back rather than creating dual process residence.
    #[allow(
      dead_code,
      reason = "publication helper remains unreachable until all legacy mutation cohorts cut over together"
    )]
    pub(crate) fn publish_legacy_process_transition(
      actor_id: ActorId,
      current: ActorProcessOf<T>,
      obligation: ProcessTransitionObligation,
      transition: LegacyProcessTransition<BlockNumberFor<T>>,
    ) -> Result<ActorProcessOf<T>, ProcessPublicationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ProcessPublicationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      {
        return Err(ProcessPublicationError::LegacyAuthorityPresent);
      }

      let stored = ActorProcesses::<T>::get(actor_id);
      match transition {
        LegacyProcessTransition::Publish(_) if stored.is_some() => {
          return Err(ProcessPublicationError::ProcessAlreadyExists);
        }
        LegacyProcessTransition::Publish(_) => {}
        _ => match stored {
          None => return Err(ProcessPublicationError::ProcessMissing),
          Some(stored) if stored != current => {
            return Err(ProcessPublicationError::CurrentProcessMismatch);
          }
          Some(_) => {}
        },
      }

      let next = plan_legacy_process_transition(current, obligation, transition)
        .map_err(ProcessPublicationError::Transition)?;
      ActorProcesses::<T>::insert(actor_id, next);
      Ok(next)
    }

    /// Atomically publishes one typed service process and inserts its generation-bound ring node.
    /// This is the complete canonical carrier owner for a caller that has already removed legacy
    /// authority; either both storage surfaces commit or neither does.
    #[allow(
      dead_code,
      reason = "atomic service publication remains unreachable until supported callers cut over"
    )]
    pub(crate) fn publish_service_member(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServicePublicationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let current = ActorProcess {
          generation: actor.generation,
          last_attempted: None,
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Service(kind)),
        };
        let result = Self::publish_legacy_process_transition(
          actor.actor_id,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(LegacyProcessPlacement::Ready(kind)),
        )
        .map_err(ServicePublicationError::Process)
        .and_then(|_| {
          Self::insert_service_member(actor, kind, now).map_err(ServicePublicationError::Ring)
        });
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Appends one generation-bound process to the inert service ring. The caller's transaction
    /// must publish the matching process and remove legacy authority before entering this boundary.
    pub(crate) fn insert_service_member(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRingMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRingMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ServiceRingMutationError::LegacyAuthorityPresent);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::ProcessMissing)?;
      if !Self::process_has_service_authority(actor, &process, kind) {
        return Err(ServiceRingMutationError::ProcessResidenceMismatch);
      }
      if ServiceNodes::<T>::contains_key(actor.actor_id) {
        return Err(ServiceRingMutationError::MemberAlreadyExists);
      }

      let mut header = ServiceHeader::<T>::get();
      match header.round_block {
        Some(round) if round > now => return Err(ServiceRingMutationError::CorruptRing),
        Some(round) if round == now => {}
        _ => header.round_block = Some(now),
      }
      let eligible_from = now
        .checked_add(&One::one())
        .ok_or(ServiceRingMutationError::BlockNumberOverflow)?;
      let next_count = header
        .count
        .checked_add(1)
        .ok_or(ServiceRingMutationError::CapacityExceeded)?;
      let node = ServiceNode {
        generation: actor.generation,
        previous: actor,
        next: actor,
        kind,
        eligible_from,
        last_considered: now,
      };
      match (header.count, header.cursor) {
        (0, None) => header.cursor = Some(actor),
        (0, Some(_)) | (_, None) => return Err(ServiceRingMutationError::CorruptRing),
        (_, Some(cursor)) => {
          let mut head = ServiceNodes::<T>::get(cursor.actor_id)
            .filter(|node| node.generation == cursor.generation)
            .ok_or(ServiceRingMutationError::CorruptRing)?;
          let tail_ref = head.previous;
          let mut tail = ServiceNodes::<T>::get(tail_ref.actor_id)
            .filter(|node| node.generation == tail_ref.generation)
            .ok_or(ServiceRingMutationError::CorruptRing)?;
          if tail.next != cursor
            || (header.count == 1
              && (tail_ref != cursor || head.next != cursor || head.previous != cursor))
            || (header.count > 1 && tail_ref == cursor)
          {
            return Err(ServiceRingMutationError::CorruptRing);
          }
          head.previous = actor;
          tail.next = actor;
          if cursor.actor_id == tail_ref.actor_id {
            head.next = actor;
            ServiceNodes::<T>::insert(cursor.actor_id, head);
          } else {
            ServiceNodes::<T>::insert(cursor.actor_id, head);
            ServiceNodes::<T>::insert(tail_ref.actor_id, tail);
          }
          ServiceNodes::<T>::insert(
            actor.actor_id,
            ServiceNode {
              previous: tail_ref,
              next: cursor,
              ..node
            },
          );
        }
      }
      if header.count == 0 {
        ServiceNodes::<T>::insert(actor.actor_id, node);
      }
      header.count = next_count;
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Opens or resumes exactly one immutable block round without moving its persistent cursor.
    pub(crate) fn begin_service_round(now: BlockNumberFor<T>) -> Result<(), ServiceRoundError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRoundError::TransactionRequired);
      }
      let mut header = ServiceHeader::<T>::get();
      if header.count == 0 {
        if header.cursor.is_some() {
          return Err(ServiceRoundError::CorruptRing);
        }
      } else if header.cursor.is_none() {
        return Err(ServiceRoundError::CorruptRing);
      }
      match header.round_block {
        Some(round) if round > now => Err(ServiceRoundError::RoundFromFuture),
        Some(round) if round == now => Ok(()),
        _ => {
          header.round_block = Some(now);
          ServiceHeader::<T>::put(header);
          Ok(())
        }
      }
    }

    /// Classifies the current frontier without changing a blocked or closed head.
    pub(crate) fn consider_service_head(
      now: BlockNumberFor<T>,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRoundError::TransactionRequired);
      }
      let header = ServiceHeader::<T>::get();
      if header.round_block != Some(now) {
        return Err(ServiceRoundError::RoundNotStarted);
      }
      let Some(actor) = header.cursor else {
        return if header.count == 0 {
          Ok(ServiceRoundEncounter::Empty)
        } else {
          Err(ServiceRoundError::CorruptRing)
        };
      };
      if header.count == 0 {
        return Err(ServiceRoundError::CorruptRing);
      }
      let node = ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      if node.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRoundError::ProcessMissing)?;
      if process.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      if !matches!(
        process.status,
        ProcessStatus::Serving | ProcessStatus::AwaitingFunding
      ) || process.residence != Some(ProcessResidence::Service(node.kind))
      {
        return Err(ServiceRoundError::ProcessResidenceMismatch);
      }
      if node.last_considered > now {
        return Err(ServiceRoundError::RoundFromFuture);
      }
      if node.last_considered == now {
        return Ok(ServiceRoundEncounter::Closed);
      }
      if node.eligible_from > now {
        return Err(ServiceRoundError::FutureMemberUnmarked);
      }
      match process.last_attempted {
        Some(attempted) if attempted > now => Err(ServiceRoundError::AttemptFromFuture),
        Some(attempted) if attempted == now => Ok(ServiceRoundEncounter::AlreadyAttempted(actor)),
        _ => Ok(ServiceRoundEncounter::Eligible(actor)),
      }
    }

    /// Commits one successful retained consideration and advances to its captured successor.
    pub(crate) fn advance_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      let mut process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRoundError::ProcessMissing)?;
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      process.last_attempted = Some(now);
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ActorProcesses::<T>::insert(actor.actor_id, process);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Advances one already-attempted defensive encounter without admitting another attempt.
    #[allow(
      dead_code,
      reason = "retained for direct canonical Service topology and rollback regressions"
    )]
    pub(crate) fn converge_attempted_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::AlreadyAttempted(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Advances one eligible member that holds no admitted work for this round without executing a
    /// Step or recording an attempt. The completed/aborted Idle resident stays in the ring; only the
    /// bounded cursor advances so a subsequently inserted independent member becomes serviceable.
    pub(crate) fn advance_idle_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      if node.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Resolves one watched asset to a collision-free retained causal source identity.
    pub(crate) fn resolve_balance_dependency_source(
      asset: T::AssetId,
    ) -> Result<DependencySourceMutation, DependencySourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencySourceError::TransactionRequired);
      }
      if let Some(source) = BalanceDependencySources::<T>::get(asset) {
        return match DependencySourceBalances::<T>::get(source) {
          Some(reverse) if reverse == asset => Ok(DependencySourceMutation::Existing(source)),
          Some(_) => Err(DependencySourceError::ReverseMismatch),
          None => Err(DependencySourceError::ReverseMissing),
        };
      }
      let mut allocator = DependencySourceAllocatorState::<T>::get();
      if allocator.exhausted {
        return Err(DependencySourceError::Exhausted);
      }
      let source = allocator.next;
      if DependencySourceBalances::<T>::contains_key(source) {
        return Err(DependencySourceError::SourceOccupied);
      }
      match source.checked_add(1) {
        Some(next) => allocator.next = next,
        None => allocator.exhausted = true,
      }
      BalanceDependencySources::<T>::insert(asset, source);
      DependencySourceBalances::<T>::insert(source, asset);
      DependencySourceAllocatorState::<T>::put(allocator);
      Ok(DependencySourceMutation::Allocated(source))
    }

    /// Resolves one watched asset and publishes its causal dependency revision.
    pub(crate) fn publish_balance_dependency_event(asset: T::AssetId) -> DispatchResult {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DispatchError::Other(
          "dependency event requires transaction",
        ));
      }
      let Some(source) = BalanceDependencySources::<T>::get(asset) else {
        return Ok(());
      };
      Self::ensure_machine_unfrozen()?;
      if DependencySourceBalances::<T>::get(source) != Some(asset) {
        return Err(DispatchError::Other("dependency source reverse mismatch"));
      }
      match Self::publish_dependency_event_with_source_retention(source) {
        Ok(
          DependencyPublicationMutation::Begun(_) | DependencyPublicationMutation::Coalesced { .. },
        ) => Ok(()),
        Ok(DependencyPublicationMutation::Exhausted) => {
          Err(DispatchError::Other("dependency revision exhausted"))
        }
        Err(DependencyPublicationError::Revision(DependencyRevisionError::TransactionRequired))
        | Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::TransactionRequired,
        )) => Err(DispatchError::Other(
          "dependency event requires transaction",
        )),
        Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::CapacityExceeded,
        )) => Err(DispatchError::Other(
          "dependency scan source capacity reached",
        )),
        Err(DependencyPublicationError::SourceCarrier(DependencyScanSourceError::ScanInactive)) => {
          Err(DispatchError::Other("dependency scan source inactive"))
        }
        Err(DependencyPublicationError::SourceCarrier(DependencyScanSourceError::Missing)) => {
          Err(DispatchError::Other("dependency scan source missing"))
        }
        Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::CorruptTopology,
        ))
        | Err(DependencyPublicationError::Revision(DependencyRevisionError::CorruptTopology)) => {
          Err(DispatchError::Other(
            "dependency scan source topology corrupt",
          ))
        }
      }
    }

    /// Advances one event-complete dependency source without wrapping its causal identity.
    #[allow(
      dead_code,
      reason = "dependency revisions remain inert until parking authority cutover"
    )]
    pub(crate) fn revise_dependency_source(
      source: DependencySourceId,
    ) -> Result<DependencyRevisionMutation, DependencyRevisionError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRevisionError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Ok(DependencyRevisionMutation::Exhausted);
      }
      let Some(next) = state.revision.checked_add(1) else {
        state.exhausted = true;
        DependencyRevisions::<T>::insert(source, state);
        return Ok(DependencyRevisionMutation::Exhausted);
      };
      state.revision = next;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyRevisionMutation::Advanced(next))
    }

    fn dependency_scan_start(header: &DependencyRegistrationHeader) -> Option<u64> {
      match header.first_page {
        Some(page) if header.count > 0 => page
          .checked_mul(32)
          .filter(|start| *start < header.next_index),
        None if header.count == 0 => Some(header.next_index),
        _ => None,
      }
    }

    /// Publishes one event-complete revision and retains exactly one fixed-target scan.
    #[allow(
      dead_code,
      reason = "dependency publication remains inert until parking authority cutover"
    )]
    pub(crate) fn publish_dependency_event(
      source: DependencySourceId,
    ) -> Result<DependencyPublicationMutation, DependencyRevisionError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRevisionError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Ok(DependencyPublicationMutation::Exhausted);
      }
      let Some(next) = state.revision.checked_add(1) else {
        state.exhausted = true;
        DependencyRevisions::<T>::insert(source, state);
        return Ok(DependencyPublicationMutation::Exhausted);
      };
      state.revision = next;
      let outcome = if let Some(active_target) = state.scan_target {
        DependencyPublicationMutation::Coalesced {
          revision: next,
          active_target,
        }
      } else {
        state.scan_target = Some(next);
        let header = DependencyRegistrationHeaders::<T>::get(source);
        state.scan_end = header.next_index;
        state.scan_cursor =
          Self::dependency_scan_start(&header).ok_or(DependencyRevisionError::CorruptTopology)?;
        DependencyPublicationMutation::Begun(next)
      };
      DependencyRevisions::<T>::insert(source, state);
      Ok(outcome)
    }

    /// Publishes one revision and transactionally retains its active source for fair scanning.
    pub(crate) fn publish_dependency_event_with_source_retention(
      source: DependencySourceId,
    ) -> Result<DependencyPublicationMutation, DependencyPublicationError> {
      let publication =
        Self::publish_dependency_event(source).map_err(DependencyPublicationError::Revision)?;
      if matches!(publication, DependencyPublicationMutation::Exhausted) {
        return Ok(publication);
      }
      Self::insert_dependency_scan_source(source)
        .map_err(DependencyPublicationError::SourceCarrier)?;
      Ok(publication)
    }

    fn dependency_scan_source_capacity() -> u32 {
      T::MaxActiveActors::get().saturating_mul(T::MaxParkedBalanceWatches::get())
    }

    /// Inserts one active source into the exact fair scan selector.
    #[allow(
      dead_code,
      reason = "dependency scan carrier remains inert until weighted cutover"
    )]
    pub(crate) fn insert_dependency_scan_source(
      source: DependencySourceId,
    ) -> Result<DependencyScanSourceMutation, DependencyScanSourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanSourceError::TransactionRequired);
      }
      if DependencyRevisions::<T>::get(source).scan_target.is_none() {
        return Err(DependencyScanSourceError::ScanInactive);
      }
      if DependencyScanSourceNodes::<T>::contains_key(source) {
        return Ok(DependencyScanSourceMutation::AlreadyActive);
      }
      let mut list = DependencyScanSourceListState::<T>::get();
      if list.count >= Self::dependency_scan_source_capacity() {
        return Err(DependencyScanSourceError::CapacityExceeded);
      }
      match list.cursor {
        None => {
          if list.count != 0 {
            return Err(DependencyScanSourceError::CorruptTopology);
          }
          DependencyScanSourceNodes::<T>::insert(
            source,
            DependencyScanSourceNode {
              previous: source,
              next: source,
            },
          );
          list.cursor = Some(source);
        }
        Some(cursor) => {
          let mut cursor_node = DependencyScanSourceNodes::<T>::get(cursor)
            .ok_or(DependencyScanSourceError::CorruptTopology)?;
          let tail = cursor_node.previous;
          if list.count == 1 {
            if tail != cursor || cursor_node.next != cursor {
              return Err(DependencyScanSourceError::CorruptTopology);
            }
            cursor_node.previous = source;
            cursor_node.next = source;
            DependencyScanSourceNodes::<T>::insert(cursor, cursor_node);
          } else {
            let mut tail_node = DependencyScanSourceNodes::<T>::get(tail)
              .ok_or(DependencyScanSourceError::CorruptTopology)?;
            if tail_node.next != cursor {
              return Err(DependencyScanSourceError::CorruptTopology);
            }
            tail_node.next = source;
            cursor_node.previous = source;
            DependencyScanSourceNodes::<T>::insert(tail, tail_node);
            DependencyScanSourceNodes::<T>::insert(cursor, cursor_node);
          }
          DependencyScanSourceNodes::<T>::insert(
            source,
            DependencyScanSourceNode {
              previous: tail,
              next: cursor,
            },
          );
        }
      }
      list.count = list
        .count
        .checked_add(1)
        .ok_or(DependencyScanSourceError::CapacityExceeded)?;
      DependencyScanSourceListState::<T>::put(list);
      Ok(DependencyScanSourceMutation::Inserted)
    }

    /// Removes one completed source while preserving a fair successor cursor.
    #[allow(
      dead_code,
      reason = "dependency scan carrier remains inert until weighted cutover"
    )]
    pub(crate) fn remove_dependency_scan_source(
      source: DependencySourceId,
    ) -> Result<DependencyScanSourceMutation, DependencyScanSourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanSourceError::TransactionRequired);
      }
      if DependencyRevisions::<T>::get(source).scan_target.is_some() {
        return Err(DependencyScanSourceError::ScanInactive);
      }
      let node =
        DependencyScanSourceNodes::<T>::get(source).ok_or(DependencyScanSourceError::Missing)?;
      let mut list = DependencyScanSourceListState::<T>::get();
      if list.count == 0 || list.cursor.is_none() {
        return Err(DependencyScanSourceError::CorruptTopology);
      }
      if list.count == 1 {
        if list.cursor != Some(source) || node.previous != source || node.next != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        list.cursor = None;
      } else if list.count == 2 {
        if node.previous != node.next {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        let survivor = node.next;
        let survivor_node = DependencyScanSourceNodes::<T>::get(survivor)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        if survivor_node.previous != source || survivor_node.next != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        DependencyScanSourceNodes::<T>::insert(
          survivor,
          DependencyScanSourceNode {
            previous: survivor,
            next: survivor,
          },
        );
        if list.cursor == Some(source) {
          list.cursor = Some(survivor);
        }
      } else {
        let mut previous = DependencyScanSourceNodes::<T>::get(node.previous)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        let mut next = DependencyScanSourceNodes::<T>::get(node.next)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        if previous.next != source || next.previous != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        previous.next = node.next;
        next.previous = node.previous;
        DependencyScanSourceNodes::<T>::insert(node.previous, previous);
        DependencyScanSourceNodes::<T>::insert(node.next, next);
        if list.cursor == Some(source) {
          list.cursor = Some(node.next);
        }
      }
      list.count = list
        .count
        .checked_sub(1)
        .ok_or(DependencyScanSourceError::CorruptTopology)?;
      DependencyScanSourceNodes::<T>::remove(source);
      DependencyScanSourceListState::<T>::put(list);
      Ok(DependencyScanSourceMutation::Removed)
    }

    /// Starts one fixed-revision source scan without disturbing an already-active target.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn begin_dependency_scan(
      source: DependencySourceId,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.is_some() {
        return Err(DependencyScanError::ScanAlreadyActive);
      }
      state.scan_target = Some(state.revision);
      let header = DependencyRegistrationHeaders::<T>::get(source);
      state.scan_end = header.next_index;
      state.scan_cursor =
        Self::dependency_scan_start(&header).ok_or(DependencyScanError::CorruptTopology)?;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyScanMutation::Begun(state.revision))
    }

    /// Processes one cursor-derived registration before advancing the fixed scan frontier.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn process_dependency_scan_member(
      source: DependencySourceId,
      expected_target: DependencyRevision,
      expected_cursor: u64,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.ok_or(DependencyScanError::ScanMissing)? != expected_target {
        return Err(DependencyScanError::TargetMismatch);
      }
      if state.scan_cursor != expected_cursor {
        return Err(DependencyScanError::CursorMismatch);
      }
      let header = DependencyRegistrationHeaders::<T>::get(source);
      if state.scan_cursor >= state.scan_end || state.scan_end > header.next_index {
        return Err(DependencyScanError::ScanComplete);
      }
      let page_id = state.scan_cursor / 32;
      let slot = (state.scan_cursor % 32) as usize;
      let position = DependencyRegistrationPosition {
        page: page_id,
        slot: slot as u8,
      };
      let mut page = DependencyRegistrationPages::<T>::get(source, page_id)
        .ok_or(DependencyScanError::CorruptTopology)?;
      let next_page_cursor = match page.next {
        Some(next) if next > page_id => next
          .checked_mul(32)
          .ok_or(DependencyScanError::CorruptTopology)?
          .min(state.scan_end),
        None => state.scan_end,
        _ => return Err(DependencyScanError::CorruptTopology),
      };
      let next_cursor = if slot == 31 {
        next_page_cursor
      } else {
        state
          .scan_cursor
          .checked_add(1)
          .ok_or(DependencyScanError::CursorExhausted)?
      };
      let encountered = *page
        .entries
        .get(slot)
        .ok_or(DependencyScanError::CorruptTopology)?;
      if let Some(handle) = encountered {
        let current = DependencyRegistrations::<T>::get(source, handle.actor.actor_id);
        if current == Some(handle) {
          if DependencyRegistrationPositions::<T>::get(source, handle.actor.actor_id)
            != Some(position)
          {
            return Err(DependencyScanError::CorruptRegistrationPosition);
          }
          let pending = PendingCheckOwners::<T>::get(handle.actor.actor_id)
            .ok_or(DependencyScanError::PendingAuthorityMissing)?;
          if pending
            != (PendingCheckOwner {
              actor: handle.actor,
              plan_revision: handle.plan_revision,
            })
          {
            return Err(DependencyScanError::PendingAuthorityMismatch);
          }
          if handle.acknowledged_revision < expected_target {
            if PendingDependencyReviews::<T>::contains_key(handle.actor.actor_id) {
              return Err(DependencyScanError::PendingDestinationMismatch);
            }
            let destination = PendingDependencyEvent {
              owner: pending,
              source,
              revision: expected_target,
            };
            match PendingDependencyEvents::<T>::get(handle.actor.actor_id) {
              None => PendingDependencyEvents::<T>::insert(handle.actor.actor_id, destination),
              Some(current) if current.owner == pending => {}
              Some(_) => return Err(DependencyScanError::PendingDestinationMismatch),
            }
            let acknowledged = DependencyRegistrationHandle {
              acknowledged_revision: expected_target,
              ..handle
            };
            let mut plan = DependencyPlans::<T>::get(handle.actor.actor_id);
            if !plan.is_empty() {
              let registration = plan
                .iter_mut(/* deos-bypass: bounded-iter -- complete plan is MaxParkedBalanceWatches-bounded. */)
                .find(|registration| registration.source == source)
                .ok_or(DependencyScanError::CorruptTopology)?;
              if registration.handle != handle {
                return Err(DependencyScanError::CorruptTopology);
              }
              registration.handle = acknowledged;
              DependencyPlans::<T>::insert(handle.actor.actor_id, plan);
            }
            page.entries[slot] = Some(acknowledged);
            DependencyRegistrationPages::<T>::insert(source, page_id, page);
            DependencyRegistrations::<T>::insert(source, acknowledged.actor.actor_id, acknowledged);
          }
        } else {
          let reverse_position =
            DependencyRegistrationPositions::<T>::get(source, handle.actor.actor_id);
          if current.is_some() && reverse_position == Some(position) {
            return Err(DependencyScanError::CorruptRegistrationPosition);
          }
          Self::retire_dependency_registration_position(source, position, &mut page)
            .map_err(|_| DependencyScanError::CorruptTopology)?;
          if current.is_none() && reverse_position == Some(position) {
            DependencyRegistrationPositions::<T>::remove(source, handle.actor.actor_id);
          }
          if !DependencyRegistrationHeaders::<T>::contains_key(source) {
            return Ok(DependencyScanMutation::Advanced(state.scan_end));
          }
          if !DependencyRegistrationPages::<T>::contains_key(source, page_id) {
            state.scan_cursor = next_page_cursor;
            DependencyRevisions::<T>::insert(source, state);
            return Ok(DependencyScanMutation::Advanced(state.scan_cursor));
          }
        }
      }
      state.scan_cursor = next_cursor;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyScanMutation::Advanced(state.scan_cursor))
    }

    /// Completes only the fixed target and immediately retains the newest revision as successor.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn complete_dependency_scan(
      source: DependencySourceId,
      expected_target: DependencyRevision,
      expected_cursor: u64,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.ok_or(DependencyScanError::ScanMissing)? != expected_target {
        return Err(DependencyScanError::TargetMismatch);
      }
      if state.scan_cursor != expected_cursor {
        return Err(DependencyScanError::CursorMismatch);
      }
      if state.scan_cursor != state.scan_end {
        return Err(DependencyScanError::ScanComplete);
      }
      let outcome = if state.revision > expected_target {
        state.scan_target = Some(state.revision);
        let header = DependencyRegistrationHeaders::<T>::get(source);
        state.scan_end = header.next_index;
        state.scan_cursor =
          Self::dependency_scan_start(&header).ok_or(DependencyScanError::CorruptTopology)?;
        DependencyScanMutation::HandedOff(state.revision)
      } else {
        state.scan_target = None;
        state.scan_cursor = 0;
        state.scan_end = 0;
        DependencyScanMutation::Completed
      };
      DependencyRevisions::<T>::insert(source, state);
      Ok(outcome)
    }

    /// Resource-admits the bounded source-selector probe without reading source-owned work first.
    pub(crate) fn dependency_scan_source_probe(
      meter: &mut WeightMeter,
    ) -> Result<bool, DependencyScanWorkerError> {
      let weight = T::WeightInfo::dependency_scan_source_probe();
      if !meter.can_consume(weight) {
        return Err(DependencyScanWorkerError::InsufficientWeight);
      }
      meter.consume(weight);
      let list = DependencyScanSourceListState::<T>::get();
      match (list.cursor, list.count) {
        (None, 0) => Ok(false),
        (Some(_), count) if count > 0 => Ok(true),
        _ => Err(DependencyScanWorkerError::Source(
          DependencyScanSourceError::CorruptTopology,
        )),
      }
    }

    /// Resource-admits one fair dependency-source unit: one retained registration position or one
    /// exact scan completion/handoff. Refusal performs no selector read; admitted failure rolls
    /// back registration, Pending, cursor, revision, and source-carrier mutation together.
    #[allow(
      dead_code,
      reason = "dependency scan worker awaits prepass resource-composition cutover"
    )]
    pub(crate) fn process_next_dependency_scan_unit(
      meter: &mut WeightMeter,
    ) -> Result<Option<DependencyScanMutation>, DependencyScanWorkerError> {
      let weight = T::WeightInfo::process_dependency_scan_unit()
        .max(T::WeightInfo::process_dependency_scan_completion_unit());
      if !meter.can_consume(weight) {
        return Err(DependencyScanWorkerError::InsufficientWeight);
      }
      meter.consume(weight);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let mut list = DependencyScanSourceListState::<T>::get();
          let Some(source) = list.cursor else {
            if list.count != 0 {
              return Err(DependencyScanWorkerError::Source(
                DependencyScanSourceError::CorruptTopology,
              ));
            }
            return Ok(None);
          };
          if list.count == 0 {
            return Err(DependencyScanWorkerError::Source(
              DependencyScanSourceError::CorruptTopology,
            ));
          }
          let node = DependencyScanSourceNodes::<T>::get(source).ok_or(
            DependencyScanWorkerError::Source(DependencyScanSourceError::Missing),
          )?;
          let state = DependencyRevisions::<T>::get(source);
          let target = state.scan_target.ok_or(DependencyScanWorkerError::Scan(
            DependencyScanError::ScanMissing,
          ))?;
          let mutation = if state.scan_cursor < state.scan_end {
            let mutation = Self::process_dependency_scan_member(source, target, state.scan_cursor)
              .map_err(DependencyScanWorkerError::Scan)?;
            if DependencyScanSourceNodes::<T>::contains_key(source) {
              list.cursor = Some(node.next);
              DependencyScanSourceListState::<T>::put(list);
            }
            mutation
          } else {
            let mutation = Self::complete_dependency_scan(source, target, state.scan_cursor)
              .map_err(DependencyScanWorkerError::Scan)?;
            match mutation {
              DependencyScanMutation::Completed => {
                Self::remove_dependency_scan_source(source)
                  .map_err(DependencyScanWorkerError::Source)?;
              }
              DependencyScanMutation::HandedOff(_) => {
                list.cursor = Some(node.next);
                DependencyScanSourceListState::<T>::put(list);
              }
              _ => {
                return Err(DependencyScanWorkerError::Scan(
                  DependencyScanError::CorruptTopology,
                ));
              }
            }
            mutation
          };
          Ok(Some(mutation))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn dependency_registration_position(
      source: DependencySourceId,
      actor_id: ActorId,
      expected: DependencyRegistrationHandle,
    ) -> Result<DependencyRegistrationPosition, DependencyRegistrationError> {
      let position = DependencyRegistrationPositions::<T>::get(source, actor_id)
        .ok_or(DependencyRegistrationError::PositionMissing)?;
      let page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if page
        .entries
        .get(position.slot as usize)
        .and_then(Option::as_ref)
        != Some(&expected)
      {
        return Err(DependencyRegistrationError::PositionMismatch);
      }
      Ok(position)
    }

    fn validate_dependency_registration(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationHandle, DependencyRegistrationError> {
      let pending = PendingCheckOwners::<T>::get(owner.actor.actor_id)
        .ok_or(DependencyRegistrationError::PendingOwnerMissing)?;
      if pending != owner {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if acknowledged_revision > state.revision {
        return Err(DependencyRegistrationError::RevisionFromFuture);
      }
      Ok(DependencyRegistrationHandle {
        actor: owner.actor,
        plan_revision: owner.plan_revision,
        acknowledged_revision,
      })
    }

    /// Commits one negative evaluation only while its exact owner and observed source remain current.
    #[allow(
      dead_code,
      reason = "negative evaluation remains inert until parking authority cutover"
    )]
    pub(crate) fn commit_negative_dependency_evaluation(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      observed_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if state.revision != observed_revision {
        return Err(DependencyRegistrationError::RevisionMismatch);
      }
      match DependencyRegistrations::<T>::get(source, owner.actor.actor_id) {
        Some(current) => {
          Self::replace_dependency_registration(source, current, owner, observed_revision)
        }
        None => Self::install_dependency_registration(source, owner, observed_revision),
      }
    }

    /// Replaces one explicitly complete dependency plan after validating every old and new source.
    #[allow(
      dead_code,
      reason = "complete dependency plans remain inert until parking authority cutover"
    )]
    pub(crate) fn commit_negative_dependency_plan(
      owner: PendingCheckOwner,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      if desired.len() > T::MaxParkedBalanceWatches::get() as usize {
        return Err(DependencyRegistrationError::PlanTooLarge);
      }
      if PendingCheckOwners::<T>::get(owner.actor.actor_id) != Some(owner) {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      for (index, entry) in desired
        .iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the complete plan. */)
        .enumerate()
      {
        if desired[..index]
          .iter(/* deos-bypass: bounded-iter -- a prefix of the MaxParkedBalanceWatches plan. */)
          .any(|prior| prior.source == entry.source)
        {
          return Err(DependencyRegistrationError::DuplicateSource);
        }
        let state = DependencyRevisions::<T>::get(entry.source);
        if state.exhausted {
          return Err(DependencyRegistrationError::SourceExhausted);
        }
        if state.revision != entry.observed_revision {
          return Err(DependencyRegistrationError::RevisionMismatch);
        }
      }

      let old = DependencyPlans::<T>::get(owner.actor.actor_id);
      for registration in &old {
        if registration.handle.actor.actor_id != owner.actor.actor_id
          || DependencyRegistrations::<T>::get(registration.source, owner.actor.actor_id)
            != Some(registration.handle)
        {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        Self::dependency_registration_position(
          registration.source,
          owner.actor.actor_id,
          registration.handle,
        )?;
      }
      let old_timed_review = DependencyTimedReviews::<T>::get(owner.actor.actor_id);
      if old_timed_review.is_some_and(|value| value.owner.actor != owner.actor) {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      if let Some(deadline) = timed_review {
        let future = match deadline {
          WakeupKey::Block(block) => block > frame_system::Pallet::<T>::block_number(),
          WakeupKey::Tick(tick) => {
            tick
              > Self::current_scheduler_tick()
                .map_err(|_| DependencyRegistrationError::ClockUnavailable)?
          }
        };
        if !future {
          return Err(DependencyRegistrationError::DeadlineNotFuture);
        }
      }
      for entry in desired {
        let current = DependencyRegistrations::<T>::get(entry.source, owner.actor.actor_id);
        let old_registration = old
          .iter(/* deos-bypass: bounded-iter -- stored plan is MaxParkedBalanceWatches-bounded. */)
          .find(|value| value.source == entry.source);
        if current != old_registration.map(|value| value.handle) {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        if current.is_none() {
          let header = DependencyRegistrationHeaders::<T>::get(entry.source);
          if header.count >= T::MaxActiveActors::get()
            || ((header.free_count == 0
              || DependencyRevisions::<T>::get(entry.source)
                .scan_target
                .is_some())
              && header.next_index == u64::MAX)
          {
            return Err(DependencyRegistrationError::CapacityExceeded);
          }
        }
      }

      let mut mutation = DependencyPlanMutation::default();
      let mut next =
        BoundedVec::<DependencyPlanRegistration, T::MaxParkedBalanceWatches>::default();
      for entry in desired {
        let result = match DependencyRegistrations::<T>::get(entry.source, owner.actor.actor_id) {
          Some(current) => Self::replace_dependency_registration(
            entry.source,
            current,
            owner,
            entry.observed_revision,
          )?,
          None => {
            Self::install_dependency_registration(entry.source, owner, entry.observed_revision)?
          }
        };
        let counter = match result {
          DependencyRegistrationMutation::Installed => &mut mutation.installed,
          DependencyRegistrationMutation::Unchanged => &mut mutation.retained,
          DependencyRegistrationMutation::Replaced => &mut mutation.replaced,
          DependencyRegistrationMutation::Removed => {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
        };
        *counter = counter
          .checked_add(1)
          .ok_or(DependencyRegistrationError::CapacityExceeded)?;
        next
          .try_push(DependencyPlanRegistration {
            source: entry.source,
            handle: DependencyRegistrationHandle {
              actor: owner.actor,
              plan_revision: owner.plan_revision,
              acknowledged_revision: entry.observed_revision,
            },
          })
          .map_err(|_| DependencyRegistrationError::PlanTooLarge)?;
      }
      for registration in &old {
        if !desired
          .iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the complete plan. */)
          .any(|entry| entry.source == registration.source)
        {
          Self::remove_dependency_registration(registration.source, registration.handle)?;
          mutation.removed = mutation
            .removed
            .checked_add(1)
            .ok_or(DependencyRegistrationError::CapacityExceeded)?;
        }
      }
      mutation.timed_review = match (old_timed_review, timed_review) {
        (None, None) => DependencyTimedReviewMutation::None,
        (None, Some(deadline)) => {
          DependencyTimedReviews::<T>::insert(
            owner.actor.actor_id,
            DependencyTimedReview { owner, deadline },
          );
          DependencyTimedReviewMutation::Installed
        }
        (Some(current), Some(deadline))
          if current == (DependencyTimedReview { owner, deadline }) =>
        {
          DependencyTimedReviewMutation::Retained
        }
        (Some(_), Some(deadline)) => {
          DependencyTimedReviews::<T>::insert(
            owner.actor.actor_id,
            DependencyTimedReview { owner, deadline },
          );
          DependencyTimedReviewMutation::Replaced
        }
        (Some(_), None) => {
          DependencyTimedReviews::<T>::remove(owner.actor.actor_id);
          DependencyTimedReviewMutation::Removed
        }
      };
      DependencyPlans::<T>::insert(owner.actor.actor_id, next);
      Ok(mutation)
    }

    /// Consumes one exact Pending event only after its complete successor plan is durable.
    #[allow(
      dead_code,
      reason = "Pending-event consumption remains inert until parking authority cutover"
    )]
    pub(crate) fn consume_pending_dependency_event(
      expected: PendingDependencyEvent,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyRegistrationError::PendingEventMissing),
        Some(current) if current != expected => {
          return Err(DependencyRegistrationError::PendingEventMismatch);
        }
        Some(_) => {}
      }
      if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let source_state = DependencyRevisions::<T>::get(expected.source);
      if source_state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if source_state.revision < expected.revision {
        return Err(DependencyRegistrationError::RevisionMismatch);
      }
      let registration =
        DependencyRegistrations::<T>::get(expected.source, expected.owner.actor.actor_id)
          .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if registration.actor != expected.owner.actor
        || registration.plan_revision != expected.owner.plan_revision
        || registration.acknowledged_revision < expected.revision
      {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      let mutation = Self::commit_negative_dependency_plan(expected.owner, desired, timed_review)?;
      match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {}
        _ => return Err(DependencyRegistrationError::PendingEventMismatch),
      }
      PendingDependencyEvents::<T>::remove(expected.owner.actor.actor_id);
      Ok(mutation)
    }

    /// Consumes one exact Pending review only after its complete successor plan is durable.
    #[allow(
      dead_code,
      reason = "Pending-review consumption remains inert until parking authority cutover"
    )]
    pub(crate) fn consume_pending_dependency_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyRegistrationError::PendingReviewMissing),
        Some(current) if current != expected => {
          return Err(DependencyRegistrationError::PendingReviewMismatch);
        }
        Some(_) => {}
      }
      let mutation = Self::commit_negative_dependency_plan(expected.owner, desired, timed_review)?;
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {}
        _ => return Err(DependencyRegistrationError::PendingReviewMismatch),
      }
      PendingDependencyReviews::<T>::remove(expected.owner.actor.actor_id);
      Ok(mutation)
    }

    /// Publishes one exact due review into durable Pending authority before releasing its deadline.
    #[allow(
      dead_code,
      reason = "due-review publication remains inert until deadline traversal cutover"
    )]
    pub(crate) fn publish_due_dependency_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
    ) -> Result<DependencyDueReviewMutation, DependencyDueReviewError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyDueReviewError::TransactionRequired);
      }
      match PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyDueReviewError::PendingOwnerMissing),
        Some(owner) if owner != expected.owner => {
          return Err(DependencyDueReviewError::PendingOwnerMismatch);
        }
        Some(_) => {}
      }
      let due = match expected.deadline {
        WakeupKey::Block(block) => block <= frame_system::Pallet::<T>::block_number(),
        WakeupKey::Tick(tick) => {
          tick
            <= Self::current_scheduler_tick()
              .map_err(|_| DependencyDueReviewError::ClockUnavailable)?
        }
      };
      if !due {
        return Err(DependencyDueReviewError::NotDue);
      }
      if PendingDependencyEvents::<T>::contains_key(expected.owner.actor.actor_id) {
        return Err(DependencyDueReviewError::DestinationOccupied);
      }
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {
          if DependencyTimedReviews::<T>::contains_key(expected.owner.actor.actor_id) {
            return Err(DependencyDueReviewError::DestinationOccupied);
          }
          return Ok(DependencyDueReviewMutation::AlreadyPending);
        }
        Some(_) => return Err(DependencyDueReviewError::DestinationOccupied),
        None => {}
      }
      match DependencyTimedReviews::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyDueReviewError::ReviewMissing),
        Some(current) if current != expected => {
          return Err(DependencyDueReviewError::ReviewMismatch);
        }
        Some(_) => {}
      }
      PendingDependencyReviews::<T>::insert(expected.owner.actor.actor_id, expected);
      DependencyTimedReviews::<T>::remove(expected.owner.actor.actor_id);
      Ok(DependencyDueReviewMutation::Published)
    }

    /// Installs or validates one exact dependency registration without releasing old authority.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn install_dependency_registration(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let handle = Self::validate_dependency_registration(source, owner, acknowledged_revision)?;
      match DependencyRegistrations::<T>::get(source, owner.actor.actor_id) {
        Some(current) if current == handle => {
          Self::dependency_registration_position(source, owner.actor.actor_id, current)?;
          Ok(DependencyRegistrationMutation::Unchanged)
        }
        Some(_) => Err(DependencyRegistrationError::RegistrationAlreadyExists),
        None => {
          let mut header = DependencyRegistrationHeaders::<T>::get(source);
          let next_count = header
            .count
            .checked_add(1)
            .ok_or(DependencyRegistrationError::CapacityExceeded)?;
          if next_count > T::MaxActiveActors::get() {
            return Err(DependencyRegistrationError::CapacityExceeded);
          }
          let scan_active = DependencyRevisions::<T>::get(source).scan_target.is_some();
          let (position, reused_free_index) = if !scan_active && header.free_count > 0 {
            let free_index = header.free_count - 1;
            let position = DependencyRegistrationFreePositions::<T>::get(source, free_index)
              .ok_or(DependencyRegistrationError::CorruptTopology)?;
            (position, Some(free_index))
          } else {
            let index = header.next_index;
            header.next_index = index
              .checked_add(1)
              .ok_or(DependencyRegistrationError::CapacityExceeded)?;
            (
              DependencyRegistrationPosition {
                page: index / 32,
                slot: (index % 32) as u8,
              },
              None,
            )
          };
          let mut page = match DependencyRegistrationPages::<T>::get(source, position.page) {
            Some(page) => page,
            None if reused_free_index.is_none() => {
              if let Some(last) = header.last_page {
                if last >= position.page {
                  return Err(DependencyRegistrationError::CorruptTopology);
                }
                DependencyRegistrationPages::<T>::try_mutate(source, last, |page| {
                  let page = page
                    .as_mut()
                    .ok_or(DependencyRegistrationError::CorruptTopology)?;
                  if page.next.is_some() {
                    return Err(DependencyRegistrationError::CorruptTopology);
                  }
                  page.next = Some(position.page);
                  Ok(())
                })?;
              } else {
                if header.first_page.is_some() || header.count != 0 {
                  return Err(DependencyRegistrationError::CorruptTopology);
                }
                header.first_page = Some(position.page);
              }
              let page = DependencyRegistrationPage {
                entries: BoundedVec::try_from(alloc::vec![None; 32])
                  .map_err(|_| DependencyRegistrationError::CorruptTopology)?,
                previous: header.last_page,
                next: None,
              };
              header.last_page = Some(position.page);
              page
            }
            None => return Err(DependencyRegistrationError::CorruptTopology),
          };
          if page.entries.len() != 32 || position.slot >= 32 {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
          if page
            .entries
            .get(position.slot as usize)
            .and_then(Option::as_ref)
            .is_some()
          {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
          page.entries[position.slot as usize] = Some(handle);
          header.count = next_count;
          if let Some(free_index) = reused_free_index {
            Self::remove_dependency_free_position(source, position, &mut header)?;
            if header.free_count != free_index {
              return Err(DependencyRegistrationError::CorruptTopology);
            }
          }
          DependencyRegistrationPages::<T>::insert(source, position.page, page);
          DependencyRegistrationHeaders::<T>::insert(source, header);
          DependencyRegistrationPositions::<T>::insert(source, owner.actor.actor_id, position);
          DependencyRegistrations::<T>::insert(source, owner.actor.actor_id, handle);
          Ok(DependencyRegistrationMutation::Installed)
        }
      }
    }

    /// Replaces one exact registration only after its successor is fully validated.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn replace_dependency_registration(
      source: DependencySourceId,
      current: DependencyRegistrationHandle,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let replacement =
        Self::validate_dependency_registration(source, owner, acknowledged_revision)?;
      let stored = DependencyRegistrations::<T>::get(source, current.actor.actor_id)
        .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if stored != current {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      if current.actor.actor_id != replacement.actor.actor_id {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let position =
        Self::dependency_registration_position(source, current.actor.actor_id, current)?;
      if current == replacement {
        return Ok(DependencyRegistrationMutation::Unchanged);
      }
      let mut page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      page.entries[position.slot as usize] = Some(replacement);
      DependencyRegistrationPages::<T>::insert(source, position.page, page);
      DependencyRegistrations::<T>::insert(source, replacement.actor.actor_id, replacement);
      Ok(DependencyRegistrationMutation::Replaced)
    }

    fn remove_dependency_free_position(
      source: DependencySourceId,
      position: DependencyRegistrationPosition,
      header: &mut DependencyRegistrationHeader,
    ) -> Result<(), DependencyRegistrationError> {
      let index = DependencyRegistrationFreeIndices::<T>::get(source, position)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      let last_index = header
        .free_count
        .checked_sub(1)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if index > last_index
        || DependencyRegistrationFreePositions::<T>::get(source, index) != Some(position)
      {
        return Err(DependencyRegistrationError::CorruptTopology);
      }
      if index != last_index {
        let last = DependencyRegistrationFreePositions::<T>::get(source, last_index)
          .ok_or(DependencyRegistrationError::CorruptTopology)?;
        if DependencyRegistrationFreeIndices::<T>::get(source, last) != Some(last_index) {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        DependencyRegistrationFreePositions::<T>::insert(source, index, last);
        DependencyRegistrationFreeIndices::<T>::insert(source, last, index);
      }
      DependencyRegistrationFreePositions::<T>::remove(source, last_index);
      DependencyRegistrationFreeIndices::<T>::remove(source, position);
      header.free_count = last_index;
      Ok(())
    }

    /// Deletes an empty page immediately; the final registration never walks source history.
    fn retire_dependency_registration_position(
      source: DependencySourceId,
      position: DependencyRegistrationPosition,
      page: &mut DependencyRegistrationPage,
    ) -> Result<(), DependencyRegistrationError> {
      if page.entries.len() != 32 || position.slot >= 32 {
        return Err(DependencyRegistrationError::CorruptTopology);
      }
      let mut header = DependencyRegistrationHeaders::<T>::get(source);
      header.count = header
        .count
        .checked_sub(1)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if header.free_count >= u64::from(T::MaxActiveActors::get()) * 32
        || DependencyRegistrationFreeIndices::<T>::contains_key(source, position)
        || DependencyRegistrationFreePositions::<T>::contains_key(source, header.free_count)
      {
        return Err(DependencyRegistrationError::CorruptTopology);
      }
      page.entries[position.slot as usize] = None;
      DependencyRegistrationFreePositions::<T>::insert(source, header.free_count, position);
      DependencyRegistrationFreeIndices::<T>::insert(source, position, header.free_count);
      header.free_count = header
        .free_count
        .checked_add(1)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if page.entries.iter(/* deos-bypass: bounded-iter -- one fixed C32 registration page. */).any(Option::is_some)
      {
        DependencyRegistrationPages::<T>::insert(source, position.page, page);
        DependencyRegistrationHeaders::<T>::insert(source, header);
        return Ok(());
      }
      for slot in 0..32u8 {
        let hole = DependencyRegistrationPosition {
          page: position.page,
          slot,
        };
        if DependencyRegistrationFreeIndices::<T>::contains_key(source, hole) {
          Self::remove_dependency_free_position(source, hole, &mut header)?;
        }
      }
      if let Some(previous) = page.previous {
        if previous >= position.page {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        DependencyRegistrationPages::<T>::try_mutate(source, previous, |previous| {
          let previous = previous
            .as_mut()
            .ok_or(DependencyRegistrationError::CorruptTopology)?;
          if previous.next != Some(position.page) {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
          previous.next = page.next;
          Ok(())
        })?;
      } else {
        if header.first_page != Some(position.page) {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        header.first_page = page.next;
      }
      if let Some(next) = page.next {
        if next <= position.page {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        DependencyRegistrationPages::<T>::try_mutate(source, next, |next| {
          let next = next
            .as_mut()
            .ok_or(DependencyRegistrationError::CorruptTopology)?;
          if next.previous != Some(position.page) {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
          next.previous = page.previous;
          Ok(())
        })?;
      } else {
        if header.last_page != Some(position.page) {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        header.last_page = page.previous;
      }
      DependencyRegistrationPages::<T>::remove(source, position.page);
      let mut state = DependencyRevisions::<T>::get(source);
      if header.count != 0 {
        if state.scan_target.is_some() && state.scan_cursor / 32 == position.page {
          state.scan_cursor = match page.next {
            Some(next) => next
              .checked_mul(32)
              .ok_or(DependencyRegistrationError::CorruptTopology)?
              .min(state.scan_end),
            None => state.scan_end,
          };
          DependencyRevisions::<T>::insert(source, state);
        }
        DependencyRegistrationHeaders::<T>::insert(source, header);
        return Ok(());
      }
      if header.first_page.is_some() || header.last_page.is_some() || header.free_count != 0 {
        return Err(DependencyRegistrationError::CorruptTopology);
      }
      if let Some(asset) = DependencySourceBalances::<T>::get(source) {
        if BalanceDependencySources::<T>::get(asset) != Some(source) {
          return Err(DependencyRegistrationError::CorruptTopology);
        }
        BalanceDependencySources::<T>::remove(asset);
        DependencySourceBalances::<T>::remove(source);
      }
      if DependencyScanSourceNodes::<T>::contains_key(source) {
        state.scan_target = None;
        DependencyRevisions::<T>::insert(source, state);
        Self::remove_dependency_scan_source(source)
          .map_err(|_| DependencyRegistrationError::CorruptTopology)?;
      }
      DependencyRegistrationHeaders::<T>::remove(source);
      DependencyRevisions::<T>::remove(source);
      Ok(())
    }

    /// Removes only the exact registration named by its generation/plan/revision handle.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn remove_dependency_registration(
      source: DependencySourceId,
      expected: DependencyRegistrationHandle,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let stored = DependencyRegistrations::<T>::get(source, expected.actor.actor_id)
        .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if stored != expected {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      let position =
        Self::dependency_registration_position(source, expected.actor.actor_id, expected)?;
      let mut page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      Self::retire_dependency_registration_position(source, position, &mut page)?;
      DependencyRegistrationPositions::<T>::remove(source, expected.actor.actor_id);
      DependencyRegistrations::<T>::remove(source, expected.actor.actor_id);
      Ok(DependencyRegistrationMutation::Removed)
    }

    /// Atomically unlinks one service member and deletes its canonical process without a tombstone.
    /// Canonical zero-Step and effectful terminal outcomes use this owner after finalization.
    pub(crate) fn retire_service_member(actor: ActorRef) -> Result<(), ServiceRetirementError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = Self::remove_service_member(actor)
          .map_err(ServiceRetirementError::Ring)
          .map(|_| ActorProcesses::<T>::remove(actor.actor_id));
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Removes exactly one generation-bound member from the inert service ring.
    #[allow(
      dead_code,
      reason = "service-ring mutation remains unreachable until the complete carrier cutover"
    )]
    pub(crate) fn remove_service_member(
      actor: ActorRef,
    ) -> Result<ServiceNode<BlockNumberFor<T>>, ServiceRingMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRingMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ServiceRingMutationError::LegacyAuthorityPresent);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::ProcessMissing)?;
      let node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::MemberMissing)?;
      if node.generation != actor.generation || process.generation != actor.generation {
        return Err(ServiceRingMutationError::StaleGeneration);
      }
      if !Self::process_has_service_authority(actor, &process, node.kind) {
        return Err(ServiceRingMutationError::ProcessResidenceMismatch);
      }

      let mut header = ServiceHeader::<T>::get();
      if header.count == 0 || header.cursor.is_none() {
        return Err(ServiceRingMutationError::CorruptRing);
      }
      if header.count == 1 {
        if header.cursor != Some(actor) || node.previous != actor || node.next != actor {
          return Err(ServiceRingMutationError::CorruptRing);
        }
        header = ServiceHeaderRecord::default();
      } else {
        let mut previous = ServiceNodes::<T>::get(node.previous.actor_id)
          .filter(|value| value.generation == node.previous.generation)
          .ok_or(ServiceRingMutationError::CorruptRing)?;
        let mut next = ServiceNodes::<T>::get(node.next.actor_id)
          .filter(|value| value.generation == node.next.generation)
          .ok_or(ServiceRingMutationError::CorruptRing)?;
        if previous.next != actor || next.previous != actor {
          return Err(ServiceRingMutationError::CorruptRing);
        }
        previous.next = node.next;
        next.previous = node.previous;
        if node.previous.actor_id == node.next.actor_id {
          previous.previous = node.previous;
          ServiceNodes::<T>::insert(node.previous.actor_id, previous);
        } else {
          ServiceNodes::<T>::insert(node.previous.actor_id, previous);
          ServiceNodes::<T>::insert(node.next.actor_id, next);
        }
        if header.cursor == Some(actor) {
          header.cursor = Some(node.next);
        }
        header.count -= 1;
      }
      ServiceNodes::<T>::remove(actor.actor_id);
      ServiceHeader::<T>::put(header);
      Ok(node)
    }

    fn deadline_handle_matches_process(
      handle: DeadlineHandleOf<T>,
      process: &ActorProcessOf<T>,
    ) -> bool {
      process.generation == handle.actor.generation
        && (process.status == ProcessStatus::Serving
          || (process.status == ProcessStatus::AwaitingFunding
            && matches!(
              process.residence,
              Some(ProcessResidence::Deadline {
                key: WakeupKey::Tick(_),
                ..
              })
            )))
        && (process.residence
          == Some(ProcessResidence::Deadline {
            key: handle.key,
            page: handle.page,
            slot: handle.slot,
          })
          || matches!(
            process.residence,
            Some(ProcessResidence::Parked(evidence))
              if PendingCheckOwners::<T>::get(handle.actor.actor_id)
                .is_some_and(|owner| owner.actor == handle.actor)
                && (DependencyTimedReviews::<T>::get(handle.actor.actor_id)
                  .or_else(|| PendingDependencyReviews::<T>::get(handle.actor.actor_id))
                  .is_some_and(|review| {
                    review.owner.actor == handle.actor && review.deadline == handle.key
                  })
                  || matches!(handle.key,
                    WakeupKey::Block(block) if evidence.review_at == Some(block)))
          ))
    }

    fn validate_deadline_header(header: &DeadlineHeader) -> Result<(), DeadlineMutationError> {
      let slots = header.page_count.saturating_mul(32);
      if header.page_count == 0
        || header.page_count > Self::deadline_destination_page_bound()
        || header.count > T::MaxActiveActors::get()
        || header.count < header.page_count
        || header.count > slots
        || header.first_vacant_page.is_none() != (header.count == slots)
        || header.first_page > header.last_page
        || header.last_page >= header.next_page
      {
        return Err(DeadlineMutationError::CorruptCarrier);
      }
      Ok(())
    }

    /// Inserts one generation-bound process obligation into an exact retained deadline slot.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_deadline_member(
      handle: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      Self::insert_deadline_member_for_owner(handle, false)
    }

    /// Inserts one independent temporal Trigger obligation into the shared deadline carrier.
    #[allow(
      dead_code,
      reason = "temporal Trigger carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_trigger_deadline_member(
      handle: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      Self::insert_deadline_member_for_owner(handle, true)
    }

    fn insert_deadline_member_for_owner(
      handle: DeadlineHandleOf<T>,
      trigger_owner: bool,
    ) -> Result<(), DeadlineMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(handle.actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(handle.actor.actor_id)
      {
        return Err(DeadlineMutationError::LegacyAuthorityPresent);
      }
      if trigger_owner {
        let Some(ActorSemanticState::Active(semantic)) =
          ActorSemanticStates::<T>::get(handle.actor.actor_id)
        else {
          return Err(DeadlineMutationError::ProcessMissing);
        };
        let expected_pointer = match handle.key {
          WakeupKey::Tick(tick) => Some(TriggerWakeupPointer {
            tick,
            page_id: handle.page,
            slot: u32::from(handle.slot),
          }),
          WakeupKey::Block(_) => return Err(DeadlineMutationError::InvalidDestination),
        };
        if semantic.generation != handle.actor.generation
          || semantic.hot.trigger_wakeup_pointer != expected_pointer
        {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      } else {
        let process = ActorProcesses::<T>::get(handle.actor.actor_id)
          .ok_or(DeadlineMutationError::ProcessMissing)?;
        if !Self::deadline_handle_matches_process(handle, &process) {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      }
      let reverse_exists = if trigger_owner {
        TriggerDeadlineHandles::<T>::contains_key(handle.actor.actor_id)
      } else {
        DeadlineHandles::<T>::contains_key(handle.actor.actor_id)
      };
      if reverse_exists {
        return Err(DeadlineMutationError::MemberAlreadyExists);
      }
      let slot = usize::from(handle.slot);
      if slot >= 32 {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let mut header = DeadlineHeaders::<T>::get(handle.key);
      if let Some(header) = &header {
        Self::validate_deadline_header(header)?;
      }
      let creates_bucket = header.is_none();
      let mut page = DeadlinePages::<T>::get(handle.key, handle.page);
      match (&header, &page) {
        (None, None) if handle.page == 0 => {}
        (Some(header), None) if handle.page == header.next_page => {}
        (Some(_), Some(_)) => {}
        _ => return Err(DeadlineMutationError::InvalidDestination),
      }
      if let Some(page) = &page {
        if page.entries.len() != 32 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        if page.entries.get(slot) != Some(&None) {
          return Err(DeadlineMutationError::InvalidDestination);
        }
      }
      let next_count = header.as_ref().map_or(Ok(1), |value| {
        value
          .count
          .checked_add(1)
          .ok_or(DeadlineMutationError::CapacityExceeded)
      })?;
      if next_count > T::MaxActiveActors::get() {
        return Err(DeadlineMutationError::CapacityExceeded);
      }
      let creates_page = page.is_none();
      if creates_page {
        if header
          .as_ref()
          .is_some_and(|header| header.page_count >= Self::deadline_destination_page_bound())
        {
          return Err(DeadlineMutationError::CapacityExceeded);
        }
        let previous_page = header.as_ref().map(|value| value.last_page);
        let next_vacant_page = header.as_ref().and_then(|value| value.first_vacant_page);
        page = Some(DeadlinePage {
          previous_page,
          next_page: None,
          previous_vacant_page: None,
          next_vacant_page,
          live_entries: 0,
          entries: BoundedVec::try_from(alloc::vec![None; 32])
            .map_err(|_| DeadlineMutationError::CorruptCarrier)?,
        });
        if let Some(previous_page) = previous_page {
          let mut previous = DeadlinePages::<T>::get(handle.key, previous_page)
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
          if previous.next_page.is_some() {
            return Err(DeadlineMutationError::CorruptCarrier);
          }
          previous.next_page = Some(handle.page);
          DeadlinePages::<T>::insert(handle.key, previous_page, previous);
        }
        if let Some(next_vacant_page) = next_vacant_page {
          let mut next = DeadlinePages::<T>::get(handle.key, next_vacant_page)
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
          if next.previous_vacant_page.is_some() {
            return Err(DeadlineMutationError::CorruptCarrier);
          }
          next.previous_vacant_page = Some(handle.page);
          DeadlinePages::<T>::insert(handle.key, next_vacant_page, next);
        }
      }
      let mut page = page.ok_or(DeadlineMutationError::CorruptCarrier)?;
      if page.live_entries >= 32 {
        return Err(DeadlineMutationError::PageFull);
      }
      page.entries[slot] = Some(handle.actor);
      page.live_entries = page
        .live_entries
        .checked_add(1)
        .ok_or(DeadlineMutationError::CapacityExceeded)?;
      match header.as_mut() {
        Some(header) => {
          if creates_page {
            header.first_vacant_page = Some(handle.page);
            header.last_page = handle.page;
            header.next_page = header
              .next_page
              .checked_add(1)
              .ok_or(DeadlineMutationError::NamespaceExhausted)?;
            header.page_count = header
              .page_count
              .checked_add(1)
              .ok_or(DeadlineMutationError::CapacityExceeded)?;
          } else if page.live_entries == 32 {
            let previous = page.previous_vacant_page;
            let next = page.next_vacant_page;
            if let Some(previous_id) = previous {
              let mut previous_page = DeadlinePages::<T>::get(handle.key, previous_id)
                .ok_or(DeadlineMutationError::CorruptCarrier)?;
              if previous_page.next_vacant_page != Some(handle.page) {
                return Err(DeadlineMutationError::CorruptCarrier);
              }
              previous_page.next_vacant_page = next;
              DeadlinePages::<T>::insert(handle.key, previous_id, previous_page);
            } else {
              if header.first_vacant_page != Some(handle.page) {
                return Err(DeadlineMutationError::CorruptCarrier);
              }
              header.first_vacant_page = next;
            }
            if let Some(next_id) = next {
              let mut next_page = DeadlinePages::<T>::get(handle.key, next_id)
                .ok_or(DeadlineMutationError::CorruptCarrier)?;
              if next_page.previous_vacant_page != Some(handle.page) {
                return Err(DeadlineMutationError::CorruptCarrier);
              }
              next_page.previous_vacant_page = previous;
              DeadlinePages::<T>::insert(handle.key, next_id, next_page);
            }
            page.previous_vacant_page = None;
            page.next_vacant_page = None;
          }
          header.count = next_count;
        }
        None => {
          header = Some(DeadlineHeader {
            first_page: 0,
            last_page: 0,
            next_page: 1,
            first_vacant_page: Some(0),
            page_count: 1,
            count: 1,
          });
        }
      }
      DeadlinePages::<T>::insert(handle.key, handle.page, page);
      let header = header.ok_or(DeadlineMutationError::CorruptCarrier)?;
      DeadlineHeaders::<T>::insert(handle.key, header);
      if trigger_owner {
        TriggerDeadlineHandles::<T>::insert(handle.actor.actor_id, handle);
      } else {
        DeadlineHandles::<T>::insert(handle.actor.actor_id, handle);
      }
      if creates_bucket {
        Self::insert_deadline_index(handle.key)?;
      } else {
        Self::update_deadline_index(handle.key)?;
      }
      Ok(())
    }

    /// Removes exactly one generation-bound process deadline and unlinks an empty retained page.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_deadline_member(
      actor: ActorRef,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      Self::remove_deadline_member_for_owner(actor, false)
    }

    /// Removes the independent temporal Trigger deadline without changing process residence.
    #[allow(
      dead_code,
      reason = "temporal Trigger carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_trigger_deadline_member(
      actor: ActorRef,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      Self::remove_deadline_member_for_owner(actor, true)
    }

    fn remove_deadline_member_for_owner(
      actor: ActorRef,
      trigger_owner: bool,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(DeadlineMutationError::LegacyAuthorityPresent);
      }
      let handle = if trigger_owner {
        TriggerDeadlineHandles::<T>::get(actor.actor_id)
      } else {
        DeadlineHandles::<T>::get(actor.actor_id)
      }
      .ok_or(DeadlineMutationError::MemberMissing)?;
      if handle.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      if trigger_owner {
        let Some(ActorSemanticState::Active(semantic)) =
          ActorSemanticStates::<T>::get(actor.actor_id)
        else {
          return Err(DeadlineMutationError::ProcessMissing);
        };
        let expected_pointer = match handle.key {
          WakeupKey::Tick(tick) => Some(TriggerWakeupPointer {
            tick,
            page_id: handle.page,
            slot: u32::from(handle.slot),
          }),
          WakeupKey::Block(_) => return Err(DeadlineMutationError::InvalidDestination),
        };
        if semantic.generation != actor.generation
          || semantic.hot.trigger_wakeup_pointer != expected_pointer
        {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      } else {
        let process =
          ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
        if process.generation != actor.generation {
          return Err(DeadlineMutationError::StaleGeneration);
        }
        if !Self::deadline_handle_matches_process(handle, &process) {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      }
      let mut header =
        DeadlineHeaders::<T>::get(handle.key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      Self::validate_deadline_header(&header)?;
      let mut page = DeadlinePages::<T>::get(handle.key, handle.page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let slot = usize::from(handle.slot);
      if page.entries.len() != 32
        || page.entries.get(slot) != Some(&Some(actor))
        || page.live_entries == 0
        || page.live_entries > 32
      {
        return Err(DeadlineMutationError::CorruptCarrier);
      }
      let was_full = page.live_entries == 32;
      page.entries[slot] = None;
      page.live_entries = page
        .live_entries
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      header.count = header
        .count
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if trigger_owner {
        TriggerDeadlineHandles::<T>::remove(actor.actor_id);
      } else {
        DeadlineHandles::<T>::remove(actor.actor_id);
      }
      if page.live_entries > 0 {
        if was_full {
          let old_head = header.first_vacant_page;
          page.previous_vacant_page = None;
          page.next_vacant_page = old_head;
          if let Some(old_head) = old_head {
            let mut old_head_page = DeadlinePages::<T>::get(handle.key, old_head)
              .ok_or(DeadlineMutationError::CorruptCarrier)?;
            if old_head_page.previous_vacant_page.is_some() {
              return Err(DeadlineMutationError::CorruptCarrier);
            }
            old_head_page.previous_vacant_page = Some(handle.page);
            DeadlinePages::<T>::insert(handle.key, old_head, old_head_page);
          }
          header.first_vacant_page = Some(handle.page);
        } else if page.previous_vacant_page.is_none()
          && header.first_vacant_page != Some(handle.page)
        {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        DeadlinePages::<T>::insert(handle.key, handle.page, page);
        DeadlineHeaders::<T>::insert(handle.key, header);
        Self::update_deadline_index(handle.key)?;
        return Ok(handle);
      }
      let previous_vacant = page.previous_vacant_page;
      let next_vacant = page.next_vacant_page;
      if let Some(previous_id) = previous_vacant {
        let mut previous = DeadlinePages::<T>::get(handle.key, previous_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if previous.next_vacant_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        previous.next_vacant_page = next_vacant;
        DeadlinePages::<T>::insert(handle.key, previous_id, previous);
      } else {
        if header.first_vacant_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        header.first_vacant_page = next_vacant;
      }
      if let Some(next_id) = next_vacant {
        let mut next = DeadlinePages::<T>::get(handle.key, next_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if next.previous_vacant_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        next.previous_vacant_page = previous_vacant;
        DeadlinePages::<T>::insert(handle.key, next_id, next);
      }
      if let Some(previous_id) = page.previous_page {
        let mut previous = DeadlinePages::<T>::get(handle.key, previous_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if previous.next_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        previous.next_page = page.next_page;
        DeadlinePages::<T>::insert(handle.key, previous_id, previous);
      }
      if let Some(next_id) = page.next_page {
        let mut next = DeadlinePages::<T>::get(handle.key, next_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if next.previous_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        next.previous_page = page.previous_page;
        DeadlinePages::<T>::insert(handle.key, next_id, next);
      }
      DeadlinePages::<T>::remove(handle.key, handle.page);
      header.page_count = header
        .page_count
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if header.page_count == 0 {
        if header.count != 0 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        DeadlineHeaders::<T>::remove(handle.key);
        Self::remove_deadline_index(handle.key)?;
      } else {
        if header.first_page == handle.page {
          header.first_page = page
            .next_page
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
        }
        if header.last_page == handle.page {
          header.last_page = page
            .previous_page
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
        }
        DeadlineHeaders::<T>::insert(handle.key, header);
        Self::update_deadline_index(handle.key)?;
      }
      Ok(handle)
    }

    /// Moves one deadline member atomically after preflighting the exact destination slot.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn move_deadline_member(
      actor: ActorRef,
      destination: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      if destination.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      let slot = usize::from(destination.slot);
      if slot >= 32 {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      if let Some(page) = DeadlinePages::<T>::get(destination.key, destination.page) {
        if page.entries.len() != 32 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        if page.entries.get(slot) != Some(&None) {
          return Err(DeadlineMutationError::InvalidDestination);
        }
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          Self::remove_deadline_member(actor)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          process.residence = Some(ProcessResidence::Deadline {
            key: destination.key,
            page: destination.page,
            slot: destination.slot,
          });
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(destination)
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn deadline_index_page_and_slot(index: u32) -> (u64, usize) {
      (u64::from(index / 32), (index % 32) as usize)
    }

    fn deadline_index_get(clock: WakeupClock, index: u32) -> Option<WakeupKey<BlockNumberFor<T>>> {
      let (page, slot) = Self::deadline_index_page_and_slot(index);
      DeadlineIndexPages::<T>::get(clock, page).and_then(|page| page.get(slot).copied())
    }

    fn deadline_index_set(
      clock: WakeupClock,
      index: u32,
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if key.clock() != clock {
        return Err(DeadlineIndexMutationError::CorruptHeap);
      }
      let (page_id, slot) = Self::deadline_index_page_and_slot(index);
      let mut page = DeadlineIndexPages::<T>::get(clock, page_id).unwrap_or_default();
      if slot < page.len() {
        page[slot] = key;
      } else if slot == page.len() {
        page
          .try_push(key)
          .map_err(|_| DeadlineIndexMutationError::CapacityExceeded)?;
      } else {
        return Err(DeadlineIndexMutationError::CorruptHeap);
      }
      DeadlineIndexPages::<T>::insert(clock, page_id, page);
      Ok(())
    }

    fn deadline_index_swap(
      clock: WakeupClock,
      left: u32,
      right: u32,
    ) -> Result<(), DeadlineIndexMutationError> {
      let left_key =
        Self::deadline_index_get(clock, left).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
      let right_key =
        Self::deadline_index_get(clock, right).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
      if DeadlineIndexPositions::<T>::get(left_key) != Some(left)
        || DeadlineIndexPositions::<T>::get(right_key) != Some(right)
      {
        return Err(DeadlineIndexMutationError::StaleIndex);
      }
      Self::deadline_index_set(clock, left, right_key)?;
      Self::deadline_index_set(clock, right, left_key)?;
      DeadlineIndexPositions::<T>::insert(right_key, left);
      DeadlineIndexPositions::<T>::insert(left_key, right);
      Ok(())
    }

    fn deadline_index_height_bound() -> u32 {
      u32::BITS.saturating_sub(T::MaxActiveActors::get().max(1).leading_zeros())
    }

    fn validate_deadline_index_header(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if ActorWaitingOccupancies::<T>::get(key) > 0
        || ActorWaitingCursorIndices::<T>::contains_key(key)
      {
        return Err(DeadlineIndexMutationError::LegacyAuthorityPresent);
      }
      let header =
        DeadlineHeaders::<T>::get(key).ok_or(DeadlineIndexMutationError::HeaderMissing)?;
      if header.count == 0
        || header.page_count == 0
        || header.count > header.page_count.saturating_mul(32)
        || header.first_page > header.last_page
        || !DeadlinePages::<T>::contains_key(key, header.first_page)
        || !DeadlinePages::<T>::contains_key(key, header.last_page)
      {
        return Err(DeadlineIndexMutationError::CorruptHeader);
      }
      Ok(())
    }

    /// Inserts one nonempty canonical deadline bucket into its clock-local inert min-heap.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          Self::validate_deadline_index_header(key)?;
          if DeadlineIndexPositions::<T>::contains_key(key) {
            return Err(DeadlineIndexMutationError::KeyAlreadyExists);
          }
          let clock = key.clock();
          let len = DeadlineIndexLen::<T>::get(clock);
          let next_len = len
            .checked_add(1)
            .ok_or(DeadlineIndexMutationError::CapacityExceeded)?;
          if len >= T::MaxActiveActors::get() {
            return Err(DeadlineIndexMutationError::CapacityExceeded);
          }
          Self::deadline_index_set(clock, len, key)?;
          DeadlineIndexPositions::<T>::insert(key, len);
          DeadlineIndexLen::<T>::insert(clock, next_len);
          let mut current = len;
          for _ in 0..Self::deadline_index_height_bound() {
            if current == 0 {
              break;
            }
            let parent = (current - 1) / 2;
            let parent_key = Self::deadline_index_get(clock, parent)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let current_key = Self::deadline_index_get(clock, current)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            if parent_key <= current_key {
              break;
            }
            Self::deadline_index_swap(clock, parent, current)?;
            current = parent;
          }
          Ok(())
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Revalidates and repairs one indexed nonempty bucket within the bounded heap height.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn update_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      Self::validate_deadline_index_header(key)?;
      let clock = key.clock();
      let index =
        DeadlineIndexPositions::<T>::get(key).ok_or(DeadlineIndexMutationError::KeyMissing)?;
      let len = DeadlineIndexLen::<T>::get(clock);
      if index >= len || Self::deadline_index_get(clock, index) != Some(key) {
        return Err(DeadlineIndexMutationError::StaleIndex);
      }
      if index > 0 {
        let parent = (index - 1) / 2;
        if Self::deadline_index_get(clock, parent).ok_or(DeadlineIndexMutationError::CorruptHeap)?
          > key
        {
          return Err(DeadlineIndexMutationError::CorruptHeap);
        }
      }
      let left = index.saturating_mul(2).saturating_add(1);
      for child in [left, left.saturating_add(1)] {
        if child < len
          && Self::deadline_index_get(clock, child)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?
            < key
        {
          return Err(DeadlineIndexMutationError::CorruptHeap);
        }
      }
      Ok(())
    }

    /// Removes one exact bucket only after its canonical deadline header has been deleted.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      if ActorWaitingOccupancies::<T>::get(key) > 0
        || ActorWaitingCursorIndices::<T>::contains_key(key)
      {
        return Err(DeadlineIndexMutationError::LegacyAuthorityPresent);
      }
      if DeadlineHeaders::<T>::contains_key(key) {
        return Err(DeadlineIndexMutationError::CorruptHeader);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          let clock = key.clock();
          let index =
            DeadlineIndexPositions::<T>::get(key).ok_or(DeadlineIndexMutationError::KeyMissing)?;
          let len = DeadlineIndexLen::<T>::get(clock);
          if index >= len || Self::deadline_index_get(clock, index) != Some(key) {
            return Err(DeadlineIndexMutationError::StaleIndex);
          }
          let last = len
            .checked_sub(1)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          let last_key =
            Self::deadline_index_get(clock, last).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          if DeadlineIndexPositions::<T>::get(last_key) != Some(last) {
            return Err(DeadlineIndexMutationError::StaleIndex);
          }
          let (page_id, slot) = Self::deadline_index_page_and_slot(last);
          let mut page = DeadlineIndexPages::<T>::get(clock, page_id)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          if slot + 1 != page.len() {
            return Err(DeadlineIndexMutationError::CorruptHeap);
          }
          page.pop();
          if page.is_empty() {
            DeadlineIndexPages::<T>::remove(clock, page_id);
          } else {
            DeadlineIndexPages::<T>::insert(clock, page_id, page);
          }
          DeadlineIndexPositions::<T>::remove(key);
          DeadlineIndexLen::<T>::insert(clock, last);
          if index == last {
            return Ok(());
          }
          Self::deadline_index_set(clock, index, last_key)?;
          DeadlineIndexPositions::<T>::insert(last_key, index);
          let mut current = index;
          for _ in 0..Self::deadline_index_height_bound() {
            if current > 0 {
              let parent = (current - 1) / 2;
              let parent_key = Self::deadline_index_get(clock, parent)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
              let current_key = Self::deadline_index_get(clock, current)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
              if parent_key > current_key {
                Self::deadline_index_swap(clock, parent, current)?;
                current = parent;
                continue;
              }
            }
            let left = current.saturating_mul(2).saturating_add(1);
            if left >= last {
              break;
            }
            let right = left.saturating_add(1);
            let left_key = Self::deadline_index_get(clock, left)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let mut smallest = left;
            if right < last
              && Self::deadline_index_get(clock, right)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?
                < left_key
            {
              smallest = right;
            }
            let current_key = Self::deadline_index_get(clock, current)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let smallest_key = Self::deadline_index_get(clock, smallest)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            if current_key <= smallest_key {
              break;
            }
            Self::deadline_index_swap(clock, current, smallest)?;
            current = smallest;
          }
          Ok(())
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn load_frame_control_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActorControlLocation<BlockNumberFor<T>>,
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      let (location, cell) = Self::load_primary_control_cell(actor_id).ok()?;
      let (identity, hot, admission) = Self::project_control_cell(&cell, location)?;
      Some((location, identity, hot, admission))
    }

    pub(crate) fn load_control_authority_with_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      let (ActorSemanticState::Active(record), _) =
        Self::load_actor_semantic_state(actor_id).ok()??
      else {
        return None;
      };
      Some((record.identity, record.hot, record.admission))
    }

    /// Intrinsic-control fixture: authenticate stored geometry without supplying prices.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_policy_step_source(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      LoadedActorStepOf<T>,
    )> {
      let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
        return None;
      };
      if !matches!(
        record.hot.cycle_state,
        CycleState::Running | CycleState::Suspended
      ) {
        return None;
      }
      let head = ActorContractHeads::<T>::get(actor_id)?;
      let geometry =
        Self::load_actor_service_geometry(actor_id, &record.hot, &record.admission, &head)?;
      if geometry
        .1
        .as_ref()
        .is_none_or(|loaded| loaded.cursor == 0 && record.hot.cycle_state != CycleState::Suspended)
      {
        return None;
      }
      let policy = Self::load_contract_behavior_policy(actor_id, &head.header)??;
      let (state, admission, loaded) = Self::materialize_actor_service_state(
        record.identity,
        record.hot,
        record.admission,
        head,
        geometry,
        Some(policy.expression),
      )?;
      Some((state, admission, loaded?))
    }

    pub(crate) fn load_current_step_from_geometry(
      actor_id: ActorId,
      head: &ActorContractHeadOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
      cursor: u32,
      tail_chunk: Option<(u32, &ActorStepChunkOf<T>)>,
    ) -> Option<LoadedActorStep<StepOf<T>>> {
      let loaded =
        Self::load_current_step_geometry(actor_id, head, certificate, cursor, tail_chunk)?;
      let behavior = if head.header.has_close_when {
        let policy =
          Self::load_service_behavior_policy_with_price(actor_id, &head.header, certificate)?;
        Some(Self::behavior_control_weight_context(&policy)?)
      } else {
        None
      };
      Self::qualify_current_step_behavior(head.header.step_count, loaded, behavior)
    }

    fn qualify_current_step_behavior(
      step_count: u32,
      loaded: LoadedActorStepOf<T>,
      behavior: Option<BehaviorControlWeightContext>,
    ) -> Option<LoadedActorStepOf<T>> {
      if let Some(behavior) = behavior {
        let units = loaded
          .step
          .precondition
          .as_ref()
          .map_or(0, Precondition::evaluation_units);
        let context = Self::step_control_weight_context(step_count, loaded.cursor, units)?;
        T::StepControlWeight::maximum_behavior_control_weight(context, &loaded.step, behavior)
          .filter(|weight| weight.all_lte(loaded.resources.control))?;
      }
      Some(loaded)
    }

    /// Load only the ordinary current-Step geometry, before optional policy/financial reads.
    /// The caller must separately admit and qualify policy control before using this Step.
    fn load_current_step_geometry(
      actor_id: ActorId,
      head: &ActorContractHeadOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
      cursor: u32,
      tail_chunk: Option<(u32, &ActorStepChunkOf<T>)>,
    ) -> Option<LoadedActorStepOf<T>> {
      if !Self::financial_wait_geometry_qualified(head.header.on_unfunded)
        || !certificate.has_valid_identity()
        || certificate.semantic_contract_id != head.header.semantic_contract_id
        || certificate.body_commitment != head.header.body_commitment
        || certificate.admission_identity != head.header.admission_identity
      {
        return None;
      }
      if cursor == 0 {
        if tail_chunk.is_some() || head.header.step_count == 0 {
          return None;
        }
        return Some(LoadedActorStep {
          cursor,
          step: head.first_step.clone()?,
          resources: head.first_step_resources?,
        });
      }
      let expected_chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
      let expected_first_step_index =
        1u32.checked_add(expected_chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
      let (chunk_index, chunk) = tail_chunk?;
      if chunk_index != expected_chunk_index
        || !chunk.matches(
          &actor_id,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          expected_first_step_index,
        )
      {
        return None;
      }
      let local_index = cursor.checked_sub(expected_first_step_index)? as usize;
      let resources = *chunk.step_resources.get(local_index)?;
      if chunk.steps.len() != chunk.step_resources.len() {
        return None;
      }
      Some(LoadedActorStep {
        cursor,
        step: chunk.steps.get(local_index)?.clone(),
        resources,
      })
    }

    pub(crate) fn decompose_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      admission_identity: [u8; 32],
      pipeline_machine_envelope: PipelineMachineEnvelope<T::Balance>,
      resources: &ActorAdmissionResourcesOf<T>,
    ) -> Option<(ActorContractHeadOf<T>, Vec<(u32, ActorStepChunkOf<T>)>)> {
      let semantic_contract_id = contract.semantic_contract_id();
      let body_commitment = contract.body_commitment()?;
      let header = contract.try_header(
        semantic_contract_id,
        body_commitment,
        admission_identity,
        pipeline_machine_envelope,
      )?;
      if resources.len() != contract.steps.len() {
        return None;
      }
      let first_step = contract.steps.first().cloned();
      let first_step_resources = resources.first().copied();
      let authority = ActorBodyAuthority {
        actor_id,
        semantic_contract_id,
        body_commitment,
        admission_identity,
      };
      let chunks = contract
        .steps
        .as_slice()
        .get(1..)
        .unwrap_or_default()
        .chunks(MAX_STEPS_PER_TAIL_CHUNK as usize)
        .enumerate()
        .map(|(chunk_index, steps)| {
          let chunk_index = u32::try_from(chunk_index).ok()?;
          let first_step_index =
            1u32.checked_add(chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
          let first_resource = usize::try_from(first_step_index).ok()?;
          let last_resource = first_resource.checked_add(steps.len())?;
          Some((
            chunk_index,
            ActorStepChunk {
              authority: authority.clone(),
              first_step_index,
              steps: BoundedVec::try_from(steps.to_vec()).ok()?,
              step_resources: BoundedVec::try_from(
                resources
                  .as_slice()
                  .get(first_resource..last_resource)?
                  .to_vec(),
              )
              .ok()?,
            },
          ))
        })
        .collect::<Option<Vec<_>>>()?;
      Some((
        ActorContractHead {
          header,
          first_step,
          first_step_resources,
        },
        chunks,
      ))
    }

    fn load_authenticated_geometry_close_when(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
    ) -> Option<Option<CloseWhenOf<T>>> {
      Some(
        match Self::load_contract_behavior_policy(actor_id, header)? {
          Some(policy) => {
            Self::load_contract_behavior_monitor(actor_id, header, &policy)?;
            Some(policy.expression)
          }
          None => None,
        },
      )
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn benchmark_geometry_behavior_authentication(
      actor_id: ActorId,
      header: &ActorContractHeaderOf<T>,
    ) -> Option<Option<CloseWhenOf<T>>> {
      Self::load_authenticated_geometry_close_when(actor_id, header)
    }

    pub(crate) fn reconstruct_contract_geometry(
      actor_id: ActorId,
      head: ActorContractHeadOf<T>,
      chunks: &[(u32, ActorStepChunkOf<T>)],
    ) -> Option<ActorContractOf<T>> {
      if !Self::financial_wait_geometry_qualified(head.header.on_unfunded)
        || head.header.step_count > T::MaxContractSteps::get()
        || chunks.len()
          != head
            .header
            .step_count
            .saturating_sub(1)
            .div_ceil(MAX_STEPS_PER_TAIL_CHUNK) as usize
      {
        return None;
      }
      let close_when = Self::load_authenticated_geometry_close_when(actor_id, &head.header)?;
      let mut steps = Vec::with_capacity(head.header.step_count as usize);
      match (head.first_step, head.first_step_resources) {
        (Some(first_step), Some(_)) if head.header.step_count > 0 => steps.push(first_step),
        (None, None) if head.header.step_count == 0 => {}
        _ => return None,
      }
      for (expected_chunk_index, (chunk_index, chunk)) in chunks
        .iter(/* deos-bypass: bounded-iter */)
        .enumerate()
      {
        let expected_chunk_index = u32::try_from(expected_chunk_index).ok()?;
        if *chunk_index != expected_chunk_index
          || chunk.steps.is_empty()
          || chunk.steps.len() != chunk.step_resources.len()
          || (expected_chunk_index + 1 < chunks.len() as u32
            && chunk.steps.len() != MAX_STEPS_PER_TAIL_CHUNK as usize)
        {
          return None;
        }
        let first_step_index =
          1u32.checked_add(expected_chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
        if !chunk.matches(
          &actor_id,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          first_step_index,
        ) {
          return None;
        }
        steps.extend(chunk
            .steps
            .iter(/* deos-bypass: bounded-iter */)
            .cloned());
      }
      if steps.len() != head.header.step_count as usize {
        return None;
      }
      let contract = ActorContract {
        trigger: head.header.trigger,
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        steps: ContractSteps::<T>::try_from(steps).ok()?,
        funding: head.header.funding,
        close_when,
        on_unfunded: head.header.on_unfunded,
        parked_balance_activation: head.header.parked_balance_activation,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
      };
      if contract.semantic_contract_id() != head.header.semantic_contract_id
        || contract.body_commitment()? != head.header.body_commitment
      {
        return None;
      }
      Some(contract)
    }

    pub(crate) fn derive_active_actor_view(
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      contract: ActorContractOf<T>,
    ) -> ActiveActorViewOf<T> {
      ActiveActorView {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger: contract.trigger,
        cooldown_blocks: contract.cooldown_blocks,
        window: contract.window,
        steps: contract.steps,
        has_close_when: contract.close_when.is_some(),
        on_unfunded: contract.on_unfunded,
        trigger_runtime_state: hot.trigger_runtime_state.clone(),
        cycle_nonce: identity.cycle_nonce,
        auto_close_at_cycle_nonce: contract.auto_close_at_cycle_nonce,
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        queue_ticket: hot.queue_ticket,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        last_control_mutation_block: identity.last_control_mutation_block,
        schedule_anchor: hot.schedule_anchor,
        temporal_anchor_tick: hot.trigger_runtime_state.temporal_anchor_tick(),
        temporal_occurrence_consumed: hot.trigger_runtime_state.temporal_occurrence_consumed(),
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub(crate) fn control_identity_exists(actor_id: ActorId) -> bool {
      ActorSemanticStates::<T>::contains_key(actor_id)
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn validate_loaded_step_authority(
      actor_id: ActorId,
      queue_ticket: QueueTicket,
      certificate: &ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> bool {
      certificate.has_valid_identity()
        && ticket.actor_id == actor_id
        && ticket.ticket == queue_ticket
        && ticket.cursor == loaded_step.cursor
        && ticket.contract_commitment.semantic_contract_id == certificate.semantic_contract_id
        && ticket.contract_commitment.body_commitment == certificate.body_commitment
    }

    pub(crate) fn load_control_identity(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
      ActorSemanticStates::<T>::get(actor_id).map(|state| match state {
        ActorSemanticState::Dormant(record) => record.identity,
        ActorSemanticState::Active(record) => record.identity,
      })
    }

    /// Loads the canonical generation-bound identity for active process carriers.
    pub(crate) fn load_actor_ref(actor_id: ActorId) -> Option<ActorRef> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) if record.generation != 0 => Some(ActorRef {
          actor_id,
          generation: record.generation,
        }),
        ActorSemanticState::Dormant(_) | ActorSemanticState::Active(_) => None,
      }
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn control_hot_exists(actor_id: ActorId) -> bool {
      matches!(
        ActorSemanticStates::<T>::get(actor_id),
        Some(ActorSemanticState::Active(_))
      )
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_control_hot(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) => Some(record.hot),
        ActorSemanticState::Dormant(_) => None,
      }
    }

    #[cfg(any(feature = "try-runtime", all(test, feature = "runtime-benchmarks")))]
    pub(crate) fn canonical_hot_entries_for_try_state() -> Vec<(ActorRef, ActorHotStateOf<T>)> {
      ActorSemanticStates::<T>::iter(/* deos-bypass: bounded-iter */)
        .filter_map(|(actor_id, state)| match state {
          ActorSemanticState::Active(record) => Some((
            ActorRef {
              actor_id,
              generation: record.generation,
            },
            record.hot,
          )),
          ActorSemanticState::Dormant(_) => None,
        })
        .collect()
    }

    /// Loads one active semantic owner and its exact canonical process carrier only when the
    /// generation-bound process, concrete residence, and semantic owner agree and no legacy
    /// control authority remains.
    pub(crate) fn load_canonical_actor_semantic_state(
      actor: ActorRef,
    ) -> Result<(ActorSemanticRecordOf<T>, ActorProcessOf<T>), ActorSemanticLoadError> {
      Self::load_canonical_actor_semantic_state_with_process(actor, None, None)
    }

    fn load_canonical_actor_semantic_state_with_process(
      actor: ActorRef,
      supplied_process: Option<ActorProcessOf<T>>,
      supplied_record: Option<ActorSemanticRecordOf<T>>,
    ) -> Result<(ActorSemanticRecordOf<T>, ActorProcessOf<T>), ActorSemanticLoadError> {
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ActorSemanticLoadError::Corrupt);
      }
      let record = match supplied_record {
        Some(record) => record,
        None => match ActorSemanticStates::<T>::get(actor.actor_id) {
          Some(ActorSemanticState::Active(record)) => record,
          _ => return Err(ActorSemanticLoadError::Corrupt),
        },
      };
      let process = supplied_process
        .or_else(|| ActorProcesses::<T>::get(actor.actor_id))
        .filter(|process| process.generation == actor.generation)
        .ok_or(ActorSemanticLoadError::Corrupt)?;
      if record.generation != actor.generation {
        return Err(ActorSemanticLoadError::Corrupt);
      }

      let service_node = ServiceNodes::<T>::get(actor.actor_id);
      let deadline_handle = DeadlineHandles::<T>::get(actor.actor_id);
      let pending_owner = PendingCheckOwners::<T>::get(actor.actor_id);
      let deadline_slot_matches = |handle: DeadlineHandleOf<T>| {
        DeadlinePages::<T>::get(handle.key, handle.page).is_some_and(|stored_page| {
          stored_page
            .entries
            .get(usize::from(handle.slot))
            .copied()
            .flatten()
            == Some(actor)
        })
      };
      let carrier_is_coherent = match (process.status, process.residence) {
        (ProcessStatus::Serving, Some(ProcessResidence::Service(kind))) => {
          service_node.is_some_and(|node| node.generation == actor.generation && node.kind == kind)
            && deadline_handle.is_none()
            && pending_owner.is_none()
        }
        (ProcessStatus::Serving, Some(ProcessResidence::Deadline { key, page, slot })) => {
          let expected = DeadlineHandle {
            actor,
            key,
            page,
            slot,
          };
          service_node.is_none()
            && pending_owner.is_none()
            && deadline_handle == Some(expected)
            && deadline_slot_matches(expected)
        }
        (ProcessStatus::AwaitingFunding, Some(ProcessResidence::Deadline { key, page, slot })) => {
          let expected = DeadlineHandle {
            actor,
            key,
            page,
            slot,
          };
          service_node.is_none()
            && pending_owner.is_none()
            && deadline_handle == Some(expected)
            && deadline_slot_matches(expected)
            && Self::financial_wait_matches_process(actor, &record, &process)
        }
        (ProcessStatus::AwaitingFunding, Some(ProcessResidence::Service(kind))) => {
          service_node.is_some_and(|node| node.generation == actor.generation && node.kind == kind)
            && deadline_handle.is_none()
            && pending_owner.is_none()
            && Self::financial_wait_matches_process(actor, &record, &process)
        }
        (ProcessStatus::Serving, Some(ProcessResidence::Parked(evidence))) => {
          let owner_matches = pending_owner.is_some_and(|owner| {
            owner.actor == actor && evidence.plan_identity == record.admission.admission_identity
          });
          let deadline_matches = match DependencyTimedReviews::<T>::get(actor.actor_id) {
            Some(review) => deadline_handle.is_some_and(|handle| {
              Self::deadline_handle_matches_process(handle, &process)
                && deadline_slot_matches(handle)
                && handle.key == review.deadline
                && pending_owner == Some(review.owner)
                && matches!(handle.key, WakeupKey::Block(block) if evidence.review_at == Some(block))
            }),
            None => deadline_handle.is_none() && evidence.review_at.is_none(),
          };
          let episode_matches = ParkedBalanceEpisodes::<T>::get(actor.actor_id)
            .is_some_and(|episode| pending_owner == Some(episode.owner));
          service_node.is_none() && owner_matches && deadline_matches && episode_matches
        }
        (ProcessStatus::Disabled(_), None) => {
          service_node.is_none() && deadline_handle.is_none() && pending_owner.is_none()
        }
        (ProcessStatus::Retired(_), None) | (_, _) => false,
      };
      carrier_is_coherent
        .then_some((record, process))
        .ok_or(ActorSemanticLoadError::Corrupt)
    }

    /// Loads one active semantic owner and its exact canonical Service residence only when the
    /// generation-bound process, ring node, and semantic owner agree.
    pub(crate) fn load_service_actor_semantic_state_with_kind(
      actor: ActorRef,
    ) -> Result<(ActorSemanticRecordOf<T>, ServiceResidenceKind), ActorSemanticLoadError> {
      let (record, process) = Self::load_canonical_actor_semantic_state(actor)?;
      let Some(ProcessResidence::Service(kind)) = process.residence else {
        return Err(ActorSemanticLoadError::Corrupt);
      };
      Ok((record, kind))
    }

    pub(crate) fn load_service_actor_semantic_state(
      actor: ActorRef,
      kind: ServiceResidenceKind,
    ) -> Result<ActorSemanticRecordOf<T>, ActorSemanticLoadError> {
      let (record, actual_kind) = Self::load_service_actor_semantic_state_with_kind(actor)?;
      (actual_kind == kind)
        .then_some(record)
        .ok_or(ActorSemanticLoadError::Corrupt)
    }

    /// Atomically commits one retained canonical Service attempt before advancing its ring head.
    /// Any semantic or frontier refusal rolls back both owners and leaves the member retryable.
    #[cfg(test)]
    pub(crate) fn commit_retained_service_attempt(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
            return Err(ServiceRoundError::CorruptRing);
          }
          Self::try_store_service_control_state(actor, kind, identity, hot)
            .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
          Self::advance_service_head(actor, now)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Atomically installs one complete generation-bound parking destination before releasing the
    /// exact canonical Service member. Refusal retains semantic state, Run, attempts, and topology.
    fn commit_parked_balance_destination(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      plan_revision: u64,
      desired: &[DependencyPlanSource],
      review_at: BlockNumberFor<T>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let record = Self::load_service_actor_semantic_state(actor, kind)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
          let (state, admission, loaded_step) = Self::load_actor_service_state_with_control(
            actor.actor_id,
            record.identity.clone(),
            record.hot.clone(),
            record.admission.clone(),
          )
          .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if state.identity != record.identity
            || state.hot != record.hot
            || admission != record.admission
            || state.hot.cycle_state != CycleState::Idle
            || state.run_state.is_some()
            || loaded_step.is_none()
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let owner = PendingCheckOwner {
            actor,
            plan_revision,
          };
          let deadline_destination =
            Self::plan_deadline_destination(actor, WakeupKey::Block(review_at))
              .map_err(DependencyRegistrationError::from)?;
          let evidence = ParkEvidence {
            plan_identity: admission.admission_identity,
            reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
            review_at: Some(review_at),
          };
          if PendingCheckOwners::<T>::contains_key(actor.actor_id)
            || !DependencyPlans::<T>::get(actor.actor_id).is_empty()
            || DependencyTimedReviews::<T>::contains_key(actor.actor_id)
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          PendingCheckOwners::<T>::insert(actor.actor_id, owner);
          let mutation = Self::commit_negative_dependency_plan(
            owner,
            desired,
            Some(WakeupKey::Block(review_at)),
          )?;
          Self::remove_service_member(actor)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != actor.generation || process.status != ProcessStatus::Serving {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          process.residence = Some(ProcessResidence::Parked(evidence));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(deadline_destination)
            .map_err(DependencyRegistrationError::from)?;
          Ok(mutation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn prepare_parked_balance_sources(
      actor: ActorRef,
      plan_revision: PlanRevision,
      activation: &ParkedBalanceActivationOf<T>,
    ) -> Result<(ParkedBalanceWatchesOf<T>, Vec<DependencyPlanSource>), ParkedBalanceTransitionError>
    {
      if ParkedBalanceEpisodes::<T>::contains_key(actor.actor_id) {
        return Err(ParkedBalanceTransitionError::EpisodeAlreadyExists);
      }
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor.actor_id)
      else {
        return Err(ParkedBalanceTransitionError::Dependency(
          DependencyRegistrationError::StoredPlanMismatch,
        ));
      };
      if record.generation != actor.generation
        || PendingCheckOwners::<T>::contains_key(actor.actor_id)
        || !DependencyPlans::<T>::get(actor.actor_id).is_empty()
        || DependencyTimedReviews::<T>::contains_key(actor.actor_id)
      {
        return Err(ParkedBalanceTransitionError::Dependency(
          DependencyRegistrationError::StoredPlanMismatch,
        ));
      }
      let watches = Self::certify_parked_balance_activation(
        &record.identity.sovereign_account,
        activation,
        plan_revision,
      )
      .map_err(ParkedBalanceTransitionError::Certification)?;
      let mut desired = Vec::with_capacity(activation.watches.len());
      for rule in &activation.watches {
        let source = match Self::resolve_balance_dependency_source(rule.asset).map_err(|_| {
          ParkedBalanceTransitionError::Dependency(DependencyRegistrationError::StoredPlanMismatch)
        })? {
          DependencySourceMutation::Allocated(source)
          | DependencySourceMutation::Existing(source) => source,
        };
        let revision = DependencyRevisions::<T>::get(source);
        if revision.exhausted {
          return Err(ParkedBalanceTransitionError::Dependency(
            DependencyRegistrationError::SourceExhausted,
          ));
        }
        desired.push(DependencyPlanSource {
          source,
          observed_revision: revision.revision,
        });
      }
      Ok((watches, desired))
    }

    /// Publishes an initial or replacement episode without an intermediate Service membership.
    /// The enclosing publication transaction owns semantic/process and both Deadline carriers.
    pub(crate) fn arm_initial_parked_balance(
      actor: ActorRef,
      plan_revision: PlanRevision,
      activation: &ParkedBalanceActivationOf<T>,
      review_deadline: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), ParkedBalanceTransitionError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ParkedBalanceTransitionError::Dependency(
          DependencyRegistrationError::TransactionRequired,
        ));
      }
      if !matches!(review_deadline, WakeupKey::Block(_)) {
        return Err(ParkedBalanceTransitionError::InvalidReviewClock);
      }
      let (watches, desired) =
        Self::prepare_parked_balance_sources(actor, plan_revision, activation)?;
      let owner = PendingCheckOwner {
        actor,
        plan_revision,
      };
      PendingCheckOwners::<T>::insert(actor.actor_id, owner);
      Self::commit_negative_dependency_plan(owner, &desired, Some(review_deadline))
        .map_err(ParkedBalanceTransitionError::Dependency)?;
      ParkedBalanceEpisodes::<T>::insert(actor.actor_id, ParkedBalanceEpisode { owner, watches });
      Ok(())
    }

    /// Captures and publishes one fixed-anchor parked-balance episode with a mandatory bounded
    /// block review. The complete destination is installed before Service membership is released;
    /// any certification, capacity, or publication refusal rolls back the anchor set as well.
    #[allow(
      dead_code,
      reason = "parked-balance publication is staged behind Actor Contract admission"
    )]
    pub(crate) fn transfer_service_member_to_parked_balance(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      plan_revision: PlanRevision,
      activation: &ParkedBalanceActivationOf<T>,
      review_deadline: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<DependencyPlanMutation, ParkedBalanceTransitionError> {
      let WakeupKey::Block(review_at) = review_deadline else {
        return Err(ParkedBalanceTransitionError::InvalidReviewClock);
      };
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::load_canonical_actor_semantic_state(actor).map_err(|_| {
            ParkedBalanceTransitionError::Dependency(
              DependencyRegistrationError::StoredPlanMismatch,
            )
          })?;
          let (watches, desired) =
            Self::prepare_parked_balance_sources(actor, plan_revision, activation)?;
          let mutation = Self::commit_parked_balance_destination(
            actor,
            kind,
            plan_revision,
            &desired,
            review_at,
          )
          .map_err(ParkedBalanceTransitionError::Dependency)?;
          let owner = PendingCheckOwner {
            actor,
            plan_revision,
          };
          ParkedBalanceEpisodes::<T>::insert(
            actor.actor_id,
            ParkedBalanceEpisode { owner, watches },
          );
          Ok(mutation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact negative dependency result only after its complete successor plan is
    /// durable for the same generation/plan-bound Park resident. Refusal preserves the Pending
    /// result, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "negative dependency continuation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_negative_dependency_event_and_rearm(
      expected: PendingDependencyEvent,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let mutation = Self::consume_pending_dependency_event(expected, desired, timed_review)?;
          let review_at = match timed_review {
            Some(WakeupKey::Block(block)) => Some(block),
            Some(WakeupKey::Tick(_)) => {
              return Err(DependencyRegistrationError::ClockUnavailable);
            }
            None => None,
          };
          let mut process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          process.residence = Some(ProcessResidence::Parked(ParkEvidence {
            review_at,
            ..evidence
          }));
          ActorProcesses::<T>::insert(expected.owner.actor.actor_id, process);
          Ok(mutation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact due-review negative result only after its complete successor plan is
    /// durable for the same generation/plan-bound Park resident. Refusal preserves the Pending
    /// review, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "negative dependency review continuation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_negative_dependency_review_and_rearm(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let mutation = Self::consume_pending_dependency_review(expected, desired, timed_review)?;
          let review_at = match timed_review {
            Some(WakeupKey::Block(block)) => Some(block),
            Some(WakeupKey::Tick(_)) => {
              return Err(DependencyRegistrationError::ClockUnavailable);
            }
            None => None,
          };
          let mut process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          process.residence = Some(ProcessResidence::Parked(ParkEvidence {
            review_at,
            ..evidence
          }));
          ActorProcesses::<T>::insert(expected.owner.actor.actor_id, process);
          Ok(mutation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Resource-admits one complete causal parked-balance Pending interpretation.
    #[allow(
      dead_code,
      reason = "causal parked-balance worker awaits the Pending selector cutover"
    )]
    pub(crate) fn process_pending_parked_balance_event(
      meter: &mut WeightMeter,
      expected: PendingDependencyEvent,
      kind: ServiceResidenceKind,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
      due_successor: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyReviewWorkerError> {
      let weight = T::WeightInfo::process_pending_parked_balance_event();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(weight);
      Self::interpret_pending_parked_balance_event(expected, kind, evidence, now, due_successor)
        .map_err(DependencyReviewWorkerError::Interpretation)
    }

    /// Rechecks one causal parked-balance Pending event against current total ownership. A
    /// subthreshold result consumes the event into the complete current source snapshot while
    /// retaining fixed anchors and the mandatory review; a qualified result wakes B+1 Service.
    /// Only the due traversal supplies a successor after detaching the old deadline; ordinary
    /// causal service retains its existing future review.
    #[allow(
      dead_code,
      reason = "causal parked-balance interpretation is staged behind the weighted Pending consumer"
    )]
    pub(crate) fn interpret_pending_parked_balance_event(
      expected: PendingDependencyEvent,
      kind: ServiceResidenceKind,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
      due_successor: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let episode = ParkedBalanceEpisodes::<T>::get(expected.owner.actor.actor_id)
            .filter(|episode| episode.owner == expected.owner)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          let Some(ActorSemanticState::Active(record)) =
            ActorSemanticStates::<T>::get(expected.owner.actor.actor_id)
          else {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          };
          let qualification = classify_parked_balance_plan(&episode.watches, |asset| {
            (
              T::AssetOps::minimum_balance(asset),
              T::AssetOps::total_balance(&record.identity.sovereign_account, asset),
            )
          })
          .map_err(|error| match error {
            ParkedBalanceClassificationError::MinimumBalanceChanged => {
              DependencyRegistrationError::ParkedBalanceMinimumChanged
            }
            ParkedBalanceClassificationError::ThresholdOverflow => {
              DependencyRegistrationError::ParkedBalanceThresholdOverflow
            }
          })?;
          match qualification {
            ParkedBalanceQualification::Qualified => {
              let timed_deadline = DependencyTimedReviews::<T>::get(expected.owner.actor.actor_id)
                .map(|review| review.deadline)
                .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
              if let Some(deadline) = DeadlineHandles::<T>::get(expected.owner.actor.actor_id) {
                if deadline.actor != expected.owner.actor || timed_deadline != deadline.key {
                  return Err(DependencyRegistrationError::StoredPlanMismatch);
                }
                Self::remove_deadline_member(expected.owner.actor)
                  .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
              }
              Self::consume_positive_dependency_event_and_wake(expected, kind, evidence, now)?;
              Ok(DependencyReviewMutation::Woke)
            }
            ParkedBalanceQualification::BelowThreshold => {
              let plan = DependencyPlans::<T>::get(expected.owner.actor.actor_id);
              let mut desired = Vec::with_capacity(plan.len());
              for registration in &plan {
                let revision = DependencyRevisions::<T>::get(registration.source);
                if revision.exhausted {
                  return Err(DependencyRegistrationError::SourceExhausted);
                }
                desired.push(DependencyPlanSource {
                  source: registration.source,
                  observed_revision: revision.revision,
                });
              }
              let deadline = due_successor.or_else(|| {
                DependencyTimedReviews::<T>::get(expected.owner.actor.actor_id)
                  .map(|review| review.deadline)
              });
              Self::consume_negative_dependency_event_and_rearm(
                expected, evidence, &desired, deadline,
              )
              .map(DependencyReviewMutation::Rearmed)
            }
          }
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact positive dependency result and wakes its generation/plan-bound Park
    /// resident only after revalidating the current Pending and registration authority.
    #[allow(
      dead_code,
      reason = "positive dependency wake remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_positive_dependency_event_and_wake(
      expected: PendingDependencyEvent,
      _kind: ServiceResidenceKind,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingEventMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingEventMismatch);
            }
            Some(_) => {}
          }
          if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let source_state = DependencyRevisions::<T>::get(expected.source);
          if source_state.exhausted || source_state.revision < expected.revision {
            return Err(DependencyRegistrationError::RevisionMismatch);
          }
          let registration =
            DependencyRegistrations::<T>::get(expected.source, expected.owner.actor.actor_id)
              .ok_or(DependencyRegistrationError::RegistrationMissing)?;
          if registration.actor != expected.owner.actor
            || registration.plan_revision != expected.owner.plan_revision
            || registration.acknowledged_revision < expected.revision
          {
            return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
          }
          PendingDependencyEvents::<T>::remove(expected.owner.actor.actor_id);
          Self::publish_positive_dependency_readiness(
            expected.owner.actor,
            expected.owner,
            evidence,
            now,
          )
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact positive due review and wakes its generation/plan-bound Park resident
    /// only while every retained source still has the interpreted revision. Refusal preserves the
    /// Pending review, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "positive dependency review wake remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_positive_dependency_review_and_wake(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      observed: &[DependencyPlanSource],
      _kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingReviewMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingReviewMismatch);
            }
            Some(_) => {}
          }
          let plan = DependencyPlans::<T>::get(expected.owner.actor.actor_id);
          if plan.len() != observed.len() {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let mut index = 0usize;
          while index < plan.len() {
            let registration = &plan[index];
            let snapshot = &observed[index];
            // A timed check may cover publication that the source scan has not acknowledged.
            // The observation must still be current; retained acknowledgement cannot be newer.
            if registration.source != snapshot.source
              || registration.handle.actor != expected.owner.actor
              || registration.handle.plan_revision != expected.owner.plan_revision
              || registration.handle.acknowledged_revision > snapshot.observed_revision
              || {
                let source_state = DependencyRevisions::<T>::get(snapshot.source);
                source_state.exhausted || source_state.revision != snapshot.observed_revision
              }
            {
              return Err(DependencyRegistrationError::RevisionMismatch);
            }
            index = index
              .checked_add(1)
              .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          }
          PendingDependencyReviews::<T>::remove(expected.owner.actor.actor_id);
          Self::publish_positive_dependency_readiness(
            expected.owner.actor,
            expected.owner,
            evidence,
            now,
          )
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Interprets one exact Pending due review from one bounded current-source snapshot, then
    /// atomically selects either canonical wake or complete-plan re-arm. Any ambiguous
    /// interpretation or authority race preserves the Pending review and Park residence.
    #[allow(
      dead_code,
      reason = "due-review interpretation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn interpret_pending_dependency_review<F>(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
      interpret: F,
    ) -> Result<DependencyReviewMutation, DependencyRegistrationError>
    where
      F: FnOnce(
        &[DependencyPlanSource],
      ) -> Result<DependencyReviewInterpretation, DependencyRegistrationError>,
    {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingReviewMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingReviewMismatch);
            }
            Some(_) => {}
          }
          if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let plan = DependencyPlans::<T>::get(expected.owner.actor.actor_id);
          let mut observed =
            BoundedVec::<DependencyPlanSource, T::MaxParkedBalanceWatches>::default();
          for registration in &plan {
            if registration.handle.actor != expected.owner.actor
              || registration.handle.plan_revision != expected.owner.plan_revision
              || DependencyRegistrations::<T>::get(
                registration.source,
                expected.owner.actor.actor_id,
              ) != Some(registration.handle)
            {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            let source_state = DependencyRevisions::<T>::get(registration.source);
            if source_state.exhausted {
              return Err(DependencyRegistrationError::SourceExhausted);
            }
            observed
              .try_push(DependencyPlanSource {
                source: registration.source,
                observed_revision: source_state.revision,
              })
              .map_err(|_| DependencyRegistrationError::PlanTooLarge)?;
          }
          match interpret(&observed)? {
            DependencyReviewInterpretation::Positive => {
              Self::consume_positive_dependency_review_and_wake(
                expected, evidence, &observed, kind, now,
              )?;
              Ok(DependencyReviewMutation::Woke)
            }
            DependencyReviewInterpretation::Negative => {
              Self::consume_negative_dependency_review_and_rearm(
                expected,
                evidence,
                &observed,
                next_review,
              )
              .map(DependencyReviewMutation::Rearmed)
            }
          }
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Interprets one due fixed-anchor balance review from current host total ownership. A
    /// negative result re-arms the deadline without replacing any anchor; a positive result wakes
    /// Service and reclaims the episode through the common generation-bound transition.
    #[allow(
      dead_code,
      reason = "parked-balance review is staged behind Actor Contract admission"
    )]
    pub(crate) fn interpret_pending_parked_balance_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyRegistrationError> {
      let episode = ParkedBalanceEpisodes::<T>::get(expected.owner.actor.actor_id)
        .filter(|episode| episode.owner == expected.owner)
        .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
      if ActorControlLocators::<T>::contains_key(expected.owner.actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(expected.owner.actor.actor_id)
      {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      let Some(ActorSemanticState::Active(record)) =
        ActorSemanticStates::<T>::get(expected.owner.actor.actor_id)
      else {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      };
      let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
        .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
      if record.generation != expected.owner.actor.generation
        || process.generation != expected.owner.actor.generation
        || process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Parked(evidence))
      {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      Self::interpret_pending_dependency_review(
        expected,
        evidence,
        kind,
        now,
        next_review,
        |snapshot| {
          if snapshot.len() != episode.watches.len()
            || snapshot
              .iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the retained source snapshot. */)
              .zip(episode.watches.iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the episode. */))
              .any(|(observed, watch)| {
                BalanceDependencySources::<T>::get(watch.asset) != Some(observed.source)
                  || DependencySourceBalances::<T>::get(observed.source) != Some(watch.asset)
              })
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let qualification = classify_parked_balance_plan(&episode.watches, |asset| {
            (
              T::AssetOps::minimum_balance(asset),
              T::AssetOps::total_balance(&record.identity.sovereign_account, asset),
            )
          })
          .map_err(|error| match error {
            ParkedBalanceClassificationError::MinimumBalanceChanged => {
              DependencyRegistrationError::ParkedBalanceMinimumChanged
            }
            ParkedBalanceClassificationError::ThresholdOverflow => {
              DependencyRegistrationError::ParkedBalanceThresholdOverflow
            }
          })?;
          Ok(match qualification {
            ParkedBalanceQualification::BelowThreshold => DependencyReviewInterpretation::Negative,
            ParkedBalanceQualification::Qualified => DependencyReviewInterpretation::Positive,
          })
        },
      )
    }

    /// Component-wise maximum of every complete dependency-review and pending-event owner.
    pub(crate) fn dependency_review_weight_upper() -> Weight {
      T::WeightInfo::process_due_parked_balance_review()
        .max(T::WeightInfo::process_due_parked_balance_review_deep_index())
        .max(T::WeightInfo::process_pending_parked_balance_event())
    }

    /// Due review may require permanent namespace cleanup; prepaid destruction does not provide
    /// current-block capacity. Ordinary pre-due causal service retains its separate owner.
    pub(crate) fn due_dependency_review_weight_upper() -> Weight {
      Self::dependency_review_weight_upper().saturating_add(Self::close_dispatch_weight_upper())
    }

    /// Resource-admits one complete fixed-anchor balance review. Classification and either
    /// same-anchor rearm or Service wake commit atomically with Pending publication.
    #[allow(
      dead_code,
      reason = "bounded parked-balance review is staged behind Actor Contract admission"
    )]
    pub(crate) fn process_due_parked_balance_review(
      meter: &mut WeightMeter,
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyReviewWorkerError> {
      let weight = T::WeightInfo::process_due_parked_balance_review();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(weight);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::publish_due_dependency_review(expected)
            .map_err(DependencyReviewWorkerError::Publication)?;
          Self::interpret_pending_parked_balance_review(expected, evidence, kind, now, next_review)
            .map_err(DependencyReviewWorkerError::Interpretation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Carries one due or causally pending parked-balance obligation through its complete owner.
    /// Park residences exist only for parked-balance plans; any other evidence is a carrier fault.
    fn process_parked_dependency_review(
      meter: &mut WeightMeter,
      actor: ActorRef,
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyReviewWorkerError> {
      match PendingDependencyEvents::<T>::get(actor.actor_id) {
        Some(pending) => Self::process_pending_parked_balance_event(
          meter,
          pending,
          kind,
          evidence,
          now,
          next_review,
        ),
        None => {
          Self::process_due_parked_balance_review(meter, expected, evidence, kind, now, next_review)
        }
      }
    }

    /// Traverses the earliest due block deadline and carries one indexed review through its
    /// family-specific complete-attempt owner. Insufficient Weight performs no reads or mutation;
    /// every admitted refusal restores the exact deadline, Pending, Park, and Service topology.
    #[allow(
      dead_code,
      reason = "bounded due-review traversal remains staged behind the mandatory service cutover"
    )]
    pub(crate) fn process_next_due_block_dependency_review(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<(ActorRef, DependencyReviewMutation), DependencyReviewWorkerError> {
      let weight = Self::due_dependency_review_weight_upper();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      let before = meter.consumed();
      let mut original_actor = None;
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let key = Self::deadline_index_get(WakeupClock::Block, 0).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::MemberMissing),
          )?;
          if !matches!(key, WakeupKey::Block(block) if block <= now) {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::InvalidDestination,
            ));
          }
          let header = DeadlineHeaders::<T>::get(key).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let page = DeadlinePages::<T>::get(key, header.first_page).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let actor = page
            .entries
            .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
            .find_map(|entry| *entry)
            .ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let process = ActorProcesses::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::ProcessMissing),
          )?;
          let Some(ProcessResidence::Parked(evidence)) = process.residence else {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::ProcessResidenceMismatch,
            ));
          };
          let expected = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
          )?;
          if expected.owner.actor != actor || expected.deadline != key {
            return Err(DependencyReviewWorkerError::Publication(
              DependencyDueReviewError::ReviewMismatch,
            ));
          }
          original_actor = Some(actor);
          Self::remove_deadline_member(actor).map_err(DependencyReviewWorkerError::Deadline)?;
          let mutation = Self::process_parked_dependency_review(
            meter,
            actor,
            expected,
            evidence,
            kind,
            now,
            next_review,
          )?;
          if matches!(mutation, DependencyReviewMutation::Rearmed(_)) {
            let review = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
              DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
            )?;
            let destination = Self::plan_deadline_destination(actor, review.deadline)
              .map_err(DependencyReviewWorkerError::Deadline)?;
            Self::insert_deadline_member(destination)
              .map_err(DependencyReviewWorkerError::Deadline)?;
          }
          Ok((actor, mutation))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      });
      let outcome = if outcome
        == Err(DependencyReviewWorkerError::Deadline(
          DeadlineMutationError::NamespaceExhausted,
        )) {
        // Only the raw carrier cause selects this terminal after whole-review rollback. Reload
        // strict full original authority; a partially published successor is never Close input.
        (|| {
          let actor = original_actor.ok_or(DependencyReviewWorkerError::TerminalCleanup)?;
          let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor.actor_id) else {
            return Err(DependencyReviewWorkerError::TerminalCleanup);
          };
          let run = state.run_state.clone();
          Self::remove_actor_publication_and_finalize(
            actor,
            state,
            run.as_ref(),
            CloseReason::SchedulerIndexExhausted,
          )
          .map_err(|_| DependencyReviewWorkerError::TerminalCleanup)?;
          Ok((actor, DependencyReviewMutation::Closed))
        })()
      } else {
        outcome
      };
      // Rollback restores authority, not work. Only committed nonterminal work releases cleanup;
      // destruction and refusal retain the complete independent Control maximum exactly once.
      if !matches!(
        outcome,
        Ok((
          _,
          DependencyReviewMutation::Woke | DependencyReviewMutation::Rearmed(_)
        ))
      ) {
        let charged = meter.consumed().saturating_sub(before);
        meter.consume(weight.saturating_sub(charged));
      }
      outcome
    }

    /// Classifies one member from the shared earliest due block bucket without mutation.
    pub(crate) fn classify_next_due_block_deadline(
      now: BlockNumberFor<T>,
    ) -> Result<DueBlockDeadlineBranch, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Block, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Block(block) if block <= now) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let actor = page
        .entries
        .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
        .find_map(|entry| *entry)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
      if process.status != ProcessStatus::Serving {
        return Err(DeadlineMutationError::ProcessResidenceMismatch);
      }
      match process.residence {
        Some(ProcessResidence::Deadline { .. }) => Ok(DueBlockDeadlineBranch::Retry(actor)),
        Some(ProcessResidence::Parked(_)) => Ok(DueBlockDeadlineBranch::Review(actor)),
        _ => Err(DeadlineMutationError::ProcessResidenceMismatch),
      }
    }

    /// Retained-bucket vacancy maintenance and bucket deletion own different storage branches.
    pub(crate) fn deadline_return_weight_upper() -> Weight {
      T::WeightInfo::return_due_block_deadline_to_service()
        .max(T::WeightInfo::return_due_block_deadline_to_service_deep_index())
    }

    /// Component-wise upper bound for the three reachable non-useful Cadenced busy residences.
    /// The useful owner remains a conservative fallback until regenerated host weights override
    /// every newly promoted branch.
    pub(crate) fn cadenced_busy_rearm_weight_upper() -> Weight {
      T::WeightInfo::cadenced_running_rearm()
        .max(T::WeightInfo::cadenced_suspended_service_rearm())
        .max(T::WeightInfo::cadenced_suspended_deadline_rearm())
        .max(T::WeightInfo::cadenced_trigger_occurrence())
    }

    /// Classifies and processes one member from the shared earliest due block bucket. Sleeping
    /// retries return directly to Service; Parked members alone enter the timed-review worker.
    /// Classification and the selected complete branch each have an independent generated Weight
    /// owner, so refusal cannot inspect state or consume a member under the wrong branch envelope.
    #[allow(
      dead_code,
      reason = "mixed deadline traversal remains staged behind the mandatory service cutover"
    )]
    pub(crate) fn process_next_due_block_deadline(
      meter: &mut WeightMeter,
      _kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DueBlockDeadlineMutation, DependencyReviewWorkerError> {
      let selector_weight = T::WeightInfo::classify_due_block_deadline();
      if !meter.can_consume(selector_weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(selector_weight);
      match Self::classify_next_due_block_deadline(now)
        .map_err(DependencyReviewWorkerError::Deadline)?
      {
        DueBlockDeadlineBranch::Retry(actor) => {
          let branch_weight = Self::deadline_return_weight_upper();
          if !meter.can_consume(branch_weight) {
            return Err(DependencyReviewWorkerError::InsufficientWeight);
          }
          // A failed return can roll back source removal, but not the performed Control work.
          meter.consume(branch_weight);
          Self::return_due_deadline_member_to_service(actor, ServiceResidenceKind::Live, now)
            .map_err(DependencyReviewWorkerError::Deadline)?;
          Ok(DueBlockDeadlineMutation::RetryReturned(actor))
        }
        DueBlockDeadlineBranch::Review(_) => Self::process_next_due_block_dependency_review(
          meter,
          ServiceResidenceKind::Pending,
          now,
          next_review,
        )
        .map(|(actor, mutation)| DueBlockDeadlineMutation::ReviewProcessed(actor, mutation)),
        DueBlockDeadlineBranch::FinancialWait(_)
        | DueBlockDeadlineBranch::TemporalTrigger(_)
        | DueBlockDeadlineBranch::TemporalTriggerBusy(_) => Err(
          DependencyReviewWorkerError::Deadline(DeadlineMutationError::InvalidDestination),
        ),
      }
    }

    /// Classifies one member from the shared earliest due Tick bucket without mutation. Financial
    /// deadlines have an explicit process reason and never dispatch as an Action retry.
    pub(crate) fn classify_next_due_tick_deadline(
      now_tick: SchedulerTick,
    ) -> Result<DueBlockDeadlineBranch, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Tick, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Tick(tick) if tick <= now_tick) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let actor = page
        .entries
        .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
        .find_map(|entry| *entry)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if TriggerDeadlineHandles::<T>::get(actor.actor_id)
        .is_some_and(|handle| handle.actor == actor && handle.key == key)
      {
        let ActorSemanticState::Active(semantic) = ActorSemanticStates::<T>::get(actor.actor_id)
          .ok_or(DeadlineMutationError::ProcessMissing)?
        else {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        };
        if semantic.generation != actor.generation {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
        return Ok(match semantic.hot.cycle_state {
          CycleState::Running | CycleState::Suspended => {
            DueBlockDeadlineBranch::TemporalTriggerBusy(actor)
          }
          CycleState::Idle => DueBlockDeadlineBranch::TemporalTrigger(actor),
        });
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
      if process.status == ProcessStatus::AwaitingFunding {
        return (process.generation == actor.generation
          && matches!(process.residence,
            Some(ProcessResidence::Deadline { key: owned_key, .. }) if owned_key == key))
        .then_some(DueBlockDeadlineBranch::FinancialWait(actor))
        .ok_or(DeadlineMutationError::ProcessResidenceMismatch);
      }
      match process.residence {
        Some(ProcessResidence::Deadline { .. }) => Ok(DueBlockDeadlineBranch::Retry(actor)),
        Some(ProcessResidence::Parked(_)) => Ok(DueBlockDeadlineBranch::Review(actor)),
        _ => Err(DeadlineMutationError::ProcessResidenceMismatch),
      }
    }

    /// Direct fixture worker; production admits and settles through metered Tick selection.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn process_due_temporal_deadline(
      actor: ActorRef,
      now_tick: SchedulerTick,
    ) -> Result<DueTickDeadlineMutation, DependencyReviewWorkerError> {
      Self::process_due_temporal_deadline_outcome(actor, now_tick).map(|(mutation, _, _)| mutation)
    }

    /// Reports the selected mutation, committed destruction and conservative wait-work settlement.
    /// The caller admits occurrence, optional wait entry and cleanup before extracting the source.
    fn process_due_temporal_deadline_outcome(
      actor: ActorRef,
      now_tick: SchedulerTick,
    ) -> Result<(DueTickDeadlineMutation, bool, bool), DependencyReviewWorkerError> {
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let handle = TriggerDeadlineHandles::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::MemberMissing),
          )?;
          if handle.actor != actor
            || !matches!(handle.key, WakeupKey::Tick(tick) if tick <= now_tick)
          {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::ProcessResidenceMismatch,
            ));
          }
          let Some(ActorSemanticState::Active(mut semantic)) =
            ActorSemanticStates::<T>::get(actor.actor_id)
          else {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::ProcessMissing,
            ));
          };
          if semantic.generation != actor.generation {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::StaleGeneration,
            ));
          }
          let (mut state, admission, loaded_step) = Self::load_actor_service_state_with_control(
            actor.actor_id,
            semantic.identity.clone(),
            semantic.hot.clone(),
            semantic.admission.clone(),
          )
          .ok_or(DependencyReviewWorkerError::TemporalOccurrence)?;
          if state.contract.on_unfunded == OnUnfunded::AwaitFunding
            && matches!(
              state.hot.trigger_runtime_state,
              TriggerRuntimeState::AtTime {
                anchor_tick: Some(_),
                consumed: false
              }
            )
          {
            let instance = Self::derive_active_actor_view(
              state.identity.clone(),
              state.hot.clone(),
              state.contract.clone(),
            );
            let classification = Self::classify_actor_loaded(&instance, state.run_state.as_ref())
              .map_err(|_| DependencyReviewWorkerError::TemporalOccurrence)?;
            let breakdown = Self::trigger_fee_for_actor(
              actor.actor_id,
              state.identity.actor_class.actor_type(),
              state.contract.on_unfunded,
              TriggerFamily::AtTime,
              T::WeightInfo::at_time_trigger_occurrence(),
            )
            .map_err(|_| DependencyReviewWorkerError::TemporalOccurrence)?;
            if classification.terminal_reason.is_none()
              && !Self::trigger_occurrence_capacity_sufficient(
                state.identity.actor_class.actor_type(),
                &state.identity.sovereign_account,
                breakdown,
              )
              .map_err(|_| DependencyReviewWorkerError::TemporalOccurrence)?
            {
              Self::temporal_financial_wait_weights()
                .ok_or(DependencyReviewWorkerError::TemporalOccurrence)?;
              let WakeupKey::Tick(due_tick) = handle.key else {
                return Err(DependencyReviewWorkerError::TemporalOccurrence);
              };
              Self::publish_financial_wait(actor, UnpaidActorService::AtTimeTrigger { due_tick })
                .map_err(|error| match error {
                FinancialWaitPublicationError::Deadline(error) => {
                  DependencyReviewWorkerError::Deadline(error)
                }
                _ => DependencyReviewWorkerError::TemporalOccurrence,
              })?;
              return Ok((
                DueTickDeadlineMutation::TemporalTriggerProcessed(actor),
                false,
                true,
              ));
            }
          }
          Self::remove_trigger_deadline_member(actor)
            .map_err(DependencyReviewWorkerError::Deadline)?;
          semantic.hot.trigger_wakeup_pointer = None;
          state.hot.trigger_wakeup_pointer = None;
          ActorSemanticStates::<T>::insert(actor.actor_id, ActorSemanticState::Active(semantic));
          let closed = Self::process_due_temporal_occurrence_loaded(
            actor.actor_id,
            state,
            admission,
            loaded_step,
            now_tick,
          )
          .map_err(DependencyReviewWorkerError::from)?;
          Ok((
            DueTickDeadlineMutation::TemporalTriggerProcessed(actor),
            closed,
            false,
          ))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      });
      if outcome
        == Err(DependencyReviewWorkerError::Deadline(
          DeadlineMutationError::NamespaceExhausted,
        ))
      {
        // The failed quantum has restored the original due source and Run. Never finalize
        // partially published rearm authority, and never infer permanence from dispatch aliases.
        let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor.actor_id) else {
          return Err(DependencyReviewWorkerError::TemporalOccurrence);
        };
        let run = state.run_state.clone();
        Self::remove_actor_publication_and_finalize(
          actor,
          state,
          run.as_ref(),
          CloseReason::SchedulerIndexExhausted,
        )
        .map_err(|_| DependencyReviewWorkerError::TemporalOccurrence)?;
        return Ok((
          DueTickDeadlineMutation::TemporalTriggerProcessed(actor),
          true,
          true,
        ));
      }
      outcome
    }

    /// Processes one temporal Trigger from the independent Tick frontier. Selection is admitted
    /// before inspection, then the selected branch is admitted before mutation. Block-clock
    /// members and incoherent Tick retries or Park reviews remain untouched.
    pub(crate) fn process_next_due_tick_deadline(
      meter: &mut WeightMeter,
      now_tick: SchedulerTick,
    ) -> Result<DueTickDeadlineMutation, DependencyReviewWorkerError> {
      let selector_weight = T::WeightInfo::classify_due_tick_deadline();
      if !meter.can_consume(selector_weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(selector_weight);
      let branch = Self::classify_next_due_tick_deadline(now_tick)
        .map_err(DependencyReviewWorkerError::Deadline)?;
      if let DueBlockDeadlineBranch::FinancialWait(actor) = branch {
        return Self::process_financial_wait_due(actor, now_tick, meter)
          .map_err(DependencyReviewWorkerError::FinancialWait);
      }
      let temporal = match branch {
        DueBlockDeadlineBranch::TemporalTrigger(actor) => Some((
          actor,
          T::WeightInfo::at_time_trigger_occurrence()
            .max(T::WeightInfo::cadenced_trigger_occurrence()),
        )),
        DueBlockDeadlineBranch::TemporalTriggerBusy(actor) => {
          Some((actor, Self::cadenced_busy_rearm_weight_upper()))
        }
        _ => None,
      };
      if let Some((actor, weight)) = temporal {
        let (entry, cleanup) = Self::temporal_financial_wait_weights()
          .unwrap_or((Weight::zero(), Self::close_dispatch_weight_upper()));
        if !meter.can_consume(weight.saturating_add(entry).saturating_add(cleanup)) {
          return Err(DependencyReviewWorkerError::InsufficientWeight);
        }
        meter.consume(weight);
        let result = Self::process_due_temporal_deadline_outcome(actor, now_tick);
        if !matches!(&result, Ok((_, _, false))) {
          meter.consume(entry);
        }
        // A failed transaction can already have performed provisional destruction. Rollback
        // restores authority, not work; only committed nonterminal work releases cleanup capacity.
        if !matches!(&result, Ok((_, false, _))) {
          meter.consume(cleanup);
        }
        return result.map(|(mutation, _, _)| mutation);
      }
      Err(DependencyReviewWorkerError::Deadline(
        DeadlineMutationError::ProcessResidenceMismatch,
      ))
    }

    /// Optional wait entry and its complete lifecycle maximum remain distinct from occurrence work.
    /// Hosts without wait qualification retain the ordinary temporal envelope exactly.
    fn temporal_financial_wait_weights() -> Option<(Weight, Weight)> {
      let bytes = Self::financial_wait_episode_bytes()?;
      let entry = T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes)?;
      T::AdmissionCertificateAuthority::financial_wait_close_additional_weight(bytes)?;
      let cleanup = T::AdmissionCertificateAuthority::current()?
        .maximum_lifecycle_weight
        .max(Self::close_dispatch_weight_upper());
      let occurrence = T::WeightInfo::at_time_trigger_occurrence()
        .max(T::WeightInfo::cadenced_trigger_occurrence())
        .max(Self::cadenced_busy_rearm_weight_upper());
      occurrence.checked_add(&entry)?.checked_add(&cleanup)?;
      Some((entry, cleanup))
    }

    /// Prepaid destruction and optional waiting still require current-block capacity.
    pub(crate) fn temporal_deadline_weight_upper(occurrence: Weight) -> Weight {
      let (entry, cleanup) = Self::temporal_financial_wait_weights()
        .unwrap_or((Weight::zero(), Self::close_dispatch_weight_upper()));
      occurrence.saturating_add(entry).saturating_add(cleanup)
    }

    /// Complete two-clock admission bound; mutually exclusive branches use component-wise maxima.
    pub fn deadline_service_weight_upper() -> Weight {
      let review = Self::due_dependency_review_weight_upper();
      let temporal = T::WeightInfo::at_time_trigger_occurrence()
        .max(T::WeightInfo::cadenced_trigger_occurrence())
        .max(Self::cadenced_busy_rearm_weight_upper());
      let tick = Self::temporal_deadline_weight_upper(temporal);
      let tick = Self::financial_wait_due_weights()
        .map_or(tick, |(due, cleanup)| tick.max(due.saturating_add(cleanup)));
      T::WeightInfo::classify_due_block_deadline()
        .saturating_add(Self::deadline_return_weight_upper().max(review))
        .saturating_add(T::WeightInfo::classify_due_tick_deadline())
        .saturating_add(tick)
    }

    /// Gives each independent deadline clock one bounded mandatory-service attempt in fixed
    /// Block-then-Tick order. The complete maximum two-frontier envelope is admitted before either
    /// selector reads storage, so an absent, refused, or continuously busy Block frontier cannot
    /// consume the Tick frontier's authority (and vice versa). Each branch still settles only its
    /// actual generated selector and worker Weight through the shared meter.
    pub(crate) fn service_due_deadline_frontiers(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      now_tick: SchedulerTick,
      next_block_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DueDeadlineServicePass, DependencyReviewWorkerError> {
      let complete_envelope = Self::deadline_service_weight_upper();
      if !meter.can_consume(complete_envelope) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      let block = Self::process_next_due_block_deadline(meter, kind, now, next_block_review);
      let tick = Self::process_next_due_tick_deadline(meter, now_tick);
      Ok(DueDeadlineServicePass { block, tick })
    }

    /// Read-only diagnostic of existing head admission, not an execution permit.
    /// Its own inspection cost is unpriced and excluded; no production caller may use this bound.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn service_head_allowance_diagnostic(
      now: BlockNumberFor<T>,
    ) -> Option<ActorStepResourceEnvelope> {
      if now != frame_system::Pallet::<T>::block_number() || GlobalCircuitBreaker::<T>::get() {
        return None;
      }
      let header = ServiceHeader::<T>::get();
      let actor = header.cursor?;
      if header.count == 0
        || header.count > T::MaxQueueLength::get()
        || header.round_block.is_some_and(|round| round > now)
      {
        return None;
      }
      let node = ServiceNodes::<T>::get(actor.actor_id)?;
      if node.generation != actor.generation
        || node.last_considered >= now
        || node.eligible_from > now
      {
        return None;
      }
      if header.count == 1 {
        if node.previous != actor || node.next != actor {
          return None;
        }
      } else {
        if node.previous == actor
          || node.next == actor
          || (header.count == 2) != (node.previous == node.next)
        {
          return None;
        }
        let previous = ServiceNodes::<T>::get(node.previous.actor_id)?;
        let next = ServiceNodes::<T>::get(node.next.actor_id)?;
        if previous.generation != node.previous.generation
          || next.generation != node.next.generation
          || previous.next != actor
          || next.previous != actor
        {
          return None;
        }
      }
      let process = ActorProcesses::<T>::get(actor.actor_id)?;
      if process.generation != actor.generation
        || process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Service(node.kind))
        || process
          .last_attempted
          .is_some_and(|attempted| attempted >= now)
      {
        return None;
      }
      let (record, _) =
        Self::load_canonical_actor_semantic_state_with_process(actor, Some(process), None).ok()?;
      let head = ActorContractHeads::<T>::get(actor.actor_id)?;
      if head.header.step_count == 0
        || head.header.step_count > T::MaxContractSteps::get()
        || head.header.has_close_when
        || head.header.on_unfunded != OnUnfunded::Close
        || !Self::admission_matches_current_authority(&record.admission)
        || !record
          .hot
          .trigger_runtime_state
          .is_compatible_with(&head.header.trigger)
        || !record
          .admission
          .authorizes_wake(head.header.trigger.wake_qualification(&head.header.window))
      {
        return None;
      }
      let (_, loaded) =
        Self::load_actor_service_geometry(actor.actor_id, &record.hot, &record.admission, &head)?;
      let resources = Self::current_step_service_resources(&loaded?)?;
      let inspection = T::WeightInfo::service_round_begin_populated()
        .checked_add(&T::WeightInfo::service_round_probe_eligible())?
        .checked_add(&T::WeightInfo::scheduler_actor_state_probe())?;
      let suffix = T::WeightInfo::service_round_admit_eligible()
        .max(T::WeightInfo::service_member_retire_interior())
        .max(T::WeightInfo::service_member_retire_pair_cursor())
        .max(T::WeightInfo::service_member_retire_singleton());
      let control = inspection
        .checked_add(&resources.control)?
        .checked_add(&Self::close_dispatch_weight_upper())?
        .checked_add(&suffix)?
        .checked_add(
          &T::AdmissionCertificateAuthority::service_refusal_completion_weight()
            .unwrap_or_else(Weight::zero),
        )?;
      control.checked_add(&resources.effect)?;
      Some(ActorStepResourceEnvelope {
        control,
        effect: resources.effect,
      })
    }

    /// Admits canonical inspection, then the complete Control/effect branch. Actor workers own
    /// nested rollback; qualified collector refusal may retain only round/cursor completion.
    /// Capacity and unsafe refusal retain the head; neither rollback erases performed work.
    pub(crate) fn service_canonical_round_head(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        None,
        BlockResourceDomain::ActorDrainEffect,
        None,
      )
      .map(|(encounter, _)| encounter)
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn service_canonical_round_head_with_resources(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      limits: BlockResourceLimits,
      effect_domain: BlockResourceDomain,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        Some((state, limits)),
        effect_domain,
        None,
      )
      .map(|(encounter, _)| encounter)
    }

    /// Variant for callers already holding the pass-wide `ActorControl` reservation. Nested step
    /// control reserves no second ledger share, but every branch must fit the caller's remaining
    /// Control independently and report its full actual work to `CyclePass` reconciliation.
    pub(crate) fn service_canonical_round_head_with_reserved_control(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      limits: BlockResourceLimits,
      effect_domain: BlockResourceDomain,
      control_remaining: Weight,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        Some((state, limits)),
        effect_domain,
        Some(control_remaining),
      )
      .map(|(encounter, _)| encounter)
    }

    /// Missing physical qualification is not permission to reuse an unrelated normal suffix.
    fn service_refusal_completion_for_contract(contract: &ActorContractOf<T>) -> Option<Weight> {
      if contract.close_when.is_some() || contract.on_unfunded != OnUnfunded::Close {
        return None;
      }
      T::AdmissionCertificateAuthority::service_refusal_completion_weight()
    }

    /// Called only after a nested Actor worker rolled back a qualified collector refusal.
    /// The source and its local ring neighborhood must still be the admitted generation. The
    /// only writes are round/cursor control; last_attempted, semantic state and Run are untouched.
    fn complete_refused_service_turn(
      actor: ActorRef,
      source: &ActiveActorStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      expected_process: ActorProcessOf<T>,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      let (record, process) = Self::load_canonical_actor_semantic_state(actor)
        .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
      if process != expected_process
        || record.identity != source.identity
        || record.hot != source.hot
        || record.admission != *admission
        || ActorRunStateStore::<T>::get(actor.actor_id) != source.run_state
      {
        return Err(ServiceRoundError::ProcessResidenceMismatch);
      }
      let header = ServiceHeader::<T>::get();
      let node = ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      if header.cursor != Some(actor)
        || header.count == 0
        || header.count > T::MaxQueueLength::get()
      {
        return Err(ServiceRoundError::CorruptRing);
      }
      if header.count == 1 {
        if node.previous != actor || node.next != actor {
          return Err(ServiceRoundError::CorruptRing);
        }
      } else {
        if node.previous == actor
          || node.next == actor
          || (header.count == 2) != (node.previous == node.next)
        {
          return Err(ServiceRoundError::CorruptRing);
        }
        let previous = ServiceNodes::<T>::get(node.previous.actor_id)
          .filter(|previous| previous.generation == node.previous.generation)
          .ok_or(ServiceRoundError::CorruptRing)?;
        let next = ServiceNodes::<T>::get(node.next.actor_id)
          .filter(|next| next.generation == node.next.generation)
          .ok_or(ServiceRoundError::CorruptRing)?;
        if previous.next != actor || next.previous != actor {
          return Err(ServiceRoundError::CorruptRing);
        }
      }
      Self::advance_idle_service_head(actor, now)
    }

    /// StopCycle has no Action effect. Preserve its stored/priced maximum while assigning its
    /// complete execution allowance to Control for admission, attempts and actual settlement.
    fn current_step_service_resources(
      loaded: &LoadedActorStepOf<T>,
    ) -> Option<ActorStepResourceEnvelope> {
      Self::step_service_resources(&loaded.step, loaded.resources)
    }

    fn step_service_resources(
      step: &StepOf<T>,
      resources: ActorStepResourceEnvelope,
    ) -> Option<ActorStepResourceEnvelope> {
      if matches!(step.task, ActorTask::StopCycle) {
        Some(ActorStepResourceEnvelope {
          control: resources.control.checked_add(&resources.effect)?,
          effect: Weight::zero(),
        })
      } else {
        Some(resources)
      }
    }

    /// Admit every reachable financial service branch before reading the financial companion.
    /// Ordinary geometry is reused after admission; Creation-backed cleanup is reserved here
    /// without being charged again. AtTime recovery and zero-Step/control-only plans need no
    /// economic effect capacity.
    fn financial_service_admission_resources(
      head: &ActorContractHeadOf<T>,
      record: &ActorSemanticRecordOf<T>,
      loaded_step: Option<&LoadedActorStepOf<T>>,
      financial_waiting: bool,
      financial_inspection: Weight,
    ) -> Option<ActorStepResourceEnvelope> {
      let bytes = Self::financial_wait_episode_bytes()?;
      let certified_cleanup = record.admission.maximum_lifecycle_weight;
      let minimum_cleanup = Self::close_dispatch_weight_upper().checked_add(
        &T::AdmissionCertificateAuthority::financial_wait_close_additional_weight(bytes)?,
      )?;
      if !minimum_cleanup.all_lte(certified_cleanup) {
        return None;
      }
      let cleanup = if head.header.has_close_when {
        certified_cleanup
      } else {
        minimum_cleanup
      };
      let policy_bounds = if head.header.has_close_when {
        Some(Self::behavior_service_inspection_bounds()?)
      } else {
        None
      };
      let policy_inspection = match policy_bounds {
        Some(bounds) => T::StepControlWeight::maximum_behavior_service_inspection_weight(bounds)?,
        None => Weight::zero(),
      };
      let suffix = T::WeightInfo::service_round_admit_eligible().max(
        T::WeightInfo::service_member_retire_interior()
          .max(T::WeightInfo::service_member_retire_pair_cursor())
          .max(T::WeightInfo::service_member_retire_singleton()),
      );
      // The canonical loader still verifies the exact owed source after admission. The only
      // coherent waiting source with unlatched Idle semantics is the original unpaid AtTime.
      let unpaid_at_time = financial_waiting
        && record.hot.cycle_state == CycleState::Idle
        && !record.hot.pending_signal;
      let (step_control, effect) = if unpaid_at_time {
        (T::WeightInfo::at_time_trigger_occurrence(), Weight::zero())
      } else if record.hot.cycle_state == CycleState::Idle && !record.hot.pending_signal {
        (suffix, Weight::zero())
      } else if let Some(loaded) = loaded_step {
        let resources = Self::current_step_service_resources(loaded)?;
        (resources.control.checked_add(&suffix)?, resources.effect)
      } else {
        let zero_step = match policy_bounds {
          Some(bounds) => T::StepControlWeight::maximum_behavior_zero_step_service_weight(bounds)?,
          None => T::WeightInfo::scheduler_inner_zero_step_complete(),
        };
        (zero_step.checked_add(&suffix)?, Weight::zero())
      };
      let control = financial_inspection
        .checked_add(&policy_inspection)?
        .checked_add(&step_control.max(suffix))?
        .checked_add(&if unpaid_at_time {
          Weight::zero()
        } else {
          T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes)?
        })?
        .checked_add(&cleanup)?;
      control.checked_add(&effect)?;
      Some(ActorStepResourceEnvelope { control, effect })
    }

    pub(crate) fn service_canonical_round_head_inner(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      mut resource_authority: Option<(
        &mut BlockResourceState<BlockNumberFor<T>>,
        BlockResourceLimits,
      )>,
      effect_domain: BlockResourceDomain,
      caller_control_remaining: Option<Weight>,
    ) -> Result<
      (
        ServiceRoundEncounter,
        Option<crate::scheduler::ActorAttemptEvidence>,
      ),
      ServiceRoundError,
    > {
      let selector_envelope = T::WeightInfo::service_round_begin_populated()
        .saturating_add(T::WeightInfo::service_round_probe_eligible());
      let loaded_classification_envelope = T::WeightInfo::scheduler_actor_state_probe();
      let mut inspection_envelope =
        selector_envelope.saturating_add(loaded_classification_envelope);
      let zero_step_suffix = T::WeightInfo::service_round_admit_eligible().max(
        T::WeightInfo::service_member_retire_interior()
          .max(T::WeightInfo::service_member_retire_pair_cursor())
          .max(T::WeightInfo::service_member_retire_singleton()),
      );
      let control_owned_by_caller = caller_control_remaining.is_some();
      if !meter.can_consume(selector_envelope)
        || caller_control_remaining.is_some_and(|remaining| !selector_envelope.all_lte(remaining))
      {
        return Err(ServiceRoundError::DiscoveryUnavailable);
      }
      // Admit round discovery before any topology read or round-marker mutation. Empty/closed
      // encounters need no Actor loading; eligible heads admit that separate owner below.
      let mut inspection_reservation =
        if let Some((resource_state, limits)) = resource_authority.as_mut() {
          if control_owned_by_caller {
            None
          } else {
            Some(
              resource_state
                .reserve(
                  *limits,
                  BlockResourceDomain::ActorControl,
                  selector_envelope,
                )
                .map_err(|_| ServiceRoundError::DiscoveryUnavailable)?,
            )
          }
        } else {
          None
        };
      let mut classification_reservation = None;
      let mut policy_inspection_reservation = None;
      let mut policy_inspection_weight = Weight::zero();
      let mut financial_inspection_reservation = None;
      let mut financial_inspection_weight = Weight::zero();
      let mut resource_after_inspection = resource_authority.as_ref().map(|(state, _)| **state);
      let mut loaded_classification = false;
      // Set only after complete admission, before entering a mutating branch. Semantic rollback
      // restores topology/custody, not work already performed under this envelope.
      let mut incurred: Option<ActorStepResourceEnvelope> = None;
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let can_consume = |control: Weight, effect: Weight| {
            meter.can_consume(control.saturating_add(effect))
              && caller_control_remaining.is_none_or(|remaining| control.all_lte(remaining))
          };
          Self::begin_service_round(now)?;
          let mut encounter = Self::consider_service_head(now)?;
          let mut execution_weight = Weight::zero();
          let mut attempt = None;
          if let ServiceRoundEncounter::Eligible(actor) = encounter {
            if !can_consume(inspection_envelope, Weight::zero()) {
              return Err(ServiceRoundError::InsufficientWeight);
            }
            if let Some((resource_state, limits)) = resource_authority.as_mut() {
              if !control_owned_by_caller {
                classification_reservation = Some(
                  resource_state
                    .reserve(
                      *limits,
                      BlockResourceDomain::ActorControl,
                      loaded_classification_envelope,
                    )
                    .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                );
              }
            }
            resource_after_inspection = resource_authority.as_ref().map(|(state, _)| **state);
            loaded_classification = true;
            let process = ActorProcesses::<T>::get(actor.actor_id)
              .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
            let financial_waiting = process.status == ProcessStatus::AwaitingFunding;
            let head = ActorContractHeads::<T>::get(actor.actor_id)
              .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
            let mut supplied_record = None;
            let mut supplied_geometry = None;
            if financial_waiting || head.header.on_unfunded == OnUnfunded::AwaitFunding {
              let Some(ActorSemanticState::Active(record)) =
                ActorSemanticStates::<T>::get(actor.actor_id)
              else {
                return Err(ServiceRoundError::ProcessResidenceMismatch);
              };
              if record.generation != actor.generation
                || !Self::admission_matches_current_authority(&record.admission)
                || head.header.semantic_contract_id != record.admission.semantic_contract_id
                || head.header.body_commitment != record.admission.body_commitment
                || head.header.admission_identity != record.admission.admission_identity
              {
                return Err(ServiceRoundError::ProcessResidenceMismatch);
              }
              let geometry = Self::load_actor_service_geometry(
                actor.actor_id,
                &record.hot,
                &record.admission,
                &head,
              )
              .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
              let additional = T::AdmissionCertificateAuthority::financial_wait_service_weight(
                Self::financial_wait_episode_bytes()
                  .ok_or(ServiceRoundError::ResourceUnavailable)?,
              )
              .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let admission = Self::financial_service_admission_resources(
                &head,
                &record,
                geometry.1.as_ref(),
                financial_waiting,
                additional,
              )
              .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let admission_control = inspection_envelope
                .checked_add(&admission.control)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              admission_control
                .checked_add(&admission.effect)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(admission_control, admission.effect) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              // Check the same frozen paired budget without booking a second reservation.
              // Actual inspection and execution retain their separate reservations/settlement.
              if let Some((resource_state, limits)) = resource_authority.as_ref() {
                let mut preflight = **resource_state;
                preflight
                  .reserve_actor_step(
                    *limits,
                    effect_domain,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      admission.control
                    },
                    admission.effect,
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              let complete = inspection_envelope
                .checked_add(&additional)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(complete, Weight::zero()) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              if let Some((resource_state, limits)) = resource_authority.as_mut() {
                if !control_owned_by_caller {
                  financial_inspection_reservation = Some(
                    resource_state
                      .reserve(*limits, BlockResourceDomain::ActorControl, additional)
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  );
                }
              }
              financial_inspection_weight = additional;
              inspection_envelope = complete;
              resource_after_inspection = resource_authority.as_ref().map(|(state, _)| **state);
              supplied_record = Some(record);
              supplied_geometry = Some(geometry);
            }
            let (semantic, source_process) =
              Self::load_canonical_actor_semantic_state_with_process(
                actor,
                Some(process),
                supplied_record,
              )
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
            let Some(ProcessResidence::Service(service_kind)) = source_process.residence else {
              return Err(ServiceRoundError::ProcessResidenceMismatch);
            };
            let financial = if financial_waiting {
              let episode = ActorFinancialWaits::<T>::get(actor.actor_id)
                .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
              Some(episode)
            } else {
              if head.header.on_unfunded == OnUnfunded::AwaitFunding
                && ActorFinancialWaits::<T>::contains_key(actor.actor_id)
              {
                return Err(ServiceRoundError::ProcessResidenceMismatch);
              }
              None
            };
            if head.header.has_close_when {
              let additional = T::StepControlWeight::maximum_behavior_service_inspection_weight(
                Self::behavior_service_inspection_bounds()
                  .ok_or(ServiceRoundError::ResourceUnavailable)?,
              )
              .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let complete = inspection_envelope
                .checked_add(&additional)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(complete, Weight::zero()) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              if let Some((resource_state, limits)) = resource_authority.as_mut() {
                if !control_owned_by_caller {
                  policy_inspection_reservation = Some(
                    resource_state
                      .reserve(*limits, BlockResourceDomain::ActorControl, additional)
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  );
                }
              }
              policy_inspection_weight = additional;
              inspection_envelope = complete;
              resource_after_inspection = resource_authority.as_ref().map(|(state, _)| **state);
            }
            let (state, admission, loaded_step) = match supplied_geometry {
              Some(geometry) => Self::load_actor_service_state_from_geometry(
                actor.actor_id,
                semantic.identity,
                semantic.hot,
                semantic.admission,
                head,
                geometry,
              ),
              None => Self::load_actor_service_state_with_head(
                actor.actor_id,
                semantic.identity,
                semantic.hot,
                semantic.admission,
                head,
              ),
            }
            .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
            // Refusal preserves the canonical Run and its paid identity; it never cancels or
            // recharges an earlier committed Pipeline under a changed host certificate.
            if !Self::admission_matches_current_authority(&admission) {
              return Err(ServiceRoundError::ProcessResidenceMismatch);
            }
            let instance = Self::derive_active_actor_view(
              state.identity.clone(),
              state.hot.clone(),
              state.contract.clone(),
            );
            // The global circuit breaker defers all ordinary Step effects and automatic terminal
            // close while retaining exact placement, mirroring the legacy admission decision which
            // returns `Skip` before terminal or capacity classification. Explicit lifecycle and
            // sweep also refuse under the machine-freeze contract.
            let unpaid_trigger_due =
              financial
                .as_ref()
                .and_then(|episode| match episode.record().unpaid_service() {
                  UnpaidActorService::AtTimeTrigger { due_tick } => Some(due_tick),
                  _ => None,
                });
            let classification = Self::classify_actor_loaded_with_unpaid_trigger(
              &instance,
              state.run_state.as_ref(),
              unpaid_trigger_due,
            )
            .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
            if classification.execution_phase
              == crate::types::ActorExecutionPhase::GlobalCircuitBreaker
            {
              return Ok((
                ServiceRoundEncounter::BreakerRefused(actor),
                Weight::zero(),
                None,
              ));
            }
            // A due schedule window or an exhausted cycle nonce is terminal before any Step
            // attempt. The canonical service round must own that decision: an Idle resident has
            // no Step to attempt, so without this branch a window-expiry deadline returned to
            // Service would only advance the ring cursor and leave the Actor active past its
            // window. Close through the same atomic owner used by authored entry points.
            let terminal_reason = classification.terminal_reason;
            let execution_deferred = terminal_reason.is_none()
              && classification.execution_phase != crate::types::ActorExecutionPhase::Ready;
            // A latched User Idle opening must own the same admission-time insolvency decision as
            // the legacy admission path: when the sovereign balance cannot cover the ledger floor
            // plus the Pipeline Machine fee, close with `CycleAdmissionInsufficient` instead of
            // attempting an effectful or zero-Step cycle. Without this the canonical round defers
            // forever behind the protected floor. Terminal reasons take precedence; other actor
            // classes and non-Idle states keep unlimited System admission.
            let admission_insufficient = if terminal_reason.is_none()
              && state.identity.actor_class.actor_type() == ActorType::User
              && state.hot.cycle_state == CycleState::Idle
              && state.hot.pending_signal
            {
              match Self::pipeline_capacity_sufficient(
                actor.actor_id,
                ActorType::User,
                &instance.sovereign_account,
              ) {
                Ok(sufficient) => !sufficient,
                Err(_) => return Err(ServiceRoundError::ProcessResidenceMismatch),
              }
            } else {
              false
            };
            // A User retry continuation that can no longer cover its current Action's maximum fee
            // plus the protected ledger floor must terminate through the same custody-neutral
            // `CycleAdmissionInsufficient` close the legacy admission decision applies, instead of
            // re-suspending or re-attempting forever behind the floor. System Actors and non-
            // Suspended states retain their unbounded admission.
            let retry_action_insufficient = if terminal_reason.is_none()
              && state.identity.actor_class.actor_type() == ActorType::User
              && state.hot.cycle_state == CycleState::Suspended
            {
              match loaded_step.as_ref() {
                Some(loaded_step) => match Self::action_capacity_sufficient(
                  ActorType::User,
                  &instance.sovereign_account,
                  &loaded_step.step,
                  loaded_step.resources,
                ) {
                  Ok(sufficient) => !sufficient,
                  Err(_) => return Err(ServiceRoundError::ProcessResidenceMismatch),
                },
                None => false,
              }
            } else {
              false
            };
            let financial_decision = financial
              .as_ref()
              .map(|episode| {
                let record = episode.record();
                episode
                  .bind(
                    record.scope(),
                    record.unpaid_service(),
                    record.unpaid_liability(),
                    T::CadenceTickMillis::get(),
                    T::MaxTemporalDelayTicks::get(),
                  )
                  .and_then(|bound| {
                    bound.classify(
                      T::AssetOps::balance(&instance.sovereign_account, T::FeeNativeAssetId::get())
                        .saturating_sub(T::MinUserBalance::get()),
                      T::Time::now(),
                    )
                  })
                  .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)
              })
              .transpose()?;
            let close_reason = terminal_reason
              .or_else(|| {
                (financial_decision == Some(FinancialWaitDecision::GraceExpired))
                  .then_some(CloseReason::FundingGraceExpired)
              })
              .or_else(|| {
                (instance.on_unfunded == OnUnfunded::Close && admission_insufficient)
                  .then_some(CloseReason::CycleAdmissionInsufficient)
              })
              .or_else(|| {
                (instance.on_unfunded == OnUnfunded::Close && retry_action_insufficient)
                  .then_some(CloseReason::CycleAdmissionInsufficient)
              });
            let idle_no_work = state.hot.cycle_state == CycleState::Idle
              && !state.hot.pending_signal
              && state.run_state.is_none();
            if let Some(reason) = close_reason {
              let close_envelope = Self::close_control_weight_for_contract(&state.contract)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          close_envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              if !can_consume(
                inspection_envelope.saturating_add(close_envelope),
                Weight::zero(),
              ) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              incurred = Some(ActorStepResourceEnvelope {
                control: close_envelope,
                effect: Weight::zero(),
              });
              // The service view repeats only the current Step; full authored authority belongs
              // to destruction and is loaded only after its complete Control admission.
              let LoadedActorStateOf::Active(full) = Self::load_actor_state(actor.actor_id) else {
                return Err(ServiceRoundError::ProcessResidenceMismatch);
              };
              let run = full.run_state.clone();
              Self::remove_actor_publication_and_finalize(actor, full, run.as_ref(), reason)
                .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      close_envelope
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = close_envelope;
              encounter = ServiceRoundEncounter::TerminallyClosed(reason);
            } else if financial.is_none()
              && instance.on_unfunded == OnUnfunded::AwaitFunding
              && admission_insufficient
            {
              let entry = T::AdmissionCertificateAuthority::financial_wait_entry_weight(
                Self::financial_wait_episode_bytes()
                  .ok_or(ServiceRoundError::ResourceUnavailable)?,
              )
              .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let cleanup = Self::close_control_weight_for_contract(&state.contract)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let envelope = entry
                .checked_add(&cleanup)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(inspection_envelope.saturating_add(envelope), Weight::zero()) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              incurred = Some(ActorStepResourceEnvelope {
                control: envelope,
                effect: Weight::zero(),
              });
              Self::publish_financial_wait(
                actor,
                UnpaidActorService::PipelineOpening {
                  cycle_nonce: state
                    .identity
                    .cycle_nonce
                    .checked_add(1)
                    .ok_or(ServiceRoundError::ProcessResidenceMismatch)?,
                },
              )
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      entry
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = entry;
              encounter = ServiceRoundEncounter::NoWork(actor);
            } else if financial_decision == Some(FinancialWaitDecision::Awaiting) {
              Self::repark_financial_service(
                actor,
                financial
                  .as_ref()
                  .ok_or(ServiceRoundError::ProcessResidenceMismatch)?,
              )
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              encounter = ServiceRoundEncounter::NoWork(actor);
            } else if financial.as_ref().is_some_and(|episode| {
              matches!(
                episode.record().unpaid_service(),
                UnpaidActorService::AtTimeTrigger { .. }
              )
            }) {
              let recovery_control = T::WeightInfo::at_time_trigger_occurrence();
              let cleanup = Self::close_control_weight_for_contract(&state.contract)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let envelope = recovery_control
                .checked_add(&cleanup)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(inspection_envelope.saturating_add(envelope), Weight::zero()) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              incurred = Some(ActorStepResourceEnvelope {
                control: envelope,
                effect: Weight::zero(),
              });
              Self::execute_financial_at_time_recovery_on_service(
                actor,
                service_kind,
                state,
                admission,
                loaded_step,
                now,
              )
              .map_err(|cause| match cause {
                crate::scheduler::AttemptTransactionError::FeeCollection => {
                  ServiceRoundError::FeeCollection
                }
                _ => ServiceRoundError::ProcessResidenceMismatch,
              })?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      recovery_control
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = recovery_control;
              encounter = ServiceRoundEncounter::NoWork(actor);
            } else if idle_no_work || execution_deferred {
              // A completed/aborted Idle member or a member blocked by lifecycle, window, cooldown,
              // or retry eligibility retains its canonical Service residence. Advance the bounded
              // cursor without opening a pipeline, executing a Step, or recording an attempt.
              let advance_envelope = T::WeightInfo::service_round_admit_eligible();
              if !can_consume(
                inspection_envelope.saturating_add(advance_envelope),
                Weight::zero(),
              ) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          advance_envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              incurred = Some(ActorStepResourceEnvelope {
                control: advance_envelope,
                effect: Weight::zero(),
              });
              Self::advance_idle_service_head(actor, now)
                .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      advance_envelope
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = advance_envelope;
              encounter = ServiceRoundEncounter::NoWork(actor);
            } else if let Some(loaded_step) = loaded_step {
              let resources = Self::current_step_service_resources(&loaded_step)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              let control_task = matches!(loaded_step.step.task, ActorTask::StopCycle);
              let financial_entry = if state.contract.on_unfunded == OnUnfunded::AwaitFunding {
                T::AdmissionCertificateAuthority::financial_wait_entry_weight(
                  Self::financial_wait_episode_bytes()
                    .ok_or(ServiceRoundError::ResourceUnavailable)?,
                )
                .ok_or(ServiceRoundError::ResourceUnavailable)?
              } else {
                Weight::zero()
              };
              let terminal_control =
                Self::service_terminal_control_upper(&state, Some(&loaded_step.step));
              let refusal_completion =
                Self::service_refusal_completion_for_contract(&state.contract);
              let refusal_control = refusal_completion.unwrap_or_else(Weight::zero);
              let effectful_control = resources
                .control
                .saturating_add(financial_entry)
                .saturating_add(terminal_control)
                .saturating_add(
                  T::WeightInfo::service_round_admit_eligible().max(
                    T::WeightInfo::service_member_retire_interior()
                      .max(T::WeightInfo::service_member_retire_pair_cursor())
                      .max(T::WeightInfo::service_member_retire_singleton()),
                  ),
                )
                .checked_add(&refusal_control)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(
                inspection_envelope.saturating_add(effectful_control),
                resources.effect,
              ) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let maximum_fee = Self::maximum_current_action_fee(
                state.identity.actor_class.actor_type(),
                &loaded_step.step,
                resources,
              )
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              let retry_deadline =
                if let StepErrorPolicy::RetryLater { max_attempts } = loaded_step.step.on_error {
                  let attempted = state.run_state.as_ref().map_or(1, |run| {
                    run.unsuccessful_attempts_at_cursor.saturating_add(1)
                  });
                  if attempted < max_attempts {
                    let eligible_at = Self::suspension_eligible_at(
                      state.contract.cooldown_blocks,
                      state.contract.window,
                      now,
                      attempted,
                    )
                    .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
                    (now.checked_add(&One::one()) != Some(eligible_at))
                      .then_some(WakeupKey::Block(eligible_at))
                  } else {
                    None
                  }
                } else {
                  None
                };
              let plan = Self::build_canonical_current_step_plan(
                actor.actor_id,
                state.identity.clone(),
                state.hot.clone(),
                state.run_state.clone(),
                admission.clone(),
                loaded_step,
                maximum_fee,
              )
              .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
              let suffix = T::WeightInfo::service_round_admit_eligible().max(
                T::WeightInfo::service_member_retire_interior()
                  .max(T::WeightInfo::service_member_retire_pair_cursor())
                  .max(T::WeightInfo::service_member_retire_singleton()),
              );
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve_actor_step(
                        *limits,
                        effect_domain,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          effectful_control
                        },
                        resources.effect,
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              incurred = Some(ActorStepResourceEnvelope {
                control: effectful_control,
                effect: resources.effect,
              });
              let refusal_source = refusal_completion.map(|_| state.clone());
              let mut evidence = match Self::execute_effectful_step_on_service_with_deadline(
                actor,
                service_kind,
                state,
                plan,
                &admission,
                now,
                retry_deadline,
              ) {
                Ok(evidence) => evidence,
                Err(failure) => {
                  let actual_effect_weight = if control_task {
                    Weight::zero()
                  } else {
                    failure.actual_effect_weight.unwrap_or(resources.effect)
                  };
                  incurred = Some(ActorStepResourceEnvelope {
                    control: effectful_control,
                    effect: actual_effect_weight,
                  });
                  if let Some(source) = refusal_source.as_ref().filter(|_| {
                    matches!(
                      failure.cause,
                      crate::scheduler::AttemptTransactionError::FeeCollection
                    )
                  }) {
                    if !actual_effect_weight.all_lte(resources.effect) {
                      return Err(ServiceRoundError::ResourceUnavailable);
                    }
                    Self::complete_refused_service_turn(actor, source, &admission, process, now)?;
                    if let (Some((resource_state, _)), Some(reservation)) =
                      (resource_authority.as_mut(), reservation.as_mut())
                    {
                      resource_state
                        .settle_actor_step(
                          reservation,
                          if control_owned_by_caller {
                            Weight::zero()
                          } else {
                            effectful_control
                          },
                          actual_effect_weight,
                        )
                        .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
                    }
                    return Ok((
                      ServiceRoundEncounter::ConsumedRefusal(actor),
                      effectful_control.saturating_add(actual_effect_weight),
                      None,
                    ));
                  }
                  let attempt = Self::finalize_service_namespace_rollback(
                    actor,
                    failure.cause,
                    terminal_control,
                  )
                  .map_err(|cause| match cause {
                    crate::scheduler::AttemptTransactionError::FeeCollection => {
                      ServiceRoundError::FeeCollection
                    }
                    _ => ServiceRoundError::ProcessResidenceMismatch,
                  })?;
                  crate::scheduler::StepCommitEvidence {
                    actual_control_weight: effectful_control,
                    actual_effect_weight,
                    attempt: Some(attempt),
                  }
                }
              };
              if control_task {
                // The inner Task port still reports its performed StopCycle work against the
                // original stored geometry. Assign that actual work to the admitted Control
                // owner without changing the publication authority or inventing an Action.
                evidence.actual_control_weight = evidence
                  .actual_control_weight
                  .checked_add(&evidence.actual_effect_weight)
                  .ok_or(ServiceRoundError::ResourceUnavailable)?;
                evidence.actual_effect_weight = Weight::zero();
              }
              incurred = Some(ActorStepResourceEnvelope {
                control: effectful_control,
                effect: evidence.actual_effect_weight,
              });
              // Persistent-completion profiles do not certify lifecycle destruction. Keep the
              // admitted Step maximum plus cleanup until a complete terminal owner permits reclaim.
              let actual_control = if evidence
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.is_closed())
              {
                effectful_control
              } else {
                evidence
                  .actual_control_weight
                  .saturating_add(suffix)
                  .saturating_add(refusal_control)
              };
              if evidence.attempt.is_none() {
                encounter = ServiceRoundEncounter::NoWork(actor);
              }
              attempt = evidence.attempt;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle_actor_step(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      actual_control
                    },
                    evidence.actual_effect_weight,
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = actual_control.saturating_add(evidence.actual_effect_weight);
            } else {
              let zero_step_control = Self::zero_step_resource_envelope(&state.contract)
                .ok_or(ServiceRoundError::ProcessResidenceMismatch)?
                .control;
              let zero_step_base = zero_step_control.saturating_add(zero_step_suffix);
              let terminal_control = Self::service_terminal_control_upper(&state, None);
              let refusal_completion =
                Self::service_refusal_completion_for_contract(&state.contract);
              let refusal_control = refusal_completion.unwrap_or_else(Weight::zero);
              let zero_step_envelope = zero_step_base
                .saturating_add(terminal_control)
                .checked_add(&refusal_control)
                .ok_or(ServiceRoundError::ResourceUnavailable)?;
              if !can_consume(
                inspection_envelope.saturating_add(zero_step_envelope),
                Weight::zero(),
              ) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          zero_step_envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              incurred = Some(ActorStepResourceEnvelope {
                control: zero_step_envelope,
                effect: Weight::zero(),
              });
              let refusal_source = refusal_completion.map(|_| state.clone());
              let evidence = match Self::execute_zero_step_on_service(
                actor,
                service_kind,
                state,
                &admission,
                now,
              ) {
                Ok(evidence) => Ok(evidence),
                Err(cause) => {
                  if let Some(source) = refusal_source.as_ref().filter(|_| {
                    matches!(
                      cause,
                      crate::scheduler::AttemptTransactionError::FeeCollection
                    )
                  }) {
                    Self::complete_refused_service_turn(actor, source, &admission, process, now)?;
                    if let (Some((resource_state, _)), Some(reservation)) =
                      (resource_authority.as_mut(), reservation.as_mut())
                    {
                      resource_state
                        .settle(
                          reservation,
                          if control_owned_by_caller {
                            Weight::zero()
                          } else {
                            zero_step_envelope
                          },
                        )
                        .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
                    }
                    return Ok((
                      ServiceRoundEncounter::ConsumedRefusal(actor),
                      zero_step_envelope,
                      None,
                    ));
                  }
                  Self::finalize_service_namespace_rollback(actor, cause, terminal_control).map(
                    |attempt| crate::scheduler::StepCommitEvidence {
                      actual_control_weight: zero_step_control,
                      actual_effect_weight: Weight::zero(),
                      attempt: Some(attempt),
                    },
                  )
                }
              }
              .map_err(|cause| match cause {
                crate::scheduler::AttemptTransactionError::FeeCollection => {
                  ServiceRoundError::FeeCollection
                }
                _ => ServiceRoundError::ProcessResidenceMismatch,
              })?;
              let actual_control = if evidence
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.is_closed())
              {
                zero_step_envelope
              } else {
                evidence
                  .actual_control_weight
                  .saturating_add(zero_step_suffix)
                  .saturating_add(refusal_control)
              };
              attempt = evidence.attempt;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      actual_control
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = actual_control;
            }
          }
          Ok((encounter, execution_weight, attempt))
        })();
        // Final resource settlement belongs inside the same Service commit boundary. Meters and
        // the frozen resource ledger retain performed work independently of storage rollback.
        let result = (|| {
          let inspection_actual = if loaded_classification {
            inspection_envelope
          } else {
            selector_envelope
          };
          let (encounter, execution_weight, attempt) = match result {
            Ok(outcome) => outcome,
            Err(error) => {
              if let (Some((state, _)), Some(after_inspection)) =
                (resource_authority.as_mut(), resource_after_inspection)
              {
                **state = after_inspection;
              }
              for (reservation, actual) in [
                (&mut inspection_reservation, selector_envelope),
                (
                  &mut classification_reservation,
                  loaded_classification_envelope,
                ),
                (&mut policy_inspection_reservation, policy_inspection_weight),
                (
                  &mut financial_inspection_reservation,
                  financial_inspection_weight,
                ),
              ] {
                if let (Some((state, _)), Some(reservation)) =
                  (resource_authority.as_mut(), reservation.as_mut())
                {
                  state
                    .settle(reservation, actual)
                    .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
                }
              }
              meter.consume(inspection_actual);
              if let Some(work) = incurred {
                meter.consume(work.control.saturating_add(work.effect));
                if let Some((state, limits)) = resource_authority.as_mut() {
                  // Restore only the failed branch's ledger booking, from its already-admitted
                  // prefix. No new capacity is needed; known effects may refund within their domain.
                  let control = if control_owned_by_caller {
                    Weight::zero()
                  } else {
                    work.control
                  };
                  let retained = state
                    .reserve_actor_step(*limits, effect_domain, control, work.effect)
                    .and_then(|mut reservation| {
                      state.settle_actor_step(&mut reservation, control, work.effect)
                    });
                  state.halt_optional_actor_work();
                  retained.map_err(|_| ServiceRoundError::ResourceUnavailable)?;
                }
              }
              return Err(error);
            }
          };
          for (reservation, actual) in [
            (&mut inspection_reservation, selector_envelope),
            (
              &mut classification_reservation,
              loaded_classification_envelope,
            ),
            (&mut policy_inspection_reservation, policy_inspection_weight),
            (
              &mut financial_inspection_reservation,
              financial_inspection_weight,
            ),
          ] {
            if let (Some((state, _)), Some(reservation)) =
              (resource_authority.as_mut(), reservation.as_mut())
            {
              state
                .settle(reservation, actual)
                .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
            }
          }
          meter.consume(inspection_actual);
          meter.consume(execution_weight);
          Ok((encounter, attempt))
        })();
        match result {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn validate_parked_dependency_plan(
      actor: ActorRef,
      owner: PendingCheckOwner,
    ) -> Result<
      BoundedVec<DependencyPlanRegistration, T::MaxParkedBalanceWatches>,
      DependencyRegistrationError,
    > {
      let episode = ParkedBalanceEpisodes::<T>::get(actor.actor_id)
        .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
      let plan = DependencyPlans::<T>::get(actor.actor_id);
      if episode.owner != owner || episode.watches.len() != plan.len() {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      for registration in &plan {
        if registration.handle.actor != actor
          || registration.handle.plan_revision != owner.plan_revision
          || DependencyRegistrations::<T>::get(registration.source, actor.actor_id)
            != Some(registration.handle)
        {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        let asset = DependencySourceBalances::<T>::get(registration.source)
          .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
        if BalanceDependencySources::<T>::get(asset) != Some(registration.source)
          || !episode.watches.iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the episode. */)
            .any(|watch| watch.asset == asset)
        {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        Self::dependency_registration_position(
          registration.source,
          actor.actor_id,
          registration.handle,
        )?;
      }
      Ok(plan)
    }

    /// Releases every exact reverse owner of one Park residence without publishing a successor.
    /// Contract replacement uses this only inside its enclosing transaction before rotating the
    /// generation, so any stale registration or deadline refuses the whole replacement.
    pub(crate) fn release_parked_dependency_authority(
      actor: ActorRef,
      evidence: ParkEvidence<BlockNumberFor<T>>,
    ) -> Result<(), DependencyRegistrationError> {
      let process = ActorProcesses::<T>::get(actor.actor_id)
        .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
      let owner = PendingCheckOwners::<T>::get(actor.actor_id)
        .filter(|owner| owner.actor == actor)
        .ok_or(DependencyRegistrationError::PendingOwnerMismatch)?;
      if process.generation != actor.generation
        || process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Parked(evidence))
      {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      let plan = Self::validate_parked_dependency_plan(actor, owner)?;
      if DeadlineHandles::<T>::contains_key(actor.actor_id) {
        Self::remove_deadline_member(actor)
          .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
      }
      for registration in &plan {
        Self::remove_dependency_registration(registration.source, registration.handle)?;
      }
      DependencyPlans::<T>::remove(actor.actor_id);
      DependencyTimedReviews::<T>::remove(actor.actor_id);
      PendingDependencyEvents::<T>::remove(actor.actor_id);
      PendingDependencyReviews::<T>::remove(actor.actor_id);
      PendingCheckOwners::<T>::remove(actor.actor_id);
      ParkedBalanceEpisodes::<T>::remove(actor.actor_id);
      Ok(())
    }

    /// Balance qualification owns the paid B+1 readiness transition. The enclosing transaction
    /// restores the exact Pending check and Park episode on payment or publication refusal.
    fn publish_positive_dependency_readiness(
      actor: ActorRef,
      owner: PendingCheckOwner,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      if PendingCheckOwners::<T>::get(actor.actor_id) != Some(owner)
        || !ActorProcesses::<T>::get(actor.actor_id).is_some_and(|process| {
          process.generation == actor.generation
            && process.status == ProcessStatus::Serving
            && process.residence == Some(ProcessResidence::Parked(evidence))
        })
      {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor.actor_id)
      else {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      };
      if record.generation != actor.generation || owner.actor != actor {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      let (state, _, _) = Self::load_actor_service_state_with_control(
        actor.actor_id,
        record.identity,
        record.hot,
        record.admission,
      )
      .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
      if state.contract.parked_balance_activation.is_none() {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      let actor_type = state.identity.actor_class.actor_type();
      let sovereign = state.identity.sovereign_account.clone();
      let breakdown = Self::trigger_fee_for_actor(
        actor.actor_id,
        actor_type,
        state.contract.on_unfunded,
        TriggerFamily::ParkedBalance,
        Self::dependency_review_weight_upper(),
      )
      .map_err(|error| DependencyRegistrationError::OccurrenceFailed(error.into()))?;
      Self::commit_canonical_trigger_occurrence_with_authority(
        actor, actor_type, &sovereign, breakdown, state, now,
      )
      .map_err(DependencyRegistrationError::OccurrenceFailed)?;
      Ok(())
    }

    /// Atomically wakes one exact generation/plan-bound Park resident into canonical Service.
    /// Stale authority and occupied Pending work refuse without consuming the retained plan.
    pub(crate) fn wake_parked_member_to_service(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      owner: PendingCheckOwner,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      if owner.actor != actor {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          if PendingCheckOwners::<T>::get(actor.actor_id) != Some(owner)
            || PendingDependencyEvents::<T>::contains_key(actor.actor_id)
            || PendingDependencyReviews::<T>::contains_key(actor.actor_id)
          {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let plan = Self::validate_parked_dependency_plan(actor, owner)?;
          let review = DependencyTimedReviews::<T>::get(actor.actor_id);
          if review.as_ref().is_some_and(|review| review.owner != owner) {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          if let Some(handle) = DeadlineHandles::<T>::get(actor.actor_id) {
            let expected_deadline = review
              .as_ref()
              .map(|review| review.deadline)
              .or_else(|| evidence.review_at.map(WakeupKey::Block));
            if handle.actor != actor || Some(handle.key) != expected_deadline {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            Self::remove_deadline_member(actor)
              .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
          }
          DependencyTimedReviews::<T>::remove(actor.actor_id);
          for registration in &plan {
            Self::remove_dependency_registration(registration.source, registration.handle)?;
          }
          DependencyPlans::<T>::remove(actor.actor_id);
          PendingCheckOwners::<T>::remove(actor.actor_id);
          ParkedBalanceEpisodes::<T>::remove(actor.actor_id);
          process.residence = Some(ProcessResidence::Service(kind));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_service_member(actor, kind, now)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Maximum reachable nonempty C32 pages in one deadline bucket.
    pub fn deadline_destination_page_bound() -> u32 {
      let actors = T::MaxActiveActors::get();
      actors / 32 + u32::from(actors % 32 != 0)
    }

    /// Selects the first canonical free slot in one deadline bucket without mutation.
    pub(crate) fn plan_deadline_destination(
      actor: ActorRef,
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      let Some(header) = DeadlineHeaders::<T>::get(key) else {
        return Ok(DeadlineHandle {
          actor,
          key,
          page: 0,
          slot: 0,
        });
      };
      Self::validate_deadline_header(&header)?;
      let Some(page_id) = header.first_vacant_page else {
        return Ok(DeadlineHandle {
          actor,
          key,
          page: header.next_page,
          slot: 0,
        });
      };
      let page =
        DeadlinePages::<T>::get(key, page_id).ok_or(DeadlineMutationError::CorruptCarrier)?;
      if page.entries.len() != 32
        || page.live_entries == 0
        || page.live_entries >= 32
        || page.previous_vacant_page.is_some()
      {
        return Err(DeadlineMutationError::CorruptCarrier);
      }
      let slot = page
        .entries
        .iter(/* deos-bypass: bounded-iter — one fixed C32 deadline page. */)
        .position(Option::is_none)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      Ok(DeadlineHandle {
        actor,
        key,
        page: page_id,
        slot: u8::try_from(slot).map_err(|_| DeadlineMutationError::CorruptCarrier)?,
      })
    }

    /// Atomically transfers one exact canonical Service member into a preselected deadline slot.
    /// The caller must commit semantic retry state first in the same outer transaction.
    pub(crate) fn transfer_service_member_to_deadline(
      actor: ActorRef,
      destination: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      if destination.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::remove_service_member(actor)
            .map_err(|_| DeadlineMutationError::ProcessResidenceMismatch)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          if process.generation != actor.generation || process.status != ProcessStatus::Serving {
            return Err(DeadlineMutationError::StaleGeneration);
          }
          process.residence = Some(ProcessResidence::Deadline {
            key: destination.key,
            page: destination.page,
            slot: destination.slot,
          });
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(destination)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Returns one genuinely due deadline member to canonical Service exactly once.
    #[allow(
      dead_code,
      reason = "canonical deadline extraction remains staged behind the atomic service cutover"
    )]
    pub(crate) fn return_due_deadline_member_to_service(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), DeadlineMutationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let handle = DeadlineHandles::<T>::get(actor.actor_id)
            .filter(|handle| handle.actor == actor)
            .ok_or(DeadlineMutationError::MemberMissing)?;
          let due = matches!(handle.key, WakeupKey::Block(block) if block <= now);
          if !due {
            return Err(DeadlineMutationError::InvalidDestination);
          }
          let process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          if process.residence
            != Some(ProcessResidence::Deadline {
              key: handle.key,
              page: handle.page,
              slot: handle.slot,
            })
          {
            return Err(DeadlineMutationError::ProcessResidenceMismatch);
          }
          Self::remove_deadline_member(actor)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          process.residence = Some(ProcessResidence::Service(kind));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_service_member(actor, kind, now)
            .map_err(|_| DeadlineMutationError::ProcessResidenceMismatch)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Extracts the canonical head member of the earliest due block bucket.
    #[allow(
      dead_code,
      reason = "canonical deadline traversal remains staged behind the atomic service cutover"
    )]
    pub(crate) fn return_next_due_block_deadline_to_service(
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<ActorRef, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Block, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Block(block) if block <= now) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let mut slot = 0usize;
      let actor = loop {
        if slot >= 32 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        if let Some(actor) = page.entries[slot] {
          break actor;
        }
        slot = slot
          .checked_add(1)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
      };
      Self::return_due_deadline_member_to_service(actor, kind, now)?;
      Ok(actor)
    }

    /// Stores identity and Hot directly through the canonical semantic owner. Canonical service
    /// mutation never recreates or updates a legacy control cell and fails closed on stale
    /// residence authority. A newly opened Run promotes Pending to Live in place without
    /// changing ring position or eligibility. Semantic, residence and retained-hold changes
    /// commit together, including the detector installed by temporal Opening rearm.
    pub(crate) fn try_store_service_control_state(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let corrupt = crate::scheduler::EnqueueOutcome::CorruptedTopology;
          let current =
            Self::load_service_actor_semantic_state(actor, kind).map_err(|_| corrupt)?;
          let promoted =
            if kind == ServiceResidenceKind::Pending && hot.cycle_state != CycleState::Idle {
              let mut process = ActorProcesses::<T>::get(actor.actor_id).ok_or(corrupt)?;
              let mut node = ServiceNodes::<T>::get(actor.actor_id).ok_or(corrupt)?;
              process.residence = Some(ProcessResidence::Service(ServiceResidenceKind::Live));
              node.kind = ServiceResidenceKind::Live;
              Some((process, node))
            } else {
              None
            };
          let mut replacement = current.clone();
          replacement.identity = identity;
          replacement.hot = hot;
          Self::mutate_actor_semantic_state(
            actor.actor_id,
            ActorSemanticMutation::Replace {
              expected: ActorSemanticState::Active(current),
              replacement: ActorSemanticState::Active(replacement.clone()),
            },
          )
          .map_err(|_| corrupt)?;
          let successor_kind = if let Some((process, node)) = promoted {
            ActorProcesses::<T>::insert(actor.actor_id, process);
            ServiceNodes::<T>::insert(actor.actor_id, node);
            ServiceResidenceKind::Live
          } else {
            kind
          };
          if replacement.identity.actor_class.actor_type() == ActorType::User {
            // Contract and maximum Run geometry are unchanged; do not reload unreached tails.
            let existing = ActorStateHolds::<T>::get(actor.actor_id).ok_or(corrupt)?;
            let mut target = existing.breakdown;
            target.detector = Self::state_hold_component(
              Self::state_hold_detector_bytes(&replacement.hot).map_err(|_| corrupt)?,
            )
            .map_err(|_| corrupt)?;
            Self::apply_actor_state_hold(
              actor.actor_id,
              Some(existing),
              replacement.identity.owner.clone(),
              target,
            )
            .map_err(|_| corrupt)?;
          }
          matches!(
            Self::load_service_actor_semantic_state(actor, successor_kind),
            Ok(stored) if stored == replacement
          )
          .then_some(())
          .ok_or(corrupt)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    #[allow(
      dead_code,
      reason = "Hot-only canonical mutation remains a focused seam for carrier validation"
    )]
    pub(crate) fn try_store_service_control_hot(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      let identity = Self::load_service_actor_semantic_state(actor, kind)
        .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology)?
        .identity;
      Self::try_store_service_control_state(actor, kind, identity, hot)
    }

    /// In-place Hot mutation resolves against whichever authority owns the active record: a live
    /// legacy primary keeps its physical mirror, while a canonically published Actor mutates only
    /// the semantic owner and fails closed on any leftover unsignaled cell. Moving transitions
    /// publish a supplied successor instead of using this in-place seam.
    pub(crate) fn try_store_control_hot_with_authority(
      actor_id: ActorId,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      if !ActorControlLocators::<T>::contains_key(actor_id) {
        if ActorUnsignaledControlCells::<T>::contains_key(actor_id) {
          return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
        }
        let current = ActorSemanticStates::<T>::get(actor_id)
          .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
        let ActorSemanticState::Active(mut record) = current.clone() else {
          return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
        };
        record.hot = hot;
        return Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected: current,
            replacement: ActorSemanticState::Active(record),
          },
        )
        .map(|_| ())
        .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology);
      }
      let current = ActorSemanticStates::<T>::get(actor_id)
        .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      };
      record.hot = hot.clone();
      Self::update_existing_frame_control_hot(actor_id, &hot)?;
      matches!(
        ActorSemanticStates::<T>::get(actor_id),
        Some(ActorSemanticState::Active(stored)) if stored == record
      )
      .then_some(())
      .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    }

    /// Mutates the physical primary without introducing a second hot-state owner.
    pub(crate) fn try_mutate_control_hot<R>(
      actor_id: ActorId,
      missing: Error<T>,
      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(missing.into());
      };
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      let output = mutate(&mut record.hot)?;
      Self::try_store_control_hot_with_authority(actor_id, record.hot)
        .map_err(|_| Error::<T>::ActorInvariant)?;
      Ok(output)
    }

    #[cfg(all(test, not(feature = "runtime-benchmarks")))]
    pub(crate) fn mutate_control_hot_or<R>(
      actor_id: ActorId,
      fallback: R,
      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> R,
    ) -> R {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return fallback;
      };
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return fallback;
      };
      let output = mutate(&mut record.hot);
      match Self::try_store_control_hot_with_authority(actor_id, record.hot) {
        Ok(()) => output,
        Err(_) => fallback,
      }
    }

    pub(crate) fn try_mutate_control_hot_with_authority<R>(
      actor_id: ActorId,
      missing: Error<T>,

      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      Self::try_mutate_control_hot(actor_id, missing, mutate)
    }

    /// Identity mutation follows its active-primary or dormant-registry owner.
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn try_mutate_control_identity<R>(
      actor_id: ActorId,
      missing: Error<T>,
      mutate: impl FnOnce(&mut ActorIdentityOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      let current = ActorSemanticStates::<T>::get(actor_id).ok_or(missing)?;
      match current.clone() {
        ActorSemanticState::Active(mut record) => {
          ensure!(
            !ActorControlLocators::<T>::contains_key(actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
            Error::<T>::ActorInvariant
          );
          let output = mutate(&mut record.identity)?;
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Replace {
              expected: current,
              replacement: ActorSemanticState::Active(record),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Ok(output)
        }
        ActorSemanticState::Dormant(mut record) => {
          ensure!(
            !ActorControlLocators::<T>::contains_key(actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
            Error::<T>::ActorInvariant
          );
          let output = mutate(&mut record.identity)?;
          ActorIdentities::<T>::insert(actor_id, &record.identity);
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Replace {
              expected: current,
              replacement: ActorSemanticState::Dormant(record),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Ok(output)
        }
      }
    }

    /// Admission is carried by the sole active primary.
    pub(crate) fn control_admission_exists(actor_id: ActorId) -> bool {
      Self::load_control_admission(actor_id).is_some()
    }

    pub(crate) fn load_control_admission(
      actor_id: ActorId,
    ) -> Option<ActorAdmissionCertificateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) => Some(record.admission),
        ActorSemanticState::Dormant(_) => None,
      }
    }

    /// Replaces admission and current-Step resources in the existing primary. Source-consumed
    /// transitions must carry admission explicitly into their destination publication.
    pub(crate) fn replace_control_admission_for_transition(
      actor_id: ActorId,
      certificate: &ActorAdmissionCertificateOf<T>,
      contract: &ActorContractOf<T>,
    ) -> bool {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return false;
      };
      let ActorSemanticState::Active(mut semantic_record) = current.clone() else {
        return false;
      };
      let Ok((location, mut cell)) = Self::load_primary_control_cell(actor_id) else {
        return false;
      };
      if semantic_record.admission != cell.admission {
        return false;
      };
      let Some(resources) = Self::derive_step_resource_envelopes(contract).and_then(|resources| {
        if contract.steps.is_empty() {
          Self::zero_step_resource_envelope(contract)
        } else {
          resources.get(cell.cursor as usize).copied()
        }
      }) else {
        return false;
      };
      let pointers = [
        cell.hot.wakeup_pointer,
        cell
          .hot
          .trigger_wakeup_pointer
          .map(|pointer| WakeupPointer {
            block: WakeupKey::Tick(pointer.tick),
            page_id: pointer.page_id,
            slot: pointer.slot,
          }),
      ];
      let mut reference_updates = Vec::with_capacity(2);
      for pointer in pointers.into_iter().flatten() {
        let key = (pointer.block, pointer.page_id);
        let Some(mut page) = ActorWaitingFrameChunks::<T>::get(key) else {
          return false;
        };
        match page
          .entries
          .get_mut(pointer.slot as usize)
          .and_then(Option::as_mut)
        {
          Some(ActorWaitingEntry::Reference(reference))
            if reference.actor_id == actor_id
              && reference.admission_identity == cell.admission.admission_identity =>
          {
            reference.admission_identity = certificate.admission_identity;
            reference_updates.push((key, page));
          }
          Some(ActorWaitingEntry::Primary(primary))
            if primary.actor_id == actor_id
              && primary.admission.admission_identity == cell.admission.admission_identity
              && location
                == (ActorControlLocation::Waiting {
                  key: pointer.block,
                  page: pointer.page_id,
                  slot: pointer.slot as u8,
                }) => {}
          _ => return false,
        }
      }
      cell.pipeline_service_identity = pipeline_service_identity(certificate.admission_identity);
      cell.admission = certificate.clone();
      cell.resources = resources;
      if Self::store_primary_control_cell(location, cell).is_err() {
        return false;
      }
      for (key, page) in reference_updates {
        ActorWaitingFrameChunks::<T>::insert(key, page);
      }
      let Some(next_generation) = next_actor_generation(semantic_record.generation) else {
        return false;
      };
      semantic_record.admission = certificate.clone();
      let Some(current_semantics @ ActorSemanticState::Active(_)) =
        ActorSemanticStates::<T>::get(actor_id)
      else {
        return false;
      };
      if current_semantics != ActorSemanticState::Active(semantic_record.clone()) {
        return false;
      }
      semantic_record.generation = next_generation;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: current_semantics,
          replacement: ActorSemanticState::Active(semantic_record),
        },
      )
      .is_ok()
    }

    pub(crate) fn mutate_actor_semantic_state(
      actor_id: ActorId,
      mutation: ActorSemanticMutation<ActorSemanticStateOf<T>>,
    ) -> Result<Option<ActorSemanticStateOf<T>>, ActorSemanticMutationError> {
      let current = ActorSemanticStates::<T>::get(actor_id);
      let replacement = apply_actor_semantic_mutation(current.as_ref(), &mutation)?;
      // Equal replacements still require the fresh read and complete expected-value validation.
      if replacement != current {
        match &replacement {
          Some(state) => ActorSemanticStates::<T>::insert(actor_id, state),
          None => ActorSemanticStates::<T>::remove(actor_id),
        }
      }
      Ok(replacement)
    }

    /// Loads the canonical semantic owner together with its independently derived placement.
    pub(crate) fn load_actor_semantic_state(
      actor_id: ActorId,
    ) -> Result<
      Option<(
        ActorSemanticStateOf<T>,
        Option<ActorControlLocation<BlockNumberFor<T>>>,
      )>,
      ActorSemanticLoadError,
    > {
      let state = ActorSemanticStates::<T>::get(actor_id);
      let location = ActorControlLocators::<T>::get(actor_id);
      let dormant_identity = ActorIdentities::<T>::get(actor_id);
      let has_contract_or_run = ActorContractHeads::<T>::contains_key(actor_id)
        || ActorRunStateStore::<T>::contains_key(actor_id);
      let has_legacy_authority =
        location.is_some() || ActorUnsignaledControlCells::<T>::contains_key(actor_id);
      match (&state, dormant_identity.as_ref()) {
        (Some(ActorSemanticState::Dormant(record)), Some(stored_identity))
          if &record.identity == stored_identity
            && !has_contract_or_run
            && !has_legacy_authority
            && !ActorProcesses::<T>::contains_key(actor_id) => {}
        (Some(ActorSemanticState::Active(record)), None)
          if !has_legacy_authority
            && has_contract_or_run
            && Self::load_canonical_actor_semantic_state(ActorRef {
              actor_id,
              generation: record.generation,
            })
            .is_ok() => {}
        (None, None)
          if !has_contract_or_run
            && !has_legacy_authority
            && !ActorProcesses::<T>::contains_key(actor_id) => {}
        _ => return Err(ActorSemanticLoadError::Corrupt),
      }
      Ok(state.map(|state| (state, None)))
    }

    pub(crate) fn load_actor_state_with_admission(
      actor_id: ActorId,
    ) -> (
      LoadedActorStateOf<T>,
      Option<ActorAdmissionCertificateOf<T>>,
    ) {
      match Self::load_actor_semantic_state(actor_id) {
        Ok(None) => (LoadedActorStateOf::NotRegistered, None),
        Ok(Some((ActorSemanticState::Dormant(record), None))) => {
          (LoadedActorStateOf::Dormant(record.identity), None)
        }
        Ok(Some((ActorSemanticState::Active(record), None))) => {
          let admission = record.admission.clone();
          let state = Self::load_active_actor_state(actor_id, record);
          match state {
            LoadedActorStateOf::Active(_) => (state, Some(admission)),
            _ => (state, None),
          }
        }
        Ok(Some(_)) | Err(_) => (LoadedActorStateOf::Corrupt, None),
      }
    }

    pub(crate) fn load_actor_state(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state_with_admission(actor_id).0
    }

    /// Strict active-state loader; malformed primary authority never falls back to dormancy.
    #[allow(
      dead_code,
      reason = "current-state consumer cutover is staged behind the atomic service writer"
    )]
    pub(crate) fn load_frame_actor_state(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state(actor_id)
    }

    fn load_active_actor_state(
      actor_id: ActorId,
      record: ActorSemanticRecordOf<T>,
    ) -> LoadedActorStateOf<T> {
      let ActorSemanticRecord {
        identity,
        generation,
        hot,
        admission: frame_admission,
      } = record;
      if Self::load_canonical_actor_semantic_state(ActorRef {
        actor_id,
        generation,
      })
      .is_err()
      {
        return LoadedActorStateOf::Corrupt;
      }
      let Some(contract) = Self::load_contract_geometry_with_admission(actor_id, &frame_admission)
      else {
        return LoadedActorStateOf::Corrupt;
      };
      if !Self::admission_authorizes_contract_wake(&frame_admission, &contract) {
        return LoadedActorStateOf::Corrupt;
      }
      let run_state = ActorRunStateStore::<T>::get(actor_id);
      let run_is_coherent = match (hot.cycle_state, run_state.as_ref()) {
        (CycleState::Idle, None) => true,
        (CycleState::Running, Some(run)) => {
          run.running_is_coherent()
            && run.has_contract_authority(
              frame_admission.semantic_contract_id,
              frame_admission.body_commitment,
              frame_admission.admission_identity,
            )
        }
        (CycleState::Suspended, Some(run)) => {
          run.suspension_is_coherent()
            && run.has_contract_authority(
              frame_admission.semantic_contract_id,
              frame_admission.body_commitment,
              frame_admission.admission_identity,
            )
        }
        _ => false,
      };
      if !run_is_coherent
        || !hot
          .trigger_runtime_state
          .is_compatible_with(&contract.trigger)
      {
        return LoadedActorStateOf::Corrupt;
      }
      LoadedActorStateOf::Active(ActiveActorState {
        identity,
        hot,
        contract,
        run_state,
      })
    }

    pub(crate) fn admission_authorizes_contract_wake(
      admission: &ActorAdmissionCertificateOf<T>,
      contract: &ActorContractOf<T>,
    ) -> bool {
      admission.authorizes_wake(contract.trigger.wake_qualification(&contract.window))
    }

    pub(crate) fn load_actor_service_state_with_control(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      Self::load_actor_service_state_with_head(actor_id, identity, hot, admission, head)
    }

    pub(crate) fn load_actor_service_state_with_head(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      head: ActorContractHeadOf<T>,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let geometry = Self::load_actor_service_geometry(actor_id, &hot, &admission, &head)?;
      Self::load_actor_service_state_from_geometry(
        actor_id, identity, hot, admission, head, geometry,
      )
    }

    fn load_actor_service_geometry(
      actor_id: ActorId,
      hot: &ActorHotStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      head: &ActorContractHeadOf<T>,
    ) -> Option<(Option<ActorRunStateOf<T>>, Option<LoadedActorStepOf<T>>)> {
      let run_state = ActorRunStateStore::<T>::get(actor_id);
      let cursor = match (hot.cycle_state, run_state.as_ref()) {
        (CycleState::Idle, None) => 0,
        (CycleState::Running, Some(run))
          if run.running_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          run.cursor
        }
        (CycleState::Suspended, Some(run))
          if run.suspension_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          run.cursor
        }
        _ => return None,
      };
      let tail_chunk = if cursor == 0 {
        None
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        Some((
          chunk_index,
          ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
        ))
      };
      let loaded_step = if head.header.step_count == 0 {
        if cursor != 0
          || tail_chunk.is_some()
          || head.first_step.is_some()
          || head.first_step_resources.is_some()
        {
          return None;
        }
        None
      } else {
        Some(Self::load_current_step_geometry(
          actor_id,
          head,
          admission,
          cursor,
          tail_chunk
            .as_ref()
            .map(|(chunk_index, chunk)| (*chunk_index, chunk)),
        )?)
      };
      Some((run_state, loaded_step))
    }

    fn load_actor_service_state_from_geometry(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      head: ActorContractHeadOf<T>,
      geometry: (Option<ActorRunStateOf<T>>, Option<LoadedActorStepOf<T>>),
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      if !Self::financial_wait_geometry_qualified(head.header.on_unfunded)
        || (head.header.on_unfunded == OnUnfunded::AwaitFunding
          && T::AdmissionCertificateAuthority::financial_wait_service_weight(
            Self::financial_wait_episode_bytes()?,
          )
          .is_none())
      {
        return None;
      }
      let behavior = if head.header.has_close_when {
        Some(Self::load_service_behavior_policy_with_price(
          actor_id,
          &head.header,
          &admission,
        )?)
      } else {
        None
      };
      let (run_state, loaded_step) = geometry;
      let behavior_context = match behavior.as_ref() {
        Some(policy) => Some(Self::behavior_control_weight_context(policy)?),
        None => None,
      };
      let loaded_step = match loaded_step {
        Some(loaded) => Some(Self::qualify_current_step_behavior(
          head.header.step_count,
          loaded,
          behavior_context,
        )?),
        None => {
          if let Some(context) = behavior_context {
            T::StepControlWeight::maximum_behavior_zero_step_control_weight(context)?;
          }
          None
        }
      };
      Self::materialize_actor_service_state(
        identity,
        hot,
        admission,
        head,
        (run_state, loaded_step),
        behavior.map(|policy| policy.expression),
      )
    }

    fn materialize_actor_service_state(
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      head: ActorContractHeadOf<T>,
      geometry: (Option<ActorRunStateOf<T>>, Option<LoadedActorStepOf<T>>),
      close_when: Option<CloseWhenOf<T>>,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let (run_state, loaded_step) = geometry;
      if !hot
        .trigger_runtime_state
        .is_compatible_with(&head.header.trigger)
      {
        return None;
      }
      let mut steps = ContractSteps::<T>::default();
      if let Some(loaded_step) = loaded_step.as_ref() {
        for _ in 0..head.header.step_count {
          steps.try_push(loaded_step.step.clone()).ok()?;
        }
      }
      let contract = ActorContract {
        trigger: head.header.trigger,
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        funding: head.header.funding,
        steps,
        close_when,
        on_unfunded: head.header.on_unfunded,
        parked_balance_activation: head.header.parked_balance_activation,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
      };
      Some((
        ActiveActorState {
          identity,
          hot,
          contract,
          run_state,
        },
        admission,
        loaded_step,
      ))
    }

    pub(crate) fn load_actor_service_state_with_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let (ActorSemanticState::Active(record), _) =
        Self::load_actor_semantic_state(actor_id).ok()??
      else {
        return None;
      };
      Self::load_actor_service_state_with_control(
        actor_id,
        record.identity,
        record.hot,
        record.admission,
      )
    }

    pub(crate) fn load_frame_actor_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      Self::load_actor_service_state_with_authority(actor_id)
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_current_step_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      LoadedActorStepOf<T>,
    )> {
      let (state, admission, loaded_step) = Self::load_frame_actor_service_state(actor_id)?;
      Some((state, admission, loaded_step?))
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn active_actor_view(actor_id: ActorId) -> Option<ActiveActorViewOf<T>> {
      let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
      else {
        return None;
      };
      Some(Self::derive_active_actor_view(
        state.identity,
        state.hot,
        state.contract,
      ))
    }

    pub(crate) fn load_actor_state_with_authority(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state(actor_id)
    }

    /// Lifecycle classification shares one strict canonical state boundary.
    pub(crate) fn load_actor_state_for_frame_control(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state_with_authority(actor_id)
    }

    pub(crate) fn active_actor_state_for_frame_control(
      actor_id: ActorId,
    ) -> Result<ActiveActorStateOf<T>, Error<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Ok(state),
        LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
          Err(Error::<T>::ActorNotFound)
        }
        LoadedActorStateOf::Corrupt => Err(Error::<T>::ActorInvariant),
      }
    }

    pub fn active_actor_state(actor_id: ActorId) -> Option<ActiveActorStateOf<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Some(state),
        _ => None,
      }
    }

    /// Projects independent chain flags without changing process, resource or lifecycle state.
    pub fn machine_control_flags() -> ActorMachineControlFlags {
      ActorMachineControlFlags {
        user_creation_enabled: UserCreationEnabled::<T>::get(),
        machine_frozen: GlobalCircuitBreaker::<T>::get(),
      }
    }

    /// Resolves identity from its lifecycle owner; malformed active authority never falls back
    /// to a dormant registry row.
    pub fn actor_identity(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Some(state.identity),
        LoadedActorStateOf::Dormant(identity) => Some(identity),
        LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Corrupt => None,
      }
    }

    /// Reads the bounded control owner independently of funding and Contract projections.
    pub fn actor_control_cell(
      actor_id: ActorId,
    ) -> Option<(
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    )> {
      Self::load_primary_control_cell(actor_id).ok()
    }

    /// Reads the active Hot state from its canonical semantic owner, falling back to the legacy
    /// primary only when no semantic record exists. Canonically published Actors keep Hot state in
    /// `ActorSemanticStates`; legacy Actors mirror it there while a primary remains.
    pub fn actor_hot(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id) {
        Some(ActorSemanticState::Active(record)) => Some(record.hot),
        Some(ActorSemanticState::Dormant(_)) => None,
        None => Self::load_frame_control_authority(actor_id).map(|(_, _, hot, _)| hot),
      }
    }

    pub fn pending_signal(actor_id: ActorId) -> bool {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot.pending_signal,
        _ => false,
      }
    }

    pub fn wakeup_buckets(block: BlockNumberFor<T>) -> Option<WakeupBucketState> {
      Self::wakeup_bucket_state(WakeupKey::Block(block))
    }

    pub(crate) fn wakeup_bucket_state(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Option<WakeupBucketState> {
      let live_entries = ActorWaitingOccupancies::<T>::get(key);
      if live_entries == 0 {
        return None;
      }
      let tail = ActorWaitingTails::<T>::get(key);
      Some(WakeupBucketState {
        head_page: ActorWaitingHeads::<T>::get(key) / 32,
        tail_page: tail.checked_sub(1)? / 32,
        next_page_id: tail.div_ceil(32),
        live_entries,
        cursor_index: ActorWaitingCursorIndices::<T>::get(key),
      })
    }

    pub fn wakeup_cursor_len() -> WakeupCursorIndex {
      WakeupCursorLen::<T>::get(WakeupClock::Block)
    }

    pub(crate) fn active_actor_exists(actor_id: ActorId) -> bool {
      matches!(
        Self::load_actor_state_for_frame_control(actor_id),
        LoadedActorStateOf::Active(_)
      )
    }

    pub(crate) fn insert_active_actor(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      contract: ActorContractOf<T>,
      intent: TriggerTransitionIntent,
    ) -> DispatchResult {
      let admission =
        Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Self::insert_active_actor_with_admission(actor_id, identity, hot, contract, intent, admission)
    }

    /// Installs the same Active transition from a caller-supplied, current host certificate.
    /// Public callers retain their independent policy admission gate and enclosing transaction.
    pub(crate) fn insert_active_actor_with_admission(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      mut hot: ActorHotStateOf<T>,
      contract: ActorContractOf<T>,
      intent: TriggerTransitionIntent,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> DispatchResult {
      ensure!(
        identity.actor_class.actor_type() != ActorType::System
          || contract.on_unfunded == OnUnfunded::Close,
        Error::<T>::AwaitFundingNotAllowedForSystemActor
      );
      Self::ensure_current_contract_admission(&contract, &admission)?;
      hot.trigger_runtime_state = Self::provisional_trigger_runtime_state(
        &contract.trigger,
        hot.trigger_runtime_state.temporal_anchor_tick(),
      );
      let step_resources = Self::derive_step_resource_envelopes(&contract)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let resources = match step_resources.first().copied() {
        Some(resources) => resources,
        None => {
          Self::zero_step_resource_envelope(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?
        }
      };
      let (generation, dormant_expected) = match intent {
        TriggerTransitionIntent::CreateActive | TriggerTransitionIntent::GenesisInstallation => {
          (1, None)
        }
        TriggerTransitionIntent::ActivateDormant => {
          let Some(ActorSemanticState::Dormant(record)) = ActorSemanticStates::<T>::get(actor_id)
          else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          ensure!(record.identity == identity, Error::<T>::ActorInvariant);
          let generation =
            next_actor_generation(record.generation).ok_or(Error::<T>::ActorInvariant)?;
          (generation, Some(record))
        }
      };
      let semantic_record = ActorSemanticRecord {
        identity: identity.clone(),
        generation,
        hot: hot.clone(),
        admission: admission.clone(),
      };
      Self::secure_at_time_installation_backing(
        &identity.owner,
        identity.actor_class.actor_type(),
        ActorRef {
          actor_id,
          generation,
        },
        &contract,
      )?;
      let semantic_mutation = match dormant_expected {
        None => ActorSemanticMutation::Publish(ActorSemanticState::Active(semantic_record)),
        Some(expected) => ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Dormant(expected),
          replacement: ActorSemanticState::Active(semantic_record),
        },
      };
      Self::mutate_actor_semantic_state(actor_id, semantic_mutation)
        .map_err(|_| Error::<T>::ActorInvariant)?;
      Self::store_actor_contract_with_admission(actor_id, contract.clone(), &admission)?;
      ActorIdentities::<T>::remove(actor_id);
      Self::publish_actor_publication(
        ActorRef {
          actor_id,
          generation,
        },
        &ActiveActorState {
          identity,
          hot,
          contract,
          run_state: None,
        },
        None,
        resources,
        frame_system::Pallet::<T>::block_number(),
        ServiceCutoff::Open,
      )
      .map_err(Self::placement_error)
    }

    fn provisional_trigger_runtime_state(
      trigger: &TriggerOf<T>,
      temporal_anchor_tick: Option<SchedulerTick>,
    ) -> TriggerRuntimeState {
      match trigger {
        Trigger::AtTime { .. } => TriggerRuntimeState::AtTime {
          anchor_tick: temporal_anchor_tick,
          consumed: false,
        },
        Trigger::Cadenced { .. } => TriggerRuntimeState::Cadenced {
          anchor_tick: temporal_anchor_tick,
        },
        Trigger::Manual | Trigger::AddressEvent { .. } => TriggerRuntimeState::Stateless,
      }
    }

    fn remove_active_actor_with_admission(
      actor_id: ActorId,
      admission: Option<&ActorAdmissionCertificateOf<T>>,
    ) -> DispatchResult {
      if ActorControlLocators::<T>::contains_key(actor_id) {
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| Error::<T>::ActorInvariant)?;
      }
      let removed_contract = match admission {
        Some(admission) => {
          Self::remove_admitted_contract_geometry_with_admission(actor_id, admission)
        }
        None => Self::remove_admitted_contract_geometry(actor_id),
      };
      ensure!(removed_contract.is_some(), Error::<T>::ActorInvariant);
      ActorRunStateStore::<T>::remove(actor_id);
      Ok(())
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn actor_identities)]
  pub type ActorIdentities<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorIdentityOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActorIdentities>,
  >;

  /// Canonical actor-keyed semantic authority. Physical service and deadline cells retain only
  /// placement data and must agree with the active/dormant partition represented here.
  #[pallet::storage]
  #[pallet::getter(fn actor_semantic_states)]
  pub type ActorSemanticStates<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorSemanticStateOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActorIdentities>,
  >;

  /// Canonical generation-bound process owner for status and primary residence.
  #[pallet::storage]
  #[pallet::getter(fn actor_processes)]
  pub type ActorProcesses<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorProcessOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  /// Canonical header for the actor-keyed persistent service ring.
  #[pallet::storage]
  #[pallet::getter(fn service_header)]
  pub type ServiceHeader<T: Config> =
    StorageValue<_, ServiceHeaderRecord<BlockNumberFor<T>>, ValueQuery>;

  /// Canonical generation-bound nodes for the persistent service ring.
  #[pallet::storage]
  #[pallet::getter(fn service_nodes)]
  pub type ServiceNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ServiceNode<BlockNumberFor<T>>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActiveActors>,
  >;

  /// Monotone scalar source allocator for exact watched-asset identities, never reused after close.
  #[pallet::storage]
  #[pallet::getter(fn dependency_source_allocator)]
  pub type DependencySourceAllocatorState<T: Config> =
    StorageValue<_, DependencySourceAllocator, ValueQuery>;

  /// Forward mapping from one watched asset to its collision-free causal source identity.
  #[pallet::storage]
  #[pallet::getter(fn balance_dependency_sources)]
  pub type BalanceDependencySources<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AssetId,
    DependencySourceId,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Reverse mapping proving watched-asset source identity ownership without a scan.
  #[pallet::storage]
  #[pallet::getter(fn dependency_source_balances)]
  pub type DependencySourceBalances<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    T::AssetId,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Checked revisions for event-complete dependency sources.
  #[pallet::storage]
  #[pallet::getter(fn dependency_revisions)]
  pub type DependencyRevisions<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    DependencyRevisionState,
    ValueQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Fair selector for sources that retain active dependency scans.
  #[pallet::storage]
  #[pallet::getter(fn dependency_scan_source_list)]
  pub type DependencyScanSourceListState<T> = StorageValue<_, DependencyScanSourceList, ValueQuery>;

  /// Exact circular-list membership for one active dependency source.
  #[pallet::storage]
  #[pallet::getter(fn dependency_scan_source_node)]
  pub type DependencyScanSourceNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    DependencyScanSourceNode,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// One-per-Actor activation-check ownership, bound to generation and plan revision.
  #[pallet::storage]
  #[pallet::getter(fn pending_check_owners)]
  pub type PendingCheckOwners<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, PendingCheckOwner, OptionQuery>;

  /// Source-owned fixed-width registration topology.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_headers)]
  pub type DependencyRegistrationHeaders<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    DependencyRegistrationHeader,
    ValueQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_pages)]
  pub type DependencyRegistrationPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    u64,
    DependencyRegistrationPage,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Bounded reusable holes; active scans defer reuse so their cursor cannot be retargeted.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_free_positions)]
  pub type DependencyRegistrationFreePositions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    u64,
    DependencyRegistrationPosition,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 31>,
  >;

  /// Exact stack offsets permit bounded removal of the holes in one empty C32 page.
  #[pallet::storage]
  pub type DependencyRegistrationFreeIndices<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    DependencyRegistrationPosition,
    u64,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 31>,
  >;

  /// Exact source/Actor positions into canonical registration pages.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_positions)]
  pub type DependencyRegistrationPositions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    ActorId,
    DependencyRegistrationPosition,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Exact source-to-Actor reverse registrations for dependency-keyed parking.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registrations)]
  pub type DependencyRegistrations<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    ActorId,
    DependencyRegistrationHandle,
    OptionQuery,
    GetDefault,
    DependencyStorageValues<T::MaxActiveActors, T::MaxParkedBalanceWatches, 1>,
  >;

  /// Actor-owned complete registration plan; this is the removal-completeness authority.
  #[pallet::storage]
  #[pallet::getter(fn dependency_plans)]
  pub type DependencyPlans<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    BoundedVec<DependencyPlanRegistration, T::MaxParkedBalanceWatches>,
    ValueQuery,
  >;

  /// One generation/plan-bound fixed-anchor episode retained only with a parked-balance plan.
  #[pallet::storage]
  #[pallet::getter(fn parked_balance_episodes)]
  pub type ParkedBalanceEpisodes<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ParkedBalanceEpisodeOf<T>, OptionQuery>;

  /// Actor-owned optional timed review retained with the complete dependency plan.
  #[pallet::storage]
  #[pallet::getter(fn dependency_timed_reviews)]
  pub type DependencyTimedReviews<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DependencyTimedReview<BlockNumberFor<T>>, OptionQuery>;

  /// Durable Pending destination for one event-complete source notification.
  #[pallet::storage]
  #[pallet::getter(fn pending_dependency_events)]
  pub type PendingDependencyEvents<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, PendingDependencyEvent, OptionQuery>;

  /// Durable Pending destination for one due dependency review.
  #[pallet::storage]
  #[pallet::getter(fn pending_dependency_reviews)]
  pub type PendingDependencyReviews<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DependencyTimedReview<BlockNumberFor<T>>, OptionQuery>;

  /// Bucket ownership for the canonical retained C32 Deadline carrier.
  #[pallet::storage]
  #[pallet::getter(fn deadline_headers)]
  pub type DeadlineHeaders<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, DeadlineHeader, OptionQuery>;

  /// Retained C32 pages for the canonical Deadline carrier.
  #[pallet::storage]
  #[pallet::getter(fn deadline_pages)]
  pub type DeadlinePages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    WakeupKey<BlockNumberFor<T>>,
    Blake2_128Concat,
    u64,
    DeadlinePage,
    OptionQuery,
  >;

  /// Generation-bound reverse handles for exact process or Park Deadline removal.
  ///
  /// Trigger deadlines use their own reverse owner because one Actor may simultaneously own a
  /// process residence and an independent temporal Trigger membership.
  #[pallet::storage]
  #[pallet::getter(fn deadline_handles)]
  pub type DeadlineHandles<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DeadlineHandleOf<T>, OptionQuery>;

  /// Generation-bound reverse handles for exact temporal Trigger Deadline removal.
  #[pallet::storage]
  #[pallet::getter(fn trigger_deadline_handles)]
  pub type TriggerDeadlineHandles<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DeadlineHandleOf<T>, OptionQuery>;

  /// C32 pages of the clock-local min-heaps over nonempty canonical Deadline buckets.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_pages)]
  pub type DeadlineIndexPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    WakeupClock,
    Blake2_128Concat,
    u64,
    DeadlineIndexPageOf<T>,
    OptionQuery,
  >;

  /// Exact reverse position for every key in the canonical deadline min-heaps.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_positions)]
  pub type DeadlineIndexPositions<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u32, OptionQuery>;

  /// Logical length of each inert clock-local canonical deadline min-heap.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_len)]
  pub type DeadlineIndexLen<T> = StorageMap<_, Blake2_128Concat, WakeupClock, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_identity_count)]
  pub type ActorIdentityCount<T> = StorageValue<_, u32, ValueQuery>;

  /// Per-Actor authority for the owner's aggregate dedicated Actor-state hold.
  #[pallet::storage]
  #[pallet::getter(fn actor_state_hold)]
  pub type ActorStateHolds<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    ActorStateHoldRecordOf<T>,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActorIdentities>,
  >;

  #[pallet::storage]
  #[pallet::getter(fn active_actor_count)]
  pub type ActiveActorCount<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn system_sovereigns)]
  pub type SystemSovereigns<T: Config> =
    StorageMap<_, Blake2_128Concat, SystemSovereignId, SystemSovereignState, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn system_sovereign_count)]
  pub type SystemSovereignCount<T> = StorageValue<_, u32, ValueQuery>;

  /// Ticket frontier frozen at block initialization before ordinary external causes.
  #[pallet::storage]
  #[pallet::getter(fn prepass_execution_cutoff)]
  pub type PrepassExecutionCutoff<T: Config> =
    StorageValue<_, (BlockNumberFor<T>, QueueTicket), OptionQuery>;

  /// Authoritative transient resource protocol state for the current block.
  #[pallet::storage]
  #[pallet::getter(fn block_resource_state)]
  pub type CurrentBlockResourceState<T: Config> =
    StorageValue<_, BlockResourceState<BlockNumberFor<T>>, OptionQuery>;

  /// Latest successfully reconciled block resource counters; read-only and non-authoritative.
  #[pallet::storage]
  #[pallet::getter(fn finalized_block_resource_telemetry)]
  pub type FinalizedBlockResourceTelemetry<T: Config> =
    StorageValue<_, FinalizedBlockResourceSnapshot<BlockNumberFor<T>>, OptionQuery>;

  /// Sole primary control cells for active Actors without current process readiness.
  #[pallet::storage]
  pub type ActorUnsignaledControlCells<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorControlCellOf<T>, OptionQuery>;

  /// Ticket-addressed C32 chunks for the one canonical Ready FIFO.
  #[pallet::storage]
  pub type ActorReadyFrameChunks<T: Config> =
    StorageMap<_, Blake2_128Concat, QueuePageId, ActorControlChunkOf<T>, OptionQuery>;

  /// Deadline-addressed C32 chunks for non-ready active control cells.
  #[pallet::storage]
  pub type ActorWaitingFrameChunks<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (WakeupKey<BlockNumberFor<T>>, QueuePageId),
    ActorWaitingPageOf<T>,
    OptionQuery,
  >;

  /// External-boundary locator for the sole primary active control cell.
  #[pallet::storage]
  pub type ActorControlLocators<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorControlLocation<BlockNumberFor<T>>, OptionQuery>;

  #[pallet::storage]
  pub type ActorReadyHead<T> = StorageValue<_, QueueTicket, ValueQuery>;

  #[pallet::storage]
  pub type ActorReadyTail<T> = StorageValue<_, QueueTicket, ValueQuery>;

  #[pallet::storage]
  pub type ActorReadyOccupancy<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingHeads<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingTails<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingOccupancies<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u32, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingCursorIndices<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, WakeupCursorIndex, OptionQuery>;

  /// Paged binary min-heaps of distinct block and timestamp-tick deadlines.
  #[pallet::storage]
  pub type WakeupCursorPages<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (WakeupClock, WakeupPageId),
    WakeupCursorPageOf<T>,
    OptionQuery,
  >;

  /// Logical length of each sparse-wakeup cursor heap.
  #[pallet::storage]
  pub type WakeupCursorLen<T> =
    StorageMap<_, Blake2_128Concat, WakeupClock, WakeupCursorIndex, ValueQuery>;

  /// Clock selected first when both temporal domains have due work.
  #[pallet::storage]
  pub type NextWakeupClock<T> = StorageValue<_, WakeupClock, ValueQuery>;

  pub type OwnerSlotBitmap = [u8; 32];

  #[pallet::storage]
  #[pallet::getter(fn owner_slot_bitmap)]
  pub type OwnerSlotBitmaps<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    OwnerSlotBitmap,
    ValueQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActorIdentities>,
  >;

  #[pallet::storage]
  #[pallet::getter(fn sovereign_index)]
  pub type SovereignIndex<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    ActorId,
    OptionQuery,
    GetDefault,
    StorageValueCountLimit<T::MaxActorIdentities>,
  >;

  /// Explicit nonzero governance-configurable active actor limit.
  #[pallet::storage]
  #[pallet::getter(fn configured_active_actor_limit)]
  pub type ActiveActorLimit<T: Config> = StorageValue<_, u32, ValueQuery>;

  /// New User identities fail closed unless the host explicitly admits them.
  #[pallet::storage]
  #[pallet::getter(fn user_creation_enabled)]
  pub type UserCreationEnabled<T> = StorageValue<_, bool, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn global_circuit_breaker)]
  pub type GlobalCircuitBreaker<T> = StorageValue<_, bool, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn idle_starvation_state)]
  pub type IdleStarvationState<T: Config> = StorageValue<_, IdleStarvationPhase, ValueQuery>;

  /// Provides runtime-specific System Actors instances to initialize at genesis.
  ///
  /// Implement this on the runtime to return System Actors specs with explicit `actor_id` values.
  /// IDs may be sparse to reserve stable addresses for non-actor accounts.
  pub trait GenesisSystemActors<AccountId, Contract> {
    fn system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability, Contract)>;

    fn dormant_system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability)> {
      alloc::vec::Vec::new()
    }

    /// Runtime-declared deterministic custody accounts that need a provider at genesis
    /// but own no generic Actors identity, contract, or scheduler state.
    fn system_custody_accounts() -> alloc::vec::Vec<ActorId> {
      alloc::vec::Vec::new()
    }

    /// Host-composition assertions that must remain true across runtime upgrades.
    fn integrity_test() {}
  }

  /// Default no-op implementation: no System Actors created at genesis.
  impl<AccountId, Contract> GenesisSystemActors<AccountId, Contract> for () {
    fn system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability, Contract)> {
      alloc::vec::Vec::new()
    }
  }

  #[pallet::genesis_config]
  #[derive(frame::prelude::DefaultNoBound)]
  pub struct GenesisConfig<T: Config> {
    pub user_creation_enabled: bool,
    #[serde(skip)]
    pub _marker: core::marker::PhantomData<T>,
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      assert!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        "MaxContractSteps must be in 1..=255"
      );
      STORAGE_VERSION.put::<Pallet<T>>();
      UserCreationEnabled::<T>::put(self.user_creation_enabled);
      if ActiveActorLimit::<T>::get() == 0 {
        ActiveActorLimit::<T>::put(Pallet::<T>::max_configurable_active_actor_limit());
      }
      for (actor_id, owner, mutability, mut contract) in T::GenesisSystemActors::system_actors() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id),
          "duplicate genesis System Actors id: {actor_id}"
        );
        let next_id = actor_id
          .checked_add(1)
          .expect("genesis Actors id must not overflow u64");
        if NextActorId::<T>::get() < next_id {
          NextActorId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System Actors sovereign collision at actor_id={actor_id}"
        );
        assert!(
          mutability == Mutability::Mutable || !contract.trigger.manual_source_enabled(),
          "genesis System Immutable Actors cannot admit Manual readiness"
        );
        Pallet::<T>::validate_trigger(&contract.trigger, contract.cooldown_blocks)
          .expect("genesis trigger and cooldown must be valid");
        if let Some(ref window) = contract.window {
          Pallet::<T>::validate_schedule_window(window)
            .expect("genesis execution window must be valid");
        }
        Pallet::<T>::validate_future_schedule_targets(&contract)
          .expect("genesis future schedule targets must be valid");
        if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
          Pallet::<T>::ensure_auto_close_target(0, target_nonce)
            .expect("genesis auto-close target must be nonzero");
        }
        Pallet::<T>::canonicalize_preconditions(&mut contract.steps)
          .expect("genesis precondition formulas must have valid bounded DNF");
        Pallet::<T>::validate_contract_steps_shape(ActorType::System, &contract.steps)
          .expect("genesis execution plan must have valid task and predicate shapes");
        Pallet::<T>::validate_unfunded_policy(ActorType::System, contract.on_unfunded)
          .expect("genesis System Actors cannot await fee funding"); // deos-bypass: panic-owner — genesis integrity rejects unsupported fee waiting before publication.
        assert!(
          contract.close_when.is_none(),
          "genesis behavior policy admission is unavailable"
        );
        Pallet::<T>::validate_parked_balance_activation(&contract)
          .expect("genesis parked-balance activation must be canonical"); // deos-bypass: panic-owner — genesis integrity validation fails closed before launch.
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .expect("genesis System Actor topology must be valid"); // deos-bypass: panic-owner — genesis integrity validation fails closed before launch.
        Pallet::<T>::validate_recipient_configuration(&contract.steps, &sovereign_account)
          .expect("genesis execution plan cannot transfer to its own sovereign account");
        Pallet::<T>::ensure_retry_later_allowed(mutability, &contract.steps)
          .expect("genesis System Immutable Actors cannot use RetryLater");
        Pallet::<T>::ensure_contract_fits_service_budget(ActorType::System, &contract)
          .unwrap_or_else(|_| {
            panic!("genesis System Actors {actor_id} exceeds the guaranteed on_idle budget")
          });
        let schedule_anchor = Pallet::<T>::schedule_anchor_at(contract.window, Zero::zero());
        // Genesis has no consensus timestamp. Temporal actors use `None` as a bounded bootstrap
        // marker and anchor from the first timestamp observed by ordinary wakeup service.
        let temporal_anchor_tick = None;
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: ActorClass::System {
            sovereign_id: actor_id,
          },
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: Zero::zero(),
        };
        let trigger_runtime_state =
          Pallet::<T>::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          trigger_runtime_state,
          unsuccessful_attempt_streak: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          trigger_wakeup_pointer: None,
          terminal_at: contract
            .window
            .map(|window| Pallet::<T>::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        let active_count = Pallet::<T>::active_instance_count();
        assert!(
          active_count < T::MaxActiveActors::get(),
          "genesis active actor capacity exceeded at actor_id={actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Occupied(actor_id));
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        Pallet::<T>::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::GenesisInstallation,
        )
        .unwrap_or_else(|error| panic!("genesis observation subscription failed: {error:?}")); // deos-bypass: panic-owner genesis construction fails before launch
        ActiveActorCount::<T>::put(
          active_count
            .checked_add(1)
            .expect("genesis active actor count must not overflow"),
        );
        ActorIdentityCount::<T>::put(
          ActorIdentityCount::<T>::get()
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
        assert!(
          ActorIdentityCount::<T>::get() <= T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at actor_id={actor_id}"
        );
      }
      for (actor_id, owner, mutability) in T::GenesisSystemActors::dormant_system_actors() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id)
            && !ActorIdentities::<T>::contains_key(actor_id),
          "duplicate genesis System Actors id: {actor_id}"
        );
        let next_id = actor_id
          .checked_add(1)
          .expect("genesis Actors id must not overflow u64");
        if NextActorId::<T>::get() < next_id {
          NextActorId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System Actors sovereign collision at actor_id={actor_id}"
        );
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner,
          actor_class: ActorClass::System {
            sovereign_id: actor_id,
          },
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: Zero::zero(),
        };
        let identity_count = ActorIdentityCount::<T>::get();
        assert!(
          identity_count < T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at actor_id={actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Occupied(actor_id));
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        assert!(
          Pallet::<T>::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Publish(ActorSemanticState::Dormant(
              DormantActorSemanticRecord {
                identity: identity.clone(),
                generation: 0,
              },
            )),
          )
          .is_ok(),
          "duplicate genesis dormant semantic state: {actor_id}"
        );
        ActorIdentities::<T>::insert(actor_id, identity);
        ActorIdentityCount::<T>::put(
          identity_count
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
      }
      for actor_id in T::GenesisSystemActors::system_custody_accounts() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id)
            && !ActorIdentities::<T>::contains_key(actor_id),
          "genesis custody account collides with actor identity: {actor_id}"
        );
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis custody account has generic sovereign index: {actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Vacant);
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
      }
    }
  }

  impl<T: Config> Pallet<T> {}

  pub(crate) struct PrepassEntry<BlockNumber> {
    pub(crate) state: BlockResourceState<BlockNumber>,
    pub(crate) budget: BlockResourceBudget,
    pub(crate) now_tick: SchedulerTick,
    pub(crate) control_envelopes: [Weight; 4],
  }

  impl<T: Config> Pallet<T> {
    pub(crate) fn begin_mandatory_prepass(
      now: BlockNumberFor<T>,
    ) -> Result<PrepassEntry<BlockNumberFor<T>>, Error<T>> {
      ensure!(
        T::PrepassContext::context_ready(),
        Error::<T>::PrepassContextIncomplete
      );
      let [
        control_weight,
        deadline_maximum,
        dependency_scan_envelope,
        drain_finalization_control,
      ] = Self::mandatory_actor_control_envelopes().ok_or(Error::<T>::ResourceProtocolFailed)?;
      if let Some(mut existing) = CurrentBlockResourceState::<T>::get()
        && existing.ensure_block(now).is_ok()
      {
        existing.halt_optional_actor_work();
        CurrentBlockResourceState::<T>::put(existing);
        return Err(Error::<T>::PrepassDuplicateOrStale);
      }

      let configured = T::BlockResourceBudget::get();
      let budget = T::BlockResourceBudget::for_prepass(now, configured)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      ensure!(
        budget.maximum_block() == configured.maximum_block()
          && budget.fixed_envelope().all_lte(configured.fixed_envelope())
          && budget
            .limits()
            .actor_control()
            .all_gte(configured.limits().actor_control())
          && budget
            .limits()
            .actor_base_turn()
            .all_gte(configured.limits().actor_base_turn())
          && budget
            .limits()
            .user_base_turn()
            .all_gte(configured.limits().user_base_turn()),
        Error::<T>::ResourceProtocolFailed
      );
      let mandatory_control =
        Self::mandatory_actor_control_weight_upper().ok_or(Error::<T>::ResourceProtocolFailed)?;
      ensure!(
        mandatory_control.all_lte(budget.limits().actor_control()),
        Error::<T>::ResourceProtocolFailed
      );
      let now_tick =
        Self::current_scheduler_tick().map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      let mut state = BlockResourceState::new(now);
      if state.begin_prepass(budget).is_err() {
        state.halt_optional_actor_work();
        CurrentBlockResourceState::<T>::put(state);
        return Err(Error::<T>::ResourceProtocolFailed);
      }
      let mut prepass_reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          control_weight,
        )
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      state
        .settle(&mut prepass_reservation, control_weight)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      Ok(PrepassEntry {
        state,
        budget,
        now_tick,
        control_envelopes: [
          control_weight,
          deadline_maximum,
          dependency_scan_envelope,
          drain_finalization_control,
        ],
      })
    }

    pub(crate) fn service_prepass_deadlines(
      now: BlockNumberFor<T>,
      now_tick: SchedulerTick,
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      budget: BlockResourceBudget,
      deadline_maximum: Weight,
    ) -> Result<Weight, Error<T>> {
      let mut deadline_reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          deadline_maximum,
        )
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      let mut deadline_meter = WeightMeter::with_limit(deadline_maximum);
      let _ = Self::service_due_deadline_frontiers(
        &mut deadline_meter,
        ServiceResidenceKind::Live,
        now,
        now_tick,
        now.checked_add(&One::one()).map(WakeupKey::Block),
      )
      .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      let deadline_actual = deadline_meter.consumed();
      state
        .settle(&mut deadline_reservation, deadline_actual)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      Ok(deadline_actual)
    }

    pub(crate) fn service_prepass_dependency_scan(
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      budget: BlockResourceBudget,
      dependency_scan_envelope: Weight,
    ) -> Result<Weight, Error<T>> {
      let mut dependency_scan_reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          dependency_scan_envelope,
        )
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      let mut dependency_scan_meter = WeightMeter::with_limit(dependency_scan_envelope);
      if Self::dependency_scan_source_probe(&mut dependency_scan_meter)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?
      {
        Self::process_next_dependency_scan_unit(&mut dependency_scan_meter)
          .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      }
      let dependency_scan_actual = dependency_scan_meter.consumed();
      state
        .settle(&mut dependency_scan_reservation, dependency_scan_actual)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      Ok(dependency_scan_actual)
    }

    fn execute_mandatory_prepass(now: BlockNumberFor<T>) -> Result<Weight, Error<T>> {
      let PrepassEntry {
        mut state,
        budget,
        now_tick,
        control_envelopes:
          [
            control_weight,
            deadline_maximum,
            dependency_scan_envelope,
            drain_finalization_control,
          ],
      } = Self::begin_mandatory_prepass(now)?;

      // Freeze preserves exact frontier and round authority while the block protocol continues.
      if GlobalCircuitBreaker::<T>::get() {
        state.halt_optional_actor_work();
        state
          .open_external_phase()
          .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
        CurrentBlockResourceState::<T>::put(state);
        return Ok(control_weight);
      }

      // Deadline authority precedes competing detector and Service work. Each clock gets one
      // admitted attempt; returned members retain the ordinary B+1 eligibility boundary.
      let deadline_actual =
        Self::service_prepass_deadlines(now, now_tick, &mut state, budget, deadline_maximum)?;

      let dependency_scan_actual =
        Self::service_prepass_dependency_scan(&mut state, budget, dependency_scan_envelope)?;

      let control_remaining = budget
        .limits()
        .actor_control()
        .checked_sub(&state.usage().actor_control_used())
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let prepass_control = control_remaining
        .checked_sub(&drain_finalization_control)
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let prepass_limit = prepass_control
        .checked_add(&budget.limits().actor_base_turn())
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let pass = Self::execute_cycle_to_cutoff_with_resources(
        prepass_limit,
        0,
        &mut state,
        budget.limits(),
        BlockResourceDomain::ActorBaseEffect,
        prepass_control,
      );
      if state.open_external_phase().is_err() {
        state.halt_optional_actor_work();
      }
      CurrentBlockResourceState::<T>::put(state);
      Ok(
        control_weight
          .saturating_add(deadline_actual)
          .saturating_add(dependency_scan_actual)
          .saturating_add(pass.consumed),
      )
    }
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn integrity_test() {
      assert!(
        T::MaxConsecutiveFailures::get() > 0,
        "MaxConsecutiveFailures must be non-zero for bounded Actor run lifetime"
      );
      assert!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        "MaxContractSteps must be in 1..=255"
      );
      assert_eq!(
        T::MaxRetryAttempts::get(),
        10,
        "MaxRetryAttempts must equal the protocol-fixed bound"
      );
      assert!(
        T::MaxContractSteps::get()
          .checked_mul(T::MaxRetryAttempts::get())
          .is_some(),
        "plan and retry bounds must compose without u32 overflow"
      );
      let target_block_time = T::TargetBlockTime::get();
      assert!(target_block_time > 0, "TargetBlockTime must be non-zero");
      assert!(
        T::CadenceTickMillis::get() > 0,
        "CadenceTickMillis must be non-zero"
      );
      let cadence_tick_millis = T::CadenceTickMillis::get();
      let expected_temporal_horizon = 315_576_000_000u64.div_ceil(cadence_tick_millis);
      assert_eq!(
        T::MaxTemporalDelayTicks::get(),
        expected_temporal_horizon,
        "MaxTemporalDelayTicks must cover exactly ten Julian years"
      );
      let expected_horizon = 315_576_000u64.div_ceil(target_block_time);
      let configured_horizon: u64 = T::MaxExecutionDelayBlocks::get().saturated_into();
      assert_eq!(
        configured_horizon, expected_horizon,
        "MaxExecutionDelayBlocks must cover exactly ten Julian years"
      );
      assert!(
        T::MinUserBalance::get() >= T::AssetOps::minimum_balance(T::FeeNativeAssetId::get()),
        "MinUserBalance must cover the fee-native asset minimum"
      );
      assert!(
        T::QueuePageSize::get() > 0,
        "QueuePageSize must be non-zero"
      );
      assert!(
        T::QueuePageSize::get() < T::MaxQueueLength::get(),
        "QueuePageSize must remain an intermediate I/O granularity"
      );
      assert!(
        T::MaxQueueEntriesScannedPerBlock::get() > 0
          && T::MaxQueueEntriesScannedPerBlock::get() <= T::MaxQueueLength::get(),
        "queue scan ceiling must be independently bounded by physical capacity"
      );
      let actor_service = Self::guaranteed_actor_service_weight()
        .expect("configured housekeeping Weight must fit ActorOnIdleReserve");
      assert!(
        Self::close_cleanup_weight_upper().all_lte(actor_service),
        "one maximum automatic cleanup must fit GuaranteedActorServiceWeight"
      );
      T::GenesisSystemActors::integrity_test();
    }

    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }

    fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
      Weight::zero()
    }

    fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let reserved = T::ActorOnIdleReserve::get();
      let available = Weight::from_parts(
        remaining_weight.ref_time().min(reserved.ref_time()),
        remaining_weight.proof_size().min(reserved.proof_size()),
      );
      let (control_available, resource_state) = match CurrentBlockResourceState::<T>::get() {
        Some(state)
          if state.ensure_block(now).is_ok()
            && state.phase() == BlockResourcePhase::ExternalPhase =>
        {
          (
            state
              .budget()
              .ok()
              .and_then(|budget| {
                budget
                  .limits()
                  .actor_control()
                  .checked_sub(&state.usage().actor_control_used())
              })
              .map(|remaining| {
                Weight::from_parts(
                  available.ref_time().min(remaining.ref_time()),
                  available.proof_size().min(remaining.proof_size()),
                )
              })
              .unwrap_or_else(Weight::zero),
            Some(state),
          )
        }
        Some(_) => (Weight::zero(), None),
        None => (available, None),
      };
      let base_weight = T::WeightInfo::scheduler_on_idle_base();
      let finalize_weight = T::WeightInfo::block_resource_finalize();
      let fixed_weight = base_weight.saturating_add(finalize_weight);
      if !fixed_weight.all_lte(control_available) {
        return Weight::zero();
      }
      let mut control_authority = match resource_state {
        Some(mut state) => match state.budget().and_then(|budget| {
          state.reserve_mandatory_actor_control(budget.limits(), control_available)
        }) {
          Ok(reservation) => Some((state, reservation)),
          Err(_) => {
            state.halt_optional_actor_work();
            CurrentBlockResourceState::<T>::put(state);
            return Weight::zero();
          }
        },
        None => None,
      };
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      let housekeeping_weight = fixed_weight;
      let remaining_after_housekeeping = available.saturating_sub(housekeeping_weight);
      Self::settle_on_idle_control(&mut control_authority, housekeeping_weight);
      let pass = match CurrentBlockResourceState::<T>::get() {
        Some(mut state)
          if state.ensure_block(now).is_ok()
            && state.phase() == BlockResourcePhase::ExternalPhase =>
        {
          let budget = match state.budget() {
            Ok(budget) => budget,
            Err(_) => {
              state.halt_optional_actor_work();
              CurrentBlockResourceState::<T>::put(state);
              return housekeeping_weight;
            }
          };
          if state.begin_drain().is_err() {
            state.halt_optional_actor_work();
            CurrentBlockResourceState::<T>::put(state);
            return housekeeping_weight;
          }
          let control_maximum = budget
            .limits()
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap_or_else(Weight::zero);
          let pass = if breaker_active || state.optional_actor_work_halted() {
            CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              // A halt carried from Prepass is still blocked service, not recovery. Breaker
              // deferral remains the independent observability freeze.
              starvation_observed: !breaker_active,
              starved: !breaker_active,
            }
          } else {
            Self::execute_cycle_to_cutoff_with_resources(
              remaining_after_housekeeping,
              0,
              &mut state,
              budget.limits(),
              BlockResourceDomain::ActorDrainEffect,
              control_maximum,
            )
          };
          if state.finish_drain().is_err() {
            state.halt_optional_actor_work();
          } else if let Ok(snapshot) = state.finalized_snapshot() {
            FinalizedBlockResourceTelemetry::<T>::put(snapshot);
          }
          CurrentBlockResourceState::<T>::put(state);
          pass
        }
        Some(mut state) => {
          state.halt_optional_actor_work();
          CurrentBlockResourceState::<T>::put(state);
          return housekeeping_weight;
        }
        None => {
          if breaker_active {
            CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              starvation_observed: false,
              starved: false,
            }
          } else {
            Self::execute_cycle_to_cutoff(remaining_after_housekeeping, 0)
          }
        }
      };
      if !breaker_active && pass.starvation_observed {
        Self::update_idle_starvation_state(now, pass.starved);
      }
      housekeeping_weight.saturating_add(pass.consumed)
    }

    fn on_finalize(now: BlockNumberFor<T>) {
      let valid = CurrentBlockResourceState::<T>::take().is_some_and(|state| {
        state.ensure_block(now).is_ok()
          && state.phase() == BlockResourcePhase::Finalizable
          && state.outstanding_reservations() == 0
      }) && FinalizedBlockResourceTelemetry::<T>::get()
        .is_some_and(|snapshot| snapshot.block_number() == now);
      assert!(valid, "Actors block resource protocol did not finalize"); // deos-bypass: panic-owner — invalid phase/reservation/telemetry makes the authored block consensus-invalid; finalization tests cover every accepted marker.
    }
  }

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    ActorCreated {
      actor_id: ActorId,
      owner: T::AccountId,
      actor_class: ActorClass,
      mutability: Mutability,
      sovereign_account: T::AccountId,
      initial_lifecycle: InitialLifecycle,
    },
    ActorActivated {
      actor_id: ActorId,
    },
    ActorDeactivated {
      actor_id: ActorId,
    },
    ActorPaused {
      actor_id: ActorId,
    },
    ActorResumed {
      actor_id: ActorId,
    },
    FinancialWaitEntered {
      receipt: FinancialWaitReceipt,
    },
    FinancialWaitRecovered {
      receipt: FinancialWaitReceipt,
    },
    ActorClosed {
      actor_id: ActorId,
      reason: CloseReason,
    },
    CycleStarted {
      actor_id: ActorId,
      cycle_nonce: u64,
    },
    CycleSummary {
      actor_id: ActorId,
      cycle_nonce: u64,
      result: CycleResult,
      outcomes: OutcomeTotals,
    },
    CycleSuspended {
      actor_id: ActorId,
      cycle_nonce: u64,
      cursor: u32,
      reason: SuspensionReason,
      cumulative_outcomes: OutcomeTotals,
    },
    CycleContinued {
      actor_id: ActorId,
      cycle_nonce: u64,
      cursor: u32,
    },
    CycleCancelled {
      actor_id: ActorId,
      cycle_nonce: u64,
      reason: CancellationReason,
    },
    CycleStopped {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
    },
    StepSkipped {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      reason: StepSkippedReason,
    },
    StepFailed {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      retry_class: RetryClass,
      error: DispatchError,
    },
    TransferExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
      to: T::AccountId,
    },
    SwapExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_in: T::AssetId,
      asset_out: T::AssetId,
      amount_in: T::Balance,
      amount_out: T::Balance,
    },
    BurnExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    MintExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    StakeExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    UnstakeExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      shares: T::Balance,
    },
    LiquidityDonated {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      max_amount_a: T::Balance,
      max_amount_b: T::Balance,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    LiquidityAdded {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
      lp_minted: T::Balance,
    },
    LiquidityRemoved {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      lp_asset: T::AssetId,
      lp_amount: T::Balance,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    ContractUpdated {
      actor_id: ActorId,
    },
    ActiveActorLimitSet {
      old_limit: u32,
      new_limit: u32,
    },
    UserCreationEnabledSet {
      enabled: bool,
    },
    GlobalCircuitBreakerSet {
      paused: bool,
    },
    ManualTriggerSet {
      actor_id: ActorId,
    },
    TriggerOccurrenceProcessed {
      actor_id: ActorId,
      trigger_family: TriggerFamily,
      fee: BalanceOf<T>,
    },
    PipelineFeeCharged {
      actor_id: ActorId,
      fee: BalanceOf<T>,
    },
    ActionFeeCharged {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      actual_effect_weight: Weight,
      fee: BalanceOf<T>,
    },
    SweepBatchProcessed {
      requested: u32,
      closed: u32,
      alive: u32,
      missing: u32,
    },
    IdleStarvationDetected {
      consecutive_blocks: u32,
    },
    IdleStarvationRecovered {
      consecutive_blocks: u32,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    ActorIdOverflow,
    ActorNotFound,
    ActiveActorCapacityExceeded,
    ActiveActorCountInvariant,
    ActorIdentityCapacityExceeded,
    ActorIdentityCountInvariant,
    ActorInvariant,
    ActorAlreadyActive,
    ActorDormant,
    ActiveActorLimitExceedsQueueCapacity,
    ActiveActorLimitTooHigh,
    ActiveActorLimitTooLow,
    ActiveActorLimitBelowCurrent,
    ActorPaused,
    ContractStepsExceedOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    UserCreationDisabled,
    AwaitFundingNotAllowedForSystemActor,
    AwaitFundingUnavailable,
    InvalidCloseWhen,
    BehaviorPolicyUnavailable,
    ImmutableActor,
    InsufficientBalance,
    InsufficientFee,
    TriggerFeeCollectionFailed,
    InvalidAmountResolution,
    InvalidParkedBalanceActivation,
    InvalidPredicate,
    InvalidAutoCloseNonce,
    InvalidScheduleWindow,
    InvalidTriggerConfiguration,
    InvalidTradeBound,
    InvalidRetryAttemptLimit,
    InvalidObservationMaxAge,
    SelfTransferNotAllowed,
    MintNotAllowedForUserActor,
    NotGovernance,
    NotOwner,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    InvalidOwnerSlot,
    ActorIdOccupied,
    SystemSovereignCapacityExceeded,
    SystemSovereignUnknown,
    SystemSovereignOccupied,
    SystemSovereignInvariant,
    SovereignAccountCollision,
    ReservedSovereignAccount,
    TooManyContractSteps,
    QueueTicketExhausted,
    SchedulerIndexExhausted,
    AutoCloseNonceHorizonExceeded,
    ControlMutationRateLimited,
    QueueCapacityUnavailable,
    RetryLaterNotAllowedForImmutableActor,
    ActorRunNotFound,
    ActorRunInvariant,
    ComputationOverflow,
    EmptyPrecondition,
    ManualSourceDisabled,
    RecipientDepositUnavailable,
    SystemActorTopologyInvalid,
    AdmissionBoundOverflow,
    StateHoldUnavailable,
    StateHoldInvariant,
    StateHoldOverflow,
    PrepassDuplicateOrStale,
    ResourceProtocolFailed,
    PrepassContextIncomplete,
  }

  impl<T: Config> Pallet<T> {}

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_user_actor())]
    pub fn create_user_actor(
      origin: OriginFor<T>,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, None, contract)
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::create_user_actor_at_slot())]
    pub fn create_user_actor_at_slot(
      origin: OriginFor<T>,
      owner_slot: u8,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, Some(owner_slot), contract)
    }

    #[pallet::call_index(2)]
    #[pallet::weight(if contract.is_some() {
      T::WeightInfo::create_system_actor()
    } else {
      T::WeightInfo::create_dormant_system_actor()
    })]
    pub fn create_system_actor(
      origin: OriginFor<T>,
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_actor(owner, mutability, contract, None)
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::create_system_actor_at_sovereign_id())]
    pub fn create_system_actor_at_sovereign_id(
      origin: OriginFor<T>,
      sovereign_id: SystemSovereignId,
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_actor(owner, mutability, contract, Some(sovereign_id))
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::pause_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn pause_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::ensure_system_pause_origin(origin, &snapshot)?;
      Self::ensure_machine_unfrozen()?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(actor_id, &snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if snapshot.lifecycle.is_paused() {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::with_control_transaction(|| {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        let resources = if state.contract.steps.is_empty() {
          Self::zero_step_resource_envelope(&state.contract).ok_or(Error::<T>::ActorInvariant)?
        } else {
          let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
          Self::derive_step_resource_envelopes(&state.contract)
            .and_then(|envelopes| envelopes.get(cursor as usize).copied())
            .ok_or(Error::<T>::ActorInvariant)?
        };
        let mut paused = state.clone();
        paused.identity.last_control_mutation_block = now;
        paused.hot.lifecycle = ActiveLifecycle::Paused;
        paused.hot.queue_ticket = None;
        Self::transition_actor_publication_to_successor(
          actor,
          &state,
          &paused,
          paused.run_state.as_ref(),
          resources,
          now,
          ServiceCutoff::Open,
        )
        .map_err(Self::placement_error)?;
        Self::deposit_event(Event::ActorPaused { actor_id });
        Ok(())
      })
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      ensure!(
        !ActorIdentities::<T>::contains_key(actor_id),
        Error::<T>::ActorInvariant
      );
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      ensure!(
        state.hot.cycle_state != CycleState::Idle
          || !ActorRunStateStore::<T>::contains_key(actor_id),
        Error::<T>::ActorInvariant
      );
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::ensure_system_pause_origin(origin, &snapshot)?;
      Self::ensure_machine_unfrozen()?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(actor_id, &snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if !snapshot.lifecycle.is_paused() {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::with_control_transaction(|| {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        Self::load_canonical_actor_semantic_state(actor).map_err(|_| Error::<T>::ActorInvariant)?;
        let resources = if state.contract.steps.is_empty() {
          Self::zero_step_resource_envelope(&state.contract).ok_or(Error::<T>::ActorInvariant)?
        } else {
          let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
          Self::derive_step_resource_envelopes(&state.contract)
            .and_then(|envelopes| envelopes.get(cursor as usize).copied())
            .ok_or(Error::<T>::ActorInvariant)?
        };
        let mut resumed = state.clone();
        resumed.identity.last_control_mutation_block = now;
        resumed.hot.lifecycle = ActiveLifecycle::Active;
        resumed.hot.queue_ticket = None;
        Self::transition_actor_publication_to_successor(
          actor,
          &state,
          &resumed,
          resumed.run_state.as_ref(),
          resources,
          now,
          ServiceCutoff::Open,
        )
        .map_err(Self::placement_error)?;
        // The successor publication must leave the canonical process for this generation in
        // place and must have cleared the paused lifecycle. A latched occurrence can leave the
        // process shape (`status`/`residence`) unchanged across resume, so the applied semantic
        // lifecycle -- not process inequality -- is the authoritative evidence that resume ran.
        ensure!(
          ActorProcesses::<T>::get(actor_id)
            .is_some_and(|successor| successor.generation == actor.generation)
            && ActorSemanticStates::<T>::get(actor_id).is_some_and(|state| {
              matches!(
                state,
                ActorSemanticState::Active(record) if !record.hot.lifecycle.is_paused()
              )
            }),
          Error::<T>::ActorInvariant
        );
        Self::deposit_event(Event::ActorResumed { actor_id });
        Ok(())
      })
    }

    #[pallet::call_index(6)]
    #[pallet::weight(
      T::WeightInfo::manual_trigger()
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper())
    )]
    pub fn manual_trigger(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResultWithPostInfo {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_machine_unfrozen()?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(actor_id, &snapshot, continuation.as_ref())? {
        Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired)?;
        return Ok(Pays::Yes.into());
      }
      ensure!(!snapshot.lifecycle.is_paused(), Error::<T>::ActorPaused);
      ensure!(
        snapshot.trigger.manual_source_enabled(),
        Error::<T>::ManualSourceDisabled
      );
      // A Manual occurrence while the current Cycle is open is intentionally ignored before
      // Trigger-fee admission: current-state service owns no deferred future-Cycle latch.
      // A duplicate occurrence that is already latched but not yet serviced is likewise a
      // coalescing no-op rather than an invariant failure: one useful false->true transition owns
      // the charged readiness and later occurrences neither re-charge nor create a second cycle.
      if snapshot.cycle_state != CycleState::Idle || snapshot.pending_signal {
        return Ok(().into());
      }
      let actor_type = snapshot.actor_class.actor_type();
      let occurrence_weight = T::WeightInfo::manual_trigger();
      let breakdown = Self::trigger_fee_for_actor(
        actor_id,
        actor_type,
        state.contract.on_unfunded,
        TriggerFamily::Manual,
        occurrence_weight,
      )?;
      let mut trigger_processed = false;
      Self::with_control_transaction(|| {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        let now = frame_system::Pallet::<T>::block_number();
        let outcome = Self::commit_canonical_trigger_occurrence_with_authority(
          actor,
          actor_type,
          &snapshot.sovereign_account,
          breakdown,
          state,
          now,
        )?;
        if matches!(outcome, crate::scheduler::ActivationOutcome::Latched) {
          Self::deposit_event(Event::ManualTriggerSet { actor_id });
        }
        trigger_processed = true;
        Ok(())
      })?;
      Ok(PostDispatchInfo {
        actual_weight: Some(occurrence_weight),
        pays_fee: if actor_type == ActorType::User && trigger_processed {
          Pays::No
        } else {
          Pays::Yes
        },
      })
    }

    #[pallet::call_index(8)]
    #[pallet::weight(Pallet::<T>::close_dispatch_weight_upper())]
    pub fn close_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let authority = Self::load_close_authority(origin, actor_id)?;
      Self::ensure_machine_unfrozen()?;
      match authority {
        CheckedCloseAuthorityOf::Active(instance) => {
          Self::finalize_actor(actor_id, &instance, CloseReason::OwnerInitiated)
        }
        CheckedCloseAuthorityOf::Dormant(identity) => {
          Self::close_inactive_actor(actor_id, &identity, CloseReason::OwnerInitiated)
        }
      }
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::update_contract().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_contract(
      origin: OriginFor<T>,
      actor_id: ActorId,
      mut contract: ActorContractOf<T>,
    ) -> DispatchResult {
      Self::ensure_machine_unfrozen()?;
      Self::canonicalize_preconditions(&mut contract.steps)?;
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let stored_cancellation_state = (state.contract.close_when.is_some()
        || state.contract.on_unfunded == OnUnfunded::AwaitFunding)
        .then(|| state.clone());
      let continuation = state.run_state;
      let current_contract = state.contract.clone();
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if current_contract == contract {
        if snapshot.actor_class.actor_type() == ActorType::System {
          Self::validate_unfunded_policy(ActorType::System, contract.on_unfunded)?;
        }
        return Ok(());
      }
      Self::validate_unfunded_policy(snapshot.actor_class.actor_type(), contract.on_unfunded)?;
      Self::ensure_retry_later_allowed(snapshot.mutability, &contract.steps)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      Self::validate_close_when(&contract)?;
      if Self::expiry_substitution_due_loaded(actor_id, &snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let schedule_changed = current_contract.trigger != contract.trigger
        || current_contract.cooldown_blocks != contract.cooldown_blocks
        || current_contract.window != contract.window;
      let steps_changed = current_contract.steps != contract.steps;
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::validate_contract_steps_shape(snapshot.actor_class.actor_type(), &contract.steps)?;
      Self::validate_parked_balance_activation(&contract)?;
      if snapshot.actor_class.actor_type() == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      Self::validate_recipient_configuration(&contract.steps, &snapshot.sovereign_account)?;
      Self::ensure_contract_fits_service_budget(snapshot.actor_class.actor_type(), &contract)?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if snapshot.actor_class.actor_type() == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(snapshot.cycle_nonce, target_nonce)?;
      }
      Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      // Every non-no-op Contract update rotates semantic and admission authority, so an open run
      // cannot remain bound to the replaced Contract even when only completion policy changes.
      let cancellation_reason = Some(CancellationReason::ContractReplaced);
      let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
      let temporal_anchor_tick =
        Self::temporal_anchor_tick(&contract.trigger).map_err(Self::placement_error)?;
      Self::with_control_transaction(|| {
        if snapshot.actor_class.actor_type() == ActorType::User
          && contract.on_unfunded == OnUnfunded::AwaitFunding
          && matches!(contract.trigger, Trigger::AtTime { .. })
          && (schedule_changed
            || matches!(
              snapshot.trigger_runtime_state,
              TriggerRuntimeState::AtTime {
                consumed: false,
                ..
              }
            ))
        {
          let current = Self::load_actor_ref(actor_id).ok_or(Error::<T>::ActorInvariant)?;
          let generation =
            next_actor_generation(current.generation).ok_or(Error::<T>::ActorInvariant)?;
          Self::secure_at_time_installation_backing(
            &snapshot.owner,
            ActorType::User,
            ActorRef {
              actor_id,
              generation,
            },
            &contract,
          )?;
        }
        if let Some(reason) = cancellation_reason {
          if let Some(state) = stored_cancellation_state {
            let admission =
              Self::load_control_admission(actor_id).ok_or(Error::<T>::ActorRunInvariant)?;
            let identity = state.identity.clone();
            Self::cancel_run_internal_loaded(
              actor_id,
              &identity,
              reason,
              None,
              crate::execution::LoadedCancellationContext::RetainedFrame { admission, state },
            )?;
          } else {
            Self::cancel_run_internal(actor_id, reason, None)?;
          }
        }
        let legacy_authority = ActorControlLocators::<T>::contains_key(actor_id)
          || ActorUnsignaledControlCells::<T>::contains_key(actor_id);
        if schedule_changed && legacy_authority {
          let (state, admission, _) =
            Self::load_frame_actor_service_state(actor_id).ok_or(Error::<T>::ActorInvariant)?;
          if state.hot.trigger_wakeup_pointer.is_some() {
            Self::trigger_wakeup_substrate_invalidate_loaded(actor_id, state, &admission)
              .map_err(Self::placement_error)?;
          }
        }
        Self::try_mutate_control_hot_with_authority(
          actor_id,
          Error::<T>::ActorNotFound,
          |hot| -> DispatchResult {
            if schedule_changed {
              hot.schedule_anchor = schedule_anchor;
              hot.trigger_runtime_state =
                Self::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
              hot.terminal_at = contract
                .window
                .map(|window| Self::window_terminal_at(&window));
            }
            if steps_changed {
              hot.unsuccessful_attempt_streak = crate::execution::transition_failure_streak(
                hot.unsuccessful_attempt_streak,
                crate::execution::FailureStreakTransition::Reset,
              )
              .ok_or(Error::<T>::ActorInvariant)?;
            }
            Self::record_control_mutation_with_authority(actor_id, now)?;
            Ok(())
          },
        )?;
        Self::store_actor_contract(actor_id, contract.clone())?;
        Self::deposit_event(Event::ContractUpdated { actor_id });
        #[cfg(test)]
        crate::mock::control_atomicity_checkpoint(actor_id)?;
        Self::reconcile_actor_state_hold_with_authority(actor_id)?;
        Ok(())
      })
    }

    /// Change new User identity admission independently of existing Actor service and freeze.
    #[pallet::call_index(12)]
    // The shared stale declaration needs physical qualification for both scalar keys and the
    // breaker's conditional current-block halt owner.
    #[pallet::weight(T::WeightInfo::set_global_circuit_breaker())]
    pub fn set_user_creation_enabled(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      Self::apply_user_creation_enabled(enabled);
      Ok(())
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::set_global_circuit_breaker())]
    pub fn set_global_circuit_breaker(origin: OriginFor<T>, paused: bool) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      Self::apply_global_circuit_breaker(paused);
      Ok(())
    }

    /// Force lifecycle evaluation for a specific actor
    #[pallet::call_index(11)]
    #[pallet::weight(T::WeightInfo::permissionless_sweep().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn permissionless_sweep(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::ensure_machine_unfrozen()?;
      Self::evaluate_actor_liveness(actor_id)
    }

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::set_active_actor_limit())]
    pub fn set_active_actor_limit(origin: OriginFor<T>, new_limit: u32) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::ensure_machine_unfrozen()?;
      ensure!(new_limit > 0, Error::<T>::ActiveActorLimitTooLow);
      ensure!(
        new_limit <= T::MaxActiveActors::get(),
        Error::<T>::ActiveActorLimitTooHigh
      );
      ensure!(
        new_limit <= T::MaxQueueLength::get(),
        Error::<T>::ActiveActorLimitExceedsQueueCapacity
      );
      let active_count = Self::active_instance_count();
      ensure!(
        new_limit >= active_count,
        Error::<T>::ActiveActorLimitBelowCurrent
      );
      let old_limit = Self::effective_active_actor_limit();
      if old_limit == new_limit {
        return Ok(());
      }
      ActiveActorLimit::<T>::put(new_limit);
      Self::deposit_event(Event::ActiveActorLimitSet {
        old_limit,
        new_limit,
      });
      Ok(())
    }

    #[pallet::call_index(14)]
    #[pallet::weight(
      T::WeightInfo::permissionless_sweep_many(actor_ids.len() as u32)
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper().saturating_mul(actor_ids.len() as u64))
    )]
    pub fn permissionless_sweep_many(
      origin: OriginFor<T>,
      actor_ids: BoundedVec<ActorId, T::MaxSweepBatch>,
    ) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::ensure_machine_unfrozen()?;
      Self::with_control_transaction(|| {
        let mut closed = 0u32;
        let mut alive = 0u32;
        let mut missing = 0u32;
        let requested = actor_ids.len() as u32;
        for actor_id in actor_ids {
          let state = match Self::load_actor_state_for_frame_control(actor_id) {
            LoadedActorStateOf::Active(state) => state,
            LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
              missing = missing.saturating_add(1);
              continue;
            }
            LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
          };
          let continuation = state.run_state;
          let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
          if let Some(reason) =
            Self::classify_actor_lifecycle_loaded(actor_id, &instance, continuation.as_ref())
              .map_err(Self::classification_dispatch_error)?
              .terminal_reason
          {
            Self::finalize_actor(actor_id, &instance, reason)?;
            closed = closed.saturating_add(1);
          } else {
            alive = alive.saturating_add(1);
          }
        }
        Self::deposit_event(Event::SweepBatchProcessed {
          requested,
          closed,
          alive,
          missing,
        });
        Ok(())
      })
    }

    #[pallet::call_index(17)]
    #[pallet::weight(T::WeightInfo::activate_actor())]
    pub fn activate_actor(
      origin: OriginFor<T>,
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> DispatchResult {
      let identity = match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Dormant(identity) => identity,
        LoadedActorStateOf::Active(_) => return Err(Error::<T>::ActorAlreadyActive.into()),
        LoadedActorStateOf::NotRegistered => return Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
      };
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::ensure_machine_unfrozen()?;
      Self::do_activate_actor(actor_id, identity, contract)
    }

    #[pallet::call_index(18)]
    #[pallet::weight(T::WeightInfo::deactivate_actor())]
    pub fn deactivate_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let instance = match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => {
          Self::derive_active_actor_view(state.identity, state.hot, state.contract)
        }
        LoadedActorStateOf::Dormant(_) => return Err(Error::<T>::ActorDormant.into()),
        LoadedActorStateOf::NotRegistered => return Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
      };
      Self::ensure_control_origin(origin, &instance)?;
      Self::ensure_machine_unfrozen()?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      Self::ensure_control_mutation_allowed(&instance, frame_system::Pallet::<T>::block_number())?;
      Self::do_deactivate_actor(actor_id, instance)
    }

    #[pallet::call_index(19)]
    #[pallet::weight(T::WeightInfo::run_cancel().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn cancel_run(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let run_state = state.run_state;
      let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin, &instance)?;
      Self::ensure_machine_unfrozen()?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if Self::expiry_substitution_due_loaded(actor_id, &instance, run_state.as_ref())? {
        return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        matches!(
          instance.cycle_state,
          CycleState::Running | CycleState::Suspended
        ),
        Error::<T>::ActorRunNotFound
      );
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&instance, now)?;
      Self::with_control_transaction(|| {
        ensure!(
          Self::cancel_run_internal(actor_id, CancellationReason::Explicit, None)?,
          Error::<T>::ActorRunNotFound
        );
        Self::record_control_mutation_with_authority(actor_id, now)?;
        Ok(())
      })
    }

    #[pallet::call_index(23)]
    // The declared bound must cover quiet allocations, not only the configured fixed envelope.
    // Actual execution still uses the frozen domains and returns its charged Weight for reclaim.
    #[pallet::weight((
      T::BlockResourceBudget::get().maximum_block(),
      DispatchClass::Mandatory,
      Pays::No,
    ))]
    pub fn actor_prepass(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
      ensure_none(origin)?;
      let now = frame_system::Pallet::<T>::block_number();
      let actual_weight = Self::execute_mandatory_prepass(now)?;
      Ok(PostDispatchInfo {
        actual_weight: Some(actual_weight),
        pays_fee: Pays::No,
      })
    }
  }

  #[pallet::inherent]
  impl<T: Config> ProvideInherent for Pallet<T> {
    type Call = Call<T>;
    type Error = ActorPrepassInherentError;
    const INHERENT_IDENTIFIER: InherentIdentifier = ACTOR_PREPASS_INHERENT_IDENTIFIER;

    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
      data
        .get_data::<CanonicalActorPrepassInherentData>(&Self::INHERENT_IDENTIFIER)
        .ok()
        .flatten()
        .filter(|provided| provided.0.version == ACTOR_PREPASS_INHERENT_VERSION)
        .map(|_| Call::actor_prepass {})
    }

    fn is_inherent_required(_data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
      Ok(Some(ActorPrepassInherentError::MissingCall))
    }

    fn check_inherent(call: &Self::Call, data: &InherentData) -> Result<(), Self::Error> {
      if !Self::is_inherent(call) {
        return Ok(());
      }
      match data.get_data::<CanonicalActorPrepassInherentData>(&Self::INHERENT_IDENTIFIER) {
        Ok(Some(provided)) if provided.0.version == ACTOR_PREPASS_INHERENT_VERSION => Ok(()),
        Ok(Some(_)) => Err(ActorPrepassInherentError::UnsupportedVersion),
        _ => Err(ActorPrepassInherentError::MissingData),
      }
    }

    fn is_inherent(call: &Self::Call) -> bool {
      matches!(call, Call::actor_prepass {})
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn weight_upper_bound(task: &TaskOf<T>) -> Weight {
      // Runtime owns upper-bound pricing via coarse task classes to reduce calibration churn
      match task {
        ActorTask::Transfer { .. } => T::WeightInfo::task_transfer(),
        ActorTask::Burn { .. } => T::WeightInfo::task_burn(),
        ActorTask::Mint { .. } => T::WeightInfo::task_mint(),
        ActorTask::SwapIn { .. } => T::WeightInfo::task_dex_exact_in(),
        ActorTask::SwapOut { .. } => T::WeightInfo::task_dex_exact_out(),
        ActorTask::AddLiquidity { .. } => T::WeightInfo::task_add_liquidity(),
        ActorTask::RemoveLiquidity { .. } => T::WeightInfo::task_remove_liquidity(),
        ActorTask::Stake { .. } => T::WeightInfo::task_stake(),
        ActorTask::DonateLiquidity { .. } => T::WeightInfo::task_donate_liquidity(),
        ActorTask::Unstake { .. } => T::WeightInfo::task_unstake(),
        ActorTask::StopCycle => T::WeightInfo::task_stop_cycle(),
      }
    }

    /// Conservative FRAME dispatch weight for explicit or lifecycle-touch pure cleanup.
    pub fn close_dispatch_weight_upper() -> Weight {
      Self::close_cleanup_weight_upper()
    }

    /// Authenticate supplied Contract/certificate/host authority before publication. A valid hash
    /// alone proves neither current wake semantics nor complete optional-state cleanup containment.
    fn ensure_current_contract_admission(
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> DispatchResult {
      ensure!(
        certificate.has_valid_identity()
          && certificate.semantic_contract_id == contract.semantic_contract_id()
          && Some(certificate.body_commitment) == contract.body_commitment()
          && certificate.wake_qualification
            == contract.trigger.wake_qualification(&contract.window)
          && Self::admission_matches_current_authority(certificate),
        Error::<T>::ActorInvariant
      );
      if contract.close_when.is_some() {
        ensure!(
          Self::close_control_weight_for_contract(contract)
            .is_some_and(|cleanup| cleanup.all_lte(certificate.maximum_lifecycle_weight)),
          Error::<T>::BehaviorPolicyUnavailable
        );
      }
      Ok(())
    }

    pub(crate) fn close_control_weight_for_contract(
      contract: &ActorContractOf<T>,
    ) -> Option<Weight> {
      // The policy-bearing Close work is inside the measured Close owner, so this bound needs no
      // separate optional-state supplement: `close_dispatch_weight_upper` is the component-wise
      // maximum of the plain and maximum-policy Close owners. Only financial waiting keeps its
      // independent supplement, because it belongs to a separate lifecycle.
      let financial = if contract.on_unfunded == OnUnfunded::AwaitFunding {
        T::AdmissionCertificateAuthority::financial_wait_close_additional_weight(
          Self::financial_wait_episode_bytes()?,
        )?
      } else {
        Weight::zero()
      };
      // A policy-bearing Contract additionally destroys optional policy and monitor state, so its
      // bound is the component-wise maximum of the plain Close owner and the measured
      // policy-bearing Close owner. A maximum of two measured owners is neither a sum of isolated
      // fragments nor the plain owner alone.
      let base = if contract.close_when.is_some() {
        let plain = Self::close_dispatch_weight_upper();
        let policy_bearing = T::WeightInfo::close_actor_policy_bearing();
        Weight::from_parts(
          plain.ref_time().max(policy_bearing.ref_time()),
          plain.proof_size().max(policy_bearing.proof_size()),
        )
      } else {
        Self::close_dispatch_weight_upper()
      };
      base.checked_add(&financial)
    }

    pub fn attempt_fee_envelope(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      start_cursor: usize,
    ) -> Result<AttemptFeeEnvelopeOf<T>, Error<T>> {
      let mut inputs = BoundedVec::default();
      for step in contract_steps {
        let evaluation = Zero::zero();
        let execution =
          if actor_type == ActorType::User && !matches!(step.task, super::types::Task::StopCycle) {
            T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task))
          } else {
            Zero::zero()
          };
        inputs
          .try_push(FeeEnvelopeInput {
            evaluation,
            execution,
          })
          .map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      }
      compose_attempt_fee_envelope(actor_type, &inputs, start_cursor).map_err(|error| match error {
        FeeEnvelopeError::CursorOutOfBounds | FeeEnvelopeError::ReservationUnderflow => {
          Error::<T>::ActorRunInvariant
        }
        FeeEnvelopeError::Overflow => Error::<T>::AdmissionBoundOverflow,
      })
    }

    /// Upper-bounds one prospective run plus pure terminal cleanup after the baseline scheduler
    /// envelope. Independently metered durable housekeeping may defer this work across blocks.
    pub fn contract_steps_admission_weight_upper(
      _actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> Weight {
      let maximum_step = if contract_steps.is_empty() {
        T::WeightInfo::scheduler_inner_zero_step_complete()
      } else {
        Self::derive_step_resource_envelopes(&ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: Zero::zero(),
          window: None,
          steps: contract_steps.clone(),
          funding: FundingSourcePolicy::OwnerOnly,
          on_unfunded: crate::OnUnfunded::Close,
          close_when: None,
          parked_balance_activation: None,
          auto_close_at_cycle_nonce: None,
        })
        .map(|resources| {
          resources
            .into_iter()
            .fold(Weight::zero(), |maximum, resource| {
              let current = resource.control.saturating_add(resource.effect);
              Weight::from_parts(
                maximum.ref_time().max(current.ref_time()),
                maximum.proof_size().max(current.proof_size()),
              )
            })
        })
        .unwrap_or(Weight::MAX)
      };
      Self::scheduler_admission_overhead()
        .saturating_add(maximum_step)
        .saturating_add(Self::close_cleanup_weight_upper())
        .max(Self::scheduler_complete_outer_weight_upper())
    }

    fn settle_on_idle_control(
      authority: &mut Option<(
        BlockResourceState<BlockNumberFor<T>>,
        BlockResourceReservation,
      )>,
      actual: Weight,
    ) {
      let Some((mut state, mut reservation)) = authority.take() else {
        return;
      };
      if state.settle(&mut reservation, actual).is_err() {
        state.halt_optional_actor_work();
      }
      CurrentBlockResourceState::<T>::put(state);
    }

    pub fn guaranteed_actor_service_weight() -> Option<Weight> {
      T::ActorOnIdleReserve::get()
        .checked_sub(&T::WeightInfo::scheduler_on_idle_base())
        .and_then(|remaining| remaining.checked_sub(&Self::scheduler_complete_outer_weight_upper()))
    }

    fn mandatory_actor_control_envelopes() -> Option<[Weight; 4]> {
      Some([
        T::WeightInfo::scheduler_on_initialize_cutoff(),
        Self::deadline_service_weight_upper(),
        T::WeightInfo::dependency_scan_source_probe().checked_add(
          &T::WeightInfo::process_dependency_scan_unit()
            .max(T::WeightInfo::process_dependency_scan_completion_unit()),
        )?,
        T::WeightInfo::scheduler_on_idle_base()
          .checked_add(&T::WeightInfo::block_resource_finalize())?,
      ])
    }

    pub(crate) fn mandatory_actor_control_weight_upper() -> Option<Weight> {
      Self::mandatory_actor_control_envelopes()?
        .into_iter()
        .try_fold(Weight::zero(), |total, weight| total.checked_add(&weight))
    }

    /// Common discovery, loaded-state inspection and outer ring transition of one admitted turn.
    /// Inner Step control and optional inspections retain their independent owners.
    pub(crate) fn service_turn_control_weight_upper() -> Option<Weight> {
      T::WeightInfo::service_round_begin_populated()
        .checked_add(&T::WeightInfo::service_round_probe_eligible())?
        .checked_add(&T::WeightInfo::scheduler_actor_state_probe())?
        .checked_add(
          &T::WeightInfo::service_round_admit_eligible()
            .max(T::WeightInfo::service_member_retire_interior())
            .max(T::WeightInfo::service_member_retire_pair_cursor())
            .max(T::WeightInfo::service_member_retire_singleton()),
        )
    }

    /// Complete conservative Service encounter, with independent Control and effect owners.
    /// Cleanup covers every reachable terminal branch; StopCycle's Task allowance is Control.
    pub(crate) fn contract_service_resources_upper(
      contract: &ActorContractOf<T>,
    ) -> Result<ActorStepResourceEnvelope, Error<T>> {
      let resources = Self::try_derive_step_resource_envelopes(contract)?;
      let mut maximum = if contract.steps.is_empty() {
        Self::try_zero_step_resource_envelope(contract)?
      } else {
        ActorStepResourceEnvelope {
          control: Weight::zero(),
          effect: Weight::zero(),
        }
      };
      for (step, resource) in contract.steps.iter(/* deos-bypass: bounded-iter */).zip(resources) {
        let resource =
          Self::step_service_resources(step, resource).ok_or(Error::<T>::AdmissionBoundOverflow)?;
        maximum.control = maximum.control.max(resource.control);
        maximum.effect = maximum.effect.max(resource.effect);
      }
      let mut cleanup = Self::close_control_weight_for_contract(contract)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let financial_inspection_and_entry = if contract.on_unfunded == OnUnfunded::AwaitFunding {
        let bytes =
          Self::financial_wait_episode_bytes().ok_or(Error::<T>::AdmissionBoundOverflow)?;
        maximum.control = maximum
          .control
          .max(T::WeightInfo::at_time_trigger_occurrence());
        if contract.close_when.is_some() {
          cleanup = cleanup.max(
            T::AdmissionCertificateAuthority::current()
              .ok_or(Error::<T>::AdmissionBoundOverflow)?
              .maximum_lifecycle_weight,
          );
          if contract.steps.is_empty() {
            maximum.control = maximum.control.max(
              T::StepControlWeight::maximum_behavior_zero_step_service_weight(
                Self::behavior_service_inspection_bounds()
                  .ok_or(Error::<T>::AdmissionBoundOverflow)?,
              )
              .ok_or(Error::<T>::BehaviorPolicyUnavailable)?,
            );
          }
        }
        T::AdmissionCertificateAuthority::financial_wait_service_weight(bytes)
          .ok_or(Error::<T>::AdmissionBoundOverflow)?
          .checked_add(
            &T::AdmissionCertificateAuthority::financial_wait_entry_weight(bytes)
              .ok_or(Error::<T>::AdmissionBoundOverflow)?,
          )
          .ok_or(Error::<T>::AdmissionBoundOverflow)?
      } else {
        Weight::zero()
      };
      let inspection = Self::policy_service_inspection_weight(contract)
        .ok_or(Error::<T>::BehaviorPolicyUnavailable)?;
      maximum.control = Self::service_turn_control_weight_upper()
        .and_then(|common| common.checked_add(&inspection))
        .and_then(|control| control.checked_add(&financial_inspection_and_entry))
        .and_then(|control| {
          control.checked_add(
            &Self::service_refusal_completion_for_contract(contract).unwrap_or_else(Weight::zero),
          )
        })
        .and_then(|control| control.checked_add(&maximum.control))
        .and_then(|control| control.checked_add(&cleanup))
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(maximum)
    }

    fn ensure_contract_fits_service_budget(
      actor_type: ActorType,
      contract: &ActorContractOf<T>,
    ) -> DispatchResult {
      let actor_service = Self::guaranteed_actor_service_weight()
        .ok_or(Error::<T>::ContractStepsExceedOnIdleBudget)?;
      ensure!(
        Self::contract_steps_admission_weight_upper(actor_type, &contract.steps)
          .all_lte(actor_service),
        Error::<T>::ContractStepsExceedOnIdleBudget
      );
      let limits = T::BlockResourceBudget::get().limits();
      let service = Self::contract_service_resources_upper(contract)?;
      let control = Self::mandatory_actor_control_weight_upper()
        .and_then(|mandatory| limits.actor_control().checked_sub(&mandatory))
        .ok_or(Error::<T>::ContractStepsExceedOnIdleBudget)?;
      ensure!(
        service.control.all_lte(control) && service.effect.all_lte(limits.actor_base_turn()),
        Error::<T>::ContractStepsExceedOnIdleBudget
      );
      if actor_type == ActorType::User {
        let resources =
          Self::derive_step_resource_envelopes(contract).ok_or(Error::<T>::ActorRunInvariant)?;
        for resource in resources.iter(/* deos-bypass: bounded-iter */) {
          Self::maximum_current_step_fee(actor_type, *resource)?;
        }
      }
      Ok(())
    }

    fn owner_slot_bitmap_is_valid(bitmap: &OwnerSlotBitmap) -> bool {
      let max_slots = T::MaxOwnerSlots::get() as usize;
      if max_slots == 0 {
        return false;
      }
      let full_bytes = max_slots / 8;
      let remaining_bits = max_slots % 8;
      for index in 0..bitmap.len() {
        let byte = bitmap[index];
        let valid = if index < full_bytes {
          true
        } else if index == full_bytes && remaining_bits > 0 {
          byte & !((1u8 << remaining_bits) - 1) == 0
        } else {
          byte == 0
        };
        if !valid {
          return false;
        }
      }
      true
    }

    pub(crate) fn owner_slot_is_set(bitmap: &OwnerSlotBitmap, owner_slot: u8) -> bool {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] & (1u8 << bit) != 0
    }

    fn set_owner_slot(bitmap: &mut OwnerSlotBitmap, owner_slot: u8) {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] |= 1u8 << bit;
    }

    fn clear_owner_slot(bitmap: &mut OwnerSlotBitmap, owner_slot: u8) {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] &= !(1u8 << bit);
    }

    fn owner_slot_bitmap_is_empty(bitmap: &OwnerSlotBitmap) -> bool {
      for byte in bitmap.as_slice() {
        if *byte != 0 {
          return false;
        }
      }
      true
    }

    fn state_hold_component(encoded_bytes: usize) -> Result<T::Balance, Error<T>> {
      if encoded_bytes == 0 {
        return Ok(T::Balance::zero());
      }
      let encoded_bytes =
        u32::try_from(encoded_bytes).map_err(|_| Error::<T>::StateHoldOverflow)?;
      let bytes: T::Balance = encoded_bytes.into();
      T::ActorStateHoldPerByte::get()
        .checked_mul(&bytes)
        .and_then(|priced_bytes| priced_bytes.checked_add(&T::ActorStateHoldBase::get()))
        .ok_or(Error::<T>::StateHoldOverflow)
    }

    fn state_hold_total(breakdown: &ActorStateHoldBreakdownOf<T>) -> Result<T::Balance, Error<T>> {
      [
        breakdown.identity,
        breakdown.contract_head,
        breakdown.contract_body,
        breakdown.detector,
        breakdown.run,
      ]
      .into_iter()
      .try_fold(T::Balance::zero(), |total, component| {
        total
          .checked_add(&component)
          .ok_or(Error::<T>::StateHoldOverflow)
      })
    }

    fn actor_state_hold_quote(
      actor_id: ActorId,
      actor_type: ActorType,
    ) -> Result<ActorStateHoldQuote<T::Balance>, ActorCostQuoteError> {
      let breakdown = if actor_type == ActorType::System {
        if ActorStateHolds::<T>::contains_key(actor_id) {
          return Err(ActorCostQuoteError::ActorInvariant);
        }
        ActorStateHoldBreakdown {
          identity: T::Balance::zero(),
          contract_head: T::Balance::zero(),
          contract_body: T::Balance::zero(),
          detector: T::Balance::zero(),
          run: T::Balance::zero(),
        }
      } else {
        ActorStateHolds::<T>::get(actor_id)
          .ok_or(ActorCostQuoteError::ActorInvariant)?
          .breakdown
      };
      let total =
        Self::state_hold_total(&breakdown).map_err(|_| ActorCostQuoteError::ComputationOverflow)?;
      Ok(ActorStateHoldQuote {
        exempt: actor_type == ActorType::System,
        base_per_component: T::ActorStateHoldBase::get(),
        per_encoded_byte: T::ActorStateHoldPerByte::get(),
        breakdown,
        total,
      })
    }

    fn add_state_hold_encoded_size<Value: codec::Encode>(
      total: &mut usize,
      value: &Value,
    ) -> Result<(), Error<T>> {
      *total = total
        .checked_add(codec::Encode::encoded_size(value))
        .ok_or(Error::<T>::StateHoldOverflow)?;
      Ok(())
    }

    fn derive_actor_state_hold_with_authority(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
      authority: Option<(&ActorHotStateOf<T>, &ActorAdmissionCertificateOf<T>)>,
    ) -> Result<ActorStateHoldBreakdownOf<T>, Error<T>> {
      let mut identity_bytes = 0usize;
      Self::add_state_hold_encoded_size(&mut identity_bytes, &actor_id)?;
      if authority.is_some() {
        let control_identity = Self::control_identity_from_scalar(identity.clone())
          .ok_or(Error::<T>::StateHoldInvariant)?;
        Self::add_state_hold_encoded_size(&mut identity_bytes, &control_identity)?;
      } else {
        Self::add_state_hold_encoded_size(&mut identity_bytes, identity)?;
      }
      Self::add_state_hold_encoded_size(&mut identity_bytes, &identity.sovereign_account)?;

      let mut breakdown = ActorStateHoldBreakdown {
        identity: Self::state_hold_component(identity_bytes)?,
        contract_head: T::Balance::zero(),
        contract_body: T::Balance::zero(),
        detector: T::Balance::zero(),
        run: T::Balance::zero(),
      };
      if identity.actor_class.actor_type() == ActorType::System {
        return Ok(ActorStateHoldBreakdown {
          identity: T::Balance::zero(),
          ..breakdown
        });
      }

      let Some((hot, admission)) = authority else {
        ensure!(
          !ActorContractHeads::<T>::contains_key(actor_id)
            && !Self::control_admission_exists(actor_id)
            && !ActorRunStateStore::<T>::contains_key(actor_id),
          Error::<T>::StateHoldInvariant
        );
        return Ok(breakdown);
      };
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;

      breakdown.contract_head =
        Self::state_hold_component(Self::control_state_hold_head_bytes(&head, admission)?)?;

      let chunk_count = head
        .header
        .step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let mut body_bytes = 0usize;
      for chunk_index in 0..chunk_count {
        let chunk = ActorContractTailChunks::<T>::get(actor_id, chunk_index)
          .ok_or(Error::<T>::StateHoldInvariant)?;
        Self::add_state_hold_encoded_size(&mut body_bytes, &chunk)?;
      }
      if head.header.has_close_when {
        let policy = Self::load_contract_behavior_policy(actor_id, &head.header)
          .flatten()
          .ok_or(Error::<T>::StateHoldInvariant)?;
        Self::add_state_hold_encoded_size(&mut body_bytes, &policy)?;
        if let Some(monitor) = Self::load_contract_behavior_monitor(actor_id, &head.header, &policy)
          .ok_or(Error::<T>::StateHoldInvariant)?
        {
          Self::add_state_hold_encoded_size(&mut body_bytes, &monitor)?;
          if policy.plan.requires_clean_cycle() && monitor.clean_cycle.is_none() {
            body_bytes = body_bytes
              .checked_add(BehaviorCleanCycle::max_encoded_len())
              .ok_or(Error::<T>::StateHoldOverflow)?;
          }
        }
      }
      if head.header.on_unfunded == OnUnfunded::AwaitFunding {
        body_bytes = body_bytes
          .checked_add(FinancialWaitEpisode::<T::Balance>::max_encoded_len())
          .ok_or(Error::<T>::StateHoldOverflow)?;
      }
      breakdown.contract_body = Self::state_hold_component(body_bytes)?;

      breakdown.detector = Self::state_hold_component(Self::state_hold_detector_bytes(hot)?)?;
      breakdown.run = Self::state_hold_component(
        <ActorRunStateOf<T> as codec::MaxEncodedLen>::max_encoded_len(),
      )?;
      Ok(breakdown)
    }

    fn control_state_hold_head_bytes(
      head: &ActorContractHeadOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Result<usize, Error<T>> {
      let mut bytes =
        <ActorControlHotState<BlockNumberFor<T>> as codec::MaxEncodedLen>::max_encoded_len();
      for component in [
        <u32 as codec::MaxEncodedLen>::max_encoded_len(),
        <Option<BlockNumberFor<T>> as codec::MaxEncodedLen>::max_encoded_len(),
        <ActorStepResourceEnvelope as codec::MaxEncodedLen>::max_encoded_len(),
      ] {
        bytes = bytes
          .checked_add(component)
          .ok_or(Error::<T>::StateHoldOverflow)?;
      }
      Self::add_state_hold_encoded_size(&mut bytes, admission)?;
      Self::add_state_hold_encoded_size(&mut bytes, head)?;
      Ok(bytes)
    }

    fn state_hold_detector_bytes(hot: &ActorHotStateOf<T>) -> Result<usize, Error<T>> {
      // Removing a detector for a paid latch or financial wait does not retire its capacity.
      // Autonomous rearm must not depend on the owner's later unheld balance.
      if matches!(
        hot.trigger_runtime_state,
        TriggerRuntimeState::Cadenced { .. }
          | TriggerRuntimeState::AtTime {
            consumed: false,
            ..
          }
      ) {
        return Ok(TriggerWakeupPointer::max_encoded_len());
      }
      let mut bytes = 0usize;
      if let Some(pointer) = hot.trigger_wakeup_pointer {
        Self::add_state_hold_encoded_size(&mut bytes, &pointer)?;
      }
      Ok(bytes)
    }

    pub(crate) fn reconcile_actor_state_hold_with_authority(actor_id: ActorId) -> DispatchResult {
      let existing = ActorStateHolds::<T>::get(actor_id);
      let target = if ActorControlLocators::<T>::contains_key(actor_id) {
        let (_, identity, hot, admission) =
          Self::load_frame_control_authority(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;
        if identity.actor_class.actor_type() == ActorType::User {
          Some((
            identity.owner.clone(),
            Self::derive_actor_state_hold_with_authority(
              actor_id,
              &identity,
              Some((&hot, &admission)),
            )?,
          ))
        } else {
          None
        }
      } else {
        ensure!(
          !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
          Error::<T>::StateHoldInvariant
        );
        match ActorSemanticStates::<T>::get(actor_id) {
          Some(ActorSemanticState::Active(record))
            if record.identity.actor_class.actor_type() == ActorType::User =>
          {
            Some((
              record.identity.owner.clone(),
              Self::derive_actor_state_hold_with_authority(
                actor_id,
                &record.identity,
                Some((&record.hot, &record.admission)),
              )?,
            ))
          }
          Some(ActorSemanticState::Dormant(record))
            if record.identity.actor_class.actor_type() == ActorType::User =>
          {
            Some((
              record.identity.owner.clone(),
              Self::derive_actor_state_hold_with_authority(actor_id, &record.identity, None)?,
            ))
          }
          Some(_) | None => None,
        }
      };
      let (owner, target_breakdown) = match (existing.as_ref(), target) {
        (Some(_), Some((owner, breakdown))) => (owner, breakdown),
        (None, Some(target)) => target,
        (Some(existing), None) => (
          existing.owner.clone(),
          ActorStateHoldBreakdown {
            identity: T::Balance::zero(),
            contract_head: T::Balance::zero(),
            contract_body: T::Balance::zero(),
            detector: T::Balance::zero(),
            run: T::Balance::zero(),
          },
        ),
        (None, None) => return Ok(()),
      };
      Self::apply_actor_state_hold(actor_id, existing, owner, target_breakdown)
    }

    fn apply_actor_state_hold(
      actor_id: ActorId,
      existing: Option<ActorStateHoldRecordOf<T>>,
      owner: T::AccountId,
      target_breakdown: ActorStateHoldBreakdownOf<T>,
    ) -> DispatchResult {
      ensure!(
        existing.as_ref().is_none_or(|record| record.owner == owner),
        Error::<T>::StateHoldInvariant
      );
      if existing
        .as_ref()
        .is_some_and(|record| record.breakdown == target_breakdown)
      {
        return Ok(());
      }
      let old_total = existing
        .as_ref()
        .map(|record| Self::state_hold_total(&record.breakdown))
        .transpose()?
        .unwrap_or_else(T::Balance::zero);
      let target_total = Self::state_hold_total(&target_breakdown)?;
      let reason: T::RuntimeHoldReason = HoldReason::ActorState.into();
      if target_total > old_total {
        let increase = target_total
          .checked_sub(&old_total)
          .ok_or(Error::<T>::StateHoldOverflow)?;
        T::StateHoldCurrency::hold(&reason, &owner, increase)
          .map_err(|_| Error::<T>::StateHoldUnavailable)?;
      } else if old_total > target_total {
        let decrease = old_total
          .checked_sub(&target_total)
          .ok_or(Error::<T>::StateHoldOverflow)?;
        let released = T::StateHoldCurrency::release(&reason, &owner, decrease, Precision::Exact)
          .map_err(|_| Error::<T>::StateHoldInvariant)?;
        ensure!(released == decrease, Error::<T>::StateHoldInvariant);
      }
      if target_total.is_zero() {
        ActorStateHolds::<T>::remove(actor_id);
      } else {
        ActorStateHolds::<T>::insert(
          actor_id,
          ActorStateHoldRecord {
            owner,
            breakdown: target_breakdown,
          },
        );
      }
      Ok(())
    }

    fn charge_creation_fee(owner: &T::AccountId) -> DispatchResult {
      let creation_fee = T::ActorCreationFee::get();
      if creation_fee.is_zero() {
        return Ok(());
      }
      let native = T::FeeNativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::FeeCollector::collect_fee(owner, &fee_sink, native, creation_fee)
        .map_err(|_| Error::<T>::InsufficientFee.into())
    }

    fn ensure_trigger_occurrence_capacity(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> DispatchResult {
      ensure!(
        Self::trigger_occurrence_capacity_sufficient(actor_type, sovereign_account, breakdown,)?,
        Error::<T>::InsufficientFee
      );
      Ok(())
    }

    pub(crate) fn trigger_occurrence_capacity_sufficient(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
        return Ok(true);
      }
      let required = T::MinUserBalance::get()
        .checked_add(&breakdown.trigger_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    fn charge_trigger_occurrence(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> DispatchResult {
      Self::ensure_trigger_occurrence_capacity(actor_type, sovereign_account, breakdown)?;
      if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
        return Ok(());
      }
      let native = T::FeeNativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::FeeCollector::collect_fee(sovereign_account, &fee_sink, native, breakdown.trigger_fee)
        .map_err(|_| Error::<T>::TriggerFeeCollectionFailed.into())
    }

    /// Atomically charges one useful Trigger occurrence and replaces an exact canonical
    /// publication with its latched successor. Payment or publication refusal restores the
    /// complete storage root.
    pub(crate) fn commit_canonical_trigger_occurrence_with_authority(
      actor: ActorRef,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      state: ActiveActorStateOf<T>,
      now: BlockNumberFor<T>,
    ) -> Result<crate::scheduler::ActivationOutcome, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| {
          ensure!(!state.hot.pending_signal, Error::<T>::ActorInvariant);
          Self::ensure_trigger_occurrence_capacity(actor_type, sovereign_account, breakdown)?;
          ensure!(
            state.identity.actor_class.actor_type() == actor_type
              && state.identity.sovereign_account == *sovereign_account,
            Error::<T>::ActorInvariant
          );

          // The caller may hold only the bounded current-Step service envelope, whose
          // reconstructed Contract cannot reproduce the full body commitment. The stored semantic
          // record is the authority: require the caller's generation, identity, Hot state, Run
          // store, and Contract trigger authorization to agree before latching it.
          let Some(ActorSemanticState::Active(stored)) =
            ActorSemanticStates::<T>::get(actor.actor_id)
          else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          ensure!(
            stored.generation == actor.generation
              && stored.identity == state.identity
              && stored.hot == state.hot
              && Self::admission_authorizes_contract_wake(&stored.admission, &state.contract)
              && ActorRunStateStore::<T>::get(actor.actor_id)
                .as_ref()
                .map(|run| run.encode())
                == state.run_state.as_ref().map(|run| run.encode())
              && !ActorControlLocators::<T>::contains_key(actor.actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id),
            Error::<T>::ActorInvariant
          );
          let expected_semantic = ActorSemanticState::Active(stored);
          let process = ActorProcesses::<T>::get(actor.actor_id)
            .filter(|process| process.generation == actor.generation)
            .ok_or(Error::<T>::ActorInvariant)?;
          let plan = plan_canonical_occurrence(
            state.hot.cycle_state,
            state.hot.pending_signal,
            process,
            now,
          )
          .map_err(|_| Error::<T>::ActorInvariant)?
          .ok_or(Error::<T>::ActorInvariant)?;

          let mut successor = state.clone();
          successor.hot.pending_signal = plan.pending_signal;
          if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
            Self::remove_trigger_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
            successor.hot.trigger_wakeup_pointer = None;
          } else {
            ensure!(
              successor.hot.trigger_wakeup_pointer.is_none(),
              Error::<T>::ActorInvariant
            );
          }

          match plan.publication {
            CanonicalOccurrencePublication::PreserveResidence => {
              ensure!(plan.process == process, Error::<T>::ActorInvariant);
            }
            CanonicalOccurrencePublication::PublishPending { .. } => {
              match process.residence {
                Some(ProcessResidence::Deadline { .. }) => {
                  Self::remove_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                Some(ProcessResidence::Parked(evidence)) => {
                  let owner = PendingCheckOwners::<T>::get(actor.actor_id)
                    .ok_or(Error::<T>::ActorInvariant)?;
                  Self::wake_parked_member_to_service(
                    actor,
                    ServiceResidenceKind::Pending,
                    owner,
                    evidence,
                    now,
                  )
                  .map_err(|_| Error::<T>::ActorInvariant)?;
                  ensure!(
                    ActorProcesses::<T>::get(actor.actor_id) == Some(plan.process),
                    Error::<T>::ActorInvariant
                  );
                }
                None if matches!(process.status, ProcessStatus::Disabled(_)) => {
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                // Readiness on an existing Idle Service member is a new B+1 occurrence:
                // refresh the latch and re-admit Pending authority for the next block.
                Some(ProcessResidence::Service(_)) => {
                  Self::remove_service_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                _ => return Err(Error::<T>::ActorInvariant.into()),
              }
              let latched = Self::derive_active_actor_view(
                successor.identity.clone(),
                successor.hot.clone(),
                successor.contract.clone(),
              );
              if let Some(due) =
                Self::latched_idle_deadline(&latched, now).map_err(Self::placement_error)?
              {
                let destination = Self::plan_deadline_destination(actor, WakeupKey::Block(due))
                  .map_err(|_| Error::<T>::ActorInvariant)?;
                Self::transfer_service_member_to_deadline(actor, destination)
                  .map_err(|_| Error::<T>::ActorInvariant)?;
              }
            }
          }

          let ActorSemanticState::Active(mut replacement) = expected_semantic.clone() else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          replacement.hot = successor.hot;
          Self::mutate_actor_semantic_state(
            actor.actor_id,
            ActorSemanticMutation::Replace {
              expected: expected_semantic,
              replacement: ActorSemanticState::Active(replacement),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Self::reconcile_actor_state_hold_with_authority(actor.actor_id)?;
          Self::charge_trigger_occurrence(actor_type, sovereign_account, breakdown)?;
          Self::deposit_event(Event::TriggerOccurrenceProcessed {
            actor_id: actor.actor_id,
            trigger_family: breakdown.trigger_family,
            fee: breakdown.trigger_fee,
          });
          Ok(crate::scheduler::ActivationOutcome::Latched)
        })();
        match result {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn commit_frame_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      _cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      let mut state = Self::active_actor_state_for_frame_control(actor_id)?;
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      state.identity = record.identity.clone();
      state.hot = record.hot.clone();
      let actor = ActorRef {
        actor_id,
        generation: record.generation,
      };
      Self::commit_canonical_trigger_occurrence_with_authority(
        actor,
        actor_type,
        sovereign_account,
        breakdown,
        state,
        frame_system::Pallet::<T>::block_number(),
      )
      .map(Some)
    }

    pub(crate) fn try_commit_frame_automatic_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::commit_frame_trigger_occurrence(
          actor_id,
          actor_type,
          sovereign_account,
          breakdown,
          cause_provenance,
        ) {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) if error == Error::<T>::InsufficientFee.into() => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(None))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    #[allow(
      dead_code,
      reason = "legacy control-reference profiles retain prechecked Trigger collection"
    )]
    pub(crate) fn try_charge_prechecked_automatic_trigger_occurrence(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> Result<bool, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true));
        }
        let result = T::FeeCollector::collect_fee(
          sovereign_account,
          &T::FeeSink::get(),
          T::FeeNativeAssetId::get(),
          breakdown.trigger_fee,
        );
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true)),
          Err(_) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::TriggerFeeCollectionFailed.into(),
          )),
        }
      })
    }

    pub(crate) fn pipeline_fee_for_actor(
      actor_id: ActorId,
      actor_type: ActorType,
    ) -> Result<PipelineFeeBreakdown<T::Balance>, Error<T>> {
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      Ok(Self::pipeline_fee_breakdown(
        actor_type,
        head.header.pipeline_machine_envelope,
      ))
    }

    pub(crate) fn pipeline_capacity_sufficient(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      Self::pipeline_capacity_sufficient_with_envelope(
        actor_type,
        sovereign_account,
        head.header.pipeline_machine_envelope,
      )
    }

    pub(crate) fn pipeline_capacity_sufficient_with_envelope(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      envelope: PipelineMachineEnvelope<T::Balance>,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let breakdown = Self::pipeline_fee_breakdown(actor_type, envelope);
      let required = T::MinUserBalance::get()
        .checked_add(&breakdown.total_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    pub(crate) fn action_capacity_sufficient(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      step: &StepOf<T>,
      resources: ActorStepResourceEnvelope,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let fee = Self::maximum_current_action_fee(actor_type, step, resources)?;
      if fee.total_fee.is_zero() {
        return Ok(true);
      }
      let required = T::MinUserBalance::get()
        .checked_add(&fee.total_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    pub(crate) fn collect_pipeline_fee(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
    ) -> Result<PipelineFeeBreakdown<T::Balance>, DispatchError> {
      let breakdown = Self::pipeline_fee_for_actor(actor_id, actor_type)?;
      if actor_type == ActorType::System || breakdown.total_fee.is_zero() {
        return Ok(breakdown);
      }
      ensure!(
        Self::pipeline_capacity_sufficient(actor_id, actor_type, sovereign_account)?,
        Error::<T>::InsufficientFee
      );
      let native = T::FeeNativeAssetId::get();
      T::FeeCollector::collect_fee(
        sovereign_account,
        &T::FeeSink::get(),
        native,
        breakdown.total_fee,
      )
      .map_err(|_| Error::<T>::InsufficientFee)?;
      Ok(breakdown)
    }

    #[cfg(test)]
    pub(crate) fn maximum_contract_step_fee(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      cursor: usize,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      let resources = Self::derive_step_resource_envelopes(&ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: Zero::zero(),
        window: None,
        steps: contract_steps.clone(),
        funding: FundingSourcePolicy::OwnerOnly,
        on_unfunded: crate::OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        auto_close_at_cycle_nonce: None,
      })
      .and_then(|resources| resources.get(cursor).copied())
      .ok_or(Error::<T>::ActorRunInvariant)?;
      let step = contract_steps
        .get(cursor)
        .ok_or(Error::<T>::ActorRunInvariant)?;
      Self::maximum_current_action_fee(actor_type, step, resources)
    }

    /// Returns ledger minimum plus the generated Pipeline Machine charge.
    /// Final Actor-state destruction is economically backed by creation.
    /// Trigger-family pricing is composed separately at ready Opening.
    pub fn user_pipeline_machine_capacity_requirement(
      contract_steps: &ContractSteps<T>,
    ) -> Result<BalanceOf<T>, Error<T>> {
      let contract = ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: Zero::zero(),
        window: None,
        steps: contract_steps.clone(),
        funding: FundingSourcePolicy::OwnerOnly,
        on_unfunded: crate::OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        auto_close_at_cycle_nonce: None,
      };
      let resources =
        Self::derive_step_resource_envelopes(&contract).ok_or(Error::<T>::ActorRunInvariant)?;
      let envelope = Self::derive_pipeline_machine_envelope(
        ActorType::User,
        contract.on_unfunded,
        contract_steps,
        &resources,
        Self::zero_step_resource_envelope(&contract)
          .map_or(Weight::zero(), |resources| resources.control),
        Weight::zero(),
      )?;
      T::MinUserBalance::get()
        .checked_add(&envelope.pipeline_machine_fee_upper)
        .ok_or(Error::<T>::AdmissionBoundOverflow)
    }

    pub fn sovereign_account_id(owner: &T::AccountId, owner_slot: u8) -> T::AccountId {
      T::SovereignAccountDeriver::user(T::PalletId::get(), owner, owner_slot)
    }

    pub fn sovereign_account_id_system(actor_id: ActorId) -> T::AccountId {
      T::SovereignAccountDeriver::system(T::PalletId::get(), actor_id)
    }

    pub(crate) fn available_owner_slot(
      owner: &T::AccountId,
      preferred_slot: Option<u8>,
    ) -> Result<u8, Error<T>> {
      let bitmap = OwnerSlotBitmaps::<T>::get(owner);
      let max_slots = T::MaxOwnerSlots::get();
      ensure!(max_slots > 0, Error::<T>::InvalidOwnerSlot);
      ensure!(
        Self::owner_slot_bitmap_is_valid(&bitmap),
        Error::<T>::InvalidOwnerSlot
      );
      match preferred_slot {
        Some(slot) => {
          ensure!(slot < max_slots, Error::<T>::InvalidOwnerSlot);
          ensure!(
            !Self::owner_slot_is_set(&bitmap, slot),
            Error::<T>::OwnerSlotOccupied
          );
          Ok(slot)
        }
        None => {
          for byte_index in 0..bitmap.len() {
            let byte = bitmap[byte_index];
            let first_slot = byte_index * 8;
            if first_slot >= max_slots as usize {
              break;
            }
            let remaining = (max_slots as usize).saturating_sub(first_slot);
            let valid_bits = if remaining >= 8 {
              u8::MAX
            } else {
              (1u8 << remaining) - 1
            };
            let free_bits = !byte & valid_bits;
            if free_bits != 0 {
              return Ok((first_slot + free_bits.trailing_zeros() as usize) as u8);
            }
          }
          Err(Error::<T>::OwnerSlotCapacityExceeded)
        }
      }
    }

    fn allocate_owner_slot(
      owner: &T::AccountId,
      preferred_slot: Option<u8>,
    ) -> Result<(u8, T::AccountId), Error<T>> {
      let mut bitmap = OwnerSlotBitmaps::<T>::get(owner);
      let owner_slot = Self::available_owner_slot(owner, preferred_slot)?;
      let sovereign_account = Self::sovereign_account_id(owner, owner_slot);
      if T::SovereignAccountPolicy::is_reserved(&sovereign_account) {
        return Err(Error::<T>::ReservedSovereignAccount);
      }
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Self::set_owner_slot(&mut bitmap, owner_slot);
      OwnerSlotBitmaps::<T>::insert(owner, bitmap);
      Ok((owner_slot, sovereign_account))
    }

    fn allocate_system_sovereign(actor_id: ActorId) -> Result<T::AccountId, Error<T>> {
      let sovereign_account = Self::sovereign_account_id_system(actor_id);
      // Context-aware reservation: a fresh (unregistered) derivation that aliases a
      // host-reserved account fails ReservedSovereignAccount; reattachment to an
      // existing registered Vacant locator is allowed for that exact locator even
      // when its account belongs to the genesis System custody range, so the locator
      // is not permanently unrecoverable after close.
      let is_registered_reattachment =
        SystemSovereigns::<T>::get(actor_id) == Some(SystemSovereignState::Vacant);
      if !is_registered_reattachment && T::SovereignAccountPolicy::is_reserved(&sovereign_account) {
        return Err(Error::<T>::ReservedSovereignAccount);
      }
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Ok(sovereign_account)
    }

    fn do_create_dormant_actor(
      owner: T::AccountId,
      actor_type: ActorType,
      preferred_user_slot: Option<u8>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      let actor_id = NextActorId::<T>::get();
      ensure!(
        matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        ),
        Error::<T>::ActorIdOccupied
      );
      let next_id = actor_id.checked_add(1).ok_or(Error::<T>::ActorIdOverflow)?;
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(actor_id);
      if actor_type == ActorType::System {
        match requested_system_sovereign_id {
          Some(_) => match SystemSovereigns::<T>::get(system_sovereign_id) {
            Some(SystemSovereignState::Vacant) => {}
            Some(SystemSovereignState::Occupied(_)) => {
              return Err(Error::<T>::SystemSovereignOccupied.into());
            }
            None => {
              return Err(Error::<T>::SystemSovereignUnknown.into());
            }
          },
          None => {
            ensure!(
              !SystemSovereigns::<T>::contains_key(system_sovereign_id),
              Error::<T>::SystemSovereignOccupied
            );
            ensure!(
              SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
              Error::<T>::SystemSovereignCapacityExceeded
            );
          }
        }
      }
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if actor_type == ActorType::User {
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let (actor_class, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (ActorClass::User { owner_slot: slot }, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (
              ActorClass::System {
                sovereign_id: system_sovereign_id,
              },
              account,
            ),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class,
          mutability: Mutability::Mutable,
          cycle_nonce: 0,
          last_control_mutation_block: frame_system::Pallet::<T>::block_number(),
        };
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Publish(ActorSemanticState::Dormant(DormantActorSemanticRecord {
            identity: identity.clone(),
            generation: 0,
          })),
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
        ActorIdentities::<T>::insert(actor_id, &identity);
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if actor_type == ActorType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(actor_id),
          );
          if requested_system_sovereign_id.is_none() {
            if let Err(error) = SystemSovereignCount::<T>::try_mutate(|count| -> DispatchResult {
              *count = count
                .checked_add(1)
                .ok_or(Error::<T>::SystemSovereignCapacityExceeded)?;
              Ok(())
            }) {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::User || requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorCreated {
          actor_id,
          owner,
          actor_class: identity.actor_class,
          mutability: Mutability::Mutable,
          sovereign_account: identity.sovereign_account,
          initial_lifecycle: InitialLifecycle::Dormant,
        });
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_create_user_actor(
      owner: T::AccountId,
      mutability: Mutability,
      preferred_slot: Option<u8>,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        UserCreationEnabled::<T>::get(),
        Error::<T>::UserCreationDisabled
      );
      match contract {
        None => {
          ensure!(
            mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::do_create_dormant_actor(owner, ActorType::User, preferred_slot, None)
        }
        Some(contract) => Self::do_create_actor(
          owner,
          ActorType::User,
          mutability,
          contract,
          preferred_slot,
          None,
        ),
      }
    }

    fn do_create_system_actor(
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      if let Some(sovereign_id) = requested_system_sovereign_id {
        match SystemSovereigns::<T>::get(sovereign_id) {
          Some(SystemSovereignState::Vacant) => {}
          Some(SystemSovereignState::Occupied(_)) => {
            return Err(Error::<T>::SystemSovereignOccupied.into());
          }
          None => {
            return Err(Error::<T>::SystemSovereignUnknown.into());
          }
        }
      }
      match contract {
        None => {
          ensure!(
            mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::do_create_dormant_actor(
            owner,
            ActorType::System,
            None,
            requested_system_sovereign_id,
          )
        }
        Some(contract) => Self::do_create_actor(
          owner,
          ActorType::System,
          mutability,
          contract,
          None,
          requested_system_sovereign_id,
        ),
      }
    }

    fn do_create_actor(
      owner: T::AccountId,
      actor_type: ActorType,
      mutability: Mutability,
      mut contract: ActorContractOf<T>,
      preferred_user_slot: Option<u8>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      Self::canonicalize_preconditions(&mut contract.steps)?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      Self::validate_contract_steps_shape(actor_type, &contract.steps)?;
      Self::validate_unfunded_policy(actor_type, contract.on_unfunded)?;
      Self::validate_close_when(&contract)?;
      Self::validate_parked_balance_activation(&contract)?;
      Self::ensure_retry_later_allowed(mutability, &contract.steps)?;
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(0, target_nonce)?;
      }
      if actor_type == ActorType::System && mutability == Mutability::Immutable {
        ensure!(
          !contract.trigger.manual_source_enabled(),
          Error::<T>::InvalidTriggerConfiguration
        );
      }
      let active_count = Self::active_instance_count();
      ensure!(
        active_count < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      Self::ensure_contract_fits_service_budget(actor_type, &contract)?;
      let actor_id = NextActorId::<T>::get();
      ensure!(
        matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        ),
        Error::<T>::ActorIdOccupied
      );
      if actor_type == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(actor_id);
      if actor_type == ActorType::System {
        match requested_system_sovereign_id {
          Some(_) => match SystemSovereigns::<T>::get(system_sovereign_id) {
            Some(SystemSovereignState::Vacant) => {}
            Some(SystemSovereignState::Occupied(_)) => {
              return Err(Error::<T>::SystemSovereignOccupied.into());
            }
            None => {
              return Err(Error::<T>::SystemSovereignUnknown.into());
            }
          },
          None => {
            ensure!(
              !SystemSovereigns::<T>::contains_key(system_sovereign_id),
              Error::<T>::SystemSovereignOccupied
            );
            ensure!(
              SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
              Error::<T>::SystemSovereignCapacityExceeded
            );
          }
        }
      }
      let prospective_sovereign_account = match actor_type {
        ActorType::User => {
          let owner_slot = Self::available_owner_slot(&owner, preferred_user_slot)?;
          Self::sovereign_account_id(&owner, owner_slot)
        }
        ActorType::System => Self::sovereign_account_id_system(system_sovereign_id),
      };
      Self::validate_recipient_configuration(&contract.steps, &prospective_sovereign_account)?;
      let next_id = actor_id.checked_add(1).ok_or(Error::<T>::ActorIdOverflow)?;
      let now = frame_system::Pallet::<T>::block_number();
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (actor_class, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (ActorClass::User { owner_slot: slot }, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (
              ActorClass::System {
                sovereign_id: system_sovereign_id,
              },
              account,
            ),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        if actor_type == ActorType::User {
          // Creation establishes process state only. Sovereign activation capacity is checked
          // when a Trigger-owned Opening becomes ready.
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
        let temporal_anchor_tick = match Self::temporal_anchor_tick(&contract.trigger) {
          Ok(anchor) => anchor,
          Err(error) => {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Self::placement_error(error),
            ));
          }
        };
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class,
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: now,
        };
        let trigger_runtime_state =
          Self::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          trigger_runtime_state,
          unsuccessful_attempt_streak: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          trigger_wakeup_pointer: None,
          terminal_at: contract
            .window
            .map(|window| Self::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        SovereignIndex::<T>::insert(sovereign_account.clone(), actor_id);
        if let Err(error) = Self::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::CreateActive,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if actor_type == ActorType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(actor_id),
          );
          if requested_system_sovereign_id.is_none() {
            if let Err(error) = SystemSovereignCount::<T>::try_mutate(|count| -> DispatchResult {
              *count = count
                .checked_add(1)
                .ok_or(Error::<T>::SystemSovereignCapacityExceeded)?;
              Ok(())
            }) {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::System && requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        Self::deposit_event(Event::ActorCreated {
          actor_id,
          owner,
          actor_class,
          mutability,
          sovereign_account,
          initial_lifecycle: InitialLifecycle::Active,
        });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_activate_actor(
      actor_id: ActorId,
      mut identity: ActorIdentityOf<T>,
      mut contract: ActorContractOf<T>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        identity.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let actor_type = identity.actor_class.actor_type();
      Self::canonicalize_preconditions(&mut contract.steps)?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      Self::validate_contract_steps_shape(actor_type, &contract.steps)?;
      Self::validate_unfunded_policy(actor_type, contract.on_unfunded)?;
      Self::validate_close_when(&contract)?;
      Self::validate_parked_balance_activation(&contract)?;
      if actor_type == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      Self::validate_recipient_configuration(&contract.steps, &identity.sovereign_account)?;
      Self::ensure_retry_later_allowed(identity.mutability, &contract.steps)?;
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(identity.cycle_nonce, target_nonce)?;
      }
      Self::ensure_contract_fits_service_budget(actor_type, &contract)?;
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      let now = frame_system::Pallet::<T>::block_number();
      ensure!(
        identity.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      let Some(ActorSemanticState::Dormant(dormant_record)) =
        ActorSemanticStates::<T>::get(actor_id)
      else {
        return Err(Error::<T>::ActorNotFound.into());
      };
      ensure!(
        dormant_record.identity == identity,
        Error::<T>::ActorInvariant
      );
      identity.last_control_mutation_block = now;
      // Reactivation anchors the fresh Active epoch at the current block; the fresh hot
      // state has no last_cycle_block, so cooldown/cadence use this conservative anchor
      // rather than block zero (spec 4.3.3).
      let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
      let temporal_anchor_tick =
        Self::temporal_anchor_tick(&contract.trigger).map_err(Self::placement_error)?;
      let trigger_runtime_state =
        Self::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
      let hot = ActorHotState {
        lifecycle: ActiveLifecycle::Active,
        cycle_state: CycleState::Idle,
        trigger_runtime_state,
        unsuccessful_attempt_streak: 0,
        pending_signal: false,
        queue_ticket: None,
        wakeup_pointer: None,
        trigger_wakeup_pointer: None,
        terminal_at: contract
          .window
          .map(|window| Self::window_terminal_at(&window)),
        schedule_anchor,
        last_cycle_block: None,
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if !Self::control_identity_exists(actor_id) || Self::active_actor_exists(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorAlreadyActive.into(),
          ));
        }
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected: ActorSemanticState::Dormant(dormant_record.clone()),
            replacement: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: identity.clone(),
              generation: dormant_record.generation,
            }),
          },
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
        ActorIdentities::<T>::insert(actor_id, &identity);
        if let Err(error) = Self::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::ActivateDormant,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorActivated { actor_id });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_deactivate_actor(actor_id: ActorId, _instance: ActiveActorViewOf<T>) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        // Canonical deactivation owns one removal transition. Publishing an intermediate Idle
        // successor before detaching it would compose two independently validated scheduler
        // transitions inside this transaction and reject the second against provisional state.
        let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id);
        if !canonical {
          if let Err(error) = Self::record_control_mutation_with_authority(actor_id, now) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          if let Err(error) =
            Self::cancel_run_internal(actor_id, CancellationReason::Deactivated, None)
          {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let Some((mut state, admission, _)) = Self::load_frame_actor_service_state(actor_id) else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        if canonical {
          let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
          else {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Error::<T>::ActorInvariant.into(),
            ));
          };
          state.contract = match Self::load_actor_contract(actor_id) {
            Some(contract) => contract,
            None => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                Error::<T>::ActorInvariant.into(),
              ));
            }
          };
          let supplied_run = state.run_state.clone();
          state = match Self::detach_actor_publication(
            ActorRef {
              actor_id,
              generation: record.generation,
            },
            state,
            supplied_run.as_ref(),
          ) {
            Ok(detached) => detached,
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          };
          // Keep stored Run/Hot authority coherent until geometry re-authentication. Only the
          // local Dormant successor advances here; retirement removes the stored Run atomically.
          if let Some(run) = supplied_run {
            state.run_state = None;
            state.hot.cycle_state = CycleState::Idle;
            state.hot.queue_ticket = None;
            state.hot.wakeup_pointer = None;
            state.identity.cycle_nonce = run.cycle_nonce;
            Self::deposit_event(Event::CycleCancelled {
              actor_id,
              cycle_nonce: run.cycle_nonce,
              reason: CancellationReason::Deactivated,
            });
            Self::deposit_event(Event::CycleSummary {
              actor_id,
              cycle_nonce: run.cycle_nonce,
              result: CycleResult::Cancelled,
              outcomes: run.cumulative_outcomes,
            });
          }
          state.identity.last_control_mutation_block = now;
        } else {
          if let Err(error) = Self::remove_actor_from_queues_with_authority(actor_id) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          if state.hot.wakeup_pointer.is_some() {
            let invalidated =
              Self::wakeup_substrate_invalidate_loaded(actor_id, state.clone(), &admission).is_ok();
            if !invalidated {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                Error::<T>::ActorNotFound.into(),
              ));
            }
          }
          if state.hot.trigger_wakeup_pointer.is_some() {
            let invalidated =
              Self::trigger_wakeup_substrate_invalidate_loaded(actor_id, state.clone(), &admission)
                .is_ok();
            if !invalidated {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                Error::<T>::ActorNotFound.into(),
              ));
            }
          }
        }
        if let Err(error) = Self::remove_active_actor_with_admission(actor_id, Some(&admission)) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        let Some(expected) = ActorSemanticStates::<T>::get(actor_id) else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        let ActorSemanticState::Active(ref active_record) = expected else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        let generation = active_record.generation;
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected,
            replacement: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: state.identity.clone(),
              generation,
            }),
          },
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
        ActorIdentities::<T>::insert(actor_id, state.identity.clone());
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorDeactivated { actor_id });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn contract_steps_contains_mint(contract_steps: &ContractSteps<T>) -> bool {
      for step in contract_steps.as_slice() {
        if matches!(step.task, ActorTask::Mint { .. }) {
          return true;
        }
      }
      false
    }

    pub(crate) fn validate_trigger(trigger: &TriggerOf<T>, cooldown_blocks: u32) -> DispatchResult {
      ensure!(
        trigger.has_canonical_filters(),
        Error::<T>::InvalidTriggerConfiguration
      );
      let max_block_delay: u32 = T::MaxExecutionDelayBlocks::get().saturated_into();
      match trigger {
        Trigger::AtTime { after_ticks } => {
          ensure!(*after_ticks > 0, Error::<T>::InvalidTriggerConfiguration);
          ensure!(
            *after_ticks <= T::MaxTemporalDelayTicks::get(),
            Error::<T>::ExecutionDelayTooLong
          );
          ensure!(
            cooldown_blocks == 0,
            Error::<T>::InvalidTriggerConfiguration
          );
        }
        Trigger::Cadenced { every_ticks } => {
          ensure!(*every_ticks > 0, Error::<T>::InvalidTriggerConfiguration);
          ensure!(
            *every_ticks <= T::MaxTemporalDelayTicks::get(),
            Error::<T>::ExecutionDelayTooLong
          );
          ensure!(
            cooldown_blocks == 0,
            Error::<T>::InvalidTriggerConfiguration
          );
        }
        Trigger::Manual | Trigger::AddressEvent { .. } => {}
      }
      ensure!(
        cooldown_blocks <= max_block_delay,
        Error::<T>::ExecutionDelayTooLong
      );
      Ok(())
    }

    fn ensure_auto_close_target(current_cycle_nonce: u64, target_nonce: u64) -> DispatchResult {
      ensure!(
        target_nonce > current_cycle_nonce,
        Error::<T>::InvalidAutoCloseNonce
      );
      let horizon = target_nonce
        .checked_sub(current_cycle_nonce)
        .ok_or(Error::<T>::InvalidAutoCloseNonce)?;
      ensure!(
        horizon <= T::MaxAutoCloseNonceHorizon::get(),
        Error::<T>::AutoCloseNonceHorizonExceeded
      );
      Ok(())
    }

    fn validate_future_schedule_targets(contract: &ActorContractOf<T>) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      let schedule_anchor = contract
        .window
        .map(|window| now.max(window.start))
        .unwrap_or(now);
      ensure!(
        now.checked_add(&One::one()).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      let cooldown: BlockNumberFor<T> = contract.cooldown_blocks.into();
      ensure!(
        schedule_anchor.checked_add(&cooldown).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      if matches!(
        contract.trigger,
        Trigger::AtTime { .. } | Trigger::Cadenced { .. }
      ) {
        ensure!(contract.window.is_none(), Error::<T>::InvalidScheduleWindow);
        return Ok(());
      }
      let first_temporal_eligible = schedule_anchor;
      if let Some(window) = contract.window {
        ensure!(
          first_temporal_eligible <= window.end,
          Error::<T>::InvalidScheduleWindow
        );
      }
      Ok(())
    }

    fn validate_schedule_window(window: &ScheduleWindow<BlockNumberFor<T>>) -> DispatchResult {
      ensure!(window.end > window.start, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.end.checked_add(&One::one()).is_some(),
        Error::<T>::InvalidScheduleWindow
      );
      // Inclusive span: `end - start + 1 >= MinWindowLength` (spec 7.3.2).
      let span = window
        .end
        .checked_sub(&window.start)
        .and_then(|distance| distance.checked_add(&One::one()))
        .ok_or(Error::<T>::InvalidScheduleWindow)?;
      ensure!(
        span >= T::MinWindowLength::get(),
        Error::<T>::InvalidScheduleWindow
      );
      let now = frame_system::Pallet::<T>::block_number();
      // Newly installed Active state requires `end >= current_block`; an in-progress
      // window (`start <= now <= end`) is admissible, and only an already-expired
      // window (`end < now`) is rejected (spec 7.3.3).
      ensure!(window.end >= now, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.start.saturating_sub(now) <= T::MaxExecutionDelayBlocks::get(),
        Error::<T>::ExecutionDelayTooLong
      );
      Ok(())
    }

    pub(crate) fn window_terminal_at(
      window: &ScheduleWindow<BlockNumberFor<T>>,
    ) -> BlockNumberFor<T> {
      window
        .end
        .checked_add(&One::one())
        .expect("admitted schedule windows have an exact terminal block")
    }

    fn ensure_retry_later_allowed(
      mutability: Mutability,
      contract_steps: &ContractSteps<T>,
    ) -> DispatchResult {
      if mutability == Mutability::Immutable {
        for step in contract_steps {
          ensure!(
            step.on_error.retry_max_attempts().is_none(),
            Error::<T>::RetryLaterNotAllowedForImmutableActor
          );
        }
      }
      Ok(())
    }

    fn canonicalize_preconditions(contract_steps: &mut ContractSteps<T>) -> DispatchResult {
      for step in contract_steps.iter_mut() {
        let Some(precondition) = &mut step.precondition else {
          continue;
        };
        let clauses = &mut precondition.clauses;
        ensure!(!clauses.is_empty(), Error::<T>::EmptyPrecondition);
        let mut canonical_clauses = alloc::vec::Vec::with_capacity(clauses.len());
        for clause in clauses.iter() {
          ensure!(!clause.is_empty(), Error::<T>::EmptyPrecondition);
          let mut predicates = clause.to_vec();
          predicates.sort_by_key(Encode::encode);
          predicates.dedup();
          canonical_clauses.push(
            BoundedVec::try_from(predicates).map_err(|_| Error::<T>::AdmissionBoundOverflow)?,
          );
        }
        canonical_clauses.sort_by_key(Encode::encode);
        ensure!(
          !canonical_clauses.windows(2).any(|pair| pair[0] == pair[1]),
          Error::<T>::InvalidPredicate
        );
        let mut absorbed = alloc::vec![false; canonical_clauses.len()];
        for subset_index in 0..canonical_clauses.len() {
          for superset_index in 0..canonical_clauses.len() {
            if subset_index == superset_index
              || canonical_clauses[subset_index].len() >= canonical_clauses[superset_index].len()
            {
              continue;
            }
            if canonical_clauses[subset_index]
              .iter() // deos-bypass: bounded-iter -- MaxPredicateClauses bounds canonical DNF.
              .all(|predicate| canonical_clauses[superset_index].contains(predicate))
            {
              absorbed[superset_index] = true;
            }
          }
        }
        canonical_clauses = canonical_clauses
          .into_iter()
          .zip(absorbed)
          .filter_map(|(clause, is_absorbed)| (!is_absorbed).then_some(clause))
          .collect();
        let predicate_count = canonical_clauses
          .iter() // deos-bypass: bounded-iter -- MaxPredicateClauses bounds canonical DNF.
          .try_fold(0u32, |total, clause| total.checked_add(clause.len() as u32))
          .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        ensure!(
          predicate_count <= T::MaxPredicatesPerStep::get(),
          Error::<T>::AdmissionBoundOverflow
        );
        *clauses = BoundedVec::try_from(canonical_clauses)
          .map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      }
      Ok(())
    }

    fn validate_parked_balance_activation(contract: &ActorContractOf<T>) -> DispatchResult {
      ensure!(
        contract
          .parked_balance_activation
          .as_ref()
          .is_none_or(|activation| {
            activation.is_canonical()
              && activation.watches.len() <= T::MaxParkedBalanceWatches::get() as usize
              && !contract.steps.is_empty()
              && contract.close_when.is_none()
              && contract.auto_close_at_cycle_nonce.is_none()
              && !matches!(contract.trigger, Trigger::Cadenced { .. })
          }),
        Error::<T>::InvalidParkedBalanceActivation
      );
      Ok(())
    }

    fn validate_unfunded_policy(actor_type: ActorType, policy: OnUnfunded) -> DispatchResult {
      if policy == OnUnfunded::Close {
        return Ok(());
      }
      ensure!(
        actor_type == ActorType::User,
        Error::<T>::AwaitFundingNotAllowedForSystemActor
      );
      // Public financial admission requires complete physical and finite wait-work qualification.
      Err(Error::<T>::AwaitFundingUnavailable.into())
    }

    fn validate_close_when(contract: &ActorContractOf<T>) -> DispatchResult {
      let Some(expression) = &contract.close_when else {
        return Ok(());
      };
      Self::validate_parked_balance_activation(contract)?;
      expression
        .validate()
        .map_err(|_| Error::<T>::InvalidCloseWhen)?;
      // Admission requires a qualified Close bound: the policy-bearing Close owner is measured on
      // the maximum policy geometry and the certificate lifecycle maximum covers it, so the bound
      // is available whenever the host catalog is bound. An unavailable bound still refuses here
      // and stays fail-closed for AwaitFunding, whose financial supplement keeps its own port.
      ensure!(
        Self::close_control_weight_for_contract(contract).is_some(),
        Error::<T>::BehaviorPolicyUnavailable
      );
      Ok(())
    }

    fn validate_contract_steps_shape(
      _actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> DispatchResult {
      ensure!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        Error::<T>::TooManyContractSteps
      );
      let mut snapshot_asset = None;
      let mut snapshot_shares = 0u32;
      for step in contract_steps.as_slice() {
        if let Some(max_attempts) = step.on_error.retry_max_attempts() {
          ensure!(
            max_attempts >= 2 && max_attempts <= T::MaxRetryAttempts::get(),
            Error::<T>::InvalidRetryAttemptLimit
          );
        }
        if let Some(precondition) = &step.precondition {
          ensure!(
            !precondition.clauses.is_empty(),
            Error::<T>::EmptyPrecondition
          );
          ensure!(
            precondition.clauses.iter().all(|clause| !clause.is_empty()),
            Error::<T>::EmptyPrecondition
          );
          ensure!(
            precondition.predicate_count() <= T::MaxPredicatesPerStep::get(),
            Error::<T>::AdmissionBoundOverflow
          );
          for predicate in precondition.clauses.iter().flat_map(|clause| clause.iter()) {
            let max_age_blocks = match predicate {
              Predicate::ObservationAbove { max_age_blocks, .. }
              | Predicate::ObservationBelow { max_age_blocks, .. }
              | Predicate::ObservationEquals { max_age_blocks, .. }
              | Predicate::ObservationNotEquals { max_age_blocks, .. } => Some(max_age_blocks),
              _ => None,
            };
            if let Some(max_age_blocks) = max_age_blocks {
              ensure!(*max_age_blocks > 0, Error::<T>::InvalidObservationMaxAge);
            }
          }
        }
        match &step.task {
          ActorTask::Transfer { asset, amount, .. } => match amount {
            AmountResolution::CaptureCycleBalance { percent, share } => {
              ensure!(
                snapshot_asset.is_none() && !percent.is_zero() && !share.is_zero(),
                Error::<T>::InvalidAmountResolution
              );
              snapshot_asset = Some(*asset);
              snapshot_shares = share.deconstruct();
            }
            AmountResolution::CycleBalancePercent(share) => {
              ensure!(
                snapshot_asset == Some(*asset) && !share.is_zero(),
                Error::<T>::InvalidAmountResolution
              );
              snapshot_shares = snapshot_shares
                .checked_add(share.deconstruct())
                .ok_or(Error::<T>::InvalidAmountResolution)?;
              ensure!(
                snapshot_shares <= Perbill::ACCURACY,
                Error::<T>::InvalidAmountResolution
              );
            }
            _ => Self::validate_amount_resolution(amount)?,
          },
          ActorTask::Burn { amount, .. }
          | ActorTask::Mint { amount, .. }
          | ActorTask::Stake { amount, .. } => Self::validate_amount_resolution(amount)?,
          ActorTask::SwapIn {
            asset_in,
            amount_in,
            asset_out,
            ..
          } => {
            ensure!(asset_in != asset_out, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_in)?;
          }
          ActorTask::SwapOut {
            asset_out,
            amount_out,
            asset_in,
            input_limit,
            ..
          } => {
            ensure!(asset_in != asset_out, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_out)?;
            if let InputLimit::Absolute(max_amount_in) = input_limit {
              ensure!(!max_amount_in.is_zero(), Error::<T>::InvalidTradeBound);
            }
          }
          ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a,
            amount_b,
            min_lp_out,
          } => {
            ensure!(asset_a != asset_b, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_a)?;
            Self::validate_amount_resolution(amount_b)?;
            ensure!(!min_lp_out.is_zero(), Error::<T>::InvalidTradeBound);
          }
          ActorTask::RemoveLiquidity {
            lp_amount,
            min_amount_a,
            min_amount_b,
            ..
          } => {
            Self::validate_amount_resolution(lp_amount)?;
            ensure!(
              !min_amount_a.is_zero() && !min_amount_b.is_zero(),
              Error::<T>::InvalidTradeBound
            );
          }
          ActorTask::DonateLiquidity {
            asset_a,
            asset_b,
            max_amount_a,
            ..
          } => {
            ensure!(asset_a != asset_b, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(max_amount_a)?;
          }
          ActorTask::Unstake { shares, .. } => Self::validate_amount_resolution(shares)?,
          ActorTask::StopCycle => {}
        }
      }
      Ok(())
    }

    fn validate_recipient_configuration(
      contract_steps: &ContractSteps<T>,
      sovereign_account: &T::AccountId,
    ) -> DispatchResult {
      for step in contract_steps {
        match &step.task {
          ActorTask::Transfer { to, .. } => {
            ensure!(to != sovereign_account, Error::<T>::SelfTransferNotAllowed);
          }
          _ => {}
        }
      }
      Ok(())
    }

    fn validate_amount_resolution(amount: &AmountResolution<T::Balance>) -> DispatchResult {
      ensure!(
        !matches!(
          amount,
          AmountResolution::CaptureCycleBalance { .. } | AmountResolution::CycleBalancePercent(_)
        ) && !matches!(amount, AmountResolution::Fixed(value) if value.is_zero())
          && !matches!(
            amount,
            AmountResolution::Percent(value) if value.is_zero()
          ),
        Error::<T>::InvalidAmountResolution
      );
      Ok(())
    }

    /// Trusted host hook; the caller authenticates its bounded admission-control delegation.
    pub fn apply_user_creation_enabled(enabled: bool) {
      UserCreationEnabled::<T>::put(enabled);
      Self::deposit_event(Event::UserCreationEnabledSet { enabled });
    }

    /// Trusted host hook; the caller authenticates its bounded freeze/thaw delegation.
    pub fn apply_global_circuit_breaker(paused: bool) {
      GlobalCircuitBreaker::<T>::put(paused);
      if paused {
        let now = frame_system::Pallet::<T>::block_number();
        if let Some(mut state) = CurrentBlockResourceState::<T>::get()
          && state.ensure_block(now).is_ok()
          && !state.optional_actor_work_halted()
        {
          state.halt_optional_actor_work();
          CurrentBlockResourceState::<T>::put(state);
        }
      }
      Self::deposit_event(Event::GlobalCircuitBreakerSet { paused });
    }

    fn ensure_not_system_immutable(instance: &ActiveActorViewOf<T>) -> DispatchResult {
      ensure!(
        !(instance.actor_class.actor_type() == ActorType::System
          && instance.mutability == Mutability::Immutable),
        Error::<T>::ImmutableActor
      );
      Ok(())
    }

    pub(crate) fn ensure_machine_unfrozen() -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      Ok(())
    }

    fn ensure_identity_control_origin(
      origin: OriginFor<T>,
      identity: &ActorIdentityOf<T>,
    ) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == identity.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        identity.actor_class.actor_type() == ActorType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    pub(crate) fn with_reused_transaction(
      operation: impl FnOnce() -> DispatchResult,
    ) -> DispatchResult {
      if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return operation();
      }
      polkadot_sdk::frame_support::storage::with_transaction(|| match operation() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      })
    }

    fn with_control_transaction(operation: impl FnOnce() -> DispatchResult) -> DispatchResult {
      Self::with_reused_transaction(operation)
    }

    fn record_control_mutation_with_authority(
      actor_id: ActorId,
      now: BlockNumberFor<T>,
    ) -> DispatchResult {
      let current = ActorSemanticStates::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      record.identity.last_control_mutation_block = now;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: current,
          replacement: ActorSemanticState::Active(record),
        },
      )
      .map_err(|_| Error::<T>::ActorInvariant)?;
      Ok(())
    }

    fn ensure_control_mutation_allowed(
      instance: &ActiveActorViewOf<T>,
      now: BlockNumberFor<T>,
    ) -> DispatchResult {
      ensure!(
        instance.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      Ok(())
    }

    pub(crate) fn ensure_control_origin(
      origin: OriginFor<T>,
      instance: &ActiveActorViewOf<T>,
    ) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == instance.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        instance.actor_class.actor_type() == ActorType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    fn ensure_system_pause_origin(
      origin: OriginFor<T>,
      instance: &ActiveActorViewOf<T>,
    ) -> DispatchResult {
      Self::ensure_control_origin(origin, instance)?;
      ensure!(
        instance.actor_class.actor_type() == ActorType::System,
        DispatchError::BadOrigin
      );
      Ok(())
    }

    pub(crate) fn remove_owner_slot_binding(
      owner: &T::AccountId,
      owner_slot: u8,
      sovereign: &T::AccountId,
    ) {
      let mut bitmap = OwnerSlotBitmaps::<T>::get(owner);
      Self::clear_owner_slot(&mut bitmap, owner_slot);
      if Self::owner_slot_bitmap_is_empty(&bitmap) {
        OwnerSlotBitmaps::<T>::remove(owner);
      } else {
        OwnerSlotBitmaps::<T>::insert(owner, bitmap);
      }
      SovereignIndex::<T>::remove(sovereign);
    }

    /// Loads public Close authority without mutating lifecycle or custody.
    pub(crate) fn load_close_authority(
      origin: OriginFor<T>,
      actor_id: ActorId,
    ) -> Result<CheckedCloseAuthorityOf<T>, DispatchError> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => {
          let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
          Self::ensure_control_origin(origin, &instance)?;
          ensure!(
            instance.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Ok(CheckedCloseAuthorityOf::Active(instance))
        }
        LoadedActorStateOf::Dormant(identity) => {
          Self::ensure_identity_control_origin(origin, &identity)?;
          ensure!(
            identity.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Ok(CheckedCloseAuthorityOf::Dormant(identity))
        }
        LoadedActorStateOf::NotRegistered => Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => Err(Error::<T>::ActorInvariant.into()),
      }
    }

    /// Reloads finalization authority and identifies canonical publication without removing it.
    pub(crate) fn load_finalization_authority(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
    ) -> Result<(ActiveActorStateOf<T>, Option<ActorRef>), DispatchError> {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let current = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      ensure!(current == *instance, Error::<T>::ActorNotFound);
      // Canonical publication is generation-bound; a legacy primary uses its retained owner.
      if !ActorControlLocators::<T>::contains_key(actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorNotFound.into());
        };
        return Ok((
          state,
          Some(ActorRef {
            actor_id,
            generation: record.generation,
          }),
        ));
      }
      Ok((state, None))
    }

    /// Performs a runtime-owned terminal transition. Public callers enforce origin/mutability;
    /// mandatory terminal outcomes remain effective for Immutable Actors.
    pub(crate) fn finalize_actor(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      let (state, canonical_actor) = Self::load_finalization_authority(actor_id, instance)?;
      if let Some(actor) = canonical_actor {
        let supplied_run = state.run_state.clone();
        return Self::remove_actor_publication_and_finalize(
          actor,
          state,
          supplied_run.as_ref(),
          reason,
        );
      }
      let (_, _, admission) =
        Self::load_control_authority_with_authority(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::finalize_actor_from_retained_state(actor_id, state, &admission, reason)
    }

    fn ensure_consumed_finalization_source(actor_id: ActorId) -> DispatchResult {
      ensure!(
        !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
        Error::<T>::ActorInvariant
      );
      Ok(())
    }

    pub(crate) fn finalize_actor_from_consumed_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      Self::ensure_consumed_finalization_source(actor_id)?;
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_inner(
        actor_id,
        &instance,
        reason,
        crate::execution::LoadedCancellationContext::ConsumedFrame {
          admission: admission.clone(),
          state,
        },
      )
    }

    /// Finalizes a retained/legacy frame after its caller has detached any canonical publication.
    /// Service attempts instead finalize consumed state and retire the process in one transaction.
    pub(crate) fn finalize_actor_from_retained_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_inner(
        actor_id,
        &instance,
        reason,
        crate::execution::LoadedCancellationContext::RetainedFrame {
          admission: admission.clone(),
          state,
        },
      )
    }

    fn finalize_actor_loaded_inner(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
      cancellation_context: crate::execution::LoadedCancellationContext<T>,
    ) -> DispatchResult {
      if instance.has_close_when {
        let contract = match &cancellation_context {
          crate::execution::LoadedCancellationContext::RetainedFrame { state, .. }
          | crate::execution::LoadedCancellationContext::ConsumedFrame { state, .. } => {
            &state.contract
          }
        };
        ensure!(
          Self::close_control_weight_for_contract(contract).is_some(),
          Error::<T>::BehaviorPolicyUnavailable
        );
      }
      if instance.on_unfunded == OnUnfunded::AwaitFunding {
        let contract = match &cancellation_context {
          crate::execution::LoadedCancellationContext::RetainedFrame { state, .. }
          | crate::execution::LoadedCancellationContext::ConsumedFrame { state, .. } => {
            &state.contract
          }
        };
        ensure!(
          Self::close_control_weight_for_contract(contract).is_some(),
          Error::<T>::AwaitFundingUnavailable
        );
      }
      Self::finalize_actor_loaded_state(actor_id, instance, reason, cancellation_context)
    }

    /// Intrinsic loaded destruction after the caller's independent resource qualification.
    fn finalize_actor_loaded_state(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
      mut cancellation_context: crate::execution::LoadedCancellationContext<T>,
    ) -> DispatchResult {
      let admission = cancellation_context.admission().clone();
      ensure!(
        ActiveActorCount::<T>::get() > 0,
        Error::<T>::ActiveActorCountInvariant
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&instance.sovereign_account) == Some(actor_id),
        Error::<T>::ActorNotFound
      );
      if let ActorClass::User { owner_slot } = instance.actor_class {
        ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&instance.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        );
      }
      if let ActorClass::System { sovereign_id } = instance.actor_class {
        // Locator truth: a live System actor must own an occupied locator entry that
        // points back at this actor; any other state is corruption surfaced by the
        // public close path with one exact invariant error.
        ensure!(
          SystemSovereigns::<T>::get(sovereign_id)
            == Some(SystemSovereignState::Occupied(actor_id)),
          Error::<T>::SystemSovereignInvariant
        );
      }
      let identity = ActorIdentity {
        sovereign_account: instance.sovereign_account.clone(),
        owner: instance.owner.clone(),
        actor_class: instance.actor_class,
        mutability: instance.mutability,
        cycle_nonce: instance.cycle_nonce,
        last_control_mutation_block: instance.last_control_mutation_block,
      };

      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| -> DispatchResult {
          let (close_state, close_admission) = match &mut cancellation_context {
            crate::execution::LoadedCancellationContext::RetainedFrame { state, admission }
            | crate::execution::LoadedCancellationContext::ConsumedFrame { state, admission } => {
              (state, admission)
            }
          };
          // A canonically published Actor owns its temporal Trigger residence in the
          // generation-bound `TriggerDeadlineHandles` carrier; only a pre-cutover Actor keeps the
          // legacy waiting-page reference that `invalidate_wakeup_reference` can release.
          if TriggerDeadlineHandles::<T>::contains_key(actor_id) {
            let generation = ActorSemanticStates::<T>::get(actor_id)
              .and_then(|state| match state {
                ActorSemanticState::Active(record) => Some(record.generation),
                ActorSemanticState::Dormant(_) => None,
              })
              .ok_or(Error::<T>::ActorInvariant)?;
            Self::remove_trigger_deadline_member(ActorRef {
              actor_id,
              generation,
            })
            .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.trigger_wakeup_pointer = None;
          } else if let Some(pointer) = close_state.hot.trigger_wakeup_pointer {
            Self::invalidate_wakeup_reference(
              actor_id,
              WakeupPointer {
                block: WakeupKey::Tick(pointer.tick),
                page_id: pointer.page_id,
                slot: pointer.slot,
              },
              close_admission.admission_identity,
            )
            .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.trigger_wakeup_pointer = None;
          }
          if let Some(pointer) = close_state.hot.wakeup_pointer {
            Self::invalidate_wakeup_reference(
              actor_id,
              pointer,
              close_admission.admission_identity,
            )
            .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.wakeup_pointer = None;
          }
          Self::cancel_run_internal_loaded(
            actor_id,
            &identity,
            CancellationReason::Closing(reason),
            None,
            cancellation_context,
          )?;

          // Exact secondary references are gone; remove the sole primary without a population scan.
          Self::remove_active_actor_with_admission(actor_id, Some(&admission))?;
          let semantic_state = ActorSemanticStates::<T>::get(actor_id)
            .filter(|state| matches!(state, ActorSemanticState::Active(_)))
            .ok_or(Error::<T>::ActorInvariant)?;
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Remove {
              expected: semantic_state,
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
            *count = count
              .checked_sub(1)
              .ok_or(Error::<T>::ActiveActorCountInvariant)?;
            Ok(())
          })?;
          ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
            *count = count
              .checked_sub(1)
              .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
            Ok(())
          })?;
          match instance.actor_class {
            ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
              &instance.owner,
              owner_slot,
              &instance.sovereign_account,
            ),
            ActorClass::System { sovereign_id } => {
              SovereignIndex::<T>::remove(&instance.sovereign_account);
              SystemSovereigns::<T>::insert(sovereign_id, SystemSovereignState::Vacant);
            }
          }
          Self::reconcile_actor_state_hold_with_authority(actor_id)?;
          Self::deposit_event(Event::ActorClosed { actor_id, reason });
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

    /// Raw loaded-finalizer measurement, not an admission path or a public Close bound.
    /// Production callers must use the resource-qualified wrapper above.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_finalize_actor_loaded_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> DispatchResult {
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_state(
        actor_id,
        &instance,
        CloseReason::OwnerInitiated,
        crate::execution::LoadedCancellationContext::ConsumedFrame { state, admission },
      )
    }

    /// Measures the consumed-state finalization boundary without supplying a resource price.
    /// Production retains the qualified wrapper; callers must retire Service in the same transaction.
    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn benchmark_finalize_actor_from_consumed_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      Self::ensure_consumed_finalization_source(actor_id)?;
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_state(
        actor_id,
        &instance,
        reason,
        crate::execution::LoadedCancellationContext::ConsumedFrame { state, admission },
      )
    }

    fn close_inactive_actor(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        Self::load_control_identity(actor_id).as_ref() == Some(identity),
        Error::<T>::ActorNotFound
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&identity.sovereign_account) == Some(actor_id),
        Error::<T>::ActorNotFound
      );
      match identity.actor_class {
        ActorClass::User { owner_slot } => ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&identity.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        ),
        ActorClass::System { sovereign_id } => ensure!(
          SystemSovereigns::<T>::get(sovereign_id)
            == Some(SystemSovereignState::Occupied(actor_id)),
          Error::<T>::SystemSovereignInvariant
        ),
      }

      Self::with_control_transaction(|| {
        Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Remove {
            expected: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: identity.clone(),
              generation: ActorSemanticStates::<T>::get(actor_id)
                .and_then(|state| match state {
                  ActorSemanticState::Dormant(record) => Some(record.generation),
                  ActorSemanticState::Active(_) => None,
                })
                .ok_or(Error::<T>::ActorInvariant)?,
            }),
          },
        )
        .map_err(|_| Error::<T>::ActorInvariant)?;
        ActorIdentities::<T>::remove(actor_id);
        ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        })?;
        match identity.actor_class {
          ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
            &identity.owner,
            owner_slot,
            &identity.sovereign_account,
          ),
          ActorClass::System { sovereign_id } => {
            SovereignIndex::<T>::remove(&identity.sovereign_account);
            SystemSovereigns::<T>::insert(sovereign_id, SystemSovereignState::Vacant);
          }
        }
        Self::reconcile_actor_state_hold_with_authority(actor_id)?;
        Self::deposit_event(Event::ActorClosed { actor_id, reason });
        Ok(())
      })
    }

    pub(crate) fn update_idle_starvation_state(_now: BlockNumberFor<T>, starved: bool) {
      let state = IdleStarvationState::<T>::get();
      if !starved {
        if let IdleStarvationPhase::Alerted { consecutive_blocks } = state {
          Self::deposit_event(Event::IdleStarvationRecovered { consecutive_blocks });
        }
        if !matches!(state, IdleStarvationPhase::Healthy) {
          IdleStarvationState::<T>::kill();
        }
        return;
      }
      let consecutive_blocks = match state {
        IdleStarvationPhase::Healthy => 1,
        IdleStarvationPhase::Starving { consecutive_blocks }
        | IdleStarvationPhase::Alerted { consecutive_blocks } => {
          consecutive_blocks.saturating_add(1)
        }
      };
      if consecutive_blocks >= T::MaxIdleStarvationBlocks::get() {
        let first_alert = !matches!(state, IdleStarvationPhase::Alerted { .. });
        IdleStarvationState::<T>::put(IdleStarvationPhase::Alerted { consecutive_blocks });
        if first_alert {
          Self::deposit_event(Event::IdleStarvationDetected { consecutive_blocks });
        }
      } else {
        IdleStarvationState::<T>::put(IdleStarvationPhase::Starving { consecutive_blocks });
      }
    }

    // --- Active Actors Set Operations ---

    pub(crate) fn effective_active_actor_limit() -> u32 {
      ActiveActorLimit::<T>::get()
    }

    pub(crate) fn max_configurable_active_actor_limit() -> u32 {
      T::MaxActiveActors::get().min(T::MaxQueueLength::get())
    }

    pub(crate) fn active_instance_count() -> u32 {
      ActiveActorCount::<T>::get()
    }

    pub(crate) fn remove_actor_from_queues_with_authority(actor_id: ActorId) -> DispatchResult {
      match ActorControlLocators::<T>::get(actor_id) {
        Some(ActorControlLocation::Unsignaled | ActorControlLocation::Waiting { .. }) => Ok(()),
        Some(ActorControlLocation::Ready { .. }) => {
          Self::remove_primary_control_cell_inner(actor_id)
            .map(|_| ())
            .map_err(|_| Error::<T>::ActorInvariant.into())
        }
        None => Err(Error::<T>::ActorInvariant.into()),
      }
    }

    #[cfg(any(feature = "try-runtime", all(test, feature = "runtime-benchmarks")))]
    fn do_try_state_service_ring() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::{BTreeMap, BTreeSet};
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let nodes: BTreeMap<_, _> = ServiceNodes::<T>::iter().collect();
      let header = ServiceHeader::<T>::get();
      if usize::try_from(header.count).ok() != Some(nodes.len())
        || header.count > T::MaxActiveActors::get()
        || header.cursor.is_none() != nodes.is_empty()
      {
        return Err(TryRuntimeError::Other(
          "Service header disagrees with resident cardinality",
        ));
      }
      for (actor_id, node) in &nodes {
        if !matches!(ActorSemanticStates::<T>::get(actor_id),
          Some(ActorSemanticState::Active(record)) if record.generation == node.generation)
          || !ActorProcesses::<T>::get(actor_id).is_some_and(|process| {
            Self::process_has_service_authority(
              ActorRef {
                actor_id: *actor_id,
                generation: node.generation,
              },
              &process,
              node.kind,
            )
          })
        {
          return Err(TryRuntimeError::Other(
            "Service node has no matching active process owner",
          ));
        }
      }
      let Some(first) = header.cursor else {
        return Ok(());
      };
      let first_node = nodes
        .get(&first.actor_id)
        .filter(|node| node.generation == first.generation)
        .ok_or(TryRuntimeError::Other(
          "Service cursor does not resolve to its generation",
        ))?;
      let tail = first_node.previous;
      let mut previous = tail;
      let mut current = first;
      let mut visited = BTreeSet::new();
      for _ in 0..header.count {
        let node = nodes
          .get(&current.actor_id)
          .filter(|node| node.generation == current.generation)
          .ok_or(TryRuntimeError::Other(
            "Service link does not resolve to its generation",
          ))?;
        if !visited.insert(current.actor_id) || node.previous != previous {
          return Err(TryRuntimeError::Other(
            "Service ring repeats a member or has a broken backlink",
          ));
        }
        previous = current;
        current = node.next;
      }
      if current != first || previous != tail {
        return Err(TryRuntimeError::Other(
          "Service ring does not close at its exact cursor",
        ));
      }
      Ok(())
    }

    #[cfg(any(feature = "try-runtime", all(test, feature = "runtime-benchmarks")))]
    fn do_try_state_deadline_carrier() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::{BTreeMap, BTreeSet};
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let mut slots = BTreeMap::new();
      for (trigger, actor_id, handle) in DeadlineHandles::<T>::iter(/* deos-bypass: bounded-iter -- Test/TryRuntime-only carrier audit. */)
        .map(|(id, handle)| (false, id, handle))
        .chain(TriggerDeadlineHandles::<T>::iter().map(|(id, handle)| (true, id, handle)))
      {
        if actor_id != handle.actor.actor_id {
          return Err(TryRuntimeError::Other(
            "Deadline handle disagrees with its Actor map key",
          ));
        }
        let (record, process) = Self::load_canonical_actor_semantic_state(handle.actor)
          .map_err(|_| TryRuntimeError::Other("Deadline handle has no matching active owner"))?;
        if trigger {
          let WakeupKey::Tick(tick) = handle.key else {
            return Err(TryRuntimeError::Other(
              "Trigger Deadline handle is not Tick-owned",
            ));
          };
          if record.hot.trigger_wakeup_pointer
            != Some(TriggerWakeupPointer {
              tick,
              page_id: handle.page,
              slot: u32::from(handle.slot),
            })
          {
            return Err(TryRuntimeError::Other(
              "Trigger Deadline handle disagrees with its hot pointer",
            ));
          }
        } else if !Self::deadline_handle_matches_process(handle, &process) {
          return Err(TryRuntimeError::Other(
            "Deadline handle disagrees with process or Park review residence",
          ));
        }
        // Process/Park deadlines need no hot pointer. Both families still owe distinct slots.
        if slots
          .insert((handle.key, handle.page, handle.slot), handle.actor)
          .is_some()
        {
          return Err(TryRuntimeError::Other(
            "multiple Deadline handles own one physical slot",
          ));
        }
      }
      let headers: BTreeMap<_, _> = DeadlineHeaders::<T>::iter().collect();
      let pages: BTreeMap<_, _> = DeadlinePages::<T>::iter()
        .map(|(key, id, page)| ((key, id), page))
        .collect();
      for ((key, id), page) in &pages {
        let live = page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).filter(|entry| entry.is_some()).count();
        if !headers.contains_key(key)
          || page.entries.len() != 32
          || live == 0
          || live != usize::from(page.live_entries)
        {
          return Err(TryRuntimeError::Other(
            "Deadline page has no header or invalid width/occupancy",
          ));
        }
        for (slot, actor) in
          page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).enumerate()
        {
          if let Some(actor) = actor {
            let slot =
              u8::try_from(slot).map_err(|_| TryRuntimeError::Other("Deadline slot exceeds u8"))?;
            if slots.remove(&(*key, *id, slot)) != Some(*actor) {
              return Err(TryRuntimeError::Other(
                "Deadline slot has no exact reverse handle",
              ));
            }
          }
        }
      }
      if !slots.is_empty() {
        return Err(TryRuntimeError::Other(
          "Deadline handle has no occupied physical slot",
        ));
      }
      let mut visited_pages = BTreeSet::new();
      for (key, header) in &headers {
        Self::validate_deadline_header(header)
          .map_err(|_| TryRuntimeError::Other("Deadline header violates its scalar bounds"))?;
        let mut current = Some(header.first_page);
        let mut previous = None;
        let mut count = 0u32;
        let mut vacant = BTreeSet::new();
        for _ in 0..header.page_count {
          let id = current.ok_or(TryRuntimeError::Other(
            "Deadline page chain ends before its count",
          ))?;
          let page = pages
            .get(&(*key, id))
            .ok_or(TryRuntimeError::Other("Deadline page link has no target"))?;
          if id >= header.next_page
            || !visited_pages.insert((*key, id))
            || page.previous_page != previous
          {
            return Err(TryRuntimeError::Other(
              "Deadline page chain has invalid links or allocation bounds",
            ));
          }
          count = count
            .checked_add(u32::from(page.live_entries))
            .ok_or(TryRuntimeError::Other("Deadline live count overflows"))?;
          if page.live_entries < 32 {
            vacant.insert(id);
          } else if page.previous_vacant_page.is_some() || page.next_vacant_page.is_some() {
            return Err(TryRuntimeError::Other(
              "full Deadline page retains vacancy links",
            ));
          }
          previous = Some(id);
          current = page.next_page;
        }
        if current.is_some() || previous != Some(header.last_page) || count != header.count {
          return Err(TryRuntimeError::Other(
            "Deadline page chain disagrees with header tail/count",
          ));
        }
        current = header.first_vacant_page;
        previous = None;
        // Bound traversal by the actual nonfull pages, not potentially corrupt vacancy links.
        for _ in 0..vacant.len() {
          let id = current.ok_or(TryRuntimeError::Other(
            "Deadline vacancy chain omits a nonfull page",
          ))?;
          let page = pages.get(&(*key, id)).ok_or(TryRuntimeError::Other(
            "Deadline vacancy link has no target",
          ))?;
          if !vacant.remove(&id) || page.previous_vacant_page != previous {
            return Err(TryRuntimeError::Other(
              "Deadline vacancy chain repeats a page or has invalid links",
            ));
          }
          previous = Some(id);
          current = page.next_vacant_page;
        }
        if current.is_some() || !vacant.is_empty() {
          return Err(TryRuntimeError::Other(
            "Deadline vacancy chain is not the exact nonfull-page set",
          ));
        }
      }
      if visited_pages.len() != pages.len() {
        return Err(TryRuntimeError::Other(
          "Deadline page is outside its header chain",
        ));
      }
      let positions: BTreeMap<_, _> = DeadlineIndexPositions::<T>::iter().collect();
      let index_pages: BTreeMap<_, _> = DeadlineIndexPages::<T>::iter()
        .map(|(clock, id, page)| ((clock, id), page))
        .collect();
      if positions.len() != headers.len() {
        return Err(TryRuntimeError::Other(
          "Deadline index positions disagree with header cardinality",
        ));
      }
      let mut expected_pages = BTreeSet::new();
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        let len = DeadlineIndexLen::<T>::get(clock);
        if len > T::MaxActiveActors::get()
          || usize::try_from(len).ok()
            != Some(headers.keys().filter(|key| key.clock() == clock).count())
        {
          return Err(TryRuntimeError::Other(
            "Deadline index length disagrees with its clock's headers",
          ));
        }
        for id in 0..len.div_ceil(32) {
          let coordinate = (clock, u64::from(id));
          let page = index_pages
            .get(&coordinate)
            .ok_or(TryRuntimeError::Other("Deadline index page is missing"))?;
          if page.len() != (len - id * 32).min(32) as usize {
            return Err(TryRuntimeError::Other(
              "Deadline index pages are not packed C32",
            ));
          }
          expected_pages.insert(coordinate);
        }
        for index in 0..len {
          let key = Self::deadline_index_get(clock, index)
            .ok_or(TryRuntimeError::Other("Deadline index entry is missing"))?;
          if key.clock() != clock
            || !headers.contains_key(&key)
            || positions.get(&key) != Some(&index)
          {
            return Err(TryRuntimeError::Other(
              "Deadline index entry has no exact header/reverse position",
            ));
          }
          if index > 0
            && Self::deadline_index_get(clock, (index - 1) / 2).is_none_or(|parent| parent > key)
          {
            return Err(TryRuntimeError::Other(
              "Deadline index violates min-heap ordering",
            ));
          }
        }
      }
      if expected_pages.len() != index_pages.len() {
        return Err(TryRuntimeError::Other("Deadline index has an orphan page"));
      }
      Ok(())
    }

    #[cfg(any(feature = "try-runtime", all(test, feature = "runtime-benchmarks")))]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let limit = Self::effective_active_actor_limit();
      let active_count = Self::active_instance_count();
      let mut semantic_identities = alloc::collections::BTreeMap::new();
      let mut active_actor_ids = Vec::new();
      for (actor_id, state) in ActorSemanticStates::<T>::iter(/* deos-bypass: bounded-iter */) {
        let identity = match &state {
          ActorSemanticState::Dormant(record) => &record.identity,
          ActorSemanticState::Active(record) => &record.identity,
        };
        if semantic_identities
          .insert(actor_id, identity.clone())
          .is_some()
        {
          return Err(TryRuntimeError::Other(
            "multiple canonical semantic states own one Actor identity",
          ));
        }
        match state {
          ActorSemanticState::Active(record) => {
            if ActorControlLocators::<T>::contains_key(actor_id)
              || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
              || Self::load_canonical_actor_semantic_state(ActorRef {
                actor_id,
                generation: record.generation,
              })
              .is_err()
            {
              return Err(TryRuntimeError::Other(
                "active semantic Actor retains legacy authority or has corrupt canonical placement",
              ));
            }
            active_actor_ids.push(actor_id);
          }
          ActorSemanticState::Dormant(record) => {
            if ActorIdentities::<T>::get(actor_id) != Some(record.identity)
              || ActorProcesses::<T>::contains_key(actor_id)
              || ActorControlLocators::<T>::contains_key(actor_id)
              || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
            {
              return Err(TryRuntimeError::Other(
                "dormant semantic Actor disagrees with its scalar identity or retains process authority",
              ));
            }
          }
        }
      }
      // Forward checks above validate active/disabled processes and reject dormant ownership.
      // The inverse check also catches close tombstones after their semantic identity is gone.
      for actor_id in ActorProcesses::<T>::iter_keys() {
        if !semantic_identities.contains_key(&actor_id) {
          return Err(TryRuntimeError::Other(
            "canonical process has no semantic Actor owner",
          ));
        }
      }
      for (actor_id, episode) in ParkedBalanceEpisodes::<T>::iter(
        /* deos-bypass: bounded-iter -- at most one episode per MaxActiveActors-bounded active Actor. */
      ) {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(TryRuntimeError::Other(
            "parked-balance episode has no active semantic owner",
          ));
        };
        if episode.owner.actor
          != (ActorRef {
            actor_id,
            generation: record.generation,
          })
          || Self::load_canonical_actor_semantic_state(episode.owner.actor).is_err()
        {
          return Err(TryRuntimeError::Other(
            "parked-balance episode disagrees with its generation-bound Park owner",
          ));
        }
      }
      for (actor_id, episode) in ActorFinancialWaits::<T>::iter(
        /* deos-bypass: bounded-iter -- at most one companion per MaxActiveActors-bounded process. */
      ) {
        if episode.record().scope().actor.actor_id != actor_id
          || ActorProcesses::<T>::get(actor_id)
            .is_none_or(|process| process.status != ProcessStatus::AwaitingFunding)
          || Self::load_canonical_actor_semantic_state(episode.record().scope().actor).is_err()
        {
          return Err(TryRuntimeError::Other(
            "financial wait has no matching process/Deadline source",
          ));
        }
      }
      let actual_active_count = u32::try_from(active_actor_ids.len())
        .map_err(|_| TryRuntimeError::Other("active semantic Actor count exceeds u32"))?;
      if T::MaxOwnerSlots::get() == 0 {
        return Err(TryRuntimeError::Other("MaxOwnerSlots must be nonzero"));
      }
      if !contract_steps_bound_is_valid(T::MaxContractSteps::get()) {
        return Err(TryRuntimeError::Other(
          "MaxContractSteps must be in 1..=255",
        ));
      }
      if limit == 0 || limit > Self::max_configurable_active_actor_limit() {
        return Err(TryRuntimeError::Other(
          "ActiveActorLimit is outside the configured bounds",
        ));
      }
      if active_count != actual_active_count {
        return Err(TryRuntimeError::Other(
          "ActiveActorCount does not match active semantic Actor cardinality",
        ));
      }
      if active_count > limit {
        return Err(TryRuntimeError::Other(
          "active semantic Actor count exceeds effective active actor limit",
        ));
      }
      for actor_id in ActorIdentities::<T>::iter_keys() {
        if !matches!(
          ActorSemanticStates::<T>::get(actor_id),
          Some(ActorSemanticState::Dormant(_))
        ) {
          return Err(TryRuntimeError::Other(
            "scalar ActorIdentity is not the mirror of a dormant semantic Actor",
          ));
        }
      }
      let actual_identity_count = u32::try_from(semantic_identities.len())
        .map_err(|_| TryRuntimeError::Other("semantic Actor identity count exceeds u32"))?;
      let identity_count = ActorIdentityCount::<T>::get();
      if identity_count != actual_identity_count {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount does not match active primaries plus dormant identities",
        ));
      }
      if identity_count > T::MaxActorIdentities::get() {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount exceeds MaxActorIdentities",
        ));
      }
      for actor_id in ActorContractHeads::<T>::iter_keys() {
        if !matches!(
          Self::load_actor_state_for_frame_control(actor_id),
          LoadedActorStateOf::Active(_)
        ) {
          return Err(TryRuntimeError::Other(
            "ActorContract entry belongs to a corrupt actor partition set",
          ));
        }
      }
      for (actor_id, chunk_index) in ActorContractTailChunks::<T>::iter_keys() {
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ActorContract tail has no Contract head owner",
        ))?;
        let tail_count = head
          .header
          .step_count
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
        if chunk_index >= tail_count {
          return Err(TryRuntimeError::Other(
            "ActorContract tail key exceeds the declared Contract geometry",
          ));
        }
      }
      for actor_id in ActorBehaviorPolicies::<T>::iter_keys() {
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "Actor behavior policy has no Contract head owner",
        ))?;
        if !head.header.has_close_when
          || Self::load_contract_behavior_policy(actor_id, &head.header).is_none()
        {
          return Err(TryRuntimeError::Other(
            "Actor behavior policy disagrees with its Contract head",
          ));
        }
      }
      for actor_id in ActorBehaviorMonitors::<T>::iter_keys() {
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "Actor behavior monitor has no Contract head owner",
        ))?;
        let policy = Self::load_contract_behavior_policy(actor_id, &head.header)
          .flatten()
          .ok_or(TryRuntimeError::Other(
            "Actor behavior monitor has no authenticated policy owner",
          ))?;
        if !Self::behavior_monitor_required(&policy.plan)
          || Self::load_contract_behavior_monitor(actor_id, &head.header, &policy).is_none()
        {
          return Err(TryRuntimeError::Other(
            "Actor behavior monitor disagrees with its episode or Cycle",
          ));
        }
      }
      let mut max_id: Option<ActorId> = None;
      for actor_id in active_actor_ids {
        let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
        else {
          return Err(TryRuntimeError::Other(
            "active semantic Actor belongs to a corrupt canonical partition set",
          ));
        };
        if !matches!(
          ActorSemanticStates::<T>::get(actor_id),
          Some(ActorSemanticState::Active(_))
        ) {
          return Err(TryRuntimeError::Other(
            "active Actor has no canonical admission authority",
          ));
        }
        let identity = state.identity;
        if identity.last_control_mutation_block > frame_system::Pallet::<T>::block_number() {
          return Err(TryRuntimeError::Other(
            "actor control mutation block is in the future",
          ));
        }
        let hot = state.hot;
        let contract = state.contract;
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "Active actor has no C6 Contract head",
        ))?;
        let resources = Self::derive_step_resource_envelopes(&contract).ok_or(
          TryRuntimeError::Other("Active actor Step resources cannot be rederived"),
        )?;
        let expected_pipeline_machine_envelope = Self::derive_pipeline_machine_envelope(
          identity.actor_class.actor_type(),
          contract.on_unfunded,
          &contract.steps,
          &resources,
          if contract.steps.is_empty() {
            Self::zero_step_resource_envelope(&contract)
              .ok_or(TryRuntimeError::Other(
                "Active actor zero-Step control cannot be rederived",
              ))?
              .control
          } else {
            Weight::zero()
          },
          Self::policy_service_inspection_weight(&contract).ok_or(TryRuntimeError::Other(
            "Active actor policy inspection control cannot be rederived",
          ))?,
        )
        .map_err(|_| {
          TryRuntimeError::Other("Active actor Pipeline Machine envelope cannot be rederived")
        })?;
        if head.header.pipeline_machine_envelope != expected_pipeline_machine_envelope {
          return Err(TryRuntimeError::Other(
            "Active actor Pipeline Machine envelope disagrees with canonical Step control resources",
          ));
        }
        // Terminal membership is derived from the schedule window: `terminal_at` is the sole
        // terminal-membership authority and must equal the window's exact terminal block, or be
        // absent without a window.
        let program_window = contract.window;
        let expected_terminal_at = program_window.map(|window| Self::window_terminal_at(&window));
        if hot.terminal_at != expected_terminal_at {
          return Err(TryRuntimeError::Other(
            "ActorHot terminal_at disagrees with schedule window terminal membership",
          ));
        }
        if hot.lifecycle.is_paused() && hot.trigger_wakeup_pointer.is_some() {
          return Err(TryRuntimeError::Other(
            "paused actor retains Trigger temporal membership",
          ));
        }
        match hot.trigger_runtime_state {
          TriggerRuntimeState::AtTime { consumed: true, .. }
            if hot.trigger_wakeup_pointer.is_some() =>
          {
            return Err(TryRuntimeError::Other(
              "consumed AtTime actor retains Trigger temporal membership",
            ));
          }
          TriggerRuntimeState::AtTime {
            consumed: false, ..
          }
          | TriggerRuntimeState::Cadenced { .. }
            if !hot.lifecycle.is_paused()
              && !hot.pending_signal
              && hot.trigger_wakeup_pointer.is_none()
              && ActorProcesses::<T>::get(actor_id)
                .is_none_or(|process| process.status != ProcessStatus::AwaitingFunding) =>
          {
            return Err(TryRuntimeError::Other(
              "unlatched temporal actor has no Trigger temporal membership",
            ));
          }
          TriggerRuntimeState::Stateless if hot.trigger_wakeup_pointer.is_some() => {
            return Err(TryRuntimeError::Other(
              "non-temporal actor retains Trigger temporal membership",
            ));
          }
          _ => {}
        }
        if Self::ensure_contract_fits_service_budget(identity.actor_class.actor_type(), &contract)
          .is_err()
        {
          return Err(TryRuntimeError::Other(
            "active Actor Contract exceeds current actor-service envelope",
          ));
        }
        let instance = Self::derive_active_actor_view(identity, hot, contract);
        max_id = Some(max_id.map_or(actor_id, |prev| prev.max(actor_id)));
        match SovereignIndex::<T>::get(&instance.sovereign_account) {
          Some(mapped_id) if mapped_id == actor_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "SovereignIndex does not map sovereign_account back to actor_id",
            ));
          }
        }
        match instance.actor_class {
          ActorClass::User { owner_slot } => {
            if owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "User Actors owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let bitmap = OwnerSlotBitmaps::<T>::get(&instance.owner);
            if !Self::owner_slot_bitmap_is_valid(&bitmap)
              || !Self::owner_slot_is_set(&bitmap, owner_slot)
            {
              return Err(TryRuntimeError::Other(
                "User Actors owner_slot is missing from OwnerSlotBitmaps",
              ));
            }
          }
          ActorClass::System { sovereign_id }
            if SystemSovereigns::<T>::get(sovereign_id)
              != Some(SystemSovereignState::Occupied(actor_id)) =>
          {
            return Err(TryRuntimeError::Other(
              "active System Actor disagrees with its sovereign locator",
            ));
          }
          ActorClass::System { .. } => {}
        }
      }
      for actor_id in ActorRunStateStore::<T>::iter_keys() {
        let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
        else {
          return Err(TryRuntimeError::Other(
            "ActorRunState entry belongs to a corrupt actor partition set",
          ));
        };
        let run_state = state.run_state.ok_or(TryRuntimeError::Other(
          "ActorRunState key is absent from loaded Active state",
        ))?;
        let hot = state.hot;
        let identity = state.identity;
        let contract = state.contract;
        if !matches!(hot.cycle_state, CycleState::Running | CycleState::Suspended)
          || identity.cycle_nonce.checked_add(1) != Some(run_state.cycle_nonce)
          || run_state.cursor >= contract.steps.len() as u32
        {
          return Err(TryRuntimeError::Other(
            "ActorRunState violates run marker, nonce, mutability, or cursor bounds",
          ));
        }
        match hot.cycle_state {
          CycleState::Suspended => {
            if identity.mutability != Mutability::Mutable || !run_state.suspension_is_coherent() {
              return Err(TryRuntimeError::Other(
                "Suspended ActorRunState has incoherent outcome or suspension authority",
              ));
            }
            let max_attempts = contract.steps[run_state.cursor as usize]
              .on_error
              .retry_max_attempts()
              .ok_or(TryRuntimeError::Other(
                "ActorRunState cursor does not own RetryLater",
              ))?;
            if run_state.unsuccessful_attempts_at_cursor == 0
              || run_state.unsuccessful_attempts_at_cursor >= max_attempts
            {
              return Err(TryRuntimeError::Other(
                "ActorRunState cursor-local attempt count is outside its live range",
              ));
            }
            let expected_eligible_at = Self::suspension_eligible_at(
              contract.cooldown_blocks,
              contract.window,
              run_state.last_attempt_block,
              run_state.unsuccessful_attempts_at_cursor,
            )
            .map_err(|_| {
              TryRuntimeError::Other("ActorRunState retry eligibility is unrepresentable")
            })?;
            if run_state.eligible_at != expected_eligible_at {
              return Err(TryRuntimeError::Other(
                "ActorRunState retry eligibility disagrees with its suspension facts",
              ));
            }
          }
          CycleState::Running => {
            if run_state.unsuccessful_attempts_at_cursor != 0 || !run_state.running_is_coherent() {
              return Err(TryRuntimeError::Other(
                "Running ActorRunState lacks a causal committed-Step boundary",
              ));
            }
          }
          CycleState::Idle => {
            return Err(TryRuntimeError::Other(
              "Idle Actor cannot retain ActorRunState",
            ));
          }
        }
      }
      for (actor_id, identity) in &semantic_identities {
        if !matches!(
          ActorSemanticStates::<T>::get(actor_id),
          Some(ActorSemanticState::Dormant(_))
        ) {
          continue;
        }
        if identity.last_control_mutation_block > frame_system::Pallet::<T>::block_number() {
          return Err(TryRuntimeError::Other(
            "actor control mutation block is in the future",
          ));
        }
        max_id = Some(max_id.map_or(*actor_id, |prev| prev.max(*actor_id)));
        if ActorRunStateStore::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "Dormant identity owns active scheduler or readiness state",
          ));
        }
        match SovereignIndex::<T>::get(&identity.sovereign_account) {
          Some(mapped_id) if mapped_id == *actor_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "Dormant SovereignIndex does not map sovereign_account back to actor_id",
            ));
          }
        }
        match identity.actor_class {
          ActorClass::User { owner_slot } => {
            if owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "Dormant User Actors owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let bitmap = OwnerSlotBitmaps::<T>::get(&identity.owner);
            if !Self::owner_slot_bitmap_is_valid(&bitmap)
              || !Self::owner_slot_is_set(&bitmap, owner_slot)
            {
              return Err(TryRuntimeError::Other(
                "Dormant User Actors owner_slot is missing from OwnerSlotBitmaps",
              ));
            }
          }
          ActorClass::System { sovereign_id } => {
            if identity.mutability == Mutability::Immutable
              && !T::GenesisSystemActors::dormant_system_actors()
                .into_iter()
                .any(|(genesis_id, owner, mutability)| {
                  genesis_id == *actor_id
                    && genesis_id == sovereign_id
                    && owner == identity.owner
                    && mutability == Mutability::Immutable
                })
            {
              return Err(TryRuntimeError::Other(
                "Immutable Dormant System Actor is not declared by genesis",
              ));
            }
            if SystemSovereigns::<T>::get(sovereign_id)
              != Some(SystemSovereignState::Occupied(*actor_id))
            {
              return Err(TryRuntimeError::Other(
                "dormant System Actor disagrees with its sovereign locator",
              ));
            }
          }
        }
      }
      for owner in OwnerSlotBitmaps::<T>::iter_keys() {
        let bitmap = OwnerSlotBitmaps::<T>::get(&owner);
        if !Self::owner_slot_bitmap_is_valid(&bitmap) || Self::owner_slot_bitmap_is_empty(&bitmap) {
          return Err(TryRuntimeError::Other(
            "OwnerSlotBitmaps contains an invalid or empty bitmap",
          ));
        }
        for owner_slot in 0..T::MaxOwnerSlots::get() {
          if !Self::owner_slot_is_set(&bitmap, owner_slot) {
            continue;
          }
          let sovereign = Self::sovereign_account_id(&owner, owner_slot);
          let Some(actor_id) = SovereignIndex::<T>::get(&sovereign) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no SovereignIndex owner",
            ));
          };
          let Some(identity) = semantic_identities.get(&actor_id) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no semantic Actor identity owner",
            ));
          };
          if identity.owner != owner
            || identity.actor_class != (ActorClass::User { owner_slot })
            || identity.sovereign_account != sovereign
          {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit disagrees with ActorIdentity",
            ));
          }
        }
      }
      let actual_system_sovereign_count = u32::try_from(SystemSovereigns::<T>::iter_keys().count())
        .map_err(|_| TryRuntimeError::Other("SystemSovereigns cardinality exceeds u32"))?;
      if SystemSovereignCount::<T>::get() != actual_system_sovereign_count {
        return Err(TryRuntimeError::Other(
          "SystemSovereignCount does not match SystemSovereigns cardinality",
        ));
      }
      if actual_system_sovereign_count > T::MaxSystemSovereigns::get() {
        return Err(TryRuntimeError::Other(
          "SystemSovereigns cardinality exceeds MaxSystemSovereigns",
        ));
      }
      let mut system_identity_owners = alloc::collections::BTreeMap::new();
      for (actor_id, identity) in &semantic_identities {
        if let ActorClass::System { sovereign_id } = identity.actor_class
          && system_identity_owners
            .insert(sovereign_id, *actor_id)
            .is_some()
        {
          return Err(TryRuntimeError::Other(
            "multiple System Actor identities own one sovereign locator",
          ));
        }
      }
      let mut derived_system_accounts = alloc::collections::BTreeSet::new();
      for (sovereign_id, locator_state) in SystemSovereigns::<T>::iter() {
        let sovereign_account = Self::sovereign_account_id_system(sovereign_id);
        if !derived_system_accounts.insert(sovereign_account.clone()) {
          return Err(TryRuntimeError::Other(
            "System sovereign locators derive a duplicate custody account",
          ));
        }
        match locator_state {
          SystemSovereignState::Vacant => {
            if system_identity_owners.contains_key(&sovereign_id)
              || SovereignIndex::<T>::contains_key(&sovereign_account)
            {
              return Err(TryRuntimeError::Other(
                "vacant System sovereign locator retains identity ownership",
              ));
            }
          }
          SystemSovereignState::Occupied(actor_id) => {
            if system_identity_owners.get(&sovereign_id) != Some(&actor_id) {
              return Err(TryRuntimeError::Other(
                "occupied System sovereign locator disagrees with identity ownership",
              ));
            }
            let identity = semantic_identities
              .get(&actor_id)
              .ok_or(TryRuntimeError::Other(
                "occupied System sovereign locator has no semantic Actor identity",
              ))?;
            if identity.sovereign_account != sovereign_account
              || SovereignIndex::<T>::get(&sovereign_account) != Some(actor_id)
            {
              return Err(TryRuntimeError::Other(
                "occupied System sovereign locator disagrees with derived custody ownership",
              ));
            }
          }
        }
      }
      if system_identity_owners.len() > actual_system_sovereign_count as usize {
        return Err(TryRuntimeError::Other(
          "System Actor identity has no sovereign locator",
        ));
      }
      let mut sovereign_index_count = 0u32;
      for (sovereign_account, actor_id) in SovereignIndex::<T>::iter() {
        sovereign_index_count =
          sovereign_index_count
            .checked_add(1)
            .ok_or(TryRuntimeError::Other(
              "SovereignIndex cardinality exceeds u32",
            ))?;
        let identity = semantic_identities
          .get(&actor_id)
          .ok_or(TryRuntimeError::Other(
            "SovereignIndex owner has no semantic Actor identity",
          ))?;
        if identity.sovereign_account != sovereign_account {
          return Err(TryRuntimeError::Other(
            "SovereignIndex key disagrees with ActorIdentity custody account",
          ));
        }
      }
      if sovereign_index_count != identity_count {
        return Err(TryRuntimeError::Other(
          "SovereignIndex cardinality does not match semantic Actor identities",
        ));
      }

      let queue_capacity = T::MaxQueueLength::get();
      if queue_capacity < limit {
        return Err(TryRuntimeError::Other(
          "MaxQueueLength is below effective active actor limit",
        ));
      }
      let queue_occupancy = ActorReadyOccupancy::<T>::get();
      if queue_occupancy > queue_capacity {
        return Err(TryRuntimeError::Other(
          "canonical queue physical occupancy exceeds MaxQueueLength",
        ));
      }
      const CONTROL_PAGE_SIZE: u64 = 32;
      let head = ActorReadyHead::<T>::get();
      let tail = ActorReadyTail::<T>::get();
      let span = tail
        .checked_sub(head)
        .ok_or(TryRuntimeError::Other("canonical Ready head exceeds tail"))?;
      if span > u64::from(queue_capacity)
        || queue_occupancy > active_count
        || u64::from(queue_occupancy) > span
      {
        return Err(TryRuntimeError::Other(
          "canonical Ready live count or physical span exceeds its bound",
        ));
      }
      let mut ready_pages = alloc::collections::BTreeSet::new();
      let mut ready_live = 0u32;
      for (page_id, page) in ActorReadyFrameChunks::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        let start = page_id
          .checked_mul(CONTROL_PAGE_SIZE)
          .ok_or(TryRuntimeError::Other(
            "canonical Ready page range overflows",
          ))?;
        if page.len() != CONTROL_PAGE_SIZE as usize
          || start >= tail
          || start.saturating_add(CONTROL_PAGE_SIZE) <= head
          || !ready_pages.insert(page_id)
        {
          return Err(TryRuntimeError::Other(
            "canonical Ready page has invalid width or range",
          ));
        }
        for (slot, cell) in page.iter(/* deos-bypass: bounded-iter -- fixed C32. */).enumerate() {
          let Some(cell) = cell else { continue };
          let ticket = start
            .checked_add(slot as u64)
            .ok_or(TryRuntimeError::Other("canonical Ready ticket overflows"))?;
          let location = ActorControlLocation::Ready { ticket };
          if ticket < head
            || ticket >= tail
            || ActorControlLocators::<T>::get(cell.actor_id) != Some(location)
            || Self::project_control_cell(cell, location).is_none()
          {
            return Err(TryRuntimeError::Other(
              "canonical Ready primary disagrees with its ticket or locator",
            ));
          }
          ready_live = ready_live.checked_add(1).ok_or(TryRuntimeError::Other(
            "canonical Ready live count overflows",
          ))?;
        }
      }
      let expected_ready_pages = if head == tail {
        0
      } else {
        (tail - 1) / CONTROL_PAGE_SIZE - head / CONTROL_PAGE_SIZE + 1
      };
      if ready_pages.len() as u64 != expected_ready_pages || ready_live != queue_occupancy {
        return Err(TryRuntimeError::Other(
          "canonical Ready pages or live occupancy disagree",
        ));
      }
      // Empty slots inside the bounded [head, tail) span are FIFO tombstones, not live occupancy.
      if T::WakeupPageSize::get() == 0 {
        return Err(TryRuntimeError::Other("WakeupPageSize must be non-zero"));
      }
      let mut waiting_pages = alloc::collections::BTreeMap::new();
      let mut waiting_live = alloc::collections::BTreeMap::new();
      let mut reference_count = 0u64;
      for ((key, page_id), page) in ActorWaitingFrameChunks::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        if page.entries.len() != CONTROL_PAGE_SIZE as usize
          || page.scan_slot >= CONTROL_PAGE_SIZE as u32
          || page
            .entries
            .iter(/* deos-bypass: bounded-iter -- fixed C32. */)
            .take(page.scan_slot as usize)
            .any(Option::is_some)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting page has invalid width or scan cursor",
          ));
        }
        let actual_live = page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).filter(|entry| entry.is_some()).count()
          as u32;
        if actual_live == 0 || actual_live != page.live_entries {
          return Err(TryRuntimeError::Other(
            "canonical Waiting page live count disagrees with slots",
          ));
        }
        let count = waiting_live.entry(key).or_insert(0u32);
        *count = count
          .checked_add(actual_live)
          .ok_or(TryRuntimeError::Other(
            "canonical Waiting per-key count overflows",
          ))?;
        for (slot, entry) in
          page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).enumerate()
        {
          let Some(entry) = entry else { continue };
          let pointer = WakeupPointer {
            block: key,
            page_id,
            slot: slot as u32,
          };
          let (actor_id, primary) = match entry {
            ActorWaitingEntry::Primary(cell) => {
              let location = ActorControlLocation::Waiting {
                key,
                page: page_id,
                slot: slot as u8,
              };
              if ActorControlLocators::<T>::get(cell.actor_id) != Some(location)
                || Self::project_control_cell(cell, location).is_none()
              {
                return Err(TryRuntimeError::Other(
                  "canonical Waiting primary disagrees with its locator",
                ));
              }
              (cell.actor_id, cell.clone())
            }
            ActorWaitingEntry::Reference(reference) => {
              let (_, primary) =
                Self::load_primary_control_cell(reference.actor_id).map_err(|_| {
                  TryRuntimeError::Other("canonical Waiting reference has no primary")
                })?;
              if primary.admission.admission_identity != reference.admission_identity {
                return Err(TryRuntimeError::Other(
                  "canonical Waiting reference admission identity disagrees",
                ));
              }
              reference_count = reference_count
                .checked_add(1)
                .ok_or(TryRuntimeError::Other(
                  "canonical Waiting reference count overflows",
                ))?;
              (reference.actor_id, primary)
            }
          };
          let authoritative = match key {
            WakeupKey::Block(_) => primary.hot.wakeup_pointer,
            WakeupKey::Tick(_) => primary
              .hot
              .trigger_wakeup_pointer
              .map(|trigger| WakeupPointer {
                block: WakeupKey::Tick(trigger.tick),
                page_id: trigger.page_id,
                slot: trigger.slot,
              }),
          };
          if primary.actor_id != actor_id || authoritative != Some(pointer) {
            return Err(TryRuntimeError::Other(
              "WakeupPage slot addresses an actor with a different clock-domain pointer",
            ));
          }
        }
        if waiting_pages.insert((key, page_id), page).is_some() {
          return Err(TryRuntimeError::Other("duplicate canonical Waiting page"));
        }
      }
      if u64::from(active_count)
        .checked_add(reference_count)
        .is_none_or(|count| count > u64::from(active_count) * 3)
      {
        return Err(TryRuntimeError::Other(
          "canonical primary plus reference cardinality exceeds 3N",
        ));
      }
      let mut directory_keys = alloc::collections::BTreeSet::new();
      let mut visited_pages = alloc::collections::BTreeSet::new();
      for (key, occupancy) in ActorWaitingOccupancies::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        if occupancy == 0
          || occupancy > T::MaxActiveActors::get()
          || waiting_live.get(&key).copied() != Some(occupancy)
          || !ActorWaitingHeads::<T>::contains_key(key)
          || !ActorWaitingTails::<T>::contains_key(key)
          || !directory_keys.insert(key)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory occupancy disagrees with pages",
          ));
        }
        let head = ActorWaitingHeads::<T>::get(key);
        let tail = ActorWaitingTails::<T>::get(key);
        if head >= tail {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory has invalid range",
          ));
        }
        let first = head / CONTROL_PAGE_SIZE;
        let last = (tail - 1) / CONTROL_PAGE_SIZE;
        let mut page_id = first;
        let mut previous = None;
        loop {
          if !visited_pages.insert((key, page_id)) {
            return Err(TryRuntimeError::Other("canonical Waiting page links cycle"));
          }
          let page = waiting_pages
            .get(&(key, page_id))
            .ok_or(TryRuntimeError::Other(
              "canonical Waiting linked page is missing",
            ))?;
          if page.previous_page != previous
            || page_id > last
            || (page_id == first && head % CONTROL_PAGE_SIZE != u64::from(page.scan_slot))
            || (page_id != first && page.scan_slot != 0)
          {
            return Err(TryRuntimeError::Other(
              "canonical Waiting page links or head cursor disagree",
            ));
          }
          if page_id == last {
            let tail_slot = ((tail - 1) % CONTROL_PAGE_SIZE + 1) as usize;
            if page.next_page.is_some()
              || page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).skip(tail_slot).any(Option::is_some)
            {
              return Err(TryRuntimeError::Other(
                "canonical Waiting tail page exceeds directory range",
              ));
            }
            break;
          }
          let next = page.next_page.ok_or(TryRuntimeError::Other(
            "canonical Waiting linked chain ends before tail",
          ))?;
          if next <= page_id {
            return Err(TryRuntimeError::Other(
              "canonical Waiting links are not strictly ordered",
            ));
          }
          previous = Some(page_id);
          page_id = next;
        }
        let index = ActorWaitingCursorIndices::<T>::get(key).ok_or(TryRuntimeError::Other(
          "canonical Waiting key has no shared cursor index",
        ))?;
        if index >= WakeupCursorLen::<T>::get(key.clock())
          || Self::wakeup_cursor_get(key.clock(), index) != Some(key)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting cursor reverse index does not resolve",
          ));
        }
      }
      if visited_pages.len() != waiting_pages.len() || directory_keys.len() != waiting_live.len() {
        return Err(TryRuntimeError::Other(
          "canonical Waiting pages are disconnected or have no directory",
        ));
      }
      for key in ActorWaitingHeads::<T>::iter_keys()
        .chain(ActorWaitingTails::<T>::iter_keys())
        .chain(ActorWaitingCursorIndices::<T>::iter_keys())
      {
        if !directory_keys.contains(&key) {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory contains an orphan key",
          ));
        }
      }
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        if directory_keys
          .iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
          .filter(|key| key.clock() == clock)
          .count() as u64
          != u64::from(WakeupCursorLen::<T>::get(clock))
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory and heap cardinality disagree",
          ));
        }
      }
      let cursor_page_size = T::WakeupPageSize::get();
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        let cursor_len = WakeupCursorLen::<T>::get(clock);
        if cursor_len > T::MaxActiveActors::get() {
          return Err(TryRuntimeError::Other(
            "WakeupCursorLen exceeds configured active actor capacity",
          ));
        }
        let expected_cursor_pages = cursor_len.div_ceil(cursor_page_size);
        let actual_cursor_pages = u32::try_from(
          WakeupCursorPages::<T>::iter_keys()
            .filter(|(stored_clock, _)| *stored_clock == clock)
            .count(),
        )
        .map_err(|_| TryRuntimeError::Other("wakeup cursor page count overflows"))?;
        if actual_cursor_pages != expected_cursor_pages {
          return Err(TryRuntimeError::Other(
            "WakeupCursorPages count disagrees with cursor length",
          ));
        }
        for page_id in 0..expected_cursor_pages {
          let Some(page) = WakeupCursorPages::<T>::get((clock, u64::from(page_id))) else {
            return Err(TryRuntimeError::Other(
              "WakeupCursorPages has a gap in logical page order",
            ));
          };
          let consumed = page_id
            .checked_mul(cursor_page_size)
            .ok_or(TryRuntimeError::Other(
              "wakeup cursor page offset overflows",
            ))?;
          let remaining = cursor_len
            .checked_sub(consumed)
            .ok_or(TryRuntimeError::Other(
              "wakeup cursor page offset exceeds length",
            ))?;
          let expected_len = remaining.min(cursor_page_size) as usize;
          if page.len() != expected_len {
            return Err(TryRuntimeError::Other(
              "WakeupCursorPage length disagrees with logical position",
            ));
          }
        }
        let mut cursor_keys = alloc::collections::BTreeSet::new();
        for index in 0..cursor_len {
          let Some(key) = Self::wakeup_cursor_get(clock, index) else {
            return Err(TryRuntimeError::Other(
              "WakeupCursor index does not resolve to a page entry",
            ));
          };
          if key.clock() != clock || !cursor_keys.insert(key) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor contains a duplicate or wrong-clock key",
            ));
          }
          if Self::wakeup_cursor_owner_index(key) != Some(index) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor key has no matching owner reverse index",
            ));
          }
          if index > 0 {
            let parent = index
              .checked_sub(1)
              .ok_or(TryRuntimeError::Other("wakeup cursor parent underflows"))?
              / 2;
            if Self::wakeup_cursor_get(clock, parent).is_none_or(|parent_key| parent_key > key) {
              return Err(TryRuntimeError::Other(
                "WakeupCursor violates min-heap ordering",
              ));
            }
          }
        }
      }
      let mut live_wakeup_pointers = alloc::collections::BTreeSet::new();
      for (actor, hot) in Self::canonical_hot_entries_for_try_state() {
        if let Some(pointer) = hot.wakeup_pointer {
          if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
            return Err(TryRuntimeError::Other(
              "multiple actors own the same wakeup pointer",
            ));
          }
          let slot = u8::try_from(pointer.slot)
            .map_err(|_| TryRuntimeError::Other("canonical Pipeline deadline slot exceeds u8"))?;
          let expected_handle = DeadlineHandle {
            actor,
            key: pointer.block,
            page: pointer.page_id,
            slot,
          };
          if DeadlineHandles::<T>::get(actor.actor_id) != Some(expected_handle)
            || DeadlinePages::<T>::get(pointer.block, pointer.page_id).is_none_or(|page| {
              page.entries.get(pointer.slot as usize).copied().flatten() != Some(actor)
            })
          {
            return Err(TryRuntimeError::Other(
              "canonical Pipeline deadline pointer does not resolve to its actor",
            ));
          }
          if let Some(terminal_at) = hot.terminal_at
            && !matches!(pointer.block, WakeupKey::Block(block) if block <= terminal_at)
          {
            return Err(TryRuntimeError::Other(
              "ActorControl Pipeline wakeup pointer exceeds its terminal membership",
            ));
          }
        }
        if let Some(trigger_pointer) = hot.trigger_wakeup_pointer {
          let pointer = WakeupPointer {
            block: WakeupKey::Tick(trigger_pointer.tick),
            page_id: trigger_pointer.page_id,
            slot: trigger_pointer.slot,
          };
          if !matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::AtTime {
              consumed: false,
              ..
            } | TriggerRuntimeState::Cadenced { .. }
          ) {
            return Err(TryRuntimeError::Other(
              "non-pending temporal Actor owns a Trigger wakeup pointer",
            ));
          }
          if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
            return Err(TryRuntimeError::Other(
              "multiple actors own the same wakeup pointer",
            ));
          }
          let slot = u8::try_from(pointer.slot)
            .map_err(|_| TryRuntimeError::Other("canonical Trigger deadline slot exceeds u8"))?;
          let expected_handle = DeadlineHandle {
            actor,
            key: pointer.block,
            page: pointer.page_id,
            slot,
          };
          if TriggerDeadlineHandles::<T>::get(actor.actor_id) != Some(expected_handle)
            || DeadlinePages::<T>::get(pointer.block, pointer.page_id).is_none_or(|page| {
              page.entries.get(pointer.slot as usize).copied().flatten() != Some(actor)
            })
          {
            return Err(TryRuntimeError::Other(
              "canonical Trigger deadline pointer does not resolve to its actor",
            ));
          }
        }
      }
      Self::do_try_state_service_ring()?;
      Self::do_try_state_deadline_carrier()?;
      let next_id = NextActorId::<T>::get();
      if let Some(max_actor_id) = max_id {
        if next_id <= max_actor_id {
          return Err(TryRuntimeError::Other(
            "NextActorId is not greater than the largest active actor_id",
          ));
        }
      }
      let mut owner_hold_totals = alloc::collections::BTreeMap::new();
      for (actor_id, identity) in &semantic_identities {
        match identity.actor_class.actor_type() {
          ActorType::System => {
            if ActorStateHolds::<T>::contains_key(actor_id) {
              return Err(TryRuntimeError::Other(
                "System Actor retains a User Actor-state hold",
              ));
            }
          }
          ActorType::User => {
            let expected = match ActorSemanticStates::<T>::get(actor_id) {
              Some(ActorSemanticState::Active(record)) => {
                Self::derive_actor_state_hold_with_authority(
                  *actor_id,
                  identity,
                  Some((&record.hot, &record.admission)),
                )
              }
              Some(ActorSemanticState::Dormant(_)) => {
                Self::derive_actor_state_hold_with_authority(*actor_id, identity, None)
              }
              None => Err(Error::<T>::ActorInvariant),
            }
            .map_err(|_| {
              TryRuntimeError::Other("User Actor-state hold geometry cannot be rederived")
            })?;
            let record = ActorStateHolds::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
              "User Actor has no Actor-state hold record",
            ))?;
            if record.owner != identity.owner || record.breakdown != expected {
              return Err(TryRuntimeError::Other(
                "User Actor-state hold record disagrees with retained geometry",
              ));
            }
            let total = Self::state_hold_total(&expected).map_err(|_| {
              TryRuntimeError::Other("User Actor-state hold total cannot be rederived")
            })?;
            let owner_total = owner_hold_totals
              .entry(record.owner)
              .or_insert(T::Balance::zero());
            *owner_total = owner_total
              .checked_add(&total)
              .ok_or(TryRuntimeError::Other(
                "aggregate User Actor-state hold overflows",
              ))?;
          }
        }
      }
      for (actor_id, _) in ActorStateHolds::<T>::iter() {
        // deos-bypass: bounded-iter -- MaxActorIdentities bounds per-Actor hold records.
        if !semantic_identities.contains_key(&actor_id) {
          return Err(TryRuntimeError::Other(
            "Actor-state hold record has no semantic Actor identity",
          ));
        }
      }
      let hold_reason: T::RuntimeHoldReason = HoldReason::ActorState.into();
      for (owner, expected) in owner_hold_totals {
        if T::StateHoldCurrency::balance_on_hold(&hold_reason, &owner) != expected {
          return Err(TryRuntimeError::Other(
            "owner Actor-state hold balance disagrees with per-Actor records",
          ));
        }
      }
      Ok(())
    }
  }
}
