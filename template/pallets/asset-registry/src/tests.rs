use crate::{Error, Event, mock::*};
use polkadot_sdk::{
  frame_support::{assert_noop, assert_ok, traits::fungibles::Inspect},
  frame_system, pallet_assets,
  sp_runtime::traits::Convert,
  staging_xcm::latest::{Junction::Parachain, Junctions, Location},
};
use primitives::assets::{CurrencyMetadata, TYPE_FOREIGN};
use std::sync::Arc;

#[test]
fn register_foreign_asset_works() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Location mapping to a namespaced ID via MockLocationToAssetId.
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let asset_id = TYPE_FOREIGN | 1000;
    let metadata = CurrencyMetadata {
      name: b"Sibling Token".to_vec(),
      symbol: b"SIBL".to_vec(),
      decimals: 12,
    };
    let min_balance = 10;
    let is_sufficient = true;

    // 1. Register
    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      location.clone(),
      metadata.clone(),
      min_balance,
      is_sufficient
    ));

    // 2. Verify Storage Persistence
    assert_eq!(
      crate::Pallet::<Test>::location_to_asset(&location),
      Some(asset_id)
    );
    assert_eq!(
      crate::Pallet::<Test>::asset_to_location(asset_id),
      Some(location.clone())
    );

    // 3. Verify Event
    frame_system::Pallet::<Test>::assert_last_event(RuntimeEvent::AssetRegistry(
      Event::ForeignAssetRegistered {
        asset_id,
        location: location.clone(),
        symbol: metadata.symbol.clone(),
      },
    ));

    // 4. Verify Assets Pallet State
    // Check Metadata
    let stored_metadata = polkadot_sdk::pallet_assets::Metadata::<Test>::get(asset_id);
    assert_eq!(stored_metadata.name, metadata.name);
    assert_eq!(stored_metadata.symbol, metadata.symbol);
    assert_eq!(stored_metadata.decimals, metadata.decimals);
  });
}

#[test]
fn host_reserved_location_cannot_enter_or_receive_the_foreign_bijection() {
  new_test_ext().execute_with(|| {
    let metadata = CurrencyMetadata {
      name: b"Reserved".to_vec(),
      symbol: b"RSV".to_vec(),
      decimals: 12,
    };
    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        Location::here(),
        metadata.clone(),
        1,
        true,
      ),
      Error::<Test>::ReservedLocation
    );
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      location.clone(),
      metadata,
      1,
      true,
    ));
    assert_noop!(
      crate::Pallet::<Test>::link_existing_asset(
        RuntimeOrigin::root(),
        Location::here(),
        TYPE_FOREIGN | 1000,
      ),
      Error::<Test>::ReservedLocation
    );
    assert_noop!(
      crate::Pallet::<Test>::migrate_location_key(
        RuntimeOrigin::root(),
        location,
        Location::here(),
      ),
      Error::<Test>::ReservedLocation
    );
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_reconciles_exact_foreign_asset_bijection_and_ledger_identity() {
  for corruption in 0u8..7 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
      let asset_id = TYPE_FOREIGN | 1000;
      assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location.clone(),
        CurrencyMetadata {
          name: b"Sibling Token".to_vec(),
          symbol: b"SIBL".to_vec(),
          decimals: 12,
        },
        10,
        true,
      ));
      assert_ok!(crate::Pallet::<Test>::do_try_state());

      match corruption {
        0 => crate::ForeignAssetLocationByAssetId::<Test>::remove(asset_id),
        1 => crate::ForeignAssetLocationByAssetId::<Test>::insert(
          asset_id,
          Location::new(1, Junctions::X1(Arc::new([Parachain(2000)]))),
        ),
        2 => crate::ForeignAssetLocationByAssetId::<Test>::insert(
          TYPE_FOREIGN | 2000,
          Location::new(1, Junctions::X1(Arc::new([Parachain(2000)]))),
        ),
        3 => {
          polkadot_sdk::pallet_assets::Asset::<Test>::remove(asset_id);
        }
        4 => crate::ForeignAssetMapping::<Test>::insert(
          Location::new(1, Junctions::X1(Arc::new([Parachain(2000)]))),
          asset_id,
        ),
        5 => {
          let local_id = 123;
          let local_location = Location::new(1, Junctions::X1(Arc::new([Parachain(3000)])));
          crate::ForeignAssetMapping::<Test>::insert(&local_location, local_id);
          crate::ForeignAssetLocationByAssetId::<Test>::insert(local_id, local_location);
        }
        6 => crate::ForeignAssetMapping::<Test>::insert(Location::here(), asset_id),
        _ => unreachable!(),
      }
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "Asset Registry corruption case {corruption} must fail",
      );
    });
  }
}

#[test]
fn register_duplicate_fails() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let metadata = CurrencyMetadata {
      name: b"Sibling Token".to_vec(),
      symbol: b"SIBL".to_vec(),
      decimals: 12,
    };

    // First registration works
    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      location.clone(),
      metadata.clone(),
      10,
      true
    ));

    // Second registration fails
    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location,
        metadata,
        10,
        true
      ),
      Error::<Test>::AssetAlreadyRegistered
    );
  });
}

#[test]
fn register_collision_fails() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let metadata = CurrencyMetadata {
      name: b"Sibling Token".to_vec(),
      symbol: b"SIBL".to_vec(),
      decimals: 12,
    };

    // Pre-occupy the generated namespaced ID.
    let asset_id = TYPE_FOREIGN | 1000;
    assert_ok!(pallet_assets::Pallet::<Test>::force_create(
      RuntimeOrigin::root(),
      asset_id,
      1,    // owner
      true, // is_sufficient
      10    // min_balance
    ));

    // Attempt to register foreign asset mapping to same ID
    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location,
        metadata,
        10,
        true
      ),
      Error::<Test>::AssetIdCollision
    );
  });
}

#[test]
fn register_foreign_asset_fails_bad_origin() {
  new_test_ext().execute_with(|| {
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let metadata = CurrencyMetadata {
      name: b"Sibling Token".to_vec(),
      symbol: b"SIBL".to_vec(),
      decimals: 12,
    };

    // Attempt with signed origin (Mock requires Root)
    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::signed(2),
        location,
        metadata,
        10,
        true
      ),
      polkadot_sdk::sp_runtime::DispatchError::BadOrigin
    );
  });
}

#[test]
fn generated_registration_rejects_non_foreign_namespace() {
  new_test_ext().execute_with(|| {
    let location = Location::parent();
    let metadata = CurrencyMetadata {
      name: b"Invalid Namespace".to_vec(),
      symbol: b"INV".to_vec(),
      decimals: 12,
    };

    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location.clone(),
        metadata,
        1,
        true,
      ),
      Error::<Test>::InvalidAssetIdMask
    );
    assert!(crate::Pallet::<Test>::location_to_asset(location).is_none());
  });
}

#[test]
fn registration_rejects_retained_reverse_identity_after_ledger_removal() {
  new_test_ext().execute_with(|| {
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let retained_location = Location::new(1, Junctions::X1(Arc::new([Parachain(999)])));
    let asset_id = TYPE_FOREIGN | 1000;
    crate::ForeignAssetLocationByAssetId::<Test>::insert(asset_id, retained_location.clone());
    let metadata = CurrencyMetadata {
      name: b"Collision".to_vec(),
      symbol: b"COL".to_vec(),
      decimals: 12,
    };

    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location.clone(),
        metadata,
        1,
        true,
      ),
      Error::<Test>::AssetIdCollision
    );
    assert!(crate::Pallet::<Test>::location_to_asset(location).is_none());
    assert_eq!(
      crate::Pallet::<Test>::asset_to_location(asset_id),
      Some(retained_location)
    );
  });
}

#[test]
fn failed_link_hook_rolls_back_both_identity_directions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(3000)])));
    let asset_id = TYPE_FOREIGN | 77;
    assert_ok!(pallet_assets::Pallet::<Test>::force_create(
      RuntimeOrigin::root(),
      asset_id,
      1,
      true,
      1,
    ));
    let events_before = frame_system::Pallet::<Test>::event_count();
    set_token_domain_hook_failure(true);

    assert_eq!(
      crate::Pallet::<Test>::link_existing_asset(
        RuntimeOrigin::root(),
        location.clone(),
        asset_id,
      ),
      Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "TokenDomainHookRejected"
      ))
    );
    set_token_domain_hook_failure(false);

    assert!(crate::Pallet::<Test>::location_to_asset(location).is_none());
    assert!(crate::Pallet::<Test>::asset_to_location(asset_id).is_none());
    assert!(<pallet_assets::Pallet<Test> as Inspect<u64>>::asset_exists(asset_id));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
  });
}

#[test]
fn link_existing_asset_emits_symbol() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(3001)])));
    let asset_id = TYPE_FOREIGN | 77;

    assert_ok!(pallet_assets::Pallet::<Test>::force_create(
      RuntimeOrigin::root(),
      asset_id,
      1,
      true,
      1
    ));
    assert_ok!(pallet_assets::Pallet::<Test>::force_set_metadata(
      RuntimeOrigin::root(),
      asset_id,
      b"Linked Token".to_vec(),
      b"LNK".to_vec(),
      12,
      false
    ));

    assert_ok!(crate::Pallet::<Test>::link_existing_asset(
      RuntimeOrigin::root(),
      location.clone(),
      asset_id
    ));

    frame_system::Pallet::<Test>::assert_last_event(RuntimeEvent::AssetRegistry(
      Event::ForeignAssetRegistered {
        asset_id,
        location,
        symbol: b"LNK".to_vec(),
      },
    ));
  });
}

#[test]
fn migrate_location_key_fails_on_occupied_new_location() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let old_location = Location::new(1, Junctions::X1(Arc::new([Parachain(8000)])));
    let new_location = Location::new(1, Junctions::X1(Arc::new([Parachain(8001)])));
    let metadata = CurrencyMetadata {
      name: b"Foreign Token".to_vec(),
      symbol: b"FRGN".to_vec(),
      decimals: 12,
    };

    // Register at old_location
    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      old_location.clone(),
      metadata.clone(),
      10,
      true
    ));

    // Register at new_location
    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      new_location.clone(),
      metadata,
      10,
      true
    ));

    // Migrate should fail because new_location is occupied
    assert_noop!(
      crate::Pallet::<Test>::migrate_location_key(
        RuntimeOrigin::root(),
        old_location.clone(),
        new_location.clone()
      ),
      Error::<Test>::AssetAlreadyRegistered
    );

    // Verify old_location mapping is preserved
    assert!(crate::Pallet::<Test>::location_to_asset(&old_location).is_some());
  });
}

#[test]
fn register_foreign_asset_fails_oversized_metadata() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let metadata = CurrencyMetadata {
      name: vec![b'X'; 51],
      symbol: b"SIBL".to_vec(),
      decimals: 12,
    };

    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset(
        RuntimeOrigin::root(),
        location.clone(),
        metadata,
        10,
        true
      ),
      Error::<Test>::MetadataTooLong
    );

    // Verify no mapping was created
    assert!(crate::Pallet::<Test>::location_to_asset(&location).is_none());
    // Verify no asset was created
    assert!(!<pallet_assets::Pallet<Test> as Inspect<u64>>::asset_exists(TYPE_FOREIGN | 1000));
  });
}

#[test]
fn register_foreign_asset_with_id_fails_oversized_metadata() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location = Location::new(1, Junctions::X1(Arc::new([Parachain(1000)])));
    let asset_id = TYPE_FOREIGN | 123;
    let metadata = CurrencyMetadata {
      name: b"SIBL".to_vec(),
      symbol: vec![b'X'; 51],
      decimals: 12,
    };

    assert_noop!(
      crate::Pallet::<Test>::register_foreign_asset_with_id(
        RuntimeOrigin::root(),
        location.clone(),
        asset_id,
        metadata,
        10,
        true
      ),
      Error::<Test>::MetadataTooLong
    );

    // Verify no mapping was created
    assert!(crate::Pallet::<Test>::location_to_asset(&location).is_none());
    // Verify no asset was created
    assert!(!<pallet_assets::Pallet<Test> as Inspect<u64>>::asset_exists(asset_id));
  });
}

#[test]
fn link_existing_asset_rejects_duplicate_asset_id() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let location1 = Location::new(1, Junctions::X1(Arc::new([Parachain(3001)])));
    let location2 = Location::new(1, Junctions::X1(Arc::new([Parachain(3002)])));
    let asset_id = TYPE_FOREIGN | 77;

    // Create asset and link to location1
    assert_ok!(pallet_assets::Pallet::<Test>::force_create(
      RuntimeOrigin::root(),
      asset_id,
      1,
      true,
      1
    ));
    assert_ok!(crate::Pallet::<Test>::link_existing_asset(
      RuntimeOrigin::root(),
      location1.clone(),
      asset_id
    ));

    // Try to link same asset_id to location2
    assert_noop!(
      crate::Pallet::<Test>::link_existing_asset(
        RuntimeOrigin::root(),
        location2.clone(),
        asset_id
      ),
      Error::<Test>::AssetAlreadyRegistered
    );

    // Verify location1 mapping is preserved
    assert_eq!(
      crate::Pallet::<Test>::location_to_asset(&location1),
      Some(asset_id)
    );
    assert_eq!(
      crate::Pallet::<Test>::asset_to_location(asset_id),
      Some(location1.clone())
    );
    // Verify location2 has no mapping
    assert!(crate::Pallet::<Test>::location_to_asset(&location2).is_none());
  });
}

#[test]
fn migrate_location_key_emits_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let old_location = Location::new(1, Junctions::X1(Arc::new([Parachain(8000)])));
    let new_location = Location::new(1, Junctions::X1(Arc::new([Parachain(8001)])));
    let metadata = CurrencyMetadata {
      name: b"Foreign Token".to_vec(),
      symbol: b"FRGN".to_vec(),
      decimals: 12,
    };

    assert_ok!(crate::Pallet::<Test>::register_foreign_asset(
      RuntimeOrigin::root(),
      old_location.clone(),
      metadata,
      10,
      true
    ));

    let asset_id = crate::Pallet::<Test>::location_to_asset(&old_location).unwrap();

    assert_ok!(crate::Pallet::<Test>::migrate_location_key(
      RuntimeOrigin::root(),
      old_location.clone(),
      new_location.clone()
    ));
    assert_eq!(
      crate::Pallet::<Test>::asset_to_location(asset_id),
      Some(new_location.clone())
    );

    frame_system::Pallet::<Test>::assert_last_event(RuntimeEvent::AssetRegistry(
      Event::MigrationApplied {
        asset_id,
        old_location,
        new_location,
      },
    ));
  });
}

#[cfg(test)]
mod proptest_asset_registry {
  use super::*;
  use proptest::prelude::*;

  fn location_for_para(para_id: u32) -> Location {
    Location::new(1, Junctions::X1(Arc::new([Parachain(para_id)])))
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn register_lookup_isomorphism(para_id in 1u32..100_000u32) {
      let (stored, converted, reverse, convert_back) = new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let location = location_for_para(para_id);
        let asset_id = TYPE_FOREIGN | (para_id & 0x0FFF_FFFF);
        let metadata = CurrencyMetadata {
          name: b"Prop Foreign".to_vec(),
          symbol: b"PFRG".to_vec(),
          decimals: 12,
        };
        assert_ok!(crate::Pallet::<Test>::register_foreign_asset_with_id(
          RuntimeOrigin::root(),
          location.clone(),
          asset_id,
          metadata,
          1,
          true,
        ));
        let stored = crate::Pallet::<Test>::location_to_asset(&location);
        let converted = <crate::Pallet<Test> as Convert<Location, Option<u32>>>::convert(location.clone());
        let reverse = crate::Pallet::<Test>::asset_to_location(asset_id);
        let convert_back = <crate::Pallet<Test> as polkadot_sdk::sp_runtime::traits::MaybeEquivalence<Location, u32>>::convert_back(&asset_id);
        (stored, converted, reverse, convert_back)
      });
      let expected = Some(TYPE_FOREIGN | (para_id & 0x0FFF_FFFF));
      prop_assert_eq!(stored, expected);
      prop_assert_eq!(converted, stored);
      prop_assert_eq!(reverse.clone(), Some(location_for_para(para_id)));
      prop_assert_eq!(convert_back, reverse);
    }
  }
}
