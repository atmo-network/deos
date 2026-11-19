//! Default-native coverage of inherent-only bare admission and ordinary external accounting.

use super::*;
use polkadot_sdk::{
  frame_system,
  sp_core::traits::{ReadRuntimeVersion, ReadRuntimeVersionExt},
  sp_externalities::Externalities,
  sp_runtime::transaction_validity::{TransactionSource, UnknownTransaction},
};

struct RejectRuntimeVersionRead;
impl ReadRuntimeVersion for RejectRuntimeVersionRead {
  fn read_runtime_version(
    &self,
    _wasm_code: &[u8],
    _ext: &mut dyn Externalities,
  ) -> Result<Vec<u8>, String> {
    Err("invalid runtime code".into())
  }
}

fn with_external_phase(check: impl FnOnce(&sr25519::Pair)) {
  with_prepass_boundary(|_| {}, check);
}

fn with_prepass_boundary(before: impl FnOnce(&sr25519::Pair), after: impl FnOnce(&sr25519::Pair)) {
  let (mut ext, signer) = prepare_external_phase(before);
  ext.execute_with(|| {
    after(&signer);
    Executive::finalize_block();
    assert!(Actors::finalized_block_resource_telemetry().is_some());
  });
}

fn prepare_external_phase(
  before: impl FnOnce(&sr25519::Pair),
) -> (TestExternalities, sr25519::Pair) {
  let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
  let parent = parent_header_for(fixture.storage.clone(), &[], 1);
  let mut ext = TestExternalities::new(fixture.storage);
  ext.register_extension(ReadRuntimeVersionExt::new(RejectRuntimeVersionRead));
  ext.execute_with(|| {
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
    Executive::initialize_block(&header);
    assert!(Actors::block_resource_state().is_none());
    let inherents = inherent_data_for(&parent, 2).create_extrinsics();
    assert_eq!(inherents.len(), 3);
    let mut seen = [false; 3];
    let mut before = Some(before);
    for inherent in inherents {
      let at = match &inherent.function {
        RuntimeCall::Timestamp(polkadot_sdk::pallet_timestamp::Call::set { .. }) => 0,
        RuntimeCall::ParachainSystem(
          polkadot_sdk::cumulus_pallet_parachain_system::Call::set_validation_data { .. },
        ) => 1,
        RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {}) => 2,
        _ => panic!("unexpected context call"),
      };
      assert!(!seen[at]);
      if at == 2 {
        assert_eq!(seen, [true, true, false]);
        assert!(Actors::block_resource_state().is_none());
        before.take().unwrap()(&fixture.signer);
      }
      seen[at] = true;
      assert_ok!(Executive::apply_extrinsic(inherent));
    }
    assert_eq!(seen, [true; 3]);
    assert!(before.is_none());
    let state = Actors::block_resource_state().expect("real Prepass freezes authority");
    assert_eq!(state.block_number(), 2);
    assert_eq!(
      state.phase(),
      pallet_deos_actors::BlockResourcePhase::ExternalPhase
    );
    assert_eq!(state.outstanding_reservations(), 0);
    assert_eq!(state.usage().user_dispatch_used(), Weight::zero());
  });
  (ext, fixture.signer)
}

fn assert_bare_refused(call: RuntimeCall) {
  let bare = crate::UncheckedExtrinsic::new_bare(call);
  let resource_before = Actors::block_resource_state();
  let accounts_before = frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  let issuance_before = Balances::total_issuance();
  let frame_before = System::block_weight();
  let length_before = System::block_size();
  let events_before = System::events();
  let parent_hash = System::parent_hash();
  let root_before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
  // Executive stages extrinsic bytes before pre_dispatch. Invalid builder applications
  // require caller rollback; rejection alone does not promise complete storage neutrality.
  polkadot_sdk::sp_io::storage::start_transaction();
  assert_eq!(
    Executive::apply_extrinsic(bare.clone()),
    Err(UnknownTransaction::NoUnsignedValidator.into()),
  );
  assert_eq!(System::block_weight(), frame_before);
  assert_eq!(System::block_size(), length_before);
  assert_eq!(Actors::block_resource_state(), resource_before);
  assert_eq!(
    frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
    accounts_before
  );
  assert_eq!(Balances::total_issuance(), issuance_before);
  assert_eq!(System::events(), events_before);
  polkadot_sdk::sp_io::storage::rollback_transaction();
  assert_eq!(
    polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
    root_before
  );
  // Pool validation creates a disposable next-block context too.
  polkadot_sdk::sp_io::storage::start_transaction();
  assert_eq!(
    Executive::validate_transaction(TransactionSource::External, bare, parent_hash),
    Err(UnknownTransaction::NoUnsignedValidator.into()),
  );
  polkadot_sdk::sp_io::storage::rollback_transaction();
  assert_eq!(
    polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
    root_before
  );
}

#[test]
fn signed_assets_transfer_all_at_max_issuance() {
  use crate::configs::address_event_ingress::AddressEventIngressExtension;
  use polkadot_sdk::{
    frame_support::assert_noop,
    pallet_assets,
    sp_core::Blake2Hasher,
    sp_runtime::{ArithmeticError, TokenError},
  };

  for (transfer_all, blocked) in [(false, false), (true, false), (true, true)] {
    let (mut ext, signer) = prepare_external_phase(|_| {});
    let alice = crate::AccountId::from(signer.public());
    let bob = super::super::common::BOB;
    let asset = 4_244;
    let fee_sink = crate::configs::actor_config::ActorFeeRecipient::get();
    let (signed, nonce, alice_refs, bob_refs, native_before, sink_before, frame_before) = ext
      .execute_with(|| {
        assert_ok!(crate::Assets::force_create(
          RuntimeOrigin::root(),
          asset,
          MultiAddress::Id(alice.clone()),
          true,
          1,
        ));
        for (account, amount) in [(&alice, 100), (&bob, u128::MAX - 100)] {
          assert_ok!(crate::Assets::mint(
            RuntimeOrigin::signed(alice.clone()),
            asset,
            MultiAddress::Id(account.clone()),
            amount,
          ));
        }
        assert_noop!(
          crate::Assets::mint(
            RuntimeOrigin::signed(alice.clone()),
            asset,
            MultiAddress::Id(alice.clone()),
            1,
          ),
          ArithmeticError::Overflow
        );
        assert_eq!(Actors::sovereign_index(&bob), None);
        if blocked {
          assert_ok!(crate::Assets::block(
            RuntimeOrigin::signed(alice.clone()),
            asset,
            MultiAddress::Id(bob.clone())
          ));
        }
        let nonce = System::account_nonce(&alice);
        let dest = MultiAddress::Id(bob.clone());
        let call = RuntimeCall::Assets(if transfer_all {
          pallet_assets::Call::transfer_all {
            id: asset,
            dest,
            keep_alive: false,
          }
        } else {
          pallet_assets::Call::transfer {
            id: asset,
            target: dest,
            amount: 100,
          }
        });
        let signed =
          actors_integration_tests::signed_extrinsic_in_current_context(&signer, nonce, call);
        (
          signed,
          nonce,
          System::account(&alice).sufficients,
          System::account(&bob).sufficients,
          Balances::free_balance(&alice),
          Balances::free_balance(&fee_sink),
          System::block_weight(),
        )
      });
    let len = signed.encoded_size();
    assert_eq!(nonce, 0);
    assert_eq!(len, if transfer_all { 145 } else { 146 });
    let info = signed.get_dispatch_info();
    // Commit setup before starting a fresh proof epoch. Do not let assertion reads or
    // finalization add to its witness. This is a local external-phase parent, not chain ancestry.
    ext.commit_all().unwrap();
    let recorder = Recorder::<Blake2Hasher>::default();
    ext.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    let result = ext.execute_with_recorder(recorder.clone(), || Executive::apply_extrinsic(signed));
    let result = result.expect("valid signed transfer");
    let originals = recorder.recorded_keys();
    let keys = originals.get(ext.backend.root()).unwrap();
    for key in [
      pallet_assets::Asset::<Runtime>::hashed_key_for(asset).to_vec(),
      pallet_assets::Account::<Runtime>::hashed_key_for(asset, &alice),
      pallet_assets::Account::<Runtime>::hashed_key_for(asset, &bob),
    ] {
      assert!(keys.contains_key(key.as_slice()));
    }
    ext.execute_with(|| {
      if blocked {
        assert_eq!(result, Err(TokenError::Blocked.into()));
        assert_eq!(crate::Assets::maybe_balance(asset, &alice), Some(100));
        assert_eq!(crate::Assets::balance(asset, &bob), u128::MAX - 100);
        assert_eq!(System::account(&alice).sufficients, alice_refs);
      } else {
        assert_ok!(result);
        assert_eq!(crate::Assets::maybe_balance(asset, &alice), None);
        assert_eq!(crate::Assets::balance(asset, &bob), u128::MAX);
        assert_eq!(System::account(&alice).sufficients, alice_refs - 1);
      }
      assert_eq!(System::account(&bob).sufficients, bob_refs);
      assert_eq!(crate::Assets::total_supply(asset), u128::MAX);
      assert_eq!(System::account_nonce(&alice), nonce + 1);
      let paid = native_before - Balances::free_balance(&alice);
      assert!(paid > 0);
      assert_eq!(Balances::free_balance(&fee_sink) - sink_before, paid);
      let after = Actors::block_resource_state().unwrap();
      let reserved = frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
        &crate::configs::RuntimeBlockWeights::get(),
        &info,
        len,
      );
      // Shared settlement precedes AddressEventIngress refunds and outer proof correction.
      assert_eq!(after.usage().user_dispatch_used(), reserved);
      assert_eq!(after.outstanding_reservations(), 0);
      let refund = AddressEventIngressExtension::post_dispatch_refund(blocked, false);
      let applied = System::events().last().unwrap().event.clone();
      let applied_info = match applied {
        RuntimeEvent::System(frame_system::Event::ExtrinsicSuccess { dispatch_info })
          if !blocked =>
        {
          dispatch_info
        }
        RuntimeEvent::System(frame_system::Event::ExtrinsicFailed {
          dispatch_info,
          dispatch_error,
        }) if blocked => {
          assert_eq!(dispatch_error, TokenError::Blocked.into());
          dispatch_info
        }
        other => panic!("unexpected Executive outcome event: {other:?}"),
      };
      assert_eq!(
        applied_info.weight.ref_time(),
        reserved.ref_time() - refund.ref_time()
      );
      assert_eq!(
        System::block_weight().get(info.class).ref_time() - frame_before.get(info.class).ref_time(),
        applied_info.weight.ref_time()
      );
      assert_eq!(System::events().iter().filter(|record| matches!(
        &record.event,
        RuntimeEvent::Assets(pallet_assets::Event::Transferred { asset_id, from, to, amount: 100 })
          if *asset_id == asset && *from == alice && *to == bob
      )).count(), usize::from(!blocked));
      Executive::finalize_block();
      assert!(Actors::finalized_block_resource_telemetry().is_some());
    });
  }
}

#[test]
fn reviewed_unsigned_rejects_unowned_bare_calls_before_frame_preparation() {
  with_external_phase(|_| {
    for call in [
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(super::super::common::BOB),
        value: 1,
      }),
      RuntimeCall::Preimage(polkadot_sdk::pallet_preimage::Call::note_preimage {
        bytes: Vec::new(),
      }),
    ] {
      assert_bare_refused(call);
    }
  });
}

#[test]
fn reviewed_unsigned_does_not_replace_signed_shared_accounting() {
  with_external_phase(|signer| {
    let recipient = super::super::common::BOB;
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let recipient_before = Balances::free_balance(&recipient);
    let signer_account = crate::AccountId::from(signer.public());
    let nonce = System::account_nonce(&signer_account);
    let before = Actors::block_resource_state().unwrap();
    let signed = actors_integration_tests::signed_extrinsic_in_current_context(
      signer,
      nonce,
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(recipient.clone()),
        value: amount,
      }),
    );
    let len = signed.encode().len();
    let class = signed.get_dispatch_info().class;
    assert_ok!(Executive::apply_extrinsic(signed));
    let after = Actors::block_resource_state().unwrap();
    let retained = crate::configs::RuntimeBlockWeights::get()
      .get(class)
      .base_extrinsic
      .checked_add(&Weight::from_parts(0, u64::try_from(len).unwrap()))
      .unwrap();
    let delta = after
      .usage()
      .user_dispatch_used()
      .checked_sub(&before.usage().user_dispatch_used())
      .unwrap();
    assert!(delta.all_gte(retained));
    assert!(delta.ref_time() > retained.ref_time());
    assert!(delta.proof_size() > retained.proof_size());
    assert_eq!(after.outstanding_reservations(), 0);
    assert_eq!(after.budget(), before.budget());
    assert_eq!(System::account_nonce(&signer_account), nonce + 1);
    assert_eq!(
      Balances::free_balance(&recipient),
      recipient_before + amount
    );
  });
}

#[test]
#[allow(deprecated)] // The upstream validator is a control, not the reference Executive policy.
fn reviewed_unsigned_rejects_bare_upgrade_even_when_system_authorizes_code() {
  use polkadot_sdk::sp_runtime::traits::ValidateUnsigned;
  with_external_phase(|_| {
    let code = vec![1u8, 2, 3, 4];
    let code_hash = BlakeTwo256::hash(&code);
    assert_ok!(System::authorize_upgrade(RuntimeOrigin::root(), code_hash));
    let call = frame_system::Call::apply_authorized_upgrade { code };
    assert!(System::validate_unsigned(TransactionSource::InBlock, &call).is_ok());
    assert_bare_refused(RuntimeCall::System(call));
    assert_eq!(
      System::authorized_upgrade().unwrap().code_hash(),
      &code_hash
    );
  });
}

#[test]
fn reviewed_unsigned_signed_upgrade_obeys_prepass_and_shared_settlement() {
  let code = vec![1u8, 2, 3, 4];
  let code_hash = BlakeTwo256::hash(&code);
  let call = RuntimeCall::System(frame_system::Call::apply_authorized_upgrade { code });
  with_prepass_boundary(
    |signer| {
      assert_ok!(System::authorize_upgrade(RuntimeOrigin::root(), code_hash));
      let nonce = System::account_nonce(crate::AccountId::from(signer.public()));
      let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let signed =
        actors_integration_tests::signed_extrinsic_in_current_context(signer, nonce, call.clone());
      assert!(Actors::block_resource_state().is_none());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      polkadot_sdk::sp_io::storage::start_transaction();
      assert_eq!(
        Executive::apply_extrinsic(signed),
        Err(InvalidTransaction::Custom(42).into())
      );
      polkadot_sdk::sp_io::storage::rollback_transaction();
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      assert!(Actors::block_resource_state().is_none());
      assert_eq!(
        System::authorized_upgrade().unwrap().code_hash(),
        &code_hash
      );
    },
    |signer| {
      let before = Actors::block_resource_state().unwrap();
      let signer_account = crate::AccountId::from(signer.public());
      let nonce = System::account_nonce(&signer_account);
      let signed =
        actors_integration_tests::signed_extrinsic_in_current_context(signer, nonce, call.clone());
      let info = signed.get_dispatch_info();
      let len = u64::try_from(signed.encode().len()).unwrap();
      let minimum = info
        .call_weight
        .checked_add(
          &crate::configs::RuntimeBlockWeights::get()
            .get(info.class)
            .base_extrinsic,
        )
        .and_then(|weight| weight.checked_add(&Weight::from_parts(0, len)))
        .unwrap();
      let code_before = polkadot_sdk::sp_io::storage::get(well_known_keys::CODE);
      assert_ok!(Executive::apply_extrinsic(signed));
      let after = Actors::block_resource_state().unwrap();
      let delta = after
        .usage()
        .user_dispatch_used()
        .checked_sub(&before.usage().user_dispatch_used())
        .unwrap();
      assert!(delta.all_gte(minimum));
      assert_eq!(after.outstanding_reservations(), 0);
      assert_eq!(after.budget(), before.budget());
      assert_eq!(
        after.phase(),
        pallet_deos_actors::BlockResourcePhase::ExternalPhase
      );
      assert_eq!(System::account_nonce(&signer_account), nonce + 1);
      assert!(System::authorized_upgrade().is_none());
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(well_known_keys::CODE),
        code_before
      );
      assert!(System::events().iter().any(|record| matches!(
        record.event,
        RuntimeEvent::System(frame_system::Event::RejectedInvalidAuthorizedUpgrade { .. })
      )));
    },
  );
}
