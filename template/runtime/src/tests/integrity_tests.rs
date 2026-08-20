//! Runtime Integrity Tests
//!
//! FRAME collects every pallet `#[pallet::integrity_test]` block into `AllPalletsWithSystem`.
//! Those assertions encode configuration invariants that hold only for one specific combination
//! of runtime constants and generated weights: derived bound equalities, non-zero page sizes, and
//! the requirement that one maximum automatic actor cleanup fits the guaranteed `on_idle` service
//! envelope.
//!
//! A parameter change or a weight regeneration can invalidate any of them, and a node only
//! discovers that at startup. Nothing else in this workspace executes them, so this module is the
//! gate that keeps those invariants inside ordinary local validation.

use super::common::new_test_ext;
use crate::AllPalletsWithSystem;
use polkadot_sdk::frame_support::traits::IntegrityTest;

/// Runs every pallet integrity assertion against the concrete DEOS runtime configuration.
#[test]
fn runtime_pallet_integrity_holds() {
  new_test_ext().execute_with(|| {
    AllPalletsWithSystem::integrity_test();
  });
}
