use super::*;
use polkadot_sdk::polkadot_parachain_primitives::primitives::HeadData;

fn expected() -> ValidationResult {
  ValidationResult {
    head_data: HeadData(vec![1]),
    new_validation_code: None,
    upward_messages: Default::default(),
    horizontal_messages: Default::default(),
    processed_downward_messages: 0,
    hrmp_watermark: 2,
  }
}

#[test]
fn optional_trace_arguments_are_unambiguous() {
  let parse = |suffix: &[&str]| {
    let args = ["wasm", "fixtures", "workers", "1"]
      .into_iter()
      .chain(suffix.iter().copied())
      .map(String::from)
      .collect::<Vec<_>>();
    options(&args).map(|(insecure, path)| (insecure, path.map(String::from)))
  };
  assert_eq!(parse(&[]).unwrap(), (false, None));
  assert_eq!(
    parse(&["--cpu-trace", "output", "--insecure-local"]).unwrap(),
    (true, Some("output".into()))
  );
  for suffix in [
    vec!["--cpu-trace"],
    vec!["--cpu-trace", "--insecure-local"],
    vec!["--cpu-trace", "a", "--cpu-trace", "b"],
    vec!["--insecure-local", "--insecure-local"],
  ] {
    assert!(parse(&suffix).is_err());
  }
}

#[test]
fn only_exact_result_and_typed_worker_invalid_are_accepted() {
  let expected = expected();
  assert!(check_result(Ok(expected.clone()), &expected, false).is_ok());
  let mut changed = expected.clone();
  changed.hrmp_watermark += 1;
  assert!(check_result(Ok(changed), &expected, false).is_err());
  assert!(check_result(Ok(expected.clone()), &expected, true).is_err());
  assert!(
    check_result(
      Err(ValidationError::Invalid(InvalidCandidate::HardTimeout)),
      &expected,
      true
    )
    .is_err()
  );
  assert!(check_result(Err(ValidationError::ExecutionDeadline), &expected, true).is_err());
  assert!(
    check_result(
      Err(ValidationError::Invalid(
        InvalidCandidate::WorkerReportedInvalid("seal".into())
      )),
      &expected,
      true
    )
    .is_ok()
  );
}
