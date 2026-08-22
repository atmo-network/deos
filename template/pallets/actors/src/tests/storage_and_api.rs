use super::*;

#[test]
fn public_api_error_signatures_use_shared_typed_cores() {
  let _: fn(ActorId) -> Result<ActorEligibility<u32, u64>, ActorClassificationError> =
    Actors::actor_eligibility;
  let _: fn(
    ActorId,
    ActorType,
    Mutability,
    RuntimeActorContract,
    SimulationMode,
  ) -> Result<crate::SimulationResultOf<Test>, SimulationError> = Actors::simulate_current_contract;

  let classification_cases = [
    (
      ActorClassificationError::ActorInvariant,
      Error::<Test>::ActorInvariant,
    ),
    (
      ActorClassificationError::ContinuationInvariant,
      Error::<Test>::ContinuationInvariant,
    ),
    (
      ActorClassificationError::ComputationOverflow,
      Error::<Test>::ComputationOverflow,
    ),
  ];
  for (core, dispatch) in classification_cases {
    assert_eq!(
      Actors::classification_dispatch_error(core).encode(),
      dispatch.encode()
    );
  }
}

#[test]
fn public_reachability_inventory_is_closed_and_canonical() {
  assert_variant_names::<RuntimeTask>(&[
    "Transfer",
    "SplitTransfer",
    "SwapIn",
    "SwapOut",
    "AddLiquidity",
    "RemoveLiquidity",
    "Burn",
    "Mint",
    "Stake",
    "DonateLiquidity",
    "Unstake",
    "StopCycle",
  ]);
  assert_variant_names::<AmountResolution<u128>>(&[
    "Fixed",
    "PercentageOfCurrent",
    "PercentageAtOpening",
    "PercentageOfLastFunding",
    "AllAvailable",
  ]);
  assert_variant_names::<InputLimit<u128>>(&["LiveQuote", "Absolute"]);
  assert_variant_names::<Predicate<TestAsset, u128, u32, u32>>(&[
    "BalanceAbove",
    "BalanceBelow",
    "BalanceEquals",
    "BalanceNotEquals",
    "BlockNumberAbove",
    "BlockNumberBelow",
    "ObservationAbove",
    "ObservationBelow",
    "ObservationEquals",
    "ObservationNotEquals",
  ]);
  assert_variant_names::<ObservationTiming>(&["Opening", "Current"]);
  assert_variant_names::<crate::PredicateError>(&["InvalidObservation"]);
  assert_variant_names::<RuntimeSourceFilter>(&["Any", "OwnerOnly", "Whitelist"]);
  assert_variant_names::<RuntimeAssetFilter>(&["Any", "Whitelist"]);
  assert_variant_names::<RuntimeTrigger>(&[
    "Manual",
    "AddressEvent",
    "ObservationChange",
    "ObservationCrossing",
    "Cadenced",
  ]);
  assert_variant_names::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "Manual",
      "AddressEvent",
      "ObservationChange",
      "ObservationCrossing",
      "Cadenced",
    ],
  );
  assert_variant_names::<ActorType>(&["User", "System"]);
  assert_variant_names::<ActorClass>(&["User", "System"]);
  assert_variant_names::<Mutability>(&["Mutable", "Immutable"]);
  assert_variant_names::<crate::CompletionPolicy>(&["Persistent", "CloseAfterProductiveCycle"]);
  assert_variant_names::<ActiveLifecycle>(&["Active", "Paused"]);
  assert_variant_names::<CycleState>(&["Idle", "Suspended"]);
  assert_variant_names::<AttemptDisposition>(&["Completed", "Failed", "Suspended", "Closed"]);
  assert_variant_names::<StepOutcome>(&[
    "Executed",
    "Stopped",
    "Skipped",
    "FundingUnavailable",
    "Failed",
  ]);
  assert_variant_names::<OpeningSurface<TestAsset>>(&[
    "PreservableAsset",
    "TargetAsset",
    "StakingShares",
  ]);
  assert_variant_names::<CloseReason>(&[
    "OwnerInitiated",
    "BalanceExhausted",
    "ConsecutiveFailures",
    "WindowExpired",
    "CycleNonceExhausted",
    "FeeBudgetExhausted",
    "AutoCloseNonceReached",
    "RetryAttemptsExhausted",
    "ProductiveCycleCompleted",
    "SchedulerIndexExhausted",
  ]);
  assert_variant_names::<StepErrorPolicy>(&["AbortCycle", "ContinueNextStep", "RetryLater"]);
  assert_variant_names::<SuspensionReason>(&["FundingUnavailable", "Temporary"]);
  assert_variant_names::<CancellationReason>(&[
    "Explicit",
    "ContractReplaced",
    "Deactivated",
    "Closing",
  ]);
  assert_variant_names::<StepSkippedReason>(&[
    "PreconditionFalse",
    "ResolutionSkipped",
    "FundingUnavailable",
  ]);
  assert_variant_names::<FundingSourcePolicy<AccountId, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "OwnerOnly",
      "SignedAllowlist",
      "RuntimePolicy",
      "AnyVerifiedIngress",
    ],
  );
  assert_variant_names::<crate::FundingProvenance>(&["Signed", "InternalProtocol", "Xcm"]);
  assert_variant_names::<RetryClass>(&["Permanent", "Temporary"]);
  assert_variant_names::<crate::ScalarObservationState<u64>>(&[
    "Unavailable",
    "Uninitialized",
    "Fresh",
    "Stale",
  ]);
  assert_variant_names::<ActorEligibility<u32, u64>>(&["NotRegistered", "Dormant", "Active"]);
  assert_variant_names::<SimulationMode>(&["FreshCurrentPlan", "CurrentContinuation"]);
  assert_variant_names::<SimulationError>(&[
    "TransactionDepthExceeded",
    "Classification",
    "ActorNotFound",
    "TypeMismatch",
    "MutabilityMismatch",
    "InvalidContract",
    "ContractMismatch",
    "ModeCycleStateMismatch",
    "GlobalCircuitBreaker",
    "Paused",
    "NotReady",
    "FeeCollectionFailed",
  ]);
}

#[test]
fn actor_storage_schema_is_explicit() {
  let storage_info = Actors::storage_info();
  assert!(
    storage_info
      .iter()
      .all(|entry| entry.pallet_name == b"Actors")
  );
  let actual: alloc::vec::Vec<_> = storage_info
    .iter()
    .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
    .collect();
  assert_eq!(
    actual,
    [
      "NextActorId",
      "ActorHot",
      "ActorContract",
      "ActorFunding",
      "ContinuationState",
      "ActorIdentities",
      "ActorIdentityCount",
      "ActiveActorCount",
      "SystemSovereigns",
      "SystemSovereignCount",
      "NextQueueTicket",
      "QueueHead",
      "QueueTail",
      "QueueOccupancy",
      "QueuePages",
      "WakeupPages",
      "WakeupBuckets",
      "WakeupCursorPages",
      "WakeupCursorLen",
      "NextWakeupClock",
      "WakeupWorkerFaultState",
      "OwnerSlotBitmaps",
      "SovereignIndex",
      "ActiveActorLimit",
      "ActorObservationFeeds",
      "ObservationSubscriptionSlot",
      "ObservationSubscriptionSlotOwner",
      "NextObservationSubscriptionSlot",
      "ObservationFreeSlotLen",
      "ObservationFreeSlotPages",
      "ObservationSubscriberPages",
      "ObservationSubscriberPageLists",
      "ObservationSubscriberCount",
      "ObservationSubscriptionCount",
      "ObservationIngressRevisions",
      "DirtyObservationFeeds",
      "DirtyObservationListState",
      "ObservationFanoutWorkerFaultState",
      "CrossingMemberships",
      "CrossingMemberPages",
      "CrossingLeafStates",
      "CrossingRadixNodes",
      "CrossingFeedMembershipCount",
      "CrossingUserFeedMembershipCount",
      "CrossingTransitionQueues",
      "CrossingPendingFeeds",
      "CrossingPendingFeedListState",
      "CrossingRangeCursors",
      "CrossingWorkerFaultState",
      "MaterializationFamilyCursor",
      "GlobalCircuitBreaker",
      "IdleStarvationState",
    ]
  );

  let metadata = Actors::storage_metadata();
  assert_eq!(metadata.prefix, "Actors");
  let actual_shapes: alloc::vec::Vec<_> = metadata
    .entries
    .iter()
    .map(|entry| {
      let optional = matches!(entry.modifier, StorageEntryModifierIR::Optional);
      let is_blake_map = match &entry.ty {
        StorageEntryTypeIR::Plain(_) => false,
        StorageEntryTypeIR::Map { hashers, .. } => {
          assert!(
            hashers
              .iter()
              .all(|hasher| *hasher == StorageHasherIR::Blake2_128Concat)
          );
          true
        }
      };
      (entry.name, optional, is_blake_map)
    })
    .collect();
  assert_eq!(
    actual_shapes,
    [
      ("NextActorId", false, false),
      ("ActorHot", true, true),
      ("ActorContract", true, true),
      ("ActorFunding", true, true),
      ("ContinuationState", true, true),
      ("ActorIdentities", true, true),
      ("ActorIdentityCount", false, false),
      ("ActiveActorCount", false, false),
      ("SystemSovereigns", true, true),
      ("SystemSovereignCount", false, false),
      ("NextQueueTicket", false, false),
      ("QueueHead", false, false),
      ("QueueTail", false, false),
      ("QueueOccupancy", false, false),
      ("QueuePages", true, true),
      ("WakeupPages", true, true),
      ("WakeupBuckets", true, true),
      ("WakeupCursorPages", true, true),
      ("WakeupCursorLen", false, true),
      ("NextWakeupClock", false, false),
      ("WakeupWorkerFaultState", true, false),
      ("OwnerSlotBitmaps", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("ActorObservationFeeds", true, true),
      ("ObservationSubscriptionSlot", true, true),
      ("ObservationSubscriptionSlotOwner", true, true),
      ("NextObservationSubscriptionSlot", false, false),
      ("ObservationFreeSlotLen", false, false),
      ("ObservationFreeSlotPages", true, true),
      ("ObservationSubscriberPages", true, true),
      ("ObservationSubscriberPageLists", true, true),
      ("ObservationSubscriberCount", false, true),
      ("ObservationSubscriptionCount", false, false),
      ("ObservationIngressRevisions", true, true),
      ("DirtyObservationFeeds", true, true),
      ("DirtyObservationListState", false, false),
      ("ObservationFanoutWorkerFaultState", true, false),
      ("CrossingMemberships", true, true),
      ("CrossingMemberPages", true, true),
      ("CrossingLeafStates", true, true),
      ("CrossingRadixNodes", true, true),
      ("CrossingFeedMembershipCount", false, true),
      ("CrossingUserFeedMembershipCount", false, true),
      ("CrossingTransitionQueues", true, true),
      ("CrossingPendingFeeds", true, true),
      ("CrossingPendingFeedListState", false, false),
      ("CrossingRangeCursors", true, true),
      ("CrossingWorkerFaultState", true, false),
      ("MaterializationFamilyCursor", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("IdleStarvationState", false, false),
    ]
  );

  let entries = &metadata.entries;
  assert_plain_storage_type::<u64>(&entries[0]);
  assert_map_storage_types::<u64, crate::ActorHotStateOf<Test>>(&entries[1]);
  assert_map_storage_types::<u64, crate::ActorContractOf<Test>>(&entries[2]);
  assert_map_storage_types::<u64, crate::ActorFundingStateOf<Test>>(&entries[3]);

  let mut registry = scale_info::Registry::new();
  let contract_type =
    registry.register_type(&scale_info::meta_type::<crate::ActorContractOf<Test>>());
  let (_, contract) = registry
    .types()
    .find(|(symbol, _)| symbol.id == contract_type.id)
    .expect("Actor Contract type is registered");
  let scale_info::TypeDef::Composite(contract_fields) = &contract.type_def else {
    panic!("Actor Contract metadata must be composite");
  };
  assert_eq!(
    contract_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor Contract field"))
      .collect::<Vec<_>>(),
    [
      "trigger",
      "cooldown_blocks",
      "window",
      "steps",
      "funding",
      "completion",
      "auto_close_at_cycle_nonce"
    ]
  );
  assert_map_storage_types::<u64, RuntimeContinuationState>(&entries[4]);
  assert_map_storage_types::<u64, crate::ActorIdentityOf<Test>>(&entries[5]);
  assert_plain_storage_type::<u32>(&entries[6]);
  assert_plain_storage_type::<u32>(&entries[7]);
  assert_map_storage_types::<u64, SystemSovereignState>(&entries[8]);
  assert_plain_storage_type::<u32>(&entries[9]);
  assert_plain_storage_type::<u64>(&entries[10]);
  assert_plain_storage_type::<u64>(&entries[11]);
  assert_plain_storage_type::<u64>(&entries[12]);
  assert_plain_storage_type::<u32>(&entries[13]);
  assert_map_storage_types::<u64, crate::QueuePageOf<Test>>(&entries[14]);
  assert_map_storage_types::<(WakeupKey<MockBlockNumber>, u64), crate::WakeupPageOf<Test>>(
    &entries[15],
  );
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, WakeupBucketState>(&entries[16]);
  assert_map_storage_types::<(WakeupClock, u64), crate::WakeupCursorPageOf<Test>>(&entries[17]);
  assert_map_storage_types::<WakeupClock, u32>(&entries[18]);
  assert_plain_storage_type::<WakeupClock>(&entries[19]);
  assert_plain_storage_type::<crate::WakeupWorkerFault<MockBlockNumber>>(&entries[20]);
  assert_map_storage_types::<AccountId, [u8; 32]>(&entries[21]);
  assert_map_storage_types::<AccountId, u64>(&entries[22]);
  assert_plain_storage_type::<u32>(&entries[23]);
  assert_map_storage_types::<u64, crate::ActorObservationFeedsOf<Test>>(&entries[24]);
  assert_map_storage_types::<u64, u32>(&entries[25]);
  assert_map_storage_types::<u32, u64>(&entries[26]);
  assert_plain_storage_type::<u32>(&entries[27]);
  assert_plain_storage_type::<u32>(&entries[28]);
  assert_map_storage_types::<u32, crate::ObservationFreeSlotPageOf<Test>>(&entries[29]);
  assert_map_storage_types::<(u32, u32), crate::ObservationSubscriberPageOf<Test>>(&entries[30]);
  assert_map_storage_types::<u32, ObservationSubscriberPageList>(&entries[31]);
  assert_map_storage_types::<u32, u32>(&entries[32]);
  assert_plain_storage_type::<u32>(&entries[33]);
  assert_map_storage_types::<u32, u64>(&entries[34]);
  assert_map_storage_types::<
    u32,
    crate::types::DirtyObservationState<
      u32,
      polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>,
    >,
  >(&entries[35]);
  assert_plain_storage_type::<crate::types::DirtyObservationList<u32>>(&entries[36]);
  assert_plain_storage_type::<crate::ObservationFanoutWorkerFault<u32>>(&entries[37]);
  assert_map_storage_types::<u64, crate::CrossingMembershipLocatorOf<Test>>(&entries[38]);
  assert_map_storage_types::<
    (crate::CrossingLeafKeyOf<Test>, u32),
    crate::CrossingMemberPageOf<Test>,
  >(&entries[39]);
  assert_map_storage_types::<crate::CrossingLeafKeyOf<Test>, crate::CrossingLeafState>(
    &entries[40],
  );
  assert_map_storage_types::<crate::CrossingRadixNodeKeyOf<Test>, u16>(&entries[41]);
  assert_map_storage_types::<u32, u32>(&entries[42]);
  assert_map_storage_types::<u32, u32>(&entries[43]);
  assert_map_storage_types::<u32, crate::CrossingTransitionQueueOf<Test>>(&entries[44]);
  assert_map_storage_types::<u32, crate::CrossingPendingFeedState<u32>>(&entries[45]);
  assert_plain_storage_type::<crate::CrossingPendingFeedList<u32>>(&entries[46]);
  assert_map_storage_types::<u32, crate::CrossingRangeCursor>(&entries[47]);
  assert_plain_storage_type::<crate::CrossingWorkerFault<u32>>(&entries[48]);
  assert_plain_storage_type::<u8>(&entries[49]);
  assert_plain_storage_type::<bool>(&entries[50]);
  assert_plain_storage_type::<IdleStarvationPhase>(&entries[51]);
}

#[test]
fn fresh_genesis_baseline_carries_no_migration_ceremony() {
  new_test_ext().execute_with(|| {
    // Genesis writes the current storage version directly; no legacy reader,
    // dual write, queue-merge bridge, or migration cursor exists (COMPAT-STORAGE).
    use polkadot_sdk::frame_support::traits::GetStorageVersion;
    let on_chain =
      polkadot_sdk::frame_support::traits::StorageVersion::get::<crate::Pallet<Test>>();
    let in_code = crate::Pallet::<Test>::in_code_storage_version();
    assert_eq!(
      on_chain, in_code,
      "genesis baseline must equal the current storage version"
    );
    // No queue-merge or migration-cursor storage exists on the schema surface.
    let storage_info = Actors::storage_info();
    let names: alloc::vec::Vec<_> = storage_info
      .iter()
      .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("UTF-8"))
      .collect();
    assert!(
      names.iter().all(|name| {
        !name.starts_with("Legacy")
          && !name.starts_with("Migration")
          && !name.contains("Merge")
          && !name.contains("OnRuntimeUpgrade")
      }),
      "fresh baseline must not carry migration-ceremony storage: {names:?}"
    );
    // The embedding fixture independently starts from the same fresh schema.
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn owner_slot_bitmap_try_state_rejects_invalid_and_orphaned_bits() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] |= 0b1000_0000);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] &= 0b0111_1111);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::insert(CHARLIE, [1; 32]);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn live_head_consume_rolls_back_on_occupancy_or_span_corruption() {
  for (tail, occupancy) in [(1u64, 0u32), (2u64, 1u32)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      QueueTail::<Test>::put(tail);
      QueueOccupancy::<Test>::put(occupancy);
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::paged_consume_head_at(0),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(
        Actors::actor_hot(actor_id).expect("hot").queue_ticket,
        Some(0)
      );
    });
  }
}

#[test]
fn reverse_index_corruption_matrix_fails_closed_for_system_close() {
  for dormant in [false, true] {
    for corruption in 0u8..5 {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
        if dormant {
          assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
          frame_system::Pallet::<Test>::set_block_number(2);
        }
        let identity = Actors::actor_identities(actor_id).expect("system identity");
        let sovereign_id = match identity.actor_class {
          ActorClass::System { sovereign_id } => sovereign_id,
          _ => unreachable!(),
        };
        match corruption {
          0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
          1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
          2 => crate::SystemSovereigns::<Test>::remove(sovereign_id),
          3 => crate::SystemSovereigns::<Test>::insert(sovereign_id, SystemSovereignState::Vacant),
          4 => crate::SystemSovereigns::<Test>::insert(
            sovereign_id,
            SystemSovereignState::Occupied(other),
          ),
          _ => unreachable!(),
        }
        let events_before = System::events();
        let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

        assert!(Actors::close_actor(RuntimeOrigin::root(), actor_id).is_err());
        assert_eq!(System::events(), events_before);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(StateVersion::V1),
          root_before,
          "dormant={dormant}, corruption={corruption}"
        );
        #[cfg(feature = "try-runtime")]
        assert!(
          crate::Pallet::<Test>::do_try_state().is_err(),
          "dormant={dormant}, corruption={corruption}"
        );
      });
    }
  }
}

#[test]
fn reverse_index_corruption_matrix_fails_closed_for_user_close() {
  for corruption in 0u8..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let other = create_user_with(
        BOB,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let identity = Actors::actor_identities(actor_id).expect("user identity");
      match corruption {
        0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
        1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
        2 => OwnerSlotBitmaps::<Test>::remove(ALICE),
        _ => unreachable!(),
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id).is_err());
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before,
        "corruption={corruption}"
      );
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}

#[test]
fn system_locator_corruption_surfaces_one_invariant_error_on_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    // Corrupt the locator truth: the live actor's entry no longer points at it.
    crate::SystemSovereigns::<Test>::insert(actor_id, SystemSovereignState::Vacant);
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::SystemSovereignInvariant
    );
  });
}

#[test]
fn temporal_membership_try_state_rejects_terminal_at_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102)
    );
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Terminal membership is derived from the schedule window: any `terminal_at` that is not
      // the exact window terminal (or absent without a window) must fail try_state.
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(999);
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorHot terminal_at disagrees with schedule window terminal membership\")"
            .into()
        )
      );
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = None;
      });
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(102);
      });
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_rejects_page_slot_pointing_at_different_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    assert!(Actors::wakeup_substrate_schedule(other, 10));
    // A physical slot whose entry addresses an actor that owns a different pointer is
    // corruption: `wakeup_pointer` is the sole ordinary temporal-membership authority.
    WakeupPages::<Test>::mutate((WakeupKey::Block(10), 0), |maybe| {
      let page = maybe.as_mut().expect("wakeup page");
      page.entries[0] = Some(crate::WakeupEntry { actor_id: other });
    });
    #[cfg(feature = "try-runtime")]
    assert_eq!(
      crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
      Err("Other(\"WakeupPage slot addresses an actor with a different wakeup pointer\")".into())
    );
  });
}

#[test]
fn missing_frozen_snapshot_is_a_permanent_invariant_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(8);
    let asset_out = TestAsset::Local(9);
    setup_pool(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      percentage_trigger_schedule(),
      None,
      contract_steps_with_step(step),
    );
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 100);
    set_temporary_dex_failure(true);
    signal_percentage_trigger(actor_id, asset_in);
    run_idle(Weight::MAX);
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("suspended continuation")
        .opening_snapshot
        .clear();
    });

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    let actor_state =
      Actors::active_actor_view(actor_id).expect("actor remains after permanent failure");
    assert_eq!(actor_state.cycle_state, CycleState::Idle);
    assert_eq!(actor_state.unsuccessful_attempt_streak, 2);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        step_index: 0,
        error,
        ..
      } if *id == actor_id && *error == Error::<Test>::SnapshotUnavailable.into()
    )));
  });
}

#[test]
fn canonical_loader_distinguishes_absence_dormancy_active_and_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let missing_id = Actors::next_actor_id();
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::NotRegistered
    ));

    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::Dormant(_)
    ));

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("active actor").pending_signal = true;
    });
    assert!(Actors::pending_signal(actor_id));
    ActorFunding::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_noop!(
      Actors::write_continuation_state(actor_id, None),
      Error::<Test>::ActorInvariant
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );

    ActorIdentities::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn canonical_loader_classifies_every_four_partition_presence_mask() {
  for mask in 0u8..16 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let identity = ActorIdentities::<Test>::take(actor_id).expect("identity fixture");
      let hot = ActorHot::<Test>::take(actor_id).expect("hot fixture");
      let contract = ActorContracts::<Test>::take(actor_id).expect("contract fixture");
      let funding = ActorFunding::<Test>::take(actor_id).expect("funding fixture");
      ContinuationStateStore::<Test>::remove(actor_id);
      if mask & 0b0001 != 0 {
        ActorIdentities::<Test>::insert(actor_id, identity);
      }
      if mask & 0b0010 != 0 {
        ActorHot::<Test>::insert(actor_id, hot);
      }
      if mask & 0b0100 != 0 {
        ActorContracts::<Test>::insert(actor_id, contract);
      }
      if mask & 0b1000 != 0 {
        ActorFunding::<Test>::insert(actor_id, funding);
      }

      match mask {
        0 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        )),
        1 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::Dormant(_)
        )),
        15 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::Active(_)
        )),
        _ => {
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          assert_eq!(
            Actors::actor_eligibility(actor_id),
            Err(ActorClassificationError::ActorInvariant)
          );
          assert_noop!(
            Actors::pause_actor(RuntimeOrigin::root(), actor_id),
            Error::<Test>::ActorInvariant
          );
          assert_noop!(
            Actors::preflight_funding_event(actor_id, TestAsset::Native, 1, None, None),
            Error::<Test>::ActorInvariant
          );
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
        }
      }
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_every_corrupt_four_partition_presence_mask() {
  for mask in 2u8..15 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let identity = ActorIdentities::<Test>::take(actor_id).expect("identity fixture");
      let hot = ActorHot::<Test>::take(actor_id).expect("hot fixture");
      let contract = ActorContracts::<Test>::take(actor_id).expect("contract fixture");
      let funding = ActorFunding::<Test>::take(actor_id).expect("funding fixture");
      if mask & 0b0001 != 0 {
        ActorIdentities::<Test>::insert(actor_id, identity);
      }
      if mask & 0b0010 != 0 {
        ActorHot::<Test>::insert(actor_id, hot);
      }
      if mask & 0b0100 != 0 {
        ActorContracts::<Test>::insert(actor_id, contract);
      }
      if mask & 0b1000 != 0 {
        ActorFunding::<Test>::insert(actor_id, funding);
      }
      crate::ActorIdentityCount::<Test>::put(u32::from(mask & 0b0001 != 0));
      crate::ActiveActorCount::<Test>::put(u32::from(mask & 0b0010 != 0));
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "mask {mask:04b}"
      );
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_system_identity_with_noncanonical_derived_account() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let original = ActorIdentities::<Test>::get(actor_id).expect("System identity fixture");
    let replacement_account = 999_998;
    crate::SovereignIndex::<Test>::remove(original.sovereign_account);
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("System identity fixture")
        .sovereign_account = replacement_account;
    });
    crate::SovereignIndex::<Test>::insert(replacement_account, actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn eligibility_projection_rejects_partial_active_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ActorFunding::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
  });
}
