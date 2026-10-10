//! Current-runtime Actor projections and rolled-back public lifecycle probes over four TOP cuts.

use codec::{Decode, Encode};
use deos_runtime::{
  AccountId, Actors, Address, Assets, Balances, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
  Signature, System, TransactionPayment, TxExtension, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorContractHeads, ActorContractOf, ActorProcesses, ActorSemanticState, ActorSemanticStates,
  ActorStateHolds, ActorType, AssetOps, BalanceDependencySources, DependencyPlans,
  DependencyRegistrationHeaders, DependencyRegistrationPositions, DependencySourceAllocatorState,
  FundingSourcePolicy, Mutability, OnUnfunded, ParkedBalanceActivationOf, ParkedBalanceEpisodes,
  ParkedBalanceRule, ProcessResidence, ServiceHeader, SovereignIndex, StepControlExecution,
  StepControlOutcome, StepControlPhase, StepControlPlacement, StepControlWeightContext,
  StepControlWeightProvider, StepErrorPolicy, StepOf, Task, TaskEffectExecution,
  TaskEffectWeightProvider, Trigger, TriggerDeadlineHandles, WeightInfo,
  classify_parked_balance_plan,
};
use polkadot_sdk::{
  frame_support::{storage::TransactionOutcome, traits::Get, weights::Weight},
  sp_core::{
    Pair,
    storage::{Storage, well_known_keys},
  },
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    generic,
    traits::{Applyable, Checkable},
    transaction_validity::TransactionSource,
  },
  sp_weights::WeightToFee,
};
use primitives::AssetKind;
use serde_json::{Value, json};
use std::collections::BTreeMap;

type HostAssetOps = <Runtime as pallet_deos_actors::Config>::AssetOps;
type ControlWeights = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
type EffectWeights = <Runtime as pallet_deos_actors::Config>::TaskEffectWeight;
type ActorWeights = <Runtime as pallet_deos_actors::Config>::WeightInfo;

fn pair(weight: Weight) -> [u64; 2] {
  [weight.ref_time(), weight.proof_size()]
}

fn callback_protocol_probes(maximum_keys: usize) -> Value {
  use polkadot_sdk::frame_support::{
    migrations::MultiStepMigrator,
    traits::{OnIdle, OnPoll},
    weights::WeightMeter,
  };
  type SystemConfig = Runtime;
  assert_eq!(
    std::any::type_name::<<SystemConfig as polkadot_sdk::frame_system::Config>::PreInherents>(),
    "()"
  );
  assert_eq!(
    std::any::type_name::<<SystemConfig as polkadot_sdk::frame_system::Config>::PostInherents>(),
    "()"
  );
  assert_eq!(
    std::any::type_name::<<SystemConfig as polkadot_sdk::frame_system::Config>::PostTransactions>(),
    "()"
  );
  assert!(!<SystemConfig as polkadot_sdk::frame_system::Config>::MultiBlockMigrator::ongoing());
  let last = polkadot_sdk::frame_system::LastRuntimeUpgrade::<Runtime>::get().unwrap();
  assert!(!last.was_upgraded(&<Runtime as polkadot_sdk::frame_system::Config>::Version::get()));
  let now = System::block_number().checked_add(1).unwrap();
  let limit = <Runtime as polkadot_sdk::frame_system::Config>::BlockWeights::get().max_block;
  let callbacks: [(&str, &str, fn(u32, Weight) -> Weight); 26] = [
    (
      "System",
      std::any::type_name::<deos_runtime::System>(),
      <deos_runtime::System as OnIdle<u32>>::on_idle,
    ),
    (
      "ParachainSystem",
      std::any::type_name::<deos_runtime::ParachainSystem>(),
      <deos_runtime::ParachainSystem as OnIdle<u32>>::on_idle,
    ),
    (
      "Timestamp",
      std::any::type_name::<deos_runtime::Timestamp>(),
      <deos_runtime::Timestamp as OnIdle<u32>>::on_idle,
    ),
    (
      "ParachainInfo",
      std::any::type_name::<deos_runtime::ParachainInfo>(),
      <deos_runtime::ParachainInfo as OnIdle<u32>>::on_idle,
    ),
    (
      "WeightReclaim",
      std::any::type_name::<deos_runtime::WeightReclaim>(),
      <deos_runtime::WeightReclaim as OnIdle<u32>>::on_idle,
    ),
    (
      "Balances",
      std::any::type_name::<deos_runtime::Balances>(),
      <deos_runtime::Balances as OnIdle<u32>>::on_idle,
    ),
    (
      "TransactionPayment",
      std::any::type_name::<deos_runtime::TransactionPayment>(),
      <deos_runtime::TransactionPayment as OnIdle<u32>>::on_idle,
    ),
    (
      "Authorship",
      std::any::type_name::<deos_runtime::Authorship>(),
      <deos_runtime::Authorship as OnIdle<u32>>::on_idle,
    ),
    (
      "CollatorSelection",
      std::any::type_name::<deos_runtime::CollatorSelection>(),
      <deos_runtime::CollatorSelection as OnIdle<u32>>::on_idle,
    ),
    (
      "Session",
      std::any::type_name::<deos_runtime::Session>(),
      <deos_runtime::Session as OnIdle<u32>>::on_idle,
    ),
    (
      "SessionRotation",
      std::any::type_name::<deos_runtime::SessionRotation>(),
      <deos_runtime::SessionRotation as OnIdle<u32>>::on_idle,
    ),
    (
      "Aura",
      std::any::type_name::<deos_runtime::Aura>(),
      <deos_runtime::Aura as OnIdle<u32>>::on_idle,
    ),
    (
      "AuraExt",
      std::any::type_name::<deos_runtime::AuraExt>(),
      <deos_runtime::AuraExt as OnIdle<u32>>::on_idle,
    ),
    (
      "XcmpQueue",
      std::any::type_name::<deos_runtime::XcmpQueue>(),
      <deos_runtime::XcmpQueue as OnIdle<u32>>::on_idle,
    ),
    (
      "PolkadotXcm",
      std::any::type_name::<deos_runtime::PolkadotXcm>(),
      <deos_runtime::PolkadotXcm as OnIdle<u32>>::on_idle,
    ),
    (
      "CumulusXcm",
      std::any::type_name::<deos_runtime::CumulusXcm>(),
      <deos_runtime::CumulusXcm as OnIdle<u32>>::on_idle,
    ),
    (
      "MessageQueue",
      std::any::type_name::<deos_runtime::MessageQueue>(),
      <deos_runtime::MessageQueue as OnIdle<u32>>::on_idle,
    ),
    (
      "Assets",
      std::any::type_name::<deos_runtime::Assets>(),
      <deos_runtime::Assets as OnIdle<u32>>::on_idle,
    ),
    (
      "AssetConversion",
      std::any::type_name::<deos_runtime::AssetConversion>(),
      <deos_runtime::AssetConversion as OnIdle<u32>>::on_idle,
    ),
    (
      "AssetRegistry",
      std::any::type_name::<deos_runtime::AssetRegistry>(),
      <deos_runtime::AssetRegistry as OnIdle<u32>>::on_idle,
    ),
    (
      "Staking",
      std::any::type_name::<deos_runtime::Staking>(),
      <deos_runtime::Staking as OnIdle<u32>>::on_idle,
    ),
    (
      "Governance",
      std::any::type_name::<deos_runtime::Governance>(),
      <deos_runtime::Governance as OnIdle<u32>>::on_idle,
    ),
    (
      "Preimage",
      std::any::type_name::<deos_runtime::Preimage>(),
      <deos_runtime::Preimage as OnIdle<u32>>::on_idle,
    ),
    (
      "DeosRouter",
      std::any::type_name::<deos_runtime::DeosRouter>(),
      <deos_runtime::DeosRouter as OnIdle<u32>>::on_idle,
    ),
    (
      "TokenMintingCurve",
      std::any::type_name::<deos_runtime::TokenMintingCurve>(),
      <deos_runtime::TokenMintingCurve as OnIdle<u32>>::on_idle,
    ),
    (
      "Oracle",
      std::any::type_name::<deos_runtime::Oracle>(),
      <deos_runtime::Oracle as OnIdle<u32>>::on_idle,
    ),
  ];
  let before = current_top(maximum_keys);
  let mut values = Vec::new();
  let mut total = Weight::zero();
  for (name, hook_type, hook) in callbacks {
    let weight = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let callback_before = current_top(maximum_keys + 1);
      let weight = hook(now, limit);
      assert!(
        current_top(maximum_keys + 1) == callback_before,
        "quiet non-Actor idle hook changed TOP: {name}"
      );
      TransactionOutcome::Rollback(weight)
    });
    assert!(current_top(maximum_keys) == before);
    assert!(weight.all_lte(limit));
    total = total.checked_add(&weight).unwrap();
    values.push(json!({"pallet": name, "type": hook_type, "returnedWeight": pair(weight), "topUnchanged": true}));
  }
  let xcmp_zero = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    let callback_before = current_top(maximum_keys + 1);
    let weight = <deos_runtime::XcmpQueue as OnIdle<u32>>::on_idle(now, Weight::zero());
    assert_eq!(weight, Weight::zero());
    assert!(current_top(maximum_keys + 1) == callback_before);
    TransactionOutcome::Rollback(pair(weight))
  });
  assert!(current_top(maximum_keys) == before);
  let poll = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    let callback_before = current_top(maximum_keys + 1);
    let mut meter = WeightMeter::with_limit(limit);
    <deos_runtime::AllPalletsWithSystem as OnPoll<u32>>::on_poll(now, &mut meter);
    assert_eq!(meter.consumed(), Weight::zero());
    assert!(current_top(maximum_keys + 1) == callback_before);
    TransactionOutcome::Rollback(pair(meter.consumed()))
  });
  assert!(current_top(maximum_keys) == before);
  json!({"scope": "native_quiet_callback_types_and_rolled_back_idle_poll_probes", "hookCount": 27,
    "allHooksType": std::any::type_name::<deos_runtime::AllPalletsWithSystem>(), "actorHookType": std::any::type_name::<Actors>(),
    "preInherents": "()", "postInherents": "()", "postTransactions": "()", "migrationOngoing": false, "runtimeUpgradeDue": false,
    "idleHooks": values, "nonActorIdleReturnedTotal": pair(total), "xcmpZeroLimitReturned": xcmp_zero,
    "allPalletsPollReturned": poll, "topUnchanged": true, "executiveBlocks": 0})
}

fn signed_probe_call(
  signer: &polkadot_sdk::sp_core::sr25519::Pair,
  call: RuntimeCall,
) -> UncheckedExtrinsic {
  let owner = AccountId::from(signer.public());
  let bytes = (
    polkadot_sdk::frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&owner)),
    polkadot_sdk::pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  )
    .encode();
  let mut input = &bytes[..];
  let extension = TxExtension::decode(&mut input).unwrap();
  assert!(input.is_empty());
  let payload = generic::SignedPayload::new(call.clone(), extension.clone()).unwrap();
  let signature = payload.using_encoded(|encoded| signer.sign(encoded));
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(owner),
    Signature::Sr25519(signature),
    extension,
  )
}

fn signed_caller_probes() -> Value {
  use polkadot_sdk::frame_support::dispatch::GetDispatchInfo;
  let now = System::block_number();
  let service = ServiceHeader::<Runtime>::get();
  let service_semantics =
    [1, 16, 17, 18, 19].map(|actor| ActorSemanticStates::<Runtime>::get(actor).unwrap().encode());
  let genesis_hash = System::block_hash(0);
  let mut cases = Vec::new();
  for (actor, signer) in [
    (20, Sr25519Keyring::Alice.pair()),
    (21, Sr25519Keyring::Bob.pair()),
  ] {
    let owner = AccountId::from(signer.public());
    assert_eq!(Actors::actor_identity(actor).unwrap().owner, owner);
    for prepared in [false, true] {
      let case = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let number = if prepared { 2_552_520_733 } else { now + 1 };
        if prepared {
          polkadot_sdk::frame_system::Account::<Runtime>::mutate(&owner, |account| {
            account.nonce = 510_457_082
          });
        }
        let nonce = System::account_nonce(&owner);
        assert_eq!(System::block_hash(0), genesis_hash);
        let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
        let before_owner = Balances::free_balance(&owner) + Balances::reserved_balance(&owner);
        let before_sink = Balances::free_balance(&sink);
        let before_hold = ActorStateHolds::<Runtime>::get(actor).unwrap();
        let mut fees = 0u128;
        let mut declared = Weight::zero();
        let mut calls = Vec::new();
        for call in [
          RuntimeCall::Actors(pallet_deos_actors::Call::activate_actor {
            actor_id: actor,
            contract: parked_probe_contract(15, 1),
          }),
          RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor { actor_id: actor }),
        ] {
          let block = number + 5 * calls.len() as u32;
          System::set_block_number(block - 1);
          System::initialize(&block, &Default::default(), &Default::default());
          System::note_finished_initialize();
          assert_eq!(System::block_hash(0), genesis_hash);
          let mut state = pallet_deos_actors::BlockResourceState::new(block);
          state
            .begin_prepass(<Runtime as pallet_deos_actors::Config>::BlockResourceBudget::get())
            .unwrap();
          state.open_external_phase().unwrap();
          pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
          let transaction = signed_probe_call(&signer, call);
          let len = transaction.encoded_size();
          let checked = transaction
            .check(&polkadot_sdk::frame_system::ChainContext::<Runtime>::default())
            .unwrap();
          let info = checked.get_dispatch_info();
          let quoted_fee = TransactionPayment::compute_fee(len as u32, &info, 0);
          declared = declared
            .checked_add(
              &polkadot_sdk::frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
                &<Runtime as polkadot_sdk::frame_system::Config>::BlockWeights::get(),
                &info,
                len,
              ),
            )
            .unwrap();
          assert!(
            checked
              .validate::<Runtime>(TransactionSource::InBlock, &info, len)
              .is_ok()
          );
          let applied = checked.apply::<Runtime>(&info, len).unwrap();
          assert!(applied.is_ok(), "signed lifecycle dispatch: {applied:?}");
          let RuntimeEvent::TransactionPayment(
            polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
              who,
              actual_fee,
              tip,
            },
          ) = System::events().last().unwrap().event.clone()
          else {
            panic!("signed pipeline must finish payment")
          };
          assert_eq!(who, owner);
          assert_eq!(tip, 0);
          assert!(actual_fee > 0 && actual_fee <= quoted_fee);
          fees = fees.checked_add(actual_fee).unwrap();
          calls.push(json!({"block": block, "encodedBytes": len, "quotedFee": quoted_fee.to_string(), "actualFee": actual_fee.to_string(),
            "userUsed": pair(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get().unwrap().usage().user_dispatch_used())}));
          assert_eq!(
            Actors::active_actor_state(actor).is_some(),
            calls.len() == 1
          );
          assert_eq!(
            pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
              .unwrap()
              .outstanding_reservations(),
            0
          );
        }
        let state = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get().unwrap();
        assert!(state.usage().user_dispatch_used().all_lte(declared));
        assert_eq!(System::account_nonce(&owner), nonce.checked_add(2).unwrap());
        assert_eq!(ActorStateHolds::<Runtime>::get(actor).unwrap(), before_hold);
        assert_eq!(
          before_owner - Balances::free_balance(&owner) - Balances::reserved_balance(&owner),
          fees
        );
        assert_eq!(Balances::free_balance(&sink) - before_sink, fees);
        assert_eq!(ServiceHeader::<Runtime>::get(), service);
        assert_eq!(
          [1, 16, 17, 18, 19].map(|member| ActorSemanticStates::<Runtime>::get(member)
            .unwrap()
            .encode()),
          service_semantics
        );
        let result = json!({"actor": actor, "owner": hex(&owner.encode()), "prepared": prepared, "block": number,
          "nonceBefore": nonce, "nonceAfter": System::account_nonce(&owner), "calls": calls,
          "userUsed": pair(state.usage().user_dispatch_used()), "declaredUserUpper": pair(declared),
          "feesCollected": fees.to_string(), "outstandingReservations": state.outstanding_reservations(),
          "genesisHashPreserved": true, "serviceSemanticsPreserved": true, "dormantHoldRestored": true});
        TransactionOutcome::Rollback(result)
      });
      cases.push(case);
    }
  }
  json!({"scope": "native_signed_extension_application_with_prepared_system_and_external_phase", "cases": cases,
    "signedCalls": 8, "runtimeHooks": false, "executiveBlocks": 0, "proofRecorder": false})
}

fn parked_probe_contract(mask: u8, steps: usize) -> ActorContractOf<Runtime> {
  assert!((1..16).contains(&mask) && matches!(steps, 1 | 12));
  let activation = ParkedBalanceActivationOf::<Runtime>::try_from_rules(
    (0..4)
      .filter(|index| mask & (1 << index) != 0)
      .map(|index| ParkedBalanceRule {
        asset: AssetKind::Local(0x7000_0001 + index),
        authored_min_delta: 100,
      })
      .collect(),
  )
  .unwrap();
  ActorContractOf::<Runtime> {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![
      StepOf::<Runtime> {
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
    parked_balance_activation: Some(activation),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  }
}

fn lifecycle_caller_probes() -> Value {
  let now = System::block_number();
  let original = [20, 21].map(|actor| {
    let Some(ActorSemanticState::Dormant(record)) = ActorSemanticStates::<Runtime>::get(actor)
    else {
      panic!("epoch seed must retain a Dormant identity")
    };
    record
  });
  let next_id = Actors::next_actor_id();
  let active_count = Actors::active_actor_count();
  let identity_count = Actors::actor_identity_count();
  let service = ServiceHeader::<Runtime>::get();
  let service_semantics =
    [1, 16, 17, 18, 19].map(|actor| ActorSemanticStates::<Runtime>::get(actor).unwrap().encode());
  let sources = [0, 1, 2, 3].map(|index| {
    BalanceDependencySources::<Runtime>::get(AssetKind::Local(0x7000_0001 + index)).unwrap()
  });
  let allocator = DependencySourceAllocatorState::<Runtime>::get();
  let mut results = Vec::new();
  for actor in [20, 21] {
    for (mask, prepared_counter) in (1..16).map(|mask| (mask, false)).chain([(15, true)]) {
      let mut record = original[(actor - 20) as usize].clone();
      if prepared_counter {
        record.generation = 510_456_672;
      }
      let contract = parked_probe_contract(mask, 1);
      let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        if prepared_counter {
          ActorSemanticStates::<Runtime>::insert(
            actor,
            ActorSemanticState::Dormant(record.clone()),
          );
        }
        System::set_block_number(now.checked_add(1).unwrap());
        let old_hold = ActorStateHolds::<Runtime>::get(actor).unwrap();
        let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
        let sink_before = Balances::free_balance(&sink);
        let native_before = Balances::free_balance(&record.identity.owner)
          + Balances::reserved_balance(&record.identity.owner);
        let frontiers = sources
          .iter(/* deos-bypass: bounded-iter -- The fixed four-LP source array. */)
          .map(|source| DependencyRegistrationHeaders::<Runtime>::get(source).next_index)
          .collect::<Vec<_>>();
        assert!(
          Actors::activate_actor(
            RuntimeOrigin::signed(record.identity.owner.clone()),
            actor,
            contract.clone()
          )
          .is_ok()
        );
        let active = Actors::active_actor_state(actor).unwrap();
        assert_eq!(active.contract, contract);
        assert_eq!(active.identity, Actors::actor_identity(actor).unwrap());
        let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<Runtime>::get(actor)
        else {
          panic!("activated semantic owner")
        };
        assert_eq!(
          semantic.generation,
          record.generation.checked_add(1).unwrap()
        );
        let process = ActorProcesses::<Runtime>::get(actor).unwrap();
        assert!(matches!(
          process.residence,
          Some(ProcessResidence::Parked(_))
        ));
        assert_eq!(process.generation, semantic.generation);
        assert!(active.run_state.is_none() && !active.hot.pending_signal);
        assert!(
          active.hot.trigger_wakeup_pointer.is_none()
            && TriggerDeadlineHandles::<Runtime>::get(actor).is_none()
        );
        assert_eq!(Actors::active_actor_count(), active_count + 1);
        assert_eq!(Actors::next_actor_id(), next_id);
        assert_eq!(Actors::actor_identity_count(), identity_count);
        assert_eq!(ServiceHeader::<Runtime>::get(), service);
        assert_eq!(
          [1, 16, 17, 18, 19]
            .map(|actor| ActorSemanticStates::<Runtime>::get(actor).unwrap().encode()),
          service_semantics
        );
        assert_eq!(DependencySourceAllocatorState::<Runtime>::get(), allocator);
        let episode = ParkedBalanceEpisodes::<Runtime>::get(actor).unwrap();
        assert_eq!(episode.watches.len(), mask.count_ones() as usize);
        assert_eq!(
          classify_parked_balance_plan(episode.watches.as_slice(), |asset| (
            HostAssetOps::minimum_balance(asset),
            HostAssetOps::total_balance(&record.identity.sovereign_account, asset)
          ))
          .unwrap(),
          pallet_deos_actors::ParkedBalanceQualification::BelowThreshold
        );
        let mut positions = Vec::new();
        for (index, source) in sources.iter(/* deos-bypass: bounded-iter -- The fixed four-LP source array. */).enumerate()
        {
          let selected = mask & (1 << index) != 0;
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          assert_eq!(header.next_index, frontiers[index] + u64::from(selected));
          let position = DependencyRegistrationPositions::<Runtime>::get(source, actor);
          assert_eq!(position.is_some(), selected);
          if let Some(position) = position {
            assert_eq!(
              position.page * 32 + u64::from(position.slot),
              frontiers[index]
            );
          }
          positions.push(format!("{position:?}"));
        }
        let hold = ActorStateHolds::<Runtime>::get(actor).unwrap();
        assert_eq!(hold.owner, record.identity.owner);
        let quote = Actors::actor_cost_quote(actor).unwrap();
        assert_eq!(Balances::free_balance(&sink), sink_before);
        assert_eq!(
          Balances::free_balance(&record.identity.owner)
            + Balances::reserved_balance(&record.identity.owner),
          native_before
        );
        System::set_block_number(now.checked_add(6).unwrap());
        assert!(
          Actors::deactivate_actor(RuntimeOrigin::signed(record.identity.owner.clone()), actor)
            .is_ok()
        );
        let Some(ActorSemanticState::Dormant(dormant)) = ActorSemanticStates::<Runtime>::get(actor)
        else {
          panic!("deactivated semantic owner")
        };
        assert_eq!(dormant.generation, semantic.generation);
        assert_eq!(dormant.identity.cycle_nonce, record.identity.cycle_nonce);
        assert_eq!(
          dormant.identity.sovereign_account,
          record.identity.sovereign_account
        );
        assert_eq!(
          ActorStateHolds::<Runtime>::get(actor).unwrap().breakdown,
          old_hold.breakdown
        );
        assert_eq!(Actors::active_actor_count(), active_count);
        assert_eq!(Actors::next_actor_id(), next_id);
        assert_eq!(Actors::actor_identity_count(), identity_count);
        assert_eq!(ServiceHeader::<Runtime>::get(), service);
        assert_eq!(
          [1, 16, 17, 18, 19]
            .map(|actor| ActorSemanticStates::<Runtime>::get(actor).unwrap().encode()),
          service_semantics
        );
        assert!(
          Actors::actor_contract(actor).is_none()
            && ActorProcesses::<Runtime>::get(actor).is_none()
            && ParkedBalanceEpisodes::<Runtime>::get(actor).is_none()
        );
        assert!(
          sources
            .iter(/* deos-bypass: bounded-iter -- The fixed four-LP source array. */)
            .all(|source| DependencyRegistrationPositions::<Runtime>::get(source, actor).is_none())
        );
        assert_eq!(DependencySourceAllocatorState::<Runtime>::get(), allocator);
        TransactionOutcome::Rollback(
          json!({"actor": actor, "mask": mask, "preparedCounterBoundary": prepared_counter, "generationBefore": record.generation, "generationAfter": dormant.generation, "positions": positions, "holdTotal": quote.actor_state_hold.total.to_string(), "holdBreakdown": format!("{:?}", hold.breakdown), "prospectivePipelineFee": quote.prospective_pipeline_fee.unwrap().total_fee.to_string(), "noActorFeeCollected": true, "dormantHoldRestored": true, "nativeOwnerTotalPreserved": true}),
        )
      });
      results.push(result);
    }
  }
  let birth = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    System::set_block_number(now.checked_add(1).unwrap());
    let owner = &original[0].identity.owner;
    let contract = parked_probe_contract(15, 12);
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let sink_before = Balances::free_balance(&sink);
    let owner_before = Balances::free_balance(owner) + Balances::reserved_balance(owner);
    let frontiers = sources
      .iter(/* deos-bypass: bounded-iter -- The fixed four-LP source array. */)
      .map(|source| DependencyRegistrationHeaders::<Runtime>::get(source).next_index)
      .collect::<Vec<_>>();
    assert!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(owner.clone()),
        5,
        Mutability::Mutable,
        Some(contract.clone())
      )
      .is_ok()
    );
    let state = Actors::active_actor_state(next_id).unwrap();
    assert_eq!(state.contract, contract);
    assert_eq!(state.identity.owner, *owner);
    assert_eq!(state.identity.cycle_nonce, 0);
    let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<Runtime>::get(next_id)
    else {
      panic!("created semantic owner")
    };
    assert_eq!(semantic.generation, 1);
    assert!(matches!(
      ActorProcesses::<Runtime>::get(next_id).unwrap().residence,
      Some(ProcessResidence::Parked(_))
    ));
    assert!(
      state.run_state.is_none()
        && !state.hot.pending_signal
        && state.hot.trigger_wakeup_pointer.is_none()
    );
    assert_eq!(Actors::next_actor_id(), next_id + 1);
    assert_eq!(Actors::active_actor_count(), active_count + 1);
    assert_eq!(Actors::actor_identity_count(), identity_count + 1);
    assert_eq!(ServiceHeader::<Runtime>::get(), service);
    assert_eq!(
      [1, 16, 17, 18, 19].map(|actor| ActorSemanticStates::<Runtime>::get(actor).unwrap().encode()),
      service_semantics
    );
    assert_eq!(DependencySourceAllocatorState::<Runtime>::get(), allocator);
    for (index, source) in
      sources.iter(/* deos-bypass: bounded-iter -- The fixed four-LP source array. */).enumerate()
    {
      let position = DependencyRegistrationPositions::<Runtime>::get(source, next_id).unwrap();
      assert_eq!(
        position.page * 32 + u64::from(position.slot),
        frontiers[index]
      );
      assert_eq!(
        DependencyRegistrationHeaders::<Runtime>::get(source).next_index,
        frontiers[index] + 1
      );
    }
    let episode = ParkedBalanceEpisodes::<Runtime>::get(next_id).unwrap();
    assert_eq!(episode.watches.len(), 4);
    assert_eq!(
      classify_parked_balance_plan(episode.watches.as_slice(), |asset| (
        HostAssetOps::minimum_balance(asset),
        HostAssetOps::total_balance(&state.identity.sovereign_account, asset)
      ))
      .unwrap(),
      pallet_deos_actors::ParkedBalanceQualification::BelowThreshold
    );
    let quote = Actors::actor_cost_quote(next_id).unwrap();
    let creation_fee = <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get();
    assert_eq!(Balances::free_balance(&sink), sink_before + creation_fee);
    assert_eq!(
      Balances::free_balance(owner) + Balances::reserved_balance(owner),
      owner_before - creation_fee
    );
    TransactionOutcome::Rollback(
      json!({"actor": next_id, "ownerSlot": 5, "generation": semantic.generation, "mask": 15, "steps": 12, "creationFee": creation_fee.to_string(), "holdTotal": quote.actor_state_hold.total.to_string(), "prospectivePipelineFee": quote.prospective_pipeline_fee.unwrap().total_fee.to_string(), "serviceUnchanged": true, "sourceAllocatorUnchanged": true, "newIdentityAndActiveCountsExact": true, "creationFeeLedgerExact": true}),
    )
  });
  json!({"scope": "current_public_native_lifecycle_under_complete_rollback_no_blocks_or_signed_extensions", "cases": results, "birth": birth})
}

fn opening_transition_inputs(actor: u64) -> Option<Value> {
  if !matches!(actor, 1 | 16..=19) {
    return None;
  }
  let state = Actors::active_actor_state(actor).unwrap();
  let actor_type = state.identity.actor_class.actor_type();
  let head = ActorContractHeads::<Runtime>::get(actor).unwrap();
  assert_eq!(head.header.step_count, 1);
  let step = head.first_step.as_ref().unwrap();
  let stored = head.first_step_resources.unwrap();
  let context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: 0,
    predicate_evaluation_units: step
      .precondition
      .as_ref()
      .map_or(0, |p| p.evaluation_units()),
  };
  let mut cases = Vec::new();
  for invoked in [false, true] {
    if actor_type == ActorType::User && !invoked {
      continue;
    }
    let task_effect = if invoked {
      TaskEffectExecution::Invoked
    } else {
      TaskEffectExecution::NotInvoked
    };
    let actual_effect = EffectWeights::actual_effect_weight(&step.task, task_effect).unwrap();
    let action_fee = if actor_type == ActorType::User {
      <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(&actual_effect)
    } else {
      0
    };
    let execution = StepControlExecution {
      phase: StepControlPhase::Opening,
      outcome: StepControlOutcome::Completed,
      placement: StepControlPlacement::Queue,
      task_effect,
      action_fee_collected: action_fee != 0,
    };
    let actual_control =
      ControlWeights::actual_control_weight(context, step, stored.control, execution).unwrap();
    assert!(actual_control.all_lte(stored.control) && actual_effect.all_lte(stored.effect));
    cases.push(
      json!({"invoked": invoked, "actualControl": pair(actual_control),
      "actualEffect": pair(actual_effect), "actionFee": action_fee.to_string()}),
    );
  }
  let trigger_fee = if actor_type == ActorType::User {
    <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
      &ActorWeights::cadenced_trigger_occurrence(),
    )
  } else {
    0
  };
  let pipeline_fee = if actor_type == ActorType::User {
    head
      .header
      .pipeline_machine_envelope
      .pipeline_machine_fee_upper
  } else {
    0
  };
  let pointer = state.hot.trigger_wakeup_pointer;
  let handle = TriggerDeadlineHandles::<Runtime>::get(actor);
  assert_eq!(pointer.is_some(), handle.is_some());
  if let (Some(pointer), Some(handle)) = (pointer, handle) {
    assert_eq!(handle.actor.actor_id, actor);
    assert_eq!(handle.actor.generation, 1);
    assert_eq!(
      handle.key,
      pallet_deos_actors::WakeupKey::Tick(pointer.tick)
    );
    assert_eq!(handle.page, pointer.page_id);
    assert_eq!(u32::from(handle.slot), pointer.slot);
    assert_eq!(pointer.encoded_size(), 20);
  }
  let detector_bytes = pointer.map_or(0, |p| p.encoded_size());
  let detector_maximum = <Runtime as pallet_deos_actors::Config>::ActorStateHoldBase::get()
    + 20 * <Runtime as pallet_deos_actors::Config>::ActorStateHoldPerByte::get();
  let hold = ActorStateHolds::<Runtime>::get(actor).map(|hold| {
    assert_eq!(actor_type, ActorType::User);
    assert_eq!(hold.owner, state.identity.owner);
    let breakdown = hold.breakdown;
    assert_eq!(
      breakdown.detector,
      if detector_bytes == 0 {
        0
      } else {
        detector_maximum
      }
    );
    let fixed =
      breakdown.identity + breakdown.contract_head + breakdown.contract_body + breakdown.run;
    json!({"owner": hex(&hold.owner.encode()), "fixed": fixed.to_string(),
      "detector": breakdown.detector.to_string(), "maximumDetector": detector_maximum.to_string(),
      "maximumTotal": (fixed + detector_maximum).to_string()})
  });
  assert_eq!(hold.is_some(), actor_type == ActorType::User);
  Some(
    json!({"actualCases": cases, "triggerFee": trigger_fee.to_string(),
    "pipelineFee": pipeline_fee.to_string(), "detectorBytes": detector_bytes,
    "temporalHandle": format!("{handle:?}"), "hold": hold}),
  )
}

fn service_resources(actor: u64) -> Option<Value> {
  if !matches!(actor, 1 | 16..=19) {
    return None;
  }
  let head = ActorContractHeads::<Runtime>::get(actor).unwrap();
  assert_eq!(head.header.step_count, 1);
  assert!(!head.header.has_close_when);
  let step = head.first_step.as_ref().unwrap();
  let stored = head.first_step_resources.unwrap();
  let units = step
    .precondition
    .as_ref()
    .map_or(0, |condition| condition.evaluation_units());
  let context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: 0,
    predicate_evaluation_units: units,
  };
  assert_eq!(
    ControlWeights::maximum_control_weight(context, step),
    Some(stored.control)
  );
  assert_eq!(
    EffectWeights::maximum_effect_weight(&step.task),
    Some(stored.effect)
  );
  let selector = ActorWeights::service_round_begin_populated()
    .checked_add(&ActorWeights::service_round_probe_eligible())
    .unwrap();
  let inspection = selector
    .checked_add(&ActorWeights::scheduler_actor_state_probe())
    .unwrap();
  let suffix = ActorWeights::service_round_admit_eligible()
    .max(ActorWeights::service_member_retire_interior())
    .max(ActorWeights::service_member_retire_pair_cursor())
    .max(ActorWeights::service_member_retire_singleton());
  let idle = inspection
    .checked_add(&ActorWeights::service_round_admit_eligible())
    .unwrap();
  let cleanup = Actors::close_dispatch_weight_upper();
  let opening = inspection
    .checked_add(&stored.control)
    .and_then(|weight| weight.checked_add(&cleanup))
    .and_then(|weight| weight.checked_add(&suffix))
    .unwrap();
  Some(
    json!({"control": pair(stored.control), "effect": pair(stored.effect),
    "predicateUnits": units, "selector": pair(selector), "inspection": pair(inspection),
    "suffix": pair(suffix), "idle": pair(idle), "cleanup": pair(cleanup),
    "opening": pair(opening), "encounter": pair(idle.max(opening))}),
  )
}

fn resource_protocol() -> Value {
  assert!(Actors::block_resource_state().is_none());
  let snapshot = Actors::finalized_block_resource_telemetry().unwrap();
  assert_eq!(snapshot.block_number(), System::block_number());
  let budget = snapshot.budget();
  assert_eq!(
    budget,
    <Runtime as pallet_deos_actors::Config>::BlockResourceBudget::get()
  );
  let limits = budget.limits();
  let housekeeping = ActorWeights::scheduler_on_idle_base()
    .checked_add(&ActorWeights::block_resource_finalize())
    .unwrap();
  let dependency = ActorWeights::dependency_scan_source_probe()
    .checked_add(
      &ActorWeights::process_dependency_scan_unit()
        .max(ActorWeights::process_dependency_scan_completion_unit()),
    )
    .unwrap();
  let mandatory = ActorWeights::scheduler_on_initialize_cutoff()
    .checked_add(&Actors::deadline_service_weight_upper())
    .and_then(|weight| weight.checked_add(&dependency))
    .and_then(|weight| weight.checked_add(&housekeeping))
    .unwrap();
  json!({"currentStateAbsent": true, "finalizedBlock": snapshot.block_number(),
    "optionalActorWorkHalted": snapshot.optional_actor_work_halted(),
    "controlUsed": pair(snapshot.usage().actor_control_used()),
    "effectUsed": pair(snapshot.usage().actor_effect_used()),
    "userUsed": pair(snapshot.usage().user_dispatch_used()),
    "maximumBlock": pair(budget.maximum_block()), "fixedEnvelope": pair(budget.fixed_envelope()),
    "control": pair(limits.actor_control()), "actorBase": pair(limits.actor_base_turn()),
    "userBase": pair(limits.user_base_turn()), "shared": pair(limits.shared_economic()),
    "housekeeping": pair(housekeeping), "mandatory": pair(mandatory),
    "idleReserve": pair(<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get()),
    "maxExecutions": <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
    "maxScanned": <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get()})
}

fn bytes(value: &Value) -> Vec<u8> {
  let encoded = value.as_str().unwrap();
  assert!(encoded.len().is_multiple_of(2));
  (0..encoded.len())
    .step_by(2)
    .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
    .collect()
}

fn hex(bytes: &[u8]) -> String {
  assert_eq!(bytes.len(), 32);
  bytes
    .iter(/* deos-bypass: bounded-iter -- SHA-256 and AccountId32 encodings have exactly 32 bytes. */)
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

fn current_top(maximum_keys: usize) -> BTreeMap<Vec<u8>, Vec<u8>> {
  let mut top = BTreeMap::new();
  let mut cursor = Vec::new();
  for _ in 0..=maximum_keys {
    let Some(next) = polkadot_sdk::sp_io::storage::next_key(&cursor) else {
      return top;
    };
    top.insert(
      next.clone(),
      polkadot_sdk::sp_io::storage::get(&next).unwrap().to_vec(),
    );
    cursor = next;
  }
  panic!("projection created more keys than the retained input");
}

fn run(path: &str) {
  let inputs: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
  assert_eq!(inputs.as_array().unwrap().len(), 4);
  for seed in inputs.as_array().unwrap() {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    assert_eq!(
      hex(&polkadot_sdk::sp_io::hashing::sha2_256(&raw)),
      seed["sha256"]
    );
    let snapshot: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(snapshot["phase"], seed["phase"]);
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let before = storage.clone();
    let code = storage.top[well_known_keys::CODE].clone();
    let mut ext = TestExternalities::new_with_code_and_state(
      &code,
      storage,
      deos_runtime::VERSION.state_version(),
    );
    ext.execute_with(|| {
      assert_eq!(System::block_number(), u32::try_from(snapshot["block"].as_u64().unwrap()).unwrap());
      let mut indices = SovereignIndex::<Runtime>::iter().take(65).collect::<Vec<_>>();
      assert!(indices.len() <= 64);
      indices.sort_by_key(|row| row.1);
      let mut semantic_ids = ActorSemanticStates::<Runtime>::iter_keys().take(65).collect::<Vec<_>>();
      assert!(semantic_ids.len() <= 64);
      semantic_ids.sort_unstable();
      assert_eq!(semantic_ids, indices.iter(/* deos-bypass: bounded-iter -- The diagnostic rejects more than 64 imported identities. */).map(|row| row.1).collect::<Vec<_>>());
      let mut rows = Vec::new();
      for (custody, actor) in indices {
        let identity = Actors::actor_identity(actor).unwrap();
        assert_eq!(identity.sovereign_account, custody);
        let generation = match ActorSemanticStates::<Runtime>::get(actor).unwrap() {
          ActorSemanticState::Dormant(record) => record.generation,
          ActorSemanticState::Active(record) => record.generation,
        };
        let active = Actors::active_actor_state(actor);
        let watches = active.as_ref().and_then(|state| state.contract.parked_balance_activation.as_ref()).map(|parked| {
          parked.watches.iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the authored plan. */).map(|rule| {
            let AssetKind::Local(asset) = rule.asset else { panic!("retained recipe watches local LP assets only") };
            assert!((0x7000_0001..=0x7000_0004).contains(&asset));
            json!({"asset": asset, "minimum": rule.authored_min_delta.to_string(), "balance": Assets::balance(asset, &custody).to_string(), "hostTotalBalance": HostAssetOps::total_balance(&custody, rule.asset).to_string(), "hostMinimumBalance": HostAssetOps::minimum_balance(rule.asset).to_string()})
          }).collect::<Vec<_>>()
        }).unwrap_or_default();
        let all_lp = (0x7000_0001..=0x7000_0004).map(|asset| Assets::balance(asset, &custody).to_string()).collect::<Vec<_>>();
        let episode = ParkedBalanceEpisodes::<Runtime>::get(actor);
        let classification = episode.as_ref().map(|episode| {
          format!("{:?}", classify_parked_balance_plan(episode.watches.as_slice(), |asset| {
            (HostAssetOps::minimum_balance(asset), HostAssetOps::total_balance(&custody, asset))
          }).unwrap())
        });
        rows.push(json!({
          "actor": actor, "class": format!("{:?}", identity.actor_class),
          "owner": hex(&identity.owner.encode()), "custody": hex(&custody.encode()),
          "cycleNonce": identity.cycle_nonce, "generation": generation,
          "serviceResources": service_resources(actor),
          "openingTransitionInputs": opening_transition_inputs(actor),
          "mutability": format!("{:?}", identity.mutability),
          "nativeFree": Balances::free_balance(&custody).to_string(), "lpBalances": all_lp,
          "active": active.is_some(), "watches": watches,
          "process": ActorProcesses::<Runtime>::get(actor).map(|process| format!("{process:?}")),
          "dependencyPlan": format!("{:?}", DependencyPlans::<Runtime>::get(actor).as_slice()),
          "parkClassification": classification,
          "parkEpisode": episode.map(|episode| json!({"actor": episode.owner.actor.actor_id, "generation": episode.owner.actor.generation, "planRevision": episode.owner.plan_revision, "watches": episode.watches.iter(/* deos-bypass: bounded-iter -- MaxParkedBalanceWatches bounds the retained episode. */).map(|watch| json!({"asset": format!("{:?}", watch.asset), "anchor": watch.anchor.to_string(), "certifiedMinimum": watch.certified_minimum_balance.to_string(), "authoredMinimum": watch.authored_min_delta.to_string(), "acknowledgedRevision": watch.acknowledged_revision})).collect::<Vec<_>>()})),
          "control": active.map(|state| json!({
            "trigger": format!("{:?}", state.contract.trigger),
            "steps": format!("{:?}", state.contract.steps),
            "closeWhen": format!("{:?}", state.contract.close_when),
            "onUnfunded": format!("{:?}", state.contract.on_unfunded),
            "cooldownBlocks": state.contract.cooldown_blocks,
            "window": format!("{:?}", state.contract.window),
            "autoCloseAtCycleNonce": state.contract.auto_close_at_cycle_nonce,
            "funding": format!("{:?}", state.contract.funding),
            "parkedActivation": state.contract.parked_balance_activation.is_some(),
            "lifecycle": format!("{:?}", state.hot.lifecycle),
            "unsuccessfulAttemptStreak": state.hot.unsuccessful_attempt_streak,
            "queueTicket": format!("{:?}", state.hot.queue_ticket),
            "wakeupPointer": format!("{:?}", state.hot.wakeup_pointer),
            "triggerWakeupPointer": format!("{:?}", state.hot.trigger_wakeup_pointer),
            "terminalAt": state.hot.terminal_at,
            "scheduleAnchor": state.hot.schedule_anchor,
            "lastCycleBlock": state.hot.last_cycle_block,
            "pendingSignal": state.hot.pending_signal,
            "cycleState": format!("{:?}", state.hot.cycle_state),
            "runState": state.run_state.map(|run| format!("{run:?}")),
          })),
        }));
      }
      assert!(
        current_top(before.top.len()) == before.top,
        "Actor projection must preserve every retained TOP byte"
      );
      let protocol = resource_protocol();
      assert!(current_top(before.top.len()) == before.top, "Resource projection must preserve TOP");
      let lifecycle = lifecycle_caller_probes();
      assert!(current_top(before.top.len()) == before.top, "Rolled-back lifecycle probes must preserve every TOP byte");
      let signed = signed_caller_probes();
      assert!(current_top(before.top.len()) == before.top, "Rolled-back signed probes must preserve every TOP byte");
      let callbacks = callback_protocol_probes(before.top.len());
      assert!(current_top(before.top.len()) == before.top, "Rolled-back callback probes must preserve every TOP byte");
      println!("{}", json!({"scope": "finite_read_only_actor_seed_projection", "preset": seed["preset"], "phase": seed["phase"], "block": snapshot["block"], "actors": rows, "resourceProtocol": protocol, "lifecycleCallerProbes": lifecycle, "signedCallerProbes": signed, "callbackProtocolProbes": callbacks}));
    });
  }
}

/// Read the two retained public-admission states without executing a callback or dispatch.
fn initial_actor_projection(path: &str) {
  use pallet_deos_actors::GenesisSystemActors;
  let inputs: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
  let inputs = inputs.as_array().unwrap();
  assert_eq!(inputs.len(), 2);
  for seed in inputs {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    assert_eq!(
      hex(&polkadot_sdk::sp_io::hashing::sha2_256(&raw)),
      seed["sha256"]
    );
    let snapshot: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(snapshot["phase"], "public-admission");
    assert_eq!(snapshot["block"], 187_204);
    assert_eq!(snapshot["childEntries"], 0);
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let before = storage.clone();
    let code = storage.top[well_known_keys::CODE].clone();
    let mut ext = TestExternalities::new_with_code_and_state(
      &code,
      storage,
      deos_runtime::VERSION.state_version(),
    );
    ext.execute_with(|| {
      let mut ids = ActorSemanticStates::<Runtime>::iter_keys().take(16).collect::<Vec<_>>();
      ids.sort_unstable();
      assert_eq!(ids, (0..15).collect::<Vec<_>>());
      let compiled = <Runtime as pallet_deos_actors::Config>::GenesisSystemActors::system_actors();
      let dormant = <Runtime as pallet_deos_actors::Config>::GenesisSystemActors::dormant_system_actors();
      assert_eq!(compiled.len(), 3);
      assert_eq!(dormant.len(), 12);
      let mut rows = Vec::new();
      for actor in ids {
        let identity = Actors::actor_identity(actor).unwrap();
        assert_eq!(identity.actor_class.actor_type(), ActorType::System);
        let account = &identity.sovereign_account;
        assert_eq!(SovereignIndex::<Runtime>::get(account), Some(actor));
        let active = Actors::active_actor_state(actor);
        let control = active.map(|state| {
          let (_, owner, mutability, contract) = compiled.iter(/* deos-bypass: bounded-iter -- Three configured active System Actors bound this diagnostic. */).find(|row| row.0 == actor).unwrap();
          assert_eq!(&identity.owner, owner);
          assert_eq!(&identity.mutability, mutability);
          assert_eq!(&state.contract, contract);
          json!({
            "canonicalContract": true, "pendingSignal": state.hot.pending_signal,
            "cycleState": format!("{:?}", state.hot.cycle_state),
            "lifecycle": format!("{:?}", state.hot.lifecycle),
            "runState": state.run_state.map(|state| format!("{state:?}")),
            "trigger": format!("{:?}", state.contract.trigger),
            "triggerRuntimeState": format!("{:?}", state.hot.trigger_runtime_state),
            "firstStepPredicate": state.contract.steps.first().map(|step| format!("{:?}", step.precondition)),
            "queueTicket": state.hot.queue_ticket, "wakeupPointer": format!("{:?}", state.hot.wakeup_pointer),
            "triggerWakeupPointer": format!("{:?}", state.hot.trigger_wakeup_pointer),
            "terminalAt": state.hot.terminal_at, "lastCycleBlock": state.hot.last_cycle_block,
          })
        });
        if control.is_none() {
          let (_, owner, mutability) = dormant.iter(/* deos-bypass: bounded-iter -- Twelve configured Dormant System Actors bound this diagnostic. */).find(|row| row.0 == actor).unwrap();
          assert_eq!(&identity.owner, owner);
          assert_eq!(&identity.mutability, mutability);
          assert!(!ActorContractHeads::<Runtime>::contains_key(actor));
        }
        rows.push(json!({
          "actor": actor, "class": format!("{:?}", identity.actor_class),
          "custody": hex(&account.encode()), "cycleNonce": identity.cycle_nonce,
          "nativeFree": Balances::free_balance(account).to_string(), "control": control,
          "process": ActorProcesses::<Runtime>::get(actor).map(|state| format!("{state:?}")),
          "dependencyPlan": format!("{:?}", DependencyPlans::<Runtime>::get(actor).as_slice()),
          "parkedEpisode": ParkedBalanceEpisodes::<Runtime>::contains_key(actor),
          "triggerDeadline": format!("{:?}", TriggerDeadlineHandles::<Runtime>::get(actor)),
        }));
      }
      assert_eq!(Actors::next_actor_id(), 15);
      assert_eq!(BalanceDependencySources::<Runtime>::iter_keys().take(1).count(), 0);
      assert_eq!(DependencyRegistrationHeaders::<Runtime>::iter_keys().take(1).count(), 0);
      assert_eq!(ParkedBalanceEpisodes::<Runtime>::iter_keys().take(1).count(), 0);
      let flags = Actors::machine_control_flags();
      assert!(flags.user_creation_enabled && !flags.machine_frozen);
      assert_eq!(current_top(before.top.len()), before.top);
      println!("INITIAL_ACTOR_FRAME {}", json!({
        "preset": seed["preset"], "phase": snapshot["phase"], "block": snapshot["block"],
        "actors": rows, "nextActorId": Actors::next_actor_id(),
        "sourceAllocator": format!("{:?}", DependencySourceAllocatorState::<Runtime>::get()),
        "serviceHeader": format!("{:?}", ServiceHeader::<Runtime>::get()),
        "userCreationEnabled": flags.user_creation_enabled, "machineFrozen": flags.machine_frozen,
        "topEntries": before.top.len(), "everyTopByteReadOnly": true,
        "scope": "Typed initial Actor authority/canonical Contract and state projection only; no callback, block, signed dispatch, proof/physical or horizon acceptance."
      }));
    });
  }
}

fn main() {
  let mut args = std::env::args().skip(1);
  match (args.next(), args.next()) {
    (Some(arg), None) if matches!(arg.as_str(), "--help" | "-h") => println!(
      "Usage: actor_retention_seed_actor_state SEED_INPUTS_JSON or --initial-actor-frame INPUTS_JSON\nThe initial mode reads two public-admission snapshots and preserves every TOP byte. It executes no callbacks or dispatch.\nReads four checksum-bound TOP snapshots and runs rolled-back lifecycle, signed extension and quiet idle/poll callback probes. Signed probes prepare System/External phase; callback probes select actual hooks. No complete blocks or physical qualification."
    ),
    (Some(flag), Some(path)) if flag == "--initial-actor-frame" && args.next().is_none() => {
      initial_actor_projection(&path)
    }
    (Some(path), None) => run(&path),
    _ => panic!("pass the retained seed-input JSON or --help"),
  }
}
