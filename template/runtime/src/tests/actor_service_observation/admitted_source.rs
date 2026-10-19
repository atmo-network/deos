//! Full native import of the unchanged, genesis-derived signed User candidate.
//! The benchmark-only notices qualify this finite transfer, never physical prices.

use super::*;
use codec::DecodeAll;
use polkadot_sdk::{
  frame_support::traits::ExecuteBlock,
  sp_runtime::traits::Header as HeaderT,
  sp_trie::proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
};

fn input(name: &str, sha256: &str) -> Vec<u8> {
  let bytes = std::fs::read(std::env::var(name).expect("explicit retained source input")).unwrap();
  assert_eq!(
    format!(
      "{:x}",
      H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
    ),
    sha256,
    "retained input must remain byte-exact"
  );
  bytes
}

fn block_notice(phase: Phase, entering: bool) {
  state(|observation| {
    snapshot(observation);
    observation.markers.push((phase, entering));
    if entering {
      observation.stack.push(phase);
    } else {
      assert_eq!(observation.stack.pop(), Some(phase));
    }
  });
}

fn import(block: &crate::Block) {
  <polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, crate::Executive> as ExecuteBlock<crate::Block>>::execute_block(block.clone().into());
}

#[derive(Clone, Copy)]
struct Source {
  actor: u64,
  block_number: u32,
  parent_sha: &'static str,
  post_sha: &'static str,
  block_sha: &'static str,
  parent_root: &'static str,
  terminal: bool,
  target_running: bool,
  prefix_completed_nonce: u64,
}

const RESIDENT: Source = Source {
  actor: 72,
  block_number: 187_464,
  parent_sha: "909e94d30993512909cae3b178b2af05a7ac93b5e53d68905aca218b184818c9",
  post_sha: "0a48e2a32f14ff45b1750419227fbb028667559c2df78bf6cf1d3d1f9463e878",
  block_sha: "a01d81b1d13abca8337c1b5737c868294c67c0c57af0115cf96d7f1883df8bd4",
  parent_root: "df56b43b6b5f196fa6f6aa7d60c97e0311b5f0e007b2451fbf3fa884bdf3b28e",
  terminal: false,
  target_running: false,
  prefix_completed_nonce: 0,
};
const TERMINAL: Source = Source {
  actor: 87,
  block_number: 187_465,
  parent_sha: "7397be5ba157a264ef34e891268cfffe4ab54500656fdfa2da2cca007ac72f7c",
  post_sha: "216baf777b9edaf3e9f10a1864daa8912815573d4dd5a183443d70cee907974e",
  block_sha: "aaa7d947521758407b93ebddab032bdba8bc97f6e1668b824cb0d792bee0e606",
  parent_root: "06924b79de11bd02572abbb263641fd3cb8944db0915112b9c767c014c3db3a3",
  terminal: true,
  target_running: false,
  prefix_completed_nonce: 0,
};
const RUNNING: Source = Source {
  actor: 88,
  block_number: 187_564,
  parent_sha: "a2a777adb5279351f990dded85da96acac1ffee9e7a22c80264d55980fec6b88",
  post_sha: "b4702064d9831c8dd0c613fcdc0caf6142ab353d02ff69bf364a3de99e61399a",
  block_sha: "b1d447323ccc4ed14134ab41040137cc6f3ccaa79cecc95b37b0376b94dc06e8",
  parent_root: "54bd48059069ef71169bcb28f25cb9e14a7c685e4aa68558f9bd00ce535ea5b7",
  terminal: true,
  target_running: true,
  prefix_completed_nonce: 2,
};

fn fixture(parent: &Storage, source: Source) -> Fixture {
  polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
    let actor = Actors::active_actor_state(source.actor).unwrap();
    assert_eq!(System::block_number(), source.block_number - 1);
    assert_eq!(actor.identity.cycle_nonce, 0);
    let pipeline_fee = if source.target_running {
      assert_eq!(actor.hot.cycle_state, CycleState::Running);
      assert!(!actor.hot.pending_signal);
      assert_eq!(actor.hot.unsuccessful_attempt_streak, 0);
      let run = actor.run_state.as_ref().unwrap();
      assert_eq!(run.cursor, 1);
      assert_eq!(run.cycle_nonce, 1);
      assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
      assert!(run.suspension.is_none() && run.eligible_at <= source.block_number);
      assert_eq!(actor.contract.steps.len(), 2);
      assert_eq!(
        actor.contract.steps[1].on_error,
        pallet_deos_actors::StepErrorPolicy::RetryLater { max_attempts: 2 }
      );
      assert_eq!(actor.contract.cooldown_blocks, 0);
      assert!(
        actor.contract.window.is_none()
          && actor.contract.close_when.is_none()
          && actor.contract.auto_close_at_cycle_nonce.is_none()
      );
      let receipts = System::events()
        .into_iter()
        .filter_map(|record| match record.event {
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::PipelineFeeCharged {
            actor_id,
            fee,
          }) if actor_id == source.actor => Some(fee),
          _ => None,
        })
        .collect::<Vec<_>>();
      assert_eq!(
        receipts.len(),
        1,
        "the real parent supplies one paid Opening receipt"
      );
      assert!(receipts[0] > 0);
      receipts[0]
    } else {
      assert!(actor.hot.pending_signal && actor.run_state.is_none());
      Actors::actor_cost_quote(source.actor)
        .unwrap()
        .prospective_pipeline_fee
        .unwrap()
        .total_fee
    };
    let native = <Runtime as Config>::FeeNativeAssetId::get();
    let paired = Asset::Local(0);
    let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((native, paired))
      .unwrap()
      .lp_token;
    let sink = <Runtime as Config>::FeeSink::get();
    Fixture {
      actor: source.actor,
      payer: actor.identity.sovereign_account,
      lp: Asset::Local(lp),
      a: native,
      b: paired,
      amount: if source.terminal {
        crate::UNIT
      } else {
        12 * crate::UNIT
      },
      spend: crate::UNIT,
      sink_before: Assets::balance(&sink, native),
      sink,
      native,
      pipeline_fee,
      inspection_predecessors: source.terminal.then_some(72),
      paid_prefix: None,
    }
  })
}

#[test]
#[ignore = "explicit hash-bound retained signed source, not a reconstructed parent"]
fn retained_signed_user_block_classifies_refusal_boundary() {
  classify_source(RESIDENT);
}

#[test]
#[ignore = "exact terminal source input pins, not the resident source or reconstructed history"]
fn retained_terminal_user_block_classifies_refusal_boundary() {
  classify_source(TERMINAL);
}

#[test]
#[ignore = "exact paid Running-policy source inputs, never reconstructed Run authority"]
fn retained_paid_running_block_classifies_refusal_boundary() {
  classify_source(RUNNING);
}

fn classify_source(source: Source) {
  let target = source.actor;
  let parent = Storage {
    top: State::decode_all(&mut input("DEOS_ADMITTED_SOURCE_PARENT", source.parent_sha).as_slice())
      .unwrap(),
    ..Default::default()
  };
  let expected =
    State::decode_all(&mut input("DEOS_ADMITTED_SOURCE_POST", source.post_sha).as_slice()).unwrap();
  let block =
    crate::Block::decode_all(&mut input("DEOS_ADMITTED_SOURCE_BLOCK", source.block_sha).as_slice())
      .unwrap();
  let parent_root = Layout::trie_root(parent.top.clone());
  assert_eq!(format!("{parent_root:x}"), source.parent_root);
  assert_eq!(*block.header.number(), source.block_number);
  assert_eq!(
    Layout::trie_root(expected.clone()),
    *block.header.state_root()
  );
  assert_eq!(
    format!(
      "{:x}",
      H256::from(polkadot_sdk::sp_io::hashing::sha2_256(
        &parent.top[polkadot_sdk::sp_core::storage::well_known_keys::CODE],
      ))
    ),
    "2df308749408946a7b4ad6f90b3d5aabca8defe1fa03c7c11806fac409ccb8d9"
  );
  let fixture = fixture(&parent, source);
  let output = std::path::PathBuf::from(std::env::var("DEOS_SERVICE_OBSERVATION_OUTPUT").unwrap());
  std::fs::create_dir(&output).unwrap();
  // Existing Encounter root probes select only the terminal source's 73rd encounter.
  // Both arms retain identical callbacks; neither is an observer-free timing baseline.
  let observer = if source.terminal {
    notice
  } else {
    block_notice
  };
  let observed = run_call(&parent, &fixture, true, false, observer, true, || {
    import(&block)
  });
  let plain = run_call(&parent, &fixture, false, false, observer, true, || {
    import(&block)
  });
  std::fs::write(output.join("observed-post.scale"), observed.top.encode()).unwrap();
  std::fs::write(output.join("plain-post.scale"), plain.top.encode()).unwrap();
  assert_eq!(observed.root, plain.root);
  assert_eq!(observed.top, plain.top);
  assert_eq!(
    observed.top, expected,
    "benchmark notices must preserve the default-native/Wasm TOP"
  );
  assert_eq!(observed.root, *block.header.state_root());
  assert_eq!(observed.db, plain.db);
  assert_eq!(observed.proof, plain.proof);
  assert_eq!(observed.observation.markers, plain.observation.markers);
  assert!(!observed.observation.fault_applied);
  let bookings = observed
    .observation
    .markers
    .iter()
    .filter_map(|(phase, entering)| match (*phase, *entering) {
      (phase @ Phase::Booking { actor, .. }, true) if actor == target => Some(phase),
      _ => None,
    })
    .collect::<Vec<_>>();
  let heads = observed
    .observation
    .markers
    .iter()
    .filter_map(|(phase, entering)| match (*phase, *entering) {
      (phase @ Phase::HeadAdmission { actor, .. }, true) if actor == target => Some(phase),
      _ => None,
    })
    .collect::<Vec<_>>();
  let compact = observed
    .proof
    .to_compact_proof::<Blake2Hasher>(parent_root)
    .unwrap();
  export_case(&output, "block", parent_root, &observed, &compact);
  assert!(
    !heads.is_empty(),
    "the actual loaded-head guard must be observed, not inferred from final shortage"
  );
  std::fs::write(output.join("boundary-before-assertions.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "target": target, "headAdmissions": format!("{heads:?}"), "bookings": format!("{bookings:?}"),
    "encounterRollbackRoots": observed.observation.refusal_roots.map(|root| root.map(|root| format!("{root:?}"))),
    "acceptance": false, "physicalPricingQualified": false,
  })).unwrap()).unwrap();
  if source.terminal {
    assert_eq!(
      bookings.len(),
      1,
      "terminal source must reach actual Base paired booking"
    );
    assert!(
      observed
        .observation
        .refusal_roots
        .iter()
        .all(Option::is_some)
    );
    assert_eq!(
      observed.observation.refusal_roots[0],
      observed.observation.refusal_roots[1]
    );
    assert_eq!(
      observed.observation.refusal_roots,
      plain.observation.refusal_roots
    );
  } else {
    assert!(
      bookings.is_empty(),
      "resident source refuses before quote/preparation and booking"
    );
  }
  let mut encounter = Vec::new();
  for marker in &observed.observation.markers {
    if *marker == (Phase::Encounter, true) {
      assert!(encounter.is_empty());
    }
    if !encounter.is_empty() || *marker == (Phase::Encounter, true) {
      encounter.push(*marker);
    }
    if *marker == (Phase::Encounter, false) {
      if encounter.iter().any(|(phase, entering)| {
        matches!((*phase, *entering), (Phase::HeadAdmission { actor, .. }, true) if actor == target)
      }) {
        if source.terminal && encounter.iter().any(|(phase, entering)| matches!((*phase, *entering), (Phase::Booking { actor, .. }, true) if actor == target)) {
          assert!(encounter.contains(&(Phase::Preflight, true)));
          assert!(encounter.contains(&(Phase::Preflight, false)));
        } else {
          assert!(!encounter.contains(&(Phase::Preflight, true)));
        }
        for phase in [
          Phase::Task,
          Phase::Retirement,
          Phase::Settlement,
          Phase::Collection,
          Phase::Authentication,
        ] {
          assert!(
            !encounter.contains(&(phase, true)),
            "refused target cannot invoke or publish an effect"
          );
        }
      }
      encounter.clear();
    }
  }
  assert!(encounter.is_empty());
  let mut final_ext = polkadot_sdk::sp_io::TestExternalities::new(Storage {
    top: observed.top.clone(),
    ..Default::default()
  });
  let (actions, pipelines, limits, final_usage) = final_ext.execute_with(|| {
    let after = Actors::active_actor_state(target).unwrap();
    let before = polkadot_sdk::sp_io::TestExternalities::new(parent.clone())
      .execute_with(|| Actors::active_actor_state(target).unwrap());
    assert_eq!(after.encode(), before.encode());
    let hold = pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(target);
    assert_eq!(observed.top.get(&hold), parent.top.get(&hold));
    let native_account =
      polkadot_sdk::frame_system::Account::<Runtime>::hashed_key_for(&fixture.payer);
    assert_eq!(
      observed.top.get(&native_account),
      parent.top.get(&native_account)
    );
    for asset in [
      0,
      match fixture.lp {
        Asset::Local(id) => id,
        _ => unreachable!(),
      },
    ] {
      let key =
        polkadot_sdk::pallet_assets::Account::<Runtime>::hashed_key_for(asset, &fixture.payer);
      assert_eq!(observed.top.get(&key), parent.top.get(&key));
    }
    assert!(Actors::block_resource_state().is_none());
    let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
    assert!(!telemetry.optional_actor_work_halted());
    let mut actions = BTreeSet::new();
    let mut pipelines = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for record in System::events() {
      match record.event {
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::ActionFeeCharged {
          actor_id,
          fee,
          ..
        }) => {
          assert_ne!(actor_id, target);
          assert!(fee > 0 && actions.insert(actor_id));
        }
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::PipelineFeeCharged {
          actor_id,
          fee,
          ..
        }) => {
          assert_ne!(actor_id, target);
          assert!(fee > 0 && pipelines.insert(actor_id));
        }
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::LiquidityAdded {
          actor_id,
          lp_minted,
          ..
        }) => {
          assert_ne!(actor_id, target);
          assert!(lp_minted > 0 && outputs.insert(actor_id));
        }
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::StepFailed { actor_id, .. }) => {
          panic!("unexpected Task failure {actor_id}")
        }
        crate::RuntimeEvent::Actors(
          pallet_deos_actors::Event::StepSkipped { actor_id, .. }
          | pallet_deos_actors::Event::CycleStarted { actor_id, .. }
          | pallet_deos_actors::Event::CycleSummary { actor_id, .. }
          | pallet_deos_actors::Event::CycleSuspended { actor_id, .. }
          | pallet_deos_actors::Event::CycleContinued { actor_id, .. }
          | pallet_deos_actors::Event::ActorClosed { actor_id, .. },
        ) if actor_id == target => {
          panic!("an uncommitted refusal fabricates no Step or lifecycle outcome")
        }
        _ => {}
      }
    }
    assert!(!actions.is_empty());
    assert_eq!(
      actions, outputs,
      "every actual LP effect has its User invocation receipt"
    );
    for id in &pipelines {
      assert!(actions.contains(id));
    }
    if source.terminal {
      assert!(
        pipelines.is_empty(),
        "paid terminal prefixes collect no second Pipeline fee"
      );
      assert_eq!(actions, (15..87).collect());
    } else {
      assert!(
        !pipelines.is_empty(),
        "fresh User Openings are separately observable"
      );
    }
    for id in &actions {
      let before = polkadot_sdk::sp_io::TestExternalities::new(parent.clone())
        .execute_with(|| Actors::active_actor_state(*id).unwrap());
      let after = Actors::active_actor_state(*id).unwrap();
      if source.terminal {
        let run = before.run_state.unwrap();
        assert_eq!(run.cycle_nonce, source.prefix_completed_nonce + 1);
        assert_eq!(run.cursor, if *id < 51 { 2 } else { 1 });
        assert_eq!(before.identity.cycle_nonce, source.prefix_completed_nonce);
        assert!(after.run_state.is_none() && !after.hot.pending_signal);
        assert_eq!(
          after.identity.cycle_nonce,
          source.prefix_completed_nonce + 1
        );
        assert_eq!(after.hot.cycle_state, CycleState::Idle);
      } else {
        let next = after
          .run_state
          .as_ref()
          .expect("twelve-Step prefix remains Running");
        if pipelines.contains(id) {
          assert!(before.hot.pending_signal && before.run_state.is_none());
          assert_eq!(next.cursor, 1);
        } else {
          assert_eq!(
            next.cursor,
            before.run_state.unwrap().cursor + 1,
            "paid Running prefix advances once without another Pipeline fee"
          );
        }
      }
    }
    let limits = telemetry.budget().limits();
    let usage = telemetry.usage();
    assert!(usage.actor_control_used().all_lte(limits.actor_control()));
    assert!(
      usage
        .actor_effect_used()
        .saturating_add(usage.user_dispatch_used())
        .all_lte(limits.shared_economic())
    );
    if source.terminal {
      let key = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key();
      let bytes = observed
        .observation
        .operations
        .iter()
        .rev()
        .find(|op| op.method == "put" && op.key == key && op.value.is_some())
        .unwrap()
        .value
        .as_ref()
        .unwrap();
      let last = pallet_deos_actors::BlockResourceState::<crate::BlockNumber>::decode_all(
        &mut bytes.as_slice(),
      )
      .unwrap();
      assert_eq!(last.outstanding_reservations(), 0);
      assert_eq!(last.usage(), usage);
      assert_eq!(last.budget().unwrap().limits(), limits);
    }
    (actions, pipelines, limits, usage)
  });
  let mut qualified_successes = BTreeSet::new();
  for encounter in observed
    .observation
    .markers
    .split_inclusive(|marker| *marker == (Phase::Encounter, false))
  {
    let actor = encounter
      .iter()
      .find_map(|(phase, entering)| match (*phase, *entering) {
        (Phase::Booking { actor, .. }, true)
          if (source.terminal && actions.contains(&actor)) || pipelines.contains(&actor) =>
        {
          Some(actor)
        }
        _ => None,
      });
    if let Some(actor) = actor {
      let phases = if source.terminal {
        vec![
          Phase::Preflight,
          Phase::Task,
          Phase::Retirement,
          Phase::Settlement,
          Phase::Collection,
          Phase::Authentication,
        ]
      } else {
        vec![
          Phase::Preflight,
          Phase::Task,
          Phase::Settlement,
          Phase::Collection,
          Phase::Authentication,
        ]
      };
      let positions = phases
        .into_iter()
        .map(|phase| {
          encounter
            .iter()
            .position(|marker| *marker == (phase, true))
            .unwrap_or_else(|| panic!("missing {phase:?} in Actor {actor} selected success"))
        })
        .collect::<Vec<_>>();
      assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "publication/settlement/fee/authentication order"
      );
      if !source.terminal {
        assert!(
          !encounter.contains(&(Phase::Retirement, true)),
          "resident continuation does not retire"
        );
      }
      assert!(qualified_successes.insert(actor));
    }
  }
  assert_eq!(
    qualified_successes,
    if source.terminal {
      actions.clone()
    } else {
      pipelines.clone()
    },
    "each selected success reaches collection and final authority validation"
  );
  for asset in [fixture.native, fixture.b, fixture.lp] {
    let balance = |top: &State| {
      polkadot_sdk::sp_io::TestExternalities::new(Storage {
        top: top.clone(),
        ..Default::default()
      })
      .execute_with(|| Assets::balance(&fixture.payer, asset))
    };
    assert_eq!(
      balance(&observed.top),
      balance(&parent.top),
      "target custody must remain exact"
    );
  }
  let weight =
    |[time, proof]: [u64; 2]| polkadot_sdk::frame_support::weights::Weight::from_parts(time, proof);
  for (index, head) in heads.iter().enumerate() {
    let Phase::HeadAdmission {
      meter_remaining,
      caller_control,
      required_control,
      required_effect,
      ..
    } = *head
    else {
      unreachable!()
    };
    let control = weight(required_control);
    let total = control.saturating_add(weight(required_effect));
    assert!(
      total.all_lte(weight(meter_remaining)),
      "the complete meter is not the refusing owner here"
    );
    let caller = weight(caller_control.expect("Control is owned by the actual Service caller"));
    assert!(control.ref_time() <= caller.ref_time());
    if source.terminal && index == 0 {
      assert!(
        control.all_lte(caller),
        "Base reaches booking only after the complete head guard fits"
      );
    } else {
      assert!(
        control.proof_size() > caller.proof_size(),
        "remaining encounters refuse on caller Control ProofSize"
      );
    }
  }
  if source.terminal {
    assert_eq!(heads.len(), 2);
    let Phase::Booking {
      ledger_effect,
      ledger_user,
      base_effect,
      required_effect,
      ..
    } = bookings[0]
    else {
      unreachable!()
    };
    assert!(base_effect);
    assert_eq!(ledger_user, [0, 0]);
    assert_eq!(required_effect, [2_399_421_000, 34_255]);
    assert_eq!(ledger_effect, [172_758_312_000, 2_466_360]);
    assert_eq!(
      final_usage.actor_effect_used(),
      weight(ledger_effect),
      "failed target booking retains only committed prefix effects"
    );
    let attempted = weight(ledger_effect).saturating_add(weight(required_effect));
    assert!(attempted.ref_time() <= limits.actor_base_turn().ref_time());
    assert_eq!(
      attempted.proof_size() - limits.actor_base_turn().proof_size(),
      1_406
    );
    assert!(observed.observation.markers.split_inclusive(|m| *m == (Phase::Encounter, false)).nth(72).unwrap()
      .iter().any(|(phase, entering)| matches!((*phase, *entering), (Phase::Booking { actor, .. }, true) if actor == target)),
      "root probes must select the actual target booking encounter");
  }
  verify_original_and_compact(&parent, parent_root, &block, &observed, &compact);
  let report = serde_json::json!({
    "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root),
    "target": target, "headAdmissions": format!("{heads:?}"), "bookings": format!("{bookings:?}"),
    "pairedBookingObserved": source.terminal,
    "refusalBoundary": if source.terminal { "base-paired-booking-and-drain-caller-control-proof" } else { "loaded-head-caller-control-proof-guard" },
    "actionActors": actions, "pipelineActors": pipelines,
    "actorEffectUsed": [final_usage.actor_effect_used().ref_time(), final_usage.actor_effect_used().proof_size()],
    "terminalRetirementActors": if source.terminal { qualified_successes.len() } else { 0 },
    "encounterRollbackRoots": observed.observation.refusal_roots.map(|root| root.map(|root| format!("{root:?}"))),
    "residentOpeningWithoutRetirement": !source.terminal,
    "completeKMaximumQualified": false, "defaultNativeWasmTopEqual": true,
    "targetCustodyAndHoldExact": true, "targetRunning": source.target_running,
    "parentPipelineFeeReceipt": source.target_running.then(|| fixture.pipeline_fee.to_string()),
    "actualRetryTransitionQualified": false, "physicalPricingQualified": false,
    "rawWitnessBytes": observed.proof.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
  });
  std::fs::write(
    output.join("report.json"),
    serde_json::to_vec_pretty(&report).unwrap(),
  )
  .unwrap();
  println!("ADMITTED_USER_REFUSAL_BOUNDARY {report}");
}

fn verify_original_and_compact(
  parent: &Storage,
  parent_root: H256,
  block: &crate::Block,
  observed: &ResultRow,
  compact: &polkadot_sdk::sp_trie::CompactProof,
) {
  let mut original = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(
    Layout::trie_build(&mut original, parent.top.clone()),
    parent_root
  );
  assert!(
    observed.proof.iter_nodes().all(|node| {
      HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)
    }),
    "all recorded nodes must belong to the original parent, never a generated post-state"
  );
  assert_eq!(
    replay(
      parent_root,
      &observed.proof,
      &parent.top,
      &observed.top,
      &observed.observation.operations
    ),
    Ok(observed.root)
  );
  assert!(
    replay(
      parent_root,
      &StorageProof::empty(),
      &parent.top,
      &observed.top,
      &observed.observation.operations
    )
    .is_err()
  );
  let (db, root) = compact.to_memory_db::<Blake2Hasher>(&parent_root).unwrap();
  assert_eq!(root, parent_root);
  let recorder = polkadot_sdk::sp_trie::recorder::Recorder::<Blake2Hasher>::default();
  let backend = polkadot_sdk::sp_state_machine::TrieBackendBuilder::new(db, root)
    .with_recorder(recorder.clone())
    .build();
  let mut overlay = OverlayedChanges::default();
  let mut extensions = polkadot_sdk::sp_externalities::Extensions::default();
  extensions.register(ProofSizeExt::new(RecordingProofSizeProvider::new(recorder)));
  let mut ext = Ext::new(&mut overlay, &backend, Some(&mut extensions));
  let checked = polkadot_sdk::sp_externalities::set_and_run_with_externalities(&mut ext, || {
    import(block);
    H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1))
  });
  assert_eq!(
    checked, observed.root,
    "the same full native caller must consume compact proof-only storage"
  );
}

#[path = "admitted_source/opening.rs"]
mod opening;

#[path = "admitted_source/retry.rs"]
mod retry;

#[path = "admitted_source/lifecycle.rs"]
mod lifecycle;

#[path = "admitted_source/geometry.rs"]
mod geometry;
