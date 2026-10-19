//! Test-only entry evidence, not a production provider or a historical non-saturation proof.

use super::*;
use pallet_deos_actors::ActorPrepassContext;
use polkadot_sdk::frame_support::dispatch::DispatchClass;
use polkadot_sdk::frame_system::Phase;

#[derive(Clone, Debug)]
struct EntryObservation {
  block: u32,
  phase: Option<Phase>,
  index: Option<u32>,
  context_ready: bool,
  frozen: bool,
  inherents_finished: bool,
  staged_prepass: bool,
  mandatory: Weight,
  normal: Weight,
  operational: Weight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
  Context,
  Frozen,
  Stage,
  Call,
  Class,
  Saturation,
  Declaration,
  Ceiling,
}

fn normalize_entry(
  entry: &EntryObservation,
  block: u32,
  declaration: Weight,
  ceiling: Weight,
) -> Result<Weight, Refusal> {
  if entry.block != block || !entry.context_ready {
    return Err(Refusal::Context);
  }
  if entry.frozen {
    return Err(Refusal::Frozen);
  }
  // This is the concrete three-provider reference topology, not a generic pallet rule.
  if entry.phase != Some(Phase::ApplyExtrinsic(2))
    || entry.index != Some(2)
    || entry.inherents_finished
  {
    return Err(Refusal::Stage);
  }
  if !entry.staged_prepass {
    return Err(Refusal::Call);
  }
  if entry.normal != Weight::zero() || entry.operational != Weight::zero() {
    return Err(Refusal::Class);
  }
  if entry.mandatory.ref_time() == u64::MAX || entry.mandatory.proof_size() == u64::MAX {
    return Err(Refusal::Saturation);
  }
  let prefix = entry
    .mandatory
    .checked_sub(&declaration)
    .ok_or(Refusal::Declaration)?;
  if prefix.any_gt(ceiling) {
    return Err(Refusal::Ceiling);
  }
  Ok(prefix)
}

fn observe_entry(staged_key: &[u8], expected_storage_value: &[u8]) -> EntryObservation {
  // Fixed-size output avoids decoding an arbitrary staged call into runtime memory.
  // It does not bound host work, recorder proof or price.
  // The metadata-derived key and fixed Prepass value are prepared outside this observer.
  let mut bytes = [0u8; 16];
  assert!(expected_storage_value.len() <= bytes.len());
  let stored_len = polkadot_sdk::sp_io::storage::read(staged_key, &mut bytes, 0);
  let staged_prepass = stored_len == Some(u32::try_from(expected_storage_value.len()).unwrap())
    && bytes[..expected_storage_value.len()] == *expected_storage_value;
  let weight = System::block_weight();
  let block = System::block_number();
  EntryObservation {
    block,
    phase: System::execution_phase(),
    index: System::extrinsic_index(),
    context_ready: <<Runtime as pallet_deos_actors::Config>::PrepassContext as ActorPrepassContext>::context_ready(),
    frozen: Actors::block_resource_state().is_some_and(|state| state.ensure_block(block).is_ok()),
    inherents_finished: System::inherents_applied(),
    staged_prepass,
    mandatory: *weight.get(DispatchClass::Mandatory),
    normal: *weight.get(DispatchClass::Normal),
    operational: *weight.get(DispatchClass::Operational),
  }
}

#[test]
#[allow(deprecated)] // Probe the same bare preparation used by the bound Executive.
fn canonical_prepass_entry_prototype_rejects_unowned_normalization() {
  use crate::configs::unsigned::RuntimeUnsignedValidator;
  use polkadot_sdk::sp_runtime::traits::{TransactionExtension, ValidateUnsigned};
  let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
  let parent = parent_header_for(fixture.storage.clone(), &[], 1);
  let header = Header::new(
    2,
    Default::default(),
    Default::default(),
    parent.hash(),
    Digest {
      logs: vec![DigestItem::PreRuntime(
        polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
        2u64.encode(),
      )],
    },
  );
  let mut ext = TestExternalities::new(fixture.storage);
  ext.execute_with(|| {
    Executive::initialize_block(&header);
    let extrinsics = inherent_data_for(&parent, 2).create_extrinsics();
    assert_eq!(extrinsics.len(), 3);
    let mut witnessed = false;
    for extrinsic in extrinsics {
      if !matches!(
        extrinsic.function,
        RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
      ) {
        assert_ok!(Executive::apply_extrinsic(extrinsic));
        continue;
      }
      witnessed = true;
      let info = extrinsic.get_dispatch_info();
      assert_eq!(info.class, DispatchClass::Mandatory);
      assert_eq!(info.extension_weight, Weight::zero());
      let maximum = BlockResourceBudgetValue::get().maximum_block();
      assert_eq!(info.call_weight, maximum);
      // A diagnostic threshold, NOT a qualified pre-execution fixed ceiling.
      let ceiling = Weight::from_parts(maximum.ref_time() / 4, maximum.proof_size() / 4);
      let encoded = extrinsic.encode();
      let len = encoded.len();
      let stored = encoded.encode(); // System stores a SCALE Vec containing the full encoded xt.
      let mut staged_key = System::storage_info()
        .into_iter()
        .find(|item| item.storage_name == b"ExtrinsicData")
        .unwrap()
        .prefix;
      let index = 2u32.encode();
      staged_key.extend(polkadot_sdk::sp_io::hashing::twox_64(&index));
      staged_key.extend(index);
      assert_eq!(System::extrinsic_data(2), Vec::<u8>::new());
      let before = System::block_weight().total();
      let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let not_staged = observe_entry(&staged_key, &stored);
      assert_eq!(
        normalize_entry(&not_staged, 2, info.total_weight(), ceiling),
        Err(Refusal::Call)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );

      polkadot_sdk::sp_io::storage::start_transaction();
      System::note_extrinsic(encoded);
      let staged_only = observe_entry(&staged_key, &stored);
      assert_eq!(
        normalize_entry(&staged_only, 2, info.total_weight(), ceiling),
        Err(Refusal::Declaration)
      );
      assert_ok!(RuntimeUnsignedValidator::pre_dispatch(&extrinsic.function));
      assert_ok!(crate::TxExtension::bare_validate_and_prepare(
        &extrinsic.function,
        &info,
        len
      ));
      let prepared_root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let entry = observe_entry(&staged_key, &stored);
      assert_eq!(System::extrinsic_data(2).encode(), stored);
      let normalized = normalize_entry(&entry, 2, info.total_weight(), ceiling).unwrap();
      let base = crate::configs::RuntimeBlockWeights::get()
        .get(info.class)
        .base_extrinsic;
      let overhead = base
        .checked_add(&Weight::from_parts(0, u64::try_from(len).unwrap()))
        .unwrap();
      assert_eq!(Some(normalized), before.checked_add(&overhead));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        prepared_root
      );

      // Deliberately wrong staged bytes under the same prepared counter are a negative
      // caller-corruption probe, not an admission-valid block or a resource-ledger injection.
      polkadot_sdk::sp_io::storage::start_transaction();
      System::note_extrinsic(
        crate::UncheckedExtrinsic::new_bare(RuntimeCall::System(
          polkadot_sdk::frame_system::Call::remark {
            remark: vec![0; 4096],
          },
        ))
        .encode(),
      );
      let wrong_call = observe_entry(&staged_key, &stored);
      assert_eq!(wrong_call.mandatory, entry.mandatory);
      assert_eq!(
        normalize_entry(&wrong_call, 2, info.total_weight(), ceiling),
        Err(Refusal::Call)
      );
      polkadot_sdk::sp_io::storage::rollback_transaction();
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        prepared_root
      );

      let cases: &[(fn(&mut EntryObservation), Refusal)] = &[
        (|e| e.block += 1, Refusal::Context),
        (|e| e.context_ready = false, Refusal::Context),
        (|e| e.frozen = true, Refusal::Frozen),
        (|e| e.phase = Some(Phase::Initialization), Refusal::Stage),
        (|e| e.index = None, Refusal::Stage),
        (|e| e.index = Some(3), Refusal::Stage),
        (|e| e.inherents_finished = true, Refusal::Stage),
        (|e| e.staged_prepass = false, Refusal::Call),
        (|e| e.normal = Weight::from_parts(1, 0), Refusal::Class),
        (|e| e.operational = Weight::from_parts(0, 1), Refusal::Class),
        (
          |e| e.mandatory = Weight::from_parts(u64::MAX, e.mandatory.proof_size()),
          Refusal::Saturation,
        ),
        (
          |e| e.mandatory = Weight::from_parts(e.mandatory.ref_time(), u64::MAX),
          Refusal::Saturation,
        ),
        (|e| e.mandatory = Weight::zero(), Refusal::Declaration),
      ];
      for (change, refusal) in cases {
        let mut invalid = entry.clone();
        change(&mut invalid);
        assert_eq!(
          normalize_entry(&invalid, 2, info.total_weight(), ceiling),
          Err(*refusal)
        );
      }
      for axis in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
        let mut invalid = entry.clone();
        invalid.mandatory = info
          .total_weight()
          .checked_add(&ceiling)
          .unwrap()
          .checked_add(&axis)
          .unwrap();
        assert_eq!(
          normalize_entry(&invalid, 2, info.total_weight(), ceiling),
          Err(Refusal::Ceiling)
        );
      }
      let mut saturated = entry.clone();
      saturated.mandatory = Weight::from_parts(u64::MAX, u64::MAX);
      assert!(
        saturated
          .mandatory
          .checked_sub(&info.total_weight())
          .is_some(),
        "checked subtraction alone accepts a saturated counter"
      );
      assert_eq!(
        normalize_entry(&saturated, 2, info.total_weight(), ceiling),
        Err(Refusal::Saturation)
      );
      // Pure cases change only observations; the separate staged-call probe was rolled back.
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        prepared_root
      );
      polkadot_sdk::sp_io::storage::rollback_transaction();
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      assert_ok!(Executive::apply_extrinsic(extrinsic));
      let duplicate = observe_entry(&staged_key, &stored);
      assert_eq!(
        normalize_entry(&duplicate, 2, info.total_weight(), ceiling),
        Err(Refusal::Frozen)
      );
      println!(
        "CANONICAL_PREPASS_ENTRY_V1 {}",
        serde_json::json!({
          "normalizedRefTime": normalized.ref_time().to_string(),
          "normalizedProofSize": normalized.proof_size().to_string(),
          "retainedEncodedBytes": len.to_string(),
          "negativeLocalCases": cases.len() + 3,
          "historicalNonSaturationProved": false,
          "productionProviderChanged": false,
        })
      );
    }
    assert!(witnessed);
    Executive::finalize_block();
  });
}
