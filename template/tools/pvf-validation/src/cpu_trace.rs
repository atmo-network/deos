use crate::Result;
use serde_json::{Value, json};
use std::{
  collections::BTreeMap,
  fmt,
  fs::File,
  io::Write,
  path::Path,
  sync::{Arc, Mutex},
};
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

const TARGET: &str = "parachain::pvf";
const MODULE: &str = "polkadot_node_core_pvf::execute::queue";
const ASSIGNED: &str = "assigning the execute worker";
const SUCCEEDED: &str = "execute worker concluded successfully";

#[derive(Clone)]
pub struct CpuTrace(Arc<Mutex<State>>);

struct State {
  output: File,
  active: Option<Case>,
  fault: Option<String>,
}

struct Case {
  negative: bool,
  artifact: String,
  worker: Option<String>,
  duration: Option<String>,
}

#[derive(Default)]
struct Fields(BTreeMap<String, String>);

impl Visit for Fields {
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    self.0.insert(field.name().into(), format!("{value:?}"));
  }
  fn record_str(&mut self, field: &Field, value: &str) {
    self.0.insert(field.name().into(), value.into());
  }
}

// SDK Debug transport, not a stable timing API. Require exact integral microseconds.
fn cpu_micros(raw: &str) -> Result<u64> {
  let invalid = || format!("unsupported worker duration: {raw}");
  let body = raw
    .strip_prefix("Some(")
    .and_then(|s| s.strip_suffix(')'))
    .ok_or_else(invalid)?;
  let (number, nanos) = [
    ("ns", 1u64),
    ("µs", 1_000),
    ("ms", 1_000_000),
    ("s", 1_000_000_000),
  ]
  .into_iter()
  .find_map(|(unit, scale)| body.strip_suffix(unit).map(|v| (v, scale)))
  .ok_or_else(invalid)?;
  let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
  if whole.is_empty()
    || !whole.bytes().all(|b| b.is_ascii_digit())
    || !fraction.bytes().all(|b| b.is_ascii_digit())
    || fraction.len() > 9
    || (number.contains('.') && fraction.is_empty())
  {
    return Err(invalid());
  }
  let whole = whole.parse::<u64>().map_err(|_| invalid())?;
  let numerator = if fraction.is_empty() {
    0
  } else {
    fraction.parse::<u64>().map_err(|_| invalid())?
  };
  let denominator = 10u64.pow(fraction.len() as u32);
  let fractional = numerator.checked_mul(nanos).ok_or_else(invalid)?;
  if fractional % denominator != 0 {
    return Err(invalid());
  }
  let total = whole
    .checked_mul(nanos)
    .and_then(|v| v.checked_add(fractional / denominator))
    .ok_or_else(invalid)?;
  if total == 0 || total % 1_000 != 0 {
    return Err(invalid());
  }
  Ok(total / 1_000)
}

impl State {
  fn record(&mut self, value: Value) -> Result<()> {
    serde_json::to_writer(&mut self.output, &value).map_err(|e| format!("CPU trace: {e}"))?;
    self
      .output
      .write_all(b"\n")
      .and_then(|()| self.output.flush())
      .map_err(|e| format!("CPU trace: {e}"))
  }

  fn observe(
    &mut self,
    fields: BTreeMap<String, String>,
    file: Option<&str>,
    line: Option<u32>,
  ) -> Result<()> {
    let message = fields.get("message").map(String::as_str).unwrap_or("");
    if message != ASSIGNED && message != SUCCEEDED {
      return Ok(());
    }
    self.record(json!({"kind":"sdk_event", "module":MODULE, "target":TARGET, "file":file, "line":line, "fields":fields}))?;
    let case = self
      .active
      .as_mut()
      .ok_or("CPU event outside a submitted case")?;
    let worker = fields
      .get("worker")
      .filter(|s| !s.is_empty())
      .ok_or("missing worker identity")?;
    if message == ASSIGNED {
      if case.worker.is_some() || fields.get("validation_code_hash") != Some(&case.artifact) {
        return Err("duplicate assignment or artifact mismatch".into());
      }
      case.worker = Some(worker.clone());
    } else {
      if case.worker.as_ref() != Some(worker)
        || fields.get("artifact_id") != Some(&case.artifact)
        || case.duration.is_some()
        || !matches!(
          fields.get("worker_rip").map(String::as_str),
          Some("true" | "false")
        )
      {
        return Err("unassigned, duplicate or mismatched success event".into());
      }
      let duration = fields.get("duration").ok_or("missing worker duration")?;
      cpu_micros(duration)?;
      case.duration = Some(duration.clone());
    }
    Ok(())
  }
}

impl CpuTrace {
  fn new(output: File) -> Self {
    Self(Arc::new(Mutex::new(State {
      output,
      active: None,
      fault: None,
    })))
  }

  pub fn install(path: &Path) -> Result<Self> {
    if tracing::level_filters::STATIC_MAX_LEVEL < tracing::level_filters::LevelFilter::TRACE {
      return Err("SDK success trace is statically disabled".into());
    }
    let trace = Self::new(
      File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("new CPU trace {}: {e}", path.display()))?,
    );
    tracing_subscriber::registry()
      .with(trace.clone())
      .try_init()
      .map_err(|e| format!("CPU trace subscriber: {e}"))?;
    Ok(trace)
  }

  fn state(&self) -> Result<std::sync::MutexGuard<'_, State>> {
    let state = self.0.lock().map_err(|_| "CPU trace lock poisoned")?;
    if let Some(fault) = &state.fault {
      return Err(fault.clone());
    }
    Ok(state)
  }

  pub fn record(&self, value: Value) -> Result<()> {
    self.state()?.record(value)
  }

  pub fn begin(
    &self,
    label: &str,
    artifact: String,
    input_sha256: &str,
    negative: bool,
  ) -> Result<()> {
    let mut state = self.state()?;
    if state.active.is_some() {
      return Err("overlapping CPU trace cases".into());
    }
    state.record(json!({"kind":"begin", "case":label, "artifact":artifact, "input_sha256":input_sha256, "negative":negative}))?;
    state.active = Some(Case {
      negative,
      artifact,
      worker: None,
      duration: None,
    });
    Ok(())
  }

  // Call only after the unchanged complete-result/typed-negative acceptance check.
  pub fn finish(&self) -> Result<()> {
    let mut state = self.state()?;
    let case = state.active.take().ok_or("no active CPU trace case")?;
    let negative = case.negative;
    if case.worker.is_none()
      || (negative && case.duration.is_some())
      || (!negative && case.duration.is_none())
    {
      return Err("missing assignment/duration or success event for negative case".into());
    }
    let micros = case.duration.as_deref().map(cpu_micros).transpose()?;
    state.record(json!({"kind":"accepted", "negative":negative, "worker":case.worker, "child_cpu_us":micros, "raw_duration":case.duration}))
  }
}

fn relevant(metadata: &tracing::Metadata<'_>) -> bool {
  metadata.target() == TARGET
    && metadata.module_path() == Some(MODULE)
    && (metadata.fields().field("duration").is_some()
      || metadata.fields().field("validation_code_hash").is_some())
}

impl<S: Subscriber> Layer<S> for CpuTrace {
  fn enabled(&self, metadata: &tracing::Metadata<'_>, _: Context<'_, S>) -> bool {
    relevant(metadata)
  }

  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let metadata = event.metadata();
    if !relevant(metadata) {
      return;
    }
    let mut fields = Fields::default();
    event.record(&mut fields);
    // Poisoning is propagated by begin/finish; callback errors stay sticky until observed.
    if let Ok(mut state) = self.0.lock() {
      if state.fault.is_none() {
        if let Err(error) = state.observe(fields.0, metadata.file(), metadata.line()) {
          state.fault = Some(error);
        }
      }
    }
  }
}

#[cfg(test)]
#[path = "cpu_trace_tests.rs"]
mod tests;
