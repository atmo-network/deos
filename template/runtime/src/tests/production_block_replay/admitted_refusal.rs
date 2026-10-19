//! Signed User-only alternative to the synthetic paused-System inspection source.
//! Bounded signed activation batches create real User continuations and fresh paid peers.
//! Literal native ancestry is distinct from the final historical-image Wasm replay.

use super::*;
use pallet_deos_actors::{
  ActorContract, FundingSourcePolicy, OnUnfunded, TaskEffectWeightProvider,
};
use primitives::AssetKind;

pub(super) fn advance(
  storage: &mut Storage,
  parent: &mut Header,
  wasm: &[u8],
  signer: &sr25519::Pair,
  calls: &[RuntimeCall],
  demand: UserDemand,
) -> AuthoredBlock {
  let owner = crate::AccountId::from(signer.public());
  let mut ext = externalities_with_bound_code(storage.clone());
  assert_construction_parent_matches_state(&mut ext, parent);
  let nonce = ext.execute_with(|| {
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(well_known_keys::CODE)
        .unwrap()
        .to_vec(),
      wasm
    );
    assert!(Actors::user_creation_enabled());
    System::account_nonce(&owner)
  });
  let authored = author_complete_block_after_with_calls(
    storage.clone(),
    wasm,
    parent,
    parent.number().checked_add(1).unwrap(),
    demand,
    signer,
    nonce,
    &BTreeMap::new(),
    calls,
  );
  assert_complete_block_replays_natively(&authored, wasm);
  *storage = authored.post_state.clone();
  *parent = authored.block.header.clone();
  authored
}

pub(super) fn transfer(to: crate::AccountId, value: u128) -> RuntimeCall {
  RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
    dest: MultiAddress::Id(to),
    value,
  })
}

#[test]
#[ignore = "literal 187204-block signed governance ancestry and explicit historical production Wasm"]
fn signed_genesis_user_liquidity_inspection_refusal() {
  let wasm =
    std::fs::read(std::env::var("DEOS_PRODUCTION_WASM").expect("explicit historical image"))
      .unwrap();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let preset = polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into();
  let genesis = preset_genesis_storage(&wasm, &preset);
  assert_eq!(genesis.top[well_known_keys::CODE], wasm);
  TestExternalities::new(genesis).execute_with(|| {
    assert!(!Actors::user_creation_enabled());
    assert_eq!(Assets::balance(0, &owner), 1u128 << 60);
    assert!(crate::Staking::staked_asset_id(0).is_some());
  });
  // Enactment, not either prepared-clock helper or the four-block preparation endpoint.
  let (mut storage, mut parent) = replay_signed_l2_admission_with_pvf(
    &signer,
    &preset,
    0,
    SignedAdmissionReplayEnd::Enactment,
    &wasm,
    None,
  );
  assert_eq!(*parent.number(), 187_204);
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_ADMITTED_REFUSAL_OUTPUT").expect("new raw output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  let paired = AssetKind::Local(0);
  advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[
      RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
        asset_a: AssetKind::Native,
        asset_b: paired,
      }),
      RuntimeCall::AssetConversion(polkadot_sdk::pallet_asset_conversion::Call::add_liquidity {
        asset1: Box::new(AssetKind::Native),
        asset2: Box::new(paired),
        amount1_desired: crate::UNIT,
        amount2_desired: crate::UNIT,
        amount1_min: 1,
        amount2_min: 1,
        mint_to: owner.clone(),
      }),
    ],
    UserDemand::ActorOnly,
  );
  let lp = externalities_with_bound_code(storage.clone()).execute_with(|| {
    polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((AssetKind::Native, paired))
      .unwrap()
      .lp_token
  });
  let task = Task::AddLiquidity {
    asset_a: AssetKind::Native,
    asset_b: paired,
    amount_a: AmountResolution::Fixed(crate::UNIT),
    amount_b: AmountResolution::Fixed(crate::UNIT),
    min_lp_out: 1,
  };
  const STEPS: u32 = 12;
  let steps: ContractSteps<Runtime> = vec![
    StepOf::<Runtime> {
      precondition: None,
      task: task.clone(),
      on_error: StepErrorPolicy::AbortCycle,
    };
    STEPS as usize
  ]
  .try_into()
  .unwrap();
  let effect =
    crate::configs::actor_config::TmctolTaskEffectWeight::maximum_effect_weight(&task).unwrap();
  const POPULATION: u8 = 128;
  let mut ids = Vec::new();
  // One owner's legal slots; no System creation, pause/resume, mint, or capacity injection.
  for slot in 0..POPULATION {
    let (id, custody, prefund) =
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        (
          Actors::next_actor_id(),
          Actors::sovereign_account_id(&owner, slot),
          Actors::user_pipeline_machine_capacity_requirement(&steps)
            .unwrap()
            .checked_add(<Runtime as pallet_deos_actors::Config>::MinUserBalance::get())
            .unwrap()
            .checked_add(crate::EXISTENTIAL_DEPOSIT)
            .unwrap(),
        )
      });
    advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &[
        transfer(custody.clone(), prefund),
        RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
          mutability: Mutability::Mutable,
          contract: Some(ActorContract {
            trigger: Trigger::manual(),
            cooldown_blocks: 0,
            window: None,
            steps: steps.clone(),
            on_unfunded: OnUnfunded::Close,
            close_when: None,
            parked_balance_activation: None,
            funding: FundingSourcePolicy::OwnerOnly,
            auto_close_at_cycle_nonce: None,
          }),
        }),
      ],
      UserDemand::ActorOnly,
    );
    let extra = externalities_with_bound_code(storage.clone()).execute_with(|| {
      let state = Actors::active_actor_state(id).unwrap();
      assert_eq!(state.identity.sovereign_account, custody);
      assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
      assert!(!state.hot.pending_signal && state.run_state.is_none());
      let q = Actors::actor_cost_quote(id).unwrap();
      let free_required = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
        .checked_add(crate::EXISTENTIAL_DEPOSIT)
        .unwrap()
        .checked_add(crate::UNIT.checked_mul(u128::from(STEPS)).unwrap())
        .unwrap()
        .checked_add(q.prospective_trigger_fee.unwrap().fee)
        .unwrap()
        .checked_add(q.prospective_pipeline_fee.unwrap().total_fee)
        .unwrap()
        .checked_add(
          q.maximum_next_action_fee
            .maximum_effect_fee
            .checked_mul(u128::from(STEPS))
            .unwrap(),
        )
        .unwrap();
      free_required.saturating_sub(Balances::free_balance(&custody))
    });
    let mut calls = vec![
      RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
        id: 0,
        target: MultiAddress::Id(custody.clone()),
        amount: crate::UNIT.checked_mul(u128::from(STEPS)).unwrap(),
      }),
      // Pay the new LP account deposit through the signed owner before the bounded Task.
      RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::touch_other {
        id: lp,
        who: MultiAddress::Id(custody.clone()),
      }),
    ];
    if extra > 0 {
      calls.push(transfer(custody, extra));
    }
    advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    ids.push(id);
  }
  let mut paid_occurrences = BTreeSet::new();
  let mut activation = None;
  let mut refusal_parent = None;
  // Keep 32 funded residents unactivated; the third bounded burst meets existing continuations.
  for batch in ids[..96].chunks(32) {
    let calls = batch
      .iter()
      .map(|id| RuntimeCall::Actors(pallet_deos_actors::Call::manual_trigger { actor_id: *id }))
      .collect::<Vec<_>>();
    refusal_parent = Some(parent.clone());
    let authored = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ContinuousShared,
    );
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      assert!(
        !System::events().iter().any(|r| matches!(r.event,
        RuntimeEvent::Actors(Event::StepFailed { actor_id, .. }) if ids.contains(&actor_id))),
        "prepared LP accounts must admit actual successful User Tasks"
      );
      for id in batch {
        assert!(System::events().iter().any(|r| matches!(r.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, fee, .. })
            if actor_id == *id && fee > 0)));
        assert!(paid_occurrences.insert(*id), "one useful readiness payment");
      }
    });
    activation = Some(authored);
  }
  let refusal = activation.unwrap();
  let refusal_parent = refusal_parent.unwrap();
  std::fs::write(
    output.join("activation-post.scale"),
    refusal.pre_state.top.encode(),
  )
  .unwrap();
  let paid_ids = externalities_with_bound_code(refusal.pre_state.clone()).execute_with(|| {
    let mut paid = Vec::new();
    let mut rows = Vec::new();
    for id in &ids {
      let state = Actors::active_actor_state(*id).unwrap();
      let fee = System::events().iter().find_map(|r| match r.event {
        RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, fee, .. })
          if actor_id == *id =>
        {
          Some(fee)
        }
        _ => None,
      });
      rows.push(serde_json::json!({
        "actor": id,
        "pending": state.hot.pending_signal,
        "cycleNonce": state.identity.cycle_nonce,
        "run": format!("{:?}", state.run_state),
        "triggerFee": fee,
      }));
      if state.hot.pending_signal && state.run_state.is_none() && paid_occurrences.contains(id) {
        assert_eq!(state.identity.cycle_nonce, 0);
        paid.push(*id);
      }
    }
    std::fs::write(
      output.join("activation.json"),
      serde_json::to_vec_pretty(&serde_json::json!({
        "block": refusal_parent.number(),
        "stateRoot": format!("{:?}", refusal_parent.state_root()),
        "constructed": ids.len(),
        "paidIdle": paid,
        "actors": rows,
      }))
      .unwrap(),
    )
    .unwrap();
    assert!(
      !paid.is_empty(),
      "actual paid Idle cohort before the selected block"
    );
    paid
  });
  std::fs::write(output.join("refusal-post.scale"), storage.top.encode()).unwrap();
  let target = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let cursor = Actors::service_header().cursor;
    eprintln!("admitted refusal candidate: paid={paid_ids:?} cursor={cursor:?}");
    let target = cursor.unwrap().actor_id;
    assert!(
      paid_ids.contains(&target),
      "target was already paid and Idle in the parent"
    );
    let index = paid_ids.iter().position(|id| *id == target).unwrap();
    assert!(
      System::events().iter().any(|r| matches!(r.event,
      RuntimeEvent::Actors(Event::LiquidityAdded { actor_id, .. })
        if ids.contains(&actor_id) && actor_id != target)),
      "real preceding committed User liquidity effects"
    );
    for id in &paid_ids[index..] {
      let state = Actors::active_actor_state(*id).unwrap();
      assert!(state.hot.pending_signal && state.run_state.is_none());
      assert_eq!(
        state.identity.cycle_nonce, 0,
        "no Opening or follower bypass"
      );
    }
    let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
    let remaining = BlockResourceBudgetValue::get()
      .limits()
      .shared_economic()
      .checked_sub(
        &telemetry
          .usage()
          .actor_effect_used()
          .saturating_add(telemetry.usage().user_dispatch_used()),
      )
      .unwrap();
    assert!(
      !effect.all_lte(remaining),
      "actual effect resource shortage, not an invented reservation"
    );
    target
  });
  let before = externalities_with_bound_code(refusal.pre_state.clone())
    .execute_with(|| Actors::active_actor_state(target).unwrap());
  let after = externalities_with_bound_code(refusal.post_state.clone())
    .execute_with(|| Actors::active_actor_state(target).unwrap());
  assert_eq!(
    before.encode(),
    after.encode(),
    "fresh paid target is preserved exactly"
  );
  // A historical accepted image is a separate execution check, never current-tree issuance.
  let refusal_output = output.join("refusal");
  std::fs::create_dir(&refusal_output).unwrap();
  pvf_fixture::export(
    &refusal_output,
    &preset,
    (
      refusal.pre_state.clone(),
      refusal.post_state.clone(),
      &refusal_parent,
      refusal.block.clone(),
      refusal.inherent_data.clone(),
    ),
  )
  .unwrap();
  replay_complete_block_in_wasm(&refusal, &wasm, "signed-genesis-user-liquidity-alternative");
  println!(
    "signed-genesis-user-liquidity-alternative: block={} target={target}; literal_native_history=true; physical_pricing=false",
    parent.number()
  );
}
