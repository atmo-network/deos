//! One ordinary parked-balance intake and payout; no financial-wait enablement.
use super::*;
use pallet_deos_actors::{
  ActorContract, ActorFinancialWaits, DependencyPlans, DependencyTimedReviews, FundingSourcePolicy,
  OnUnfunded, ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule,
  PendingDependencyEvents,
};
use primitives::AssetKind;

const WATCHED: u32 = 500_003;
const INTAKE: u128 = 117;

#[test]
fn parked_intake_then_native_delivery_reparks_without_repeat() {
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let owner = crate::AccountId::from(signer.public());
  let recipient = super::super::super::common::BOB;
  let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
  let prefunding = crate::EXISTENTIAL_DEPOSIT.checked_mul(1_000).unwrap();
  let contract: pallet_deos_actors::ActorContractOf<Runtime> = ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: Workload::Transfer.steps(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(vec![ParkedBalanceRule {
        asset: AssetKind::Local(WATCHED),
        authored_min_delta: 100,
      }])
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  let mut prepared = TestExternalities::new(reference_genesis_storage(&[]));
  let (id, member, creation_fee, initial) = prepared.execute_with(|| {
    System::set_block_number(1);
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(crate::SLOT_DURATION);
    assert!(!Actors::user_creation_enabled());
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(super::super::super::common::create_test_asset(
      WATCHED, &owner
    ));
    add_balance(
      &owner,
      crate::EXISTENTIAL_DEPOSIT.checked_mul(1_000_000).unwrap(),
    );
    if total(&recipient) < crate::EXISTENTIAL_DEPOSIT {
      add_balance(&recipient, crate::EXISTENTIAL_DEPOSIT - total(&recipient));
    }
    let account = Actors::sovereign_account_id(&owner, 0);
    assert_eq!(total(&account), 0);
    add_balance(&account, prefunding);
    let owner_before = total(&owner);
    let id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(owner.clone()),
      0,
      Mutability::Mutable,
      Some(contract.clone())
    ));
    let identity = Actors::actor_identity(id).unwrap();
    assert_eq!(identity.sovereign_account, account);
    assert_eq!(identity.last_control_mutation_block, 1);
    let quote = Actors::actor_cost_quote(id).unwrap();
    let creation_fee = owner_before.checked_sub(total(&owner)).unwrap();
    assert_eq!(creation_fee, quote.creation_fee);
    assert_eq!(
      quote.prospective_trigger_fee.unwrap().trigger_family,
      TriggerFamily::ParkedBalance
    );
    let episode = ParkedBalanceEpisodes::<Runtime>::get(id).unwrap();
    assert_eq!(episode.watches.len(), 1);
    assert_eq!(
      (
        episode.watches[0].anchor,
        episode.watches[0].authored_min_delta,
        episode.watches[0].certified_minimum_balance
      ),
      (0, 100, 1)
    );
    assert_eq!(
      DependencyTimedReviews::<Runtime>::get(id).unwrap().deadline,
      WakeupKey::Block(2)
    );
    assert!(!Actors::pending_signal(id));
    assert!(Actors::service_nodes(id).is_none());
    assert!(Actors::actor_run_state(id).is_none());
    let initial = [
      total(&account),
      total(&recipient),
      total(&owner),
      total(&sink),
    ];
    (
      id,
      Member {
        owner: owner.clone(),
        account,
        generation: generation(id),
        quote,
      },
      creation_fee,
      initial,
    )
  });
  prepared.commit_all().unwrap();
  let mut storage = prepared.execute_with(current_top_storage);
  let profiles = BTreeMap::from([(
    id,
    WorkloadActorProfile {
      step_count: 1,
      opening_predicates_per_step: 0,
      task: Some(WorkloadTask::Transfer),
    },
  )]);
  let effect_hint = member.quote.maximum_next_action_fee.maximum_effect_weight;
  let control_hint =
    Actors::contract_steps_admission_weight_upper(ActorType::User, &contract.steps)
      .checked_sub(&effect_hint)
      .unwrap();
  let members = BTreeMap::from([(id, member)]);
  let member = &members[&id];
  let [native_before, recipient_before, owner_before, sink_before] = initial;
  let mut parent = parent_header_for(storage.clone(), &[], 1);
  let mut fifo = VecDeque::new();
  let mut actor_fees = [0u128; 3];
  let mut transaction_fees = 0u128;
  let (mut sink_credits, mut sink_debits) = (0u128, 0u128);
  let mut rows = Vec::new();
  for number in 2..=8 {
    let nonce = TestExternalities::new(storage.clone()).execute_with(|| {
      if number == 4 {
        assert_eq!(
          DependencyTimedReviews::<Runtime>::get(id).unwrap().deadline,
          WakeupKey::Block(4)
        );
        assert!(!PendingDependencyEvents::<Runtime>::contains_key(id));
        assert_eq!(Assets::balance(WATCHED, &member.account), INTAKE);
      }
      System::account_nonce(&owner)
    });
    let calls = if number == 3 {
      vec![RuntimeCall::Assets(
        polkadot_sdk::pallet_assets::Call::mint {
          id: WATCHED,
          beneficiary: member.account.clone().into(),
          amount: INTAKE,
        },
      )]
    } else {
      Vec::new()
    };
    let authored = author_complete_block_after_with_calls(
      storage,
      &[],
      &parent,
      number,
      UserDemand::ActorOnly,
      &signer,
      nonce,
      &profiles,
      &calls,
    );
    assert_complete_block_replays_natively(&authored, &[]);
    let step = u32::from(number == 5);
    assert_eq!(authored.metrics.actor_steps, step);
    assert_eq!(authored.metrics.distinct_actors, step);
    assert_eq!(authored.metrics.transfer_steps, step);
    assert_eq!(authored.metrics.completed_cycles, step);
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(authored.metrics.user_calls, u32::from(number == 3));
    assert_eq!(
      authored.metrics.trigger_occurrences,
      if number == 4 {
        vec![(id, TriggerFamily::ParkedBalance)]
      } else {
        Vec::new()
      }
    );
    if number != 3 {
      assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    } else {
      assert!(authored.metrics.user_dispatch.ref_time() > 0);
    }
    let accounting = inspect_block(
      &[],
      &authored,
      &members,
      &mut fifo,
      (control_hint, effect_hint),
      number == 8,
      StepPosition {
        workload: Workload::Transfer,
        index: 0,
        completes_cycle: true,
      },
    );
    assert_eq!(accounting["successfulActions"], step);
    let fees = accounting["collectedFees"]
      .as_array()
      .unwrap()
      .iter()
      .map(|v| v.as_str().unwrap().parse::<u128>().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(fees[0] > 0, number == 4);
    assert_eq!(fees[1] > 0, number == 5);
    assert_eq!(fees[2] > 0, number == 5);
    for index in 0..3 {
      actor_fees[index] = actor_fees[index].checked_add(fees[index]).unwrap();
    }
    let transaction = &accounting["transactionFees"];
    assert_eq!(transaction["count"], u32::from(number == 3));
    assert_eq!(transaction["tip"], "0");
    let tx_fee = transaction["actualFeeIncludingTip"]
      .as_str()
      .unwrap()
      .parse::<u128>()
      .unwrap();
    assert_eq!(tx_fee > 0, number == 3);
    transaction_fees = transaction_fees.checked_add(tx_fee).unwrap();
    let credit = accounting["sinkIn"]
      .as_str()
      .unwrap()
      .parse::<u128>()
      .unwrap();
    let debit = accounting["sinkOut"]
      .as_str()
      .unwrap()
      .parse::<u128>()
      .unwrap();
    let target_credit = fees
      .iter()
      .try_fold(tx_fee, |sum, fee| sum.checked_add(*fee))
      .unwrap();
    assert!(
      credit >= target_credit,
      "other reference credits remain distinct from target fees"
    );
    sink_credits = sink_credits.checked_add(credit).unwrap();
    sink_debits = sink_debits.checked_add(debit).unwrap();
    let paid = actor_fees
      .into_iter()
      .try_fold(0u128, u128::checked_add)
      .unwrap();
    let money = TestExternalities::new(authored.post_state.clone()).execute_with(|| {
      let state = Actors::active_actor_state(id).unwrap();
      assert_eq!(state.contract, contract);
      assert_eq!(generation(id), member.generation);
      assert_eq!(state.identity.last_control_mutation_block, 1);
      assert_eq!(state.identity.cycle_nonce, u64::from(number >= 5));
      assert_eq!(state.hot.cycle_state, CycleState::Idle);
      assert_eq!(state.hot.pending_signal, number == 4);
      assert!(state.run_state.is_none());
      assert!(!ActorFinancialWaits::<Runtime>::contains_key(id));
      let local = if number >= 3 { INTAKE } else { 0 };
      assert_eq!(Assets::balance(WATCHED, &member.account), local);
      let delivered = if number >= 5 {
        crate::EXISTENTIAL_DEPOSIT
      } else {
        0
      };
      assert_eq!(
        total(&member.account),
        native_before
          .checked_sub(delivered)
          .unwrap()
          .checked_sub(paid)
          .unwrap()
      );
      assert_eq!(
        total(&recipient),
        recipient_before.checked_add(delivered).unwrap()
      );
      assert_eq!(
        total(&owner),
        owner_before.checked_sub(transaction_fees).unwrap()
      );
      assert_eq!(
        total(&sink),
        sink_before
          .checked_add(sink_credits)
          .unwrap()
          .checked_sub(sink_debits)
          .unwrap()
      );
      if number == 4 {
        assert!(!ParkedBalanceEpisodes::<Runtime>::contains_key(id));
        assert!(DependencyPlans::<Runtime>::get(id).is_empty());
        assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 5);
      } else {
        assert!(Actors::service_nodes(id).is_none());
        let episode = ParkedBalanceEpisodes::<Runtime>::get(id).unwrap();
        assert_eq!(episode.watches.len(), 1);
        assert_eq!(
          episode.watches[0].anchor,
          if number >= 5 { INTAKE } else { 0 }
        );
        assert_eq!(DependencyPlans::<Runtime>::get(id).len(), 1);
        assert!(matches!(
          Actors::actor_processes(id).unwrap().residence,
          Some(ProcessResidence::Parked(_))
        ));
      }
      serde_json::json!({"sovereignNative": total(&member.account).to_string(),
        "recipientNative": total(&recipient).to_string(), "issuerNative": total(&owner).to_string(),
        "sinkNative": total(&sink).to_string(), "watchedLocal": local.to_string()})
    });
    rows.push(serde_json::json!({"block": number, "targetSteps": authored.metrics.actor_steps,
      "targetActors": authored.metrics.distinct_actors, "targetCompletedCycles": authored.metrics.completed_cycles,
      "targetTriggerOccurrences": authored.metrics.trigger_occurrences.len(), "signedIntakeCalls": authored.metrics.user_calls,
      "globalBookedControl": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
      "globalBookedEffect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
      "globalBookedUserDispatch": [authored.metrics.user_dispatch.ref_time(), authored.metrics.user_dispatch.proof_size()],
      "money": money, "accounting": accounting}));
    parent = authored.block.header;
    storage = authored.post_state;
  }
  assert_eq!(rows.len(), 7);
  assert!(fifo.is_empty());
  let native_actor_cost = actor_fees
    .into_iter()
    .try_fold(creation_fee, u128::checked_add)
    .unwrap();
  println!(
    "ACTORS_H2_PARKED_DELIVERY_V1 {}",
    serde_json::json!({
      "schema": 1, "scope": "prepared ordinary-composition native full Executive; explicit model identity capacity, Root-enabled permission, synthetic relay; not default authority/public genesis/Wasm/PVF",
      "identityCapacityModel": cfg!(feature = "benchmark-authority-scaffolding"),
      "runtimeBenchmarks": cfg!(feature = "runtime-benchmarks"),
      "optionalAuthoritySupplements": cfg!(all(feature = "benchmark-authority-scaffolding", feature = "runtime-benchmarks")),
      "productionCapacityQualified": false, "productionArtifactCompatible": false,
      "population": 1, "actor": id, "birthBlock": 1, "firstBlock": 2, "lastBlock": 8,
      "intakeBlock": 3, "wakeBlock": 4, "eligibleBlock": 5, "completionBlock": 5,
      "watchedAsset": WATCHED, "watchedMinimumBalance": "1", "initialAnchor": "0", "minimumDelta": "100", "localIntake": INTAKE.to_string(),
      "nativePrefunding": prefunding.to_string(), "fundingScope": "finite fixture, not a complete fee/funding upper",
      "initialMoney": {"sovereignNative": native_before.to_string(), "recipientNative": recipient_before.to_string(),
        "issuerNative": owner_before.to_string(), "sinkNative": sink_before.to_string()},
      "creationFeeObserved": creation_fee.to_string(), "actorFeesObserved": actor_fees.map(|f| f.to_string()),
      "sinkCreditsObserved": sink_credits.to_string(), "sinkDebitsObserved": sink_debits.to_string(),
      "nativeActorFeesIncludingCreation": native_actor_cost.to_string(), "signedIntakeTransactionFeeIncludingTip": transaction_fees.to_string(),
      "nativeDelivered": crate::EXISTENTIAL_DEPOSIT.to_string(), "usefulWorkflowsSucceeded": 1, "failed": 0, "pending": 0, "censored": 0,
      "intakeToWakeBlocks": 1, "wakeToCompletionBlocks": 1, "terminalNoRepeatBlocks": [6,7,8],
      "financialWait": "unsupported and unused", "contractScaleSha256": format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&contract.encode()))),
      "sourceWeightExecutableIdentityScope": "retained caller source/protected/native executable manifests",
      "directSetupTransactionCost": "excluded; actual signed intake is included separately", "blocks": rows,
      "logicalStorageOperations": null, "physicalMetrics": null,
    })
  );
}
