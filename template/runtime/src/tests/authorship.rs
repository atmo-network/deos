//! Default-native transition conformance, not block admission, ancestry or physical pricing.

use super::{h1_storage_trace, production_block_replay::preset_genesis_storage};
use crate::{
  Aura, Authorship, Balances, CollatorSelection, Runtime, RuntimeCall, RuntimeOrigin, Session,
  Staking, System, configs::RuntimeSessionRotation,
};
use codec::Encode;
use pallet_session_rotation::SessionRotation as Rotation;
use polkadot_sdk::{
  frame_executive::OnInitializeWithWeightRegistration,
  frame_support::{
    assert_noop, assert_ok,
    dispatch::DispatchClass,
    traits::{FindAuthor, Hooks, StorageInfoTrait},
    weights::Weight,
  },
  pallet_aura,
  pallet_collator_selection::{self, WeightInfo as _},
  pallet_session,
  sp_io::TestExternalities,
  sp_runtime::{Digest, DigestItem, DispatchError, traits::Dispatchable},
};

#[test]
fn authorship_empty_queue_transition_uses_actual_reference_calls() {
  for preset in crate::genesis_config_presets::preset_names() {
    for purge in [false, true] {
      h1_storage_trace::recorded(|| {
        // Empty code is the existing native-genesis convention, not an executing Wasm.
        TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
          assert_eq!(Staking::native_security_mode(), pallet_staking::NativeSecurityMode::TrustedSet);
          let validators = Session::validators();
          let queued = Session::queued_keys();
          let authorities = pallet_aura::Authorities::<Runtime>::get();
          assert!(!validators.is_empty());
          assert_eq!(queued.len(), validators.len());
          assert_eq!(authorities.len(), validators.len());
          assert_eq!(Session::current_index(), 0);
          assert!(!pallet_session::QueuedChanged::<Runtime>::get());
          let pot = CollatorSelection::account_id();
          assert!(!validators.contains(&pot));
          assert_ok!(Balances::transfer_keep_alive(
            RuntimeOrigin::signed(validators[0].clone()),
            pot.clone().into(),
            16 * crate::EXISTENTIAL_DEPOSIT,
          ));
          assert_noop!(Session::purge_keys(RuntimeOrigin::none()), DispatchError::BadOrigin);
          for validator in &validators {
            assert!(Session::load_keys(validator).is_some());
            if purge {
              assert_ok!(RuntimeCall::Session(pallet_session::Call::purge_keys {}).dispatch(
                RuntimeOrigin::signed(validator.clone())
              ));
              assert!(Session::load_keys(validator).is_none());
            }
          }
          assert_eq!(Session::validators(), validators);
          assert_eq!(Session::queued_keys(), queued);
          assert!(!pallet_session::QueuedChanged::<Runtime>::get());
          assert_eq!(pallet_aura::Authorities::<Runtime>::get(), authorities);

          // Invoke the actual adapter twice, without pretending to execute periodic blocks.
          <RuntimeSessionRotation as Rotation<crate::BlockNumber>>::rotate();
          assert_eq!(Session::current_index(), 1);
          assert_eq!(Session::validators(), validators);
          assert_eq!(Session::queued_keys().is_empty(), purge);
          assert!(pallet_session::QueuedChanged::<Runtime>::get());
          assert_eq!(pallet_aura::Authorities::<Runtime>::get(), authorities);
          <RuntimeSessionRotation as Rotation<crate::BlockNumber>>::rotate();
          assert_eq!(Session::current_index(), 2);
          assert_eq!(Session::validators().is_empty(), purge);
          assert_eq!(Session::queued_keys().is_empty(), purge);
          assert!(pallet_session::QueuedChanged::<Runtime>::get());
          assert_eq!(pallet_aura::Authorities::<Runtime>::get(), authorities);

          // One component context, not a sparse or fabricated chain history.
          let digest = Digest {
            logs: vec![DigestItem::PreRuntime(
              polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
              1u64.encode(),
            )],
          };
          System::initialize(&1, &System::block_hash(0), &digest);
          let index = <Aura as FindAuthor<u32>>::find_author(
            digest.logs.iter().filter_map(DigestItem::as_pre_runtime),
          ).unwrap();
          let candidate = validators[index as usize].clone();
          let cache_key = Authorship::storage_info().into_iter()
            .find(|info| info.storage_name == b"Author")
            .expect("SDK Authorship cache metadata exists").prefix;
          assert!(!polkadot_sdk::sp_io::storage::exists(&cache_key));
          let pot_before = Balances::free_balance(&pot);
          let recipient_before = Balances::free_balance(&candidate);
          let issuance = Balances::total_issuance();
          let last_authored = pallet_collator_selection::LastAuthoredBlock::<Runtime>::get(&candidate);
          let reward = pot_before.saturating_sub(crate::EXISTENTIAL_DEPOSIT) / 2;
          assert!(reward > 0);
          let mut expected_weight = System::block_weight();
          if !purge {
            expected_weight.accrue(
              <Runtime as pallet_collator_selection::Config>::WeightInfo::note_author(),
              DispatchClass::Mandatory,
            );
          }
          let sample = h1_storage_trace::Sample::new();
          let returned = sample.phase("authorship_initialize", || {
            <(Authorship,) as OnInitializeWithWeightRegistration<Runtime>>::on_initialize_with_weight_registration(1)
          });
          assert_eq!(returned, Weight::zero());
          assert_eq!(System::block_weight(), expected_weight);
          assert_eq!(Authorship::author(), (!purge).then_some(candidate.clone()));
          assert_eq!(polkadot_sdk::sp_io::storage::exists(&cache_key), !purge);
          assert_eq!(Balances::total_issuance(), issuance);
          assert_eq!(Balances::free_balance(&pot), pot_before - if purge { 0 } else { reward });
          assert_eq!(Balances::free_balance(&candidate), recipient_before + if purge { 0 } else { reward });
          assert_eq!(
            pallet_collator_selection::LastAuthoredBlock::<Runtime>::get(&candidate),
            if purge { last_authored } else { 1 },
          );
          sample.phase("authorship_finalize", || Authorship::on_finalize(1));
          assert!(!polkadot_sdk::sp_io::storage::exists(&cache_key));
          let trace = sample.finish().expect("existing logical observer is enabled");
          let cache = &trace["owners"]["Authorship.Author"];
          assert_eq!(cache["calls"]["Get"], 1);
          assert_eq!(cache["calls"]["Delete"], 1);
          assert_eq!(cache["net_changed_keys"], 0);
          assert_eq!(cache["calls"]["Put"], if purge { serde_json::Value::Null } else { 1.into() });
          assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Put"], if purge { 1 } else { 2 });
          println!("AUTHORSHIP_EMPTY_QUEUE {}", serde_json::json!({
            "preset": preset, "purged": purge, "rotations": 2,
            "aura_count": authorities.len(), "session_count": Session::validators().len(),
            "trace": trace,
          }));
        });
      });
    }
  }
}
