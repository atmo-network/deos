use super::common::{
  ALICE, ASSET_A, BOB, CHARLIE, add_liquidity, create_pool, create_test_asset, deos_router_account,
  mint_tokens, seeded_test_ext, update_actor_contract_partial,
};
macro_rules! update_actor_contract_partial {
  ($origin:expr, $actor:expr, $value:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, $value)
  };
  ($origin:expr, $actor:expr, $first:expr, $second:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, ($first, $second))
  };
}

use crate::{
  AccountId, Actors, Address, Assets, Balance, Balances, Executive, Oracle, Runtime, RuntimeCall,
  RuntimeEvent, RuntimeOrigin, Signature, Staking, System, TxExtension, UncheckedExtrinsic,
  configs::{
    RuntimeAddressEventIngress,
    actor_config::{
      TmctolAssetOps, TmctolDexOps, TmctolFeeCollector, TmctolGenesisSystemActors,
      TmctolLiquidityOps, classify_remove_liquidity_failure, classify_router_execution_failure,
      classify_router_failure, validate_remove_liquidity_output,
    },
    address_event_ingress::AddressEventIngressExtension,
    deos_router_config::market_execution_failure,
    pool_index::PoolIndexExtension,
  },
};
use alloc::boxed::Box;
use codec::Encode;
use pallet_deos_actors::adapters::SovereignAccountPolicy;
use pallet_deos_actors::{
  ActiveContractInput, ActorId, ActorType, AmountResolution, AssetFilter, AssetFilterOf, AssetOps,
  CloseReason, CompletionPolicy, ContractInput, CycleResult, DexOps, Error, Event,
  ExecutionContext, ExecutionPlanOf, FeeCollector, FundingSourcePolicy, IdleStarvationPhase,
  IdleStarvationState, InputLimit, LiquidityOps, Mutability, OutcomeTotals, RetryClass, Schedule,
  ScheduleOf, ScheduleWindow, SimulationMode, SimulationStatus, SimulationStepOutcome,
  SourceFilter, SourceFilterOf, SplitLeg, SplitTransferLegsOf, StakingOps, StepErrorPolicy, StepOf,
  StepSkippedReason, Task, TaskOf, Trigger, TriggerSource, WeightInfo,
};
use pallet_deos_router::FeeRoutingAdapter;
use polkadot_sdk::frame_support::{
  BoundedVec, assert_noop, assert_ok,
  dispatch::{DispatchClass, GetDispatchInfo},
  traits::{
    Currency, ExistenceRequirement, Get, GetStorageVersion, Hooks, ReservableCurrency,
    StorageVersion,
    fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
    tokens::imbalance::{ImbalanceAccounting, UnsafeConstructorDestructor, UnsafeManualAccounting},
  },
  weights::Weight,
};
use polkadot_sdk::sp_core::{Pair, crypto::Ss58Codec, sr25519};
use polkadot_sdk::sp_runtime::traits::{AccountIdConversion, TransactionExtension};
use polkadot_sdk::sp_runtime::{DispatchError, Perbill, generic};
use polkadot_sdk::sp_weights::{WeightMeter, WeightToFee};
use polkadot_sdk::{
  staging_xcm as xcm,
  staging_xcm_executor::{AssetsInHolding, traits::TransactAsset},
};
use primitives::AssetKind;

type RuntimeSchedule = ScheduleOf<Runtime>;
type RuntimeSourceFilter = SourceFilterOf<Runtime>;

#[test]
fn canonical_actors_seed_derives_documented_accounts() {
  let pallet_account: AccountId =
    crate::configs::actor_config::ActorsPalletId::get().into_account_truncating();
  assert_eq!(
    pallet_account,
    AccountId::from_ss58check("5EYCAe5fiQWMqjyVakD96Nwxv8toW2XYiWaTHmnmop8X9u5J").unwrap()
  );
  let expected = [
    "5HG3S6PLHrykv65Vw8j19zRaEx2Bmb37iywfo2qK3cHosGKX",
    "5Eiik51gjANLwbjZUXnVJv8pPpoTTVVic2x5sNwy8NaoVaJ9",
    "5EL8uyEoZA3JQkhCC3ackopXhdujtKjHHRYVSM1BVrf5x6LW",
    "5DHChJzyAY9pz54d6PXLmScG5vhdiarfNY2VjhkP4pG8vqSs",
    "5F6w8Jd8mHTPphhHgBdUJdkTaT2hQ8mKYojDhzCre5TJqGPg",
    "5CMBGiT8bLjfecCBLf7jSeWXoHKwEXtF7epoFHaLSTmxPhyp",
    "5Epu2U8sJbpBH1AQhc2KW6yuPA62Hst9r3zSdEHx4vS386JW",
    "5CvGRScqAYFFZRymun1fNJogwgUZCigd2ncmxCGvpquWy4nM",
    "5FZaRybmQEh2eHXM95zB2tyty3vxBZPyrCYTekHu5YxuCKj8",
    "5CeoQfeA6zkG7yToYZm3L8g5gjR5aMikm4b1gVLK69CgYzsC",
    "5H3KvwhcEmU5QZNcXWjwwmtduXdrKTrR5WYZqjrJm23KK14u",
    "5D7ZRz4hMphgVdq9UYBA9Gtk1q2cBjKTgoDCqpBETQi6Ziq4",
    "5EoWnoVuB925BHs9UwHUfLkcm5rSbmqzrHgFZRzY5nA4M5B6",
    "5CE6WsJ12vyyjAPMuvaqf2cdSQMVzAAxVjZDvXZK99VswFGe",
    "5CX93X5agA9cbvbv4JKpXmR8RF9ywdLbyg6WR9qY15evri5L",
  ];
  for (actor_id, expected) in expected.into_iter().enumerate() {
    assert_eq!(
      Actors::sovereign_account_id_system(actor_id as u64),
      AccountId::from_ss58check(expected).unwrap()
    );
  }
}
type RuntimeAssetFilter = AssetFilterOf<Runtime>;
type RuntimeTask = TaskOf<Runtime>;
type RuntimeStep = StepOf<Runtime>;
type ExecutionPlan = ExecutionPlanOf<Runtime>;

#[test]
fn runtime_oracle_change_hook_coalesces_into_actor_dirty_feed_state() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(7));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    create_system(
      ALICE,
      observation_schedule(feed),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("one step fits"),
    );

    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    let first = Actors::dirty_observation_feeds(feed).expect("Actors hook marks the feed dirty");
    assert_eq!(first.latest_revision, 1);
    assert_eq!(first.fanout_revision, 0);
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000_000_000_000,
    ));
    assert_eq!(Actors::dirty_observation_feeds(feed), Some(first));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      2_000_000_000_000,
    ));
    let latest = Actors::dirty_observation_feeds(feed).expect("dirty feed remains coalesced");
    assert_eq!(latest.previous_dirty_feed, first.previous_dirty_feed);
    assert_eq!(latest.next_dirty_feed, first.next_dirty_feed);
    assert_eq!(latest.latest_revision, 2);
    assert_eq!(Actors::dirty_observation_feed_count(), 1);
  });
}

#[test]
fn oracle_publication_rolls_back_when_actor_change_hook_rejects() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let producer = deos_router_account();
    let feed =
      crate::configs::oracle_config::deos_router_pool_feed(AssetKind::Native, AssetKind::Local(8));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      primitives::OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      pallet_oracle::Aggregation::Ema {
        half_life_blocks: 100,
      },
      pallet_oracle::ZeroPolicy::Reject,
      false,
    ));
    create_system(
      ALICE,
      observation_schedule(feed),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("one step fits"),
    );
    pallet_deos_actors::DirtyObservationListState::<Runtime>::mutate(|list| {
      list.count = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get()
        .saturating_mul(<Runtime as pallet_deos_actors::Config>::MaxTriggerSources::get());
    });
    let actor_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_noop!(
      Oracle::publish(
        RuntimeOrigin::signed(producer.clone()),
        feed,
        1_000_000_000_000
      ),
      Error::<Runtime>::DirtyObservationCapacityExceeded
    );
    assert!(Oracle::observations(feed).is_none());
    assert!(Actors::dirty_observation_feeds(feed).is_none());
    assert_eq!(Actors::dirty_observation_list(), actor_before);
    assert_eq!(System::events(), events_before);

    pallet_deos_actors::DirtyObservationListState::<Runtime>::kill();
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      1_000_000_000_000,
    ));
    assert_eq!(
      Oracle::observations(feed).expect("retry commits").revision,
      1
    );
    assert_eq!(
      Actors::dirty_observation_feeds(feed)
        .expect("retry reaches Actors")
        .latest_revision,
      1
    );
  });
}

#[test]
fn native_flow_anchor_topology_is_unique_and_funded_with_one_ed() {
  super::common::new_test_ext().execute_with(|| {
    let anchors = TmctolGenesisSystemActors::native_flow_anchor_accounts();
    let unique = anchors.iter().collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), anchors.len());
    for (index, account) in anchors.into_iter().enumerate() {
      assert_eq!(
        Balances::free_balance(&account),
        crate::EXISTENTIAL_DEPOSIT,
        "native-flow anchor {index} ({account:?}) must start with one ED"
      );
    }
  });
}

#[test]
fn actor_0_7_storage_schema_is_a_fresh_genesis_baseline() {
  seeded_test_ext().execute_with(|| {
    let baseline = StorageVersion::new(1);
    assert_eq!(Actors::in_code_storage_version(), baseline);
    assert_eq!(Actors::on_chain_storage_version(), baseline);
  });
}

fn signed_extrinsic(
  signer: &sr25519::Pair,
  nonce: crate::Nonce,
  call: RuntimeCall,
) -> UncheckedExtrinsic {
  let tx_ext = TxExtension::new((
    polkadot_sdk::frame_system::AuthorizeCall::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckNonZeroSender::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckSpecVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckTxVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckGenesis::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(nonce),
    polkadot_sdk::frame_system::CheckWeight::<Runtime>::new(),
    AddressEventIngressExtension,
    PoolIndexExtension,
    polkadot_sdk::pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  ));
  let payload =
    generic::SignedPayload::new(call.clone(), tx_ext.clone()).expect("signed payload must encode");
  let signature = payload.using_encoded(|encoded| signer.sign(encoded));
  let account = crate::AccountId::from(signer.public());
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(account),
    Signature::Sr25519(signature),
    tx_ext,
  )
}

fn make_step(task: RuntimeTask) -> RuntimeStep {
  StepOf::<Runtime> {
    preconditions: pallet_deos_actors::Preconditions::Unconditional,
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }
}

fn all_preconditions(
  predicates: Vec<pallet_deos_actors::Predicate<AssetKind, u128, u32, primitives::OracleFeedId>>,
) -> pallet_deos_actors::PreconditionsOf<Runtime> {
  let clause = BoundedVec::try_from(
    predicates
      .into_iter()
      .map(|predicate| pallet_deos_actors::TimedPredicate {
        timing: pallet_deos_actors::ObservationTiming::Current,
        predicate,
      })
      .collect::<Vec<_>>(),
  )
  .expect("runtime predicates fit");
  pallet_deos_actors::Preconditions::AnyOf(
    BoundedVec::try_from(vec![clause]).expect("runtime clause fits"),
  )
}

fn any_preconditions(
  predicates: Vec<pallet_deos_actors::Predicate<AssetKind, u128, u32, primitives::OracleFeedId>>,
) -> pallet_deos_actors::PreconditionsOf<Runtime> {
  let clauses = predicates
    .into_iter()
    .map(|predicate| {
      BoundedVec::try_from(vec![pallet_deos_actors::TimedPredicate {
        timing: pallet_deos_actors::ObservationTiming::Current,
        predicate,
      }])
      .expect("runtime predicate fits")
    })
    .collect::<Vec<_>>();
  pallet_deos_actors::Preconditions::AnyOf(
    BoundedVec::try_from(clauses).expect("runtime clauses fit"),
  )
}

fn inert_task() -> RuntimeTask {
  Task::StopCycle
}

fn manual_schedule() -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::immediate_manual(),
    cooldown_blocks: 0,
  }
}

fn observation_schedule(feed: primitives::OracleFeedId) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::Immediate {
      sources: BoundedVec::try_from(vec![TriggerSource::OnObservationChange { feed }])
        .expect("one observation source fits"),
    },
    cooldown_blocks: 0,
  }
}

fn on_address_event_schedule(
  source_filter: RuntimeSourceFilter,
  asset_filter: RuntimeAssetFilter,
) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::immediate_manual_and_address_event(source_filter, asset_filter),
    cooldown_blocks: 0,
  }
}

fn transfer_execution_plan(to: crate::AccountId, asset: AssetKind, amount: u128) -> ExecutionPlan {
  BoundedVec::try_from(vec![make_step(Task::Transfer {
    to,
    asset,
    amount: AmountResolution::Fixed(amount),
  })])
  .expect("steps fits")
}

fn user_active_contract(
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: ExecutionPlan,
) -> pallet_deos_actors::ContractInputOf<Runtime> {
  pallet_deos_actors::ContractInput::Active(pallet_deos_actors::ActiveContractInput {
    schedule,
    schedule_window,
    steps,
    completion: pallet_deos_actors::CompletionPolicy::Persistent,
    funding: pallet_deos_actors::FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  })
}

fn system_active_contract(
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: ExecutionPlan,
) -> pallet_deos_actors::ContractInputOf<Runtime> {
  pallet_deos_actors::ContractInput::Active(pallet_deos_actors::ActiveContractInput {
    schedule,
    schedule_window,
    steps,
    completion: pallet_deos_actors::CompletionPolicy::Persistent,
    funding: pallet_deos_actors::FundingSourcePolicy::RuntimePolicy,
    auto_close_at_cycle_nonce: None,
  })
}

fn create_user(
  who: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: ExecutionPlan,
) -> ActorId {
  prefund_active_user_creation(&who, &steps);
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_user_actor(
    RuntimeOrigin::signed(who),
    Mutability::Mutable,
    user_active_contract(schedule, schedule_window, steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn create_system(
  owner: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  steps: ExecutionPlan,
) -> ActorId {
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_system_actor(
    RuntimeOrigin::root(),
    owner,
    Mutability::Mutable,
    system_active_contract(schedule, schedule_window, steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn age_fixture_control_clock(actor_id: ActorId) {
  let now = System::block_number();
  if now == 0 {
    System::set_block_number(1);
    return;
  }
  pallet_deos_actors::ActorIdentities::<Runtime>::mutate(actor_id, |maybe| {
    maybe
      .as_mut()
      .expect("fixture actor identity exists")
      .last_control_mutation_block = now.saturating_sub(1);
  });
}

fn actor_funding(actor_id: ActorId) -> pallet_deos_actors::ActorFundingStateOf<Runtime> {
  Actors::actor_funding(actor_id).expect("active actor funding exists")
}

fn actor_account(actor_id: ActorId) -> crate::AccountId {
  Actors::active_actor_view(actor_id)
    .map(|instance| instance.sovereign_account)
    .expect("Actors must exist")
}

fn fund_native(actor_id: ActorId, amount: u128) {
  let actor_acc = actor_account(actor_id);
  let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&actor_acc, amount);
}

/// User Active prefunding requirement: `MinUserBalance + attempt_fee_envelope(plan, 0, User).total`.
fn user_prefunding_requirement(plan: &ExecutionPlan) -> u128 {
  <Runtime as pallet_deos_actors::Config>::MinUserBalance::get().saturating_add(
    Actors::attempt_fee_envelope(ActorType::User, plan, 0)
      .expect("fixture plan has a checked fee envelope")
      .total,
  )
}

/// Lowest free owner slot for the deterministic prospective User sovereign; mirrors the
/// pallet's `available_owner_slot(None)` lowest-free-slot scan over the public bitmap.
fn lowest_free_owner_slot(owner: &crate::AccountId) -> u8 {
  let bitmap = pallet_deos_actors::OwnerSlotBitmaps::<Runtime>::get(owner);
  let max_slots = <Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get();
  for (byte_index, byte) in bitmap.iter().enumerate() {
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
    let free_bits = !*byte & valid_bits;
    if free_bits != 0 {
      return (first_slot + free_bits.trailing_zeros() as usize) as u8;
    }
  }
  panic!("fixture owner has no free User owner slot");
}

/// Pre-funds the deterministic User sovereign so Active creation/activation admits (spec 7.1)
/// without mutating any pallet state.
fn prefund_user_sovereign(owner: &crate::AccountId, slot: u8, plan: &ExecutionPlan) {
  let sovereign = Actors::sovereign_account_id(owner, slot);
  let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
    &sovereign,
    user_prefunding_requirement(plan),
  );
}

/// Pre-funds the next automatically allocated User slot for a direct Active creation fixture.
fn prefund_active_user_creation(owner: &crate::AccountId, plan: &ExecutionPlan) {
  let slot = lowest_free_owner_slot(owner);
  prefund_user_sovereign(owner, slot, plan);
}

/// Depletes the sovereign fee-native balance after creation, restoring the historical
/// unfunded post-creation fixture state while keeping creation itself admitted.
fn deplete_user_sovereign(actor_id: ActorId, amount: u128) {
  let acc = actor_account(actor_id);
  let (_, remainder) = <Balances as Currency<crate::AccountId>>::slash(&acc, amount);
  assert_eq!(
    remainder, 0,
    "fixture depletion must not overdraw the sovereign"
  );
}

#[test]
fn deos_runtime_executes_unconditional_and_dnf_with_fixed_successors() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let transfer = |amount| Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(amount),
    };
    let plan = BoundedVec::try_from(vec![
      pallet_deos_actors::Step {
        preconditions: pallet_deos_actors::Preconditions::Unconditional,
        task: transfer(7),
        on_error: StepErrorPolicy::AbortCycle,
      },
      pallet_deos_actors::Step {
        preconditions: all_preconditions(vec![pallet_deos_actors::Predicate::BlockNumberAbove {
          threshold: 0,
        }]),
        task: transfer(11),
        on_error: StepErrorPolicy::AbortCycle,
      },
      pallet_deos_actors::Step {
        preconditions: any_preconditions(vec![pallet_deos_actors::Predicate::BlockNumberAbove {
          threshold: 0,
        }]),
        task: transfer(13),
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .expect("three-step User plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 10_000_000_000_000);
    let bob_before = Balances::free_balance(BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let _ = Actors::on_idle(1, Weight::MAX);
    assert_eq!(Balances::free_balance(BOB), bob_before.saturating_add(31));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains active")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn genesis_anchor_buckets_are_custody_only_accounts() {
  seeded_test_ext().execute_with(|| {
    for actor_id in [
      primitives::ecosystem::actor_ids::TOL_BUCKET_A_ACTORS_ID,
      primitives::ecosystem::actor_ids::BLDR_BUCKET_A_ACTORS_ID,
    ] {
      let sovereign = Actors::sovereign_account_id_system(actor_id);
      assert!(Actors::active_actor_view(actor_id).is_none());
      assert!(Actors::actor_identities(actor_id).is_none());
      assert!(Actors::sovereign_index(sovereign).is_none());
      let plan = transfer_execution_plan(BOB, AssetKind::Native, 1);
      assert_noop!(
        update_actor_contract_partial!(
          RuntimeOrigin::root(),
          actor_id,
          (plan, CompletionPolicy::Persistent,)
        ),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::pause_actor(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::manual_trigger(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
      assert_noop!(
        Actors::close_actor(RuntimeOrigin::root(), actor_id),
        Error::<Runtime>::ActorNotFound
      );
    }
  });
}

fn fund_native_via_call(funder: crate::AccountId, actor_id: ActorId, amount: u128) {
  let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
  let provenance = pallet_deos_actors::FundingProvenance::Signed;
  assert_ok!(Actors::preflight_funding_event(
    actor_id,
    AssetKind::Native,
    amount,
    Some(&funder),
    Some(&provenance),
  ));
  assert_ok!(<Balances as Currency<crate::AccountId>>::transfer(
    &funder,
    &instance.sovereign_account,
    amount,
    polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
  ));
  assert_ok!(Actors::notify_address_event(
    actor_id,
    AssetKind::Native,
    amount,
    &funder
  ));
}

fn native_balance(who: &crate::AccountId) -> u128 {
  Balances::free_balance(who)
}

fn account_location(who: crate::AccountId) -> xcm::latest::Location {
  let mut id = [0u8; 32];
  id.copy_from_slice(who.as_ref());
  xcm::latest::Location::new(
    0,
    [xcm::latest::Junction::AccountId32 { network: None, id }],
  )
}

fn native_xcm_asset(amount: u128) -> xcm::latest::Asset {
  xcm::latest::Asset {
    id: xcm::latest::AssetId(xcm::latest::Location::parent()),
    fun: xcm::latest::Fungibility::Fungible(amount),
  }
}

#[derive(Clone)]
struct MockCredit(u128);

impl UnsafeConstructorDestructor<u128> for MockCredit {
  fn unsafe_clone(&self) -> Box<dyn ImbalanceAccounting<u128>> {
    Box::new(Self(self.0))
  }

  fn forget_imbalance(&mut self) -> u128 {
    core::mem::take(&mut self.0)
  }
}

impl UnsafeManualAccounting<u128> for MockCredit {
  fn saturating_subsume(&mut self, mut other: Box<dyn ImbalanceAccounting<u128>>) {
    self.0 = self.0.saturating_add(other.amount());
    let _ = other.forget_imbalance();
  }
}

impl ImbalanceAccounting<u128> for MockCredit {
  fn amount(&self) -> u128 {
    self.0
  }

  fn saturating_take(&mut self, amount: u128) -> Box<dyn ImbalanceAccounting<u128>> {
    let taken = self.0.min(amount);
    self.0 -= taken;
    Box::new(Self(taken))
  }
}

fn asset_to_holding(asset: xcm::latest::Asset) -> AssetsInHolding {
  let mut holding = AssetsInHolding::new();
  match asset.fun {
    xcm::latest::Fungibility::Fungible(amount) => {
      holding
        .fungible
        .insert(asset.id, Box::new(MockCredit(amount)));
    }
    xcm::latest::Fungibility::NonFungible(instance) => {
      holding.non_fungible.insert((asset.id, instance));
    }
  }
  holding
}

fn run_idle(weight: Weight) {
  Actors::on_idle(System::block_number(), weight);
}

fn starvation_observation_weight() -> Weight {
  <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_on_idle_base()
}

/// Proof-limited on_idle budget that admits the wakeup cursor, queue scan, hot/contract probes, and
/// the head consume, but not the actor's full cycle admission. Materializes the only spec 8.6.3
/// starvation trigger: a live FIFO head blocked by weight with no admitted attempt.
fn starvation_blocked_budget(actor_id: ActorId) -> Weight {
  let base = starvation_observation_weight();
  let cursor = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_future();
  let scan =
    <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_tombstone_drain(1);
  let hot = Actors::scheduler_actor_hot_probe_weight_upper();
  let contract = Actors::scheduler_actor_contract_probe_weight_upper();
  let consume = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_preserve_page()
    .max(<<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_delete_page());
  let instance = Actors::active_actor_view(actor_id).expect("actor exists");
  let cycle =
    Actors::compute_cycle_weight_upper(instance.actor_class.actor_type(), &instance.steps);
  let full = base
    .saturating_add(cursor)
    .saturating_add(scan)
    .saturating_add(hot)
    .saturating_add(contract)
    .saturating_add(consume)
    .saturating_add(cycle);
  Weight::from_parts(u64::MAX, full.proof_size().saturating_sub(1))
}

fn run_idle_until_cycle_nonce(actor_id: ActorId, target_cycle_nonce: u64) {
  for _ in 0..20 {
    run_idle(Weight::MAX);
    if Actors::active_actor_view(actor_id)
      .map(|instance| instance.cycle_nonce >= target_cycle_nonce)
      .unwrap_or(false)
    {
      return;
    }
  }
  panic!("cycle nonce did not reach target");
}

fn actor_events() -> alloc::vec::Vec<Event<Runtime>> {
  System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(event) => Some(event),
      _ => None,
    })
    .collect()
}

pub fn has_actor_event(predicate: impl Fn(&Event<Runtime>) -> bool) -> bool {
  actor_events().iter().any(predicate)
}

// --- Actors Platform: Lifecycle ---

#[test]
fn manual_trigger_executes_transfer_execution_plan() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 5_000_000_000_000u128;
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            committed_effectful_tasks: 1,
            skipped_conditions: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn productive_run_completion_closes_runtime_actor_after_committed_transfer() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 5_000_000_000_000u128;
    let actor_id = Actors::next_actor_id();
    let contract = ContractInput::Active(ActiveContractInput {
      schedule: manual_schedule(),
      schedule_window: None,
      steps: transfer_execution_plan(BOB, AssetKind::Native, amount),
      completion: CompletionPolicy::CloseAfterProductiveCycle,
      funding: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    });
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      contract.clone(),
    ));
    let actor = Actors::sovereign_account_id_system(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    let simulation = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready productive contract simulates");
    assert_eq!(
      simulation.status,
      SimulationStatus::Closed(CloseReason::ProductiveCycleCompleted)
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&actor), actor_before);

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(amount));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn native_staking_liquidity_actor_activation_requires_initialized_pool() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_noop!(
      TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1),
      DispatchError::Other("StakedAssetUnavailable")
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_noop!(
      TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1),
      DispatchError::Other("NativeStakingAmmUnavailable")
    );
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = AssetKind::Local(0);
    let staked_asset = AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    assert_ok!(TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1));
    let actor = Actors::active_actor_view(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    )
    .expect("Native Staking Liquidity Actor must exist");
    assert!(matches!(
      actor.steps.first().map(|step| &step.task),
      Some(Task::DonateLiquidity { .. })
    ));
  });
}

#[test]
fn pool_creation_owns_an_exact_lp_reverse_index() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_001;
    System::set_block_number(1);
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1));
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    assert_eq!(
      crate::DeosRouter::lp_pair_by_token_id(pool.lp_token),
      Some(pair)
    );
    assert_noop!(
      crate::DeosRouter::register_lp_pair(
        pool.lp_token,
        (
          AssetKind::Native,
          AssetKind::Local(INDEXED_ASSET.saturating_add(1))
        ),
      ),
      pallet_deos_router::Error::<Runtime>::LpTokenPairCollision
    );
  });
}

#[test]
fn remove_liquidity_requires_and_uses_the_exact_lp_reverse_index() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_002;
    System::set_block_number(1);
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    let liquidity = 1_000_000_000_000_000u128;
    assert_ok!(mint_tokens(
      INDEXED_ASSET,
      &ALICE,
      &ALICE,
      liquidity.saturating_mul(2),
    ));
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      pair.0,
      pair.1,
      liquidity,
      liquidity,
      1,
      1,
      &ALICE,
    ));
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    let lp_before_add_bound = Assets::balance(pool.lp_token, &ALICE);
    assert_eq!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::add_liquidity(
        &ALICE,
        pair.0,
        pair.1,
        liquidity / 10,
        liquidity / 10,
        Balance::MAX,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("MinimumLpOutputNotMet")
      ))
    );
    assert_eq!(Assets::balance(pool.lp_token, &ALICE), lp_before_add_bound);
    let lp_amount = Assets::balance(pool.lp_token, &ALICE) / 2;
    pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
      pairs.remove(&pool.lp_token);
    });
    assert_noop!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::remove_liquidity(
        &ALICE,
        AssetKind::Local(pool.lp_token),
        pair.0,
        pair.1,
        lp_amount,
        1,
        1,
      ),
      DispatchError::Other("Pool not found for LP token")
    );
    assert_ok!(crate::DeosRouter::register_lp_pair(pool.lp_token, pair));
    let lp_before_bound_failure = Assets::balance(pool.lp_token, &ALICE);
    assert_eq!(
      <TmctolLiquidityOps as LiquidityOps<AccountId, AssetKind, Balance>>::remove_liquidity(
        &ALICE,
        AssetKind::Local(pool.lp_token),
        pair.0,
        pair.1,
        lp_amount,
        Balance::MAX,
        Balance::MAX,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        crate::pallet_asset_conversion::Error::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum
      ))
    );
    assert_eq!(
      Assets::balance(pool.lp_token, &ALICE),
      lp_before_bound_failure
    );
    assert_ok!(<TmctolLiquidityOps as LiquidityOps<
      AccountId,
      AssetKind,
      Balance,
    >>::remove_liquidity(
      &ALICE,
      AssetKind::Local(pool.lp_token),
      pair.0,
      pair.1,
      lp_amount,
      1,
      1,
    ));
  });
}

#[test]
fn executive_pool_creation_indexes_the_lp_without_event_scanning() {
  seeded_test_ext().execute_with(|| {
    const INDEXED_ASSET: u32 = 901_003;
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[43u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    assert_ok!(create_test_asset(INDEXED_ASSET, &ALICE));
    crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
    let pair = (AssetKind::Native, AssetKind::Local(INDEXED_ASSET));
    let call =
      RuntimeCall::AssetConversion(polkadot_sdk::pallet_asset_conversion::Call::create_pool {
        asset1: Box::new(pair.0),
        asset2: Box::new(pair.1),
      });
    let result = Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call));
    assert!(matches!(result, Ok(Ok(_))), "{result:?}");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .expect("created pool must exist");
    assert_eq!(
      crate::DeosRouter::lp_pair_by_token_id(pool.lp_token),
      Some(pair)
    );
  });
}

#[test]
fn system_actor_executes_native_staking_lp_donation_task() {
  seeded_test_ext().execute_with(|| {
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;
    System::set_block_number(1);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = AssetKind::Local(0);
    let staked_asset = AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .expect("NTVE/stNTVE pool id must resolve");
    let pool_account =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
        .expect("NTVE/stNTVE pool account must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    let ratio_failure =
      crate::configs::AssetConversionAdapter::donate_balanced_liquidity_classified(
        &BOB,
        base_asset,
        staked_asset,
        40,
        20,
        Perbill::from_percent(1),
      )
      .expect_err("ratio movement must fail before transfer");
    assert_eq!(
      ratio_failure.retry,
      pallet_deos_actors::RetryClass::Temporary
    );
    let lp_supply_before =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    let steps = TmctolGenesisSystemActors::build_native_staking_liquidity_execution_plan(1);
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      0,
      sovereign.clone().into(),
      81,
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let lp_supply_after =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after, lp_supply_before);
    assert_eq!(Assets::balance(0, pool_account.clone()), 440);
    assert_eq!(Assets::balance(staked_asset_id, pool_account), 440);
    assert_eq!(Assets::balance(0, sovereign.clone()), 1);
    assert_eq!(Assets::balance(staked_asset_id, sovereign), 0);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::LiquidityDonated {
          actor_id: id,
          asset_a: AssetKind::Local(0),
          asset_b,
          max_amount_a: 80,
          amount_a: 40,
          amount_b: 40,
          ..
        } if *id == actor_id && *asset_b == AssetKind::Local(staked_asset_id)
      )
    }));
  });
}

#[test]
fn create_user_charges_creation_fee_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let sink_before = native_balance(&fee_sink);
    let alice_before = native_balance(&ALICE);
    let _ = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(native_balance(&ALICE), alice_before.saturating_sub(fee));
  });
}

#[test]
fn actor_fee_collector_routes_the_full_amount_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let payer = BOB;
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let payer_before = native_balance(&payer);
    let fee_sink_before = native_balance(&fee_sink);
    assert_ok!(TmctolFeeCollector::collect_fee(
      &payer,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));
    assert_eq!(native_balance(&payer), payer_before.saturating_sub(amount));
    assert_eq!(
      native_balance(&fee_sink),
      fee_sink_before.saturating_add(amount)
    );
    // Fee collection is an explicit certified producer: it latches Fee Sink
    // readiness via the paired AddressEvent (payer source, internal-protocol
    // provenance) so the bounded split plan becomes schedulable.
    assert!(
      Actors::pending_signal(fee_sink_id),
      "Fee Sink must latch readiness after fee collection"
    );
    let hot = Actors::actor_hot(fee_sink_id).expect("Fee Sink hot state");
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
    // Trigger matching and funding authorization stay independent: the Fee Sink's
    // default-deny RuntimePolicy accumulates no authoritative funding from the fee
    // ingress, yet readiness latches for the bounded split plan.
    let funding = Actors::actor_funding(fee_sink_id).expect("Fee Sink funding state");
    assert!(funding.funding_accumulated.is_empty());
  });
}

#[test]
fn fee_sink_redistributes_native_fifty_fifty_to_staking_and_liquidity() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    // Genesis seeds the Fee Sink with an initial balance; add a fresh inflow so the split is
    // observable on top of the seeded anchor.
    let inflow = 2_000_000_000_000_000u128;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&fee_sink, inflow);
    let total = native_balance(&fee_sink);
    let staking_pool = crate::Staking::pool_account_for(0);
    let staking_liquidity_actor = crate::Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let pool_before = native_balance(&staking_pool);
    let liquidity_before = native_balance(&staking_liquidity_actor);

    // Governance owns the Fee Sink; the Manual source is enabled, so root triggers the cycle.
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), fee_sink_id));
    run_idle(Weight::MAX);

    // The Phase-1 plan splits AllAvailable exactly 50/50 to the staking pool account and the
    // staking Liquidity Actor sovereign; only the indivisible remainder stays in the sink.
    let pool_delta = native_balance(&staking_pool).saturating_sub(pool_before);
    let liquidity_delta = native_balance(&staking_liquidity_actor).saturating_sub(liquidity_before);
    let distributed = pool_delta.saturating_add(liquidity_delta);
    assert_eq!(
      pool_delta, liquidity_delta,
      "Fee Sink must split its native balance exactly 50/50 between staking ingress and liquidity"
    );
    assert!(
      distributed >= total.saturating_sub(2 * crate::EXISTENTIAL_DEPOSIT),
      "nearly all Fee Sink balance is distributed (total={total}, distributed={distributed})"
    );
    assert!(
      native_balance(&fee_sink) <= 2 * crate::EXISTENTIAL_DEPOSIT,
      "only the free-balance anchor stays in Fee Sink, got {}",
      native_balance(&fee_sink)
    );
  });
}

#[test]
fn permissionless_sweep_many_batches_lifecycle_evaluation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user_a_prefunded =
      user_prefunding_requirement(&transfer_execution_plan(BOB, AssetKind::Native, 1));
    let user_b_prefunded =
      user_prefunding_requirement(&transfer_execution_plan(ALICE, AssetKind::Native, 1));
    let user_a = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let user_b = create_user(
      BOB,
      manual_schedule(),
      None,
      transfer_execution_plan(ALICE, AssetKind::Native, 1),
    );
    deplete_user_sovereign(user_a, user_a_prefunded);
    deplete_user_sovereign(user_b, user_b_prefunded);
    let system_alive = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let sweep_ids: BoundedVec<ActorId, <Runtime as pallet_deos_actors::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b, system_alive]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_view(user_a).is_none());
    assert!(Actors::active_actor_view(user_b).is_none());
    assert!(Actors::active_actor_view(system_alive).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 3,
          closed: 2,
          alive: 1,
          missing: 0,
        }
      )
    }));
  });
}

#[test]
fn zombie_spam_attack_cost_dominates_batch_cleanup_cost() {
  seeded_test_ext().execute_with(|| {
    let active_cap = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    let creation_fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let create_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::create_user_actor();
    let create_tx_fee = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&create_weight);
    let attacker_cost_per_actor = creation_fee.saturating_add(create_tx_fee);
    let attacker_total_cost = attacker_cost_per_actor.saturating_mul(active_cap as u128);
    let sweep_batch_size = <Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get().max(1);
    let sweep_calls = active_cap.div_ceil(sweep_batch_size);
    let batch_sweep_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::permissionless_sweep_many(
        sweep_batch_size,
      );
    let batch_sweep_tx_fee =
      <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&batch_sweep_weight);
    let cleanup_total_cost = batch_sweep_tx_fee.saturating_mul(sweep_calls as u128);
    assert!(cleanup_total_cost > 0, "Cleanup fee floor must be non-zero");
    assert!(
      attacker_total_cost >= cleanup_total_cost.saturating_mul(100),
      "Creation-cost floor must dominate bounded cleanup cost by >=100x"
    );
    let cost_ratio_bp = attacker_total_cost.saturating_mul(10_000) / cleanup_total_cost;
    println!(
      "Actors zombie economics: active_cap={}, creation_fee={}, create_tx_fee={}, attacker_total_cost={}, sweep_batch_size={}, sweep_calls={}, batch_sweep_tx_fee={}, cleanup_total_cost={}, cost_ratio={:.2}x",
      active_cap,
      creation_fee,
      create_tx_fee,
      attacker_total_cost,
      sweep_batch_size,
      sweep_calls,
      batch_sweep_tx_fee,
      cleanup_total_cost,
      (cost_ratio_bp as f64) / 10_000.0,
    );
  });
}

#[test]
fn min_user_balance_is_not_below_native_existential_deposit() {
  seeded_test_ext().execute_with(|| {
    let configured_min_user_balance = crate::configs::actor_config::ActorMinUserBalance::get();
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let native_ed = <Balances as Currency<crate::AccountId>>::minimum_balance();
    assert_eq!(
      min_user_balance,
      configured_min_user_balance.max(native_ed),
      "Runtime MinUserBalance guard must clamp below-ED configurations"
    );
    assert!(
      min_user_balance >= native_ed,
      "MinUserBalance must be >= native ExistentialDeposit"
    );
  });
}

#[test]
fn paged_queue_limits_are_independent_runtime_controls() {
  seeded_test_ext().execute_with(|| {
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::QueuePageSize::get(),
      64,
      "64 is the balanced production choice from the 32/64/128 production-Wasm comparison"
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get(),
      10_000
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get(),
      64
    );
    let fanout_limit = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
    assert!(fanout_limit.ref_time() > 0 && fanout_limit.proof_size() > 0);
    assert!(fanout_limit.all_lte(
      <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get()
    ));
    assert!(
      crate::Actors::observation_change_ingress_weight().all_lte(fanout_limit)
    );
    assert!(
      <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_page()
        .all_lte(fanout_limit),
      "one maximum-density fanout page must fit the dedicated two-dimensional runtime budget"
    );
    assert_eq!(
      <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
      1_000,
      "the execution count is a safety ceiling; WeightMeter remains primary"
    );
    assert_ne!(
      <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get(),
      <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
      "physical inspection and successful execution must remain independent controls"
    );
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 0);
  });
}

#[test]
fn reactive_delivery_envelopes_follow_production_weights_and_topology_bounds() {
  let base =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_base();
  let unit =
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::observation_fanout_page();
  let limit = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
  let configured_units =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get());
  let available = limit.saturating_sub(base);
  let units_per_block = configured_units
    .min(available.ref_time() / unit.ref_time())
    .min(available.proof_size() / unit.proof_size());

  assert_eq!(base, Weight::from_parts(31_565_000, 1_543));
  assert_eq!(unit, Weight::from_parts(12_170_187_000, 166_430));
  assert_eq!(limit, Weight::from_parts(400_000_000_000, 1_000_000));
  assert_eq!(
    units_per_block, 5,
    "conservative ProofSize is the active fanout limit"
  );

  let max_actors = u64::from(<Runtime as pallet_deos_actors::Config>::MaxActiveActors::get());
  let page_size = u64::from(<Runtime as pallet_deos_actors::Config>::QueuePageSize::get());
  let max_sources = u64::from(<Runtime as pallet_deos_actors::Config>::MaxTriggerSources::get());
  let subscription_pages = max_actors.div_ceil(page_size);
  let dense_single_feed_units = subscription_pages;
  let sparse_high_slot_units = 1u64;
  let compact_four_feed_units = subscription_pages.saturating_mul(max_sources);
  let quiescent_revision_race_units = subscription_pages.saturating_mul(2);

  assert_eq!((max_actors, page_size, max_sources), (10_000, 64, 4));
  assert_eq!(subscription_pages, 157);
  assert_eq!(dense_single_feed_units.div_ceil(units_per_block), 32);
  assert_eq!(sparse_high_slot_units.div_ceil(units_per_block), 1);
  assert_eq!(compact_four_feed_units.div_ceil(units_per_block), 126);
  assert_eq!(quiescent_revision_race_units.div_ceil(units_per_block), 63);
}

#[test]
fn sched_workers_static_envelope_leaves_one_actor_unit_inside_guaranteed_budget() {
  use crate::weights::pallet_deos_actors::SubstrateWeight;
  type W = SubstrateWeight<Runtime>;
  let base = W::scheduler_on_idle_base();
  // Maximum wakeup worker envelope: cursor probe plus one worst-case complete wakeup unit per
  // `MaxWakeupsPerBlock` slot, capped by the dedicated two-dimensional `WakeupWeightLimit`.
  let cursor_probe = W::scheduler_wakeup_cursor_worker_future();
  let wakeup_unit = crate::Actors::wakeup_cursor_drain_unit_weight_upper(true);
  let wakeup_ceiling = <Runtime as pallet_deos_actors::Config>::WakeupWeightLimit::get();
  let wakeup_per_block =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxWakeupsPerBlock::get());
  let wakeup_envelope = cursor_probe
    .saturating_add(wakeup_unit.saturating_mul(wakeup_per_block))
    .min(wakeup_ceiling);
  assert!(wakeup_envelope.ref_time() > 0 && wakeup_envelope.proof_size() > 0);
  assert!(
    wakeup_envelope.all_lte(wakeup_ceiling),
    "worker stays in its own envelope"
  );

  // Maximum fanout worker envelope: base plus one page per configured slot, capped by the limit.
  let fanout_base = W::observation_fanout_base();
  let fanout_page = W::observation_fanout_page();
  let fanout_ceiling = <Runtime as pallet_deos_actors::Config>::ObservationFanoutWeightLimit::get();
  let fanout_per_block =
    u64::from(<Runtime as pallet_deos_actors::Config>::MaxObservationFanoutPagesPerBlock::get());
  let fanout_envelope = fanout_base
    .saturating_add(fanout_page.saturating_mul(fanout_per_block))
    .min(fanout_ceiling);
  assert!(
    fanout_envelope.all_lte(fanout_ceiling),
    "fanout stays in its own envelope"
  );

  // One maximum actor unit: admission overhead plus one full cycle admission plus pure cleanup.
  let actor_unit = crate::Actors::scheduler_admission_overhead()
    .saturating_add(crate::Actors::close_dispatch_weight_upper());
  let guaranteed = <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get();
  let combined = base
    .saturating_add(wakeup_envelope)
    .saturating_add(fanout_envelope)
    .saturating_add(actor_unit);
  assert!(
    combined.all_lte(guaranteed),
    "fixed base + max wakeup worker + max fanout worker + one max actor unit must fit ActorOnIdleReserve: base={base:?}, wakeup={wakeup_envelope:?}, fanout={fanout_envelope:?}, actor={actor_unit:?}, combined={combined:?}, guaranteed={guaranteed:?}"
  );
  println!(
    "SCHED-WORKERS: base={base:?}, wakeup={wakeup_envelope:?}, fanout={fanout_envelope:?}, actor={actor_unit:?}, combined={combined:?}, guaranteed={guaranteed:?}"
  );
}

#[test]
fn queue_length_covers_active_actor_capacity() {
  seeded_test_ext().execute_with(|| {
    let queue_cap = <Runtime as pallet_deos_actors::Config>::MaxQueueLength::get();
    let active_cap = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    assert!(
      queue_cap >= active_cap,
      "MaxQueueLength must be >= MaxActiveActors to avoid scheduler actor loss under full activation"
    );
  });
}

#[test]
fn close_actor_emits_owner_initiated_reason() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let fee_sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let fee_sink_before = native_balance(&fee_sink);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&fee_sink), fee_sink_before);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::OwnerInitiated,
        } if *id == actor_id
      )
    }));
  });
}

// --- Actors Platform: Amount Resolution ---

#[test]
fn percentage_of_last_funding_keeps_system_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    fund_native_via_call(ALICE, actor_id, 10_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    System::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    System::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 3);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(
      instance.lifecycle,
      pallet_deos_actors::ActiveLifecycle::Active
    );
    fund_native_via_call(CHARLIE, actor_id, 8_000_000_000_000);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&8_000_000_000_000)
    );
  });
}

#[test]
fn cycle_summary_reports_funding_unavailable_skip() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 0,
            committed_effectful_tasks: 0,
            skipped_conditions: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 1,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn percentage_of_last_funding_keeps_user_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    })])
    .expect("steps fits");
    let prefunded = user_prefunding_requirement(&steps);
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    deplete_user_sovereign(actor_id, prefunded);
    fund_native_via_call(ALICE, actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn swap_exact_in_zero_tolerance_matches_caller_aware_router_quote() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let amount_in = crate::EXISTENTIAL_DEPOSIT.saturating_mul(10);
    let quote = crate::DeosRouter::quote_exact_input(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount_in,
    )
    .expect("caller-aware route is quotable");
    let amount_out = crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount_in,
      Perbill::zero(),
    )
    .expect("zero-tolerance exact-input swap succeeds at its executable quote");
    assert_eq!(amount_out.recipient_amount_out, quote.amount_out);
  });
}

#[test]
fn exact_out_nonzero_tolerance_requires_capacity_for_adjusted_bound() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let required_in = crate::DeosRouter::quote_exact_out(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    let balance_before = native_balance(&ALICE);
    assert_eq!(
      crate::configs::actor_config::TmctolDexOps::swap_exact_out(
        ExecutionContext::new(&ALICE, ActorType::User),
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        target_out,
        required_in,
        Perbill::from_percent(1),
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("ExactOutInputCapacityExceeded",)
      ))
    );
    assert_eq!(native_balance(&ALICE), balance_before);
  });
}

#[test]
fn exact_out_execution_is_bounded_by_the_tolerance_cap_not_the_preservable_balance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let required_in = crate::DeosRouter::quote_exact_out(
      ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    // The tolerance-bound cap is required_in + ceil(1% * required_in). The supplied
    // preservable cap is larger than that, so the execution must be bounded by the
    // tolerance cap, not the preservable balance.
    let tolerance_cap = required_in + (required_in * 10_000_000 / 1_000_000_000) + 1;
    let preservable = tolerance_cap.saturating_mul(2);
    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_out(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
      preservable,
      Perbill::from_percent(1),
    ));
  });
}

#[test]
fn user_exact_out_zero_tolerance_preserves_floor_and_later_step_fees() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![
      make_step(Task::SwapOut {
        asset_out: AssetKind::Local(ASSET_A),
        amount_out: AmountResolution::Fixed(target_out),
        asset_in: AssetKind::Native,
        input_limit: InputLimit::Absolute(100_000_000_000_000),
        slippage_tolerance: Perbill::zero(),
      }),
      make_step(Task::Stake {
        asset: AssetKind::Local(999),
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      }),
    ])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let required_in = crate::DeosRouter::quote_exact_out(
      sovereign.clone(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      target_out,
    )
    .expect("native exact-output route is quotable")
    .amount_in;
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let fee_reserve =
      Actors::attempt_fee_envelope(instance.actor_class.actor_type(), &instance.steps, 0)
        .expect("admitted plan has a checked fee envelope")
        .total;
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    fund_native(
      actor_id,
      required_in
        .saturating_add(fee_reserve)
        .saturating_add(min_user_balance),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SwapExecuted { actor_id: id, amount_in, amount_out, .. }
          if *id == actor_id && *amount_in == required_in && *amount_out == target_out
      )
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 1, skipped_resolution: 1, failed_steps: 0, .. },
          ..
        } if *id == actor_id
      )
    }));
    assert!(native_balance(&sovereign) >= min_user_balance);
  });
}

#[test]
fn swap_out_rounding_boundary_uses_minimal_input_for_target_output() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let out_before = Assets::balance(ASSET_A, sovereign.clone());
    let effective_quote = |gross_in: u128| -> Option<u128> {
      if gross_in == 0 {
        return None;
      }
      let fee = if crate::DeosRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::DeosRouter::calculate_router_fee(gross_in)
      };
      let net_in = gross_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      crate::DeosRouter::quote_price(AssetKind::Native, AssetKind::Local(ASSET_A), net_in).ok()
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match effective_quote(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match effective_quote(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let expected_required_in = high;
    if expected_required_in > 1 {
      let prev_quote = effective_quote(expected_required_in.saturating_sub(1)).unwrap_or_default();
      assert!(
        prev_quote < target_out,
        "selected input must be minimal at rounding boundary"
      );
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let events = actor_events();
    let (amount_in, amount_out) = events
      .iter()
      .find_map(|event| match event {
        Event::SwapExecuted {
          actor_id: id,
          asset_in,
          asset_out,
          amount_in,
          amount_out,
          ..
        } if *id == actor_id
          && *asset_in == AssetKind::Native
          && *asset_out == AssetKind::Local(ASSET_A) =>
        {
          Some((*amount_in, *amount_out))
        }
        _ => None,
      })
      .unwrap_or_else(|| panic!("SwapExecuted must be emitted, events={events:?}"));
    assert_eq!(amount_out, target_out);
    assert_eq!(amount_in, expected_required_in);
    let out_after = Assets::balance(ASSET_A, sovereign.clone());
    assert!(out_after >= out_before.saturating_add(target_out));
  });
}

#[test]
fn swap_exact_out_liquidity_boundary_fails_without_partial_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let impossible_out = super::common::LIQUIDITY_AMOUNT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(impossible_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn swap_out_fails_when_required_input_exceeds_actor_balance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let steps = BoundedVec::try_from(vec![make_step(Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(100_000_000_000_000),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let quote_output = |amount_in: u128| -> Option<u128> {
      if amount_in == 0 {
        return None;
      }
      let fee = if crate::DeosRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::DeosRouter::calculate_router_fee(amount_in)
      };
      let net_in = amount_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      crate::DeosRouter::quote_price(AssetKind::Native, AssetKind::Local(ASSET_A), net_in).ok()
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match quote_output(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match quote_output(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let required_in = high;
    fund_native(actor_id, required_in.saturating_sub(1));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn dex_exact_out_adapter_retries_unfunded_input_with_explicit_error() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let unfunded = crate::AccountId::new([99u8; 32]);
    let result = <crate::configs::actor_config::TmctolDexOps as DexOps<
      crate::AccountId,
      AssetKind,
      u128,
    >>::swap_exact_out(
      ExecutionContext::new(&unfunded, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      crate::EXISTENTIAL_DEPOSIT,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(100),
      Perbill::zero(),
    );
    assert_eq!(
      result,
      Err(pallet_deos_actors::TaskFailure::temporary(
        pallet_deos_router::Error::<Runtime>::InsufficientInputBalance
      ))
    );
  });
}

#[test]
fn remove_liquidity_failure_classifier_is_explicit_and_typed() {
  use crate::pallet_asset_conversion::Error as AssetConversionError;
  use pallet_deos_actors::RetryClass;

  for error in [
    AssetConversionError::<Runtime>::AssetOneWithdrawalDidNotMeetMinimum,
    AssetConversionError::<Runtime>::AssetTwoWithdrawalDidNotMeetMinimum,
  ] {
    assert_eq!(
      classify_remove_liquidity_failure(error.into()).retry,
      RetryClass::Temporary
    );
  }
  for error in [
    AssetConversionError::<Runtime>::InvalidAssetPair,
    AssetConversionError::<Runtime>::PoolNotFound,
    AssetConversionError::<Runtime>::ZeroLiquidity,
  ] {
    assert_eq!(
      classify_remove_liquidity_failure(error.into()).retry,
      RetryClass::Permanent
    );
  }
}

#[test]
fn remove_liquidity_post_delta_guard_rejects_each_adversarial_mismatch() {
  assert_ok!(validate_remove_liquidity_output(10, 20, 10, 20));
  for result in [
    validate_remove_liquidity_output(9, 20, 10, 20),
    validate_remove_liquidity_output(10, 19, 10, 20),
  ] {
    assert_eq!(
      result,
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("MinimumLiquidityOutputNotMet")
      ))
    );
  }
}

#[test]
fn remove_liquidity_passes_each_minimum_to_asset_conversion_before_mutation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, AssetKind::Local(ASSET_A));
    let AssetKind::Local(lp_id) = lp_asset else {
      panic!("pool LP asset must be local");
    };
    let lp_before = Assets::balance(lp_id, &ALICE);
    let native_before = Balances::free_balance(&ALICE);
    let asset_before = Assets::balance(ASSET_A, &ALICE);
    let events_before = System::event_count();
    assert!(lp_before > 1);

    for (min_amount_a, min_amount_b) in [(u128::MAX, 1), (1, u128::MAX)] {
      let failure = TmctolLiquidityOps::remove_liquidity(
        &ALICE,
        lp_asset,
        pair.0,
        pair.1,
        lp_before / 2,
        min_amount_a,
        min_amount_b,
      )
      .expect_err("downstream authored minimum must reject before mutation");
      assert_eq!(failure.retry, RetryClass::Temporary);
      assert_eq!(Assets::balance(lp_id, &ALICE), lp_before);
      assert_eq!(Balances::free_balance(&ALICE), native_before);
      assert_eq!(Assets::balance(ASSET_A, &ALICE), asset_before);
      assert_eq!(System::event_count(), events_before);
    }
  });
}

#[test]
fn remove_liquidity_minimum_failure_preserves_each_error_policy_path() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, AssetKind::Local(ASSET_A));
    let AssetKind::Local(lp_id) = lp_asset else {
      panic!("pool LP asset must be local");
    };
    let lp_amount = Assets::minimum_balance(lp_id).max(10);

    for policy in [
      StepErrorPolicy::AbortCycle,
      StepErrorPolicy::ContinueNextStep,
      StepErrorPolicy::RetryLater { max_attempts: 3 },
    ] {
      let plan = alloc::vec![
        pallet_deos_actors::Step {
          preconditions: pallet_deos_actors::Preconditions::Unconditional,
          task: Task::RemoveLiquidity {
            lp_asset,
            asset_a: pair.0,
            asset_b: pair.1,
            lp_amount: AmountResolution::Fixed(lp_amount),
            min_amount_a: Balance::MAX,
            min_amount_b: Balance::MAX,
          },
          on_error: policy,
        },
        pallet_deos_actors::Step {
          preconditions: pallet_deos_actors::Preconditions::Unconditional,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        },
      ]
      .try_into()
      .expect("two-step plan fits");
      let actor_id = create_system(ALICE, manual_schedule(), None, plan);
      let actor = actor_account(actor_id);
      fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(2));
      assert_ok!(<Assets as FungiblesMutate<AccountId>>::mint_into(
        lp_id,
        &actor,
        lp_amount.saturating_mul(2)
      ));
      let lp_before = Assets::balance(lp_id, &actor);
      let native_before = Balances::free_balance(&actor);
      System::reset_events();

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);

      assert_eq!(Assets::balance(lp_id, &actor), lp_before);
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::LiquidityRemoved { actor_id: id, .. } if *id == actor_id
      )));
      assert!(
        has_actor_event(|event| matches!(
          event,
          Event::StepFailed { actor_id: id, .. } if *id == actor_id
        )),
        "minimum failure must emit StepFailed: {:?}",
        System::events()
      );
      match policy {
        StepErrorPolicy::AbortCycle => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(!has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, .. } if *id == actor_id
          )));
          assert!(Actors::continuation_state(actor_id).is_none());
        }
        StepErrorPolicy::ContinueNextStep => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, step_index: 1, .. } if *id == actor_id
          )));
          assert!(Actors::continuation_state(actor_id).is_none());
        }
        StepErrorPolicy::RetryLater { .. } => {
          assert_eq!(Balances::free_balance(&actor), native_before);
          assert!(!has_actor_event(|event| matches!(
            event,
            Event::CycleStopped { actor_id: id, .. } if *id == actor_id
          )));
          let continuation =
            Actors::continuation_state(actor_id).expect("temporary minimum failure suspends");
          assert_eq!(continuation.cursor, 0);
          assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);
        }
      }
    }
  });
}

#[test]
fn router_failure_classifier_is_exhaustive_and_typed() {
  use pallet_deos_actors::RetryClass;
  use pallet_deos_router::Error as RouterError;

  for error in [
    RouterError::<Runtime>::SlippageExceeded,
    RouterError::<Runtime>::PriceDeviationExceeded,
    RouterError::<Runtime>::NoRouteFound,
    RouterError::<Runtime>::InsufficientLiquidity,
    RouterError::<Runtime>::InvalidOracleData,
    RouterError::<Runtime>::NoMultiHopRoute,
    RouterError::<Runtime>::InsufficientInputBalance,
  ] {
    assert_eq!(classify_router_failure(error).retry, RetryClass::Temporary);
  }

  let temporary_adapter =
    pallet_deos_router::ExecutionError::<Runtime>::from(pallet_deos_router::AdapterFailure::new(
      DispatchError::Other("PublicationCapacity"),
      pallet_deos_router::RouterFailureClass::PublicationRejected,
      pallet_deos_router::RetryDisposition::RetryLater,
    ));
  assert_eq!(
    classify_router_execution_failure(temporary_adapter).retry,
    RetryClass::Temporary,
  );
  let unknown_adapter = pallet_deos_router::ExecutionError::<Runtime>::from(
    pallet_deos_router::AdapterFailure::unknown(DispatchError::Other("UnknownAdapterFailure")),
  );
  assert_eq!(
    classify_router_execution_failure(unknown_adapter).retry,
    RetryClass::Permanent,
  );

  for error in [
    RouterError::<Runtime>::IdenticalAssets,
    RouterError::<Runtime>::ZeroAmount,
    RouterError::<Runtime>::AmountTooLow,
    RouterError::<Runtime>::DeadlinePassed,
    RouterError::<Runtime>::FeeRoutingFailed,
    RouterError::<Runtime>::RouterFeeTooHigh,
    RouterError::<Runtime>::LpTokenPairCollision,
    RouterError::<Runtime>::LpPairCapacityExceeded,
    RouterError::<Runtime>::InvalidPoolPair,
    RouterError::<Runtime>::PreparedRouteMismatch,
  ] {
    assert_eq!(classify_router_failure(error).retry, RetryClass::Permanent);
  }
}

#[test]
fn market_execution_classifier_uses_the_concrete_cause() {
  use pallet_deos_actors::RetryClass as ActorRetryClass;
  use pallet_deos_router::{RetryDisposition as RouterRetryClass, RouterFailureClass};

  let recoverable = market_execution_failure(
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::PoolEmpty.into(),
  );
  assert_eq!(
    recoverable.failure_class(),
    RouterFailureClass::LiquidityUnavailable
  );
  assert_eq!(
    recoverable.retry_disposition(),
    RouterRetryClass::RetryLater
  );
  assert_eq!(
    classify_router_execution_failure(recoverable.into()).retry,
    ActorRetryClass::Temporary
  );

  let permanent = market_execution_failure(
    polkadot_sdk::pallet_asset_conversion::Error::<Runtime>::InvalidPath.into(),
  );
  assert_eq!(
    permanent.failure_class(),
    RouterFailureClass::InvariantViolation
  );
  assert_eq!(permanent.retry_disposition(), RouterRetryClass::Permanent);
  assert_eq!(
    classify_router_execution_failure(permanent.into()).retry,
    ActorRetryClass::Permanent
  );
}

#[test]
fn system_actor_preserves_task_local_swap_amounts_without_fifo_priority() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let amount = 200 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    let quote = crate::DeosRouter::quote_exact_input(
      actor.clone(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
    )
    .expect("large System quote exists");
    let reference = quote
      .amount_out
      .saturating_mul(PRECISION)
      .saturating_div(quote.amount_after_fee);
    publish_deos_router_observation(AssetKind::Native, AssetKind::Local(ASSET_A), reference);
    let before = native_balance(&actor);
    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&actor, ActorType::System),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
      Perbill::one(),
    ));
    assert_eq!(before.saturating_sub(native_balance(&actor)), amount);
  });
}

#[test]
fn typed_system_swap_uses_stricter_reference_deviation_than_user_swap() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let amount = 10 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      PRECISION.saturating_mul(110).saturating_div(100),
    );
    let actor_before = native_balance(&actor);
    assert_eq!(
      crate::configs::actor_config::TmctolDexOps::swap_exact_in(
        ExecutionContext::new(&actor, ActorType::System),
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        amount,
        Perbill::one(),
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
    assert_eq!(native_balance(&actor), actor_before);

    assert_ok!(crate::configs::actor_config::TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount,
      Perbill::one(),
    ));
  });
}

#[test]
fn missing_or_uninitialized_pool_feed_does_not_block_a_valid_user_swap() {
  use primitives::ecosystem::params::PRECISION;

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(ASSET_A);
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    assert_eq!(
      Oracle::observation_state(feed, 1).expect("maximum age is valid"),
      pallet_oracle::ObservationState::Uninitialized
    );
    assert_ok!(TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      asset_in,
      asset_out,
      10 * PRECISION,
      Perbill::one(),
    ));

    pallet_oracle::Feeds::<Runtime>::remove(feed);
    assert_eq!(
      Oracle::observation_state(feed, 1).expect("maximum age is valid"),
      pallet_oracle::ObservationState::Unavailable
    );
    assert_ok!(TmctolDexOps::swap_exact_in(
      ExecutionContext::new(&ALICE, ActorType::User),
      asset_in,
      asset_out,
      10 * PRECISION,
      Perbill::one(),
    ));
  });
}

fn publish_deos_router_observation(asset_in: AssetKind, asset_out: AssetKind, value: Balance) {
  crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)
    .expect("test pair feed admission succeeds");
  Oracle::publish(
    RuntimeOrigin::signed(deos_router_account()),
    crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out),
    value,
  )
  .expect("DEOS Router producer publishes the observation");
}

#[test]
fn system_reference_guard_enforces_freshness_boundary_and_reserve_fallback() {
  seeded_test_ext().execute_with(|| {
    use crate::configs::actor_config::ActorMaxSystemReferenceAgeBlocks;
    use primitives::ecosystem::params::PRECISION;

    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(999_999);
    let max_age = ActorMaxSystemReferenceAgeBlocks::get();
    System::set_block_number(1);
    publish_deos_router_observation(asset_in, asset_out, PRECISION);
    System::set_block_number(max_age.saturating_add(1));
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      asset_out,
      PRECISION,
      PRECISION,
    ));

    System::set_block_number(max_age.saturating_add(2));
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        PRECISION,
        PRECISION,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemReferencePriceUnavailable")
      ))
    );

    let uninitialized_out = AssetKind::Local(999_998);
    assert_ok!(
      crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, uninitialized_out,)
    );
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        uninitialized_out,
        PRECISION,
        PRECISION,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemReferencePriceUnavailable")
      ))
    );

    assert_ok!(super::common::setup_deos_router_infrastructure());
    let pooled_out = AssetKind::Local(ASSET_A);
    publish_deos_router_observation(asset_in, pooled_out, PRECISION.saturating_mul(10));
    System::set_block_number(
      System::block_number()
        .saturating_add(max_age)
        .saturating_add(1),
    );
    let (reserve_in, reserve_out) =
      crate::AssetConversion::get_reserves(asset_in, pooled_out).expect("pool reserves exist");
    let reserve_reference = reserve_out.saturating_mul(PRECISION) / reserve_in;
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      pooled_out,
      PRECISION,
      reserve_reference,
    ));
  });
}

#[test]
fn checked_reference_guard_is_exact_at_the_deviation_boundary_and_rejects_above() {
  seeded_test_ext().execute_with(|| {
    use primitives::ecosystem::params::PRECISION;
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(999_997);
    System::set_block_number(1);
    // Reference price 1.0 (scaled PRECISION).
    publish_deos_router_observation(asset_in, asset_out, PRECISION);
    let max_dev = crate::configs::actor_config::ActorMaxSystemPriceDeviation::get().deconstruct();
    // Exactly at the deviation limit: |exec_out * ref_in - ref_out * exec_in| * ACCURACY
    // == max_dev * ref_out * exec_in passes; one part above fails. With ref price 1.0
    // and exec_in = PRECISION, the exact margin is max_dev * PRECISION / ACCURACY.
    let margin = (max_dev as u128).saturating_mul(PRECISION) / 1_000_000_000u128;
    let exec_in = PRECISION;
    let exec_out = PRECISION.saturating_add(margin);
    assert_ok!(TmctolDexOps::ensure_system_reference_price(
      &ExecutionContext::new(&ALICE, ActorType::System),
      asset_in,
      asset_out,
      exec_in,
      exec_out,
    ));
    let exec_out_above = PRECISION.saturating_add(margin).saturating_add(1);
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        exec_in,
        exec_out_above,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
    // Orientation reversal: a swapped quote (exec_out below reference by the same
    // margin) is rejected symmetrically by the absolute-value cross-multiplication.
    let exec_out_low = PRECISION.saturating_sub(margin).saturating_sub(1);
    assert_eq!(
      TmctolDexOps::ensure_system_reference_price(
        &ExecutionContext::new(&ALICE, ActorType::System),
        asset_in,
        asset_out,
        exec_in,
        exec_out_low,
      ),
      Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("SystemPriceDeviationExceeded")
      ))
    );
  });
}

#[test]
fn excessive_system_reference_deviation_suspends_without_fill_and_backs_off() {
  use primitives::ecosystem::{actor_ids, params::PRECISION};

  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let actor_id = actor_ids::TREASURY_B_ACTORS_ID;
    let actor = Actors::sovereign_account_id_system(actor_id);
    let amount = 10 * PRECISION;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&actor, amount.saturating_mul(2));
    let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
      preconditions: pallet_deos_actors::Preconditions::Unconditional,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(ASSET_A),
        amount_in: AmountResolution::Fixed(amount),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    }])
    .expect("single-step deviation retry plan fits");
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_id,
      ContractInput::Active(ActiveContractInput {
        schedule: Schedule {
          trigger: Trigger::immediate_manual(),
          cooldown_blocks: 0,
        },
        schedule_window: None,
        steps: plan,
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: FundingSourcePolicy::RuntimePolicy,
        auto_close_at_cycle_nonce: None,
      }),
    ));
    publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      PRECISION.saturating_mul(110).saturating_div(100),
    );
    let before = native_balance(&actor);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    let continuation = Actors::continuation_state(actor_id).expect("deviation suspends");
    assert_eq!(continuation.attempt, 0);
    assert_eq!(continuation.cursor, 0);
    assert_eq!(native_balance(&actor), before);
    let first_retry = Actors::actor_hot(actor_id).expect("actor stays hot");
    assert!(first_retry.queue_ticket.is_some());
    assert!(first_retry.wakeup_pointer.is_none());

    System::set_block_number(2);
    run_idle(Weight::MAX);
    let continuation = Actors::continuation_state(actor_id).expect("deviation resuspends");
    assert_eq!(continuation.attempt, 1);
    assert_eq!(continuation.cursor, 0);
    assert_eq!(native_balance(&actor), before);
    let second_retry = Actors::actor_hot(actor_id).expect("actor stays hot");
    assert!(second_retry.queue_ticket.is_none());
    assert_eq!(
      second_retry.wakeup_pointer.map(|pointer| pointer.block),
      Some(4)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        reason: pallet_deos_actors::SuspensionReason::Temporary,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_market_failure_opens_the_single_retry_continuation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
      preconditions: pallet_deos_actors::Preconditions::Unconditional,
      task: Task::SwapOut {
        asset_out: AssetKind::Local(ASSET_A),
        amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
        asset_in: AssetKind::Native,
        input_limit: InputLimit::Absolute(1),
        slippage_tolerance: Perbill::zero(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    }])
    .expect("single-step retry plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let continuation = Actors::continuation_state(actor_id).expect("Temporary failure suspends");
    assert_eq!(continuation.cursor, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        cursor: 0,
        reason: pallet_deos_actors::SuspensionReason::Temporary,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_oracle_capacity_failure_rolls_back_economics_and_has_one_retry_owner() {
  use primitives::ecosystem::params::PRECISION;

  for exact_output in [false, true] {
    seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(super::common::setup_deos_router_infrastructure());
      for block in 2..=20 {
        System::set_block_number(block);
        Actors::on_initialize(block);
        run_idle(Weight::MAX);
      }
      let asset_in = AssetKind::Native;
      let asset_out = AssetKind::Local(ASSET_A);
      let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
      crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out)
        .expect("directional pool feeds fit");
      create_system(
        ALICE,
        observation_schedule(feed),
        None,
        BoundedVec::try_from(vec![make_step(inert_task())]).expect("one inert step fits"),
      );
      let task = if exact_output {
        Task::SwapOut {
          asset_out,
          amount_out: AmountResolution::Fixed(PRECISION),
          asset_in,
          input_limit: InputLimit::Absolute(100 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      } else {
        Task::SwapIn {
          asset_in,
          asset_out,
          amount_in: AmountResolution::Fixed(10 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      };
      let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
        preconditions: pallet_deos_actors::Preconditions::Unconditional,
        task,
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      }])
      .expect("single-step publication retry plan fits");
      let actor_id = create_user(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 1_000 * PRECISION);
      let actor = actor_account(actor_id);
      let input_before = native_balance(&actor);
      let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
      let burn_actor = super::common::burn_actor_account();
      let router_fee_before = native_balance(&burn_actor);
      let burn_cycle_before = Actors::active_actor_view(burn_actor_id)
        .expect("Burn Actor exists")
        .cycle_nonce;
      let output_before = Assets::balance(ASSET_A, &actor);
      let pool_before =
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool exists");
      let reward_liability_before = Staking::native_security_reward_liability();
      let reward_account = Staking::native_security_reward_account();
      let reward_custody_before = native_balance(&reward_account);
      let dirty_capacity = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get()
        .saturating_mul(<Runtime as pallet_deos_actors::Config>::MaxTriggerSources::get());
      pallet_deos_actors::DirtyObservationListState::<Runtime>::mutate(|list| {
        list.count = dirty_capacity;
      });

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);

      let input_after_failure = native_balance(&actor);
      assert!(input_after_failure < input_before);
      assert_eq!(native_balance(&burn_actor), router_fee_before);
      assert_eq!(
        Actors::active_actor_view(burn_actor_id)
          .expect("Burn Actor remains active")
          .cycle_nonce,
        burn_cycle_before,
      );
      assert_eq!(Assets::balance(ASSET_A, &actor), output_before);
      assert_eq!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert!(Oracle::observations(feed).is_none());
      assert!(Actors::dirty_observation_feeds(feed).is_none());
      assert_eq!(Actors::dirty_observation_feed_count(), dirty_capacity);
      assert_eq!(
        Staking::native_security_reward_liability(),
        reward_liability_before
      );
      assert_eq!(native_balance(&reward_account), reward_custody_before);
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(|event| matches!(
            event,
            Event::CycleSuspended {
              actor_id: id,
              reason: pallet_deos_actors::SuspensionReason::Temporary,
              ..
            } if *id == actor_id
          ))
          .count(),
        1,
      );
      let continuation = Actors::continuation_state(actor_id).expect("publication retry suspends");
      assert_eq!(continuation.cursor, 0);
      let hot = Actors::actor_hot(actor_id).expect("suspended Actor stays hot");
      assert!(hot.queue_ticket.is_some());
      assert!(hot.wakeup_pointer.is_none());

      pallet_deos_actors::DirtyObservationListState::<Runtime>::kill();
      System::set_block_number(21);
      run_idle(Weight::MAX);

      assert!(Actors::continuation_state(actor_id).is_none());
      assert!(native_balance(&actor) < input_after_failure);
      assert!(Assets::balance(ASSET_A, &actor) > output_before);
      assert_ne!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert_eq!(
        Oracle::observations(feed)
          .expect("retry publishes")
          .revision,
        1
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        1
      );

      System::set_block_number(22);
      run_idle(Weight::MAX);
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        1
      );
    });
  }
}

#[test]
fn permanent_publication_invariant_terminates_without_cross_system_mutation_or_retry() {
  use primitives::ecosystem::params::PRECISION;

  for exact_output in [false, true] {
    seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(super::common::setup_deos_router_infrastructure());
      for block in 2..=20 {
        System::set_block_number(block);
        Actors::on_initialize(block);
        run_idle(Weight::MAX);
      }
      let asset_in = AssetKind::Native;
      let asset_out = AssetKind::Local(ASSET_A);
      let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
      pallet_oracle::Feeds::<Runtime>::mutate(feed, |maybe| {
        maybe.as_mut().expect("pool feed is registered").producer = ALICE;
      });
      let task = if exact_output {
        Task::SwapOut {
          asset_out,
          amount_out: AmountResolution::Fixed(PRECISION),
          asset_in,
          input_limit: InputLimit::Absolute(100 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      } else {
        Task::SwapIn {
          asset_in,
          asset_out,
          amount_in: AmountResolution::Fixed(10 * PRECISION),
          slippage_tolerance: Perbill::zero(),
        }
      };
      let plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
        preconditions: pallet_deos_actors::Preconditions::Unconditional,
        task,
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      }])
      .expect("single-step permanent publication plan fits");
      let actor_id = create_user(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 1_000 * PRECISION);
      let actor = actor_account(actor_id);
      let actor_input_before = native_balance(&actor);
      let actor_contract_before = Actors::actor_contract(actor_id).expect("Actor Contract exists");
      let output_before = Assets::balance(ASSET_A, &actor);
      let pool_before =
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool exists");
      let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
      let burn_actor = super::common::burn_actor_account();
      let burn_balance_before = native_balance(&burn_actor);
      let burn_cycle_before = Actors::active_actor_view(burn_actor_id)
        .expect("Burn Actor exists")
        .cycle_nonce;
      let reward_liability_before = Staking::native_security_reward_liability();
      let reward_account = Staking::native_security_reward_account();
      let reward_custody_before = native_balance(&reward_account);
      let staking_participants_before = Staking::native_security_participants();
      let governance_coefficient_before = Staking::governance_participation_coefficient(0, &actor);

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);

      assert!(
        native_balance(&actor) < actor_input_before,
        "the bounded attempt remains paid"
      );
      assert_eq!(
        Actors::actor_contract(actor_id).expect("Actor Contract remains"),
        actor_contract_before,
      );
      assert!(Actors::continuation_state(actor_id).is_none());
      let hot = Actors::actor_hot(actor_id).expect("Actor hot state remains");
      assert!(hot.queue_ticket.is_none());
      assert!(hot.wakeup_pointer.is_none());
      assert_eq!(Assets::balance(ASSET_A, &actor), output_before);
      assert_eq!(
        crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains"),
        pool_before
      );
      assert_eq!(native_balance(&burn_actor), burn_balance_before);
      assert_eq!(
        Actors::active_actor_view(burn_actor_id)
          .expect("Burn Actor remains active")
          .cycle_nonce,
        burn_cycle_before,
      );
      assert!(Oracle::observations(feed).is_none());
      assert!(Actors::dirty_observation_feeds(feed).is_none());
      assert_eq!(
        Staking::native_security_reward_liability(),
        reward_liability_before
      );
      assert_eq!(native_balance(&reward_account), reward_custody_before);
      assert_eq!(
        Staking::native_security_participants(),
        staking_participants_before
      );
      assert_eq!(
        Staking::governance_participation_coefficient(0, &actor),
        governance_coefficient_before,
      );
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );

      System::set_block_number(21);
      run_idle(Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
      assert_eq!(
        actor_events()
          .iter()
          .filter(
            |event| matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
          )
          .count(),
        0
      );

      if !exact_output {
        assert_noop!(
          crate::DeosRouter::swap(
            RuntimeOrigin::signed(ALICE),
            asset_in,
            asset_out,
            10 * PRECISION,
            0,
            BOB,
            u32::MAX,
          ),
          pallet_deos_router::Error::<Runtime>::InvalidOracleData
        );
      }
    });
  }
}

#[test]
fn staking_adapter_supports_liquid_native_stake_without_operator_context() {
  seeded_test_ext().execute_with(|| {
    let who = crate::AccountId::new([77u8; 32]);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &who,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10)
    ));
    let result =
      <crate::configs::actor_config::TmctolStakingOps as pallet_deos_actors::adapters::StakingOps<
        crate::AccountId,
        AssetKind,
        u128,
      >>::stake(&who, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT);
    assert_ok!(result);
    assert!(crate::Staking::live_native_staked_receipt_balance(&who).unwrap_or_default() > 0);
  });
}

#[test]
fn actor_unstake_percentage_current_resolves_live_staking_shares() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let steps = BoundedVec::try_from(vec![make_step(Task::Unstake {
      asset: AssetKind::Native,
      shares: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let actor_id = create_user(BOB, manual_schedule(), None, steps);
    let actor = actor_account(actor_id);
    let stake_amount = crate::EXISTENTIAL_DEPOSIT.saturating_mul(10);
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &actor,
      stake_amount.saturating_add(crate::EXISTENTIAL_DEPOSIT),
    ));
    assert_ok!(crate::configs::actor_config::TmctolStakingOps::stake(
      &actor,
      AssetKind::Native,
      stake_amount,
    ));
    fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(10));
    let shares_before =
      crate::configs::actor_config::TmctolStakingOps::share_balance(&actor, AssetKind::Native);
    assert!(shares_before > 0);
    assert_eq!(
      crate::configs::actor_config::TmctolStakingOps::share_asset(AssetKind::Native),
      crate::Staking::staked_asset_id_for_queries(0).map(AssetKind::Local)
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      crate::configs::actor_config::TmctolStakingOps::share_balance(&actor, AssetKind::Native),
      shares_before.saturating_sub(Perbill::from_percent(50).mul_floor(shares_before))
    );
  });
}

#[test]
fn actor_native_stake_task_mints_liquid_stntve_without_binding() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    crate::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let steps = BoundedVec::try_from(vec![make_step(Task::Stake {
      asset: AssetKind::Local(0),
      amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
    })])
    .expect("steps fits");
    let actor_id = create_user(BOB, manual_schedule(), None, steps);
    let actor_acc = actor_account(actor_id);
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &actor_acc,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10),
    ));
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), actor_id));
    run_idle(Weight::MAX);
    assert!(
      crate::Staking::live_native_staked_receipt_balance(&actor_acc).unwrap_or_default() > 0,
      "Actors sovereign must receive stNTVE after native stake"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StakeExecuted {
          actor_id: id,
          asset: AssetKind::Local(0),
          amount,
          ..
        } if *id == actor_id && *amount == crate::EXISTENTIAL_DEPOSIT
      )
    }));
  });
}

// --- Actors Platform: SplitTransfer ---

#[test]
fn split_transfer_uses_perbill_and_keeps_remainder_on_actor() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let total = 101u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let steps = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    })])
    .expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    let actor_acc = actor_account(actor_id);
    let actor_before = native_balance(&actor_acc);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    let spent = actor_before.saturating_sub(native_balance(&actor_acc));
    assert!(
      spent >= 100,
      "Actors must spend at least distributed amount"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          actor_id: id,
          total: amount,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == actor_id
          && *amount == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn native_preflight_requires_a_free_ed_anchor_for_sub_ed_ingress() {
  seeded_test_ext().execute_with(|| {
    let provider_only = AccountId::new([70u8; 32]);
    let reserved_anchor = AccountId::new([71u8; 32]);
    let free_anchor = AccountId::new([72u8; 32]);
    let existential_deposit = crate::EXISTENTIAL_DEPOSIT;
    let amount = existential_deposit / 2;
    let unavailable = || {
      Err(pallet_deos_actors::TaskFailure::temporary(
        Error::<Runtime>::RecipientDepositUnavailable,
      ))
    };

    System::inc_providers(&provider_only);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &provider_only, AssetKind::Native, amount,),
      unavailable()
    );

    System::inc_providers(&reserved_anchor);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &reserved_anchor,
      existential_deposit,
      ExistenceRequirement::AllowDeath,
    ));
    assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(
      &reserved_anchor,
      existential_deposit,
    ));
    assert_eq!(Balances::free_balance(&reserved_anchor), 0);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &reserved_anchor, AssetKind::Native, amount,),
      unavailable()
    );

    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &free_anchor,
      existential_deposit,
      ExistenceRequirement::AllowDeath,
    ));
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE,
      &free_anchor,
      AssetKind::Native,
      amount,
    ));
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &free_anchor,
      AssetKind::Native,
      amount,
    ));
    assert_eq!(
      Balances::free_balance(&free_anchor),
      existential_deposit.saturating_add(amount)
    );
  });
}

#[test]
fn anchored_split_transfer_rolls_back_when_a_later_recipient_is_unavailable() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let anchored = AccountId::new([73u8; 32]);
    let provider_only = AccountId::new([74u8; 32]);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &anchored,
      crate::EXISTENTIAL_DEPOSIT,
      ExistenceRequirement::AllowDeath,
    ));
    System::inc_providers(&provider_only);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: anchored.clone(),
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: provider_only.clone(),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("two split legs fit");
    let plan = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(2),
      legs,
    })])
    .expect("split plan fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, crate::EXISTENTIAL_DEPOSIT.saturating_mul(10));
    let anchored_before = native_balance(&anchored);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&anchored), anchored_before);
    assert_eq!(native_balance(&provider_only), 0);
  });
}

#[test]
fn foreign_asset_preflight_enforces_exact_minimum_boundary() {
  seeded_test_ext().execute_with(|| {
    const ASSET_ID: u32 = 77_707;
    let below = AccountId::new([71u8; 32]);
    let equal = AccountId::new([72u8; 32]);
    let above = AccountId::new([73u8; 32]);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      ASSET_ID,
      ALICE.clone().into(),
      true,
      100,
    ));
    assert_ok!(mint_tokens(ASSET_ID, &ALICE, &ALICE, 1_000));
    let asset = AssetKind::Foreign(ASSET_ID);
    assert_eq!(
      TmctolAssetOps::preflight_transfer(&ALICE, &below, asset, 99),
      Err(pallet_deos_actors::TaskFailure::temporary(
        Error::<Runtime>::RecipientDepositUnavailable,
      ))
    );
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE, &equal, asset, 100
    ));
    assert_ok!(TmctolAssetOps::transfer(&ALICE, &equal, asset, 100));
    assert_ok!(TmctolAssetOps::preflight_transfer(
      &ALICE, &above, asset, 101
    ));
    assert_ok!(TmctolAssetOps::transfer(&ALICE, &above, asset, 101));
    assert_eq!(
      <Assets as FungiblesInspect<AccountId>>::balance(ASSET_ID, &equal),
      100
    );
    assert_eq!(
      <Assets as FungiblesInspect<AccountId>>::balance(ASSET_ID, &above),
      101
    );
  });
}

#[test]
fn split_transfer_rejects_ed_ineligible_recipient_then_retries_atomically() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let unknown = crate::AccountId::new([9u8; 32]);
    let total = 100u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: unknown.clone(),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let mut step = make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let steps = BoundedVec::try_from(vec![step]).expect("steps fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100_000_000_000_000);
    let actor = actor_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&unknown), 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, error, .. }
        if *id == actor_id
          && *error == Error::<Runtime>::RecipientDepositUnavailable.into()
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id
    )));
    let continuation = Actors::continuation_state(actor_id).expect("temporary rejection suspends");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);

    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&unknown, crate::EXISTENTIAL_DEPOSIT);
    let unknown_before = native_balance(&unknown);
    System::set_block_number(2);
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), actor_before - total);
    assert_eq!(native_balance(&BOB), bob_before + 50);
    assert_eq!(native_balance(&unknown), unknown_before + 50);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id: id,
        total: 100,
        distributed: 100,
        retained: 0,
        legs: 2,
        effective_legs: 2,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn create_rejects_split_transfer_share_sum_above_one() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(60),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let steps = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    })])
    .expect("steps fits");
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, steps),
      ),
      Error::<Runtime>::InvalidSplitTransfer
    );
  });
}

// --- Actors Platform: Bounds & Validation ---

#[test]
fn split_transfer_leg_count_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_legs =
      <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get() as usize;
    let within_limit = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let above_limit = (0..max_legs.saturating_add(1))
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    assert!(SplitTransferLegsOf::<Runtime>::try_from(within_limit).is_ok());
    assert!(SplitTransferLegsOf::<Runtime>::try_from(above_limit).is_err());
  });
}

#[test]
fn whitelist_size_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_whitelist =
      <<Runtime as pallet_deos_actors::Config>::MaxWhitelistSize as Get<u32>>::get() as usize;
    let within_limit = (0..max_whitelist)
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    let above_limit = (0..max_whitelist.saturating_add(1))
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize>::try_from(
        within_limit
      )
      .is_ok()
    );
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize>::try_from(
        above_limit
      )
      .is_err()
    );
  });
}

#[test]
fn ten_julian_year_horizon_matches_six_second_runtime_binding() {
  const JULIAN_YEAR_MILLIS: u64 = 36525 * 24 * 60 * 60 * 10;
  const TEN_JULIAN_YEARS_MILLIS: u64 = JULIAN_YEAR_MILLIS * 10;
  let derived = TEN_JULIAN_YEARS_MILLIS.div_ceil(crate::SLOT_DURATION);
  assert_eq!(crate::SLOT_DURATION, 6_000);
  assert_eq!(derived, 52_596_000);
  assert_eq!(
    u64::from(crate::configs::actor_config::ActorMaxExecutionDelayBlocks::get()),
    derived
  );
}

#[test]
fn timer_horizon_validation_includes_runtime_jitter_bound() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_delay: u32 = <Runtime as pallet_deos_actors::Config>::MaxExecutionDelayBlocks::get();
    let max_jitter =
      <<Runtime as pallet_deos_actors::Config>::MaxTimerJitterBlocks as Get<u32>>::get()
        .saturating_sub(1);
    let largest_valid_cadence = max_delay.saturating_sub(max_jitter);
    let schedule = |every_blocks| Schedule {
      trigger: Trigger::cadenced_always(every_blocks),
      cooldown_blocks: 0,
    };
    let valid_plan = transfer_execution_plan(BOB, AssetKind::Native, 1);
    prefund_active_user_creation(&ALICE, &valid_plan);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(schedule(largest_valid_cadence), None, valid_plan),
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          schedule(largest_valid_cadence.saturating_add(1)),
          None,
          transfer_execution_plan(BOB, AssetKind::Native, 1),
        ),
      ),
      Error::<Runtime>::ExecutionDelayTooLong
    );
  });
}

// --- Actors Platform: Trigger & Source Filter ---

#[test]
fn on_address_event_owner_only_respects_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &BOB
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_asset_filter_is_enforced() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let asset_whitelist = BoundedVec::try_from(vec![AssetKind::Local(ASSET_A)]).expect("fits");
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(asset_whitelist)),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Local(ASSET_A),
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_without_source_is_ignored_for_filtered_trigger() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      AssetKind::Native,
      100
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn internal_asset_transfer_rolls_back_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let alice_before = native_balance(&ALICE);
    let sovereign_before = native_balance(&sovereign);
    assert_eq!(
      <TmctolAssetOps as AssetOps<AccountId, AssetKind, Balance>>::transfer(
        &ALICE,
        &sovereign,
        AssetKind::Native,
        1,
      ),
      Err(pallet_deos_actors::TaskFailure::permanent(
        Error::<Runtime>::FundingAccumulatorOverflow,
      ))
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn asset_ops_transfer_notifies_on_address_event_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_amount = 1_000u128;
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, receiver_amount),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_execution_plan(receiver_sovereign, AssetKind::Native, 5_000),
    );
    let sender_sovereign = actor_account(sender_id);
    let sender_whitelist = BoundedVec::try_from(vec![sender_sovereign]).expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      receiver_id,
      on_address_event_schedule(SourceFilter::Whitelist(sender_whitelist), AssetFilter::Any),
      None,
    ));
    fund_native(sender_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(receiver_id)
        .expect("receiver exists")
        .cycle_nonce,
      0
    );
    assert!(
      Actors::actor_hot(receiver_id)
        .expect("receiver hot state")
        .queue_ticket
        .is_some(),
      "an address event created during on_idle must survive as next-block work"
    );
    assert!(Actors::pending_signal(receiver_id));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before.saturating_add(receiver_amount)
    );
  });
}

#[test]
fn repeated_same_block_transfers_coalesce_to_one_actor_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    fund_native_via_call(ALICE, actor_id, 100_000_000_000_000);
    fund_native_via_call(ALICE, actor_id, 50_000_000_000_000);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor exists")
        .cycle_nonce,
      1,
      "multiple same-block funding events must coalesce into one execution"
    );
  });
}

#[test]
fn split_transfer_legs_to_actor_sovereigns_notify_through_certified_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_a = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_b = create_user(
      BOB,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_a_sovereign = actor_account(receiver_a);
    let receiver_b_sovereign = actor_account(receiver_b);
    let legs = SplitTransferLegsOf::<Runtime>::try_from(vec![
      SplitLeg {
        to: receiver_a_sovereign.clone(),
        share: Perbill::from_percent(40),
      },
      SplitLeg {
        to: receiver_b_sovereign,
        share: Perbill::from_percent(40),
      },
    ])
    .expect("two legs fit");
    let sender = create_system(
      CHARLIE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(10_000),
        legs,
      })])
      .expect("plan fits"),
    );
    fund_native(sender, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), sender));
    run_idle(Weight::MAX);
    assert!(
      Actors::pending_signal(receiver_a),
      "first SplitTransfer leg to an Actors sovereign must latch readiness"
    );
    assert!(
      Actors::pending_signal(receiver_b),
      "second SplitTransfer leg to an Actors sovereign must latch readiness"
    );
  });
}

#[test]
fn mint_to_actor_sovereign_notifies_source_less_certified_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("plan fits"),
    );
    let receiver_sovereign = actor_account(receiver);
    let before = native_balance(&receiver_sovereign);
    // A certified Mint to an Actors sovereign destination (the Actors Mint task calls
    // the same adapter) must create the value and notify source-less ingress.
    assert_ok!(TmctolAssetOps::mint(
      &receiver_sovereign,
      AssetKind::Native,
      10_000,
    ));
    assert_eq!(
      native_balance(&receiver_sovereign),
      before.saturating_add(10_000),
      "mint creates the value movement"
    );
    assert!(
      Actors::pending_signal(receiver),
      "Mint to an Actors sovereign must notify source-less certified ingress"
    );
  });
}

#[test]
fn manual_signal_then_same_block_funding_coalesces_to_one_actor_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    fund_native(actor_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    fund_native_via_call(ALICE, actor_id, 50_000_000_000_000);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor exists")
        .cycle_nonce,
      1,
      "manual and funding readiness must share one live queue membership"
    );
  });
}

#[test]
fn runtime_rejects_self_transfer_before_contract_replacement() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    assert_noop!(
      update_actor_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        transfer_execution_plan(before.sovereign_account.clone(), AssetKind::Native, 1_000,),
        CompletionPolicy::Persistent,
      ),
      Error::<Runtime>::SelfTransferNotAllowed
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
  });
}

#[test]
fn circular_actor_graph_cannot_reexecute_an_actor_in_the_same_block() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let event_schedule = on_address_event_schedule(SourceFilter::Any, AssetFilter::Any);
    let actor_a = create_user(
      ALICE,
      event_schedule.clone(),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let actor_a_account = actor_account(actor_a);
    let actor_b = create_user(
      CHARLIE,
      event_schedule,
      None,
      transfer_execution_plan(actor_a_account, AssetKind::Native, 1_000),
    );
    let actor_b_account = actor_account(actor_b);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_a,
      transfer_execution_plan(actor_b_account, AssetKind::Native, 1_000),
      CompletionPolicy::Persistent,
    ));
    System::set_block_number(2);
    for (owner, actor_id) in [(ALICE, actor_a), (CHARLIE, actor_b)] {
      assert_ok!(update_actor_contract_partial!(
        RuntimeOrigin::signed(owner.clone()),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress,
      ));
      fund_native(actor_id, 100_000_000_000_000);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(owner),
        actor_id
      ));
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_a)
        .expect("actor A exists")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(actor_b)
        .expect("actor B exists")
        .cycle_nonce,
      1
    );
    assert!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot state")
        .queue_ticket
        .is_some(),
      "B triggering already-executed A must create next-block work"
    );
    System::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_a)
        .expect("actor A exists")
        .cycle_nonce,
      2
    );
    assert_eq!(
      Actors::active_actor_view(actor_b)
        .expect("actor B exists")
        .cycle_nonce,
      1,
      "A-triggered recursive work for B must remain beyond the next block's cutoff"
    );
  });
}

#[test]
fn actor_observation_provider_maps_oracle_state_without_concrete_pallet_dependency() {
  seeded_test_ext().execute_with(|| {
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(ASSET_A);
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 0, 10),
      pallet_deos_actors::ScalarObservationState::Unavailable
    );
    assert_ok!(crate::configs::oracle_config::ensure_deos_router_pool_feeds(asset_in, asset_out,));
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 0, 10),
      pallet_deos_actors::ScalarObservationState::Uninitialized
    );
    System::set_block_number(1);
    publish_deos_router_observation(asset_in, asset_out, 50);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 1, 10),
      pallet_deos_actors::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      }
    );
    System::set_block_number(12);
    assert_eq!(
      <crate::configs::actor_config::TmctolObservationProvider as pallet_deos_actors::ObservationProvider<
        primitives::OracleFeedId,
        crate::BlockNumber,
      >>::observe(&feed, 12, 10),
      pallet_deos_actors::ScalarObservationState::Stale
    );
  });
}

#[test]
fn router_fee_routing_notifies_burn_actor_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
    ));
    System::set_block_number(2);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      transfer_execution_plan(BOB, AssetKind::Native, 777),
      CompletionPolicy::Persistent,
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(
      crate::configs::deos_router_config::FeeManagerImpl::<Runtime>::route_fee(
        &ALICE,
        AssetKind::Native,
        10_000,
      )
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(777));
  });
}

#[test]
fn router_fee_transfer_rolls_back_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let burn_actor_id = primitives::ecosystem::actor_ids::BURN_ACTOR_ID;
    let funding_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      (funding_plan, CompletionPolicy::Persistent,)
    ));
    System::set_block_number(2);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      burn_actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(burn_actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(burn_actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Burn Actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let alice_before = native_balance(&ALICE);
    let sovereign_before = native_balance(&sovereign);
    assert_noop!(
      crate::configs::deos_router_config::FeeManagerImpl::<Runtime>::route_fee(
        &ALICE,
        AssetKind::Native,
        10_000,
      ),
      pallet_deos_router::AdapterFailure::new(
        Error::<Runtime>::FundingAccumulatorOverflow.into(),
        pallet_deos_router::RouterFailureClass::IngressRejected,
        pallet_deos_router::RetryDisposition::Permanent,
      )
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn deos_sovereign_account_policy_reserves_genesis_custody_accounts() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // Every genesis System Actors custody account (including the Fee Sink) is host-reserved by
    // DeosSovereignAccountPolicy; a hashed sovereign derivation can never alias them.
    let ids = primitives::ecosystem::actor_ids::BURN_ACTOR_ID
      ..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID;
    for id in ids {
      let sovereign = crate::Actors::sovereign_account_id_system(id);
      assert!(
        crate::configs::actor_config::DeosSovereignAccountPolicy::is_reserved(&sovereign),
        "genesis System Actors id {id} sovereign must be reserved"
      );
    }
    // A User-slot derived sovereign is not reserved, so ordinary creation remains admissible.
    let slot = 200u8;
    let user_sovereign = crate::Actors::sovereign_account_id(&ALICE, slot);
    assert!(
      !crate::configs::actor_config::DeosSovereignAccountPolicy::is_reserved(&user_sovereign)
    );
  });
}

#[test]
fn genesis_system_locator_is_recoverable_after_close_through_reattachment() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // A genesis System locator (the Fee Sink) is host-reserved for fresh derivation
    // but MUST be recoverable by reattaching a fresh actor to its exact registered
    // Vacant locator after close (spec 5.4): context-aware reservation.
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let sovereign = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let original_sovereign_balance_before = Balances::free_balance(&sovereign);
    let preserved = crate::EXISTENTIAL_DEPOSIT.saturating_mul(777);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, preserved);
    let original_identity = Actors::actor_identities(fee_sink_id).expect("genesis identity");
    let _original_nonce = original_identity.cycle_nonce;

    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), fee_sink_id));
    assert_eq!(
      crate::Actors::system_sovereigns(fee_sink_id),
      Some(pallet_deos_actors::SystemSovereignState::Vacant)
    );

    // Reattachment to the exact registered Vacant locator is allowed even though the
    // account belongs to the genesis System custody range.
    let fresh_id = crate::Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      fee_sink_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(
        manual_schedule(),
        None,
        transfer_execution_plan(BOB, AssetKind::Native, 1),
      ),
    ));
    let fresh = Actors::active_actor_view(fresh_id).expect("fresh Fee Sink identity");
    assert_ne!(fresh_id, fee_sink_id);
    assert_eq!(fresh.sovereign_account, sovereign);
    // Reattachment mints a fresh identity with a fresh nonce sequence (zero), never
    // inheriting the closed actor's nonce or run state.
    assert_eq!(fresh.cycle_nonce, 0, "reattachment resets the nonce");
    assert_eq!(
      Balances::free_balance(&sovereign),
      preserved + original_sovereign_balance_before,
      "reattachment preserves residual custody balances"
    );
  });
}

#[test]
fn ingress_adapter_without_source_matches_any_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(RuntimeAddressEventIngress::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(333));
  });
}

#[test]
fn ingress_adapter_without_source_is_ignored_by_owner_only_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = actor_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(RuntimeAddressEventIngress::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn transfer_ingress_updates_system_snapshot_without_pause_resume() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("steps fits");
    let target_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      target_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    fund_native_via_call(ALICE, target_id, 10_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 1);
    System::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 2);
    System::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      target_id
    ));
    run_idle_until_cycle_nonce(target_id, 3);
    let instance = Actors::active_actor_view(target_id).expect("Actors exists");
    assert_eq!(
      instance.lifecycle,
      pallet_deos_actors::ActiveLifecycle::Active
    );
    let target_sovereign = actor_account(target_id);
    let refill_amount = 8_000_000_000_000u128;
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_execution_plan(target_sovereign, AssetKind::Native, refill_amount),
    );
    fund_native(sender_id, 100_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      actor_funding(target_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&refill_amount)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == target_id)
    }));
  });
}

#[test]
fn xcm_ingress_with_source_triggers_owner_only_on_address_event() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [7u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn system_runtime_policy_defaults_deny_for_signed_internal_and_xcm_provenance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let recipient = account_location(sovereign.clone());
    let sourced_amount = 10_000_000_000_000;
    let context = xcm::latest::XcmContext {
      origin: Some(account_location(ALICE)),
      message_id: [6u8; 32],
      topic: None,
    };
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(native_xcm_asset(sourced_amount)),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    let source_less_amount = 7_000_000_000_000;
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(native_xcm_asset(source_less_amount)),
        &recipient,
        None,
      )
      .is_ok()
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      3_000,
      &ALICE
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      AssetKind::Native,
      4_000,
      &ALICE
    ));
    assert_eq!(
      native_balance(&sovereign),
      sourced_amount.saturating_add(source_less_amount)
    );
    let funding = actor_funding(actor_id);
    assert!(
      funding
        .funding_accumulated
        .get(&AssetKind::Native)
        .is_none()
    );
  });
}

#[test]
fn xcm_deposit_rejects_before_value_movement_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let recipient = account_location(sovereign.clone());
    let context = xcm::latest::XcmContext {
      origin: Some(account_location(ALICE)),
      message_id: [8u8; 32],
      topic: None,
    };
    let sovereign_before = native_balance(&sovereign);
    let result = <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
      asset_to_holding(native_xcm_asset(5_000)),
      &recipient,
      Some(&context),
    );
    assert!(matches!(
      result,
      Err((_, xcm::latest::Error::FailedToTransactAsset(_)))
    ));
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn xcm_ingress_without_source_is_ignored_for_owner_only() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        None,
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn xcm_mixed_ingress_single_deposit_triggers_single_cycle() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = actor_account(actor_id);
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [9u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::ActorAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

// --- Actors Platform: Scheduling & Budget ---

#[test]
fn cycle_does_not_execute_when_budget_is_too_small() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    let step = make_step(heavy_task);
    let steps = BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("steps fits");
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let attempt_weight =
      Actors::compute_cycle_weight_upper(instance.actor_class.actor_type(), &instance.steps);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let housekeeping_weight = Actors::on_idle(System::block_number(), Weight::MAX);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    System::set_block_number(2);
    let target_weight = housekeeping_weight
      .saturating_add(Actors::scheduler_admission_overhead())
      .saturating_add(attempt_weight)
      .saturating_sub(Weight::from_parts(1, 0));
    run_idle(target_weight);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(instance.pending_signal);
  });
}

#[test]
fn cycle_closes_with_balance_exhausted_before_the_smaller_weight_derived_fee() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    let step = make_step(heavy_task.clone());
    let steps = BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("steps fits");
    let fee_envelope = Actors::attempt_fee_envelope(ActorType::User, &steps, 0)
      .expect("runtime plan has a checked fee envelope");
    let min_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    assert!(
      fee_envelope.total < min_balance,
      "reference Weight-derived fee should remain below MinUserBalance"
    );
    let prefunded = user_prefunding_requirement(&steps);
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, min_balance.saturating_sub(1));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn fee_insufficiency_is_terminal_without_deferral_guard() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    let step = make_step(heavy_task.clone());
    let steps = BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("steps fits");
    let attempt_fee = Actors::attempt_fee_envelope(ActorType::User, &steps, 0)
      .expect("runtime plan has a checked fee envelope")
      .total;
    let prefunded = user_prefunding_requirement(&steps);
    let actor_id = create_user(ALICE, manual_schedule(), None, steps);
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, attempt_fee.saturating_sub(1));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn scheduler_fifo_order_is_deterministic_across_actor_types() {
  let cases = [(2u32, 2u32), (3u32, 3u32), (4u32, 2u32)];
  for (system_count, user_count) in cases {
    let run_case = || -> (alloc::vec::Vec<ActorId>, alloc::vec::Vec<ActorId>) {
      seeded_test_ext().execute_with(|| {
        System::set_block_number(1);
        let schedule = Schedule {
          trigger: Trigger::cadenced_always(1),
          cooldown_blocks: 0,
        };
        let steps = BoundedVec::try_from(vec![make_step(inert_task())]).expect("steps fits");
        let mut tracked: alloc::vec::Vec<ActorId> = alloc::vec::Vec::new();
        for _ in 0..system_count {
          tracked.push(create_system(ALICE, schedule.clone(), None, steps.clone()));
        }
        for _ in 0..user_count {
          let user_id = create_user(ALICE, schedule.clone(), None, steps.clone());
          fund_native(user_id, 100_000_000_000);
          tracked.push(user_id);
        }
        System::set_block_number(2);
        run_idle(Weight::MAX);
        let actual = actor_events()
          .into_iter()
          .filter_map(|event| match event {
            Event::CycleStarted { actor_id, .. } if tracked.contains(&actor_id) => Some(actor_id),
            _ => None,
          })
          .collect();
        (tracked, actual)
      })
    };
    let first = run_case();
    let second = run_case();
    assert_eq!(first.1, first.0, "scheduler must preserve FIFO order");
    assert_eq!(
      first, second,
      "FIFO order must be deterministic for system_count={}, user_count={}",
      system_count, user_count
    );
  }
}

#[test]
fn strict_head_of_line_heavy_head_deferral_preserves_follower_order() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // Heavy head first: three transfer steps make its cycle envelope strictly larger than the
    // single-step followers behind it, while the constrained remainder admits the head's probes
    // and consume but not its full cycle admission.
    let step = |amount| {
      make_step(Task::Transfer {
        to: BOB,
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(amount),
      })
    };
    let heavy_plan =
      BoundedVec::try_from(vec![step(1), step(2), step(3)]).expect("plan fits runtime bound");
    let head = create_system(ALICE, manual_schedule(), None, heavy_plan);
    fund_native(head, 1_000_000_000_000_000);
    let light_a = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    fund_native(light_a, 1_000_000_000_000_000);
    let light_b = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    fund_native(light_b, 1_000_000_000_000_000);
    for actor_id in [head, light_a, light_b] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    let tickets: Vec<_> = [head, light_a, light_b]
      .into_iter()
      .map(|id| {
        Actors::actor_hot(id)
          .and_then(|hot| hot.queue_ticket)
          .expect("triggered actor is queued")
      })
      .collect();
    assert_eq!(
      tickets,
      vec![0, 1, 2],
      "physical FIFO order is head, light A, light B"
    );

    System::set_block_number(2);
    System::reset_events();
    run_idle(starvation_blocked_budget(head));

    let head_inst = Actors::active_actor_view(head).expect("head survives deferral");
    assert_eq!(
      head_inst.cycle_nonce, 0,
      "head attempt is deferred, not admitted"
    );
    assert!(head_inst.pending_signal);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == head
    )));
    for (id, ticket) in [(light_a, 1), (light_b, 2)] {
      let inst = Actors::active_actor_view(id).expect("follower survives");
      assert_eq!(
        inst.cycle_nonce, 0,
        "follower never admitted behind the head"
      );
      assert!(
        Actors::actor_hot(id).is_some_and(|hot| hot.queue_ticket == Some(ticket)),
        "follower retains its exact physical ticket"
      );
    }
    assert!(
      !has_actor_event(|event| matches!(
        event,
        Event::CycleStarted { actor_id: id, .. } if *id == light_a || *id == light_b
      )),
      "no follower attempt starts behind an unadmitted head"
    );

    // Conforming full envelope: the head advances first, then followers in exact ticket order.
    System::set_block_number(3);
    System::reset_events();
    run_idle(Weight::MAX);
    let started: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![head, light_a, light_b]);
    for id in [head, light_a, light_b] {
      assert_eq!(
        Actors::active_actor_view(id)
          .expect("actor executed")
          .cycle_nonce,
        1
      );
    }
  });
}

#[test]
fn exact_input_task_uses_measured_caller_aware_router_weight() {
  seeded_test_ext().execute_with(|| {
    let task = Task::SwapIn {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let actor_upper = Actors::weight_upper_bound(&task);
    let measured =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_in();
    assert_eq!(actor_upper, measured);
  });
}

#[test]
fn exact_output_task_uses_generated_native_router_weight() {
  seeded_test_ext().execute_with(|| {
    let exact_in = Task::SwapIn {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let exact_out = Task::SwapOut {
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(1),
      asset_in: AssetKind::Native,
      input_limit: InputLimit::Absolute(10),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let exact_out_upper = Actors::weight_upper_bound(&exact_out);
    let measured =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_out();
    assert_eq!(exact_out_upper, measured);
    assert_eq!(
      Actors::weight_upper_bound(&exact_in),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_dex_exact_in()
    );
  });
}

#[test]
fn staking_tasks_use_separate_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let stake = Task::Stake {
      asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    let unstake = Task::Unstake {
      asset: AssetKind::Local(ASSET_A),
      shares: AmountResolution::Fixed(1),
    };
    assert_eq!(
      Actors::weight_upper_bound(&stake),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_stake()
    );
    assert_eq!(
      Actors::weight_upper_bound(&unstake),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_unstake()
    );
    assert!(
      Actors::weight_upper_bound(&unstake).ref_time()
        > Actors::weight_upper_bound(&stake).ref_time()
    );
  });
}

#[test]
fn liquidity_tasks_use_separate_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let add = Task::AddLiquidity {
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      amount_a: AmountResolution::Fixed(1),
      amount_b: AmountResolution::Fixed(1),
      min_lp_out: 1,
    };
    let donation = Task::DonateLiquidity {
      asset_a: AssetKind::Local(0),
      asset_b: AssetKind::Local(ASSET_A),
      max_amount_a: AmountResolution::Fixed(1),
      max_ratio_error: Perbill::zero(),
    };
    let remove = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      asset_a: AssetKind::Local(1),
      asset_b: AssetKind::Local(2),
      lp_amount: AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    };
    assert_eq!(
      Actors::weight_upper_bound(&add),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_add_liquidity()
    );
    assert_eq!(
      Actors::weight_upper_bound(&donation),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_donate_liquidity()
    );
    assert_eq!(
      Actors::weight_upper_bound(&remove),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::task_remove_liquidity()
    );
    assert_ne!(
      Actors::weight_upper_bound(&remove),
      Actors::weight_upper_bound(&add)
    );
  });
}

#[test]
fn wakeup_registration_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let expected =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_append_new_page()
        .saturating_add(
          <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_insert(),
        )
        .saturating_add(
          <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_remove_exact(),
        );
    assert_eq!(Actors::wakeup_registration_weight_upper(), expected);
  });
}

#[test]
fn scheduler_actor_probe_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let hot =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_hot_probe();
    let contract =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_contract_probe();
    assert_eq!(Actors::scheduler_actor_hot_probe_weight_upper(), hot);
    assert_eq!(Actors::scheduler_actor_contract_probe_weight_upper(), contract);
    assert_eq!(
      Actors::scheduler_actor_probe_weight_upper(),
      hot.saturating_add(contract)
    );
  });
}

#[test]
fn scheduler_paged_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let scan = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_tombstone_drain(1);
    let consume = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_paged_consume_delete_page();
    let hot = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_hot_probe();
    let contract =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_actor_contract_probe();
    assert!(scan.ref_time() > 0 && scan.proof_size() > 0);
    assert!(consume.ref_time() > 0 && consume.proof_size() > 0);
    assert_eq!(Actors::scheduler_actor_hot_probe_weight_upper(), hot);
    assert_eq!(Actors::scheduler_actor_contract_probe_weight_upper(), contract);
  });
}

#[test]
fn wakeup_drain_admission_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    assert_eq!(
      Actors::wakeup_cursor_drain_unit_weight_upper(false),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_partial()
    );
    assert_eq!(
      Actors::wakeup_cursor_drain_unit_weight_upper(true),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::scheduler_wakeup_cursor_worker_remove()
    );
  });
}

#[test]
fn transaction_extension_ingress_uses_generated_runtime_weights() {
  seeded_test_ext().execute_with(|| {
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: polkadot_sdk::sp_runtime::MultiAddress::Id(BOB),
      value: 1,
    });
    let notify = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_notify();
    let base = <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_base();
    assert_eq!(AddressEventIngressExtension.weight(&call), notify);
    assert!(base.all_lte(notify));
    assert!(base.proof_size() > 0);
    let unmatched_refund = AddressEventIngressExtension::post_dispatch_refund(false, false);
    assert_eq!(notify.saturating_sub(unmatched_refund), base);
    assert_eq!(
      AddressEventIngressExtension::post_dispatch_refund(false, true),
      Weight::zero()
    );
    assert_eq!(
      AddressEventIngressExtension::post_dispatch_refund(true, false),
      notify
    );
  });
}

#[test]
fn certified_ingress_inventory_is_closed_and_typed() {
  seeded_test_ext().execute_with(|| {
    let inventory = RuntimeAddressEventIngress::certified_producer_inventory();
    assert!(
      !inventory.is_empty(),
      "inventory must name every producer path"
    );
    let mut ids = alloc::vec::Vec::new();
    for producer in inventory {
      assert!(!producer.id.is_empty());
      assert!(!producer.credited_surface.is_empty());
      assert!(!producer.source_provenance.is_empty());
      assert!(!producer.preflight_owner.is_empty());
      assert!(!producer.notify_owner.is_empty());
      assert!(!producer.rollback_owner.is_empty());
      assert!(!producer.weight_owner.is_empty());
      ids.push(producer.id);
    }
    let unique = ids
      .iter()
      .collect::<alloc::collections::BTreeSet<_>>()
      .len();
    assert_eq!(ids.len(), unique, "producer ids must be unique");
    // The runtime adapter implements the typed boundary: absent destinations are
    // balance-only no-ops for both preflight and notify.
    let event = pallet_deos_actors::AddressEvent {
      destination: BOB,
      source: Some(ALICE),
      asset: AssetKind::Native,
      amount: 1,
      provenance: Some(pallet_deos_actors::FundingProvenance::Signed),
    };
    assert_ok!(
      <RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
        AccountId,
        AssetKind,
        Balance,
      >>::preflight(&event)
    );
    assert_ok!(
      <RuntimeAddressEventIngress as pallet_deos_actors::AddressEventIngress<
        AccountId,
        AssetKind,
        Balance,
      >>::notify(&event)
    );
  });
}

#[test]
fn certified_extension_notify_failure_rejects_and_rolls_back_value_and_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer_pair = sr25519::Pair::from_seed(&[53u8; 32]);
    let signer = crate::AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer,
      1_000_000_000_000_000_000,
    );
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    // Monotonic ticket namespace at the ceiling: the certified post-movement
    // notify cannot place readiness and must reject the whole outer transaction,
    // restoring the value movement together with every Actors effect (spec 5.3).
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let sovereign_before = native_balance(&sovereign);
    let signer_before = native_balance(&signer);
    let transfer_amount = 25_000_000_000_000u128;
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: transfer_amount,
    });
    // A rejected certified movement aborts the block in production, so the ledger
    // revert is block-level: FRAME does not retroactively roll back an already
    // dispatched call on a post-dispatch extension error. Model that boundary
    // explicitly: the rejected extrinsic leaves no balance or Actors residue.
    let rejected = polkadot_sdk::frame_support::storage::with_transaction(
      || -> polkadot_sdk::frame_support::storage::TransactionOutcome<Result<bool, DispatchError>> {
        let result = Executive::apply_extrinsic(signed_extrinsic(&signer_pair, 0, call));
        let rejected = result.is_err() || matches!(result, Ok(Err(_)));
        match rejected {
          true => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(rejected)),
          false => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(rejected)),
        }
      },
    )
    .expect("certified movement rejection check");
    assert!(
      rejected,
      "certified movement must fail when Actors readiness cannot commit"
    );
    assert_eq!(
      native_balance(&sovereign),
      sovereign_before,
      "rejected certified movement restores the value movement"
    );
    assert_eq!(
      native_balance(&signer),
      signer_before,
      "rejected extrinsic leaves the payer untouched"
    );
    assert!(
      !Actors::pending_signal(actor_id),
      "no readiness latch survives a rejected certified movement"
    );
  });
}

#[test]
fn asset_ops_transfer_preserves_ingress_classification_through_task_failure() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      BoundedVec::try_from(vec![make_step(inert_task())]).expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&ALICE, 100_000_000_000_000);
    // Monotonic ticket exhaustion is Permanent through the certified Actors transfer
    // path, and the movement rolls back with the failed notify (spec 6.1).
    pallet_deos_actors::NextQueueTicket::<Runtime>::put(u64::MAX);
    let actor_before = native_balance(&sovereign);
    let failure = TmctolAssetOps::transfer(&ALICE, &sovereign, AssetKind::Native, 5_000)
      .expect_err("ticket exhaustion must reject the certified transfer");
    assert_eq!(
      failure.retry,
      RetryClass::Permanent,
      "monotonic namespace exhaustion stays Permanent through TaskFailure"
    );
    assert_eq!(
      native_balance(&sovereign),
      actor_before,
      "certified transfer rolls back movement and Actors effects together"
    );
    // An absent sovereign destination is balance-only: the same transfer succeeds
    // and performs no Actors work.
    let bob_before = native_balance(&BOB);
    assert_ok!(TmctolAssetOps::transfer(
      &ALICE,
      &BOB,
      AssetKind::Native,
      3_000,
    ));
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(3_000));
  });
}

#[test]
fn signed_balance_deposit_credits_rejected_donor_but_only_owner_activates_funding() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[45u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[46u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    for account in [&owner, &donor] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(owner.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let donor_amount = 9_000_000_000_000;
    let donor_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: donor_amount,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, donor_call)),
      Ok(Ok(_))
    ));
    let dust_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: 1,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 1, dust_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      native_balance(&sovereign),
      sovereign_before
        .saturating_add(donor_amount)
        .saturating_add(1)
    );
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    let owner_amount = 11_000_000_000_000;
    let owner_call =
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(sovereign.clone()),
        value: owner_amount,
      });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, owner_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&owner_amount)
    );
  });
}

#[test]
fn signed_asset_deposit_keeps_rejected_donor_balance_only_and_owner_authoritative() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[47u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[48u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    for account in [&owner, &donor] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let asset_id = 4_242u32;
    assert_ok!(create_test_asset(asset_id, &owner));
    assert_ok!(mint_tokens(asset_id, &owner, &owner, 100_000));
    assert_ok!(mint_tokens(asset_id, &owner, &donor, 100_000));
    let tracked_asset = AssetKind::Local(asset_id);
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: tracked_asset,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(owner.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let donor_amount = 9_000;
    let donor_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
      id: asset_id,
      target: Address::Id(sovereign.clone()),
      amount: donor_amount,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, donor_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, sovereign.clone()), donor_amount);
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    let owner_amount = 11_000;
    let owner_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
      id: asset_id,
      target: Address::Id(sovereign.clone()),
      amount: owner_amount,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, owner_call)),
      Ok(Ok(_))
    ));
    assert_eq!(
      Assets::balance(asset_id, sovereign),
      donor_amount.saturating_add(owner_amount)
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&tracked_asset),
      Some(&owner_amount)
    );
  });
}

#[test]
fn dynamic_asset_producers_notify_directly_with_balance_only_provenance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let owner_pair = sr25519::Pair::from_seed(&[50u8; 32]);
    let donor_pair = sr25519::Pair::from_seed(&[51u8; 32]);
    let delegate_pair = sr25519::Pair::from_seed(&[52u8; 32]);
    let owner = crate::AccountId::from(owner_pair.public());
    let donor = crate::AccountId::from(donor_pair.public());
    let delegate = crate::AccountId::from(delegate_pair.public());
    for account in [&owner, &donor, &delegate] {
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        account,
        1_000_000_000_000_000_000,
      );
    }
    let asset_id = 4_243u32;
    assert_ok!(create_test_asset(asset_id, &owner));
    assert_ok!(mint_tokens(asset_id, &owner, &donor, 100_000));
    let tracked_asset = AssetKind::Local(asset_id);
    let make_actor = || {
      create_user(
        owner.clone(),
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        None,
        BoundedVec::try_from(vec![make_step(Task::Transfer {
          to: BOB,
          asset: tracked_asset,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
        })])
        .expect("execution plan fits"),
      )
    };

    let mint_actor = make_actor();
    let mint_sovereign = actor_account(mint_actor);
    let mint_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::mint {
      id: asset_id,
      beneficiary: Address::Id(mint_sovereign.clone()),
      amount: 7_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 0, mint_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, mint_sovereign), 7_000);
    assert!(
      Actors::actor_hot(mint_actor)
        .expect("mint actor")
        .pending_signal
    );
    assert!(actor_funding(mint_actor).funding_accumulated.is_empty());

    let force_actor = make_actor();
    let force_sovereign = actor_account(force_actor);
    let force_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::force_transfer {
      id: asset_id,
      source: Address::Id(donor.clone()),
      dest: Address::Id(force_sovereign.clone()),
      amount: 8_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&owner_pair, 1, force_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, force_sovereign), 8_000);
    assert!(
      Actors::actor_hot(force_actor)
        .expect("force actor")
        .pending_signal
    );
    assert!(actor_funding(force_actor).funding_accumulated.is_empty());

    let approved_actor = make_actor();
    let approved_sovereign = actor_account(approved_actor);
    let approve_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::approve_transfer {
      id: asset_id,
      delegate: Address::Id(delegate.clone()),
      amount: 9_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&donor_pair, 0, approve_call)),
      Ok(Ok(_))
    ));
    let approved_call = RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer_approved {
      id: asset_id,
      owner: Address::Id(donor),
      destination: Address::Id(approved_sovereign.clone()),
      amount: 9_000,
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&delegate_pair, 0, approved_call)),
      Ok(Ok(_))
    ));
    assert_eq!(Assets::balance(asset_id, approved_sovereign), 9_000);
    assert!(
      Actors::actor_hot(approved_actor)
        .expect("approved actor")
        .pending_signal
    );
    assert!(actor_funding(approved_actor).funding_accumulated.is_empty());
  });
}

#[test]
fn signed_fixed_transfer_is_rejected_before_dispatch_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[43u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(signer_account.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: 1,
    });
    assert!(Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call)).is_err());
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn signed_transfer_all_is_rejected_before_dispatch_when_funding_pending_overflows() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[44u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000,
    );
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(signer_account.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(AssetKind::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_all {
      dest: Address::Id(sovereign.clone()),
      keep_alive: true,
    });
    assert!(Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call)).is_err());
    assert_eq!(native_balance(&sovereign), sovereign_before);
  });
}

#[test]
fn signed_transfer_all_records_actual_post_fee_movement_without_event_scan() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[49u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000,
    );
    let steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("execution plan fits");
    let actor_id = create_user(signer_account.clone(), manual_schedule(), None, steps);
    let sovereign = actor_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_all {
      dest: Address::Id(sovereign.clone()),
      keep_alive: true,
    });

    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer, 0, call)),
      Ok(Ok(_))
    ));
    let actual = native_balance(&sovereign).saturating_sub(sovereign_before);
    assert!(actual > 0);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&actual)
    );
  });
}

#[test]
fn executive_pipeline_covers_transaction_extension_ingress_and_refunds() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let signer = sr25519::Pair::from_seed(&[42u8; 32]);
    let signer_account = crate::AccountId::from(signer.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer_account,
      1_000_000_000_000_000_000_000_000,
    );
    let sources = BoundedVec::try_from(vec![
      TriggerSource::OnAddressEvent {
        source_filter: SourceFilter::Any,
        asset_filter: AssetFilter::Any,
      },
      TriggerSource::OnAddressEvent {
        source_filter: SourceFilter::OwnerOnly,
        asset_filter: AssetFilter::Any,
      },
    ])
    .expect("two trigger sources fit");
    let actor_id = create_user(
      signer_account.clone(),
      Schedule {
        trigger: Trigger::Immediate { sources },
        cooldown_blocks: 0,
      },
      None,
      BoundedVec::try_from(vec![make_step(Task::Transfer {
        to: BOB,
        asset: AssetKind::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })])
      .expect("execution plan fits"),
    );
    let sovereign = actor_account(actor_id);
    let notify_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_notify();
    let base_weight =
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::transaction_extension_ingress_base();
    let transfer_amount = 10_000_000_000_000;
    let matched = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign.clone()),
      value: transfer_amount,
    });
    let balance_before_matched = native_balance(&signer_account);
    let matched_result = Executive::apply_extrinsic(signed_extrinsic(&signer, 0, matched));
    assert!(matches!(matched_result, Ok(Ok(_))), "{matched_result:?}");
    let matched_fee = balance_before_matched
      .saturating_sub(native_balance(&signer_account))
      .saturating_sub(transfer_amount);
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&AssetKind::Native),
      Some(&transfer_amount)
    );
    let unmatched = RuntimeCall::Balances(
      polkadot_sdk::pallet_balances::Call::transfer_allow_death {
        dest: Address::Id(BOB),
        value: transfer_amount,
      },
    );
    let balance_before_unmatched = native_balance(&signer_account);
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer, 1, unmatched)),
      Ok(Ok(_))
    ));
    let unmatched_fee = balance_before_unmatched
      .saturating_sub(native_balance(&signer_account))
      .saturating_sub(transfer_amount);
    assert!(
      unmatched_fee < matched_fee,
      "successful tracked calls without an Actors recipient must refund the unused notification envelope"
    );
    assert!(notify_weight.saturating_sub(base_weight) != Weight::zero());
    assert!(Actors::pending_signal(actor_id));
    let untracked = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
      remark: b"untracked ingress call".to_vec(),
    });
    assert!(matches!(
      Executive::apply_extrinsic(signed_extrinsic(&signer, 2, untracked)),
      Ok(Ok(_))
    ));
    assert!(Actors::pending_signal(actor_id));
    let failed_value = native_balance(&signer_account).saturating_add(1);
    let failed = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_allow_death {
      dest: Address::Id(sovereign),
      value: failed_value,
    });
    let failed_extrinsic = signed_extrinsic(&signer, 3, failed);
    let declared_failed_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Runtime>::compute_fee(
      failed_extrinsic.encoded_size() as u32,
      &failed_extrinsic.get_dispatch_info(),
      0,
    );
    let balance_before_failed = native_balance(&signer_account);
    assert!(matches!(
      Executive::apply_extrinsic(failed_extrinsic),
      Ok(Err(_))
    ));
    let failed_fee = balance_before_failed.saturating_sub(native_balance(&signer_account));
    assert!(Actors::pending_signal(actor_id));
    assert!(
      failed_fee < declared_failed_fee,
      "failed tracked calls must pay less than their declared envelope after post-dispatch refund"
    );
  });
}

#[test]
fn split_transfer_task_uses_the_single_runtime_weight_authority() {
  seeded_test_ext().execute_with(|| {
    let max_legs = <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get();
    let legs = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let task = Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs: SplitTransferLegsOf::<Runtime>::try_from(legs).expect("maximum legs fit"),
    };
    assert_eq!(
      Actors::weight_upper_bound(&task),
      <<Runtime as pallet_deos_actors::Config>::WeightInfo as pallet_deos_actors::WeightInfo>::task_split_transfer(
        max_legs,
      )
    );
  });
}

#[test]
fn maximum_single_task_attempt_and_cleanup_fit_derived_service_envelope() {
  seeded_test_ext().execute_with(|| {
    let max_legs =
      <<Runtime as pallet_deos_actors::Config>::MaxSplitTransferLegs as Get<u32>>::get();
    let legs = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let task = Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs: SplitTransferLegsOf::<Runtime>::try_from(legs).expect("maximum legs fit"),
    };
    let plan: ExecutionPlanOf<Runtime> =
      BoundedVec::try_from(vec![make_step(task)]).expect("one maximum task fits");
    let service = Actors::guaranteed_actor_service_weight().expect("runtime envelope is valid");

    let maximum_attempt = Actors::execution_plan_admission_weight_upper(ActorType::System, &plan);
    assert!(
      maximum_attempt.all_lte(service),
      "maximum_attempt={maximum_attempt:?}, service={service:?}"
    );
    assert!(Actors::close_dispatch_weight_upper().all_lte(service));
  });
}

#[test]
fn on_initialize_is_a_zero_weight_noop() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(actor_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::set_block_number(2);
    assert_eq!(Actors::on_initialize(2), Weight::zero());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn block_weight_partition_is_50_dispatch_50_on_idle_without_operational_reserve() {
  let maximum = crate::MAXIMUM_BLOCK_WEIGHT;
  let normal = crate::NORMAL_DISPATCH_RATIO * maximum;
  let on_idle = crate::MIN_ON_IDLE_RESERVE_RATIO * maximum;
  let dispatchable = crate::configs::MaxDispatchableExtrinsicWeight::get();
  let operational = dispatchable.saturating_sub(normal);

  assert_eq!(normal, Perbill::from_percent(50) * maximum);
  assert_eq!(operational, Weight::zero());
  assert_eq!(on_idle, Perbill::from_percent(50) * maximum);
  assert_eq!(
    crate::configs::RuntimeBlockWeights::get()
      .get(DispatchClass::Operational)
      .reserved,
    None
  );
  assert_eq!(
    normal.saturating_add(operational).saturating_add(on_idle),
    maximum
  );
}

#[test]
fn configured_on_idle_reserve_admits_every_genesis_actor_with_pure_cleanup() {
  seeded_test_ext().execute_with(|| {
    let reserve = <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    assert_eq!(
      reserve,
      crate::MIN_ON_IDLE_RESERVE_RATIO * crate::MAXIMUM_BLOCK_WEIGHT
    );
    let mut actor_count = 0u32;
    let mut max_ref_time = (0u64, 0u64);
    let mut max_proof_size = (0u64, 0u64);
    for actor_id in pallet_deos_actors::ActorHot::<Runtime>::iter_keys() {
      let instance = Actors::active_actor_view(actor_id).expect("split active actor exists");
      let required = Actors::execution_plan_admission_weight_upper(
        instance.actor_class.actor_type(),
        &instance.steps,
      );
      assert!(
        required.all_lte(reserve),
        "actor_id={actor_id}, required={required:?}, reserve={reserve:?}",
      );
      if required.ref_time() > max_ref_time.1 {
        max_ref_time = (actor_id, required.ref_time());
      }
      if required.proof_size() > max_proof_size.1 {
        max_proof_size = (actor_id, required.proof_size());
      }
      actor_count = actor_count.saturating_add(1);
    }
    assert!(
      actor_count > 0,
      "reference genesis must contain System Actors"
    );
    println!(
      "Actors admission: actors={actor_count}, reserve={reserve:?}, max_ref_time={max_ref_time:?}, max_proof_size={max_proof_size:?}"
    );
  });
}

#[test]
fn configured_on_idle_reserve_admits_one_scheduler_actor_probe() {
  let required = Actors::scheduler_admission_overhead();
  let reserve = crate::MIN_ON_IDLE_RESERVE_RATIO * crate::MAXIMUM_BLOCK_WEIGHT;
  assert!(
    required.all_lte(reserve),
    "required={required:?}, reserve={reserve:?}"
  );
}

#[test]
fn starvation_emits_observability_event_once_on_threshold_crossing() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    System::set_block_number(1);
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 10),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(starvation_blocked_budget(actor_id));
    assert!(matches!(
      IdleStarvationState::<Runtime>::get(),
      IdleStarvationPhase::Starving {
        consecutive_blocks: 1,
      }
    ));
    for block in 2..=(threshold + 2) {
      System::set_block_number(block);
      run_idle(starvation_blocked_budget(actor_id));
    }
    let detections = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationDetected { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(detections, vec![threshold]);
    assert_eq!(
      IdleStarvationState::<Runtime>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold + 2,
      }
    );
  });
}

#[test]
fn starvation_requires_live_fifo_work() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    assert!(!IdleStarvationState::<Runtime>::exists());
    // An empty queue with an exhausted budget must never starve: no live FIFO work exists.
    for block in 1..=(threshold + 2) {
      System::set_block_number(block);
      run_idle(starvation_observation_weight());
    }
    assert!(!IdleStarvationState::<Runtime>::exists());
  });
}

#[test]
fn starvation_recovery_is_observable_and_healthy_idle_stays_sparse() {
  seeded_test_ext().execute_with(|| {
    let threshold =
      <<Runtime as pallet_deos_actors::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    assert!(!IdleStarvationState::<Runtime>::exists());
    System::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 10),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    for block in 2..=(threshold + 1) {
      System::set_block_number(block);
      run_idle(starvation_blocked_budget(actor_id));
    }
    assert_eq!(
      IdleStarvationState::<Runtime>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    System::set_block_number(threshold.saturating_add(2));
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationRecovered { consecutive_blocks }
        if *consecutive_blocks == threshold
    )));
    let recovery_count = System::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        )
      })
      .count();
    System::set_block_number(threshold.saturating_add(3));
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Runtime>::exists());
    assert_eq!(
      System::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      recovery_count
    );
  });
}

// --- Actors Platform: Owner Slots ---

#[test]
fn system_actor_count_is_not_limited_by_owner_slots() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let attempts =
      <<Runtime as pallet_deos_actors::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<crate::AccountId> = Vec::new();
    for _ in 0..attempts {
      let actor_id = create_system(
        ALICE,
        manual_schedule(),
        None,
        transfer_execution_plan(BOB, AssetKind::Native, 1),
      );
      let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert_eq!(
        inst.actor_class,
        pallet_deos_actors::ActorClass::System {
          sovereign_id: actor_id,
        }
      );
      sovereign_accounts.push(inst.sovereign_account);
    }
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    for i in 0..sovereign_accounts.len() {
      for j in i + 1..sovereign_accounts.len() {
        assert_ne!(sovereign_accounts[i], sovereign_accounts[j]);
      }
    }
  });
}

#[test]
fn governance_can_update_active_actor_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_limit = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    assert_ok!(Actors::set_active_actor_limit(
      RuntimeOrigin::root(),
      max_limit - 1,
    ));
    assert_eq!(
      pallet_deos_actors::ActiveActorLimit::<Runtime>::get(),
      max_limit - 1
    );
    let actor_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 0),
      pallet_deos_actors::Error::<Runtime>::ActiveActorLimitTooLow
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      pallet_deos_actors::Error::<Runtime>::ActiveActorLimitTooHigh
    );
  });
}

#[test]
fn owner_slot_reuses_freed_slot_after_close() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let id0 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let id1 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot0 = Actors::active_actor_view(id0)
      .expect("id0 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    let slot1 = Actors::active_actor_view(id1)
      .expect("id1 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot2 = Actors::active_actor_view(id2)
      .expect("id2 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot2, slot0);
  });
}

// --- User DCA Lifecycle ---

#[test]
fn user_dca_e2e_lifecycle_with_explicit_close() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let create_fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    let initial_alice_balance = Balances::free_balance(&ALICE);
    let schedule = Schedule {
      trigger: Trigger::cadenced_always(5),
      cooldown_blocks: 0,
    };
    let foreign = AssetKind::Local(ASSET_A);
    let swap_amount = primitives::ecosystem::params::PRECISION;
    let steps = BoundedVec::try_from(vec![StepOf::<Runtime> {
      preconditions: pallet_deos_actors::Preconditions::Unconditional,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: foreign,
        amount_in: AmountResolution::Fixed(swap_amount),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .unwrap();
    let id = create_user(ALICE, schedule, None, steps.clone());
    assert!(has_actor_event(
      |e| matches!(e, Event::ActorCreated { actor_id, .. } if *actor_id == id)
    ));
    assert_eq!(
      Balances::free_balance(&ALICE),
      initial_alice_balance - create_fee
    );
    let sov = Actors::sovereign_account_id(&ALICE, 0);
    let min_user_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let inst = Actors::active_actor_view(id).unwrap();
    let per_cycle_fee = Actors::attempt_fee_envelope(inst.actor_class.actor_type(), &inst.steps, 0)
      .expect("admitted plan has a checked fee envelope")
      .total;
    let native_funding = min_user_balance + (per_cycle_fee + swap_amount) * 3;
    let _ = <Balances as Currency<crate::AccountId>>::transfer(
      &ALICE,
      &sov,
      native_funding,
      polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
    );
    let mut max_nonce = 0;
    for block in 2..=20 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      for event in System::events() {
        if let RuntimeEvent::Actors(Event::CycleSummary {
          actor_id: ev_id,
          cycle_nonce,
          ..
        }) = event.event
          && ev_id == id
          && cycle_nonce > max_nonce
        {
          max_nonce = cycle_nonce;
        }
      }
      System::reset_events();
    }
    assert!(max_nonce >= 2, "Should have executed at least 2 cycles");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
    assert!(Actors::active_actor_view(id).is_none());
    let id_new = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot_new = Actors::active_actor_view(id_new)
      .expect("id_new exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot_new, 0);
  });
}

// --- Circular Transfer Chain Stress Tests ---

/// Creates `n` System Actors with explicit StopCycle contracts for scheduler stress testing.
fn inert_timer_contract() -> pallet_deos_actors::ContractInputOf<Runtime> {
  system_active_contract(
    Schedule {
      trigger: Trigger::cadenced_always(1),
      cooldown_blocks: 0,
    },
    None,
    alloc::vec![pallet_deos_actors::Step {
      preconditions: Default::default(),
      task: inert_task(),
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits"),
  )
}

fn setup_inert_actors(n: u64, initial_balance: u128) -> alloc::vec::Vec<u64> {
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
  }
  actor_ids
}

fn setup_mixed_inert_actors(n: u64, initial_balance: u128) -> alloc::vec::Vec<u64> {
  let mut actor_ids = alloc::vec::Vec::new();
  let inert_plan: ExecutionPlan = alloc::vec![pallet_deos_actors::Step {
    preconditions: Default::default(),
    task: inert_task(),
    on_error: StepErrorPolicy::AbortCycle,
  }]
  .try_into()
  .expect("fits");
  for index in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    if index % 2 == 0 {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        inert_timer_contract(),
      ));
    } else {
      let mut owner_bytes = [0u8; 32];
      owner_bytes[..8].copy_from_slice(&index.to_le_bytes());
      owner_bytes[31] = 0xA7;
      let owner = crate::AccountId::from(owner_bytes);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&owner, initial_balance);
      prefund_active_user_creation(&owner, &inert_plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(owner),
        Mutability::Mutable,
        inert_timer_contract(),
      ));
    }
    age_fixture_control_clock(actor_id);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("mixed stress actor exists")
      .sovereign_account;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, initial_balance);
    actor_ids.push(actor_id);
  }
  actor_ids
}

fn setup_inert_actors_sparse(n: u64, initial_balance: u128, stride: u64) -> alloc::vec::Vec<u64> {
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let effective_stride = stride.max(2);
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_id);
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    let bumped_next = actor_id.saturating_add(effective_stride);
    pallet_deos_actors::NextActorId::<Runtime>::put(bumped_next);
  }
  actor_ids
}

/// Helper: creates `n` System Actors in a circular transfer chain.
/// Returns (actor_ids, sovereign_accounts).
fn setup_circular_chain(
  n: u64,
  initial_balance: u128,
) -> (alloc::vec::Vec<u64>, alloc::vec::Vec<crate::AccountId>) {
  let transfer_pct = Perbill::from_percent(1);
  let mut actor_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let mut sovereign_accounts = alloc::vec::Vec::new();
  for _ in 0..n {
    let actor_id = crate::Actors::next_actor_id();
    actor_ids.push(actor_id);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_id);
    let sov = Actors::sovereign_account_id_system(actor_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    sovereign_accounts.push(sov);
  }
  for i in 0..n {
    let next_sov = sovereign_accounts[((i + 1) % n) as usize].clone();
    let steps: ExecutionPlanOf<Runtime> = alloc::vec![pallet_deos_actors::Step {
      preconditions: all_preconditions(alloc::vec![pallet_deos_actors::Predicate::BalanceAbove {
        asset: primitives::AssetKind::Native,
        threshold: crate::EXISTENTIAL_DEPOSIT,
      },]),
      task: Task::Transfer {
        to: next_sov,
        asset: primitives::AssetKind::Native,
        amount: AmountResolution::PercentageOfCurrent(transfer_pct),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_ids[i as usize],
      (steps, CompletionPolicy::Persistent,)
    ));
  }
  (actor_ids, sovereign_accounts)
}

/// Per-block diagnostic counters collected during stress run.
struct StressDiagnostics {
  actor_cycle_counts: alloc::collections::BTreeMap<u64, u32>,
  total_failed_steps: u32,
  min_per_block: u32,
  max_per_block: u32,
}

struct QueuePressureDiagnostics {
  max_queue_occupancy: u32,
  max_wakeup_backlog: u32,
  max_wakeup_buckets: u32,
}

/// Runs `num_blocks` blocks with on_initialize + on_idle, collecting per-block diagnostics.
fn run_blocks_with_diagnostics(
  actor_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> StressDiagnostics {
  let (diag, _) = run_blocks_with_queue_diagnostics(actor_ids, num_blocks, weight);
  diag
}

fn run_blocks_with_queue_diagnostics(
  actor_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> (StressDiagnostics, QueuePressureDiagnostics) {
  let mut diag = StressDiagnostics {
    actor_cycle_counts: actor_ids.iter().map(|&id| (id, 0u32)).collect(),
    total_failed_steps: 0,
    min_per_block: u32::MAX,
    max_per_block: 0,
  };
  let mut queue_diag = QueuePressureDiagnostics {
    max_queue_occupancy: 0,
    max_wakeup_backlog: 0,
    max_wakeup_buckets: 0,
  };
  for block in 2..=(num_blocks + 1) {
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    Actors::on_idle(block, weight);
    let mut block_executions = 0u32;
    for evt in System::events() {
      match &evt.event {
        RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, outcomes, ..
        }) => {
          if let Some(count) = diag.actor_cycle_counts.get_mut(actor_id) {
            *count += 1;
          }
          block_executions += 1;
          diag.total_failed_steps += outcomes.failed_steps;
        }
        _ => {}
      }
    }
    let queue_occupancy = Actors::queue_tail()
      .saturating_sub(Actors::queue_head())
      .min(u64::from(u32::MAX)) as u32;
    let mut wakeup_backlog = 0u32;
    let mut wakeup_buckets = 0u32;
    for (_, bucket) in pallet_deos_actors::WakeupBuckets::<Runtime>::iter() {
      wakeup_backlog = wakeup_backlog.saturating_add(bucket.live_entries);
      wakeup_buckets = wakeup_buckets.saturating_add(1);
    }
    queue_diag.max_queue_occupancy = queue_diag.max_queue_occupancy.max(queue_occupancy);
    queue_diag.max_wakeup_backlog = queue_diag.max_wakeup_backlog.max(wakeup_backlog);
    queue_diag.max_wakeup_buckets = queue_diag.max_wakeup_buckets.max(wakeup_buckets);
    diag.min_per_block = diag.min_per_block.min(block_executions);
    diag.max_per_block = diag.max_per_block.max(block_executions);
  }
  (diag, queue_diag)
}

/// Asserts stability invariants that apply regardless of capacity scenario.
fn assert_core_stability(actor_ids: &[u64], diag: &StressDiagnostics) {
  assert_eq!(
    diag.total_failed_steps, 0,
    "All transfer steps must succeed (got {} failures)",
    diag.total_failed_steps,
  );
  for &id in actor_ids {
    let inst = Actors::active_actor_view(id).expect("actor must still exist");
    assert_eq!(
      inst.consecutive_failures, 0,
      "Actors {} has {} consecutive failures",
      id, inst.consecutive_failures,
    );
  }
}

/// Under-capacity: 45 chain actors plus active genesis work fit both the
/// configurable execution ceiling and an unbounded diagnostic WeightMeter.
/// Dormant and custody-only genesis addresses never compete for scheduler capacity.
///
/// Asserts: exact balance conservation, 100% per-block coverage, zero deferrals,
/// zero failures, uniform cycle_nonce, zero consecutive_failures.
#[test]
fn circular_chain_under_capacity_every_actor_every_block() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 45u64;
    let num_blocks = 50u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (actor_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag = run_blocks_with_diagnostics(
      &actor_ids,
      num_blocks,
      Weight::from_parts(u64::MAX, u64::MAX),
    );
    // Balance conservation (exact: System Actors pay no fees)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Every chain actor must execute exactly once per block
    for &id in &actor_ids {
      let count = diag.actor_cycle_counts[&id];
      assert_eq!(
        count, num_blocks,
        "Actors {} executed {}/{} blocks",
        id, count, num_blocks,
      );
    }
    // Throughput: at least chain_len per block (genesis actors add more)
    assert!(
      diag.min_per_block >= chain_len as u32,
      "Min per-block throughput: expected≥{}, got={}",
      chain_len,
      diag.min_per_block,
    );
    // Fairness: all chain actors must have identical cycle_nonce
    let nonces: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter_map(|&id| Actors::active_actor_view(id).map(|i| i.cycle_nonce))
      .collect();
    let (min_n, max_n) = (*nonces.iter().min().unwrap(), *nonces.iter().max().unwrap());
    assert_eq!(
      min_n, max_n,
      "Fairness: cycle_nonce spread must be 0 (min={}, max={})",
      min_n, max_n
    );
    assert_eq!(
      min_n, num_blocks as u64,
      "cycle_nonce must equal block count"
    );
    assert_core_stability(&actor_ids, &diag);
  });
}

/// Diagnostic test: trace first 5 blocks in detail (execute_cycle only, no emergency)
#[test]
fn diagnose_over_capacity_first_blocks() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (_actor_ids, _sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    println!("\n=== Initial state ===");
    let active_count = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    println!("Active instances len: {}", active_count);
    for block in 2..=6 {
      System::set_block_number(block);
      System::reset_events();
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
      let executions: alloc::vec::Vec<u64> = System::events()
        .iter()
        .filter_map(|evt| {
          if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
            Some(*actor_id)
          } else {
            None
          }
        })
        .collect();
      let min_id = executions.iter().min().copied();
      let max_id = executions.iter().max().copied();
      println!("\n=== Block {} ===", block);
      println!(
        "Executions: {} (IDs: {:?}..{:?})",
        executions.len(),
        min_id,
        max_id
      );
      // Check zero actors (2006-2020)
      let zero_actors: alloc::vec::Vec<u64> = (2006..=2020).collect();
      let zero_executed: alloc::vec::Vec<u64> = executions
        .iter()
        .filter(|id| zero_actors.contains(id))
        .cloned()
        .collect();
      println!(
        "Zero actors (2006-2020) executed: {} {:?}",
        zero_executed.len(),
        zero_executed
      );
    }
    // After 5 blocks, check nonce of zero actors
    println!("\n=== After 5 blocks ===");
    for id in 2006..=2010 {
      if let Some(inst) = Actors::active_actor_view(id) {
        println!(
          "Actors {}: cycle_nonce={}, last_cycle_block={}",
          id,
          inst.cycle_nonce,
          inst
            .last_cycle_block
            .map(|b| b.to_string())
            .unwrap_or_else(|| String::from("None"))
        );
      }
    }
    for id in 2006..=2010 {
      println!(
        "Actors {} present: {}",
        id,
        pallet_deos_actors::ActorHot::<Runtime>::contains_key(id)
      );
    }
  });
}

/// A 100-actor chain remains fair while the configurable count ceiling and
/// WeightMeter independently bound per-block execution.
#[test]
fn circular_chain_respects_execution_ceiling_and_remains_fair() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let num_blocks = 100u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (actor_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag = run_blocks_with_diagnostics(
      &actor_ids,
      num_blocks,
      Weight::from_parts(u64::MAX, u64::MAX),
    );
    // Balance conservation (exact)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Per-block execution cap respected
    let execution_ceiling = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get();
    assert!(
      diag.max_per_block <= execution_ceiling,
      "Per-block throughput must not exceed MaxExecutionsPerBlock={execution_ceiling} (got {})",
      diag.max_per_block,
    );
    assert!(
      diag.min_per_block > 0,
      "Every block must execute at least some actors",
    );
    // No starvation: every chain actor must have executed multiple times
    let min_count = *diag.actor_cycle_counts.values().min().unwrap();
    let zero_actors: alloc::vec::Vec<u64> = diag
      .actor_cycle_counts
      .iter()
      .filter(|(_id, count)| **count == 0)
      .map(|(id, _)| *id)
      .collect();
    assert!(
      min_count > 0,
      "No starvation: every actor must execute at least once (min_count={}, \
       zero_actors={:?}, active_actors_len={})",
      min_count,
      &zero_actors[..zero_actors.len().min(10)],
      pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count(),
    );
    // Fairness: examine cycle_nonce spread across chain actors.
    // With identical periodic actors, the queue scheduler should keep nonce spread minimal (≤ 2).
    let nonces: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter_map(|&id| Actors::active_actor_view(id).map(|i| i.cycle_nonce))
      .collect();
    let min_nonce = *nonces.iter().min().unwrap();
    let max_nonce = *nonces.iter().max().unwrap();
    let nonce_spread = max_nonce - min_nonce;
    assert!(
      nonce_spread <= 2,
      "Fairness: nonce spread {} exceeds 2 (min={}, max={})",
      nonce_spread,
      min_nonce,
      max_nonce,
    );
    // Total throughput: should utilize most available slots
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    let theoretical_max = num_blocks * 48;
    assert!(
      total_executions > theoretical_max * 9 / 10,
      "Total executions {} must exceed 90% of theoretical max {}",
      total_executions,
      theoretical_max,
    );
    assert_core_stability(&actor_ids, &diag);
  });
}

fn clear_genesis_system_actors_for_stress_fixture() {
  let actors: alloc::vec::Vec<_> = pallet_deos_actors::ActorHot::<Runtime>::iter().collect();
  for (actor_id, _hot) in actors {
    pallet_deos_actors::ActorHot::<Runtime>::remove(actor_id);
    pallet_deos_actors::ActorContract::<Runtime>::remove(actor_id);
    pallet_deos_actors::ActorFunding::<Runtime>::remove(actor_id);
    let identity = Actors::actor_identities(actor_id).expect("actor identity exists");
    pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
  }
  let identities: alloc::vec::Vec<_> =
    pallet_deos_actors::ActorIdentities::<Runtime>::iter().collect();
  for (actor_id, identity) in identities {
    pallet_deos_actors::ActorIdentities::<Runtime>::remove(actor_id);
    pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
  }
  // Isolate the synthetic active-capacity profile from retained genesis locators.
  // Production close preserves those locators for deterministic reattachment.
  let _ = pallet_deos_actors::SystemSovereigns::<Runtime>::clear(u32::MAX, None);
  pallet_deos_actors::SystemSovereignCount::<Runtime>::put(0);
  let _ = pallet_deos_actors::WakeupPages::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::WakeupBuckets::<Runtime>::clear(u32::MAX, None);
  let _ = pallet_deos_actors::WakeupCursorPages::<Runtime>::clear(u32::MAX, None);
  pallet_deos_actors::WakeupCursorLen::<Runtime>::put(0);
  let _ = pallet_deos_actors::QueuePages::<Runtime>::clear(u32::MAX, None);
  pallet_deos_actors::QueueHead::<Runtime>::put(0);
  pallet_deos_actors::QueueTail::<Runtime>::put(0);
  pallet_deos_actors::QueueOccupancy::<Runtime>::put(0);
  pallet_deos_actors::NextQueueTicket::<Runtime>::put(0);
  pallet_deos_actors::ActiveActorCount::<Runtime>::put(0);
  pallet_deos_actors::ActorIdentityCount::<Runtime>::put(0);
}

fn close_genesis_system_actors() {
  clear_genesis_system_actors_for_stress_fixture();
}

fn run_fairness_matrix_case(total_actors: u64, num_blocks: u32) -> StressDiagnostics {
  System::set_block_number(1);
  close_genesis_system_actors();
  assert_eq!(
    pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count(),
    0,
    "Genesis actors must be removed for isolated fairness matrix",
  );
  let initial_balance = 10_000u128;
  let actor_ids = setup_inert_actors(total_actors, initial_balance);
  let active_count = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
  assert_eq!(
    active_count, total_actors,
    "Scenario must start with exact actor count (expected={}, got={})",
    total_actors, active_count,
  );
  let diag = run_blocks_with_diagnostics(&actor_ids, num_blocks, Weight::MAX);
  let budget = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() as u64;
  assert!(
    diag.max_per_block as u64 <= budget,
    "Per-block throughput must not exceed MaxExecutionsPerBlock={} (got {})",
    budget,
    diag.max_per_block,
  );
  let min_count = *diag.actor_cycle_counts.values().min().unwrap() as u64;
  let max_count = *diag.actor_cycle_counts.values().max().unwrap() as u64;
  let spread = max_count.saturating_sub(min_count);
  assert!(
    spread <= 4,
    "Fairness: nonce spread {} exceeds 4 (min={}, max={}, actors={}, blocks={})",
    spread,
    min_count,
    max_count,
    total_actors,
    num_blocks,
  );
  // Actual measured throughput, rather than the configured count ceiling, must still
  // cover every actor. The bounded spread assertion above owns relative fairness.
  let total_served: u64 = diag
    .actor_cycle_counts
    .values()
    .map(|&c| u64::from(c))
    .sum();
  assert!(
    total_served >= total_actors,
    "Scenario must serve every actor at least once (actors={}, served={})",
    total_actors,
    total_served,
  );
  let full_rotation_blocks = total_actors.div_ceil(budget);
  assert!(
    num_blocks as u64 >= full_rotation_blocks,
    "Scenario blocks {} must cover at least one full rotation {}",
    num_blocks,
    full_rotation_blocks,
  );
  assert_core_stability(&actor_ids, &diag);
  diag
}

// --- Scheduler Fast FIFO Stress (CI) ---

#[test]
fn scheduler_fast_fifo_dense_vs_sparse_topology_smoke() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 2] = [(64, 96, 8), (256, 128, 16)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert!(
      dense_total.abs_diff(sparse_total) <= 1,
      "Finite-horizon topology throughput may differ by at most one tail admission (actors={}, blocks={}, stride={}, dense={}, sparse={})",
      actors,
      blocks,
      stride,
      dense_total,
      sparse_total,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 3,
      "Dense fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 3,
      "Sparse fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
fn scheduler_fast_fifo_sparse_topology_liveness_smoke() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 256u64;
    let blocks = 192u32;
    let stride = 32u64;
    let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Sparse topology smoke must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

#[test]
fn reference_idle_budget_admits_mixed_tasks_without_starvation() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let mut actor_ids = setup_inert_actors(32, 10_000u128);
    let (transfer_ids, _) = setup_circular_chain(32, 10_000u128);
    actor_ids.extend(transfer_ids);
    let budget =
      <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    let diag = run_blocks_with_diagnostics(&actor_ids, 40, budget);
    let counts: alloc::vec::Vec<u32> = actor_ids
      .iter()
      .map(|id| diag.actor_cycle_counts[id])
      .collect();
    let min_cycles = *counts.iter().min().expect("actors exist");
    let max_cycles = *counts.iter().max().expect("actors exist");
    assert!(min_cycles > 0, "every admitted actor must make progress");
    assert!(
      max_cycles.saturating_sub(min_cycles) <= 1,
      "FIFO carry-over must keep mixed-task nonce spread <= 1: {counts:?}"
    );
    assert_eq!(diag.total_failed_steps, 0);
  });
}

#[test]
fn reference_idle_budget_converges_paged_wakeup_and_pure_close_pressure() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let retry_ids = setup_inert_actors(
      u64::from(<Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get()),
      10_000u128,
    );
    for &actor_id in &retry_ids {
      assert!(Actors::wakeup_substrate_schedule(actor_id, 102));
    }

    let expired_count = <Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get();
    let mut expired_ids = alloc::vec::Vec::new();
    for _ in 0..expired_count {
      expired_ids.push(create_system(
        ALICE,
        manual_schedule(),
        Some(ScheduleWindow { start: 1, end: 101 }),
        BoundedVec::try_from(vec![make_step(inert_task())]).expect("steps fits"),
      ));
    }
    let asset_id = 90u32;
    assert_ok!(create_test_asset(asset_id, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      asset_id,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let close_id = expired_ids[0];
    let close_account = actor_account(close_id);
    assert_ok!(mint_tokens(asset_id, &ALICE, &close_account, 500));
    let budget =
      <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as Get<Weight>>::get();
    for block in 102..=150 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      run_idle(budget);
      let retries_done = retry_ids
        .iter()
        .all(|id| Actors::actor_hot(*id).is_some_and(|hot| hot.wakeup_pointer.is_none()));
      let closes_done = expired_ids
        .iter()
        .all(|id| Actors::active_actor_view(*id).is_none());
      let live_progress = retry_ids
        .iter()
        .all(|id| Actors::active_actor_view(*id).is_some_and(|actor| actor.cycle_nonce > 0));
      if retries_done && closes_done && live_progress {
        break;
      }
    }

    assert!(
      retry_ids
        .iter()
        .all(|id| Actors::actor_hot(*id).is_some_and(|hot| hot.wakeup_pointer.is_none())),
      "paged wakeups must converge"
    );
    assert!(
      retry_ids
        .iter()
        .all(|id| Actors::active_actor_view(*id).is_some_and(|actor| actor.cycle_nonce > 0)),
      "live actors must progress while cleanup converges"
    );
    let repair_batch = BoundedVec::try_from(expired_ids.clone()).expect("repair batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(ALICE),
      repair_batch,
    ));
    assert!(
      expired_ids
        .iter()
        .all(|id| Actors::active_actor_view(*id).is_none()),
      "explicit bounded repair must close externally stranded actors"
    );
    assert_eq!(
      Assets::balance(asset_id, close_account),
      500,
      "pure terminal cleanup must preserve sovereign balances"
    );
  });
}

// --- Scheduler Stress FIFO (scheduled/nightly) ---

#[test]
#[ignore] // Heavy: run in the scheduled nightly FIFO stress job (release mode)
fn scheduler_stress_fifo_over_capacity_fairness_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 418)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let _ = run_fairness_matrix_case(actors, blocks);
    });
  }
}

#[test]
#[ignore] // Heavy topology matrix, run in the scheduled nightly FIFO stress job
fn scheduler_stress_fifo_dense_vs_sparse_topology_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 3] = [(100, 200, 8), (1000, 300, 16), (5000, 420, 32)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert_eq!(
      dense_total, sparse_total,
      "Topology must not change total execution throughput (actors={}, blocks={}, stride={})",
      actors, blocks, stride,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 3,
      "Dense fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 3,
      "Sparse fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
#[ignore] // Heavy long-run sparse-liveness check, run in the scheduled nightly FIFO stress job
fn scheduler_stress_fifo_sparse_topology_long_run_liveness() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 2000u64;
    let blocks = 1024u32;
    let stride = 32u64;
    let actor_ids = setup_inert_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Long-run sparse topology must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Long-run sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

#[test]
#[ignore] // Checkpoint A capacity acceptance; run through scripts/actors-assurance.sh.
fn checkpoint_a_s6_dense_10k_wakeups_converge_without_drops() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actor_count = 10_000u32;
    let wakeup_block = 10;
    let actor_ids = setup_inert_actors(actor_count.into(), 10_000u128);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(actor_ids.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id).is_some_and(|hot| hot.queue_ticket.is_none())
    }));
    for actor_id in &actor_ids {
      assert!(Actors::wakeup_substrate_schedule(*actor_id, wakeup_block));
    }

    let bucket = Actors::wakeup_buckets(wakeup_block).expect("dense wakeup bucket");
    assert_eq!(bucket.live_entries, actor_count);
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(wakeup_block));

    let mut scanned = 0u32;
    let mut passes = 0u32;
    while Actors::wakeup_cursor_len() > 0 {
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      let stats = Actors::drain_overdue_wakeups_cursor(wakeup_block, &mut meter);
      assert!(stats.entries_scanned > 0, "each pass must make progress");
      scanned = scanned.saturating_add(stats.entries_scanned);
      passes = passes.saturating_add(1);
      assert!(passes <= actor_count, "dense drain must remain bounded");
    }

    assert_eq!(scanned, actor_count);
    assert!(Actors::wakeup_buckets(wakeup_block).is_none());
    assert!(actor_ids.iter().all(|actor_id| {
      let hot = Actors::actor_hot(*actor_id).expect("active actor");
      hot.wakeup_pointer.is_none() && hot.queue_ticket.is_some()
    }));
  });
}

#[test]
#[ignore] // Queue/wakeup occupancy diagnostics for over-capacity stress scenario
fn profile_scheduler_queue_wakeup_occupancy_10k() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 10_000u64;
    let blocks = 418u32;
    let actor_ids = setup_inert_actors(actors, 10_000u128);
    let (diag, queue_diag) = run_blocks_with_queue_diagnostics(&actor_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let spread = max_count.saturating_sub(min_count);
    println!(
      "Actors queue profile: actors={}, blocks={}, min_cycle_nonce={}, max_cycle_nonce={}, spread={}, max_queue_occupancy={}, max_wakeup_backlog={}, max_wakeup_buckets={}",
      actors,
      blocks,
      min_count,
      max_count,
      spread,
      queue_diag.max_queue_occupancy,
      queue_diag.max_wakeup_backlog,
      queue_diag.max_wakeup_buckets,
    );
    assert!(min_count > 0, "10k stress profile must remain starvation-free");
    assert!(
      spread <= 4,
      "10k stress profile nonce spread {} exceeds release bound 4 (min={}, max={})",
      spread,
      min_count,
      max_count,
    );
  });
}

// Profiling utility: run manually in release mode for wall-clock matrix
#[test]
#[ignore]
fn profile_scheduler_wallclock_matrix() {
  use super::common::new_test_ext;
  use std::time::Instant;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 418)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let started = Instant::now();
      let diag = run_fairness_matrix_case(actors, blocks);
      let elapsed = started.elapsed();
      let total_executions: u32 = diag.actor_cycle_counts.values().sum();
      let ms_per_block = (elapsed.as_secs_f64() * 1_000.0) / (blocks as f64);
      println!(
        "Actors scheduler profile: actors={}, blocks={}, elapsed_ms={:.3}, ms_per_block={:.4}, total_executions={}",
        actors,
        blocks,
        elapsed.as_secs_f64() * 1_000.0,
        ms_per_block,
        total_executions,
      );
    });
  }
}

#[test]
fn genesis_sparse_id_space_executes_only_active_actors() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // Genesis reserves IDs 0-14 as three active actors, ten dormant identities,
    // and two custody-only accounts. The gap after ID 14 stays empty until a
    // new actor is created.
    //
    // Ringless scheduler iterates ActiveActors BTreeSet directly,
    // so sparse IDs are handled efficiently — no scanning over empty slots.
    //
    // Direct test funding bypasses ingress notification. The three genesis
    // contracts must therefore remain idle while the explicit timer fixture runs.
    assert_eq!(Actors::active_actor_count(), 3);
    assert_eq!(Actors::actor_identity_count(), 13);
    let genesis_ids_all: alloc::vec::Vec<u64> =
      alloc::vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,];
    for &id in &genesis_ids_all {
      let sov = Actors::sovereign_account_id_system(id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    }
    // Dormant and custody identities own no executable contract.
    for id in [2, 4, 5, 6, 7, 8, 9, 11, 13, 14] {
      assert!(Actors::actor_identities(id).is_some());
      assert!(Actors::active_actor_view(id).is_none());
    }
    for id in [3, 12] {
      assert!(Actors::actor_identities(id).is_none());
      assert!(Actors::active_actor_view(id).is_none());
    }
    // Create a fresh actor at the current high end to extend the sparse space.
    let fresh_id = crate::Actors::next_actor_id();
    assert_eq!(fresh_id, 15);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    let sov_fresh = Actors::sovereign_account_id_system(fresh_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_fresh, initial_balance);
    let all_ids: alloc::vec::Vec<u64> = alloc::vec![fresh_id];
    // Block 2: only the explicit timer fixture fires.
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    let executed_block_2: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    for &id in &all_ids {
      assert!(
        executed_block_2.contains(&id),
        "Actors {} must execute in first block despite sparse ID gaps \
         (total_actors={}, id_space=0..{}, executed={:?})",
        id,
        all_ids.len(),
        crate::Actors::next_actor_id(),
        executed_block_2,
      );
    }
    for id in [0, 1, 10] {
      assert!(!executed_block_2.contains(&id));
    }
    // The fresh timer actor continues without causing work for ingress-driven
    // genesis contracts. Advance to block 13 to verify sparse-ID stability.
    let block = 13u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    let executed_block_13: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &evt.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    assert_eq!(executed_block_13, all_ids);
  });
}

#[test]
fn execution_order_lower_id_executes_before_higher_id() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // Actors-A (lower ID): transfers 10% of current NTVE to Actors-B sovereign
    let actor_a_id = crate::Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_a_id);
    let sov_a = Actors::sovereign_account_id_system(actor_a_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_a, initial_balance);
    // Actors-B (higher ID): transfers 10% of current NTVE to CHARLIE
    let actor_b_id = crate::Actors::next_actor_id();
    assert!(actor_b_id > actor_a_id, "B must have higher ID than A");
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      inert_timer_contract(),
    ));
    age_fixture_control_clock(actor_b_id);
    let sov_b = Actors::sovereign_account_id_system(actor_b_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_b, initial_balance);
    // Update Actors-A steps: Transfer 10% NTVE → Actors-B sovereign
    let pct = Perbill::from_percent(10);
    let execution_plan_a: ExecutionPlanOf<Runtime> = alloc::vec![pallet_deos_actors::Step {
      preconditions: Default::default(),
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: sov_b.clone(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_a_id,
      (execution_plan_a, CompletionPolicy::Persistent,)
    ));
    // Update Actors-B steps: Transfer 10% NTVE → CHARLIE
    let execution_plan_b: ExecutionPlanOf<Runtime> = alloc::vec![pallet_deos_actors::Step {
      preconditions: Default::default(),
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: CHARLIE,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_b_id,
      (execution_plan_b, CompletionPolicy::Persistent,)
    ));
    let charlie_before = Balances::free_balance(CHARLIE);
    // Run one block
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    Actors::on_initialize(block);
    Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    // If A executed before B: A transferred 10% to B, then B has initial + A's transfer,
    // and B transfers 10% of that total to CHARLIE.
    // If B executed before A: B transfers 10% of initial only, then A transfers to B.
    // We can distinguish by checking CHARLIE's balance.
    let minimum = crate::EXISTENTIAL_DEPOSIT;
    let a_transfer = pct.mul_floor(initial_balance.saturating_sub(minimum));
    let b_balance_after_a = initial_balance + a_transfer;
    let b_transfer_correct_order = pct.mul_floor(b_balance_after_a.saturating_sub(minimum));
    let b_transfer_wrong_order = pct.mul_floor(initial_balance.saturating_sub(minimum));
    let charlie_after = Balances::free_balance(CHARLIE);
    let charlie_received = charlie_after.saturating_sub(charlie_before);
    assert_eq!(
      charlie_received, b_transfer_correct_order,
      "Actors-A (id={}) must execute before Actors-B (id={}): \
       correct_order_transfer={}, wrong_order_transfer={}, actual={}",
      actor_a_id, actor_b_id, b_transfer_correct_order, b_transfer_wrong_order, charlie_received,
    );
    assert_ne!(
      b_transfer_correct_order, b_transfer_wrong_order,
      "Test must distinguish between execution orders"
    );
  });
}

// --- 10K Actors Stress Test ---

/// Validates the queue scheduler at production scale (10,000 active actors).
///
/// Runtime starts with genesis System Actors already occupying part of the active set.
/// This test fills the remaining capacity so ActiveActors reaches exactly 10,000,
/// then verifies starvation-freedom and fairness for newly added stress actors.
///
/// The configured execution ceiling and FIFO size determine the count-limited
/// rotation horizon; WeightMeter remains an independent limiter under finite budgets.
/// Nonce spread (max - min) must remain ≤ 3 for near-perfect fairness.
///
/// Acceptance criteria:
/// - ActiveActors reaches exactly 10,000
/// - Every stress actor executes at least once
/// - Nonce spread ≤ 2
/// - Zero deferrals (System Actors, Weight::MAX budget)
/// - Zero failed steps
#[test]
fn runtime_simulation_core_rolls_back_deos_adapter_effects() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = transfer_execution_plan(BOB, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT);
    let expected_contract = system_active_contract(manual_schedule(), None, steps.clone());
    let actor_id = create_system(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000 * crate::EXISTENTIAL_DEPOSIT);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let actor_balance_before = Balances::free_balance(&actor_before.sovereign_account);
    let bob_before = Balances::free_balance(BOB);
    let events_before = System::event_count();

    let result = Actors::simulate_current_contract(
      actor_id,
      pallet_deos_actors::ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready DEOS actor simulates");

    assert_eq!(result.status, SimulationStatus::Completed);
    assert_eq!(result.cycle_nonce, 1);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].outcome, SimulationStepOutcome::Executed);
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(Balances::free_balance(BOB), bob_before);
    assert_eq!(
      Balances::free_balance(&actor_account(actor_id)),
      actor_balance_before
    );
    assert_eq!(System::event_count(), events_before);
  });
}

#[test]
#[ignore] // ~30s wall-clock; run manually: cargo test --release stress_10k_actors_queue_scheduler -- --ignored
fn stress_10k_actors_queue_scheduler() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let num_blocks = 500u32;
    let initial_balance: u128 = 1_000 * crate::EXISTENTIAL_DEPOSIT;
    let max_active = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get() as u64;
    // Retain paused active genesis actors to validate mixed ready/non-ready fairness.
    // Remove dormant genesis identities so the identity cap does not prevent saturating
    // the independently asserted active-actor cap.
    let genesis_ids: alloc::vec::Vec<u64> = alloc::vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    for &id in &genesis_ids {
      let _ = Actors::pause_actor(RuntimeOrigin::root(), id);
    }
    let dormant: alloc::vec::Vec<_> = pallet_deos_actors::ActorIdentities::<Runtime>::iter()
      .filter(|(actor_id, _)| !pallet_deos_actors::ActorHot::<Runtime>::contains_key(actor_id))
      .collect();
    for (actor_id, identity) in &dormant {
      pallet_deos_actors::ActorIdentities::<Runtime>::remove(actor_id);
      pallet_deos_actors::SovereignIndex::<Runtime>::remove(&identity.sovereign_account);
    }
    pallet_deos_actors::ActorIdentityCount::<Runtime>::mutate(|count| {
      *count = count.saturating_sub(dormant.len() as u32);
    });
    let active_before = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
    assert!(
      active_before < max_active,
      "Test precondition failed: active_before={} must be < max_active={}",
      active_before,
      max_active,
    );
    let actor_count = max_active - active_before;
    let actor_ids = setup_mixed_inert_actors(actor_count, initial_balance);
    assert_eq!(actor_ids.len(), actor_count as usize);
    let active_after = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count() as u64;
    assert_eq!(
      active_after, max_active,
      "ActiveActors must be saturated to max capacity",
    );
    let diag = run_blocks_with_diagnostics(&actor_ids, num_blocks, Weight::MAX);
    // All stress actors must execute at least once
    let zero_actors: alloc::vec::Vec<u64> = actor_ids
      .iter()
      .filter(|&&id| *diag.actor_cycle_counts.get(&id).unwrap_or(&0) == 0)
      .copied()
      .collect();
    assert!(
      zero_actors.is_empty(),
      "Starvation: {} stress actors never executed (first 10: {:?})",
      zero_actors.len(),
      &zero_actors[..zero_actors.len().min(10)],
    );
    // Fairness: nonce spread ≤ 3
    let nonces: alloc::vec::Vec<u32> = actor_ids
      .iter()
      .map(|&id| *diag.actor_cycle_counts.get(&id).unwrap_or(&0))
      .collect();
    let min_nonce = *nonces.iter().min().unwrap();
    let max_nonce = *nonces.iter().max().unwrap();
    let nonce_spread = max_nonce - min_nonce;
    assert!(
      nonce_spread <= 3,
      "Fairness: nonce spread {} exceeds 3 (min={}, max={})",
      nonce_spread,
      min_nonce,
      max_nonce,
    );
    // MaxExecutionsPerBlock is a count ceiling, not a throughput promise. The measured
    // two-dimensional Weight envelope controls actual service; saturation evidence therefore
    // requires bounded execution, complete first traversal, and fairness rather than utilization
    // against an unreachable count-only maximum.
    let execution_ceiling = <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get();
    assert!(
      diag.max_per_block <= execution_ceiling,
      "Per-block executions {} exceeds MaxExecutionsPerBlock={execution_ceiling}",
      diag.max_per_block,
    );
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    assert!(
      u64::from(total_executions) >= actor_count,
      "Total executions {total_executions} must cover the complete {actor_count}-actor traversal",
    );
    assert_core_stability(&actor_ids, &diag);
  });
}

#[test]
fn dust_attack_min_balance_actors_preserve_scheduler_stability() {
  seeded_test_ext().execute_with(|| {
    let min_balance = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
    let actor_count = 96u32;
    let baseline_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    let mut actor_ids = Vec::new();
    for i in 0..actor_count {
      let mut owner_bytes = [0u8; 32];
      owner_bytes[0] = (i & 0xFF) as u8;
      owner_bytes[31] = ((i + 17) & 0xFF) as u8;
      let owner = crate::AccountId::from(owner_bytes);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &owner,
        min_balance.saturating_mul(20),
      );
      let schedule = Schedule {
        trigger: Trigger::cadenced_always(1),
        cooldown_blocks: 0,
      };
      let actor_id = create_user(
        owner.clone(),
        schedule,
        None,
        transfer_execution_plan(owner, AssetKind::Native, 1),
      );
      let sovereign = actor_account(actor_id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &sovereign,
        min_balance.saturating_mul(10),
      );
      actor_ids.push(actor_id);
    }
    let initial_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    assert_eq!(initial_active, baseline_active + actor_count as usize);
    for block in 1..=32u32 {
      System::set_block_number(block);
      Actors::on_idle(block, Weight::MAX);
    }
    let final_active = pallet_deos_actors::ActorHot::<Runtime>::iter_keys().count();
    let progressed = actor_ids
      .iter()
      .filter(|id| {
        Actors::active_actor_view(**id)
          .map(|inst| inst.cycle_nonce > 0)
          .unwrap_or(true)
      })
      .count();
    assert!(
      progressed > 0,
      "Scheduler should execute or terminally close at least some dust actors"
    );
    assert!(
      final_active > 0,
      "Dust load must not collapse scheduler to zero active actors"
    );
    assert!(
      final_active <= initial_active,
      "Active actors cannot increase without new creations"
    );
  });
}

#[test]
fn fee_ingress_accumulates_exactly_amount_never_double() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    // A Mutable System actor with an accepting funding policy and a
    // Native-tracking plan: exactly one certified ingress notification must
    // accumulate exactly `amount`, never `2 * amount` from a duplicate
    // submission of the same movement.
    let tracking_plan =
      pallet_deos_actors::ExecutionPlanOf::<Runtime>::try_from(vec![pallet_deos_actors::Step {
        preconditions: Default::default(),
        task: pallet_deos_actors::Task::Transfer {
          to: BOB,
          asset: AssetKind::Native,
          amount: pallet_deos_actors::AmountResolution::PercentageOfLastFunding(
            polkadot_sdk::sp_runtime::Perbill::from_percent(100),
          ),
        },
        on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
      }])
      .expect("tracking plan fits");
    let actor_id = create_system(ALICE, manual_schedule(), None, tracking_plan);
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let amount = crate::EXISTENTIAL_DEPOSIT.saturating_mul(3);
    let payer = BOB;
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_ok!(<Balances as Currency<crate::AccountId>>::transfer(
      &payer,
      &instance.sovereign_account,
      amount,
      polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
    ));
    // One certified notification (the exact ingress the FeeCollector emits).
    assert_ok!(Actors::notify_address_event(
      actor_id,
      AssetKind::Native,
      amount,
      &payer,
    ));
    let funding = actor_funding(actor_id);
    let accumulated = funding
      .funding_accumulated
      .iter()
      .find(|(asset, _)| **asset == AssetKind::Native)
      .map(|(_, v)| *v)
      .unwrap_or(0);
    assert_eq!(
      accumulated, amount,
      "one certified ingress must accumulate exactly amount, never 2 * amount"
    );
  });
}

#[test]
fn fee_collector_one_charge_creates_one_placement_attempt() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let payer = BOB;
    assert_ok!(TmctolFeeCollector::collect_fee(
      &payer,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));
    let hot = Actors::actor_hot(fee_sink_id).expect("Fee Sink hot state");
    // Signal coalescing stays unchanged: one charge latches readiness once, so
    // the actor owns exactly one live queue ticket or wakeup pointer, not two.
    let membership = u8::from(hot.queue_ticket.is_some()) + u8::from(hot.wakeup_pointer.is_some());
    assert!(
      membership == 1,
      "one charge creates exactly one placement path, got membership={membership}"
    );
  });
}

#[test]
fn fee_collector_noop_zero_emits_no_ingress() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = crate::Actors::sovereign_account_id_system(fee_sink_id);
    let events_before = System::event_count();
    assert_ok!(TmctolFeeCollector::collect_fee(
      &BOB,
      &fee_sink,
      AssetKind::Native,
      0,
    ));
    assert_eq!(
      System::event_count(),
      events_before,
      "zero/no-op collection must emit no ingress events"
    );
    let hot = Actors::actor_hot(fee_sink_id);
    assert!(
      hot.is_none_or(|hot| hot.queue_ticket.is_none() && hot.wakeup_pointer.is_none()),
      "zero collection must not latch readiness"
    );
  });
}

#[test]
fn eligibility_projection_binds_genesis_actors_and_signal_readiness() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;

    let missing = Actors::actor_eligibility(primitives::ecosystem::actor_ids::BURN_ACTOR_ID + 1000)
      .expect("projection computes");
    assert_eq!(
      missing.phase,
      pallet_deos_actors::ActorEligibilityPhase::NotRegistered
    );
    assert_eq!(missing.next_eligible_block, None);

    let idle = Actors::actor_eligibility(fee_sink_id).expect("projection computes");
    assert_eq!(
      idle.phase,
      pallet_deos_actors::ActorEligibilityPhase::WaitingSignal
    );
    assert_eq!(idle.next_eligible_block, Some(1));

    fund_native_via_call(BOB, fee_sink_id, 1_000);
    let latched = Actors::actor_eligibility(fee_sink_id).expect("projection computes");
    assert_eq!(
      latched.phase,
      pallet_deos_actors::ActorEligibilityPhase::Ready
    );
    assert_eq!(latched.next_eligible_block, Some(1));
  });
}
