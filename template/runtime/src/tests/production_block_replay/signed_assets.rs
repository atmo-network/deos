//! Finite signed Assets diagnostic on explicitly selected Wasm, not an accepted-artifact selector.

use super::*;
use polkadot_sdk::{
  frame_system::{Event as SystemEvent, Phase},
  pallet_assets::{self, Call, Event as AssetEvent},
  pallet_transaction_payment::Event as PaymentEvent,
  sp_io::hashing::sha2_256,
  sp_runtime::{ArithmeticError, TokenError},
};

#[test]
#[ignore = "requires DEOS_ASSETS_DIAGNOSTIC_WASM and DEOS_ASSETS_DIAGNOSTIC_SHA256"]
fn signed_assets_diagnostic_wasm() {
  super::super::common::ensure_artifact_profile().expect("normal artifact profile");
  assert!(
    !cfg!(feature = "try-runtime"),
    "use default runtime features"
  );
  let path = std::path::PathBuf::from(std::env::var_os("DEOS_ASSETS_DIAGNOSTIC_WASM").unwrap());
  assert!(path.is_absolute(), "select an explicit absolute Wasm path");
  let wasm = std::fs::read(&path).unwrap();
  let digest = sha2_256(&wasm)
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
  assert_eq!(
    digest,
    std::env::var("DEOS_ASSETS_DIAGNOSTIC_SHA256").unwrap()
  );
  let pvf_output = std::env::var_os("DEOS_ASSETS_DIAGNOSTIC_PVF").map(std::path::PathBuf::from);
  if let Some(output) = &pvf_output {
    assert!(
      output.is_absolute(),
      "select an explicit absolute fixture path"
    );
    std::fs::create_dir(output).expect("new fixture directory");
  }
  for preset in crate::genesis_config_presets::preset_names() {
    let signer = sr25519::Pair::from_seed(&[66; 32]);
    let alice = crate::AccountId::from(signer.public());
    let bob = super::super::common::BOB;
    let empty = super::super::common::CHARLIE;
    let sink = crate::configs::actor_config::ActorFeeRecipient::get();
    let (ordinary, all, retained, blocked, unknown) = (4244, 4245, 4246, 4247, 4248);
    let mut ext = externalities_with_bound_code(preset_genesis_storage(&wasm, &preset));
    let (pre_state, refs, bob_refs, payer_before, sink_before, blocked_before) =
      ext.execute_with(|| {
        assert_ok!(Balances::force_set_balance(
          RuntimeOrigin::root(),
          MultiAddress::Id(alice.clone()),
          1_000_000 * crate::EXISTENTIAL_DEPOSIT,
        ));
        for id in [ordinary, all, retained, blocked] {
          let issuer = if id == blocked { &bob } else { &alice };
          assert_ok!(Assets::force_create(
            RuntimeOrigin::root(),
            id,
            MultiAddress::Id(issuer.clone()),
            true,
            1,
          ));
          for (who, amount) in [(&alice, 100), (&bob, u128::MAX - 100)] {
            assert_ok!(Assets::mint(
              RuntimeOrigin::signed(issuer.clone()),
              id,
              MultiAddress::Id(who.clone()),
              amount,
            ));
          }
          assert_eq!(Assets::total_supply(id), u128::MAX);
        }
        assert_ok!(Assets::block(
          RuntimeOrigin::signed(bob.clone()),
          blocked,
          MultiAddress::Id(bob.clone()),
        ));
        assert!(pallet_assets::Asset::<Runtime>::get(unknown).is_none());
        assert_eq!(System::account_nonce(&alice), 0);
        (
          current_top_storage(),
          System::account(&alice).sufficients,
          System::account(&bob).sufficients,
          Balances::free_balance(&alice),
          Balances::free_balance(&sink),
          [
            pallet_assets::Account::<Runtime>::get(blocked, &alice),
            pallet_assets::Account::<Runtime>::get(blocked, &bob),
          ],
        )
      });
    let dest = || MultiAddress::Id(bob.clone());
    let mint = |id, amount| Call::mint {
      id,
      beneficiary: MultiAddress::Id(empty.clone()),
      amount,
    };
    let burn = |id| Call::burn {
      id,
      who: MultiAddress::Id(alice.clone()),
      amount: 0,
    };
    let transferred = |asset_id, amount| AssetEvent::Transferred {
      asset_id,
      from: alice.clone(),
      to: bob.clone(),
      amount,
    };
    let denied = pallet_assets::Error::<Runtime>::NoPermission.into();
    let missing = pallet_assets::Error::<Runtime>::Unknown.into();
    let cases = [
      (
        Call::transfer {
          id: ordinary,
          target: dest(),
          amount: 100,
        },
        Ok(()),
        Some(transferred(ordinary, 100)),
      ),
      (
        Call::transfer_all {
          id: all,
          dest: dest(),
          keep_alive: false,
        },
        Ok(()),
        Some(transferred(all, 100)),
      ),
      (
        Call::transfer_all {
          id: retained,
          dest: dest(),
          keep_alive: true,
        },
        Ok(()),
        Some(transferred(retained, 99)),
      ),
      (mint(all, 1), Err(ArithmeticError::Overflow.into()), None),
      (
        Call::transfer_all {
          id: blocked,
          dest: dest(),
          keep_alive: false,
        },
        Err(TokenError::Blocked.into()),
        None,
      ),
      (mint(blocked, 0), Err(denied), None),
      (burn(blocked), Err(denied), None),
      (mint(unknown, 0), Err(missing), None),
      (burn(unknown), Err(missing), None),
      (
        mint(retained, 0),
        Ok(()),
        Some(AssetEvent::Issued {
          asset_id: retained,
          owner: empty.clone(),
          amount: 0,
        }),
      ),
      (
        burn(retained),
        Ok(()),
        Some(AssetEvent::Burned {
          asset_id: retained,
          owner: alice.clone(),
          balance: 0,
        }),
      ),
    ];
    let calls = cases
      .iter()
      .map(|(call, _, _)| RuntimeCall::Assets(call.clone()))
      .collect::<Vec<_>>();
    let results = cases
      .iter()
      .map(|(_, result, _)| *result)
      .collect::<Vec<_>>();
    let signed = calls
      .iter()
      .enumerate()
      .map(|(i, call)| (&signer, u32::try_from(i).unwrap(), call))
      .collect::<Vec<_>>();
    let parent = parent_header_for(pre_state.clone(), &wasm, 0);
    let authored = author_complete_block_after_with_expected_dispatch(
      pre_state,
      &wasm,
      &parent,
      1,
      UserDemand::ActorOnly,
      &signer,
      0,
      &BTreeMap::new(),
      &signed,
      inherent_data_for(&parent, 1),
      Some(&results),
    );
    let state_root = *authored.block.header.state_root();
    assert_eq!(
      *parent_header_for(authored.post_state.clone(), &wasm, 1).state_root(),
      state_root
    );
    externalities_with_bound_code(authored.post_state.clone()).execute_with(|| {
      for (id, source, destination) in [
        (ordinary, None, u128::MAX),
        (all, None, u128::MAX),
        (retained, Some(1), u128::MAX - 1),
        (blocked, Some(100), u128::MAX - 100),
      ] {
        assert_eq!(Assets::maybe_balance(id, &alice), source);
        assert_eq!(Assets::balance(id, &bob), destination);
        assert_eq!(Assets::maybe_balance(id, &empty), None);
        assert_eq!(Assets::total_supply(id), u128::MAX);
      }
      assert!(pallet_assets::Asset::<Runtime>::get(unknown).is_none());
      assert_eq!(
        [
          pallet_assets::Account::<Runtime>::get(blocked, &alice),
          pallet_assets::Account::<Runtime>::get(blocked, &bob),
        ],
        blocked_before
      );
      assert_eq!(System::account(&alice).sufficients, refs - 2);
      assert_eq!(System::account(&bob).sufficients, bob_refs);
      assert_eq!(System::account(&empty).sufficients, 0);
      let events = System::events();
      let start = authored.block.extrinsics.len() - cases.len();
      let mut total_fee = 0;
      for (i, (_, result, expected_asset)) in cases.iter().enumerate() {
        let phase = Phase::ApplyExtrinsic(u32::try_from(start + i).unwrap());
        let events = events
          .iter()
          .filter(|record| record.phase == phase)
          .map(|record| &record.event)
          .collect::<Vec<_>>();
        let actual_assets = events
          .iter()
          .filter_map(|event| match event {
            RuntimeEvent::Assets(event) => Some(event),
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(
          actual_assets,
          expected_asset.iter().collect::<Vec<_>>(),
          "asset events for signed call {i}"
        );
        match (result, events.last().unwrap()) {
          (Ok(()), RuntimeEvent::System(SystemEvent::ExtrinsicSuccess { .. })) => (),
          (
            Err(expected),
            RuntimeEvent::System(SystemEvent::ExtrinsicFailed { dispatch_error, .. }),
          ) => assert_eq!(dispatch_error, expected),
          _ => panic!("missing or incorrect Executive dispatch outcome for signed call {i}"),
        }
        let fees = events
          .iter()
          .filter_map(|event| match event {
            RuntimeEvent::TransactionPayment(PaymentEvent::TransactionFeePaid {
              who,
              actual_fee,
              tip,
            }) => {
              assert_eq!(who, &alice);
              assert_eq!(*tip, 0);
              Some(*actual_fee)
            }
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(fees.len(), 1, "fee event for signed call {i}");
        assert!(fees[0] > 0);
        total_fee += fees[0];
      }
      assert_eq!(
        System::account_nonce(&alice),
        u32::try_from(cases.len()).unwrap()
      );
      assert_eq!(Balances::free_balance(&alice), payer_before - total_fee);
      assert_eq!(Balances::free_balance(&sink), sink_before + total_fee);
    });
    let replay = super::super::wasm_replay::replay_block(
      authored.pre_state.clone(),
      &wasm,
      &authored.block.encode(),
      &authored.inherent_data,
    )
    .expect("exact Wasm completes the full block and independently replays with only its proof");
    assert_eq!(replay.pre_state_root, *parent.state_root());
    assert_eq!(replay.post_state_root, state_root);
    assert_eq!(replay.block_hash, authored.block.header.hash());
    println!(
      "signed_assets preset={preset} sha256={digest} calls={} refused={} block={:?} parent={:?} final_root={state_root:?} execution_proof={} verification_compact_proof={} ancestry=prepared",
      cases.len(),
      results.iter().filter(|result| result.is_err()).count(),
      replay.block_hash,
      replay.pre_state_root,
      replay.execution_proof.storage_proof_scale_bytes,
      replay.verification_proof.compact_proof_scale_bytes,
    );
    if let Some(output) = &pvf_output {
      pvf_fixture::export(
        output,
        &preset,
        (
          authored.pre_state,
          authored.post_state,
          &parent,
          authored.block,
          authored.inherent_data,
        ),
      )
      .expect("export the disputed signed calls with their sealed proof and typed expected result");
    }
  }
}
