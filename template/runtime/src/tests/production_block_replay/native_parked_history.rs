//! Model-native prepared suffixes; no authenticated Governance prefix or artifact qualification.
use super::*;

fn assert_prepared_suffix(end: SignedParkedReplayEnd, start: u32) {
  for output in ["DEOS_PARKED_STATE_OUTPUT", "DEOS_PROOF_PREFIX_OUTPUT"] {
    if std::env::var_os(output).is_some() {
      super::super::common::ensure_artifact_profile()
        .expect("model-only prepared history cannot export retained state or proofs");
    }
  }
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert!(!Actors::user_creation_enabled());
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      if start > 0 {
        System::set_block_number(start);
        prepare_canonical_system_hash_window(start);
      }
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], start);
    ext.execute_with(|| replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, end));
    println!(
      "PARKED_NATIVE_HISTORY_V1 {}",
      serde_json::json!({
        "preset": preset, "initialBlock": start, "nativeOnly": true,
        "identityCapacityModel": true, "optionalAuthoritySupplements": false,
        "productionCapacityQualified": false, "productionArtifactQualified": false,
        "governancePrefixQualified": false
      })
    );
  }
}

/// Root preparation isolates the suffix from the long Governance prefix.
#[test]
fn root_prepared_one_watch_signed_churn_suffix_reconciles_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::Complete, 0);
}

/// The contiguous suffix crosses the real scheduled hook, not public history to its parent.
#[test]
fn clock_prepared_parked_signed_setup_preserves_service_through_session_rotation() {
  assert_prepared_suffix(
    SignedParkedReplayEnd::Rotation,
    crate::configs::Period::get() - 32,
  );
}

/// The exact setup calls run before churn, with four empty blocks after each pool batch.
#[test]
fn root_prepared_parked_signed_setup_keeps_fee_multiplier_bounded_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::Setup, 0);
}

/// A finite one-watch gap establishes the full retained-page scan base.
#[test]
fn root_prepared_parked_signed_bootstrap_establishes_full_page_scans_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::Bootstrap, 0);
}

/// Ordinary signed creation appends a permanent page while scans protect retained holes.
#[test]
fn root_prepared_parked_signed_birth_preserves_active_scans_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::Birth, 0);
}

/// A maximum-width AddressEvent/window Contract stays parked through public signed birth.
#[test]
fn root_prepared_parked_signed_wide_birth_preserves_idle_authority_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::WideBirth, 0);
}

/// The repeated construction retires a temporary identity and creates a permanent one together.
#[test]
fn root_prepared_parked_signed_birth_pair_preserves_active_scans_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::BirthPair, 0);
}

/// Every nonempty subset of the current watch domain preserves the retained page.
#[test]
fn root_prepared_parked_signed_watch_masks_preserve_custody_and_sources_both_presets() {
  assert_prepared_suffix(SignedParkedReplayEnd::WatchMasks, 0);
}
