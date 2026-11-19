//! Declaration preflight and signed-genesis construction of a terminal-User candidate.
//! Native history and selected historical-image replay do not qualify physical prices.

use super::admitted_refusal::{advance, transfer};
use super::*;
use pallet_deos_actors::{
  ActorContract, FundingSourcePolicy, OnUnfunded, StepControlExecution, StepControlOutcome,
  StepControlPhase, StepControlPlacement, StepControlWeightContext, StepControlWeightProvider,
  StepSkippedReason, TaskEffectExecution,
};

type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
type Control = crate::configs::actor_config::RuntimeStepControlWeight;

pub(super) fn lp_step() -> StepOf<Runtime> {
  StepOf::<Runtime> {
    precondition: None,
    task: Task::AddLiquidity {
      asset_a: primitives::AssetKind::Native,
      asset_b: primitives::AssetKind::Local(0),
      amount_a: AmountResolution::Fixed(crate::UNIT),
      amount_b: AmountResolution::Fixed(crate::UNIT),
      min_lp_out: 1,
    },
    on_error: StepErrorPolicy::AbortCycle,
  }
}

fn declaration_gate() {
  // These are pure legal Weight-provider inputs, not fabricated execution or Run authority.
  let inspection = W::service_round_begin_populated()
    .saturating_add(W::service_round_probe_eligible())
    .saturating_add(W::scheduler_actor_state_probe());
  let suffix = W::service_round_admit_eligible().max(
    W::service_member_retire_interior()
      .max(W::service_member_retire_pair_cursor().max(W::service_member_retire_singleton())),
  );
  let common = inspection.saturating_add(suffix);
  let context = |cursor, steps_in_fragment, opening_tail_chunks| StepControlWeightContext {
    cursor,
    steps_in_fragment,
    opening_tail_chunks,
    predicate_evaluation_units: 0,
  };
  let execution = |phase, outcome, placement, invoked| StepControlExecution {
    phase,
    outcome,
    placement,
    task_effect: if invoked {
      TaskEffectExecution::Invoked
    } else {
      TaskEffectExecution::NotInvoked
    },
    action_fee_collected: invoked,
  };
  let actual = |context, step: &StepOf<Runtime>, execution| {
    let maximum = Control::maximum_control_weight(context, step).unwrap();
    Control::actual_control_weight(context, step, maximum, execution)
      .unwrap()
      .saturating_add(common)
  };
  let lp = lp_step();
  // This id selects only the task class here. A signed source must discover the actual pool
  // LP id, prepare its accounts publicly and prove the zero balances before each skip.
  let skipped_transfer = StepOf::<Runtime> {
    precondition: None,
    task: Task::Transfer {
      to: crate::AccountId::new([0x42; 32]),
      asset: primitives::AssetKind::Local(primitives::TYPE_LP),
      amount: AmountResolution::Percent(polkadot_sdk::sp_runtime::Perbill::one()),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  let terminal = |size| {
    actual(
      context(size, size, 0),
      &lp,
      execution(
        StepControlPhase::Running,
        StepControlOutcome::Completed,
        StepControlPlacement::None,
        true,
      ),
    )
  };
  let skipped_opening = actual(
    context(0, 1, 1),
    &skipped_transfer,
    execution(
      StepControlPhase::Opening,
      StepControlOutcome::Continued,
      StepControlPlacement::Queue,
      false,
    ),
  );
  let skipped_running = actual(
    context(1, 2, 0),
    &skipped_transfer,
    execution(
      StepControlPhase::Running,
      StepControlOutcome::Continued,
      StepControlPlacement::Queue,
      false,
    ),
  );
  let head = Control::maximum_control_weight(context(0, 1, 0), &lp)
    .unwrap()
    .saturating_add(common);
  // Bind the calculation to the actual twelve-Step signed source's loaded-head operands.
  // This is declaration correspondence, not transfer of its refusal branch to this candidate.
  let original_head = Control::maximum_control_weight(context(0, 1, 3), &lp)
    .unwrap()
    .saturating_add(common);
  assert_eq!(original_head, Weight::from_parts(7_227_262_500, 109_463));
  let effect = W::task_add_liquidity();
  let limits = crate::configs::BlockResourceBudgetValue::get().limits();
  let prefix = terminal(2)
    .saturating_mul(36)
    .saturating_add(terminal(1).saturating_mul(36));
  let warm = skipped_running
    .saturating_mul(36)
    .saturating_add(skipped_opening.saturating_mul(36));
  let effect_prefix = effect.saturating_mul(72);
  let combined = prefix
    .saturating_add(effect_prefix)
    .saturating_add(head)
    .saturating_add(effect);
  let base_meter = limits
    .actor_control()
    .saturating_add(limits.actor_base_turn());
  let control_slack = limits
    .actor_control()
    .checked_sub(&prefix.saturating_add(head))
    .unwrap();
  let meter_slack = base_meter.checked_sub(&combined).unwrap();
  let catalog = [
    ("Transfer", W::task_transfer()),
    ("Burn", W::task_burn()),
    ("Mint", W::task_mint()),
    ("SwapIn", W::task_dex_exact_in()),
    ("SwapOut", W::task_dex_exact_out()),
    ("AddLiquidity", effect),
    ("RemoveLiquidity", W::task_remove_liquidity()),
    ("DonateLiquidity", W::task_donate_liquidity()),
    ("Stake", W::task_stake()),
    ("Unstake", W::task_unstake()),
  ];
  assert!(
    catalog
      .iter()
      .all(|(_, weight)| weight.proof_size() <= effect.proof_size())
  );
  let pair = |weight: Weight| [weight.ref_time(), weight.proof_size()];
  println!(
    "TERMINAL_USER_DECLARATION_PREFLIGHT {}",
    serde_json::json!({
      "terminalTailTwo": pair(terminal(2)), "terminalTailOne": pair(terminal(1)),
      "skippedOpening": pair(skipped_opening), "skippedRunning": pair(skipped_running),
      "head": pair(head), "originalTwelveStepHead": pair(original_head),
      "effect": pair(effect), "prefix": pair(prefix), "warm": pair(warm),
      "controlLimit": pair(limits.actor_control()), "baseEffectLimit": pair(limits.actor_base_turn()),
      "effectPrefix": pair(effect_prefix), "baseMeter": pair(base_meter),
      "combinedClaim": pair(combined), "maximumControlPrelude": pair(control_slack),
      "maximumMeterPrelude": pair(meter_slack), "configuredCeilingOnly": true,
      "sourceExecutionQualified": false, "physicalPricingQualified": false,
    })
  );
  let warm_head = Control::maximum_control_weight(context(0, 1, 1), &skipped_transfer)
    .unwrap()
    .saturating_add(common);
  assert!(
    warm
      .checked_sub(&skipped_opening)
      .unwrap()
      .saturating_add(warm_head)
      .all_lte(limits.actor_control()),
    "warmup must admit its last skipped Step even before resolving its live zero balance"
  );
  assert!(
    prefix.saturating_add(head).all_lte(limits.actor_control()),
    "terminal prefix must leave the fresh target's complete declared head Control"
  );
  assert!(
    combined.all_lte(base_meter),
    "the combined loaded-head meter must also fit"
  );
  assert!(effect_prefix.all_lte(limits.actor_base_turn()));
  assert!(
    effect_prefix
      .saturating_add(effect)
      .any_gt(limits.actor_base_turn()),
    "the next real LP effect must exceed an independent base axis"
  );
}

#[test]
fn terminal_user_source_declaration_preflight() {
  TestExternalities::default().execute_with(declaration_gate);
}

#[test]
#[ignore = "literal signed genesis history, terminal User cohorts and explicit historical Wasm"]
fn signed_genesis_terminal_user_refusal() {
  crate::tests::common::ensure_artifact_profile().expect("signed-genesis artifact profile");
  let wasm = std::fs::read(std::env::var("DEOS_PRODUCTION_WASM").expect("explicit image")).unwrap();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_TERMINAL_USER_OUTPUT").expect("new source output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let preset = polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into();
  let genesis = preset_genesis_storage(&wasm, &preset);
  assert_eq!(genesis.top[well_known_keys::CODE], wasm);
  TestExternalities::new(genesis).execute_with(|| assert!(!Actors::user_creation_enabled()));
  let (mut storage, mut parent) = replay_signed_l2_admission_with_pvf(
    &signer,
    &preset,
    0,
    SignedAdmissionReplayEnd::Enactment,
    &wasm,
    None,
  );
  assert_eq!(*parent.number(), 187_204);
  let paired = primitives::AssetKind::Local(0);
  advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[
      RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
        asset_a: primitives::AssetKind::Native,
        asset_b: paired,
      }),
      RuntimeCall::AssetConversion(polkadot_sdk::pallet_asset_conversion::Call::add_liquidity {
        asset1: Box::new(primitives::AssetKind::Native),
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
    polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
      primitives::AssetKind::Native,
      paired,
    ))
    .unwrap()
    .lp_token
  });
  // Preserve the paired asset's actual ED as well as the authored LP spend.
  let paired_funding = externalities_with_bound_code(storage.clone()).execute_with(|| {
    crate::UNIT.checked_add(
      <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::minimum_balance(0)
    ).unwrap()
  });
  let skipped = StepOf::<Runtime> {
    precondition: None,
    task: Task::Transfer {
      to: owner.clone(),
      asset: primitives::AssetKind::Local(lp),
      amount: AmountResolution::Percent(Perbill::one()),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  let effect = W::task_add_liquidity();
  let lp_fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&effect);
  let mut ids = Vec::new();
  for slot in 0u8..128 {
    let skipped_count = if slot < 36 {
      2
    } else if slot < 72 {
      1
    } else {
      0
    };
    let steps: ContractSteps<Runtime> = std::iter::repeat_n(skipped.clone(), skipped_count)
      .chain(std::iter::once(lp_step()))
      .collect::<Vec<_>>()
      .try_into()
      .unwrap();
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
            steps,
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
      let q = Actors::actor_cost_quote(id).unwrap();
      <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
        .checked_add(crate::EXISTENTIAL_DEPOSIT)
        .unwrap()
        .checked_add(crate::UNIT)
        .unwrap()
        .checked_add(q.prospective_trigger_fee.unwrap().fee)
        .unwrap()
        .checked_add(q.prospective_pipeline_fee.unwrap().total_fee)
        .unwrap()
        .checked_add(lp_fee)
        .unwrap()
        .saturating_sub(Balances::free_balance(&custody))
    });
    let mut calls = vec![
      RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
        id: 0,
        target: MultiAddress::Id(custody.clone()),
        amount: paired_funding,
      }),
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
  let target = ids[72];
  let mut openings = BTreeSet::new();
  let mut skipped_steps = 0;
  for batch in [&ids[..36], &ids[36..72], &ids[72..73]] {
    // Every zero is read from publicly prepared LP custody, never an authored zero amount.
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      for id in &ids {
        let actor = Actors::active_actor_state(*id).unwrap();
        assert_eq!(Assets::balance(lp, &actor.identity.sovereign_account), 0);
      }
    });
    let calls = batch
      .iter()
      .map(|id| RuntimeCall::Actors(pallet_deos_actors::Call::manual_trigger { actor_id: *id }))
      .collect::<Vec<_>>();
    advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      for id in batch {
        assert!(System::events().iter().any(|record| matches!(record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, fee, .. })
            if actor_id == *id && fee > 0)));
      }
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id,
            reason: StepSkippedReason::ResolutionSkipped,
            ..
          }) if ids[..72].contains(&actor_id) => {
            skipped_steps += 1;
          }
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, fee })
            if ids[..72].contains(&actor_id) =>
          {
            assert!(fee > 0);
            assert!(
              openings.insert(actor_id),
              "one paid Pipeline per prefix Actor"
            );
          }
          RuntimeEvent::Actors(
            Event::StepFailed { actor_id, .. }
            | Event::ActionFeeCharged { actor_id, .. }
            | Event::LiquidityAdded { actor_id, .. }
            | Event::TransferExecuted { actor_id, .. },
          ) if ids.contains(&actor_id) => panic!(
            "warmup must skip without invoking a Task: {:?}",
            record.event
          ),
          _ => {}
        }
      }
    });
  }
  // Preserve diagnostics before candidate-specific acceptance assertions.
  std::fs::write(output.join("parent.scale"), storage.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), parent.encode()).unwrap();
  let parent_rows = externalities_with_bound_code(storage.clone()).execute_with(|| {
    ids
      .iter()
      .map(|id| {
        let actor = Actors::active_actor_state(*id).unwrap();
        serde_json::json!({"actor": id, "cycleNonce": actor.identity.cycle_nonce,
        "pending": actor.hot.pending_signal, "run": format!("{:?}", actor.run_state),
        "lpBalance": Assets::balance(lp, &actor.identity.sovereign_account).to_string()})
      })
      .collect::<Vec<_>>()
  });
  std::fs::write(
    output.join("parent.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "block": parent.number(), "root": format!("{:?}", parent.state_root()), "target": target,
      "skippedSteps": skipped_steps, "pipelineActors": openings, "actors": parent_rows,
      "literalNativeHistory": true, "physicalPricingQualified": false,
    }))
    .unwrap(),
  )
  .unwrap();
  assert_eq!(skipped_steps, 108);
  assert_eq!(openings, ids[..72].iter().copied().collect());
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    for (index, id) in ids.iter().enumerate() {
      let actor = Actors::active_actor_state(*id).unwrap();
      assert_eq!(Assets::balance(lp, &actor.identity.sovereign_account), 0);
      assert_eq!(
        Assets::balance(0, &actor.identity.sovereign_account),
        paired_funding
      );
      if index < 72 {
        let run = actor.run_state.unwrap();
        assert_eq!(run.cursor, if index < 36 { 2 } else { 1 });
        assert_eq!(run.cycle_nonce, 1);
        assert_eq!(
          actor.identity.cycle_nonce, 0,
          "identity records completed Cycles"
        );
        assert!(!actor.hot.pending_signal);
      } else {
        assert!(actor.run_state.is_none());
        assert_eq!(actor.identity.cycle_nonce, 0);
        assert_eq!(actor.hot.pending_signal, index == 72);
      }
    }
  });
  let selected_parent = parent.clone();
  let selected = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ContinuousShared,
  );
  std::fs::write(output.join("post.scale"), storage.top.encode()).unwrap();
  let pvf = output.join("selected");
  std::fs::create_dir(&pvf).unwrap();
  pvf_fixture::export(
    &pvf,
    &preset,
    (
      selected.pre_state.clone(),
      selected.post_state.clone(),
      &selected_parent,
      selected.block.clone(),
      selected.inherent_data.clone(),
    ),
  )
  .unwrap();
  let (actions, fees, summaries, service_cursor) = externalities_with_bound_code(storage.clone())
    .execute_with(|| {
      let mut actions = Vec::new();
      let mut fees = Vec::new();
      let mut summaries = Vec::new();
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::LiquidityAdded {
            actor_id,
            lp_minted,
            ..
          }) if ids.contains(&actor_id) => {
            assert!(lp_minted > 0);
            actions.push(actor_id);
          }
          RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, fee, .. })
            if ids.contains(&actor_id) =>
          {
            assert!(fee > 0);
            fees.push(actor_id);
          }
          RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) if ids.contains(&actor_id) => {
            summaries.push(actor_id)
          }
          RuntimeEvent::Actors(Event::StepFailed { actor_id, .. }) if ids.contains(&actor_id) => {
            panic!("real LP terminal Step failed: {:?}", record.event)
          }
          _ => {}
        }
      }
      (actions, fees, summaries, Actors::service_header().cursor)
    });
  std::fs::write(
    output.join("result.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "block": parent.number(), "target": target, "actions": actions, "actionFeeActors": fees,
      "cycleSummaryActors": summaries, "serviceCursor": format!("{service_cursor:?}"),
      "parentRoot": format!("{:?}", selected_parent.state_root()),
      "postRoot": format!("{:?}", selected.block.header.state_root()),
      "pairedBookingQualified": false, "physicalPricingQualified": false,
    }))
    .unwrap(),
  )
  .unwrap();
  assert_eq!(actions, ids[..72]);
  assert_eq!(fees, actions);
  assert_eq!(summaries, actions);
  assert_eq!(service_cursor.unwrap().actor_id, target);
  let before = externalities_with_bound_code(selected.pre_state.clone())
    .execute_with(|| Actors::active_actor_state(target).unwrap());
  let after = externalities_with_bound_code(selected.post_state.clone()).execute_with(|| {
    for id in &ids[..72] {
      let actor = Actors::active_actor_state(*id).unwrap();
      assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
      assert_eq!(actor.hot.cycle_state, CycleState::Idle);
      assert_eq!(actor.identity.cycle_nonce, 1);
      assert!(Assets::balance(lp, &actor.identity.sovereign_account) > 0);
    }
    Actors::active_actor_state(target).unwrap()
  });
  assert_eq!(
    before.encode(),
    after.encode(),
    "fresh paid target remains byte-exact"
  );
  replay_complete_block_in_wasm(&selected, &wasm, "signed-genesis-terminal-user-refusal");
}

#[test]
#[ignore = "exact signed-source checkpoint and SDK-verified native parent header"]
fn retained_terminal_source_fresh_opening_successor() {
  use codec::DecodeAll;
  use polkadot_sdk::frame_support::traits::ExecuteBlock;
  use polkadot_sdk::sp_consensus_aura::{
    digests::CompatibleDigestItem, sr25519::AuthoritySignature,
  };
  let input = |name: &str, sha: &str| {
    let bytes = std::fs::read(std::env::var(name).expect("explicit source checkpoint")).unwrap();
    assert_eq!(
      format!(
        "{:x}",
        H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
      ),
      sha
    );
    bytes
  };
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_TERMINAL_SOURCE_PARENT",
        "7397be5ba157a264ef34e891268cfffe4ab54500656fdfa2da2cca007ac72f7c",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_TERMINAL_SOURCE_POST",
        "216baf777b9edaf3e9f10a1864daa8912815573d4dd5a183443d70cee907974e",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_TERMINAL_SOURCE_BLOCK",
      "aaa7d947521758407b93ebddab032bdba8bc97f6e1668b824cb0d792bee0e606",
    )
    .as_slice(),
  )
  .unwrap();
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let mut verified = sealed.clone().into();
  externalities_with_bound_code(prior).execute_with(|| {
    <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified);
  });
  // Recover the exact native constructor header from the verified packet, not a new parent.
  let mut parent = sealed.header.clone();
  let seal = parent.digest_mut().pop().unwrap();
  assert!(<DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(&seal).is_some());
  assert_eq!(
    format!("{:x}", parent.hash()),
    "1135c9bb8929e85fb24ebc9cad8a403174604dc5554d243a5947ddd4db3c726e"
  );
  assert_eq!(*parent.number(), 187_465);
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let (before, quote, lp, native_before, paired_before, lp_before, sink_before) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let actor = Actors::active_actor_state(87).unwrap();
      assert_eq!(actor.identity.owner, owner);
      assert_eq!(actor.identity.cycle_nonce, 0);
      assert!(actor.hot.pending_signal && actor.run_state.is_none());
      assert_eq!(actor.contract.steps.len(), 1);
      let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
        primitives::AssetKind::Native,
        primitives::AssetKind::Local(0),
      ))
      .unwrap()
      .lp_token;
      let custody = &actor.identity.sovereign_account;
      (
        actor.clone(),
        Actors::actor_cost_quote(87).unwrap(),
        lp,
        Balances::free_balance(custody),
        Assets::balance(0, custody),
        Assets::balance(lp, custody),
        Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()),
      )
    });
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_TERMINAL_OPENING_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::write(output.join("parent.scale"), storage.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), parent.encode()).unwrap();
  let selected_parent = parent.clone();
  let selected = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ActorOnly,
  );
  std::fs::write(output.join("post.scale"), storage.top.encode()).unwrap();
  let pvf = output.join("selected");
  std::fs::create_dir(&pvf).unwrap();
  let preset = polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into();
  pvf_fixture::export(
    &pvf,
    &preset,
    (
      selected.pre_state.clone(),
      selected.post_state.clone(),
      &selected_parent,
      selected.block.clone(),
      selected.inherent_data.clone(),
    ),
  )
  .unwrap();
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut pipeline = Vec::new(); let mut actions = Vec::new(); let mut outputs = Vec::new();
    let mut summaries = Vec::new();
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: 87, fee }) => pipeline.push(fee),
        RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id: 87, fee, .. }) => actions.push(fee),
        RuntimeEvent::Actors(Event::LiquidityAdded { actor_id: 87, lp_minted, .. }) => outputs.push(lp_minted),
        RuntimeEvent::Actors(Event::CycleSummary { actor_id: 87, cycle_nonce, .. }) => summaries.push(cycle_nonce),
        RuntimeEvent::Actors(Event::StepSkipped { actor_id: 87, .. } | Event::StepFailed { actor_id: 87, .. }) =>
          panic!("fresh terminal Task must actually succeed: {:?}", record.event),
        _ => {},
      }
    }
    assert_eq!(pipeline, [quote.prospective_pipeline_fee.unwrap().total_fee]);
    assert_eq!(actions, [quote.maximum_next_action_fee.maximum_effect_fee]);
    assert!(pipeline[0] > 0 && actions[0] > 0);
    assert_eq!(outputs.len(), 1); assert!(outputs[0] > 0);
    assert_eq!(summaries, [1]);
    let after = Actors::active_actor_state(87).unwrap();
    assert_eq!(after.identity.cycle_nonce, 1);
    assert_eq!(after.hot.cycle_state, CycleState::Idle);
    assert!(after.run_state.is_none() && !after.hot.pending_signal);
    assert_eq!(after.contract, before.contract);
    assert_eq!(native_before - Balances::free_balance(&after.identity.sovereign_account),
      crate::UNIT + pipeline[0] + actions[0]);
    assert_eq!(paired_before - Assets::balance(0, &after.identity.sovereign_account), crate::UNIT);
    assert_eq!(Assets::balance(lp, &after.identity.sovereign_account) - lp_before, outputs[0]);
    assert_eq!(Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()) - sink_before,
      pipeline[0] + actions[0]);
    std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
      "block": parent.number(), "actor": 87, "parentNativeHash": format!("{:?}", selected_parent.hash()),
      "parentRoot": format!("{:?}", selected_parent.state_root()),
      "postRoot": format!("{:?}", selected.block.header.state_root()),
      "pipelineFee": pipeline[0].to_string(), "actionFee": actions[0].to_string(), "lpMinted": outputs[0].to_string(),
      "inheritedSignedGenesisAncestry": true, "physicalPricingQualified": false,
    })).unwrap()).unwrap();
  });
  replay_complete_block_in_wasm(
    &selected,
    &wasm,
    "signed-terminal-source-fresh-opening-successor",
  );
}

#[test]
fn running_retry_source_declaration_preflight() {
  TestExternalities::default().execute_with(|| {
    // Conditional declaration inputs only: a real source must prove paid Running authority,
    // zero failure counters and cooldown zero, so RetryLater(2) needs no terminal Close reserve.
    let common = W::service_round_begin_populated()
      .saturating_add(W::service_round_probe_eligible())
      .saturating_add(W::scheduler_actor_state_probe())
      .saturating_add(
        W::service_round_admit_eligible().max(
          W::service_member_retire_interior()
            .max(W::service_member_retire_pair_cursor().max(W::service_member_retire_singleton())),
        ),
      );
    let mut step = lp_step();
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let head = |cursor, steps_in_fragment| {
      Control::maximum_control_weight(
        StepControlWeightContext {
          cursor,
          steps_in_fragment,
          opening_tail_chunks: 0,
          predicate_evaluation_units: 0,
        },
        &step,
      )
      .unwrap()
      .saturating_add(common)
    };
    let base = Weight::from_parts(119_309_001_820, 105_753);
    let drain = Weight::from_parts(118_726_793_820, 93_221);
    let effect = W::task_add_liquidity();
    let limits = crate::configs::BlockResourceBudgetValue::get().limits();
    let effects = effect.saturating_mul(72);
    let pair = |w: Weight| [w.ref_time(), w.proof_size()];
    let rows = [(1, 1), (1, 4), (11, 3)].map(|(cursor, fragment)| {
      let control = head(cursor, fragment);
      serde_json::json!({"cursor": cursor, "fragmentSteps": fragment,
        "headControl": pair(control), "fitsPriorBaseCaller": control.all_lte(base),
        "fitsPriorDrainCaller": control.all_lte(drain)})
    });
    println!(
      "RUNNING_RETRY_DECLARATION_PREFLIGHT {}",
      serde_json::json!({
        "cases": rows, "priorBaseCaller": pair(base), "priorDrainCaller": pair(drain),
        "requiredEffect": pair(effect), "effectPrefix": pair(effects),
        "baseLimit": pair(limits.actor_base_turn()), "actualSourceQualified": false,
        "physicalPricingQualified": false,
      })
    );
    assert!(head(1, 1).all_lte(base));
    assert!(
      !head(1, 1).all_lte(drain),
      "the unchanged prefix does not supply Drain booking coverage"
    );
    assert_eq!(head(1, 1).proof_size() - drain.proof_size(), 8_695);
    assert!(effects.all_lte(limits.actor_base_turn()));
    assert!(
      effects
        .saturating_add(effect)
        .any_gt(limits.actor_base_turn())
    );
    assert!(
      head(1, 1)
        .saturating_add(effect)
        .all_lte(Weight::from_parts(249_780_972_208, 138_602)),
      "Base complete-meter gate must fit as well, not only caller Control"
    );
  });
}
