//! Immutable historical transition-oracle seal for the retired control-frame candidate.
//!
//! The candidate executor and its synthetic Ready/Waiting/control-cell projections were removed
//! after canonical Service/Deadline ownership superseded them. The sealed rows remain historical
//! experiment evidence; current behavior is owned by the canonical pallet tests.

const ACTOR_CONTROL_TRANSITION_ORACLE: &str =
  include_str!("fixtures/actor_control_transition_oracle_v1.tsv");

#[test]
fn control_baseline_transition_oracle_fixture_is_complete_and_unique() {
  let expected = [
    "address_event_collection_failure",
    "address_event_success",
    "at_time_collection_failure",
    "at_time_success",
    "cadenced_success",
    "direct_funding_unavailable",
    "direct_stop_cycle",
    "direct_temporary_failure",
    "direct_transfer_p0_match_true",
    "direct_transfer_p2_match_false",
    "direct_transfer_p2_match_true",
    "direct_transfer_p4_match_true",
    "funding_retry_fails_false",
    "funding_retry_fails_true",
    "manual_collection_failure",
    "manual_success",
    "middle_running_step",
    "observation_change_collection_failure",
    "observation_change_success",
    "observation_crossing_collection_failure",
    "observation_crossing_success",
    "temporary_retry_fails_false",
    "temporary_retry_fails_true",
    "terminal_running_step",
    "zero_step_user_opening",
  ];
  let entries = ACTOR_CONTROL_TRANSITION_ORACLE
    .lines()
    .filter(|line| !line.starts_with('#'))
    .map(|line| line.split_once('|').expect("oracle row has one delimiter"))
    .collect::<Vec<_>>();
  assert_eq!(entries.len(), expected.len());
  assert_eq!(
    entries.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
    expected
  );
  for (_, digest) in entries {
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
  }
}
