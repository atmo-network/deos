use super::*;

fn trace() -> CpuTrace {
  CpuTrace::new(tempfile::tempfile().unwrap())
}
fn event(message: &str, artifact: &str, worker: &str) -> BTreeMap<String, String> {
  BTreeMap::from([
    ("message".into(), message.into()),
    ("worker".into(), worker.into()),
    ("validation_code_hash".into(), artifact.into()),
    ("artifact_id".into(), artifact.into()),
    ("duration".into(), "Some(12.345ms)".into()),
    ("worker_rip".into(), "false".into()),
  ])
}
fn observe(trace: &CpuTrace, message: &str, artifact: &str, worker: &str) -> Result<()> {
  trace
    .state()?
    .observe(event(message, artifact, worker), Some("queue.rs"), Some(1))
}

fn emit(message: &str, artifact: &str, worker: &str) {
  static SITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&META);
  static META: tracing::Metadata<'static> = tracing::Metadata::new(
    "synthetic SDK event",
    TARGET,
    tracing::Level::TRACE,
    Some("queue.rs"),
    Some(572),
    Some(MODULE),
    tracing::field::FieldSet::new(
      &[
        "message",
        "validation_code_hash",
        "artifact_id",
        "worker",
        "duration",
        "worker_rip",
      ],
      tracing_core::identify_callsite!(&SITE),
    ),
    tracing::metadata::Kind::EVENT,
  );
  tracing::dispatcher::get_default(|dispatch| {
    dispatch.event(&Event::new(
      &META,
      &META.fields().value_set_all(&[
        Some(&message),
        Some(&tracing::field::display(artifact)),
        Some(&tracing::field::display(artifact)),
        Some(&tracing::field::display(worker)),
        Some(&tracing::field::debug(Some(
          std::time::Duration::from_micros(12_345),
        ))),
        Some(&false),
      ]),
    ))
  });
}

#[test]
fn subscriber_preserves_raw_fields_and_accepted_cpu() {
  use std::io::{Read, Seek, SeekFrom};
  let output = tempfile::tempfile().unwrap();
  let mut reader = output.try_clone().unwrap();
  let trace = CpuTrace::new(output);
  let subscriber = tracing_subscriber::registry().with(trace.clone());
  tracing::subscriber::with_default(subscriber, || {
    trace
      .begin("positive", "artifact".into(), "input", false)
      .unwrap();
    emit(ASSIGNED, "artifact", "Worker(1v1)");
    emit(SUCCEEDED, "artifact", "Worker(1v1)");
    trace.finish().unwrap();
  });
  reader.seek(SeekFrom::Start(0)).unwrap();
  let mut bytes = String::new();
  reader.read_to_string(&mut bytes).unwrap();
  let rows = bytes
    .lines()
    .map(|s| serde_json::from_str::<Value>(s).unwrap())
    .collect::<Vec<_>>();
  assert_eq!(rows.len(), 4);
  assert_eq!(rows[2]["fields"]["duration"], "Some(12.345ms)");
  assert_eq!(rows[3]["child_cpu_us"], 12_345);
}

#[test]
fn subscriber_rejects_wrong_origin_and_retains_callback_failure() {
  let trace = trace();
  let subscriber = tracing_subscriber::registry().with(trace.clone());
  tracing::subscriber::with_default(subscriber, || {
    trace
      .begin("positive", "artifact".into(), "input", false)
      .unwrap();
    tracing::trace!(target: TARGET, duration = ?Some(std::time::Duration::from_micros(1)), "execute worker concluded successfully");
    assert!(trace.finish().is_err());
    trace
      .begin("positive", "artifact".into(), "input", false)
      .unwrap();
    emit(ASSIGNED, "artifact", "Worker(1v1)");
    emit(SUCCEEDED, "wrong artifact", "Worker(1v1)");
    assert!(trace.finish().is_err());
    assert!(trace.record(json!({"kind":"complete"})).is_err());
  });
}

#[test]
fn existing_output_and_write_failures_refuse() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("trace.jsonl");
  std::fs::write(&path, "protected").unwrap();
  assert!(CpuTrace::install(&path).is_err());
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "protected");
  let trace = CpuTrace::new(File::open(&path).unwrap());
  assert!(
    trace
      .begin("positive", "artifact".into(), "input", false)
      .is_err()
  );
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "protected");
}

#[test]
fn exact_integral_microseconds_only() {
  for duration in [
    std::time::Duration::from_micros(1),
    std::time::Duration::from_micros(12_345),
    std::time::Duration::from_micros(1_234_567),
  ] {
    assert_eq!(
      cpu_micros(&format!("{:?}", Some(duration))).unwrap(),
      duration.as_micros() as u64
    );
  }
  for bad in [
    "None",
    "Some(0ns)",
    "Some(1ns)",
    "Some(1.1µs)",
    "Some(-1s)",
    "Some(NaNs)",
    "Some(1.s)",
    "Some(1e3ms)",
    "Some(18446744073709551615s)",
    "Some(1.0000000000s)",
  ] {
    assert!(cpu_micros(bad).is_err(), "{bad}");
  }
}

#[test]
fn one_assignment_and_success_required_for_positive() {
  let trace = trace();
  trace
    .begin("positive", "artifact".into(), "input", false)
    .unwrap();
  observe(&trace, ASSIGNED, "artifact", "Worker(1v1)").unwrap();
  observe(&trace, SUCCEEDED, "artifact", "Worker(1v1)").unwrap();
  trace.finish().unwrap();
  assert!(trace.finish().is_err());
}

#[test]
fn missing_and_overlapping_cases_refuse() {
  let trace = trace();
  assert!(observe(&trace, SUCCEEDED, "artifact", "worker").is_err());
  trace
    .begin("positive", "artifact".into(), "input", false)
    .unwrap();
  assert!(
    trace
      .begin("overlap", "artifact".into(), "input", false)
      .is_err()
  );
  assert!(trace.finish().is_err());
  trace
    .begin("positive", "artifact".into(), "input", false)
    .unwrap();
  observe(&trace, ASSIGNED, "artifact", "worker").unwrap();
  assert!(trace.finish().is_err());
}

#[test]
fn wrong_identity_order_and_duplicates_refuse() {
  let trace = trace();
  trace
    .begin("positive", "artifact".into(), "input", false)
    .unwrap();
  assert!(observe(&trace, SUCCEEDED, "artifact", "worker").is_err());
  assert!(observe(&trace, ASSIGNED, "other", "worker").is_err());
  observe(&trace, ASSIGNED, "artifact", "worker").unwrap();
  assert!(observe(&trace, ASSIGNED, "artifact", "worker").is_err());
  assert!(observe(&trace, SUCCEEDED, "other", "worker").is_err());
  assert!(observe(&trace, SUCCEEDED, "artifact", "other").is_err());
  observe(&trace, SUCCEEDED, "artifact", "worker").unwrap();
  assert!(observe(&trace, SUCCEEDED, "artifact", "worker").is_err());
}

#[test]
fn negatives_have_no_cpu_sample() {
  let trace = trace();
  trace
    .begin("negative", "artifact".into(), "input", true)
    .unwrap();
  observe(&trace, ASSIGNED, "artifact", "worker").unwrap();
  trace.finish().unwrap();
  trace
    .begin("negative", "artifact".into(), "input", true)
    .unwrap();
  observe(&trace, ASSIGNED, "artifact", "worker").unwrap();
  observe(&trace, SUCCEEDED, "artifact", "worker").unwrap();
  assert!(trace.finish().is_err());
}
