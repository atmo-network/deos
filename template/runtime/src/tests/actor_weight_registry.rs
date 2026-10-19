use std::collections::{BTreeMap, BTreeSet};

use polkadot_sdk::{
  frame_support::{self, derive_impl},
  frame_system,
  sp_core::H256,
  sp_weights::{RuntimeDbWeight, Weight},
};
use serde_json::{Value, json};

type ActualWeights = crate::weights::pallet_deos_actors::SubstrateWeight<crate::Runtime>;
type ProbeWeights = crate::weights::pallet_deos_actors::SubstrateWeight<ProbeRuntime>;
type Domains = Vec<(String, u32, u32)>;

const GENERATED: &str = include_str!("../weights/pallet_deos_actors.rs");

frame_support::construct_runtime! {
  pub struct ProbeRuntime {
    System: frame_system,
  }
}

frame_support::parameter_types! {
  pub static ProbeDbWeight: RuntimeDbWeight = RuntimeDbWeight { read: 0, write: 0 };
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for ProbeRuntime {
  type Block = frame_system::mocking::MockBlock<Self>;
  type DbWeight = ProbeDbWeight;
}

fn declared_domains(source: &str) -> Result<BTreeMap<String, Domains>, &'static str> {
  let mut methods = BTreeMap::new();
  let mut ranges = BTreeMap::new();
  for line in source.lines().map(str::trim) {
    if line.starts_with("/// The range of component ") {
      let parts = line.split('`').collect::<Vec<_>>();
      let name = *parts.get(1).ok_or("missing component")?;
      let range = parts.get(3).ok_or("missing range")?;
      let range = range
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or("invalid range brackets")?;
      let (low, high) = range.split_once(',').ok_or("invalid range pair")?;
      let low = low.trim().parse::<u32>().map_err(|_| "invalid low")?;
      let high = high.trim().parse::<u32>().map_err(|_| "invalid high")?;
      if low > high || ranges.insert(name.to_owned(), (low, high)).is_some() {
        return Err("invalid or duplicate range");
      }
    } else if let Some(signature) = line.strip_prefix("fn ") {
      let (name, tail) = signature.split_once('(').ok_or("invalid method")?;
      let args = tail
        .strip_suffix(") -> Weight {")
        .ok_or("unsupported method signature")?;
      let mut domains = Vec::new();
      for arg in args.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (parameter, kind) = arg.split_once(':').ok_or("invalid parameter")?;
        if kind.trim() != "u32" {
          return Err("unsupported parameter type");
        }
        let parameter = parameter.trim();
        let (low, high) = ranges.remove(parameter).ok_or("missing declared range")?;
        domains.push((parameter.to_owned(), low, high));
      }
      if !ranges.is_empty() || methods.insert(name.to_owned(), domains).is_some() {
        return Err("orphan range or duplicate method");
      }
    }
  }
  if methods.is_empty() || !ranges.is_empty() {
    return Err("empty registry or dangling range");
  }
  Ok(methods)
}

fn corners(domains: &[(String, u32, u32)]) -> Vec<Vec<u32>> {
  let mut points = vec![Vec::new()];
  for (_, low, high) in domains {
    points = points
      .into_iter()
      .flat_map(|prefix| {
        let values = if low == high {
          vec![*low]
        } else {
          vec![*low, *high]
        };
        values.into_iter().map(move |value| {
          let mut point = prefix.clone();
          point.push(value);
          point
        })
      })
      .collect();
  }
  points
}

fn observe_declaration(
  name: &str,
  domains: Domains,
  actual: impl Fn(&[u32]) -> Weight,
  probe: impl Fn(&[u32]) -> Weight,
) -> Value {
  let db = <crate::Runtime as frame_system::Config>::DbWeight::get();
  let samples = corners(&domains)
    .into_iter()
    .map(|arguments| {
      ProbeDbWeight::set(RuntimeDbWeight { read: 0, write: 0 });
      let coefficient = probe(&arguments);
      ProbeDbWeight::set(RuntimeDbWeight { read: 1, write: 0 });
      let read_unit = probe(&arguments);
      ProbeDbWeight::set(RuntimeDbWeight { read: 0, write: 1 });
      let write_unit = probe(&arguments);
      let reads = read_unit
        .ref_time()
        .checked_sub(coefficient.ref_time())
        .unwrap();
      let writes = write_unit
        .ref_time()
        .checked_sub(coefficient.ref_time())
        .unwrap();
      assert_eq!(coefficient.proof_size(), read_unit.proof_size());
      assert_eq!(coefficient.proof_size(), write_unit.proof_size());
      let db_charge = db.reads_writes(reads, writes);
      let full = actual(&arguments);
      assert_ne!(
        full,
        Weight::MAX,
        "{name}: no saturated declaration accepted"
      );
      assert_eq!(coefficient.checked_add(&db_charge), Some(full), "{name}");
      ProbeDbWeight::set(db);
      assert_eq!(probe(&arguments), full, "{name}: production DB binding");
      json!({
        "arguments": arguments,
        "coefficient": [coefficient.ref_time(), coefficient.proof_size()],
        "reads": reads, "writes": writes,
        "db_charge": [db_charge.ref_time(), db_charge.proof_size()],
        "full": [full.ref_time(), full.proof_size()]
      })
    })
    .collect::<Vec<_>>();
  ProbeDbWeight::set(RuntimeDbWeight { read: 0, write: 0 });
  json!({"method": name, "domains": domains, "samples": samples})
}

#[test]
fn generated_weight_registry_source_ranges_fail_closed() {
  assert!(declared_domains("fn a(p: u32, ) -> Weight {").is_err());
  assert!(declared_domains("fn a() -> Weight {\nfn a() -> Weight {").is_err());
  assert!(
    declared_domains("/// The range of component `p` is `[2, 1]`.\nfn a(p: u32, ) -> Weight {")
      .is_err()
  );
  assert!(
    declared_domains("/// The range of component `p` is `[1, 2]`.\nfn a() -> Weight {").is_err()
  );
  let parsed =
    declared_domains("/// The range of component `p` is `[1, 2]`.\nfn a(p: u32, ) -> Weight {")
      .unwrap();
  assert_eq!(corners(&parsed["a"]), vec![vec![1], vec![2]]);
  assert_eq!(corners(&[("p".into(), 1, 1)]), vec![vec![1]]);
  assert_eq!(corners(&[]), vec![Vec::<u32>::new()]);
  assert_eq!(
    corners(&[("s".into(), 1, 2), ("p".into(), 0, 3)]),
    vec![vec![1, 0], vec![1, 3], vec![2, 0], vec![2, 3]]
  );
}

#[test]
fn generated_weight_registry_evaluates_compiled_owner_corners_with_db_terms() {
  use pallet_deos_actors::WeightInfo as _;
  let declared = declared_domains(GENERATED).unwrap();
  let mut rows = Vec::new();
  macro_rules! register {
    ($method:ident) => {
      rows.push(observe_declaration(
        stringify!($method), Vec::new(),
        |_| ActualWeights::$method(), |_| ProbeWeights::$method(),
      ));
    };
    ($method:ident; $($arg:ident: [$low:literal, $high:literal]),+) => {
      rows.push(observe_declaration(
        stringify!($method), vec![$((stringify!($arg).into(), $low, $high)),+],
        |args| {
          let [$($arg),+] = args else { panic!("registry arity"); };
          ActualWeights::$method($(*$arg),+)
        },
        |args| {
          let [$($arg),+] = args else { panic!("registry arity"); };
          ProbeWeights::$method($(*$arg),+)
        },
      ));
    };
  }
  register!(create_user_actor);
  register!(create_user_actor_at_slot);
  register!(create_system_actor);
  register!(create_system_actor_at_sovereign_id);
  register!(create_dormant_system_actor);
  register!(activate_actor);
  register!(deactivate_actor);
  register!(pause_actor);
  register!(resume_actor);
  register!(manual_trigger);
  register!(address_event_trigger_occurrence);
  register!(close_actor);
  register!(close_actor_policy_bearing);
  register!(behavior_service_inspection);
  register!(update_contract);
  register!(set_global_circuit_breaker);
  register!(dependency_scan_source_probe);
  register!(process_dependency_scan_unit);
  register!(process_dependency_scan_completion_unit);
  register!(complete_cycle_to_parked_balance);
  register!(process_pending_parked_balance_event);
  register!(process_due_parked_balance_review);
  register!(process_due_parked_balance_review_deep_index);
  register!(classify_due_block_deadline);
  register!(classify_due_tick_deadline);
  register!(deadline_destination_search; p: [1, 313]);
  register!(return_due_block_deadline_to_service_deep_index);
  register!(return_due_block_deadline_to_service);
  register!(set_active_actor_limit);
  register!(permissionless_sweep);
  register!(permissionless_sweep_many; n: [1, 5]);
  register!(fee_collection);
  register!(action_invocation_receipt);
  register!(task_transfer);
  register!(task_burn);
  register!(task_mint);
  register!(predicate_set_evaluation; c: [1, 4]);
  register!(task_stop_cycle);
  register!(xcm_asset_deposit);
  register!(task_add_liquidity);
  register!(task_donate_liquidity);
  register!(task_remove_liquidity);
  register!(task_stake);
  register!(task_unstake);
  register!(task_dex_exact_in);
  register!(task_dex_exact_out);
  register!(scheduler_on_initialize_cutoff);
  register!(scheduler_on_idle_base);
  register!(service_member_publish_empty);
  register!(service_member_publish_populated);
  register!(service_member_retire_singleton);
  register!(service_member_retire_pair_cursor);
  register!(service_member_retire_interior);
  register!(service_member_insert_populated);
  register!(service_member_to_deadline_new_key);
  register!(service_round_begin_populated);
  register!(service_round_probe_eligible);
  register!(service_round_admit_eligible);
  register!(dependency_publication_begun_empty_source_list);
  register!(dependency_publication_begun_populated_source_list);
  register!(dependency_publication_coalesced_active_source);
  register!(contract_geometry_create; c: [0, 3]);
  register!(contract_geometry_close; c: [0, 3]);
  register!(contract_geometry_reconstruct; c: [0, 3]);
  register!(current_step_load_head);
  register!(current_step_load_tail; s: [1, 4]);
  register!(current_step_plan_opening_head);
  register!(current_step_plan_suspended_head);
  register!(current_step_plan_running_tail; s: [1, 4]);
  register!(predicate_asset_evaluation; p: [1, 4]);
  register!(predicate_observation_heavy_evaluation; o: [1, 3]);
  register!(scheduler_actor_state_probe);
  register!(at_time_trigger_occurrence);
  register!(cadenced_trigger_occurrence);
  register!(cadenced_running_rearm);
  register!(cadenced_suspended_service_rearm);
  register!(cadenced_suspended_deadline_rearm);
  register!(scheduler_inner_zero_step_complete);
  register!(scheduler_paged_execute_opening_max);
  register!(scheduler_inner_opening_failed_min; t: [0, 3]);
  register!(scheduler_inner_opening_retry_min; t: [0, 3]);
  register!(scheduler_inner_opening_failed_max; t: [0, 3]);
  register!(scheduler_inner_opening_retry_max; t: [0, 3]);
  register!(scheduler_inner_opening_complete_min; t: [0, 3]);
  register!(scheduler_inner_opening_user_complete_header_max);
  register!(scheduler_inner_opening_user_complete_header_max_tail; t: [1, 3]);
  register!(scheduler_inner_opening_progress_min; t: [1, 3]);
  register!(scheduler_inner_opening_user_progress_min; t: [1, 3]);
  register!(scheduler_inner_opening_complete_max; t: [0, 3]);
  register!(scheduler_inner_opening_progress_max; t: [1, 3]);
  register!(scheduler_inner_running_complete; s: [1, 4], p: [0, 4]);
  register!(scheduler_inner_running_progress; s: [2, 4], p: [0, 4]);
  register!(scheduler_inner_suspended_tail_retry; s: [1, 4], p: [0, 4]);
  register!(scheduler_inner_suspended_tail_complete; s: [1, 4], p: [0, 4]);
  register!(scheduler_inner_suspended_tail_progress; s: [2, 4], p: [0, 4]);
  register!(scheduler_inner_suspended_head_retry; n: [0, 22], p: [0, 4]);
  register!(scheduler_inner_suspended_head_complete; p: [0, 4]);
  register!(scheduler_inner_suspended_head_progress; n: [0, 22], p: [0, 4]);
  register!(scheduler_inner_suspended_head_opening_retry; n: [0, 22], p: [0, 3]);
  register!(scheduler_inner_suspended_head_opening_complete; p: [0, 3]);
  register!(scheduler_inner_suspended_head_opening_progress; n: [0, 22], p: [0, 3]);
  register!(scheduler_service_successful_interior);
  register!(scheduler_service_retry_to_deadline);
  register!(scheduler_service_retry_to_deadline_new_key);
  register!(scheduler_service_late_refusal_rollback);
  register!(scheduler_service_terminal_retain_close);
  register!(scheduler_service_minimal_apoptosis);
  register!(scheduler_due_deadline_to_service);
  register!(scheduler_due_deadline_to_service_deep_index);
  register!(run_progress);
  register!(run_suspend);
  register!(run_complete);
  register!(run_cancel);
  register!(transaction_extension_ingress_base);
  register!(transaction_extension_ingress_notify);
  register!(maximum_context_inherent);
  register!(block_resource_finalize);
  register!(block_resource_meter_extension);
  register!(maximum_xcm_version_discovery);
  register!(scheduler_inner_suspended_user_tail_retry; s: [1, 4], p: [0, 4]);
  register!(scheduler_inner_suspended_user_tail_progress; s: [2, 4], p: [0, 4]);
  let mut visited = BTreeSet::new();
  for row in &rows {
    let name = row["method"].as_str().unwrap();
    assert!(visited.insert(name.to_owned()), "duplicate registry owner");
    assert_eq!(
      row["domains"],
      json!(declared[name]),
      "{name}: source range drift"
    );
    assert!(!row["samples"].as_array().unwrap().is_empty());
  }
  assert_eq!(visited, declared.keys().cloned().collect());
  let digest = |bytes| {
    format!(
      "{:?}",
      H256::from(polkadot_sdk::sp_io::hashing::sha2_256(bytes))
    )
  };
  let db = <crate::Runtime as frame_system::Config>::DbWeight::get();
  println!(
    "ACTOR_WEIGHT_REGISTRY_HEADER {}",
    json!({
      "format": "deos.actor.generated-weight-registry", "format_version": 1,
      "source_sha256": digest(GENERATED.as_bytes()),
      "trait_sha256": digest(include_bytes!("../../../pallets/actors/src/weights.rs")),
      "test_sha256": digest(include_bytes!("actor_weight_registry.rs")),
      "runtime_config_sha256": digest(include_bytes!("../configs/mod.rs")),
      "package_version": env!("CARGO_PKG_VERSION"),
      "runtime_benchmarks": cfg!(feature = "runtime-benchmarks"),
      "try_runtime": cfg!(feature = "try-runtime"),
      "host_arch": std::env::consts::ARCH,
      "host_os": std::env::consts::OS,
      "db_ref_time_per_read": db.read, "db_ref_time_per_write": db.write,
      "generated_owners": rows.len(),
      "parameterized_owners": declared.values().filter(|d| !d.is_empty()).count(),
      "coordinates": "documented generated-component endpoint corners",
      "co_reachable_production_domain": false, "physical_measurement": false,
      "production_acceptance": false
    })
  );
  for row in rows {
    println!("ACTOR_WEIGHT_REGISTRY_ROW {row}");
  }
}
