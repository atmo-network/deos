mod actor_retention_corpus;

use actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest, fixed_pages};
use codec::{Decode, Encode, MaxEncodedLen};
use deos_runtime::{
  AccountId, Actors, Address, Balance, Nonce, Runtime, RuntimeCall, Signature, TransactionPayment,
  TxExtension, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorAdmissionCertificateOf, ActorContract, ActorContractHeadOf, ActorControlHotState,
  ActorControlIdentity, ActorIdentityOf, ActorRunStateOf, ActorStepChunkOf,
  ActorStepResourceEnvelope, AmountResolution, Config, ContractSteps, FundingSourcePolicy,
  MAX_STEPS_PER_TAIL_CHUNK, OnUnfunded, ParkedBalanceActivationOf, ParkedBalanceRule, Step,
  StepErrorPolicy, Task, TaskEffectWeightProvider, Trigger, TriggerWakeupPointer, WeightInfo,
};
use polkadot_sdk::{
  frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo, Pays},
    traits::Get,
  },
  frame_system, pallet_transaction_payment,
  sp_core::{U512, sr25519},
  sp_io::TestExternalities,
  sp_runtime::{generic, traits::Convert},
  sp_weights::WeightToFee,
};
use primitives::AssetKind;

const SPACER_BLOCKS: u64 = 4;

fn encoded_transaction(call: RuntimeCall) -> UncheckedExtrinsic {
  // The other retained extensions encode unit values. Decode through the public complete
  // type, consuming exactly its bytes without exporting runtime-private host adapters.
  let encoded_extension = (
    frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    frame_system::CheckNonce::<Runtime>::from(Nonce::MAX - 1),
    pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  )
    .encode();
  let mut extension_input = &encoded_extension[..];
  let extension = TxExtension::decode(&mut extension_input).unwrap();
  assert!(extension_input.is_empty());
  // Codec-only: fixed-width signature bytes are never authenticated or dispatched.
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(AccountId::new([1; 32])),
    Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
    extension,
  )
}

fn transaction_fee(call: RuntimeCall) -> (u32, Balance, Balance) {
  let transaction = encoded_transaction(call);
  let bytes = u32::try_from(transaction.encoded_size()).unwrap();
  let info = transaction.get_dispatch_info();
  assert_eq!(info.pays_fee, Pays::Yes);
  let details = TransactionPayment::compute_fee_details(bytes, &info, 0);
  let total = details.final_fee();
  (bytes, details.inclusion_fee.unwrap().len_fee, total)
}

fn signed_caller_inputs() {
  let owner = AccountId::new([1; 32]);
  let assets =
    std::array::from_fn::<_, 4, _>(|index| primitives::assets::TYPE_LP | (index as u32 + 1));
  let parked = |steps| ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![
      Step {
        precondition: None,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      };
      steps
    ]
    .try_into()
    .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        assets
          .map(|asset| ParkedBalanceRule {
            asset: AssetKind::Local(asset),
            authored_min_delta: 100,
          })
          .to_vec(),
      )
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  let transfer = |value| {
    RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
      dest: Address::Id(owner.clone()),
      value,
    })
  };
  let create = |slot, contract| {
    RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor_at_slot {
      owner_slot: slot,
      mutability: pallet_deos_actors::Mutability::Mutable,
      contract,
    })
  };
  let mut producers = Vec::new();
  for (index, asset) in assets.into_iter().enumerate() {
    producers.push(transfer(320_000 * deos_runtime::UNIT));
    producers.push(RuntimeCall::Assets(
      polkadot_sdk::pallet_assets::Call::transfer {
        id: asset,
        target: Address::Id(owner.clone()),
        amount: 10_000_000_000,
      },
    ));
    producers.push(create(
      index as u8 + 1,
      Some(ActorContract {
        trigger: Trigger::cadenced(1),
        cooldown_blocks: 0,
        window: None,
        steps: vec![Step {
          precondition: None,
          task: Task::Transfer {
            to: owner.clone(),
            asset: AssetKind::Local(asset),
            amount: AmountResolution::Fixed(1),
          },
          on_error: StepErrorPolicy::AbortCycle,
        }]
        .try_into()
        .unwrap(),
        on_unfunded: OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        funding: FundingSourcePolicy::OwnerOnly,
        auto_close_at_cycle_nonce: None,
      }),
    ));
  }
  let cases = [
    (
      "donors",
      (0..7)
        .map(|index| transfer(if index == 6 { 200_000 } else { 900_000 } * deos_runtime::UNIT))
        .collect(),
    ),
    (
      "creatorFunding",
      vec![transfer(30_000 * deos_runtime::UNIT); 40],
    ),
    (
      "pool",
      vec![
        RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
          asset_a: AssetKind::Native,
          asset_b: AssetKind::Local(assets[0]),
        }),
        RuntimeCall::AssetConversion(polkadot_sdk::pallet_asset_conversion::Call::add_liquidity {
          asset1: Box::new(AssetKind::Native),
          asset2: Box::new(AssetKind::Local(assets[0])),
          amount1_desired: deos_runtime::UNIT,
          amount2_desired: deos_runtime::UNIT / 4,
          amount1_min: 1,
          amount2_min: 1,
          mint_to: owner.clone(),
        }),
      ],
    ),
    (
      "firstParkedBirth",
      vec![create(
        0,
        Some(parked(<Runtime as Config>::MaxContractSteps::get() as usize)),
      )],
    ),
    ("producers", producers),
    (
      "dormantIdentities",
      vec![create(253, None), create(254, None)],
    ),
    (
      "pairedPermanentClose",
      vec![
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: 15 }),
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: 22 }),
      ],
    ),
    (
      "sourceRecreation",
      vec![RuntimeCall::Actors(
        pallet_deos_actors::Call::activate_actor {
          actor_id: 20,
          contract: parked(1),
        },
      )],
    ),
    (
      "pairedTemporaryClose",
      vec![
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: 20 }),
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: 21 }),
      ],
    ),
    (
      "pairedEpochMutations",
      vec![
        RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor { actor_id: 20 }),
        RuntimeCall::Actors(pallet_deos_actors::Call::activate_actor {
          actor_id: 21,
          contract: parked(1),
        }),
      ],
    ),
  ];
  let pair =
    |weight: polkadot_sdk::frame_support::weights::Weight| [weight.ref_time(), weight.proof_size()];
  let block_weights = <Runtime as frame_system::Config>::BlockWeights::get();
  let mut rows = serde_json::Map::new();
  for (name, calls) in cases {
    let mut total = polkadot_sdk::frame_support::weights::Weight::zero();
    let mut charged = polkadot_sdk::frame_support::weights::Weight::zero();
    let mut bytes = 0;
    let mut inputs = Vec::new();
    for call in calls {
      let transaction = encoded_transaction(call);
      let info = transaction.get_dispatch_info();
      assert_eq!(info.class, DispatchClass::Normal);
      assert_eq!(info.pays_fee, Pays::Yes);
      total = total.checked_add(&info.total_weight()).unwrap();
      let length = transaction.encoded_size();
      let charge = frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
        &block_weights,
        &info,
        length,
      );
      charged = charged.checked_add(&charge).unwrap();
      bytes += length;
      inputs.push(serde_json::json!({"call": pair(info.call_weight), "extension": pair(info.extension_weight), "total": pair(info.total_weight()), "length": length, "frameCharge": pair(charge)}));
    }
    rows.insert(
      name.to_owned(),
      serde_json::json!({"calls": inputs, "declaredTotal": pair(total), "lengthTotal": bytes, "frameUserCharge": pair(charged)}),
    );
  }
  let budget = <Runtime as Config>::BlockResourceBudget::get();
  let limits = budget.limits();
  println!(
    "{}",
    serde_json::json!({
      "scope": "Current native complete DispatchInfo and codec inputs for fixed setup/reclamation call shapes; maximum nonce width, fixed-width representative addresses/signatures; no authentication, dispatch, state proof or physical Weight qualification",
      "cases": rows,
      "configuredBudget": {
        "maximumBlock": pair(budget.maximum_block()),
        "fixedEnvelope": pair(budget.fixed_envelope()),
        "actorControl": pair(limits.actor_control()),
        "sharedEconomic": pair(limits.shared_economic()),
        "actorBaseTurn": pair(limits.actor_base_turn()),
        "userBaseTurn": pair(limits.user_base_turn()),
        "normalMaximumExtrinsic": block_weights.get(DispatchClass::Normal).max_extrinsic.map(pair),
      },
    })
  );
}

fn state_hold_upper() -> Balance {
  type BlockNumber = deos_runtime::BlockNumber;
  let identity = u64::max_encoded_len()
    + ActorIdentityOf::<Runtime>::max_encoded_len()
      .max(ActorControlIdentity::<AccountId, BlockNumber>::max_encoded_len())
    + AccountId::max_encoded_len();
  let head = ActorControlHotState::<BlockNumber>::max_encoded_len()
    + u32::max_encoded_len()
    + Option::<BlockNumber>::max_encoded_len()
    + ActorStepResourceEnvelope::max_encoded_len()
    + ActorAdmissionCertificateOf::<Runtime>::max_encoded_len()
    + ActorContractHeadOf::<Runtime>::max_encoded_len();
  let tails =
    (<Runtime as Config>::MaxContractSteps::get() - 1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK) as usize;
  let components = [
    identity,
    head,
    tails * ActorStepChunkOf::<Runtime>::max_encoded_len(),
    TriggerWakeupPointer::max_encoded_len(),
    ActorRunStateOf::<Runtime>::max_encoded_len(),
  ];
  let upper = components.into_iter().fold(0, |total, bytes| {
    total
      + <Runtime as Config>::ActorStateHoldBase::get()
      + bytes as u128 * <Runtime as Config>::ActorStateHoldPerByte::get()
  });
  println!("no_policy_hold_component_byte_uppers={components:?} per_user_hold_upper={upper}");
  upper
}

fn qualify_fee_spacers() {
  use pallet_transaction_payment::MultiplierUpdate;
  type Update = <Runtime as pallet_transaction_payment::Config>::FeeMultiplierUpdate;
  let weights = <Runtime as frame_system::Config>::BlockWeights::get();
  let normal = weights.get(DispatchClass::Normal).max_total.unwrap();
  let one = pallet_transaction_payment::Multiplier::from_u32(1);
  let minimum = Update::min();
  assert!(minimum < one);
  assert_eq!(
    Update::target(),
    polkadot_sdk::sp_runtime::Perquintill::from_percent(25)
  );
  assert_eq!(normal.ref_time() % 4, 0);
  assert_eq!(normal.proof_size() % 4, 0);
  deos_runtime::System::set_block_consumed_resources(normal, 0);
  let full = Update::convert(one);
  deos_runtime::System::set_block_consumed_resources(Default::default(), 0);
  let quiet = Update::convert(one);
  assert!(full > one && quiet < one);
  let accuracy = U512::from(one.into_inner());
  let increase = U512::from(full.into_inner() - one.into_inner());
  let decrease = U512::from(one.into_inner() - quiet.into_inner());
  let quiet_factor = accuracy - decrease;
  let numerator = (accuracy + increase) * quiet_factor * quiet_factor * quiet_factor * quiet_factor;
  let denominator = accuracy * accuracy * accuracy * accuracy * accuracy;
  // Positive fixed-point rounding can only reduce the increase; each quiet subtraction
  // adds less than one raw unit. Clamping returns the minimum, already below the start.
  // This exact rational margin covers all start multipliers in [minimum, one].
  assert!(numerator < denominator);
  assert!(
    U512::from(minimum.into_inner()) * (denominator - numerator)
      > U512::from(SPACER_BLOCKS) * denominator
  );
  for start in [minimum, one] {
    deos_runtime::System::set_block_consumed_resources(normal, 0);
    let mut current = Update::convert(start);
    for _ in 0..SPACER_BLOCKS {
      deos_runtime::System::set_block_consumed_resources(Default::default(), 0);
      current = Update::convert(current);
    }
    assert!(current <= start);
    println!(
      "spacer_start={} spacer_end={} quiet_blocks={SPACER_BLOCKS}",
      start.into_inner(),
      current.into_inner()
    );
  }
  println!(
    "normal_max_ref_time={} normal_max_proof_size={} full_increase_raw={} quiet_decrease_raw={}",
    normal.ref_time(),
    normal.proof_size(),
    full.into_inner() - one.into_inner(),
    one.into_inner() - quiet.into_inner()
  );
}

fn signed_fee_upper() -> Balance {
  let weights = <Runtime as frame_system::Config>::BlockWeights::get();
  let length = <Runtime as frame_system::Config>::BlockLength::get();
  assert_eq!(
    weights.get(DispatchClass::Normal).base_extrinsic,
    weights.get(DispatchClass::Operational).base_extrinsic
  );
  let info = polkadot_sdk::frame_support::dispatch::DispatchInfo {
    call_weight: weights.max_block,
    extension_weight: Default::default(),
    class: DispatchClass::Normal,
    pays_fee: Pays::Yes,
  };
  TransactionPayment::compute_fee(*length.max.get(DispatchClass::Operational), &info, 0)
}

fn qualify_scalar_horizon(epochs: u64, producer_span: u64, end_block: u64, next_actor_id: u64) {
  use pallet_deos_actors::{
    first_cadence_due_tick, next_actor_generation, next_cadence_due_tick, scheduler_tick_ceil,
    scheduler_tick_floor,
  };
  // The recipe creates identities once. Dormant reactivation increments generation;
  // deactivation preserves it. No Contract replacement or recertification is admitted here.
  assert!(next_actor_id.checked_add(1).is_some());
  let temporary_generation_upper = epochs.checked_add(1).unwrap();
  assert!(next_actor_generation(temporary_generation_upper).is_some());
  // Q1 admits at most one single-Step producer Cycle per block. Its LP Transfer
  // publishes once; no other flow in the declared recipe mutates those four assets.
  let producer_cycle_nonce_upper = producer_span;
  let watched_source_revision_upper = producer_span;
  assert!(producer_cycle_nonce_upper.checked_add(1).is_some());
  assert!(watched_source_revision_upper.checked_add(1).is_some());
  let block = deos_runtime::BlockNumber::try_from(end_block).unwrap();
  assert!(block.checked_add(1).is_some());
  assert!(
    block
      .checked_add(<Runtime as Config>::MaxExecutionDelayBlocks::get())
      .is_some()
  );
  // These are the complete native driver's Timestamp and relay-slot formulas.
  let timestamp = end_block.checked_mul(deos_runtime::SLOT_DURATION).unwrap();
  let tick_millis = <Runtime as Config>::CadenceTickMillis::get();
  let tick = scheduler_tick_floor(timestamp, tick_millis).unwrap();
  let ceil_tick = scheduler_tick_ceil(timestamp, tick_millis).unwrap();
  assert_eq!(tick, ceil_tick);
  let max_delay = <Runtime as Config>::MaxTemporalDelayTicks::get();
  let latest_due = first_cadence_due_tick(timestamp, tick_millis, max_delay).unwrap();
  assert_eq!(latest_due, tick.checked_add(max_delay).unwrap());
  assert!(latest_due.checked_mul(tick_millis).is_some());
  // One-tick producers and the fixed Fee Sink cadence coalesce missed periods.
  for period in [1, 120, max_delay] {
    for anchor in [0, tick.checked_sub(1).unwrap(), tick] {
      let due = next_cadence_due_tick(anchor, period, tick).unwrap();
      assert!(tick < due && due <= latest_due);
      assert!(due.checked_mul(tick_millis).is_some());
    }
  }
  println!(
    "scope=conditional_recipe_scalar_horizon next_actor_id_upper={next_actor_id} temporary_generation_upper={temporary_generation_upper} producer_cycle_nonce_upper={producer_cycle_nonce_upper} watched_source_revision_upper={watched_source_revision_upper} timestamp_millis={timestamp} relay_slot_upper={end_block} scheduler_tick={tick} maximum_delay_due_tick={latest_due}"
  );
}

fn qualify_session_horizon(end_block: u64) {
  use pallet_session_rotation::SessionRotation;
  use polkadot_sdk::frame_support::traits::EstimateNextSessionRotation;

  // Complete recipe blocks start at one; only the periodic rotation hook advances
  // the genesis-zero Session index. The queued callback uses the next index too.
  let period = u64::from(
    <<Runtime as polkadot_sdk::pallet_session::Config>::NextSessionRotation as
      EstimateNextSessionRotation<deos_runtime::BlockNumber>>::average_session_length(),
  );
  assert!(period > 0);
  type Rotation = <Runtime as pallet_session_rotation::Config>::SessionRotation;
  assert!(Rotation::should_rotate(0));
  assert!(!Rotation::should_rotate(u32::try_from(period - 1).unwrap()));
  assert!(Rotation::should_rotate(u32::try_from(period).unwrap()));
  let session_index_upper = u32::try_from(end_block / period).unwrap();
  let callback_index_upper = session_index_upper.checked_add(1).unwrap();
  let last_rotation_block = u64::from(session_index_upper).checked_mul(period).unwrap();
  let next_rotation_block = u64::from(callback_index_upper).checked_mul(period).unwrap();
  assert!(last_rotation_block <= end_block && end_block < next_rotation_block);
  assert!(u32::try_from(next_rotation_block).is_ok());
  println!(
    "scope=conditional_recipe_session_horizon session_period_blocks={period} session_index_upper={session_index_upper} session_callback_index_upper={callback_index_upper} last_rotation_block={last_rotation_block} next_rotation_block={next_rotation_block}"
  );
}

fn qualify_quiet_governance_horizon(end_block: u64) {
  // After all service buckets drain, the sole cursor advances by one per block.
  // No pending/member producer remains in the declared construction.
  let maximum_catch_up = <Runtime as pallet_governance::Config>::MaxEpochCatchUpPerBlock::get();
  assert_eq!(maximum_catch_up, 1);
  let terminal: <Runtime as pallet_governance::Config>::Epoch = u32::try_from(end_block).unwrap();
  let previous = terminal.checked_sub(1).unwrap();
  assert_eq!(previous.checked_add(maximum_catch_up).unwrap(), terminal);
  assert!(terminal.checked_add(1).is_some());
  println!(
    "scope=conditional_quiet_governance_horizon governance_epoch_upper={terminal} governance_catch_up_per_block={maximum_catch_up}"
  );
}

fn qualify_namespace_horizon(end_block: u64) {
  // Registration preparation resolves these four assets once. Balance publication
  // only looks up an existing source; it never invokes the allocator.
  let source_allocator_upper = WATCHES as u64;
  assert!(source_allocator_upper.checked_add(1).is_some());
  let active_bound = u64::from(<Runtime as Config>::MaxActiveActors::get());
  // At most one Q1 successor per current identity, two clock quanta, one scan
  // quantum and four setup publication calls (two in repeated phases). A successor has one
  // primary and one independent Trigger deadline. Failed transitions roll back
  // their namespace increments.
  let deadline_insertions_per_block = active_bound
    .checked_add(2 + 1 + WATCHES as u64)
    .and_then(|owners| owners.checked_mul(2))
    .unwrap();
  let deadline_page_allocator_upper = end_block
    .checked_mul(deadline_insertions_per_block)
    .and_then(|count| count.checked_add(2 * active_bound))
    .unwrap();
  assert!(deadline_page_allocator_upper.checked_add(1).is_some());
  let deadline_live_page_bound = Actors::deadline_destination_page_bound();
  assert_eq!(
    u64::from(deadline_live_page_bound),
    active_bound.div_ceil(32)
  );
  assert!(
    u32::try_from(active_bound)
      .unwrap()
      .checked_add(1)
      .is_some()
  );
  println!(
    "scope=conditional_recipe_namespace_horizon source_allocator_upper={source_allocator_upper} deadline_insertions_per_block={deadline_insertions_per_block} deadline_page_allocator_upper={deadline_page_allocator_upper} deadline_bucket_live_page_bound={deadline_live_page_bound} deadline_clock_live_key_bound={active_bound}"
  );
  let block_keys = PAGES as u64 + 1;
  let tick_members = WATCHES as u64 + 1;
  // Old Manual Park reviews are due no later than now. Only one due-review
  // rearm and two repeated-phase publications can enter the fresh B+1 bucket.
  let block_bucket_members = 1u64 + 2;
  assert!(block_keys < active_bound && tick_members < active_bound);
  assert!(block_bucket_members < 32 && tick_members < 32);
  println!(
    "scope=conditional_recipe_deadline_destinations block_clock_key_upper={block_keys} tick_clock_member_upper={tick_members} block_bucket_member_upper={block_bucket_members} retained_pages_per_bucket=1 maximum_deadline_page_id=0"
  );
}

fn qualify_capacity_arithmetic(
  pages: &[Vec<u64>],
  epochs: u64,
  producer_span: u64,
  other_signed_calls: u64,
) {
  // The native recipe reserves Alice's four producers and one temporary identity,
  // and Bob's other temporary identity, before assigning permanent cohorts.
  let mut owner_identities = [0u32; 40];
  owner_identities[0] = WATCHES as u32 + 1;
  owner_identities[1] = 1;
  for _ in 0..PAGES {
    let owner = owner_identities
      .iter() // deos-bypass: bounded-iter -- the forty funded owners in the fixed recipe
      .enumerate()
      .min_by_key(|(_, count)| **count)
      .unwrap()
      .0;
    assert!(owner_identities[owner] < u32::from(<Runtime as Config>::MaxOwnerSlots::get()));
    owner_identities[owner] += 1;
  }
  let user_identities = owner_identities
    .iter() // deos-bypass: bounded-iter -- the forty fixed owner counts
    .sum::<u32>();
  assert_eq!(user_identities, (PAGES + WATCHES + 2) as u32);
  let peak_identities = 15 + user_identities;
  let peak_active = (3 + WATCHES + PAGES + 1) as u32;
  let peak_source_registrations = (PAGES + 1) as u32;
  assert!(peak_identities <= <Runtime as Config>::MaxActorIdentities::get());
  assert!(peak_active <= <Runtime as Config>::MaxActiveActors::get());
  assert!(peak_source_registrations <= <Runtime as Config>::MaxActiveActors::get());
  let retiring_holes = 31 * PAGES as u64 + 1;
  assert!(retiring_holes <= u64::from(<Runtime as Config>::MaxActiveActors::get()) * 32);
  let largest_frontier = pages
    .iter() // deos-bypass: bounded-iter -- four hash-bound source frontiers
    .map(|source| (source[PAGES - 1] + 1).checked_mul(32).unwrap())
    .max()
    .unwrap();
  assert_eq!(largest_frontier, 320_000_128);
  // The complete native driver pins this unshortened Genesis-to-admission prefix.
  let construction_end_block = 187_204 + producer_span;
  assert!(construction_end_block < u64::from(u32::MAX));
  // Each operator owns half the epochs and signs two calls per epoch. Adding all
  // permanent births and setup/prefix calls to either operator is conservative.
  let operator_nonce_upper = epochs + PAGES as u64 + other_signed_calls;
  assert!(operator_nonce_upper < u64::from(u32::MAX));
  qualify_scalar_horizon(
    epochs,
    producer_span,
    construction_end_block,
    u64::from(peak_identities),
  );
  qualify_session_horizon(construction_end_block);
  qualify_quiet_governance_horizon(construction_end_block);
  qualify_namespace_horizon(construction_end_block);
  println!(
    "owner_identity_counts={owner_identities:?} peak_identities={peak_identities} peak_active={peak_active} retiring_holes={retiring_holes} largest_frontier={largest_frontier} construction_end_block={construction_end_block} operator_nonce_upper={operator_nonce_upper}"
  );
}

fn qualify_construction_pacing(construction_blocks: u64, setup_calls: u32) {
  // These are blocks, not calls: all calls in one batch share the parent multiplier.
  // The first permanent birth is already one of the construction blocks. Removing
  // its five-block turn from the executable 105-block setup leaves these eight batches.
  let setup_batches = [7u32, 40, 2, 2, 2, 2, 3 * WATCHES as u32, 2];
  assert_eq!(setup_batches.into_iter().sum::<u32>(), setup_calls);
  let period = SPACER_BLOCKS + 1;
  let setup_mutation_blocks = setup_batches.len() as u64;
  let setup_extra_empty_blocks = 64 - SPACER_BLOCKS;
  let setup_span_excluding_first_birth = period * setup_mutation_blocks + setup_extra_empty_blocks;
  assert_eq!(setup_span_excluding_first_birth + period, 105);

  // The selected final reclamation fragment closes two permanent targets, activates
  // one temporary, then closes both temporaries. Its five calls have three fee updates.
  let terminal_batches = [2u32, 1, 2];
  let terminal_calls = terminal_batches.into_iter().sum::<u32>();
  assert_eq!(terminal_calls, 5);
  let terminal_mutation_blocks = terminal_batches.len() as u64;
  let terminal_span = period * terminal_mutation_blocks;
  let priming_empty_blocks = 9;
  let required_extra_span = setup_span_excluding_first_birth + priming_empty_blocks + terminal_span;
  let setup_allowance = period * u64::from(setup_calls) + 64;
  assert_eq!((required_extra_span, setup_allowance), (124, 409));
  let padding_empty_blocks = setup_allowance.checked_sub(required_extra_span).unwrap();
  let mutation_blocks = construction_blocks + setup_mutation_blocks + terminal_mutation_blocks;
  let normal_empty_blocks = SPACER_BLOCKS * mutation_blocks
    + setup_extra_empty_blocks
    + priming_empty_blocks
    + padding_empty_blocks;
  let producer_span = period * construction_blocks + setup_allowance;
  assert_eq!(mutation_blocks + normal_empty_blocks, producer_span);
  assert_eq!(padding_empty_blocks, 285);
  println!(
    "pacing_mutation_blocks={mutation_blocks} pacing_normal_empty_blocks={normal_empty_blocks} setup_batches={setup_batches:?} terminal_batches={terminal_batches:?} terminal_signed_calls={terminal_calls} setup_span_excluding_first_birth={setup_span_excluding_first_birth} priming_empty_blocks={priming_empty_blocks} terminal_span={terminal_span} padding_empty_blocks={padding_empty_blocks} pacing_producer_span={producer_span}"
  );
  println!(
    "Pacing projects one four-empty-block turn per temporary epoch, permanent birth and final retirement, plus eight setup and three terminal batches. Retirement may share the next activation or permanent birth block; it is paid in the epoch pair reservation. Extra priming/padding is Normal-empty. This is conditional recipe geometry, not execution of the omitted history or proof of successful admission/currency/source transitions."
  );
}

/// Projects ordered source phases without executing individual temporary epochs.
/// The registration/review source lemmas own the conditional transition semantics.
fn qualify_ordered_phase_projection(
  pages: &[Vec<u64>],
  expected_by_width: &[u64; WATCHES + 1],
) -> [[u64; WATCHES + 1]; 2] {
  let mut frontiers = [0u64; WATCHES];
  let mut owner_epochs = [[0u64; WATCHES + 1]; 2];
  let mut epochs_by_mask = [0u64; 1 << WATCHES];
  let mut epochs = 0u64;
  let mut gap_segments = 0u64;
  let mut permanent_holes = 0u64;
  let mut owner_identities = [0u32; 40];
  owner_identities[0] = WATCHES as u32 + 1;
  owner_identities[1] = 1;

  for cohort in 0..PAGES {
    if cohort > 0 {
      let gaps: [u64; WATCHES] = std::array::from_fn(|source| {
        (pages[source][cohort] - pages[source][cohort - 1] - 1)
          .checked_mul(32)
          .unwrap()
      });
      let mut boundaries = gaps;
      boundaries.sort_unstable();
      let mut previous = 0;
      for boundary in boundaries {
        if boundary == previous {
          continue;
        }
        let duration = boundary - previous;
        let mut mask = 0usize;
        for source in 0..WATCHES {
          if gaps[source] > previous {
            mask |= 1 << source;
            frontiers[source] = frontiers[source].checked_add(duration).unwrap();
          }
          assert!(frontiers[source] <= pages[source][cohort] * 32);
        }
        let width = mask.count_ones() as usize;
        assert!(width > 0 && width <= WATCHES);
        assert_eq!(duration % 32, 0);
        let first_owner = (epochs % 2) as usize;
        owner_epochs[first_owner][width] += duration.div_ceil(2);
        owner_epochs[1 - first_owner][width] += duration / 2;
        epochs_by_mask[mask] += duration;
        epochs = epochs.checked_add(duration).unwrap();
        gap_segments += 1;
        previous = boundary;
      }
    }
    for source in 0..WATCHES {
      assert_eq!(frontiers[source], pages[source][cohort] * 32);
      // The preceding temporary generation is retired in the birth block, then
      // this new permanent registration takes slot zero on every source.
      frontiers[source] = frontiers[source].checked_add(1).unwrap();
    }
    // Match the signed fragment's first Alice-owned cohort, then use the funded
    // forty-creator assignment. Reserved producers/temporary slots count up front.
    let creator = if cohort == 0 {
      0
    } else {
      owner_identities
        .iter() // deos-bypass: bounded-iter -- forty fixed funded creators
        .enumerate()
        .min_by_key(|(_, count)| **count)
        .unwrap()
        .0
    };
    owner_identities[creator] += 1;
    assert!(owner_identities[creator] <= u32::from(<Runtime as Config>::MaxOwnerSlots::get()));
    // Thirty-one temporary retirements leave holes in the retained new page.
    // Their odd duration alternates the first operator between successive cohorts.
    let first_owner = (epochs % 2) as usize;
    owner_epochs[first_owner][WATCHES] += 16;
    owner_epochs[1 - first_owner][WATCHES] += 15;
    epochs_by_mask[(1 << WATCHES) - 1] += 31;
    epochs += 31;
    permanent_holes += 31;
    for source in 0..WATCHES {
      frontiers[source] += 31;
      assert_eq!(frontiers[source], (pages[source][cohort] + 1) * 32);
    }
    assert_eq!(permanent_holes, 31 * (cohort as u64 + 1));
  }
  for width in 0..=WATCHES {
    assert_eq!(owner_epochs[0][width], owner_epochs[1][width]);
    assert_eq!(
      owner_epochs[0][width] + owner_epochs[1][width],
      expected_by_width[width]
    );
  }
  assert_eq!(
    epochs,
    expected_by_width.iter(/* deos-bypass: bounded-iter -- five fixed watch-width counters. */).sum::<u64>()
  );
  assert_eq!(epochs_by_mask[0], 0);
  assert!(gap_segments <= (WATCHES * (PAGES - 1)) as u64);
  assert_eq!(permanent_holes, 31 * PAGES as u64);
  assert_eq!(
    *owner_identities.iter(/* deos-bypass: bounded-iter -- forty fixed creator counters. */).max().unwrap(),
    250
  );
  let mut all_greedy = [0u32; 40];
  all_greedy[0] = WATCHES as u32 + 1;
  all_greedy[1] = 1;
  for _ in 0..PAGES {
    let creator = all_greedy
      .iter() // deos-bypass: bounded-iter -- forty fixed creator counters.
      .enumerate()
      .min_by_key(|(_, count)| **count)
      .unwrap()
      .0;
    all_greedy[creator] += 1;
  }
  assert_eq!(owner_identities, all_greedy);
  let operator_calls = owner_epochs.map(|counts| 2 * counts.into_iter().sum::<u64>());
  assert_eq!(operator_calls, [epochs; 2]);
  println!(
    "scope=conditional_ordered_recipe_projection gap_segments={gap_segments} cohort_births={PAGES} frontiers={frontiers:?} permanent_holes={permanent_holes} first_permanent_creator=Alice owner_identities={owner_identities:?}"
  );
  println!(
    "ordered_epochs_by_mask={epochs_by_mask:?} ordered_epochs_by_operator_width={owner_epochs:?} temporary_activate_and_deactivate_calls_by_operator={operator_calls:?}"
  );
  owner_epochs
}

fn run(path: &str) {
  let pages = fixed_pages(path);
  let mut epochs_by_width = [0u64; WATCHES + 1];
  let hole_epochs = 31 * PAGES as u64;
  epochs_by_width[WATCHES] = hole_epochs;
  let mut serial_gap_epochs = 0u64;
  let mut gap_epochs = 0u64;
  for cohort in 1..PAGES {
    let mut remaining: Vec<_> = pages
      .iter() // deos-bypass: bounded-iter -- four hash-bound sources
      .map(|source| 32 * (source[cohort] - source[cohort - 1] - 1))
      .collect();
    serial_gap_epochs += remaining
      .iter() // deos-bypass: bounded-iter -- four validated source gaps
      .sum::<u64>();
    remaining.sort_unstable();
    gap_epochs += remaining[WATCHES - 1];
    let mut previous = 0;
    for (index, stop) in remaining.into_iter().enumerate() {
      epochs_by_width[WATCHES - index] += stop - previous;
      previous = stop;
    }
  }
  let epochs = gap_epochs + hole_epochs;
  assert_eq!(
    serial_gap_epochs + hole_epochs + PAGES as u64,
    1_279_043_392
  );
  assert_eq!(epochs, 510_456_656);
  assert_eq!(
    epochs_by_width
      .iter() // deos-bypass: bounded-iter -- five fixed watch-width counters
      .sum::<u64>(),
    epochs
  );
  let watched_insertions: u64 = epochs_by_width
    .iter() // deos-bypass: bounded-iter -- five fixed watch-width counters
    .enumerate()
    .map(|(width, count)| width as u64 * count)
    .sum();
  assert_eq!(
    watched_insertions + (PAGES * WATCHES) as u64,
    pages
      .iter() // deos-bypass: bounded-iter -- four validated source frontiers
      .map(|source| 32 * (source[PAGES - 1] + 1))
      .sum::<u64>()
  );
  let epochs_by_operator_width = qualify_ordered_phase_projection(&pages, &epochs_by_width);
  println!("scope=fixed_corpus_epoch_arithmetic_and_codec_tariff input_sha256={INPUT_SHA256}");
  println!(
    "gap_epochs={gap_epochs} hole_epochs={hole_epochs} temporary_epochs={epochs} permanent_births={PAGES}"
  );
  println!("epochs_by_watch_width={epochs_by_width:?}");
  for (owner, source) in [
    ("runtime_lib", include_bytes!("../src/lib.rs").as_slice()),
    (
      "actor_config",
      include_bytes!("../src/configs/actor_config.rs").as_slice(),
    ),
    (
      "actors_weights",
      include_bytes!("../src/weights/pallet_deos_actors.rs").as_slice(),
    ),
    (
      "actors_lib",
      include_bytes!("../../pallets/actors/src/lib.rs").as_slice(),
    ),
    ("cargo_lock", include_bytes!("../../Cargo.lock").as_slice()),
  ] {
    println!("{owner}_sha256={}", digest(source));
  }

  TestExternalities::default().execute_with(|| {
    assert_eq!(
      pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get(),
      pallet_transaction_payment::Multiplier::from_u32(1)
    );
    let (deactivate_bytes, deactivate_length_fee, deactivate_fee) = transaction_fee(RuntimeCall::Actors(
      pallet_deos_actors::Call::deactivate_actor { actor_id: 0 },
    ));
    let mut length_reservation = 0u128;
    let mut declared_reservation = 0u128;
    let mut operator_reservations = [0u128; 2];
    for (width, count) in epochs_by_width
      .iter() // deos-bypass: bounded-iter -- five fixed watch-width counters
      .copied()
      .enumerate()
      .skip(1)
    {
      let contract = ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
        window: None,
        steps: vec![Step {
          precondition: None,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        }]
        .try_into()
        .unwrap(),
        on_unfunded: OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: Some(
          ParkedBalanceActivationOf::<Runtime>::try_from_rules(
            (0..width)
              .map(|asset| ParkedBalanceRule {
                asset: AssetKind::Local(asset as u32),
                authored_min_delta: 100,
              })
              .collect(),
          )
          .unwrap(),
        ),
        funding: FundingSourcePolicy::OwnerOnly,
        auto_close_at_cycle_nonce: None,
      };
      let (bytes, length_fee, fee) = transaction_fee(RuntimeCall::Actors(
        pallet_deos_actors::Call::activate_actor { actor_id: 1, contract },
      ));
      length_reservation += u128::from(count) * (length_fee + deactivate_length_fee);
      declared_reservation += u128::from(count) * (fee + deactivate_fee);
      for operator in 0..2 {
        operator_reservations[operator] +=
          u128::from(epochs_by_operator_width[operator][width]) * (fee + deactivate_fee);
      }
      println!(
        "watches={width} epochs={count} activate_bytes={bytes} deactivate_bytes={deactivate_bytes} pair_length_fee={} pair_declared_fee={}",
        length_fee + deactivate_length_fee,
        fee + deactivate_fee
      );
    }
    println!(
      "epoch_length_fee_reservation={length_reservation} epoch_declared_fee_reservation={declared_reservation} twelve_preset_endowments={}",
      12u128 * (1u128 << 60)
    );
    let steps: ContractSteps<Runtime> = vec![Step {
      precondition: None,
      task: Task::Transfer {
        to: AccountId::new([2; 32]),
        asset: AssetKind::Local(0),
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap();
    let machine_fee = Actors::user_pipeline_machine_capacity_requirement(&steps).unwrap()
      - <Runtime as Config>::MinUserBalance::get();
    let trigger_fee = <Runtime as Config>::WeightToFee::weight_to_fee(
      &<Runtime as Config>::WeightInfo::cadenced_trigger_occurrence(),
    );
    let action_fee = <Runtime as Config>::WeightToFee::weight_to_fee(
      &<Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&steps[0].task).unwrap(),
    );
    let cycle_fee = trigger_fee + machine_fee + action_fee;
    // Earlier temporary epochs can retire in the next registration block. The final
    // Active epoch needs its own B+1 deactivation; its fee is already in the pair total.
    let construction_blocks = epochs + PAGES as u64 + 1;
    let producer_allowance = WATCHES as u128 * u128::from(construction_blocks) * cycle_fee;
    println!(
      "producer_trigger_fee={trigger_fee} producer_pipeline_fee={machine_fee} producer_action_fee={action_fee} producer_cycle_fee={cycle_fee} construction_blocks={construction_blocks} four_producer_cycle_allowance={producer_allowance}"
    );
    println!(
      "epoch_and_producer_fee_allowance={} producer_lp_units_per_account={construction_blocks}",
      declared_reservation + producer_allowance
    );
    qualify_fee_spacers();
    let spaced_blocks = (SPACER_BLOCKS + 1) * construction_blocks;
    assert!(spaced_blocks < u64::from(u32::MAX));
    let spaced_producer_allowance = WATCHES as u128 * u128::from(spaced_blocks) * cycle_fee;
    let user_births = PAGES as u128 + WATCHES as u128 + 2;
    let hold_per_user = state_hold_upper();
    let holds = user_births * hold_per_user;
    let creation_fees = user_births * <Runtime as Config>::ActorCreationFee::get();
    println!("spaced_construction_blocks={spaced_blocks} spaced_producer_cycle_allowance={spaced_producer_allowance} user_births={user_births} creation_fees={creation_fees} refundable_hold_upper={holds}");
    println!("spaced_epoch_producer_creation_hold_reservation={}",
      declared_reservation + spaced_producer_allowance + creation_fees + holds);
    // The signed recipe has seven top-ups, forty owner-funding calls, two pool calls and
    // three producer preparation calls per asset, plus two Dormant births. The first
    // permanent Actor is already included in PAGES and must not be counted twice.
    let setup_calls = 7 + 40 + 5 * WATCHES as u32 + 2;
    let prefix_calls = 5;
    let terminal_calls = 5;
    let other_signed_calls = setup_calls + prefix_calls + terminal_calls;
    qualify_construction_pacing(construction_blocks, setup_calls);
    let setup_blocks = (SPACER_BLOCKS + 1) * u64::from(setup_calls) + 64;
    let producer_span = spaced_blocks + setup_blocks;
    qualify_capacity_arithmetic(&pages, epochs, producer_span, u64::from(other_signed_calls));
    let producer_funding = WATCHES as u128 * 320_000 * deos_runtime::UNIT;
    let cycle_reservation = WATCHES as u128 * u128::from(producer_span) * cycle_fee;
    let native_floor = deos_runtime::EXISTENTIAL_DEPOSIT;
    let native_total_lower = (320_000 * deos_runtime::UNIT)
      .checked_sub(u128::from(producer_span) * cycle_fee)
      .unwrap();
    let native_available_lower = native_total_lower.checked_sub(native_floor).unwrap();
    assert!(
      native_available_lower > <Runtime as Config>::MinUserBalance::get() + cycle_fee
    );
    assert!(cycle_reservation
      + WATCHES as u128 * (native_floor + <Runtime as Config>::MinUserBalance::get() + cycle_fee)
      < producer_funding);
    assert!(producer_span < 10_000_000_000);
    let lp_lower = 10_000_000_000u128 - u128::from(producer_span);
    assert!(lp_lower > 1);
    println!(
      "producer_native_total_lower={native_total_lower} producer_native_available_lower={native_available_lower} producer_native_account_floor={native_floor} producer_lp_lower={lp_lower}; quantity bounds retain next-block fee readiness"
    );
    let other_signed_fees = (PAGES as u128 + u128::from(other_signed_calls))
      * signed_fee_upper();
    let payload_bytes = RuntimeCall::Actors(
      pallet_deos_actors::Call::set_user_creation_enabled { enabled: true },
    ).encoded_size() as u32;
    let governance_backing = <Runtime as pallet_governance::Config>::ProposalOpeningFee::get()
      + <Runtime as pallet_governance::Config>::PayloadAdmissionWitnessDeposit::get()
      + <<Runtime as pallet_governance::Config>::ProposalPayloadPreimageNoteCostProvider
        as pallet_governance::ProposalPayloadPreimageNoteCostProvider<Balance>>::note_cost(payload_bytes).unwrap();
    let asset_backing = WATCHES as u128
      * <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolSetupFee::get()
      + (WATCHES as u128 + 1)
        * (<Runtime as polkadot_sdk::pallet_assets::Config>::AssetDeposit::get()
          + <Runtime as polkadot_sdk::pallet_assets::Config>::MetadataDepositBase::get()
          + 2 * u128::from(<Runtime as polkadot_sdk::pallet_assets::Config>::StringLimit::get())
            * <Runtime as polkadot_sdk::pallet_assets::Config>::MetadataDepositPerByte::get())
      + 62 * <Runtime as polkadot_sdk::pallet_assets::Config>::AssetAccountDeposit::get();
    let native_liquidity = WATCHES as u128 * deos_runtime::UNIT;
    let owner_funding = 40 * 30_000 * deos_runtime::UNIT;
    let owner_obligations = other_signed_fees + governance_backing + asset_backing
      + creation_fees + holds + native_liquidity
      + 40 * deos_runtime::EXISTENTIAL_DEPOSIT;
    assert!(owner_obligations < owner_funding);
    let owner_slots = <Runtime as Config>::MaxOwnerSlots::get() as u128;
    let single_owner_upper = (owner_slots + u128::from(other_signed_calls))
      * signed_fee_upper()
      + owner_slots * (hold_per_user + <Runtime as Config>::ActorCreationFee::get())
      + governance_backing + asset_backing + native_liquidity
      + 40 * deos_runtime::EXISTENTIAL_DEPOSIT;
    assert!(single_owner_upper < 30_000 * deos_runtime::UNIT);
    assert!(epochs_by_width.into_iter().all(|count| count % 2 == 0));
    assert_eq!(operator_reservations, [declared_reservation / 2; 2]);
    let capital = declared_reservation + producer_funding + owner_funding;
    assert!(capital < 12u128 * (1u128 << 60));
    println!("setup_calls={setup_calls} prefix_calls={prefix_calls} terminal_calls={terminal_calls} other_signed_calls={other_signed_calls} setup_blocks={setup_blocks} producer_span={producer_span} maximum_signed_fee={} other_signed_fee_upper={other_signed_fees}", signed_fee_upper());
    println!("producer_funding={producer_funding} producer_cycle_reservation={cycle_reservation} owner_funding={owner_funding} owner_obligation_upper={owner_obligations} governance_backing={governance_backing} asset_backing={asset_backing}");
    println!("partitioned_coded_capital_reservation={capital}");
    println!("single_owner_non_epoch_obligation_upper={single_owner_upper} each_operator_epoch_reservation={} ordered_operator_epoch_reservations={operator_reservations:?}", declared_reservation / 2);
  });
  println!(
    "Reservations use one StopCycle per temporary contract, maximum nonce encoding and zero tips. Four Normal-empty spacers bound the next mutation's multiplier by one. Producer allowance assumes at most one complete one-Step cycle per actor per block. Hold bounds apply only without close_when/AwaitFunding. Producer and owner funding contain their fee/hold obligations rather than add them twice. This arithmetic alone does not qualify native composition, complete chronology, certified tariffs or physical admission."
  );
}

fn main() {
  let mut args = std::env::args().skip(1);
  match (args.next(), args.next()) {
    (Some(arg), None) if arg == "--signed-call-inputs" => signed_caller_inputs(),
    (Some(arg), None) if matches!(arg.as_str(), "--help" | "-h") => println!(
      "Fixed retention-history ordered-phase arithmetic and current native codec tariff.\nPass the unchanged actor_retention_pages JSON, or --signed-call-inputs for setup/reclamation DispatchInfo and codec inputs. No geometry or pricing parameters are accepted.\nThis projects source masks, frontiers and operator counts without replaying individual epochs. It does not execute a signed history, certify capital or qualify generated Weight."
    ),
    (Some(path), None) => run(&path),
    _ => {
      eprintln!("Pass the fixed JSON path or --help.");
      std::process::exit(2);
    }
  }
}
