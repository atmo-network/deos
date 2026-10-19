//! Test-only observation of the pinned SDK's existing `state` events, not database I/O.

use crate::{AllPalletsWithSystem, System};
use polkadot_sdk::{
  frame_support::traits::StorageInfoTrait,
  sp_tracing::{
    tracing::{
      self, Event, Subscriber,
      field::{Field, Visit},
    },
    tracing_subscriber::{Layer, layer::Context, prelude::*},
  },
};
use serde_json::{Value, json};
use std::{
  cell::{Cell, RefCell},
  collections::{BTreeMap, BTreeSet},
  sync::{Arc, Mutex},
};

type State = BTreeMap<Vec<u8>, Vec<u8>>;

thread_local! {
  static ENABLED: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn enabled() -> bool {
  ENABLED.get()
}

// Serialize diagnostic subscriber installation; concurrent observation is not this collector's domain.
static OBSERVER_SCOPE: Mutex<()> = Mutex::new(());

pub(super) fn recorded<R>(run: impl FnOnce() -> R) -> R {
  assert!(!enabled(), "storage observer scopes cannot nest");
  let _scope = OBSERVER_SCOPE
    .lock()
    .expect("previous storage observer panicked");
  struct Reset;
  impl Drop for Reset {
    fn drop(&mut self) {
      ENABLED.set(false);
    }
  }
  ENABLED.set(true);
  let _reset = Reset;
  run()
}

fn snapshot() -> State {
  // Validation only. No trace subscriber is installed during either snapshot.
  let mut state = State::new();
  let mut cursor = Vec::new();
  while let Some(key) = polkadot_sdk::sp_io::storage::next_key(&cursor) {
    assert!(
      state.len() < 4_096,
      "H1 snapshot exceeded its diagnostic key domain"
    );
    assert!(!polkadot_sdk::sp_core::storage::well_known_keys::is_child_storage_key(&key));
    state.insert(
      key.clone(),
      polkadot_sdk::sp_io::storage::get(&key).unwrap().to_vec(),
    );
    cursor = key;
  }
  state
}

fn hex(value: &str) -> Vec<u8> {
  assert_eq!(value.len() % 2, 0, "truncated/nonhex SDK field");
  value
    .as_bytes()
    .chunks_exact(2)
    .map(|pair| {
      let digit = |c: u8| {
        (c as char)
          .to_digit(16)
          .expect("truncated/nonhex SDK field") as u8
      };
      (digit(pair[0]) << 4) | digit(pair[1])
    })
    .collect()
}

fn value_bytes(value: &str) -> Option<usize> {
  if value == "None" {
    return None;
  }
  let inner = value
    .strip_prefix("Some(")
    .and_then(|v| v.strip_suffix(')'))
    .expect("SDK Option<HexDisplay> debug schema changed");
  Some(hex(inner).len())
}

#[derive(Default)]
struct Fields(BTreeMap<String, String>);

impl Visit for Fields {
  fn record_str(&mut self, field: &Field, value: &str) {
    self.0.insert(field.name().into(), value.into());
  }
  fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
    // Get/Put debug values are untruncated; their *_encoded Display fields are not.
    if matches!(
      field.name(),
      "method" | "key" | "prefix" | "result" | "value" | "ext_id"
    ) {
      self.0.insert(field.name().into(), format!("{value:?}"));
    }
  }
}

#[derive(Clone)]
struct Operation {
  externality: String,
  phase: &'static str,
  key: Vec<u8>,
  method: String,
  bytes: usize,
  missing: bool,
}

#[derive(Clone)]
struct StateEvents {
  phase: &'static str,
  operations: Arc<Mutex<Vec<Operation>>>,
}

impl<S: Subscriber> Layer<S> for StateEvents {
  fn enabled(&self, metadata: &tracing::Metadata<'_>, _: Context<'_, S>) -> bool {
    metadata.target() == "state"
  }
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    if event.metadata().target() != "state" {
      return;
    }
    let mut fields = Fields::default();
    event.record(&mut fields);
    let get = |name: &str| {
      fields
        .0
        .get(name)
        .unwrap_or_else(|| panic!("SDK state event lacks {name}"))
    };
    let method = get("method").clone();
    let (key, bytes, missing) = match method.as_str() {
      "Get" => {
        let bytes = value_bytes(get("result"));
        (hex(get("key")), bytes.unwrap_or(0), bytes.is_none())
      }
      "Put" => {
        let bytes = value_bytes(get("value"));
        (hex(get("key")), bytes.unwrap_or(0), bytes.is_none())
      }
      "Exists" => (hex(get("key")), 0, get("result") == "false"),
      "Hash" => (hex(get("key")), 0, get("result") == "None"),
      "Append" => (hex(get("key")), hex(get("value")).len(), false),
      "ClearPrefix" => (hex(get("prefix")), 0, false),
      "StorageRoot" => (Vec::new(), 0, false),
      _ => panic!("unsupported SDK state event: {method}"),
    };
    let mut operations = self.operations.lock().unwrap();
    assert!(
      operations.len() < 100_000,
      "H1 trace exceeded its operation domain"
    );
    operations.push(Operation {
      externality: get("ext_id").clone(),
      phase: self.phase,
      key,
      method,
      bytes,
      missing,
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
fn capture_operations<R>(phase: &'static str, run: impl FnOnce() -> R) -> (R, Vec<Operation>) {
  assert!(enabled());
  let operations = Arc::default();
  let subscriber = polkadot_sdk::sp_tracing::tracing_subscriber::registry().with(StateEvents {
    phase,
    operations: Arc::clone(&operations),
  });
  let result = tracing::subscriber::with_default(subscriber, run);
  let operations = operations.lock().unwrap();
  assert!(
    operations
      .iter()
      .map(|op| &op.externality)
      .collect::<BTreeSet<_>>()
      .len()
      <= 1
  );
  (result, operations.clone())
}

/// Selected-key logical events only; the caller independently verifies complete state and Weight.
/// No snapshot-domain increase or claim of CPU/backend I/O observation.
#[cfg(feature = "runtime-benchmarks")]
pub(super) fn key_operations<R>(
  phase: &'static str,
  key: &[u8],
  run: impl FnOnce() -> R,
) -> (R, Vec<(String, usize, bool)>) {
  let (result, operations) = capture_operations(phase, run);
  let selected = operations
    .into_iter()
    .filter(|op| op.key == key)
    .map(|op| (op.method, op.bytes, op.missing))
    .collect();
  (result, selected)
}

#[cfg(feature = "runtime-benchmarks")]
pub(super) struct PointKeys {
  pub all: BTreeSet<Vec<u8>>,
  pub written: BTreeSet<Vec<u8>>,
}

/// Actual logical point keys, not a minimal read proof. Prefix/root work fails closed.
#[cfg(feature = "runtime-benchmarks")]
pub(super) fn point_access_keys<R>(phase: &'static str, run: impl FnOnce() -> R) -> (R, PointKeys) {
  let (result, operations) = capture_operations(phase, run);
  assert!(
    operations.iter().all(|op| matches!(
      op.method.as_str(),
      "Get" | "Put" | "Exists" | "Hash" | "Append"
    )),
    "point-key capture cannot qualify prefix/root operations"
  );
  let written = operations
    .iter()
    .filter(|op| matches!(op.method.as_str(), "Put" | "Append"))
    .map(|op| op.key.clone())
    .collect();
  (
    result,
    PointKeys {
      all: operations.into_iter().map(|op| op.key).collect(),
      written,
    },
  )
}

#[derive(Default)]
struct Counts {
  methods: BTreeMap<String, u64>,
  keys: BTreeSet<Vec<u8>>,
  gets: BTreeMap<Vec<u8>, u64>,
  get_bytes: usize,
  key_bytes: usize,
  submitted_bytes: usize,
  absent_reads: u64,
  changed_keys: usize,
  final_value_bytes: usize,
  removed_value_bytes: usize,
}

impl Counts {
  fn observe(&mut self, op: &Operation) {
    let method = if op.method == "Put" && op.missing {
      "Delete"
    } else {
      &op.method
    };
    *self.methods.entry(method.into()).or_default() += 1;
    if op.method != "StorageRoot" {
      self.keys.insert(op.key.clone());
    }
    self.key_bytes += op.key.len();
    if op.method == "Get" {
      *self.gets.entry(op.key.clone()).or_default() += 1;
      self.get_bytes += op.bytes;
    }
    if matches!(op.method.as_str(), "Put" | "Append") {
      self.submitted_bytes += op.bytes;
    }
    if op.missing && matches!(op.method.as_str(), "Get" | "Exists" | "Hash") {
      self.absent_reads += 1;
    }
  }
  fn json(&self, include_net: bool) -> Value {
    let mut result = json!({
      "calls": self.methods, "keys_or_prefixes": self.keys.len(),
      "get_keys": self.gets.len(), "max_gets_per_key": self.gets.values().copied().max().unwrap_or(0),
      "get_value_bytes": self.get_bytes, "input_key_bytes": self.key_bytes,
      "submitted_value_bytes": self.submitted_bytes,
      "absent_reads": self.absent_reads,
    });
    if include_net {
      result["net_changed_keys"] = json!(self.changed_keys);
      result["net_final_value_bytes"] = json!(self.final_value_bytes);
      result["net_removed_value_bytes"] = json!(self.removed_value_bytes);
    }
    result
  }
}

pub(super) struct Sample {
  before: Option<State>,
  phase_names: RefCell<BTreeSet<&'static str>>,
  operations: Arc<Mutex<Vec<Operation>>>,
}

impl Sample {
  pub(super) fn new() -> Self {
    Self {
      before: enabled().then(snapshot),
      phase_names: RefCell::default(),
      operations: Arc::default(),
    }
  }
  pub(super) fn phase<R>(&self, phase: &'static str, run: impl FnOnce() -> R) -> R {
    if self.before.is_none() {
      return run();
    }
    self.phase_names.borrow_mut().insert(phase);
    let subscriber = polkadot_sdk::sp_tracing::tracing_subscriber::registry().with(StateEvents {
      phase,
      operations: self.operations.clone(),
    });
    tracing::subscriber::with_default(subscriber, run)
  }
  pub(super) fn finish(self) -> Option<Value> {
    let before = self.before?;
    let after = snapshot();
    let mut catalog = AllPalletsWithSystem::storage_info();
    // System is included by PalletInfo; guard its familiar key before using the namespace map.
    assert!(catalog.iter().any(|item| item.pallet_name == b"System"));
    catalog.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let owner = |key: &[u8]| {
      catalog
        .iter()
        .find(|entry| key.starts_with(&entry.prefix))
        .map(|entry| {
          format!(
            "{}.{}",
            String::from_utf8(entry.pallet_name.clone()).unwrap(),
            String::from_utf8(entry.storage_name.clone()).unwrap()
          )
        })
        .unwrap_or_else(|| {
          format!(
            "raw:{}",
            polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&key)
          )
        })
    };
    let mut owners = BTreeMap::<String, Counts>::new();
    let mut phases = self
      .phase_names
      .into_inner()
      .into_iter()
      .map(|phase| (phase, Counts::default()))
      .collect::<BTreeMap<_, _>>();
    let mut total = Counts::default();
    let externalities = self
      .operations
      .lock()
      .unwrap()
      .iter()
      .map(|op| op.externality.clone())
      .collect::<BTreeSet<_>>();
    assert!(
      externalities.len() <= 1,
      "H1 collector crossed externality identities"
    );
    for op in self.operations.lock().unwrap().iter() {
      owners.entry(owner(&op.key)).or_default().observe(op);
      phases.entry(op.phase).or_default().observe(op);
      total.observe(op);
    }
    for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
      if before.get(key) == after.get(key) {
        continue;
      }
      assert!(
        self.operations.lock().unwrap().iter().any(|op| {
          (matches!(op.method.as_str(), "Put" | "Append") && op.key == *key)
            || (op.method == "ClearPrefix" && key.starts_with(&op.key))
        }),
        "net mutation has no observed write owner"
      );
      let stats = owners.entry(owner(key)).or_default();
      stats.changed_keys += 1;
      total.changed_keys += 1;
      if let Some(value) = after.get(key) {
        stats.final_value_bytes += value.len();
        total.final_value_bytes += value.len();
      } else {
        stats.removed_value_bytes += before[key].len();
        total.removed_value_bytes += before[key].len();
      }
    }
    Some(json!({
      "collector": "sdk-state-events-r1", "observed_externalities": externalities.len(), "total": total.json(true),
      "owners": owners.iter().map(|(name, stats)| (name, stats.json(true))).collect::<BTreeMap<_, _>>(),
      "phases": phases.iter().map(|(name, stats)| (name, stats.json(false))).collect::<BTreeMap<_, _>>(),
      "unsupported": ["untraced-seeks", "transaction-boundaries", "child-state", "backend-io", "physical-proof", "cpu"],
    }))
  }
}

/// Observes the SDK's real recording window, excluding setup and verification.
#[cfg(feature = "runtime-benchmarks")]
pub(super) struct BenchmarkRecording {
  phase: &'static str,
  sample: Option<Sample>,
  guard: Option<tracing::subscriber::DefaultGuard>,
  result: Option<Value>,
}

#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkRecording {
  pub(super) fn new(phase: &'static str) -> Self {
    assert!(enabled());
    Self {
      phase,
      sample: None,
      guard: None,
      result: None,
    }
  }

  pub(super) fn finish(self) -> Value {
    assert!(self.guard.is_none() && self.sample.is_none());
    self
      .result
      .expect("SDK must complete exactly one recording window")
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl polkadot_sdk::frame_benchmarking::Recording for BenchmarkRecording {
  fn start(&mut self) {
    assert!(self.sample.is_none() && self.guard.is_none() && self.result.is_none());
    let sample = Sample::new();
    sample.phase_names.borrow_mut().insert(self.phase);
    let subscriber = polkadot_sdk::sp_tracing::tracing_subscriber::registry().with(StateEvents {
      phase: self.phase,
      operations: sample.operations.clone(),
    });
    self.guard = Some(tracing::subscriber::set_default(subscriber));
    self.sample = Some(sample);
  }

  fn stop(&mut self) {
    drop(self.guard.take().expect("SDK must start before stopping"));
    self.result = self
      .sample
      .take()
      .expect("SDK must start before stopping")
      .finish();
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn point_access_keys_reject_prefix_and_root_operations() {
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    recorded(|| {
      polkadot_sdk::sp_io::storage::set(b"range:item", &[1]);
      let range = std::panic::catch_unwind(|| {
        point_access_keys("range", || {
          let _ = polkadot_sdk::sp_io::storage::clear_prefix(b"range:", None);
        })
      });
      assert!(range.is_err());
      let root = std::panic::catch_unwind(|| {
        point_access_keys("root", || {
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_core::storage::StateVersion::V1)
        })
      });
      assert!(root.is_err());
    });
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn point_access_keys_include_other_point_keys_but_exclude_setup() {
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    recorded(|| {
      polkadot_sdk::sp_io::storage::set(b"setup", &[1]);
      let (_, keys) = point_access_keys("body", || {
        let _ = polkadot_sdk::sp_io::storage::get(b"read");
        polkadot_sdk::sp_io::storage::set(b"write", &[2]);
        polkadot_sdk::sp_io::storage::clear(b"missing");
      });
      assert_eq!(
        keys.all,
        BTreeSet::from([b"read".to_vec(), b"write".to_vec(), b"missing".to_vec()])
      );
      assert_eq!(
        keys.written,
        BTreeSet::from([b"write".to_vec(), b"missing".to_vec()])
      );
    });
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn selected_key_operations_exclude_setup_verification_and_other_keys() {
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    recorded(|| {
      polkadot_sdk::sp_io::storage::set(b"selected", &[1, 2]);
      let (result, operations) = key_operations("body", b"selected", || {
        assert_eq!(
          polkadot_sdk::sp_io::storage::get(b"selected")
            .unwrap()
            .len(),
          2
        );
        polkadot_sdk::sp_io::storage::set(b"other", &[9]);
        polkadot_sdk::sp_io::storage::set(b"selected", &[1, 2, 3]);
        polkadot_sdk::sp_io::storage::clear(b"selected");
        assert!(polkadot_sdk::sp_io::storage::get(b"selected").is_none());
        7
      });
      assert!(polkadot_sdk::sp_io::storage::get(b"selected").is_none());
      assert_eq!(result, 7);
      assert_eq!(
        operations,
        vec![
          ("Get".into(), 2, false),
          ("Put".into(), 3, false),
          ("Put".into(), 0, true),
          ("Get".into(), 0, true),
        ]
      );
    });
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn sdk_recording_excludes_setup_and_verification() {
  use polkadot_sdk::frame_benchmarking::Recording;
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    recorded(|| {
      let mut recording = BenchmarkRecording::new("body");
      polkadot_sdk::sp_io::storage::set(b"setup", &[1]);
      recording.start();
      polkadot_sdk::sp_io::storage::set(b"body", &[2]);
      recording.stop();
      polkadot_sdk::sp_io::storage::set(b"verification", &[3]);
      let result = recording.finish();
      assert_eq!(result["total"]["calls"]["Put"], 1);
      assert_eq!(result["total"]["net_changed_keys"], 1);
    })
  });
}

#[test]
pub(super) fn h1_storage_trace_scopes_validation_and_preserves_provisional_work() {
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    recorded(|| {
      // Both large Debug values and compact Append values must be counted without truncation.
      polkadot_sdk::sp_io::storage::set(b"value", &[7; 2_000]);
      polkadot_sdk::sp_io::storage::set(b"list", &[0]);
      let sample = Sample::new();
      let _ = polkadot_sdk::sp_io::storage::get(b"value");
      sample.phase("fixture", || {
        assert_eq!(
          polkadot_sdk::sp_io::storage::get(b"value").unwrap().len(),
          2_000
        );
        assert!(polkadot_sdk::sp_io::storage::get(b"missing").is_none());
        assert!(!polkadot_sdk::sp_io::storage::exists(b"absent"));
        polkadot_sdk::sp_io::storage::start_transaction();
        polkadot_sdk::sp_io::storage::set(b"rolled-back", &[3; 16]);
        polkadot_sdk::sp_io::storage::rollback_transaction();
        polkadot_sdk::sp_io::storage::set(b"value", &[7; 2_000]);
        polkadot_sdk::sp_io::storage::append(b"list", vec![9]);
        polkadot_sdk::sp_io::storage::clear(b"absent-delete");
      });
      let result = sample.finish().unwrap();
      let _ = System::block_number();
      assert_eq!(result["total"]["calls"]["Get"], 2);
      assert_eq!(result["total"]["calls"]["Exists"], 1);
      assert_eq!(result["total"]["calls"]["Put"], 2);
      assert_eq!(result["total"]["calls"]["Append"], 1);
      assert_eq!(result["total"]["calls"]["Delete"], 1);
      assert_eq!(result["total"]["get_value_bytes"], 2_000);
      assert_eq!(result["total"]["submitted_value_bytes"], 2_017);
      assert_eq!(result["total"]["absent_reads"], 2);
      assert_eq!(result["total"]["net_changed_keys"], 1);
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(b"list").unwrap().to_vec(),
        vec![4, 9]
      );
      assert!(polkadot_sdk::sp_io::storage::get(b"rolled-back").is_none());
      let empty = Sample::new();
      empty.phase("empty", || ());
      assert_eq!(empty.finish().unwrap()["total"]["calls"], json!({}));
    })
  });
}
