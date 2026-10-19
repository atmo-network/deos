//! Native reference-ledger and canonical-caller boundaries, not physical I/K maxima.

use super::{actors_design_comparison, common, h1_storage_trace};
use crate::{
  AccountId, Actors, Assets, Balance, Balances, Runtime, RuntimeEvent, RuntimeOrigin, System,
  configs::{BlockResourceBudgetValue, actor_config::TmctolAssetOps},
};
use codec::{Encode, MaxEncodedLen};
use pallet_deos_actors::{
  self as actors, AssetOps, BlockResourceDomain, BlockResourcePhase, FundingSourcePolicy,
  Mutability, OnUnfunded, Trigger, WeightInfo,
};
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{Currency, Get, Hooks, LockableCurrency, ReservableCurrency, WithdrawReasons},
  weights::Weight,
};
use primitives::AssetKind;
use serde_json::json;
use std::collections::BTreeSet;

type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

fn resources(now: u32, constrained: Option<bool>) -> actors::BlockResourceState<u32> {
  System::set_block_number(now);
  common::set_consensus_timestamp(
    u64::from(now) * primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS,
  );
  let budget = actors_design_comparison::with_isolated_block(now, || {
    actors_design_comparison::prepass_budget(now, BlockResourceBudgetValue::get()).unwrap()
  });
  let mut state = actors::BlockResourceState::new(now);
  state.begin_prepass(budget).unwrap();
  state.open_external_phase().unwrap();
  if let Some(proof) = constrained {
    let needed = bookkeeping() + inspection();
    let control = budget.limits().actor_control();
    assert!(needed.all_lte(control));
    let prior = if proof {
      Weight::from_parts(0, control.proof_size() - needed.proof_size())
    } else {
      Weight::from_parts(control.ref_time() - needed.ref_time(), 0)
    };
    let mut reservation = state
      .reserve(budget.limits(), BlockResourceDomain::ActorControl, prior)
      .unwrap();
    state.settle(&mut reservation, prior).unwrap();
  }
  state
}

fn bookkeeping() -> Weight {
  W::scheduler_on_idle_base() + W::block_resource_finalize()
}

fn inspection() -> Weight {
  W::service_round_begin_populated()
    + W::service_round_probe_eligible()
    + W::scheduler_actor_state_probe()
}

fn traced_idle(now: u32) -> (Weight, serde_json::Value) {
  h1_storage_trace::recorded(|| {
    let sample = h1_storage_trace::Sample::new();
    let used = sample.phase("capacity", || Actors::on_idle(now, Weight::MAX));
    (used, sample.finish().unwrap())
  })
}

#[test]
fn native_capacity_ledger_preserves_refused_source_and_distinguishes_total_custody() {
  for (ledger, reserve_eds, frozen_eds, untouchable_eds) in [
    ("plain", 0, 0, 0),
    ("freeze_dominant", 3, 10, 7),
    ("provider_ed", 10, 3, 1),
  ] {
    for short in [false, true] {
      for proof in [false, true] {
        common::synthetic_actor_test_ext().execute_with(|| {
          System::set_block_number(1);
          common::set_consensus_timestamp(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS);
          assert!(actors::OwnerSlotBitmaps::<Runtime>::get(common::ALICE).iter().all(|byte| *byte == 0));
          let payer = Actors::sovereign_account_id(&common::ALICE, 0);
          let steps = Default::default();
          let required = Actors::user_pipeline_machine_capacity_requirement(&steps).unwrap();
          let restricted = reserve_eds != 0;
          let reserved = reserve_eds * crate::EXISTENTIAL_DEPOSIT;
          let frozen = frozen_eds * crate::EXISTENTIAL_DEPOSIT;
          let untouchable = untouchable_eds * crate::EXISTENTIAL_DEPOSIT;
          // Reserve discounts frozen; a required provider may retain ED even with no residual freeze.
          let custody_buffer = reserved + untouchable;
          TmctolAssetOps::mint(&payer, AssetKind::Native, required + custody_buffer).unwrap();
          let id = Actors::next_actor_id();
          assert_ok!(Actors::create_user_actor(
            RuntimeOrigin::signed(common::ALICE), Mutability::Mutable,
            Some(actors::ActorContract {
              trigger: Trigger::Manual, cooldown_blocks: 0, window: None, steps,
              funding: FundingSourcePolicy::OwnerOnly, on_unfunded: OnUnfunded::Close,
              close_when: None, parked_balance_activation: None, auto_close_at_cycle_nonce: None,
            }),
          ));
          let quote = Actors::actor_cost_quote(id).unwrap();
          let pipeline = quote.prospective_pipeline_fee.unwrap().total_fee;
          let trigger = quote.prospective_trigger_fee.unwrap().fee;
          let floor = <Runtime as actors::Config>::MinUserBalance::get();
          assert_eq!(required, floor + pipeline);
          assert!(trigger > 0);
          TmctolAssetOps::mint(&payer, AssetKind::Native, trigger - Balance::from(short)).unwrap();
          if restricted {
            assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(&payer, reserved));
            <Balances as LockableCurrency<AccountId>>::set_lock(
              *b"cap-test", &payer, frozen, WithdrawReasons::all(),
            );
          }
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(common::ALICE), id));
          assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 2);
          let references = System::account(&payer);
          let provider_required = !System::can_dec_provider(&payer);
          assert_eq!(references.providers, 1, "Active creation must not fabricate a second provider");
          assert_eq!(provider_required, restricted);
          assert_eq!(references.consumers > 0, restricted);
          let reducible = TmctolAssetOps::balance(&payer, AssetKind::Native);
          assert_eq!(reducible, required - Balance::from(short), "Native spendability at {ledger}");
          assert_eq!(Balances::total_balance(&payer), required + custody_buffer - Balance::from(short));
          assert_eq!(references.data.free - reducible, untouchable);
          if ledger == "provider_ed" {
            assert_eq!(references.data.frozen.saturating_sub(references.data.reserved), 0);
            assert_eq!(untouchable, crate::EXISTENTIAL_DEPOSIT);
          }
          if restricted {
            assert!(Balances::total_balance(&payer) >= required);
            assert!(Balances::free_balance(&payer) >= required, "free custody also overstates spendability under the lock");
          }
          assert_eq!(Balances::reserved_balance(&payer), reserved);

          let prepared = resources(2, Some(proof));
          let before_usage = prepared.usage();
          actors::CurrentBlockResourceState::<Runtime>::put(prepared);
          let sink = <Runtime as actors::Config>::FeeSink::get();
          let (used, trace) = traced_idle(2);
          let changed = trace["owners"].as_object().unwrap().iter()
            .filter(|(_, stats)| stats["net_changed_keys"].as_u64().unwrap() != 0)
            .map(|(name, _)| name.as_str()).collect::<Vec<_>>();
          assert_eq!(changed, vec![
            "Actors.CurrentBlockResourceState", "Actors.FinalizedBlockResourceTelemetry",
            "Actors.IdleStarvationState",
          ], "no Actor/source, ring, hold, custody or fee key may change");
          assert_eq!(trace["total"]["net_changed_keys"], 3);
          assert!(trace["owners"]["System.Account"]["calls"].as_object().unwrap().keys()
            .all(|method| matches!(method.as_str(), "Get" | "Exists" | "Hash")),
            "physical admission refusal cannot attempt a native-ledger write");
          assert_eq!(used, bookkeeping() + inspection());
          let refused = Actors::block_resource_state().unwrap();
          assert_eq!(refused.phase(), BlockResourcePhase::Finalizable);
          assert_eq!(refused.usage().actor_control_used(), before_usage.actor_control_used() + used);
          assert_eq!(refused.usage().actor_effect_used(), Weight::zero());
          assert_eq!(refused.outstanding_reservations(), 0);
          assert!(!refused.optional_actor_work_halted());
          assert!(trace["owners"]["System.Account"]["calls"]["Get"].as_u64().unwrap() >= 1);
          Actors::on_finalize(2);

          actors::CurrentBlockResourceState::<Runtime>::put(resources(3, None));
          let custody = System::account(&payer).encode();
          let sink_before = Balances::free_balance(&sink);
          let (used, recovery_trace) = traced_idle(3);
          let recovered = Actors::block_resource_state().unwrap();
          assert_eq!(recovered.phase(), BlockResourcePhase::Finalizable);
          assert!(!recovered.optional_actor_work_halted());
          assert_eq!(recovered.usage().actor_control_used(), used);
          assert_eq!(recovered.usage().actor_effect_used(), Weight::zero());
          assert_eq!(recovered.outstanding_reservations(), 0);
          if short {
            assert!(Actors::active_actor_state(id).is_none(), "reducible shortfall selects minimal apoptosis, not a Pipeline opening");
            assert_eq!(System::account(&payer).encode(), custody, "close does not unwind reserved or frozen custody");
            assert_eq!(Balances::free_balance(&sink), sink_before);
            assert!(System::events().iter().any(|record| matches!(
              record.event, RuntimeEvent::Actors(actors::Event::ActorClosed {
                actor_id, reason: actors::CloseReason::CycleAdmissionInsufficient
              }) if actor_id == id
            )));
          } else {
            let state = Actors::active_actor_state(id).unwrap();
            assert!(!state.hot.pending_signal && state.run_state.is_none());
            assert_eq!(state.identity.cycle_nonce, 1);
            assert_eq!(TmctolAssetOps::balance(&payer, AssetKind::Native), floor);
            assert_eq!(Balances::free_balance(&sink), sink_before + pipeline);
          }
          assert_eq!(Balances::reserved_balance(&payer), reserved);
          assert_eq!(System::account(&payer).data.frozen, frozen);
          Actors::on_finalize(3);
          eprintln!("NATIVE_CAPACITY_LEDGER {}", json!({
            "ledger": ledger, "restricted": restricted, "short": short, "proof_axis": proof,
            "required": required.to_string(), "reserved": reserved.to_string(), "frozen": frozen.to_string(),
            "untouchable": untouchable.to_string(), "providers": references.providers,
            "consumers": references.consumers, "provider_required": provider_required,
            "refusal": trace, "recovery": recovery_trace,
            "scope": "Native Currency API fixture and synthetic resource preparation; no authenticated restriction-ingress, physical-bound or Full Executive claim",
          }));
        });
      }
    }
  }
}

fn full_capacity_contract() -> actors::ActorContractOf<Runtime> {
  let count = <Runtime as actors::Config>::MaxWhitelistSize::get();
  let mut sources = (1..=count)
    .map(|index| AccountId::new([u8::try_from(index).unwrap(); 32]))
    .collect::<Vec<_>>();
  sources.sort_by_key(Encode::encode);
  assert!(sources.contains(&common::ALICE));
  let mut assets = Vec::new();
  for index in 1..=count {
    let id = index;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      id,
      polkadot_sdk::sp_runtime::MultiAddress::Id(common::ALICE),
      true,
      1,
    ));
    assets.push(AssetKind::Local(id));
  }
  assets.sort_by_key(Encode::encode);
  let predicates = <Runtime as actors::Config>::MaxPredicatesPerStep::get().min(
    <Runtime as actors::Config>::MaxPreconditionClauses::get()
      * <Runtime as actors::Config>::MaxPredicatesPerClause::get(),
  );
  let mut precondition = actors::PreconditionOf::<Runtime> {
    clauses: (1..=predicates)
      .map(|index| {
        let asset_in = AssetKind::Local(0x3000_0000 + index);
        let asset_out = AssetKind::Local(0x3001_0000 + index);
        assert_ok!(common::publish_deos_router_observation(
          asset_in, asset_out, 1
        ));
        let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
        assert_eq!(
          feed.encoded_size(),
          primitives::OracleFeedId::max_encoded_len()
        );
        vec![actors::Predicate::ObservationAbove {
          feed,
          threshold: 0,
          max_age_blocks: 100,
        }]
        .try_into()
        .unwrap()
      })
      .collect::<Vec<_>>()
      .try_into()
      .unwrap(),
  };
  precondition.clauses.sort_by_key(Encode::encode);
  let asset = assets[0];
  let step = actors::Step {
    precondition: Some(precondition),
    task: actors::Task::RemoveLiquidity {
      lp_asset: asset,
      asset_a: asset,
      asset_b: asset,
      lp_amount: actors::AmountResolution::Fixed(1),
      min_amount_a: 1,
      min_amount_b: 1,
    },
    on_error: actors::StepErrorPolicy::RetryLater {
      max_attempts: <Runtime as actors::Config>::MaxRetryAttempts::get(),
    },
  };
  assert_eq!(
    step.task.encoded_size(),
    actors::TaskOf::<Runtime>::max_encoded_len()
  );
  let contract = actors::ActorContract {
    trigger: Trigger::AddressEvent {
      source_filter: actors::SourceFilter::Whitelist(sources.clone().try_into().unwrap()),
      asset_filter: actors::AssetFilter::Whitelist(assets.try_into().unwrap()),
    },
    cooldown_blocks: <Runtime as actors::Config>::MaxExecutionDelayBlocks::get(),
    window: Some(actors::ScheduleWindow {
      start: 1,
      end: <Runtime as actors::Config>::MinWindowLength::get() + 2,
    }),
    steps: vec![step; <Runtime as actors::Config>::MaxContractSteps::get() as usize]
      .try_into()
      .unwrap(),
    funding: FundingSourcePolicy::SignedAllowlist(
      sources
        .into_iter()
        .collect::<BTreeSet<_>>()
        .try_into()
        .unwrap(),
    ),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: None,
    auto_close_at_cycle_nonce: Some(<Runtime as actors::Config>::MaxAutoCloseNonceHorizon::get()),
  };
  assert_eq!(
    contract.trigger.encoded_size(),
    actors::TriggerOf::<Runtime>::max_encoded_len()
  );
  assert_eq!(
    contract.funding.encoded_size(),
    actors::FundingSourcePolicyOf::<Runtime>::max_encoded_len()
  );
  contract
}

#[test]
fn native_capacity_full_head_preserves_provider_ed_source_on_refusal() {
  for short in [false, true] {
    for proof in [false, true] {
      common::synthetic_actor_test_ext().execute_with(|| {
        System::set_block_number(1);
        common::set_consensus_timestamp(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS);
        let contract = full_capacity_contract();
        let payer = Actors::sovereign_account_id(&common::ALICE, 0);
        let floor = <Runtime as actors::Config>::MinUserBalance::get();
        TmctolAssetOps::mint(&payer, AssetKind::Native, floor).unwrap();
        let id = Actors::next_actor_id();
        let creation = Actors::create_user_actor(
          RuntimeOrigin::signed(common::ALICE), Mutability::Mutable, Some(contract.clone()),
        );
        eprintln!("NATIVE_FULL_HEAD_ADMISSION {:?}", creation);
        assert_ok!(creation);
        assert_eq!(Actors::actor_contract(id).unwrap().encode(), contract.encode());
        let head = actors::ActorContractHeads::<Runtime>::get(id).unwrap();
        assert_eq!(head.header.encoded_size(),
          actors::ActorContractHeaderOf::<Runtime>::max_encoded_len()
            - actors::ParkedBalanceActivationOf::<Runtime>::max_encoded_len());
        assert_eq!(head.header.step_count, <Runtime as actors::Config>::MaxContractSteps::get());
        let first = head.first_step.as_ref().unwrap();
        assert_eq!(first.task.encoded_size(), actors::TaskOf::<Runtime>::max_encoded_len());
        assert_eq!(first.precondition.as_ref().unwrap().predicate_count(),
          <Runtime as actors::Config>::MaxPredicatesPerStep::get());
        let chunks = actors::ActorContractTailChunks::<Runtime>::iter_prefix(id).count();
        assert_eq!(chunks, (head.header.step_count - 1).div_ceil(actors::MAX_STEPS_PER_TAIL_CHUNK) as usize);
        let quote = Actors::actor_cost_quote(id).unwrap();
        let pipeline = quote.prospective_pipeline_fee.unwrap().total_fee;
        let trigger = quote.prospective_trigger_fee.unwrap().fee;
        let required = floor + pipeline;
        let reserved = 10 * crate::EXISTENTIAL_DEPOSIT;
        let frozen = 3 * crate::EXISTENTIAL_DEPOSIT;
        assert!(trigger > 0);
        TmctolAssetOps::mint(&payer, AssetKind::Native,
          pipeline + trigger + reserved + crate::EXISTENTIAL_DEPOSIT - Balance::from(short)).unwrap();
        assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(&payer, reserved));
        <Balances as LockableCurrency<AccountId>>::set_lock(*b"cap-test", &payer, frozen, WithdrawReasons::all());
        let asset = 1;
        assert_ok!(Assets::mint(RuntimeOrigin::signed(common::ALICE), asset,
          polkadot_sdk::sp_runtime::MultiAddress::Id(common::ALICE), 1));
        use crate::configs::address_event_ingress::AddressEventIngressExtension;
        use polkadot_sdk::frame_support::{dispatch::GetDispatchInfo, storage::TransactionOutcome};
        use polkadot_sdk::sp_runtime::traits::{Dispatchable, TransactionExtension};
        let call = crate::RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
          id: asset, target: crate::Address::Id(payer.clone()), amount: 1,
        });
        assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(|| {
          let origin = RuntimeOrigin::signed(common::ALICE);
          let info = call.get_dispatch_info();
          let pre = AddressEventIngressExtension.prepare((), &origin, &call, &info, 0)
            .expect("certified signed transfer preparation");
          let (post, result) = match call.dispatch(origin) {
            Ok(post) => (post, Ok(())),
            Err(error) => (error.post_info, Err(error.error)),
          };
          AddressEventIngressExtension::post_dispatch_details(pre, &info, &post, 0, &result)
            .expect("certified signed transfer consequence");
          if result.is_ok() { TransactionOutcome::Commit(result) }
          else { TransactionOutcome::Rollback(result) }
        }));
        assert_eq!(TmctolAssetOps::balance(&payer, AssetKind::Native), required - Balance::from(short));
        let references = System::account(&payer);
        assert_eq!((references.providers, references.consumers), (1, 1));
        assert!(!System::can_dec_provider(&payer));
        assert_eq!(references.data.frozen.saturating_sub(references.data.reserved), 0);
        let state = Actors::active_actor_state(id).unwrap();
        assert!(state.hot.pending_signal && state.run_state.is_none());
        assert_eq!(state.identity.cycle_nonce, 0);
        assert!(actors::ActorStateHolds::<Runtime>::contains_key(id));
        assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 2);
        let prepared = resources(2, Some(proof));
        let prior = prepared.usage().actor_control_used();
        actors::CurrentBlockResourceState::<Runtime>::put(prepared);
        let (used, trace) = traced_idle(2);
        let changed = trace["owners"].as_object().unwrap().iter()
          .filter(|(_, stats)| stats["net_changed_keys"].as_u64().unwrap() != 0)
          .map(|(name, _)| name.as_str()).collect::<Vec<_>>();
        assert_eq!(changed, vec!["Actors.CurrentBlockResourceState",
          "Actors.FinalizedBlockResourceTelemetry", "Actors.IdleStarvationState"]);
        assert_eq!(trace["total"]["net_changed_keys"], 3);
        let native = &trace["owners"]["System.Account"]["calls"];
        assert!(native["Get"].as_u64().unwrap_or(0) >= 1,
          "canonical full-Head inspection must actually read Native spendability");
        assert!(native.as_object().unwrap().keys()
          .all(|method| matches!(method.as_str(), "Get" | "Exists" | "Hash")));
        assert!(trace["owners"]["Actors.ActorContractHead"]["get_value_bytes"].as_u64().unwrap()
          >= head.encoded_size() as u64);
        assert!(trace["owners"].get("Actors.ActorContractTailChunks").is_none(),
          "inspection cannot load unreached tails");
        assert!(trace["owners"].as_object().unwrap().keys().all(|name| !name.starts_with("Oracle.")),
          "inspection cannot evaluate Step observations before complete admission");
        assert_eq!(used, bookkeeping() + inspection());
        let refused = Actors::block_resource_state().unwrap();
        assert_eq!(refused.phase(), BlockResourcePhase::Finalizable);
        assert_eq!(refused.usage().actor_control_used(), prior + used);
        assert_eq!(refused.usage().actor_effect_used(), Weight::zero());
        assert_eq!(refused.outstanding_reservations(), 0);
        assert!(!refused.optional_actor_work_halted());
        Actors::on_finalize(2);
        eprintln!("NATIVE_FULL_HEAD_CAPACITY {}", json!({
          "short": short, "proof_axis": proof, "head_bytes": head.encoded_size(),
          "header_bytes": head.header.encoded_size(), "first_step_bytes": first.encoded_size(),
          "task_bytes": first.task.encoded_size(), "steps": head.header.step_count,
          "tail_chunks": chunks, "pipeline_fee": pipeline.to_string(),
          "required": required.to_string(), "providers": references.providers,
          "consumers": references.consumers, "trace": trace,
          "scope": "Ordinary native public admission/paid AddressEvent/provider ED and synthetic resource refusal; no Task success, physical I/K or Full Executive claim",
        }));
      });
    }
  }
}
