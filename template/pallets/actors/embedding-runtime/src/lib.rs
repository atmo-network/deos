#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use pallet_deos_actors::{AssetOps, FeeCollector};
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
    DispatchError, DispatchResult, generic, impl_tx_ext_default,
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
    Actors: pallet_deos_actors,
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
  fn ensure_native(asset: AssetId) -> Result<(), pallet_deos_actors::TaskFailure> {
    if asset == NATIVE_ASSET {
      Ok(())
    } else {
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("UnsupportedAsset"),
      ))
    }
  }
}

impl AssetOps<AccountId, AssetId, Balance> for NativeAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), pallet_deos_actors::TaskFailure> {
    Self::ensure_native(asset)?;
    <Balances as Currency<AccountId>>::transfer(from, to, amount, ExistenceRequirement::AllowDeath)
      .map_err(pallet_deos_actors::TaskFailure::permanent)
  }

  fn burn(
    who: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), pallet_deos_actors::TaskFailure> {
    Self::ensure_native(asset)?;
    if <Balances as Currency<AccountId>>::free_balance(who) < amount {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("InsufficientBalance"),
      ));
    }
    let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
    if remainder == 0 {
      Ok(())
    } else {
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("InsufficientBalance"),
      ))
    }
  }

  fn mint(
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), pallet_deos_actors::TaskFailure> {
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
  ) -> Result<(), pallet_deos_actors::TaskFailure> {
    Self::ensure_native(asset)?;
    <Balances as NativeInspect<AccountId>>::can_withdraw(from, amount)
      .into_result(false)
      .map_err(pallet_deos_actors::TaskFailure::permanent)?;
    <Balances as NativeInspect<AccountId>>::can_deposit(to, amount, Provenance::Extant)
      .into_result()
      .map_err(pallet_deos_actors::TaskFailure::permanent)
  }
}

/// Runtime-local direct ingress adapter for native value transfers.
///
/// Preflight, value movement, and Actors notification share one storage transaction so callers never
/// observe funding without its signal or a signal without its funding.
pub fn transfer_and_notify_actor(
  actor_id: pallet_deos_actors::ActorId,
  source: &AccountId,
  asset: AssetId,
  amount: Balance,
) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
  let actor = Actors::active_actor_state(actor_id)
    .ok_or(pallet_deos_actors::Error::<Runtime>::ActorNotFound)?;
  let provenance = pallet_deos_actors::FundingProvenance::Signed;
  Actors::preflight_funding_event(actor_id, asset, amount, Some(source), Some(&provenance))?;
  polkadot_sdk::frame_support::storage::with_transaction(|| {
    let result = NativeAssetOps::transfer(source, &actor.identity.sovereign_account, asset, amount)
      .and_then(|()| {
        Actors::notify_address_event(actor_id, asset, amount, source)
          .map_err(pallet_deos_actors::TaskFailure::permanent)
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
impl pallet_deos_actors::DexOps<AccountId, AssetId, Balance> for FixedRateDex {
  fn swap_exact_in(
    context: pallet_deos_actors::ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    _: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<pallet_deos_actors::DexSwapOutcome<Balance>, pallet_deos_actors::TaskFailure> {
    let who = context.actor;
    if asset_in != NATIVE_ASSET || asset_out != 2 {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("UnsupportedPair"),
      ));
    }
    if polkadot_sdk::frame_system::Pallet::<Runtime>::block_number() <= 1 {
      return Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("TemporaryExactInFailure"),
      ));
    }
    NativeAssetOps::transfer(who, &DEX_SINK, asset_in, amount_in)?;
    Ok(pallet_deos_actors::DexSwapOutcome {
      total_amount_in: amount_in,
      recipient_amount_out: amount_in,
    })
  }

  fn swap_exact_out(
    context: pallet_deos_actors::ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: Balance,
    max_amount_in: Balance,
    _: polkadot_sdk::sp_runtime::Perbill,
  ) -> Result<pallet_deos_actors::DexSwapOutcome<Balance>, pallet_deos_actors::TaskFailure> {
    let who = context.actor;
    if asset_in != NATIVE_ASSET || asset_out != 1 {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("UnsupportedPair"),
      ));
    }
    let amount_in = amount_out;
    if amount_in > max_amount_in {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("MaximumInputExceeded"),
      ));
    }
    NativeAssetOps::transfer(who, &DEX_SINK, asset_in, amount_in)?;
    Ok(pallet_deos_actors::DexSwapOutcome {
      total_amount_in: amount_in,
      recipient_amount_out: amount_out,
    })
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
  actor_id: pallet_deos_actors::ActorId,
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
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as pallet_deos_actors::WeightInfo>::transaction_extension_ingress_notify()
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
    let Some(actor_id) = Actors::sovereign_index(dest) else {
      return Ok(None);
    };
    let provenance = pallet_deos_actors::FundingProvenance::Signed;
    Actors::preflight_funding_event(
      actor_id,
      NATIVE_ASSET,
      *value,
      Some(&source),
      Some(&provenance),
    )
    .map_err(|_| TransactionValidityError::from(InvalidTransaction::Custom(40)))?;
    Ok(Some(NativeIngressPre {
      actor_id,
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
      Actors::notify_address_event(pre.actor_id, NATIVE_ASSET, pre.amount, &pre.source)
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

pub struct ActorsPalletId;
impl Get<PalletId> for ActorsPalletId {
  fn get() -> PalletId {
    PalletId(*b"aaindep0")
  }
}

/// Fixture worker and reserve budgets mirror the reference runtime ratios over
/// the default maximum block weight: 20% per worker and a 50% guaranteed
/// on_idle reserve so the derived ActorServiceReserve stays strictly positive
/// (spec 5.4). `Weight::MAX` placeholders would underflow the checked reserve
/// derivation and reject every plan.
pub struct ObservationFanoutWeightLimit;
impl Get<Weight> for ObservationFanoutWeightLimit {
  fn get() -> Weight {
    let max_block = <() as polkadot_sdk::frame_support::traits::Get<
      polkadot_sdk::frame_system::limits::BlockWeights,
    >>::get()
    .max_block;
    polkadot_sdk::sp_runtime::Perbill::from_percent(20) * max_block
  }
}

pub struct WakeupWeightLimit;
impl Get<Weight> for WakeupWeightLimit {
  fn get() -> Weight {
    let max_block = <() as polkadot_sdk::frame_support::traits::Get<
      polkadot_sdk::frame_system::limits::BlockWeights,
    >>::get()
    .max_block;
    polkadot_sdk::sp_runtime::Perbill::from_percent(20) * max_block
  }
}

pub struct ActorOnIdleReserve;
impl Get<Weight> for ActorOnIdleReserve {
  fn get() -> Weight {
    let max_block = <() as polkadot_sdk::frame_support::traits::Get<
      polkadot_sdk::frame_system::limits::BlockWeights,
    >>::get()
    .max_block;
    polkadot_sdk::sp_runtime::Perbill::from_percent(50) * max_block
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
impl pallet_deos_actors::BenchmarkHelper<AccountId, AssetId, Balance, u32>
  for FixtureBenchmarkHelper
{
  fn setup_add_liquidity(
    _: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_donate_liquidity(_: &AccountId) -> Result<(AssetId, AssetId, Balance), DispatchError> {
    Err(DispatchError::Other("BenchmarkCapabilityUnsupported"))
  }

  fn setup_remove_liquidity(
    _: &AccountId,
  ) -> Result<(AssetId, AssetId, AssetId, Balance), DispatchError> {
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

  fn setup_predicate_assets(_: &AccountId, max: u32) -> Result<Vec<AssetId>, DispatchError> {
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

impl pallet_deos_actors::Config for Runtime {
  type AssetId = AssetId;
  type Balance = Balance;
  type FeeNativeAssetId = ConstU32<NATIVE_ASSET>;
  type AssetOps = NativeAssetOps;
  type ObservationFeedId = u32;
  type ObservationProvider = ();
  type FundingAuthority = ();
  type SovereignAccountPolicy = ();
  type DexOps = RuntimeDexOps;
  type StakingOps = ();
  type LiquidityOps = ();
  type MinWindowLength = ConstU64<2>;
  type PalletId = ActorsPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxContractSteps = ConstU32<8>;
  type MaxFundingTrackedAssets = ConstU32<4>;
  type MaxOpeningSnapshotEntries = ConstU32<16>;
  type MaxOpeningPredicateResults = ConstU32<16>;
  type MaxPreconditionClauses = ConstU32<2>;
  type MaxPredicatesPerClause = ConstU32<2>;
  type MaxPredicatesPerStep = ConstU32<2>;
  type MaxOwnerSlots = ConstU8<2>;
  type MaxExecutionsPerBlock = ConstU32<16>;
  type MaxQueueLength = ConstU32<128>;
  type QueuePageSize = ConstU32<8>;
  type WakeupPageSize = ConstU32<8>;
  type ObservationPageSize = ConstU32<8>;
  type MaxQueueEntriesScannedPerBlock = ConstU32<128>;
  type MaxObservationFanoutPagesPerBlock = ConstU32<8>;
  type ObservationFanoutWeightLimit = ObservationFanoutWeightLimit;
  type WakeupWeightLimit = WakeupWeightLimit;
  type MaxWakeupsPerBlock = ConstU32<16>;
  type MaxSweepBatch = ConstU32<4>;
  type MaxWhitelistSize = ConstU32<4>;
  type MaxTriggerSources = ConstU32<4>;
  type MaxSplitTransferLegs = ConstU32<4>;
  type TargetBlockTime = ConstU64<315_576>;
  type MaxExecutionDelayBlocks = ConstU64<1_000>;
  type MaxIdleStarvationBlocks = ConstU32<3>;
  type ActorOnIdleReserve = ActorOnIdleReserve;
  type MaxAutoCloseNonceHorizon = ConstU64<1_000>;
  type MaxActiveActors = ConstU32<64>;
  type MaxActorIdentities = ConstU32<96>;
  type MaxSystemSovereigns = ConstU32<96>;
  type ActorCreationFee = ConstU128<100>;
  type WeightToFee = LinearWeightToFee;
  type FeeSink = FeeSink;
  type FeeCollector = NativeFeeCollector;
  type MaxConsecutiveFailures = ConstU32<2>;
  type MaxRetryAttempts = ConstU32<10>;
  type MinUserBalance = ConstU128<10>;
  type WeightInfo = pallet_deos_actors::weights::SubstrateWeight<Runtime>;
  type GenesisSystemActors = ();
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
  pallet_deos_actors::GenesisConfig::<Runtime>::default()
    .assimilate_storage(&mut storage)
    .expect("Actors genesis assimilates");
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
      <() as pallet_deos_actors::ObservationProvider<u32, BlockNumber>>::observe(&7, 1, 10),
      pallet_deos_actors::ScalarObservationState::Unavailable
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

  fn contract_steps(
    task: pallet_deos_actors::TaskOf<Runtime>,
  ) -> pallet_deos_actors::ContractSteps<Runtime> {
    BoundedVec::try_from(alloc::vec![pallet_deos_actors::StepOf::<Runtime> {
      precondition: None,
      task,
      on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
    }])
    .expect("one-step plan fits")
  }

  fn all_precondition(
    predicates: alloc::vec::Vec<pallet_deos_actors::Predicate<u32, Balance, u32, u32>>,
  ) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
    let clause = BoundedVec::try_from(
      predicates
        .into_iter()
        .map(|predicate| pallet_deos_actors::TimedPredicate {
          timing: pallet_deos_actors::ObservationTiming::Current,
          predicate,
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("predicates fit");
    Some(pallet_deos_actors::Precondition {
      clauses: BoundedVec::try_from(alloc::vec![clause]).expect("clause fits"),
    })
  }

  fn any_precondition(
    predicates: alloc::vec::Vec<pallet_deos_actors::Predicate<u32, Balance, u32, u32>>,
  ) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
    let clauses = predicates
      .into_iter()
      .map(|predicate| {
        BoundedVec::try_from(alloc::vec![pallet_deos_actors::TimedPredicate {
          timing: pallet_deos_actors::ObservationTiming::Current,
          predicate,
        }])
        .expect("predicate fits")
      })
      .collect::<alloc::vec::Vec<_>>();
    Some(pallet_deos_actors::Precondition {
      clauses: BoundedVec::try_from(clauses).expect("clauses fit"),
    })
  }

  /// Spec 7.1 prefunding requirement: `MinUserBalance + attempt fee envelope`.
  fn user_prefunding_requirement(plan: &pallet_deos_actors::ContractSteps<Runtime>) -> Balance {
    let min_user_balance: Balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    min_user_balance.saturating_add(
      Actors::attempt_fee_envelope(pallet_deos_actors::ActorType::User, plan, 0)
        .expect("fixture plan has a checked fee envelope")
        .total,
    )
  }

  fn lowest_free_owner_slot(owner: AccountId) -> u8 {
    let bitmap = pallet_deos_actors::OwnerSlotBitmaps::<Runtime>::get(owner);
    let max_slots: u8 = <Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get();
    for (byte_index, byte) in bitmap.into_iter().enumerate() {
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
        return (first_slot + free_bits.trailing_zeros() as usize) as u8;
      }
    }
    panic!("fixture owner has no free User owner slot");
  }

  /// Pre-funds the next automatically allocated User slot so Active creation or
  /// activation admits under the fixture's spec 7.1 prefunding requirement.
  fn prefund_user_sovereign(
    owner: AccountId,
    slot: u8,
    plan: &pallet_deos_actors::ContractSteps<Runtime>,
  ) {
    let sovereign = Actors::sovereign_account_id(&owner, slot);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(
      &sovereign,
      user_prefunding_requirement(plan),
    );
  }

  fn prefund_active_user_creation(
    owner: AccountId,
    plan: &pallet_deos_actors::ContractSteps<Runtime>,
  ) {
    let slot = lowest_free_owner_slot(owner);
    prefund_user_sovereign(owner, slot, plan);
  }

  fn prefund_active_contract(
    owner: AccountId,
    contract: &pallet_deos_actors::ActorContractOf<Runtime>,
  ) {
    prefund_active_user_creation(owner, &contract.steps);
  }

  fn contract_with_task(
    trigger: pallet_deos_actors::TriggerOf<Runtime>,
    task: pallet_deos_actors::TaskOf<Runtime>,
  ) -> pallet_deos_actors::ActorContractOf<Runtime> {
    let plan = contract_steps(task);
    pallet_deos_actors::ActorContract {
      trigger,
      cooldown_blocks: 0,
      window: None,
      steps: plan,
      completion: pallet_deos_actors::CompletionPolicy::Persistent,
      funding: pallet_deos_actors::FundingSourcePolicy::AnyVerifiedIngress,
      auto_close_at_cycle_nonce: None,
    }
  }

  fn transfer_contract(
    trigger: pallet_deos_actors::TriggerOf<Runtime>,
    amount: Balance,
  ) -> pallet_deos_actors::ActorContractOf<Runtime> {
    contract_with_task(
      trigger,
      pallet_deos_actors::Task::Transfer {
        to: BOB,
        asset: NATIVE_ASSET,
        amount: pallet_deos_actors::AmountResolution::Fixed(amount),
      },
    )
  }

  fn step(
    task: pallet_deos_actors::TaskOf<Runtime>,
    on_error: pallet_deos_actors::StepErrorPolicy,
  ) -> pallet_deos_actors::StepOf<Runtime> {
    pallet_deos_actors::Step {
      precondition: None,
      task,
      on_error,
    }
  }

  fn active_contract(
    trigger: pallet_deos_actors::TriggerOf<Runtime>,
    cooldown_blocks: u32,
    steps: Vec<pallet_deos_actors::StepOf<Runtime>>,
  ) -> pallet_deos_actors::ActorContractOf<Runtime> {
    pallet_deos_actors::ActorContract {
      trigger,
      cooldown_blocks,
      window: None,
      steps: BoundedVec::try_from(steps).expect("fixture plan fits"),
      completion: pallet_deos_actors::CompletionPolicy::Persistent,
      funding: pallet_deos_actors::FundingSourcePolicy::AnyVerifiedIngress,
      auto_close_at_cycle_nonce: None,
    }
  }

  #[cfg(feature = "dex-fixture")]
  fn temporary_swap_step() -> pallet_deos_actors::StepOf<Runtime> {
    step(
      pallet_deos_actors::Task::SwapIn {
        asset_in: NATIVE_ASSET,
        amount_in: pallet_deos_actors::AmountResolution::Fixed(10),
        asset_out: 2,
        slippage_tolerance: Perbill::zero(),
      },
      pallet_deos_actors::StepErrorPolicy::RetryLater { max_attempts: 3 },
    )
  }

  #[test]
  fn independent_runtime_metadata_exposes_split_actor_storage() {
    let encoded = Runtime::metadata().encode();
    for expected in [
      b"Actors".as_slice(),
      b"ActorHot".as_slice(),
      b"ActorContract".as_slice(),
      b"ActorFunding".as_slice(),
      b"ContinuationState".as_slice(),
      b"QueuePages".as_slice(),
      b"WakeupPages".as_slice(),
      b"Precondition".as_slice(),
      b"precondition".as_slice(),
      b"All".as_slice(),
      b"Any".as_slice(),
      b"StopCycle".as_slice(),
      b"EmptyPrecondition".as_slice(),
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
  fn precondition_scale_round_trip_preserves_optional_canonical_dnf() {
    let atom = pallet_deos_actors::Predicate::BlockNumberAbove { threshold: 0 };
    let values: alloc::vec::Vec<Option<pallet_deos_actors::PreconditionOf<Runtime>>> =
      alloc::vec![None, all_precondition(alloc::vec![atom])];
    let encoded = alloc::vec![values[0].encode(), values[1].encode()];
    assert_eq!([encoded[0][0], encoded[1][0]], [0, 1]);
    for (expected, bytes) in values.into_iter().zip(encoded) {
      let decoded = Option::<pallet_deos_actors::PreconditionOf<Runtime>>::decode(&mut &bytes[..])
        .expect("optional Precondition decodes");
      assert_eq!(decoded, expected);
    }
  }

  #[cfg(feature = "try-runtime")]
  #[test]
  fn independent_runtime_try_state_accepts_condition_aggregate_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let precondition = any_precondition(alloc::vec![
        pallet_deos_actors::Predicate::BlockNumberAbove { threshold: 0 },
      ]);
      let contract = active_contract(
        pallet_deos_actors::Trigger::immediate_manual(),
        0,
        alloc::vec![pallet_deos_actors::Step {
          precondition,
          task: pallet_deos_actors::Task::StopCycle,
          on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
        }],
      );
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      assert!(Actors::try_state(1).is_ok());
    });
  }

  #[test]
  fn independent_runtime_starts_from_the_fresh_schema_without_system_topology() {
    new_test_ext().execute_with(|| {
      let baseline = StorageVersion::new(1);
      assert_eq!(Actors::in_code_storage_version(), baseline);
      assert_eq!(Actors::on_chain_storage_version(), baseline);
      assert_eq!(Actors::actor_identity_count(), 0);
      assert_eq!(Actors::active_actor_count(), 0);
      assert_eq!(
        pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count(),
        0
      );
    });
  }

  #[test]
  fn dormant_user_round_trips_without_scheduler_state_and_closes_purely() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        None,
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let identity = Actors::actor_identities(actor_id).expect("identity exists");
      assert!(Actors::active_actor_state(actor_id).is_none());
      assert_eq!(Actors::active_actor_count(), 0);
      assert!(pallet_deos_actors::ActorHot::<Runtime>::get(actor_id).is_none());
      // Spec 7.1: the existing dormant sovereign must cover the activation
      // prefunding requirement before the Active Actor Contract commits.
      let activate_plan = contract_steps(pallet_deos_actors::Task::Transfer {
        to: BOB,
        asset: NATIVE_ASSET,
        amount: pallet_deos_actors::AmountResolution::Fixed(5),
      });
      let _ = <Balances as Currency<AccountId>>::deposit_creating(
        &identity.sovereign_account,
        user_prefunding_requirement(&activate_plan),
      );
      System::set_block_number(2);
      assert_ok!(Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        transfer_contract(pallet_deos_actors::Trigger::immediate_manual(), 5),
      ));
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &identity.sovereign_account,
        1_000,
        ExistenceRequirement::AllowDeath,
      ));
      System::set_block_number(3);
      assert_ok!(Actors::deactivate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      assert!(Actors::active_actor_state(actor_id).is_none());
      let expected = user_prefunding_requirement(&activate_plan).saturating_add(1_000);
      assert_eq!(Balances::free_balance(identity.sovereign_account), expected);
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert!(Actors::actor_identities(actor_id).is_none());
      assert_eq!(Actors::actor_identity_count(), 0);
      assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
      assert_eq!(Balances::free_balance(identity.sovereign_account), expected);
    });
  }

  #[test]
  fn independent_runtime_executes_a_native_transfer_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let contract = transfer_contract(pallet_deos_actors::Trigger::immediate_manual(), 50);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let actor = Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .sovereign_account;
      let actor_funding = 10_000_000_000u128;
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &actor,
        actor_funding,
        ExistenceRequirement::AllowDeath,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor remains")
          .identity
          .cycle_nonce,
        1
      );
    });
  }

  #[test]
  fn executive_executes_unconditional_and_dnf_as_one_linear_plan() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let transfer = |amount| pallet_deos_actors::Task::Transfer {
        to: BOB,
        asset: NATIVE_ASSET,
        amount: pallet_deos_actors::AmountResolution::Fixed(amount),
      };
      let all = alloc::vec![
        pallet_deos_actors::Predicate::BlockNumberAbove { threshold: 0 },
        pallet_deos_actors::Predicate::BalanceAbove {
          asset: NATIVE_ASSET,
          threshold: 0,
        },
      ];
      let any = alloc::vec![
        pallet_deos_actors::Predicate::BlockNumberBelow { threshold: 0 },
        pallet_deos_actors::Predicate::BlockNumberAbove { threshold: 0 },
      ];
      let contract = active_contract(
        pallet_deos_actors::Trigger::immediate_manual(),
        0,
        alloc::vec![
          pallet_deos_actors::Step {
            precondition: None,
            task: transfer(7),
            on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
          },
          pallet_deos_actors::Step {
            precondition: all_precondition(all),
            task: transfer(11),
            on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
          },
          pallet_deos_actors::Step {
            precondition: any_precondition(any),
            task: transfer(13),
            on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
          },
        ],
      );
      prefund_active_contract(ALICE, &contract);
      let create = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
        mutability: pallet_deos_actors::Mutability::Mutable,
        contract: Some(contract),
      });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, create)),
        Ok(Ok(_))
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let actor = Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .sovereign_account;
      assert_ok!(<Balances as Currency<AccountId>>::transfer(
        &ALICE,
        &actor,
        10_000_000_000,
        ExistenceRequirement::AllowDeath,
      ));
      let bob_before = Balances::free_balance(BOB);
      let trigger = RuntimeCall::Actors(pallet_deos_actors::Call::manual_trigger { actor_id });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 1, trigger)),
        Ok(Ok(_))
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(31));
    });
  }

  #[test]
  fn executive_balance_transfer_submits_direct_ingress_exact_once() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_deos_actors::Trigger::immediate_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      );
      let contract = transfer_contract(trigger, 50);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let sovereign = Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .sovereign_account;
      let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: sovereign,
        value: 100_000_000_000,
      });
      let result = Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, call));
      assert!(matches!(result, Ok(Ok(_))), "{result:?}");
      assert!(Actors::pending_signal(actor_id));
      let bob_before = Balances::free_balance(BOB);
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      System::set_block_number(2);
      let _ = Actors::on_idle(2, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
    });
  }

  #[test]
  fn failed_executive_transfer_submits_neither_value_nor_signal() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_deos_actors::Trigger::immediate_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      );
      let contract = transfer_contract(trigger, 50);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let sovereign = Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .sovereign_account;
      let before = Balances::free_balance(sovereign);
      let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: sovereign,
        value: INITIAL_BALANCE.saturating_add(1),
      });
      let result = Executive::apply_extrinsic(signed_extrinsic(BOB, 0, call));
      assert!(matches!(result, Ok(Err(_))), "{result:?}");
      assert_eq!(Balances::free_balance(sovereign), before);
      assert!(!Actors::pending_signal(actor_id));
    });
  }

  #[test]
  fn whole_plan_admission_uses_runtime_local_limits_and_cached_envelope() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let step = || pallet_deos_actors::StepOf::<Runtime> {
        precondition: None,
        task: pallet_deos_actors::Task::Transfer {
          to: BOB,
          asset: NATIVE_ASSET,
          amount: pallet_deos_actors::AmountResolution::Fixed(1),
        },
        on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
      };
      assert!(
        BoundedVec::<_, <Runtime as pallet_deos_actors::Config>::MaxContractSteps>::try_from(
          alloc::vec![
            step(),
            step(),
            step(),
            step(),
            step(),
            step(),
            step(),
            step(),
            step()
          ],
        )
        .is_err()
      );

      let admitted = BoundedVec::try_from(alloc::vec![
        step(),
        step(),
        step(),
        step(),
        step(),
        step(),
        step(),
        step(),
      ])
      .expect("eight steps fit the shared bound");
      let admission = Actors::contract_steps_admission_weight_upper(
        pallet_deos_actors::ActorType::User,
        &admitted,
      );
      assert!(admission.all_lte(ActorOnIdleReserve::get()));
      let admitted_contract = pallet_deos_actors::ActorContract {
        trigger: pallet_deos_actors::Trigger::immediate_manual(),
        cooldown_blocks: 0,
        window: None,
        steps: admitted.clone(),
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: pallet_deos_actors::FundingSourcePolicy::AnyVerifiedIngress,
        auto_close_at_cycle_nonce: None,
      };
      prefund_active_contract(ALICE, &admitted_contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(admitted_contract),
      ));
      assert!(
        Actors::active_actor_state(
          pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1)
        )
        .is_some()
      );
    });
  }

  #[test]
  fn direct_runtime_ingress_is_exact_once() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let trigger = pallet_deos_actors::Trigger::immediate_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      );
      let contract = transfer_contract(trigger, 50);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        10_000_000_000,
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      System::set_block_number(2);
      let _ = Actors::on_idle(2, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(50));
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor remains")
          .identity
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
        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          pallet_deos_actors::Mutability::Mutable,
          Some(transfer_contract(
            pallet_deos_actors::Trigger::cadenced_always(20),
            1,
          )),
        ));
        let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
        assert_ok!(transfer_and_notify_actor(
          actor_id,
          &ALICE,
          NATIVE_ASSET,
          10,
        ));
        actor_ids.push(actor_id);
      }
      assert_eq!(
        pallet_deos_actors::WakeupPages::<Runtime>::iter().count(),
        2
      );
      for block in 21..=25 {
        System::set_block_number(block);
        let _ = Actors::on_idle(block, Weight::MAX);
      }
      for actor_id in &actor_ids {
        assert_eq!(
          Actors::active_actor_state(*actor_id)
            .expect("actor remains")
            .identity
            .cycle_nonce,
          1
        );
      }
      assert_eq!(pallet_deos_actors::QueuePages::<Runtime>::iter().count(), 0);
      for actor_id in &actor_ids {
        assert!(
          pallet_deos_actors::ActorHot::<Runtime>::get(*actor_id)
            .expect("hot state remains")
            .wakeup_pointer
            .is_some()
        );
      }
      assert!(pallet_deos_actors::WakeupPages::<Runtime>::iter().count() >= 2);
    });
  }

  #[test]
  fn mutable_system_actor_reattaches_to_its_sovereign_account() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        Some(transfer_contract(
          pallet_deos_actors::Trigger::immediate_manual(),
          5,
        )),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let sovereign = Actors::active_actor_state(actor_id)
        .expect("system actor exists")
        .identity
        .sovereign_account;
      let _ = <Balances as Currency<AccountId>>::deposit_creating(&sovereign, 777);
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
      assert_eq!(Balances::free_balance(sovereign), 777);
      let fresh_id = pallet_deos_actors::NextActorId::<Runtime>::get();
      assert_ok!(Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        actor_id,
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        None,
      ));
      let identity = Actors::actor_identities(fresh_id).expect("fresh system identity reattaches");
      assert_eq!(identity.sovereign_account, sovereign);
      assert_eq!(Balances::free_balance(sovereign), 777);
    });
  }

  #[test]
  fn system_mint_executes_while_user_admission_rejects_it_everywhere() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mint_task = || pallet_deos_actors::Task::Mint {
        asset: NATIVE_ASSET,
        amount: pallet_deos_actors::AmountResolution::Fixed(100),
      };
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          pallet_deos_actors::Mutability::Mutable,
          Some(contract_with_task(
            pallet_deos_actors::Trigger::immediate_manual(),
            mint_task(),
          )),
        ),
        pallet_deos_actors::Error::<Runtime>::MintNotAllowedForUserActor
      );
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        None,
      ));
      let dormant_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      System::set_block_number(2);
      assert_noop!(
        Actors::activate_actor(
          RuntimeOrigin::signed(ALICE),
          dormant_id,
          contract_with_task(pallet_deos_actors::Trigger::immediate_manual(), mint_task()),
        ),
        pallet_deos_actors::Error::<Runtime>::MintNotAllowedForUserActor
      );
      let plan = transfer_contract(pallet_deos_actors::Trigger::immediate_manual(), 1);
      prefund_active_contract(ALICE, &plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(plan),
      ));
      let active_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      System::set_block_number(3);
      let mut contract = pallet_deos_actors::ActorContracts::<Runtime>::get(active_id)
        .expect("Actor Contract exists");
      contract.steps = contract_steps(mint_task());
      contract.completion = pallet_deos_actors::CompletionPolicy::Persistent;
      assert_noop!(
        Actors::update_contract(RuntimeOrigin::signed(ALICE), active_id, contract),
        pallet_deos_actors::Error::<Runtime>::MintNotAllowedForUserActor
      );

      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        Some(contract_with_task(
          pallet_deos_actors::Trigger::immediate_manual(),
          mint_task(),
        )),
      ));
      let system_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let sovereign = Actors::active_actor_state(system_id)
        .expect("system mint actor exists")
        .identity
        .sovereign_account;
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        system_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(sovereign), 100);
      assert_eq!(
        Actors::active_actor_state(system_id)
          .expect("system actor remains")
          .identity
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
      let plan = active_contract(
        pallet_deos_actors::Trigger::immediate_manual(),
        2,
        alloc::vec![
          step(
            pallet_deos_actors::Task::Transfer {
              to: BOB,
              asset: NATIVE_ASSET,
              amount: pallet_deos_actors::AmountResolution::Fixed(5),
            },
            pallet_deos_actors::StepErrorPolicy::AbortCycle,
          ),
          temporary_swap_step(),
          step(
            pallet_deos_actors::Task::Transfer {
              to: BOB,
              asset: NATIVE_ASSET,
              amount: pallet_deos_actors::AmountResolution::Fixed(7),
            },
            pallet_deos_actors::StepErrorPolicy::AbortCycle,
          ),
        ],
      );
      prefund_active_contract(ALICE, &plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(plan),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);

      let suspended = Actors::continuation_state(actor_id).expect("temporary failure suspends");
      assert_eq!(suspended.cursor, 1);
      assert_eq!(suspended.unsuccessful_attempts_at_cursor, 1);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(5));
      System::set_block_number(2);
      let _ = Actors::on_idle(2, Weight::MAX);
      assert_eq!(
        Actors::continuation_state(actor_id)
          .expect("cooldown defers retry")
          .unsuccessful_attempts_at_cursor,
        1
      );

      System::set_block_number(3);
      let _ = Actors::on_idle(3, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor completes")
          .identity
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
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        Some(active_contract(
          pallet_deos_actors::Trigger::immediate_manual(),
          1,
          alloc::vec![temporary_swap_step()],
        )),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        1_000,
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("system actor suspends")
          .hot
          .cycle_state,
        pallet_deos_actors::CycleState::Suspended
      );

      System::set_block_number(2);
      let _ = Actors::on_idle(2, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("system actor completes")
          .identity
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
      let trigger = pallet_deos_actors::Trigger::immediate_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      );
      let contract = active_contract(trigger, 2, alloc::vec![temporary_swap_step()]);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      let before = Actors::active_actor_state(actor_id).expect("actor suspends");
      let before_hot =
        pallet_deos_actors::ActorHot::<Runtime>::get(actor_id).expect("hot state exists");
      assert_eq!(
        before.hot.cycle_state,
        pallet_deos_actors::CycleState::Suspended
      );

      let sovereign = before.identity.sovereign_account;
      let first_call =
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
          dest: sovereign,
          value: 1_000,
        });
      assert!(matches!(
        Executive::apply_extrinsic(signed_extrinsic(ALICE, 0, first_call)),
        Ok(Ok(_))
      ));
      let after = Actors::active_actor_state(actor_id).expect("actor remains suspended");
      let after_hot =
        pallet_deos_actors::ActorHot::<Runtime>::get(actor_id).expect("hot state remains");
      assert!(after.hot.pending_signal);
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
      let repeated_hot =
        pallet_deos_actors::ActorHot::<Runtime>::get(actor_id).expect("hot state remains");
      assert_eq!(repeated_hot.queue_ticket, after_hot.queue_ticket);
      assert_eq!(repeated_hot.wakeup_pointer, after_hot.wakeup_pointer);
      System::set_block_number(2);
      assert_ok!(Actors::cancel_continuation(
        RuntimeOrigin::signed(ALICE),
        actor_id,
      ));
      assert!(Actors::continuation_state(actor_id).is_none());
      assert!(Actors::pending_signal(actor_id));

      System::set_block_number(3);
      let _ = Actors::on_idle(3, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("latched run completes")
          .identity
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
      let contract = active_contract(
        pallet_deos_actors::Trigger::immediate_manual(),
        1,
        alloc::vec![temporary_swap_step()],
      );
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      let sovereign = Actors::active_actor_state(actor_id)
        .expect("actor exists")
        .identity
        .sovereign_account;
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_some());
      let balance_before = Balances::free_balance(sovereign);
      System::set_block_number(2);
      assert_ok!(Actors::cancel_continuation(
        RuntimeOrigin::signed(ALICE),
        actor_id,
      ));
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert!(Actors::active_actor_state(actor_id).is_none());
      assert_eq!(Balances::free_balance(sovereign), balance_before);
    });
  }

  #[test]
  fn abort_policy_terminates_permanent_unsupported_failure() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        Some(active_contract(
          pallet_deos_actors::Trigger::immediate_manual(),
          0,
          alloc::vec![step(
            pallet_deos_actors::Task::Stake {
              asset: NATIVE_ASSET,
              amount: pallet_deos_actors::AmountResolution::Fixed(10),
            },
            pallet_deos_actors::StepErrorPolicy::AbortCycle,
          )],
        )),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      let actor = Actors::active_actor_state(actor_id).expect("actor remains after first failure");
      assert_eq!(actor.hot.unsuccessful_attempt_streak, 1);
      assert_eq!(actor.hot.cycle_state, pallet_deos_actors::CycleState::Idle);
      assert!(Actors::continuation_state(actor_id).is_none());
    });
  }

  #[test]
  fn immutable_and_unsupported_paths_never_create_continuation() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let unsupported_retry = active_contract(
        pallet_deos_actors::Trigger::immediate_manual(),
        1,
        alloc::vec![step(
          pallet_deos_actors::Task::Stake {
            asset: NATIVE_ASSET,
            amount: pallet_deos_actors::AmountResolution::Fixed(10),
          },
          pallet_deos_actors::StepErrorPolicy::RetryLater { max_attempts: 3 },
        )],
      );
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          pallet_deos_actors::Mutability::Immutable,
          Some(unsupported_retry.clone()),
        ),
        pallet_deos_actors::Error::<Runtime>::RetryLaterNotAllowedForImmutableActor
      );
      prefund_active_contract(ALICE, &unsupported_retry);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(unsupported_retry),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor remains")
          .hot
          .cycle_state,
        pallet_deos_actors::CycleState::Idle
      );
    });
  }

  #[test]
  fn independent_runtime_binds_nonzero_continuation_weights() {
    let suspend = <pallet_deos_actors::weights::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::continuation_suspend(12);
    let retry =
      <pallet_deos_actors::weights::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::continuation_retry(
      );
    let cancel = <pallet_deos_actors::weights::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::continuation_cancel();
    assert!(suspend.ref_time() > retry.ref_time());
    assert!(cancel.proof_size() > retry.proof_size());
  }

  #[cfg(all(feature = "dex-fixture", feature = "try-runtime"))]
  #[test]
  fn try_state_accepts_a_suspended_independent_actor() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        pallet_deos_actors::Mutability::Mutable,
        Some(active_contract(
          pallet_deos_actors::Trigger::immediate_manual(),
          1,
          alloc::vec![temporary_swap_step()],
        )),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        1_000
      ));
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_some());
      assert!(Actors::try_state(1).is_ok());
    });
  }

  #[cfg(not(feature = "dex-fixture"))]
  #[test]
  fn unsupported_adapters_follow_ordinary_step_error_policy() {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let plan = BoundedVec::try_from(alloc::vec![
        pallet_deos_actors::StepOf::<Runtime> {
          precondition: None,
          task: pallet_deos_actors::Task::Stake {
            asset: NATIVE_ASSET,
            amount: pallet_deos_actors::AmountResolution::Fixed(10),
          },
          on_error: pallet_deos_actors::StepErrorPolicy::ContinueNextStep,
        },
        pallet_deos_actors::StepOf::<Runtime> {
          precondition: None,
          task: pallet_deos_actors::Task::Transfer {
            to: BOB,
            asset: NATIVE_ASSET,
            amount: pallet_deos_actors::AmountResolution::Fixed(5),
          },
          on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
        },
      ])
      .expect("two-step plan fits");
      let contract = pallet_deos_actors::ActorContract {
        trigger: pallet_deos_actors::Trigger::immediate_manual(),
        cooldown_blocks: 0,
        window: None,
        steps: plan,
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: pallet_deos_actors::FundingSourcePolicy::AnyVerifiedIngress,
        auto_close_at_cycle_nonce: None,
      };
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let bob_before = Balances::free_balance(BOB);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(5));
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor remains")
          .identity
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
      let task = pallet_deos_actors::Task::SwapOut {
        asset_out: 1,
        amount_out: pallet_deos_actors::AmountResolution::Fixed(50),
        asset_in: NATIVE_ASSET,
        input_limit: pallet_deos_actors::InputLimit::Absolute(100),
        slippage_tolerance: Perbill::zero(),
      };
      let contract = contract_with_task(pallet_deos_actors::Trigger::immediate_manual(), task);
      prefund_active_contract(ALICE, &contract);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        pallet_deos_actors::Mutability::Mutable,
        Some(contract),
      ));
      let actor_id = pallet_deos_actors::NextActorId::<Runtime>::get().saturating_sub(1);
      assert_ok!(transfer_and_notify_actor(
        actor_id,
        &ALICE,
        NATIVE_ASSET,
        100_000_000_000,
      ));
      let sink_before = Balances::free_balance(DEX_SINK);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let _ = Actors::on_idle(1, Weight::MAX);
      assert_eq!(
        Balances::free_balance(DEX_SINK),
        sink_before.saturating_add(50)
      );
      assert_eq!(
        Actors::active_actor_state(actor_id)
          .expect("actor remains")
          .identity
          .cycle_nonce,
        1
      );
    });
  }

  #[cfg(not(feature = "dex-fixture"))]
  #[test]
  fn optional_domain_adapters_fail_deterministically() {
    assert_eq!(
      <() as pallet_deos_actors::DexOps<AccountId, AssetId, Balance>>::swap_exact_out(
        pallet_deos_actors::ExecutionContext::new(&ALICE, pallet_deos_actors::ActorType::User),
        NATIVE_ASSET,
        1,
        10,
        10,
        Perbill::zero(),
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("DexOps not configured",)
      ))
    );
    assert_eq!(
      <() as pallet_deos_actors::StakingOps<AccountId, AssetId, Balance>>::stake(
        &ALICE,
        NATIVE_ASSET,
        10,
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("StakingOps not configured",)
      ))
    );
    assert_eq!(
      <() as pallet_deos_actors::LiquidityOps<AccountId, AssetId, Balance>>::add_liquidity(
        &ALICE,
        NATIVE_ASSET,
        1,
        10,
        10,
        1,
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("LiquidityOps not configured",)
      ))
    );
    assert_eq!(
      <() as pallet_deos_actors::LiquidityOps<AccountId, AssetId, Balance>>::remove_liquidity(
        &ALICE,
        NATIVE_ASSET,
        NATIVE_ASSET,
        1,
        10,
        1,
        1,
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("LiquidityOps not configured",)
      ))
    );
    assert_eq!(
      <() as pallet_deos_actors::LiquidityOps<AccountId, AssetId, Balance>>::donate_liquidity(
        &ALICE,
        NATIVE_ASSET,
        1,
        10,
        10,
        Perbill::zero(),
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("LiquidityOps not configured",)
      ))
    );
  }
}
