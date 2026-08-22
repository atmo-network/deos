use super::*;

#[test]
fn observation_only_sources_admit_non_trigger_amount_resolutions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, observation_schedule(vec![4]), None, plan);
    assert_eq!(Actors::observation_subscriber_count(4), 1);
    assert!(Actors::actor_contract(actor_id).is_some());
  });
}

#[test]
fn observation_subscriptions_follow_schedule_lifecycle_exactly() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      observation_schedule(vec![1]),
      None,
      inert_contract_steps(),
    );
    let slot = Actors::observation_subscription_slot(actor_id).expect("subscription slot");
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![1]))
    );
    assert_eq!(Actors::observation_subscription_count(), 1);
    assert_eq!(Actors::observation_subscriber_count(1), 1);
    assert_eq!(Actors::observation_ingress_revision(1), None);
    assert_eq!(Actors::observation_ingress_revision(2), None);
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert!(Actors::dirty_observation_feeds(2).is_none());
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_id = slot / page_size;
    let offset = (slot % page_size) as usize;
    assert_eq!(
      Actors::observation_subscriber_pages(1, page_id).expect("subscriber page")[offset],
      Some(actor_id)
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      observation_schedule(vec![3]),
      None,
    ));
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![3]))
    );
    assert_eq!(Actors::observation_subscriber_count(1), 0);
    assert_eq!(Actors::observation_subscriber_count(3), 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      transfer_contract_steps(BOB, 1),
      crate::CompletionPolicy::Persistent,
    ));
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![3]))
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::actor_observation_feeds(actor_id).is_none());
    assert!(Actors::observation_subscription_slot(actor_id).is_none());
    assert_eq!(Actors::observation_subscription_count(), 0);
    assert_eq!(crate::ObservationFreeSlotLen::<Test>::get(), 1);
    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      {
        let schedule = observation_schedule(vec![4]);
        ActorContract {
          trigger: schedule.trigger,
          cooldown_blocks: schedule.cooldown_blocks,
          window: None,
          steps: inert_contract_steps(),
          completion: crate::CompletionPolicy::Persistent,
          funding: FundingSourcePolicy::OwnerOnly,
          auto_close_at_cycle_nonce: None,
        }
      },
    ));
    assert_eq!(Actors::observation_subscription_slot(actor_id), Some(slot));
    assert_eq!(crate::ObservationFreeSlotLen::<Test>::get(), 0);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::observation_subscription_count(), 0);
    assert_eq!(Actors::observation_subscriber_count(4), 0);
    assert!(Actors::observation_subscriber_pages(4, page_id).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn resumed_observation_fanout_preserves_the_per_block_page_cap() {
  new_test_ext().execute_with(|| {
    let cap = <Test as crate::Config>::MaxObservationFanoutPagesPerBlock::get();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::fanout_dirty_observations_with_pages(Weight::MAX, cap),
      (Weight::zero(), cap)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before,
      "a resumed family at its component cap must not probe or mutate again"
    );
  });
}

#[test]
fn observation_occupied_page_list_follows_live_pages_after_fragmentation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = <Test as crate::Config>::ObservationPageSize::get();
    let mut actors = Vec::new();
    for _ in 0..=page_size {
      actors.push(create_system_with(
        ALICE,
        observation_schedule(vec![17]),
        None,
        inert_contract_steps(),
      ));
    }

    assert_eq!(
      Actors::observation_subscriber_page_list(17),
      Some(ObservationSubscriberPageList {
        head: 0,
        tail: 1,
        count: 2,
      })
    );
    let first = Actors::observation_subscriber_pages(17, 0).expect("first occupied page");
    let second = Actors::observation_subscriber_pages(17, 1).expect("second occupied page");
    assert_eq!((first.previous, first.next), (None, Some(1)));
    assert_eq!((second.previous, second.next), (Some(0), None));

    let remaining_actor = *actors.last().expect("one actor remains");
    for actor_id in actors.iter().copied().take(page_size as usize) {
      assert_ok!(Actors::deactivate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }

    assert!(Actors::observation_subscriber_pages(17, 0).is_none());
    assert_eq!(
      Actors::observation_subscriber_page_list(17),
      Some(ObservationSubscriberPageList {
        head: 1,
        tail: 1,
        count: 1,
      })
    );
    let remaining = Actors::observation_subscriber_pages(17, 1).expect("remaining occupied page");
    assert_eq!((remaining.previous, remaining.next), (None, None));

    assert_ok!(Actors::note_observation_changed(17, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    assert_eq!(
      Actors::fanout_dirty_observations(base.saturating_add(unit)),
      base.saturating_add(unit)
    );
    assert!(Actors::dirty_observation_feeds(17).is_none());
    assert!(Actors::pending_signal(remaining_actor));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn observation_provider_mutation_without_certified_ingress_has_no_actor_effect() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 1;
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![feed]),
      None,
      inert_contract_steps(),
    );

    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );

    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::observation_ingress_revision(feed), None);
    assert!(Actors::dirty_observation_feeds(feed).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
  });
}

#[test]
fn stale_observation_subscriber_page_fails_closed_without_losing_dirty_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![19]),
      None,
      inert_contract_steps(),
    );
    let slot = Actors::observation_subscription_slot(actor_id).expect("subscription slot");
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_id = slot / page_size;
    assert_ok!(Actors::note_observation_changed(19, 1));
    crate::ObservationSubscriberPages::<Test>::remove(19, page_id);
    let dirty_before = Actors::dirty_observation_feeds(19).expect("dirty feed");
    let list_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_eq!(
      crate::Pallet::<Test>::do_fanout_dirty_observation_page(),
      Err(Error::<Test>::DirtyObservationInvariant.into())
    );
    assert_eq!(Actors::dirty_observation_feeds(19), Some(dirty_before));
    assert_eq!(Actors::dirty_observation_list(), list_before);
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn observation_change_ingress_coalesces_latest_revision_without_subscriber_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::note_observation_changed(1, 0),
      Error::<Test>::InvalidObservationRevision
    );
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::observation_ingress_revision(1), None);
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());

    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![1]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::observation_ingress_revision(1), Some(1));
    assert!(!Actors::pending_signal(actor_id));
    let initial = Actors::dirty_observation_feeds(1).expect("dirty feed");
    assert_eq!(initial.latest_revision, 1);
    assert_eq!(initial.fanout_revision, 0);
    assert_eq!(initial.dirty_since, 1);
    assert_eq!(initial.next_subscriber_page, None);
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::dirty_observation_feeds(1), Some(initial));
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::note_observation_changed(1, 3));
    assert_eq!(Actors::observation_ingress_revision(1), Some(3));
    let coalesced = Actors::dirty_observation_feeds(1).expect("coalesced dirty feed");
    assert_eq!(coalesced.latest_revision, 3);
    assert_eq!(coalesced.dirty_since, 1);
    assert_noop!(
      Actors::note_observation_changed(1, 2),
      Error::<Test>::InvalidObservationRevision
    );
    assert_eq!(Actors::dirty_observation_feed_count(), 1);
    assert_eq!(Actors::dirty_observation_list().head, Some(1));
    assert_eq!(Actors::dirty_observation_list().tail, Some(1));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(1));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    assert_ok!(crate::Pallet::<Test>::clear_dirty_observation_feed(1));
    assert_eq!(Actors::observation_ingress_revision(1), Some(3));
    assert_noop!(
      Actors::note_observation_changed(1, 2),
      Error::<Test>::InvalidObservationRevision
    );
    frame_system::Pallet::<Test>::set_block_number(13);
    assert_ok!(Actors::note_observation_changed(1, 4));
    assert_eq!(Actors::observation_ingress_revision(1), Some(4));
    assert_eq!(
      Actors::dirty_observation_feeds(1)
        .expect("new dirty interval")
        .dirty_since,
      13
    );
  });
}

#[test]
fn last_subscription_cleanup_unlinks_exact_dirty_feed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    let second = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(7, 1));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      first
    ));
    assert_eq!(Actors::observation_subscriber_count(7), 1);
    assert!(Actors::dirty_observation_feeds(7).is_some());
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      second
    ));
    assert_eq!(Actors::observation_subscriber_count(7), 0);
    assert_eq!(Actors::observation_ingress_revision(7), None);
    assert!(Actors::dirty_observation_feeds(7).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());

    create_system_with(
      ALICE,
      observation_schedule(vec![8]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(8, 4));
    assert_eq!(Actors::dirty_observation_list().head, Some(8));
    assert_eq!(Actors::dirty_observation_list().tail, Some(8));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(8));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn same_block_wakeup_precedes_fanout_in_ticket_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A timer actor with a due wakeup and an observation subscriber with a dirty feed
    // in the same block: the on_idle phase order (wakeups before fanout) must give the
    // wakeup-eligible actor a strictly earlier queue ticket than the fanout-signaled
    // subscriber (spec 8.2.1). We observe this through the execution order of the two
    // one-shot transfers: the wakeup actor's transfer must precede the fanout actor's.
    let wakeup_id = create_system_with(
      ALICE,
      timer_schedule(3),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(wakeup_id, 1_000);
    let subscriber_id = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      transfer_contract_steps(CHARLIE, 10),
    );
    fund_native(subscriber_id, 1_000);
    // The timer's first wakeup fires at block 4 (anchor 1 + 3); the observation change
    // lands at block 4 too, so both are due in the same on_idle pass.
    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::note_observation_changed(7, 1));
    assert_eq!(scheduled_wakeup_block(wakeup_id), Some(4));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let wakeup_pos = events
      .iter()
      .position(|event| matches!(
        event,
        Event::TransferExecuted { actor_id: id, to, .. } if *id == wakeup_id && *to == BOB
      ))
      .expect("wakeup actor transfer executed");
    let fanout_pos = events
      .iter()
      .position(|event| matches!(
        event,
        Event::TransferExecuted { actor_id: id, to, .. } if *id == subscriber_id && *to == CHARLIE
      ))
      .expect("fanout actor transfer executed");
    assert!(
      wakeup_pos < fanout_pos,
      "wakeup-enqueued actor must execute before fanout-enqueued actor: wakeup={wakeup_pos}, fanout={fanout_pos}"
    );
  });
}

#[test]
fn subscription_cleanup_failure_rolls_back_actor_deactivation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![18]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(18, 1));
    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = None);
    let actor_before = Actors::actor_hot(actor_id).expect("active actor");
    let dirty_before = Actors::dirty_observation_feeds(18).expect("dirty feed");
    let list_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::DirtyObservationInvariant
    );
    assert_eq!(Actors::actor_hot(actor_id), Some(actor_before));
    assert_eq!(Actors::observation_subscriber_count(18), 1);
    assert_eq!(Actors::dirty_observation_feeds(18), Some(dirty_before));
    assert_eq!(Actors::dirty_observation_list(), list_before);
    assert_eq!(System::events(), events_before);

    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = Some(18));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::actor_hot(actor_id).is_none());
    assert_eq!(Actors::observation_subscriber_count(18), 0);
    assert!(Actors::dirty_observation_feeds(18).is_none());
    assert_eq!(Actors::dirty_observation_list(), Default::default());
  });
}

#[test]
fn multiple_dense_dirty_feeds_receive_round_robin_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let feeds = [21u32, 22, 23];
    let actors = feeds
      .into_iter()
      .map(|feed| {
        (0..=page_size)
          .map(|_| {
            create_system_with(
              ALICE,
              observation_schedule(vec![feed]),
              None,
              inert_contract_steps(),
            )
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    for feed in feeds {
      assert_ok!(Actors::note_observation_changed(feed, 1));
    }
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);

    for (index, feed) in feeds.into_iter().enumerate() {
      assert_eq!(Actors::dirty_observation_list().cursor, Some(feed));
      assert_eq!(Actors::fanout_dirty_observations(budget), budget);
      let delivered = actors[index]
        .iter()
        .filter(|actor_id| Actors::pending_signal(**actor_id))
        .count();
      assert!(delivered > 0);
      assert!(delivered < actors[index].len());
    }
    assert_eq!(Actors::dirty_observation_list().cursor, Some(feeds[0]));

    for _ in 0..12 {
      if Actors::dirty_observation_feed_count() == 0 {
        break;
      }
      Actors::fanout_dirty_observations(budget);
    }
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    let tickets = actors
      .iter()
      .flatten()
      .map(|actor_id| {
        let hot = Actors::actor_hot(*actor_id).expect("dense-feed actor");
        assert!(hot.pending_signal);
        hot.queue_ticket.expect("dense-feed actor queued")
      })
      .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(tickets.len(), feeds.len() * (page_size as usize + 1));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn dirty_feed_capacity_failure_rolls_back_list_insertion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_system_with(
      ALICE,
      observation_schedule(vec![9]),
      None,
      inert_contract_steps(),
    );
    let maximum: u32 = <Test as crate::Config>::MaxActiveActors::get();
    crate::DirtyObservationListState::<Test>::put(crate::types::DirtyObservationList {
      count: maximum,
      ..Default::default()
    });
    assert_noop!(
      Actors::note_observation_changed(9, 1),
      Error::<Test>::DirtyObservationCapacityExceeded
    );
    assert!(Actors::dirty_observation_feeds(9).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), maximum);
  });
}

#[test]
fn fanout_requires_complete_ref_time_and_proof_size_before_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![10]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(10, 1));
    let before = Actors::dirty_observation_feeds(10).expect("dirty feed");
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let required = base.saturating_add(unit);

    let ref_time_short = Weight::from_parts(required.ref_time().saturating_sub(1), u64::MAX);
    assert_eq!(Actors::fanout_dirty_observations(ref_time_short), base);
    assert_eq!(Actors::dirty_observation_feeds(10), Some(before));
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(10));

    let proof_short = Weight::from_parts(u64::MAX, required.proof_size().saturating_sub(1));
    assert_eq!(Actors::fanout_dirty_observations(proof_short), base);
    assert_eq!(Actors::dirty_observation_feeds(10), Some(before));
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(10));
    assert!(Actors::observation_fanout_worker_fault().is_none());
  });
}

#[test]
fn fanout_structural_fault_is_bounded_and_requires_repair_before_resume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![10]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(10, 1));
    let page = crate::ObservationSubscriberPages::<Test>::take(10, 0).expect("subscriber page");
    let dirty_before = Actors::dirty_observation_feeds(10).expect("dirty feed");
    Actors::fanout_dirty_observations(Weight::MAX);
    assert_eq!(Actors::dirty_observation_feeds(10), Some(dirty_before));
    assert_eq!(
      Actors::observation_fanout_worker_fault(),
      Some(crate::ObservationFanoutWorkerFault {
        feed: 10,
        revision: 1,
        subscriber_page: None,
        class: crate::CrossingWorkerFaultClass::Invariant,
      })
    );
    assert_eq!(
      Actors::fanout_dirty_observations(Weight::MAX),
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base()
    );
    assert_noop!(
      Actors::clear_observation_fanout_worker_fault(RuntimeOrigin::signed(ALICE)),
      DispatchError::BadOrigin
    );
    crate::ObservationSubscriberPages::<Test>::insert(10, 0, page);
    assert_ok!(Actors::clear_observation_fanout_worker_fault(
      RuntimeOrigin::root()
    ));
    Actors::fanout_dirty_observations(Weight::MAX);
    assert!(Actors::observation_fanout_worker_fault().is_none());
    assert!(Actors::dirty_observation_feeds(10).is_none());
    assert!(Actors::pending_signal(actor_id));
  });
}

#[test]
fn one_fanout_page_sets_existing_latches_and_scheduler_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = (0..3)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![11]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(11, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    assert_eq!(
      Actors::fanout_dirty_observations(base.saturating_add(unit)),
      base.saturating_add(unit)
    );
    for actor_id in actors {
      let hot = Actors::actor_hot(actor_id).expect("active actor");
      assert!(hot.pending_signal);
      assert!(hot.queue_ticket.is_some());
    }
    assert!(Actors::dirty_observation_feeds(11).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn saturated_queue_materializes_fanout_through_the_canonical_deferred_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![16]),
      None,
      inert_contract_steps(),
    );
    let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
    let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first));
      let entries = (0..len)
        .map(|offset| QueueEntry {
          ticket: u64::from(first.saturating_add(offset)),
          actor_id: 20_000_000u64.saturating_add(u64::from(first.saturating_add(offset))),
        })
        .collect::<Vec<_>>();
      QueuePages::<Test>::insert(
        u64::from(page_id),
        BoundedVec::try_from(entries).expect("saturated queue page fits"),
      );
    }
    QueueHead::<Test>::put(0);
    QueueTail::<Test>::put(u64::from(capacity));
    QueueOccupancy::<Test>::put(capacity);
    crate::NextQueueTicket::<Test>::put(u64::from(capacity));
    assert_ok!(Actors::note_observation_changed(16, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);

    Actors::fanout_dirty_observations(budget);
    assert!(Actors::dirty_observation_feeds(16).is_none());
    assert!(Actors::pending_signal(actor_id));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor")
        .queue_ticket
        .is_none()
    );

    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
  });
}

#[test]
fn on_idle_fanout_feeds_the_existing_scheduler_without_direct_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![14]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(14, 1));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("identity")
        .cycle_nonce,
      0
    );
    assert!(!Actors::pending_signal(actor_id));

    let consumed = <Actors as Hooks<MockBlockNumber>>::on_idle(1, Weight::MAX);
    assert_ne!(consumed, Weight::zero());
    assert!(Actors::dirty_observation_feeds(14).is_none());
    let after = Actors::actor_hot(actor_id).expect("actor survives productive cycle");
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("identity")
        .cycle_nonce,
      1
    );
    assert!(!after.pending_signal);
  });
}

#[test]
fn newer_revision_during_fanout_restarts_from_the_first_subscriber_page() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![12]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(12, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let in_progress = Actors::dirty_observation_feeds(12).expect("fanout remains in progress");
    assert_eq!(in_progress.fanout_revision, 1);
    assert_eq!(in_progress.next_subscriber_page, Some(1));
    let first = actors[0];
    let first_ticket = Actors::actor_hot(first).expect("first actor").queue_ticket;
    crate::ActorHot::<Test>::mutate(first, |maybe| {
      maybe.as_mut().expect("first actor").pending_signal = false;
    });

    assert_ok!(Actors::note_observation_changed(12, 2));
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let restarted = Actors::dirty_observation_feeds(12).expect("new revision restarts fanout");
    assert_eq!(restarted.latest_revision, 2);
    assert_eq!(restarted.fanout_revision, 2);
    assert_eq!(restarted.next_subscriber_page, Some(0));
    assert!(!Actors::pending_signal(first));

    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::pending_signal(first));
    assert_eq!(
      Actors::actor_hot(first).expect("first actor").queue_ticket,
      first_ticket
    );
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::dirty_observation_feeds(12).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn latest_revision_fanout_model_converges_across_seeded_races() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..page_size.saturating_mul(2).saturating_add(1))
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![15]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);
    let mut latest_revision = 1u64;
    let mut delivered = vec![0u64; actors.len()];
    let mut seed = 0xDE05_0777u64;
    assert_ok!(Actors::note_observation_changed(15, latest_revision));

    let process_one = |delivered: &mut Vec<u64>| {
      let Some(before) = Actors::dirty_observation_feeds(15) else {
        return false;
      };
      let page_revision = if before.fanout_revision == 0 {
        before.latest_revision
      } else {
        before.fanout_revision
      };
      let page_id = before.next_subscriber_page.unwrap_or_else(|| {
        Actors::observation_subscriber_page_list(15)
          .expect("occupied pages")
          .head
      });
      let start = page_id.saturating_mul(page_size) as usize;
      let end = start
        .saturating_add(page_size as usize)
        .min(delivered.len());
      Actors::fanout_dirty_observations(budget);
      for revision in &mut delivered[start..end] {
        *revision = (*revision).max(page_revision);
      }
      true
    };

    for _step in 0..96u32 {
      seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
      if seed.is_multiple_of(3) {
        latest_revision += 1;
        assert_ok!(Actors::note_observation_changed(15, latest_revision));
      }
      if seed.is_multiple_of(5) {
        let index = (seed as usize) % actors.len();
        crate::ActorHot::<Test>::mutate(actors[index], |maybe| {
          maybe.as_mut().expect("model actor").pending_signal = false;
        });
      }
      process_one(&mut delivered);
    }
    for _ in 0..256 {
      if !process_one(&mut delivered) {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(15).is_none());
    assert!(
      delivered
        .iter()
        .all(|revision| *revision == latest_revision)
    );
    let tickets = actors
      .iter()
      .filter_map(|actor_id| Actors::actor_hot(*actor_id).and_then(|hot| hot.queue_ticket))
      .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(tickets.len(), actors.len());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn partial_fanout_page_then_deactivation_reconciles_dirty_feed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![30]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(30, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let in_progress = Actors::dirty_observation_feeds(30).expect("first page consumed");
    assert_eq!(in_progress.next_subscriber_page, Some(1));

    // Deactivate the only subscriber on the unvisited second page mid-fanout. This unlinks the
    // page and must adjust the dirty feed's next page instead of leaving a dangling invariant.
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actors[page_size as usize]
    ));
    let repaired = Actors::dirty_observation_feeds(30).expect("dirty feed survives");
    assert!(
      repaired.next_subscriber_page.is_none(),
      "unlinked page must not remain the fanout cursor"
    );

    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::dirty_observation_feeds(30).is_none());
    for actor_id in &actors[..page_size as usize] {
      assert!(
        Actors::pending_signal(*actor_id),
        "first-page subscriber signalled"
      );
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn subscriber_mutation_during_fanout_reconciles_without_invariant_errors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![31]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(31, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert_eq!(
      Actors::dirty_observation_feeds(31)
        .expect("in progress")
        .next_subscriber_page,
      Some(1)
    );

    // Remove the only second-page subscriber mid-fanout via a schedule change away from the feed.
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actors[page_size as usize],
      manual_schedule(),
      None,
    ));
    assert!(
      Actors::dirty_observation_feeds(31).is_some(),
      "dirty feed survives"
    );

    // Re-add a subscriber to the same feed mid-fanout; the fanout must finish without an
    // invariant error and signal the first page. The late subscriber joins delivery on the next
    // revision rather than the in-flight one.
    let late_subscriber = create_system_with(
      ALICE,
      observation_schedule(vec![31]),
      None,
      inert_contract_steps(),
    );
    assert!(!Actors::pending_signal(late_subscriber));

    for _ in 0..4 {
      Actors::fanout_dirty_observations(base.saturating_add(unit));
      if Actors::dirty_observation_feeds(31).is_none() {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(31).is_none());
    for actor_id in &actors[..page_size as usize] {
      assert!(
        Actors::pending_signal(*actor_id),
        "first-page subscriber signalled"
      );
    }

    assert_ok!(Actors::note_observation_changed(31, 2));
    for _ in 0..4 {
      Actors::fanout_dirty_observations(base.saturating_add(unit));
      if Actors::dirty_observation_feeds(31).is_none() {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(31).is_none());
    assert!(
      Actors::pending_signal(late_subscriber),
      "late subscriber delivered next revision"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn maximum_density_fanout_converges_without_duplicate_queue_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_count = Actors::effective_active_actor_limit();
    let actors = (0..actor_count)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![13]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(13, 1));
    let consumed = Actors::fanout_dirty_observations(Weight::MAX);
    assert_ne!(consumed, Weight::zero());
    assert!(Actors::dirty_observation_feeds(13).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    let mut tickets = alloc::collections::BTreeSet::new();
    for actor_id in actors {
      let hot = Actors::actor_hot(actor_id).expect("active actor");
      assert!(hot.pending_signal);
      assert!(tickets.insert(hot.queue_ticket.expect("one queue ticket")));
    }
    assert_eq!(tickets.len() as u32, actor_count);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn maximum_observation_subscription_density_is_paged_and_bounded() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_count = Actors::effective_active_actor_limit();
    let feed = 1;
    for _ in 0..actor_count {
      create_system_with(
        ALICE,
        observation_schedule(vec![feed]),
        None,
        inert_contract_steps(),
      );
    }
    assert_eq!(Actors::active_actor_count(), actor_count);
    assert_eq!(Actors::observation_subscription_count(), actor_count);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_count = actor_count.div_ceil(page_size);
    assert_eq!(Actors::observation_subscriber_count(feed), actor_count);
    assert_eq!(
      crate::ObservationSubscriberPages::<Test>::iter_prefix(feed).count() as u32,
      page_count
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_dirty_observation_list_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_system_with(
      ALICE,
      observation_schedule(vec![6]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(6, 1));
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ObservationIngressRevisions::<Test>::insert(6, 2);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::ObservationIngressRevisions::<Test>::insert(6, 1);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = None);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_subscription_reverse_index_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ActorObservationFeeds::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn creation_subscription_failures_roll_back_exactly() {
  for corrupt_topology in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      if corrupt_topology {
        crate::ObservationSubscriptionSlotOwner::<Test>::insert(0, 999);
      } else {
        crate::NextObservationSubscriptionSlot::<Test>::put(
          <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
        );
      }
      let expected_error = if corrupt_topology {
        Error::<Test>::ObservationSubscriptionInvariant
      } else {
        Error::<Test>::ObservationSubscriptionCapacityExceeded
      };
      prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let events_before = System::events();

      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(
            observation_schedule(vec![1]),
            None,
            transfer_contract_steps(BOB, 1),
          ),
        ),
        expected_error
      );
      assert_eq!(Actors::next_actor_id(), 0);
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
    });
  }
}

#[test]
fn manual_trigger_rejects_address_and_observation_only_policies() {
  let observation_schedule = observation_schedule(vec![7]);
  for schedule in [
    on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
    observation_schedule,
  ] {
    new_test_ext().execute_with(move || {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule,
        None,
        transfer_contract_steps(BOB, 10),
      );
      assert_noop!(
        Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
        Error::<Test>::ManualSourceDisabled
      );
      let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert!(!instance.pending_signal);
      assert!(instance.queue_ticket.is_none());
    });
  }
}

#[test]
fn close_after_productive_cycle_rechecks_latest_observation_before_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 1;
    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationBelow {
        feed,
        threshold: 100,
        max_age_blocks: 10,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        observation_schedule(vec![feed]),
        None,
        contract_steps_with_step(step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::note_observation_changed(feed, 1));
    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 150,
        observed_at: 1,
      },
    );

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn unconfigured_observation_provider_fails_closed() {
  assert_eq!(
    <() as crate::ObservationProvider<u32, u64>>::observe(&1, 0, 10),
    crate::ScalarObservationState::Unavailable
  );
}

#[test]
fn observation_conditions_compare_only_fresh_scalar_values() {
  new_test_ext().execute_with(|| {
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 0,
      },
    );
    set_observation(2, crate::ScalarObservationState::Unavailable);
    set_observation(3, crate::ScalarObservationState::Uninitialized);
    set_observation(4, crate::ScalarObservationState::Stale);
    let fresh = all_conditions(vec![
      Predicate::ObservationAbove {
        feed: 1,
        threshold: 49,
        max_age_blocks: 10,
      },
      Predicate::ObservationBelow {
        feed: 1,
        threshold: 51,
        max_age_blocks: 10,
      },
      Predicate::ObservationEquals {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      },
      Predicate::ObservationNotEquals {
        feed: 1,
        threshold: 49,
        max_age_blocks: 10,
      },
    ]);
    assert_eq!(
      Actors::evaluate_precondition(fresh.as_ref().expect("bounded precondition"), &ALICE, 0),
      Ok(true)
    );

    for feed in 2..=4 {
      let unavailable = all_conditions(vec![Predicate::ObservationNotEquals {
        feed,
        threshold: 50,
        max_age_blocks: 10,
      }]);
      assert_eq!(
        Actors::evaluate_precondition(
          unavailable.as_ref().expect("bounded precondition"),
          &ALICE,
          0
        ),
        Ok(false)
      );
    }
  });
}

#[test]
fn invalid_fresh_observation_fails_permanently_and_applies_step_policy() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10);
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 11,
      },
    );
    let invalid_condition_step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 1,
        max_age_blocks: 5,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(7),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![invalid_condition_step, succeeding_step])
        .expect("two-step plan fits"),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 7);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        step_index: 0,
        error,
        ..
      } if *id == actor_id && *error == Error::<Test>::InvalidPredicate.into()
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        outcomes: OutcomeTotals { failed_steps: 1, committed_effectful_tasks: 1, .. },
        ..
      } if *id == actor_id
    )));

    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 4,
      },
    );
    let over_age = all_conditions(vec![Predicate::ObservationAbove {
      feed: 1,
      threshold: 1,
      max_age_blocks: 5,
    }]);
    assert_eq!(
      Actors::evaluate_precondition(over_age.as_ref().expect("bounded precondition"), &ALICE, 0),
      Err(Error::<Test>::InvalidPredicate.into())
    );
  });
}

#[test]
fn zero_observation_max_age_is_rejected_during_plan_validation() {
  new_test_ext().execute_with(|| {
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 0,
        max_age_blocks: 0,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, plan),
      ),
      Error::<Test>::InvalidObservationMaxAge
    );
  });
}

#[test]
fn unavailable_observation_skips_without_incrementing_failures() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationNotEquals {
        feed: 99,
        threshold: 0,
        max_age_blocks: 10,
      }]),
      task: Task::StopCycle,
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      0
    );
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}
