#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use pallet_aaa::{AssetOps, FeeCollector};
use polkadot_sdk::{
  frame_support::{
    PalletId, construct_runtime,
    traits::{
      ConstU8, ConstU32, ConstU64, ConstU128, Currency, ExistenceRequirement, Get,
      fungible::Inspect as NativeInspect, tokens::Provenance,
    },
    weights::Weight,
  },
  frame_system::EnsureRoot,
  sp_runtime::{
    DispatchError, DispatchResult, Perbill, generic, impl_tx_ext_default,
    traits::{
      BlakeTwo256, DispatchInfoOf, IdentifyAccount, IdentityLookup, Lazy, PostDispatchInfoOf,
      TransactionExtension, Verify,
    },
    transaction_validity::{InvalidTransaction, TransactionValidityError},
  },
};
use scale_info::TypeInfo;

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;
pub type BlockNumber = u64;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type TxExtension = (
  polkadot_sdk::frame_system::CheckNonZeroSender<Runtime>,
  polkadot_sdk::frame_system::CheckNonce<Runtime>,
  polkadot_sdk::frame_system::CheckWeight<Runtime>,
  NativeIngressExtension,
);
pub type UncheckedExtrinsic =
  generic::UncheckedExtrinsic<AccountId, RuntimeCall, FixtureSignature, TxExtension>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

pub const NATIVE_ASSET: AssetId = 0;
pub const ALICE: AccountId = 11;
pub const BOB: AccountId = 22;
pub const FEE_SINK: AccountId = 90;
pub const DEX_SINK: AccountId = 91;
pub const INITIAL_BALANCE: Balance = 10_000_000_000_000;

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct FixtureSigner(pub AccountId);

impl IdentifyAccount for FixtureSigner {
  type AccountId = AccountId;

  fn into_account(self) -> Self::AccountId {
    self.0
  }
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct FixtureSignature {
  signer: AccountId,
  payload: Vec<u8>,
}

impl Verify for FixtureSignature {
  type Signer = FixtureSigner;

  fn verify<L: Lazy<[u8]>>(&self, mut message: L, signer: &AccountId) -> bool {
    self.signer == *signer && message.get() == self.payload
  }
}

construct_runtime!(
  pub enum Runtime {
    System: polkadot_sdk::frame_system,
    Balances: polkadot_sdk::pallet_balances,
    AAA: pallet_aaa,
  }
);

pub type Executive = polkadot_sdk::frame_executive::Executive<
  Runtime,
  Block,
  polkadot_sdk::frame_system::ChainContext<Runtime>,
  Runtime,
  AllPalletsWithSystem,
>;

impl polkadot_sdk::frame_system::Config for Runtime {
  type BaseCallFilter = polkadot_sdk::frame_support::traits::Everything;
  type BlockWeights = ();
  type BlockLength = ();
  type DbWeight = ();
  type RuntimeOrigin = RuntimeOrigin;
  type RuntimeCall = RuntimeCall;
  type Nonce = u64;
  type Hash = polkadot_sdk::sp_core::H256;
  type Hashing = BlakeTwo256;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Block = Block;
  type RuntimeEvent = RuntimeEvent;
  type BlockHashCount = ConstU64<250>;
  type Version = ();
  type PalletInfo = PalletInfo;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<Balance>;
  type OnNewAccount = ();
  type OnKilledAccount = ();
  type SystemWeightInfo = ();
  type SS58Prefix = ();
  type OnSetCode = ();
  type MaxConsumers = ConstU32<16>;
  type RuntimeTask = ();
  type ExtensionsWeightInfo = ();
  type SingleBlockMigrations = ();
  type MultiBlockMigrator = ();
  type PreInherents = ();
  type PostInherents = ();
  type PostTransactions = ();
}

impl polkadot_sdk::pallet_balances::Config for Runtime {
  type MaxLocks = ConstU32<16>;
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = Balance;
  type RuntimeEvent = RuntimeEvent;
  type DustRemoval = ();
  type ExistentialDeposit = ConstU128<1>;
  type AccountStore = System;
  type WeightInfo = ();
  type FreezeIdentifier = ();
  type MaxFreezes = ();
  type RuntimeHoldReason = RuntimeHoldReason;
  type RuntimeFreezeReason = RuntimeFreezeReason;
  type DoneSlashHandler = ();
}

pub struct NativeAssetOps;

impl NativeAssetOps {
  fn ensure_native(asset: AssetId) -> Result<(), pallet_aaa::TaskFailure> {
    if asset == NATIVE_ASSET {
      Ok(())
    } else {
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "UnsupportedAsset",
      )))
    }
  }
}

impl AssetOps<AccountId, AssetId, Balance> for NativeAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), pallet_aaa::TaskFailure> {
    Self::ensure_native(asset)?;
    <Balances as Currency<AccountId>>::transfer(from, to, amount, ExistenceRequirement::AllowDeath)
      .map_err(pallet_aaa::TaskFailure::permanent)
  }

  fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), pallet_aaa::TaskFailure> {
    Self::ensure_native(asset)?;
    if <Balances as Currency<AccountId>>::free_balance(who) < amount {
      return Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "InsufficientBalance",
      )));
    }
    let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
    if remainder == 0 {
      Ok(())
    } else {
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "InsufficientBalance",
      )))
    }
  }

  fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), pallet_aaa::TaskFailure> {
    Self::ensure_native(asset)?;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
    Ok(())
  }

  fn balance(who: &AccountId, asset: AssetId) -> Balance {
    if asset == NATIVE_ASSET {
      <Balances as Currency<AccountId>>::free_balance(who)
    } else {
      0
    }
  }

  fn minimum_balance(asset: AssetId) -> Balance {
    if asset == NATIVE_ASSET { 1 } else { 0 }
  }

  fn preflight_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), pallet_aaa::TaskFailure> {
    Self::ensure_native(asset)?;
    <Balances as NativeInspect<AccountId>>::can_withdraw(from, amount)
      .into_result(false)
      .map_err(pallet_aaa::TaskFailure::permanent)?;
    <Balances as NativeInspect<AccountId>>::can_deposit(to, amount, Provenance::Extant)
      .into_result()
      .map_err(pallet_aaa::TaskFailure::permanent)
  }
}

/// Runtime-local direct ingress adapter for native value transfers.
///
/// Preflight, value movement, and AAA notification share one storage transaction so callers never
/// observe funding without its signal or a signal without its funding.
pub fn transfer_and_notify_actor(
  aaa_id: pallet_aaa::AaaId,
  source: &AccountId,
  asset: AssetId,
  amount: Balance,
) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
  let actor = AAA::aaa_instances(aaa_id).ok_or(pallet_aaa::Error::<Runtime>::AaaNotFound)?;
  let provenance = pallet_aaa::FundingProvenance::Signed(*source);
  AAA::preflight_funding_event(aaa_id, asset, amount, Some(&provenance))?;
  polkadot_sdk::frame_support::storage::with_transaction(|| {
    let result = NativeAssetOps::transfer(source, &actor.sovereign_account, asset, amount)
      .and_then(|()| {
        AAA::notify_address_event(aaa_id, asset, amount, source)
          .map_err(pallet_aaa::TaskFailure::permanent)
      });
    match result {
      Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
      Err(failure) => {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(failure.error))
      }
    }
  })
}

#[cfg(feature = "dex-fixture")]
pub struct FixedRateDex;

#[cfg(feature = "dex-fixture")]
impl pallet_aaa::DexOps<AccountId, AssetId, Balance> for FixedRateDex {
  fn swap_exact_in(
    context: pallet_aaa::ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    _: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<Balance, pallet_aaa::TaskFailure> {
    let who = context.actor;
    if asset_in != NATIVE_ASSET || asset_out != 2 {
      return Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "UnsupportedPair",
      )));
    }
    if polkadot_sdk::frame_system::Pallet::<Runtime>::block_number() <= 1 {
      return Err(pallet_aaa::TaskFailure::temporary(DispatchError::Other(
        "TemporaryExactInFailure",
      )));
    }
    NativeAssetOps::transfer(who, &DEX_SINK, asset_in, amount_in)?;
    Ok(amount_in)
  }

  fn swap_exact_out(
    context: pallet_aaa::ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: Balance,
    max_amount_in: Balance,
    _: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<Balance, pallet_aaa::TaskFailure> {
    let who = context.actor;
    if asset_in != NATIVE_ASSET || asset_out != 1 {
      return Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "UnsupportedPair",
      )));
    }
    let amount_in = amount_out;
    if amount_in > max_amount_in {
      return Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "MaximumInputExceeded",
      )));
    }
    NativeAssetOps::transfer(who, &DEX_SINK, asset_in, amount_in)?;
    Ok(amount_in)
  }
}

#[cfg(feature = "dex-fixture")]
type RuntimeDexOps = FixedRateDex;
#[cfg(not(feature = "dex-fixture"))]
type RuntimeDexOps = ();

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct NativeIngressExtension;

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct NativeIngressPre {
  aaa_id: pallet_aaa::AaaId,
  source: AccountId,
  amount: Balance,
}

impl TransactionExtension<RuntimeCall> for NativeIngressExtension {
  const IDENTIFIER: &'static str = "IndependentNativeIngress";
  type Implicit = ();
  type Val = ();
  type Pre = Option<NativeIngressPre>;

  fn weight(&self, call: &RuntimeCall) -> Weight {
    if matches!(
      call,
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death { .. })
    ) {
      <<Runtime as pallet_aaa::Config>::WeightInfo as pallet_aaa::WeightInfo>::transaction_extension_ingress_notify()
    } else {
      Weight::zero()
    }
  }

  fn prepare(
    self,
    _val: Self::Val,
    origin: &<RuntimeCall as polkadot_sdk::sp_runtime::traits::Dispatchable>::RuntimeOrigin,
    call: &RuntimeCall,
    _info: &DispatchInfoOf<RuntimeCall>,
    _len: usize,
  ) -> Result<Self::Pre, TransactionValidityError> {
    let RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest,
      value,
    }) = call
    else {
      return Ok(None);
    };
    let source = polkadot_sdk::frame_system::ensure_signed(origin.clone())
      .map_err(|_| TransactionValidityError::from(InvalidTransaction::BadSigner))?;
    let Some(aaa_id) = AAA::sovereign_index(dest) else {
      return Ok(None);
    };
    let provenance = pallet_aaa::FundingProvenance::Signed(source);
    AAA::preflight_funding_event(aaa_id, NATIVE_ASSET, *value, Some(&provenance))
      .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(NativeIngressPre {
      aaa_id,
      source,
      amount: *value,
    }))
  }

  fn post_dispatch_details(
    pre: Self::Pre,
    _info: &DispatchInfoOf<RuntimeCall>,
    _post_info: &PostDispatchInfoOf<RuntimeCall>,
    _len: usize,
    result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    if result.is_err() {
      return Ok(Weight::zero());
    }
    if let Some(pre) = pre {
      AAA::notify_address_event(pre.aaa_id, NATIVE_ASSET, pre.amount, &pre.source)
        .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    }
    Ok(Weight::zero())
  }

  impl_tx_ext_default!(RuntimeCall; validate);
}

pub struct NativeFeeCollector;

impl FeeCollector<AccountId, AssetId, Balance> for NativeFeeCollector {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    NativeAssetOps::transfer(payer, fee_sink, native_asset, amount).map_err(|failure| failure.error)
  }
}

pub struct AaaPalletId;
impl Get<PalletId> for AaaPalletId {
  fn get() -> PalletId {
    PalletId(*b"aaindep0")
  }
}

pub struct ObservationFanoutWeightLimit;
impl Get<Weight> for ObservationFanoutWeightLimit {
  fn get() -> Weight {
    Weight::MAX
  }
}

pub struct GuaranteedOnIdleWeight;
impl Get<Weight> for GuaranteedOnIdleWeight {
  fn get() -> Weight {
    Weight::MAX
  }
}

pub struct LinearWeightToFee;
impl polkadot_sdk::sp_weights::WeightToFee for LinearWeightToFee {
  type Balance = Balance;

  fn weight_to_fee(weight: &Weight) -> Self::Balance {
    Balance::from(weight.ref_time())
  }
}

pub struct FeeSink;
impl Get<AccountId> for FeeSink {
  fn get() -> AccountId {
    FEE_SINK
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct FixtureBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_aaa::BenchmarkHelper<AccountId, AssetId, Balance, u32> for FixtureBenchmarkHelper {
  fn setup_add_liquidity(
    _: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_donate_liquidity(_: &AccountId) -> Result<(AssetId, AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_remove_liquidity(_: &AccountId) -> Result<(AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_stake(_: &AccountId) -> Result<(AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_unstake(_: &AccountId) -> Result<(AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_swap_exact_in(_: &AccountId) -> Result<(AssetId, AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_swap_exact_out(
    _: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn funding_assets(max: u32) -> Vec<AssetId> {
    if max == 0 {
      Vec::new()
    } else {
      alloc::vec![NATIVE_ASSET]
    }
  }

  fn setup_condition_assets(_: &AccountId, max: u32) -> Result<Vec<AssetId>, DispatchError> {
    Ok((0..max).map(|_| NATIVE_ASSET).collect())
  }

  fn setup_observation_feeds(max: u32) -> Result<Vec<u32>, DispatchError> {
    Ok((1..=max).collect())
  }

  fn setup_address_event_ingress(
    _: &AccountId,
    _: &AccountId,
    _: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    Err(DispatchError::Other("BenchmarkIngressUnsupported"))
  }

  fn run_address_event_ingress(_: &AccountId, _: &AccountId, _: Balance) -> bool {
    false
  }

  fn setup_xcm_asset_deposit() -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    Err(DispatchError::Other("BenchmarkIngressUnsupported"))
  }

  fn run_xcm_asset_deposit(
    _: &AccountId,
    _: &AccountId,
    _: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    Err(DispatchError::Other("BenchmarkIngressUnsupported"))
  }
}

pub struct ExecutionServiceShare;
impl Get<Perbill> for ExecutionServiceShare {
  fn get() -> Perbill {
    Perbill::from_percent(20)
  }
}

impl pallet_aaa::Config for Runtime {
  type AssetId = AssetId;
  type Balance = Balance;
  type NativeAssetId = ConstU32<NATIVE_ASSET>;
  type AssetOps = NativeAssetOps;
  type ObservationFeedId = u32;
  type ObservationProvider = ();
  type FundingAuthority = ();
  type DexOps = RuntimeDexOps;
  type StakingOps = ();
  type LiquidityOps = ();
  type MinWindowLength = ConstU64<2>;
  type PalletId = AaaPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxExecutionPlanSteps = ConstU32<6>;
  type MaxUserExecutionPlanSteps = ConstU32<3>;
  type MaxSystemExecutionPlanSteps = ConstU32<6>;
  type MaxFundingTrackedAssets = ConstU32<4>;
  type MaxContinuationSnapshotEntries = ConstU32<12>;
  type MaxConditionsPerStep = ConstU32<2>;
  type MaxOwnerSlots = ConstU8<2>;
  type MaxExecutionsPerBlock = ConstU32<16>;
  type SystemExecutionReserve = ExecutionServiceShare;
  type UserExecutionGuarantee = ExecutionServiceShare;
  type MaxQueueLength = ConstU32<128>;
  type QueuePageSize = ConstU32<8>;
  type WakeupPageSize = ConstU32<8>;
  type MaxQueueEntriesScannedPerBlock = ConstU32<128>;
  type MaxObservationFanoutPagesPerBlock = ConstU32<8>;
  type ObservationFanoutWeightLimit = ObservationFanoutWeightLimit;
  type MaxWakeupsPerBlock = ConstU32<16>;
  type MaxSweepPerBlock = ConstU32<4>;
  type MaxWhitelistSize = ConstU32<4>;
  type MaxTriggerSources = ConstU32<4>;
  type MaxSplitTransferLegs = ConstU32<4>;
  type MaxExecutionDelayBlocks = ConstU64<1_000>;
  type MaxTimerJitterBlocks = ConstU32<0>;
  type MaxIdleStarvationBlocks = ConstU32<3>;
  type GuaranteedOnIdleWeight = GuaranteedOnIdleWeight;
  type MaxAutoCloseNonceHorizon = ConstU64<1_000>;
  type MaxActiveActors = ConstU32<64>;
  type MaxActorIdentities = ConstU32<96>;
  type StepBaseFee = ConstU128<10>;
  type ConditionReadFee = ConstU128<1>;
  type AaaCreationFee = ConstU128<100>;
  type WeightToFee = LinearWeightToFee;
  type TaskWeightInfo = pallet_aaa::weights::SubstrateTaskWeightInfo<Runtime>;
  type FeeSink = FeeSink;
  type FeeCollector = NativeFeeCollector;
  type MaxConsecutiveFailures = ConstU32<2>;
  type MinUserBalance = ConstU128<10>;
  type WeightInfo = pallet_aaa::weights::SubstrateWeight<Runtime>;
  type GenesisSystemAaas = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = FixtureBenchmarkHelper;
}

#[cfg(feature = "std")]
use polkadot_sdk::sp_runtime::BuildStorage;

#[cfg(feature = "std")]
pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut storage = polkadot_sdk::frame_system::GenesisConfig::<Runtime>::default()
    .build_storage()
    .expect("system genesis builds");
  polkadot_sdk::pallet_balances::GenesisConfig::<Runtime> {
    balances: alloc::vec![
      (ALICE, INITIAL_BALANCE),
      (BOB, INITIAL_BALANCE),
      (FEE_SINK, 1),
      (DEX_SINK, 1),
    ],
    dev_accounts: None,
  }
  .assimilate_storage(&mut storage)
  .expect("balance genesis assimilates");
  pallet_aaa::GenesisConfig::<Runtime>::default()
    .assimilate_storage(&mut storage)
    .expect("AAA genesis assimilates");
  polkadot_sdk::sp_io::TestExternalities::new(storage)
}

#[cfg(test)]
mod tests {
  use super::*;
  use polkadot_sdk::frame_support::{
    BoundedVec, assert_noop, assert_ok,
    traits::{Currency, GetStorageVersion, Hooks, StorageVersion},
  };
  use polkadot_sdk::sp_runtime::Perbill;

  #[test]
  fn observation_boundary_is_independently_fail_closed() {
    assert_eq!(
      <() as pallet_aaa::ObservationProvider<u32>>::observation(7, 10),
      pallet_aaa::ScalarObservationState::Unavailable
    );
  }

  fn signed_extrinsic(signer: AccountId, nonce: u64, call: RuntimeCall) -> UncheckedExtrinsic {
    let tx_ext = (
      polkadot_sdk::frame_system::CheckNonZeroSender::<Runtime>::new(),
      polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(nonce),
      polkadot_sdk::frame_system::CheckWeight::<Runtime>::new(),
      NativeIngressExtension,
    );
    let payload = generic::SignedPayload::new(call.clone(), tx_ext.clone())
      .expect("fixture signed payload encodes");
    let signature = payload.using_encoded(|encoded| FixtureSignature {
      signer,
      payload: encoded.to_vec(),
    });
    UncheckedExtrinsic::new_signed(call, signer, signature, tx_ext)
  }

  fn execution_plan(task: pallet_aaa::TaskOf<Runtime>) -> pallet_aaa::ExecutionPlanOf<Runtime> {
    BoundedVec::try_from(alloc::vec![pallet_aaa::StepOf::<Runtime> {
      conditions: pallet_aaa::ConditionSet::Always,
      task,
      on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
    }])
    .expect("one-step plan fits")
  }

  fn program_with_task(
    trigger: pallet_aaa::TriggerOf<Runtime>,
    task: pallet_aaa::TaskOf<Runtime>,
  ) -> pallet_aaa::ProgramInputOf<Runtime> {
    let plan = execution_plan(task);
    pallet_aaa::ProgramInput::Active {
      schedule: pallet_aaa::Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      schedule_window: None,
      execution_plan: plan,
      completion_policy: pallet_aaa::CompletionPolicy::Persistent,
      funding_source_policy: pallet_aaa::FundingSourcePolicy::AnySource,
    }
  }

  fn transfer_program(
    trigger: pallet_aaa::TriggerOf<Runtime>,
    amount: Balance,
  ) -> pallet_aaa::ProgramInputOf<Runtime> {
    program_with_task(
      trigger,
      pallet_aaa::Task::Transfer {
        to: BOB,
        asset: NATIVE_ASSET,
        amount: pallet_aaa::AmountResolution::Fixed(amount),
      },
    )
  }

  fn step(
    task: pallet_aaa::TaskOf<Runtime>,
    on_error: pallet_aaa::StepErrorPolicy,
  ) -> pallet_aaa::StepOf<Runtime> {
    pallet_aaa::Step {
      conditions: pallet_aaa::ConditionSet::Always,
      task,
      on_error,
    }
  }

  fn active_program(
    trigger: pallet_aaa::TriggerOf<Runtime>,
    cooldown_blocks: u32,
    steps: Vec<pallet_aaa::StepOf<Runtime>>,
  ) -> pallet_aaa::ProgramInputOf<Runtime> {
    pallet_aaa::ProgramInput::Active {
      schedule: pallet_aaa::Schedule {
        trigger,
        cooldown_blocks,
      },
      schedule_window: None,
      execution_plan: BoundedVec::try_from(steps).expect("fixture plan fits"),
      completion_policy: pallet_aaa::CompletionPolicy::Persistent,
      funding_source_policy: pallet_aaa::FundingSourcePolicy::AnySource,
    }
  }

  #[cfg(feature = "dex-fixture")]
  fn temporary_swap_step() -> pallet_aaa::StepOf<Runtime> {
    step(
      pallet_aaa::Task::SwapIn {
        asset_in: NATIVE_ASSET,
        amount_in: pallet_aaa::AmountResolution::Fixed(10),
        asset_out: 2,
        slippage_tolerance: Perbill::zero(),
      },
      pallet_aaa::StepErrorPolicy::RetryLater { max_attempts: 3 },
    )
  }

  #[test]
  fn independent_runtime_metadata_exposes_split_aaa_storage() {
    let encoded = Runtime::metadata().encode();
    for expected in [
      b"AAA".as_slice(),
      b"ActorHot".as_slice(),
      b"ActorProgram".as_slice(),
      b"ActorFunding".as_slice(),
      b"ContinuationState".as_slice(),
      b"QueuePages".as_slice(),
      b"WakeupPages".as_slice(),
      b"ConditionSet".as_slice(),
      b"Always".as_slice(),
      b"All".as_slice(),
      b"Any".as_slice(),
      b"StopCycle".as_slice(),
      b"EmptyConditionSet".as_slice(),
    ] {
      assert!(
        encoded
          .windows(expected.len())
          .any(|window| window == expected),
        "metadata omits {}",
        core::str::from_utf8(expected).expect("fixture metadata names are UTF-8")
      );
    }
  }

  #[test]
  fn condition_set_scale_round_trip_preserves_canonical_mode() {
    let atom = pallet_aaa::Condition::BlockNumberAbove { threshold: 0 };
    let grouped = BoundedVec::try_from(alloc::vec![atom]).expect("one condition fits");
    let values: alloc::vec::Vec<pallet_aaa::ConditionSetOf<Runtime>> = alloc::vec![
      pallet_aaa::ConditionSet::Always,
      pallet_aaa::ConditionSet::All(grouped.clone()),
      pallet_aaa::ConditionSet::Any(grouped),
    ];
    let encoded = alloc::vec![values[0].encode(), values[1].encode(), values[2].encode()];
    assert_eq!([encoded[0][0], encoded[1][0], encoded[2][0]], [0, 1, 2]);
    for (expected, bytes) in values.into_iter().zip(encoded) {
      let decoded = pallet_aaa::ConditionSetOf::<Runtime>::decode(&mut &bytes[..])
        .expect("ConditionSet decodes");
      assert_eq!(decoded, expected);
    }
  }

  #[cfg(feature = "try-runtime")]
  #[test]
  fn independent_runtime_try_state_accepts_condition_aggregate_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let conditions = BoundedVec::try_from(alloc::vec![pallet_aaa::Condition::BlockNumberAbove {
        threshold: 0
      },])
      .expect("one condition fits");
      let program = active_program(
        pallet_aaa::Trigger::immediate_manual(),
        0,
        alloc::vec![pallet_aaa::Step {
          conditions: pallet_aaa::ConditionSet::Any(conditions),
          task: pallet_aaa::Task::StopCycle,
          on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
        }],
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        program,
      ));
      assert!(AAA::try_state(1).is_ok());
    });
  }

  #[test]
  fn independent_runtime_starts_from_the_fresh_schema_without_system_topology() {
    new_test_ext().execute_with(|| {
      let baseline = StorageVersion::new(1);
      assert_eq!(AAA::in_code_storage_version(), baseline);
      assert_eq!(AAA::on_chain_storage_version(), baseline);
      assert_eq!(AAA::actor_identity_count(), 0);
      assert_eq!(AAA::active_aaa_count(), 0);
      assert_eq!(pallet_aaa::ActorHot::<Runtime>::iter_keys().count(), 0);
    });
  }

  #[test]
  fn dormant_user_round_trips_without_scheduler_state_and_closes_purely() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        pallet_aaa::ProgramInput::Dormant,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let identity = AAA::dormant_aaa_identities(aaa_id).expect("identity exists");
      assert!(AAA::aaa_instances(aaa_id).is_none());
      assert_eq!(AAA::active_aaa_count(), 0);
      assert!(pallet_aaa::ActorHot::<Runtime>::get(aaa_id).is_none());
      assert_ok!(AAA::activate_aaa(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        transfer_program(pallet_aaa::Trigger::immediate_manual(), 5),
      ));
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &identity.sovereign_account,
        1_000,
        ExistenceRequirement::AllowDeath,
      ));
      assert_ok!(AAA::deactivate_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
      assert!(AAA::aaa_instances(aaa_id).is_none());
      assert_eq!(Balances::free_balance(identity.sovereign_account), 1_000);
      assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
      assert!(AAA::dormant_aaa_identities(aaa_id).is_none());
      assert_eq!(AAA::actor_identity_count(), 0);
      assert_eq!(AAA::owner_slot_mask(ALICE), 0);
      assert_eq!(Balances::free_balance(identity.sovereign_account), 1_000);
    });
  }

  #[test]
  fn independent_runtime_executes_a_native_transfer_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let program = transfer_program(pallet_aaa::Trigger::immediate_manual(), 50);
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        program,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let actor = AAA::aaa_instances(aaa_id)
        .expect("actor exists")
        .sovereign_account;
      let actor_funding = 10_000_000_000u128;
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &actor,
        actor_funding,
        ExistenceRequirement::AllowDeath,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("actor remains")
          .cycle_nonce,
        1
      );
    });
  }

  #[test]
  fn executive_executes_always_all_and_any_as_one_linear_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let transfer = |amount| pallet_aaa::Task::Transfer {
        to: BOB,
        asset: NATIVE_ASSET,
        amount: pallet_aaa::AmountResolution::Fixed(amount),
      };
      let all = BoundedVec::try_from(alloc::vec![
        pallet_aaa::Condition::BlockNumberAbove { threshold: 0 },
        pallet_aaa::Condition::BalanceAbove {
          asset: NATIVE_ASSET,
          threshold: 0,
        },
      ])
      .expect("maximum All group fits");
      let any = BoundedVec::try_from(alloc::vec![
        pallet_aaa::Condition::BlockNumberBelow { threshold: 0 },
        pallet_aaa::Condition::BlockNumberAbove { threshold: 0 },
      ])
      .expect("maximum Any group fits");
      let program = active_program(
        pallet_aaa::Trigger::immediate_manual(),
        0,
        alloc::vec![
          pallet_aaa::Step {
            conditions: pallet_aaa::ConditionSet::Always,
            task: transfer(7),
            on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
          },
          pallet_aaa::Step {
            conditions: pallet_aaa::ConditionSet::All(all),
            task: transfer(11),
            on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
          },
          pallet_aaa::Step {
            conditions: pallet_aaa::ConditionSet::Any(any),
            task: transfer(13),
            on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
          },
        ],
      );
      let create = RuntimeCall::AAA(pallet_aaa::Call::create_user_aaa {
        mutability: pallet_aaa::Mutability::Mutable,
        program,
      });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, create)),
        Ok(Ok(_))
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let actor = AAA::aaa_instances(aaa_id)
        .expect("actor exists")
        .sovereign_account;
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &actor,
        10_000_000_000,
        ExistenceRequirement::AllowDeath,
      ));
      let bob_before = Balances::free_balance(BOB);
      let trigger = RuntimeCall::AAA(pallet_aaa::Call::manual_trigger { aaa_id });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 1, trigger)),
        Ok(Ok(_))
      ));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(31));
    });
  }

  #[test]
  fn executive_balance_transfer_submits_direct_ingress_exact_once() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_aaa::Trigger::immediate_address_event(
        pallet_aaa::SourceFilter::Any,
        pallet_aaa::AssetFilter::Any,
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        transfer_program(trigger, 50),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let sovereign = AAA::aaa_instances(aaa_id)
        .expect("actor exists")
        .sovereign_account;
      let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: sovereign,
        value: 100_000_000_000,
      });
      let result = Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, call));
      assert!(matches!(result, Ok(Ok(_))), "{result:?}");
      assert!(AAA::pending_signal(aaa_id));
      let bob_before = Balances::free_balance(BOB);
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      System::set_block_number(2);
      let _ = AAA::on_idle(2, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
    });
  }

  #[test]
  fn failed_executive_transfer_submits_neither_value_nor_signal() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_aaa::Trigger::immediate_address_event(
        pallet_aaa::SourceFilter::Any,
        pallet_aaa::AssetFilter::Any,
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        transfer_program(trigger, 50),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let sovereign = AAA::aaa_instances(aaa_id)
        .expect("actor exists")
        .sovereign_account;
      let before = Balances::free_balance(sovereign);
      let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: sovereign,
        value: INITIAL_BALANCE.saturating_add(1),
      });
      let result = Executive::apply_extrinsic(signed_extrinsic(BOB, 0, call));
      assert!(matches!(result, Ok(Err(_))), "{result:?}");
      assert_eq!(Balances::free_balance(sovereign), before);
      assert!(!AAA::pending_signal(aaa_id));
    });
  }

  #[test]
  fn whole_plan_admission_uses_runtime_local_limits_and_cached_envelope() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let step = || pallet_aaa::StepOf::<Runtime> {
        conditions: pallet_aaa::ConditionSet::Always,
        task: pallet_aaa::Task::Transfer {
          to: BOB,
          asset: NATIVE_ASSET,
          amount: pallet_aaa::AmountResolution::Fixed(1),
        },
        on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
      };
      let oversized = BoundedVec::try_from(alloc::vec![step(), step(), step(), step()])
        .expect("four steps fit the global six-step bound");
      let oversized_program = pallet_aaa::ProgramInput::Active {
        schedule: pallet_aaa::Schedule {
          trigger: pallet_aaa::Trigger::immediate_manual(),
          cooldown_blocks: 0,
        },
        schedule_window: None,
        execution_plan: oversized,
        completion_policy: pallet_aaa::CompletionPolicy::Persistent,
        funding_source_policy: pallet_aaa::FundingSourcePolicy::AnySource,
      };
      assert_noop!(
        AAA::create_user_aaa(
          RuntimeOrigin::signed(ALICE),
          pallet_aaa::Mutability::Mutable,
          oversized_program,
        ),
        pallet_aaa::Error::<Runtime>::ExecutionPlanTooLong
      );

      let admitted = BoundedVec::try_from(alloc::vec![step(), step(), step()])
        .expect("three steps fit the User bound");
      let admission =
        AAA::execution_plan_admission_weight_upper(pallet_aaa::AaaType::User, &admitted);
      assert!(admission.all_lte(GuaranteedOnIdleWeight::get()));
      let admitted_program = pallet_aaa::ProgramInput::Active {
        schedule: pallet_aaa::Schedule {
          trigger: pallet_aaa::Trigger::immediate_manual(),
          cooldown_blocks: 0,
        },
        schedule_window: None,
        execution_plan: admitted,
        completion_policy: pallet_aaa::CompletionPolicy::Persistent,
        funding_source_policy: pallet_aaa::FundingSourcePolicy::AnySource,
      };
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        admitted_program,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let cached = AAA::aaa_instances(aaa_id)
        .expect("admitted actor exists")
        .cycle_weight_upper;
      assert_ne!(cached, Weight::zero());
      assert!(cached.all_lte(admission));
    });
  }

  #[test]
  fn direct_runtime_ingress_is_exact_once() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_aaa::Trigger::immediate_address_event(
        pallet_aaa::SourceFilter::Any,
        pallet_aaa::AssetFilter::Any,
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        transfer_program(trigger, 50),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        10_000_000_000,
      ));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      System::set_block_number(2);
      let _ = AAA::on_idle(2, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("actor remains")
          .cycle_nonce,
        1
      );
    });
  }

  #[test]
  fn paged_timer_wakeups_feed_fifo_queue_across_page_boundary() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mut actor_ids = alloc::vec::Vec::new();
      for _ in 0..9 {
        assert_ok!(AAA::create_system_aaa(
          RuntimeOrigin::root(),
          ALICE,
          pallet_aaa::Mutability::Mutable,
          transfer_program(pallet_aaa::Trigger::cadenced_always(20), 1),
        ));
        let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
        assert_ok!(transfer_and_notify_actor(aaa_id, &ALICE, NATIVE_ASSET, 10,));
        actor_ids.push(aaa_id);
      }
      assert_eq!(pallet_aaa::WakeupPages::<Runtime>::iter().count(), 2);
      for block in 21..=25 {
        System::set_block_number(block);
        let _ = AAA::on_idle(block, Weight::MAX);
      }
      assert!(
        actor_ids
          .iter() // deos-bypass: bounded-iter — fixed nine-actor fixture assertion
          .all(|aaa_id| {
            AAA::aaa_instances(*aaa_id)
              .expect("actor remains")
              .cycle_nonce
              == 1
          })
      );
      assert_eq!(pallet_aaa::SystemQueuePages::<Runtime>::iter().count(), 0);
      assert_eq!(pallet_aaa::UserQueuePages::<Runtime>::iter().count(), 0);
      assert!(
        actor_ids
          .iter() // deos-bypass: bounded-iter — fixed nine-actor fixture assertion
          .all(|aaa_id| {
            pallet_aaa::ActorHot::<Runtime>::get(*aaa_id)
              .expect("hot state remains")
              .wakeup_pointer
              .is_some()
          })
      );
      assert!(pallet_aaa::WakeupPages::<Runtime>::iter().count() >= 2);
    });
  }

  #[test]
  fn mutable_system_actor_reattaches_to_its_sovereign_account() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        pallet_aaa::Mutability::Mutable,
        transfer_program(pallet_aaa::Trigger::immediate_manual(), 5),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let sovereign = AAA::aaa_instances(aaa_id)
        .expect("system actor exists")
        .sovereign_account;
      let _ = <Balances as Currency<AccountId>>::deposit_creating(&sovereign, 777);
      assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), aaa_id));
      assert_eq!(Balances::free_balance(sovereign), 777);
      assert_ok!(AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        aaa_id,
        ALICE,
        pallet_aaa::Mutability::Mutable,
        pallet_aaa::ProgramInput::Dormant,
      ));
      let identity = AAA::dormant_aaa_identities(aaa_id).expect("system identity reattaches");
      assert_eq!(identity.sovereign_account, sovereign);
      assert_eq!(Balances::free_balance(sovereign), 777);
    });
  }

  #[test]
  fn system_mint_executes_while_user_admission_rejects_it_everywhere() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mint_task = || pallet_aaa::Task::Mint {
        asset: NATIVE_ASSET,
        amount: pallet_aaa::AmountResolution::Fixed(100),
      };
      assert_noop!(
        AAA::create_user_aaa(
          RuntimeOrigin::signed(ALICE),
          pallet_aaa::Mutability::Mutable,
          program_with_task(pallet_aaa::Trigger::immediate_manual(), mint_task()),
        ),
        pallet_aaa::Error::<Runtime>::MintNotAllowedForUserAaa
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        pallet_aaa::ProgramInput::Dormant,
      ));
      let dormant_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_noop!(
        AAA::activate_aaa(
          RuntimeOrigin::signed(ALICE),
          dormant_id,
          program_with_task(pallet_aaa::Trigger::immediate_manual(), mint_task()),
        ),
        pallet_aaa::Error::<Runtime>::MintNotAllowedForUserAaa
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        transfer_program(pallet_aaa::Trigger::immediate_manual(), 1),
      ));
      let active_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_noop!(
        AAA::update_execution_plan(
          RuntimeOrigin::signed(ALICE),
          active_id,
          execution_plan(mint_task()),
        ),
        pallet_aaa::Error::<Runtime>::MintNotAllowedForUserAaa
      );

      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        pallet_aaa::Mutability::Mutable,
        program_with_task(pallet_aaa::Trigger::immediate_manual(), mint_task()),
      ));
      let system_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let sovereign = AAA::aaa_instances(system_id)
        .expect("system mint actor exists")
        .sovereign_account;
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), system_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(sovereign), 100);
      assert_eq!(
        AAA::aaa_instances(system_id)
          .expect("system actor remains")
          .cycle_nonce,
        1
      );
    });
  }

  #[cfg(feature = "dex-fixture")]
  #[test]
  fn mutable_user_continuation_preserves_prefix_and_residual_admission() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let plan = active_program(
        pallet_aaa::Trigger::immediate_manual(),
        2,
        alloc::vec![
          step(
            pallet_aaa::Task::Transfer {
              to: BOB,
              asset: NATIVE_ASSET,
              amount: pallet_aaa::AmountResolution::Fixed(5),
            },
            pallet_aaa::StepErrorPolicy::AbortCycle,
          ),
          temporary_swap_step(),
          step(
            pallet_aaa::Task::Transfer {
              to: BOB,
              asset: NATIVE_ASSET,
              amount: pallet_aaa::AmountResolution::Fixed(7),
            },
            pallet_aaa::StepErrorPolicy::AbortCycle,
          ),
        ],
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        plan,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);

      let suspended = AAA::continuation_state(aaa_id).expect("temporary failure suspends");
      assert_eq!(suspended.cursor, 1);
      assert_eq!(suspended.attempt, 0);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(5));
      System::set_block_number(2);
      let _ = AAA::on_idle(2, Weight::MAX);
      assert_eq!(
        AAA::continuation_state(aaa_id)
          .expect("cooldown defers retry")
          .attempt,
        0
      );

      System::set_block_number(3);
      let _ = AAA::on_idle(3, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_none());
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("actor completes")
          .cycle_nonce,
        1
      );
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(12));
    });
  }

  #[cfg(feature = "dex-fixture")]
  #[test]
  fn mutable_system_continuation_retries_without_external_topology() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        pallet_aaa::Mutability::Mutable,
        active_program(
          pallet_aaa::Trigger::immediate_manual(),
          1,
          alloc::vec![temporary_swap_step()],
        ),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        1_000,
      ));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("system actor suspends")
          .run_state,
        pallet_aaa::RunState::Suspended
      );

      System::set_block_number(2);
      let _ = AAA::on_idle(2, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_none());
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("system actor completes")
          .cycle_nonce,
        1
      );
    });
  }

  #[cfg(feature = "dex-fixture")]
  #[test]
  fn suspended_direct_ingress_latches_once_and_survives_cancellation() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_aaa::Trigger::immediate_address_event(
        pallet_aaa::SourceFilter::Any,
        pallet_aaa::AssetFilter::Any,
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        active_program(trigger, 2, alloc::vec![temporary_swap_step()]),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let _ = AAA::on_idle(1, Weight::MAX);
      let before = AAA::aaa_instances(aaa_id).expect("actor suspends");
      let before_hot = pallet_aaa::ActorHot::<Runtime>::get(aaa_id).expect("hot state exists");
      assert_eq!(before.run_state, pallet_aaa::RunState::Suspended);

      let sovereign = before.sovereign_account;
      let first_call =
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
          dest: sovereign,
          value: 1_000,
        });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, first_call)),
        Ok(Ok(_))
      ));
      let after = AAA::aaa_instances(aaa_id).expect("actor remains suspended");
      let after_hot = pallet_aaa::ActorHot::<Runtime>::get(aaa_id).expect("hot state remains");
      assert!(after.pending_signal);
      assert_eq!(after_hot.wakeup_pointer, before_hot.wakeup_pointer);
      assert!(after_hot.queue_ticket.is_some());
      let repeated_call =
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
          dest: sovereign,
          value: 1_000,
        });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 1, repeated_call)),
        Ok(Ok(_))
      ));
      let repeated_hot = pallet_aaa::ActorHot::<Runtime>::get(aaa_id).expect("hot state remains");
      assert_eq!(repeated_hot.queue_ticket, after_hot.queue_ticket);
      assert_eq!(repeated_hot.wakeup_pointer, after_hot.wakeup_pointer);
      assert_ok!(AAA::cancel_continuation(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
      ));
      assert!(AAA::continuation_state(aaa_id).is_none());
      assert!(AAA::pending_signal(aaa_id));

      System::set_block_number(3);
      let _ = AAA::on_idle(3, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_none());
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("latched run completes")
          .cycle_nonce,
        2
      );
    });
  }

  #[cfg(feature = "dex-fixture")]
  #[test]
  fn continuation_cancel_then_pure_close_preserves_sovereign_balance() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        active_program(
          pallet_aaa::Trigger::immediate_manual(),
          1,
          alloc::vec![temporary_swap_step()],
        ),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      let sovereign = AAA::aaa_instances(aaa_id)
        .expect("actor exists")
        .sovereign_account;
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_some());
      let balance_before = Balances::free_balance(sovereign);
      assert_ok!(AAA::cancel_continuation(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
      ));
      assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
      assert!(AAA::aaa_instances(aaa_id).is_none());
      assert_eq!(Balances::free_balance(sovereign), balance_before);
    });
  }

  #[test]
  fn abort_policy_terminates_permanent_unsupported_failure() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        pallet_aaa::Mutability::Mutable,
        active_program(
          pallet_aaa::Trigger::immediate_manual(),
          0,
          alloc::vec![step(
            pallet_aaa::Task::Stake {
              asset: NATIVE_ASSET,
              amount: pallet_aaa::AmountResolution::Fixed(10),
            },
            pallet_aaa::StepErrorPolicy::AbortCycle,
          )],
        ),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(aaa_id, &ALICE, NATIVE_ASSET, 100));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      let actor = AAA::aaa_instances(aaa_id).expect("actor remains after first failure");
      assert_eq!(actor.consecutive_failures, 1);
      assert_eq!(actor.run_state, pallet_aaa::RunState::Idle);
      assert!(AAA::continuation_state(aaa_id).is_none());
    });
  }

  #[test]
  fn immutable_and_unsupported_paths_never_create_continuation() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let unsupported_retry = active_program(
        pallet_aaa::Trigger::immediate_manual(),
        1,
        alloc::vec![step(
          pallet_aaa::Task::Stake {
            asset: NATIVE_ASSET,
            amount: pallet_aaa::AmountResolution::Fixed(10),
          },
          pallet_aaa::StepErrorPolicy::RetryLater { max_attempts: 3 },
        )],
      );
      assert_noop!(
        AAA::create_system_aaa(
          RuntimeOrigin::root(),
          ALICE,
          pallet_aaa::Mutability::Immutable,
          unsupported_retry.clone(),
        ),
        pallet_aaa::Error::<Runtime>::RetryLaterNotAllowedForImmutableAaa
      );
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        unsupported_retry,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_none());
      assert_eq!(
        AAA::aaa_instances(aaa_id).expect("actor remains").run_state,
        pallet_aaa::RunState::Idle
      );
    });
  }

  #[test]
  fn independent_runtime_binds_nonzero_continuation_weights() {
    let suspend = <pallet_aaa::weights::SubstrateWeight<Runtime> as pallet_aaa::WeightInfo>::continuation_suspend(12);
    let retry =
      <pallet_aaa::weights::SubstrateWeight<Runtime> as pallet_aaa::WeightInfo>::continuation_retry(
      );
    let cancel = <pallet_aaa::weights::SubstrateWeight<Runtime> as pallet_aaa::WeightInfo>::continuation_cancel();
    assert!(suspend.ref_time() > retry.ref_time());
    assert!(cancel.proof_size() > retry.proof_size());
  }

  #[cfg(all(feature = "dex-fixture", feature = "try-runtime"))]
  #[test]
  fn try_state_accepts_a_suspended_independent_actor() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        pallet_aaa::Mutability::Mutable,
        active_program(
          pallet_aaa::Trigger::immediate_manual(),
          1,
          alloc::vec![temporary_swap_step()],
        ),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        1_000
      ));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert!(AAA::continuation_state(aaa_id).is_some());
      assert!(AAA::try_state(1).is_ok());
    });
  }

  #[cfg(not(feature = "dex-fixture"))]
  #[test]
  fn unsupported_adapters_follow_ordinary_step_error_policy() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let plan = BoundedVec::try_from(alloc::vec![
        pallet_aaa::StepOf::<Runtime> {
          conditions: pallet_aaa::ConditionSet::Always,
          task: pallet_aaa::Task::Stake {
            asset: NATIVE_ASSET,
            amount: pallet_aaa::AmountResolution::Fixed(10),
          },
          on_error: pallet_aaa::StepErrorPolicy::ContinueNextStep,
        },
        pallet_aaa::StepOf::<Runtime> {
          conditions: pallet_aaa::ConditionSet::Always,
          task: pallet_aaa::Task::Transfer {
            to: BOB,
            asset: NATIVE_ASSET,
            amount: pallet_aaa::AmountResolution::Fixed(5),
          },
          on_error: pallet_aaa::StepErrorPolicy::AbortCycle,
        },
      ])
      .expect("two-step plan fits");
      let program = pallet_aaa::ProgramInput::Active {
        schedule: pallet_aaa::Schedule {
          trigger: pallet_aaa::Trigger::immediate_manual(),
          cooldown_blocks: 0,
        },
        schedule_window: None,
        execution_plan: plan,
        completion_policy: pallet_aaa::CompletionPolicy::Persistent,
        funding_source_policy: pallet_aaa::FundingSourcePolicy::AnySource,
      };
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        program,
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(5));
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("actor remains")
          .cycle_nonce,
        1
      );
    });
  }

  #[cfg(feature = "dex-fixture")]
  #[test]
  fn exact_output_swap_remains_available_with_a_runtime_adapter() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let task = pallet_aaa::Task::SwapOut {
        asset_out: 1,
        amount_out: pallet_aaa::AmountResolution::Fixed(50),
        asset_in: NATIVE_ASSET,
        input_limit: pallet_aaa::InputLimit::Absolute(100),
        slippage_tolerance: Perbill::zero(),
      };
      assert_ok!(AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        pallet_aaa::Mutability::Mutable,
        program_with_task(pallet_aaa::Trigger::immediate_manual(), task),
      ));
      let aaa_id = pallet_aaa::NextAaaId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        aaa_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let sink_before = Balances::free_balance(DEX_SINK);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      let _ = AAA::on_idle(1, Weight::MAX);
      assert_eq!(
        Balances::free_balance(DEX_SINK),
        sink_before.saturating_add(50)
      );
      assert_eq!(
        AAA::aaa_instances(aaa_id)
          .expect("actor remains")
          .cycle_nonce,
        1
      );
    });
  }

  #[cfg(not(feature = "dex-fixture"))]
  #[test]
  fn optional_domain_adapters_fail_deterministically() {
    assert_eq!(
      <() as pallet_aaa::DexOps<AccountId, AssetId, Balance>>::swap_exact_out(
        pallet_aaa::ExecutionContext::new(&ALICE, pallet_aaa::AaaType::User),
        NATIVE_ASSET,
        1,
        10,
        10,
        Perbill::zero(),
      ),
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "DexOps not configured",
      )))
    );
    assert_eq!(
      <() as pallet_aaa::StakingOps<AccountId, AssetId, Balance>>::stake(&ALICE, NATIVE_ASSET, 10,),
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "StakingOps not configured",
      )))
    );
    assert_eq!(
      <() as pallet_aaa::LiquidityOps<AccountId, AssetId, Balance>>::add_liquidity(
        &ALICE,
        NATIVE_ASSET,
        1,
        10,
        10,
        1,
      ),
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "LiquidityOps not configured",
      )))
    );
    assert_eq!(
      <() as pallet_aaa::LiquidityOps<AccountId, AssetId, Balance>>::remove_liquidity(
        &ALICE,
        NATIVE_ASSET,
        10,
        1,
        1,
      ),
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "LiquidityOps not configured",
      )))
    );
    assert_eq!(
      <() as pallet_aaa::LiquidityOps<AccountId, AssetId, Balance>>::donate_liquidity(
        &ALICE,
        NATIVE_ASSET,
        1,
        10,
        Perbill::zero(),
      ),
      Err(pallet_aaa::TaskFailure::permanent(DispatchError::Other(
        "LiquidityOps not configured",
      )))
    );
  }
}
