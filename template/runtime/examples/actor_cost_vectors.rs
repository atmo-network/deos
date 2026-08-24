use deos_runtime::{AccountId, Actors, Balance, Balances, Oracle, Runtime, RuntimeOrigin, System};
use pallet_deos_actors::{
  ActorContract, ActorCostQuote, ActorType, AmountResolution, AssetFilter, CompletionPolicy,
  ContractSteps, CrossingDirection, FundingSourcePolicy, Mutability, SourceFilter, Step,
  StepErrorPolicy, Task, Trigger, TriggerFamily,
};
use polkadot_sdk::{
  frame_support::{BoundedVec, traits::Currency},
  polkadot_runtime_common::BuildStorage,
  sp_io::TestExternalities,
  sp_io::hashing::sha2_256,
};
use primitives::{
  AssetKind, LocalPoolObservationMethod, OracleAggregationId, OracleFeedId, OracleProvenance,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path, process};

const FORMAT: &str = "deos.actor.cost-vectors";
const FORMAT_VERSION: u32 = 1;
const RUNTIME_API_VERSION: u32 = 1;
const FIXTURE_BALANCE: Balance = 1u128 << 100;

type RuntimeSteps = ContractSteps<Runtime>;
type RuntimeTrigger = pallet_deos_actors::TriggerOf<Runtime>;

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect() // deos-bypass: bounded-iter -- fixed SHA-256 and runtime identity digests
}

fn file_identity(path: &Path) -> String {
  let bytes = fs::read(path).unwrap_or_else(|error| {
    panic!(
      "Actor cost evidence file {} is unreadable: {error}",
      path.display()
    )
  });
  hex(&sha2_256(&bytes))
}

fn trigger_family_name(family: TriggerFamily) -> &'static str {
  match family {
    TriggerFamily::Manual => "Manual",
    TriggerFamily::AddressEvent => "AddressEvent",
    TriggerFamily::ObservationChange => "ObservationChange",
    TriggerFamily::ObservationCrossing => "ObservationCrossing",
    TriggerFamily::AtTime => "AtTime",
    TriggerFamily::Cadenced => "Cadenced",
  }
}

fn actor_type_name(actor_type: ActorType) -> &'static str {
  match actor_type {
    ActorType::User => "User",
    ActorType::System => "System",
  }
}

fn weight_value(weight: polkadot_sdk::sp_weights::Weight) -> Value {
  json!({
    "refTime": weight.ref_time().to_string(),
    "proofSize": weight.proof_size().to_string(),
  })
}

fn hold_breakdown_value(breakdown: pallet_deos_actors::ActorStateHoldBreakdown<Balance>) -> Value {
  json!({
    "identity": breakdown.identity.to_string(),
    "contractHead": breakdown.contract_head.to_string(),
    "contractBody": breakdown.contract_body.to_string(),
    "detector": breakdown.detector.to_string(),
    "funding": breakdown.funding.to_string(),
    "run": breakdown.run.to_string(),
  })
}

fn quote_value(quote: ActorCostQuote<Balance>) -> Value {
  let trigger = quote.prospective_trigger_fee.map(|trigger| {
    json!({
      "family": trigger_family_name(trigger.trigger_family),
      "maximumWeight": weight_value(trigger.maximum_weight),
      "fee": trigger.fee.to_string(),
      "productionWeightIdentity": hex(&trigger.production_weight_identity),
    })
  });
  let pipeline = quote.prospective_pipeline_fee.map(|pipeline| {
    json!({
      "machineFee": pipeline.pipeline_machine_fee.to_string(),
      "cleanupFee": pipeline.cleanup_fee.to_string(),
      "totalFee": pipeline.total_fee.to_string(),
      "strategy": "UpfrontBounded",
      "admissionIdentity": hex(&pipeline.admission_identity),
      "productionWeightIdentity": hex(&pipeline.production_weight_identity),
    })
  });
  json!({
    "actorType": actor_type_name(quote.actor_type),
    "creationFee": quote.creation_fee.to_string(),
    "prospectiveTriggerFee": trigger,
    "prospectivePipelineFee": pipeline,
    "maximumNextActionFee": {
      "maximumEffectWeight": weight_value(quote.maximum_next_action_fee.maximum_effect_weight),
      "maximumEffectFee": quote.maximum_next_action_fee.maximum_effect_fee.to_string(),
      "productionWeightIdentity": hex(&quote.maximum_next_action_fee.production_weight_identity),
    },
    "stateHold": {
      "exempt": quote.actor_state_hold.exempt,
      "basePerComponent": quote.actor_state_hold.base_per_component.to_string(),
      "perEncodedByte": quote.actor_state_hold.per_encoded_byte.to_string(),
      "components": hold_breakdown_value(quote.actor_state_hold.breakdown),
      "total": quote.actor_state_hold.total.to_string(),
    },
  })
}

fn transfer_steps(step_count: usize, destination: &AccountId) -> RuntimeSteps {
  let step = Step {
    precondition: None,
    task: Task::Transfer {
      to: destination.clone(),
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  BoundedVec::try_from(vec![step; step_count]).expect("cost-vector Contract geometry fits")
}

fn active_contract(
  trigger: RuntimeTrigger,
  steps: RuntimeSteps,
) -> pallet_deos_actors::ActorContractOf<Runtime> {
  ActorContract {
    trigger,
    cooldown_blocks: 0,
    window: None,
    steps,
    completion: CompletionPolicy::Persistent,
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  }
}

fn prefund_user_sovereign(owner: &AccountId, steps: &RuntimeSteps) {
  let sovereign = Actors::sovereign_account_id(owner, 0);
  let required = Actors::user_pipeline_machine_capacity_requirement(steps)
    .expect("cost-vector Pipeline capacity computes");
  let _ = <Balances as Currency<AccountId>>::deposit_creating(&sovereign, required);
}

fn create_user_vector(
  name: &'static str,
  owner_byte: u8,
  trigger: RuntimeTrigger,
  steps: RuntimeSteps,
) -> Value {
  let owner = AccountId::new([owner_byte; 32]);
  let family = trigger.family();
  let step_count = steps.len();
  prefund_user_sovereign(&owner, &steps);
  let actor_id = Actors::next_actor_id();
  Actors::create_user_actor(
    RuntimeOrigin::signed(owner),
    Mutability::Mutable,
    Some(active_contract(trigger, steps)),
  )
  .expect("cost-vector User Actor creation succeeds");
  let quote = Actors::actor_cost_quote(actor_id).expect("cost-vector User quote computes");
  json!({
    "name": name,
    "actorId": actor_id.to_string(),
    "contractStepCount": step_count,
    "triggerFamily": trigger_family_name(family),
    "quote": quote_value(quote),
  })
}

fn create_system_vector(destination: &AccountId) -> Value {
  let owner = AccountId::new([240; 32]);
  let steps = transfer_steps(1, destination);
  let actor_id = Actors::next_actor_id();
  Actors::create_system_actor(
    RuntimeOrigin::root(),
    owner,
    Mutability::Mutable,
    Some(ActorContract {
      trigger: Trigger::Manual,
      cooldown_blocks: 0,
      window: None,
      steps,
      completion: CompletionPolicy::Persistent,
      funding: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    }),
  )
  .expect("cost-vector System Actor creation succeeds");
  let quote = Actors::actor_cost_quote(actor_id).expect("cost-vector System quote computes");
  json!({
    "name": "system-manual-1",
    "actorId": actor_id.to_string(),
    "contractStepCount": 1,
    "triggerFamily": "Manual",
    "quote": quote_value(quote),
  })
}

fn create_dormant_vector() -> Value {
  let owner = AccountId::new([241; 32]);
  let actor_id = Actors::next_actor_id();
  Actors::create_user_actor(RuntimeOrigin::signed(owner), Mutability::Mutable, None)
    .expect("cost-vector dormant User Actor creation succeeds");
  let quote = Actors::actor_cost_quote(actor_id).expect("cost-vector dormant quote computes");
  json!({
    "name": "user-dormant",
    "actorId": actor_id.to_string(),
    "contractStepCount": null,
    "triggerFamily": null,
    "quote": quote_value(quote),
  })
}

fn manifest() -> Value {
  let destination = AccountId::new([250; 32]);
  let owners = (1u8..=20)
    .map(|byte| (AccountId::new([byte; 32]), FIXTURE_BALANCE))
    .chain([
      (AccountId::new([240; 32]), FIXTURE_BALANCE),
      (AccountId::new([241; 32]), FIXTURE_BALANCE),
      (destination.clone(), FIXTURE_BALANCE),
    ])
    .collect::<Vec<_>>();
  let mut storage = polkadot_sdk::frame_system::GenesisConfig::<Runtime>::default()
    .build_storage()
    .expect("cost-vector System genesis builds");
  polkadot_sdk::pallet_balances::GenesisConfig::<Runtime> {
    balances: owners,
    ..Default::default()
  }
  .assimilate_storage(&mut storage)
  .expect("cost-vector balances genesis assimilates");
  pallet_deos_actors::GenesisConfig::<Runtime>::default()
    .assimilate_storage(&mut storage)
    .expect("cost-vector Actors genesis assimilates");

  let mut ext = TestExternalities::new(storage);
  let vectors = ext.execute_with(|| {
    System::set_block_number(1);
    let feed = OracleFeedId::directional_local_pool_price(
      AssetKind::Native,
      AssetKind::Local(1),
      LocalPoolObservationMethod::PreExecutionSpot,
      OracleAggregationId::Ema {
        half_life_blocks: 1,
      },
      12,
    );
    Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      destination.clone(),
      feed.meaning(),
      OracleProvenance::DeosRouterPreExecutionReserves,
      12,
      pallet_oracle::Aggregation::LastValue,
      pallet_oracle::ZeroPolicy::Reject,
      false,
    )
    .expect("cost-vector observation feed registers");
    Oracle::publish(RuntimeOrigin::signed(destination.clone()), feed, 100)
      .expect("cost-vector observation publishes");

    let mut vectors = vec![
      create_user_vector(
        "user-manual-0",
        1,
        Trigger::Manual,
        transfer_steps(0, &destination),
      ),
      create_user_vector(
        "user-manual-1",
        2,
        Trigger::Manual,
        transfer_steps(1, &destination),
      ),
      create_user_vector(
        "user-manual-4",
        3,
        Trigger::Manual,
        transfer_steps(4, &destination),
      ),
      create_user_vector(
        "user-manual-8",
        4,
        Trigger::Manual,
        transfer_steps(8, &destination),
      ),
      create_user_vector(
        "user-manual-32",
        5,
        Trigger::Manual,
        transfer_steps(32, &destination),
      ),
      create_user_vector(
        "user-address-event-1",
        6,
        Trigger::AddressEvent {
          source_filter: SourceFilter::Any,
          asset_filter: AssetFilter::Any,
        },
        transfer_steps(1, &destination),
      ),
      create_user_vector(
        "user-observation-change-1",
        7,
        Trigger::ObservationChange { feed },
        transfer_steps(1, &destination),
      ),
      create_user_vector(
        "user-observation-crossing-1",
        8,
        Trigger::ObservationCrossing {
          feed,
          direction: CrossingDirection::Rising,
          threshold: 200,
          rearm_threshold: 100,
        },
        transfer_steps(1, &destination),
      ),
      create_user_vector(
        "user-at-time-1",
        9,
        Trigger::AtTime { after_ticks: 10 },
        transfer_steps(1, &destination),
      ),
      create_user_vector(
        "user-cadenced-1",
        10,
        Trigger::Cadenced { every_ticks: 10 },
        transfer_steps(1, &destination),
      ),
    ];
    vectors.push(create_system_vector(&destination));
    vectors.push(create_dormant_vector());
    vectors
  });

  json!({
    "format": FORMAT,
    "formatVersion": FORMAT_VERSION,
    "runtimeApiVersion": RUNTIME_API_VERSION,
    "metadataSha256": file_identity(Path::new("../web-client/.papi/metadata/deos.scale")),
    "weightSha256": file_identity(Path::new("runtime/src/weights/pallet_deos_actors.rs")),
    "vectors": vectors,
  })
}

fn usage() {
  eprintln!(
    "Usage: cargo run -p deos-runtime --example actor_cost_vectors -- [PATH | --check PATH]"
  );
}

fn main() {
  let rendered = serde_json::to_string(&manifest()).expect("Actor cost vectors serialize") + "\n";
  let args = env::args().skip(1).collect::<Vec<_>>();
  match args.as_slice() {
    [] => print!("{rendered}"),
    [path] => fs::write(Path::new(path), rendered).expect("Actor cost vector artifact is writable"),
    [flag, path] if flag == "--check" => {
      let actual =
        fs::read_to_string(Path::new(path)).expect("Actor cost vector artifact is readable");
      assert_eq!(actual, rendered, "Actor cost vector artifact is stale");
    }
    _ => {
      usage();
      process::exit(1);
    }
  }
}
