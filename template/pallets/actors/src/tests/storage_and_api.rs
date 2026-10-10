use super::*;
#[cfg(feature = "try-runtime")]
use crate::TriggerDeadlineHandles;
use crate::weights::WeightInfo as _;
use crate::{
  ActorContractHeads, ActorContractTailChunks, ActorCostQuoteError, ActorProcess, ActorProcesses,
  ActorRef, ActorSemanticExecutionProjection, ActorSemanticLoadError, ActorSemanticMutation,
  ActorSemanticMutationError, ActorSemanticProjectionError, ActorSemanticRecord,
  ActorSemanticState, ActorSemanticStates, ActorStepResourceEnvelope, ActorUnsignaledControlCells,
  ActorWaitingOccupancies, BalanceDependencySources, BlockResourceDomain, CanonicalOccurrenceError,
  CanonicalOccurrencePlan, CanonicalOccurrencePublication, CloseReason, DeadlineHandle,
  DeadlineHandles, DeadlineHeader, DeadlineHeaders, DeadlineIndexLen, DeadlineIndexMutationError,
  DeadlineIndexPages, DeadlineIndexPositions, DeadlineMutationError, DeadlinePage, DeadlinePages,
  DependencyDueReviewError, DependencyDueReviewMutation, DependencyPlanMutation,
  DependencyPlanSource, DependencyPlans, DependencyPublicationError, DependencyPublicationMutation,
  DependencyRegistrationError, DependencyRegistrationFreePositions, DependencyRegistrationHandle,
  DependencyRegistrationHeaders, DependencyRegistrationMutation, DependencyRegistrationPages,
  DependencyRegistrationPosition, DependencyRegistrationPositions, DependencyRegistrations,
  DependencyReviewMutation, DependencyReviewWorkerError, DependencyRevisionError,
  DependencyRevisionMutation, DependencyRevisionState, DependencyRevisions, DependencyScanError,
  DependencyScanMutation, DependencyScanSourceError, DependencyScanSourceList,
  DependencyScanSourceListState, DependencyScanSourceMutation, DependencyScanSourceNode,
  DependencyScanSourceNodes, DependencySourceAllocator, DependencySourceBalances,
  DependencyTimedReview, DependencyTimedReviewMutation, DependencyTimedReviews,
  DormantActorSemanticRecord, DueBlockDeadlineMutation, LegacyProcessPlacement,
  LegacyProcessTransition, ParkEvidence, ParkNegativeReason, ParkedBalanceActivation,
  ParkedBalanceActivationError, ParkedBalanceActivationOf, ParkedBalanceCertificationError,
  ParkedBalanceClassificationError, ParkedBalanceEpisodes, ParkedBalanceQualification,
  ParkedBalanceRule, ParkedBalanceTransitionError, PendingCheckOwner, PendingCheckOwners,
  PendingDependencyEvent, PendingDependencyEvents, PendingDependencyReviews,
  PipelineMachineFeeStrategy, ProcessCompileError, ProcessDisableCause, ProcessDisablement,
  ProcessPublicationError, ProcessResidence, ProcessRevivalAuthority, ProcessStatus,
  ProcessTransitionError, ProcessTransitionObligation, ServiceHeader, ServiceHeaderRecord,
  ServiceNode, ServiceNodes, ServicePublicationError, ServiceResidenceKind, ServiceRetirementError,
  ServiceRingMutationError, ServiceRoundEncounter, ServiceRoundError, SuspendedProcessBasis,
  UnsignaledProcessEvidence, apply_actor_semantic_mutation, certify_parked_balance_watch,
  classify_parked_balance, classify_parked_balance_plan, compile_legacy_process,
  next_actor_generation, plan_canonical_occurrence, plan_legacy_process_transition,
  project_actor_semantic_execution,
};
use frame::traits::ConstU32;
use std::collections::BTreeMap;

#[test]
fn process_skeleton_uses_generation_bound_references() {
  assert_eq!(
    ActorRef {
      actor_id: 7,
      generation: 11
    }
    .encoded_size(),
    16
  );
  assert_ne!(
    ActorRef {
      actor_id: 7,
      generation: 11
    },
    ActorRef {
      actor_id: 7,
      generation: 12
    }
  );
}

#[test]
fn parked_balance_activation_is_nonempty_unique_and_canonical() {
  type Activation = ParkedBalanceActivation<u32, u128, ConstU32<2>>;
  assert_eq!(
    Activation::try_from_rules(vec![]),
    Err(ParkedBalanceActivationError::Empty)
  );
  assert_eq!(
    Activation::try_from_rules(vec![ParkedBalanceRule {
      asset: 1,
      authored_min_delta: 0,
    }]),
    Err(ParkedBalanceActivationError::ZeroAuthoredMinimum)
  );
  assert_eq!(
    Activation::try_from_rules(vec![
      ParkedBalanceRule {
        asset: 1,
        authored_min_delta: 10,
      },
      ParkedBalanceRule {
        asset: 1,
        authored_min_delta: 20,
      },
    ]),
    Err(ParkedBalanceActivationError::DuplicateAsset)
  );
  assert_eq!(
    Activation::try_from_rules(vec![
      ParkedBalanceRule {
        asset: 3,
        authored_min_delta: 30,
      },
      ParkedBalanceRule {
        asset: 2,
        authored_min_delta: 20,
      },
      ParkedBalanceRule {
        asset: 1,
        authored_min_delta: 10,
      },
    ]),
    Err(ParkedBalanceActivationError::TooManyAssets)
  );
  let activation = Activation::try_from_rules(vec![
    ParkedBalanceRule {
      asset: 2,
      authored_min_delta: 20,
    },
    ParkedBalanceRule {
      asset: 1,
      authored_min_delta: 10,
    },
  ])
  .expect("bounded unique watch set certifies");
  assert_eq!(
    activation.watches.as_slice(),
    &[
      ParkedBalanceRule {
        asset: 1,
        authored_min_delta: 10,
      },
      ParkedBalanceRule {
        asset: 2,
        authored_min_delta: 20,
      },
    ],
    "asset order is canonical regardless of author input order"
  );
}

#[test]
fn configured_parked_watch_limit_preserves_contract_capacity_and_close_custody() {
  use codec::{Decode, Encode};

  for actor_type in [ActorType::User, ActorType::System] {
    new_test_ext().execute_with(|| {
      assert_eq!(
        <<Test as crate::Config>::MaxParkedBalanceWatches as Get<u32>>::get(),
        4
      );
      assert_eq!(
        <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get(),
        12
      );
      assert_eq!(
        <<Test as crate::Config>::MaxWhitelistSize as Get<u32>>::get(),
        16
      );
      let rules: Vec<_> = (1..=5)
        .map(|asset| ParkedBalanceRule {
          asset: TestAsset::Local(asset),
          authored_min_delta: 100,
        })
        .collect();
      let before = storage_root();
      assert_eq!(
        ParkedBalanceActivationOf::<Test>::try_from_rules(rules.clone()),
        Err(ParkedBalanceActivationError::TooManyAssets)
      );
      let oversized =
        ParkedBalanceActivation::<TestAsset, u128, ConstU32<5>>::try_from_rules(rules.clone())
          .unwrap();
      assert!(ParkedBalanceActivationOf::<Test>::decode(&mut &oversized.encode()[..]).is_err());
      assert_eq!(storage_root(), before);

      let step = inert_contract_steps()[0].clone();
      let steps = vec![step; 12].try_into().unwrap();
      let mut contract = system_active_contract(manual_schedule(), None, steps).unwrap();
      contract.parked_balance_activation =
        Some(ParkedBalanceActivationOf::<Test>::try_from_rules(rules[..4].to_vec()).unwrap());
      let actor_id = Actors::next_actor_id();
      if actor_type == ActorType::User {
        prefund_active_user_creation(ALICE, &contract.steps);
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          Some(contract),
        ));
      } else {
        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          Some(contract),
        ));
      }
      let identity = Actors::actor_identity(actor_id).unwrap();
      assert_eq!(Actors::actor_contract(actor_id).unwrap().steps.len(), 12);
      assert_eq!(crate::DependencyPlans::<Test>::get(actor_id).len(), 4);
      assert_eq!(
        crate::ParkedBalanceEpisodes::<Test>::get(actor_id)
          .unwrap()
          .watches
          .len(),
        4
      );
      assert!(!crate::BalanceDependencySources::<Test>::contains_key(
        TestAsset::Local(5)
      ));
      for rule in &rules {
        assert_ok!(MockAssetOps::mint(
          &identity.sovereign_account,
          rule.asset,
          1_000
        ));
      }
      let balances: Vec<_> = rules
        .iter()
        .map(|rule| {
          (
            rule.asset,
            MockAssetOps::balance(&identity.sovereign_account, rule.asset),
          )
        })
        .collect();
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert!(!crate::ParkedBalanceEpisodes::<Test>::contains_key(
        actor_id
      ));
      assert!(crate::DependencyPlans::<Test>::get(actor_id).is_empty());
      for (asset, balance) in balances {
        assert_eq!(
          MockAssetOps::balance(&identity.sovereign_account, asset),
          balance
        );
        assert!(!crate::BalanceDependencySources::<Test>::contains_key(
          asset
        ));
      }
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().unwrap();
    });
  }
}

#[test]
fn actor_contract_admits_only_canonical_parked_balance_activation() {
  new_test_ext().execute_with(|| {
    let activation = ParkedBalanceActivationOf::<Test>::try_from_rules(vec![
      ParkedBalanceRule {
        asset: TestAsset::Local(9),
        authored_min_delta: 100,
      },
      ParkedBalanceRule {
        asset: TestAsset::Native,
        authored_min_delta: 200,
      },
    ])
    .expect("canonical activation");
    let mut contract = system_active_contract(manual_schedule(), None, inert_contract_steps())
      .expect("active Contract");
    let without_activation = contract.semantic_contract_id();
    contract.parked_balance_activation = Some(activation.clone());
    assert_ne!(contract.semantic_contract_id(), without_activation);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(contract),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert_eq!(
      Actors::actor_contract(actor_id)
        .expect("admitted Contract reconstructs")
        .parked_balance_activation,
      Some(activation)
    );

    let mut malformed = system_active_contract(manual_schedule(), None, inert_contract_steps())
      .expect("active Contract");
    malformed.parked_balance_activation = Some(ParkedBalanceActivation {
      watches: BoundedVec::default(),
    });
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        BOB,
        Mutability::Mutable,
        Some(malformed),
      ),
      Error::<Test>::InvalidParkedBalanceActivation
    );
  });
}

#[test]
fn temporal_balance_headers_reject_terminal_and_empty_contracts() {
  new_test_ext().execute_with(|| {
    for trigger in [Trigger::at_time(100), Trigger::cadenced(100)] {
      for incompatible in 0..3 {
        let mut contract = system_active_contract(
          Schedule {
            trigger: trigger.clone(),
            cooldown_blocks: 0,
          },
          None,
          inert_contract_steps(),
        )
        .unwrap();
        contract.parked_balance_activation = Some(
          ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
            asset: TestAsset::Native,
            authored_min_delta: 100,
          }])
          .unwrap(),
        );
        match incompatible {
          0 => contract.steps = BoundedVec::default(),
          1 => contract.auto_close_at_cycle_nonce = Some(1),
          _ => contract.close_when = Some(crate::tests::close_when_action_and_normal_cycle()),
        }
        assert_noop!(
          Actors::create_system_actor(
            RuntimeOrigin::root(),
            ALICE,
            Mutability::Mutable,
            Some(contract),
          ),
          Error::<Test>::InvalidParkedBalanceActivation
        );
      }
    }
  });
}

fn public_parked_balance_contract(trigger: crate::TriggerOf<Test>) -> crate::ActorContractOf<Test> {
  let mut contract = user_active_contract(
    Schedule {
      trigger,
      cooldown_blocks: 0,
    },
    None,
    contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(9),
      amount: AmountResolution::Fixed(1),
    })),
  )
  .unwrap();
  contract.parked_balance_activation = Some(
    ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
      asset: TestAsset::Local(9),
      authored_min_delta: 100,
    }])
    .unwrap(),
  );
  contract
}

#[test]
fn parked_balance_public_admission_rejects_cadenced_atomically() {
  for actor_type in [ActorType::User, ActorType::System] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let invalid = public_parked_balance_contract(Trigger::cadenced(10));
      prefund_active_user_creation(ALICE, &invalid.steps);
      let actor_id = Actors::next_actor_id();
      let create = || match actor_type {
        ActorType::User => Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          Some(invalid.clone()),
        ),
        ActorType::System => Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          Some(invalid.clone()),
        ),
      };
      assert_noop!(create(), Error::<Test>::InvalidParkedBalanceActivation);
      assert!(Actors::build_admission_certificate(&invalid).is_none());
      assert_ok!(match actor_type {
        ActorType::User =>
          Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, None),
        ActorType::System =>
          Actors::create_system_actor(RuntimeOrigin::root(), ALICE, Mutability::Mutable, None),
      });
      let origin = match actor_type {
        ActorType::User => RuntimeOrigin::signed(ALICE),
        ActorType::System => RuntimeOrigin::root(),
      };
      System::set_block_number(2);
      assert_noop!(
        Actors::activate_actor(origin.clone(), actor_id, invalid.clone()),
        Error::<Test>::InvalidParkedBalanceActivation
      );
      assert_ok!(Actors::activate_actor(
        origin.clone(),
        actor_id,
        public_parked_balance_contract(Trigger::Manual)
      ));
      System::set_block_number(3);
      assert_noop!(
        Actors::update_contract(origin, actor_id, invalid),
        Error::<Test>::InvalidParkedBalanceActivation
      );
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn parked_balance_activation_arms_current_custody_without_execution() {
  for actor_type in [ActorType::User, ActorType::System] {
    for trigger in [Trigger::Manual, Trigger::at_time(100)] {
      for dormant in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          let contract = public_parked_balance_contract(trigger.clone());
          prefund_active_user_creation(ALICE, &contract.steps);
          let actor_id = Actors::next_actor_id();
          let sovereign = match actor_type {
            ActorType::User => Actors::sovereign_account_id(&ALICE, 0),
            ActorType::System => Actors::sovereign_account_id_system(actor_id),
          };
          set_asset_balance(&sovereign, TestAsset::Local(9), 321);
          let recipient = MockAssetOps::balance(&BOB, TestAsset::Local(9));
          assert_ok!(match actor_type {
            ActorType::User => Actors::create_user_actor(
              RuntimeOrigin::signed(ALICE),
              Mutability::Mutable,
              (!dormant).then(|| contract.clone())
            ),
            ActorType::System => Actors::create_system_actor(
              RuntimeOrigin::root(),
              ALICE,
              Mutability::Mutable,
              (!dormant).then(|| contract.clone())
            ),
          });
          if dormant {
            System::set_block_number(2);
            assert_ok!(Actors::activate_actor(
              match actor_type {
                ActorType::User => RuntimeOrigin::signed(ALICE),
                ActorType::System => RuntimeOrigin::root(),
              },
              actor_id,
              contract
            ));
          }
          let actor = Actors::load_actor_ref(actor_id).unwrap();
          let episode = ParkedBalanceEpisodes::<Test>::get(actor_id).unwrap();
          assert_eq!(episode.owner.actor, actor);
          assert_eq!(episode.owner.plan_revision, 0);
          assert_eq!(episode.watches[0].anchor, 321);
          assert_eq!(
            PendingCheckOwners::<Test>::get(actor_id),
            Some(episode.owner)
          );
          assert!(!ServiceNodes::<Test>::contains_key(actor_id));
          assert!(matches!(
            ActorProcesses::<Test>::get(actor_id).unwrap().residence,
            Some(ProcessResidence::Parked(_))
          ));
          assert_eq!(
            DeadlineHandles::<Test>::get(actor_id).unwrap().key,
            WakeupKey::Block(System::block_number() + 1)
          );
          assert_eq!(
            crate::TriggerDeadlineHandles::<Test>::contains_key(actor_id),
            matches!(trigger, Trigger::AtTime { .. })
          );
          let sink =
            MockAssetOps::balance(&<Test as crate::Config>::FeeSink::get(), TestAsset::Native);
          run_next_idle(Weight::MAX);
          let state = Actors::active_actor_state(actor_id).unwrap();
          assert_eq!(state.identity.cycle_nonce, 0);
          assert_eq!(state.hot.cycle_state, CycleState::Idle);
          assert!(!state.hot.pending_signal);
          assert!(state.run_state.is_none());
          assert_eq!(MockAssetOps::balance(&BOB, TestAsset::Local(9)), recipient);
          assert_eq!(MockAssetOps::balance(&sovereign, TestAsset::Local(9)), 321);
          assert_eq!(
            MockAssetOps::balance(&<Test as crate::Config>::FeeSink::get(), TestAsset::Native),
            sink
          );
          assert_eq!(
            ParkedBalanceEpisodes::<Test>::get(actor_id)
              .unwrap()
              .watches,
            episode.watches
          );
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        });
      }
    }
  }
}

#[test]
fn parked_balance_reconfiguration_preserves_paid_readiness() {
  for paid in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mut contract = public_parked_balance_contract(Trigger::Manual);
      contract.parked_balance_activation = None;
      prefund_active_user_creation(ALICE, &contract.steps);
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(contract)
      ));
      let actor = Actors::load_actor_ref(actor_id).unwrap();
      let sovereign = sovereign_account(actor_id);
      set_asset_balance(&sovereign, TestAsset::Local(9), 321);
      if paid {
        fund_native(actor_id, 1_000_000_000_000_000_000);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
      }
      let sink = MockAssetOps::balance(&<Test as crate::Config>::FeeSink::get(), TestAsset::Native);
      System::set_block_number(2);
      assert_ok!(Actors::update_contract(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        public_parked_balance_contract(Trigger::Manual)
      ));
      let successor = Actors::load_actor_ref(actor_id).unwrap();
      assert_eq!(successor.generation, actor.generation + 1);
      let state = Actors::active_actor_state(actor_id).unwrap();
      assert_eq!(state.identity.cycle_nonce, 0);
      assert_eq!(state.hot.pending_signal, paid);
      assert_eq!(
        MockAssetOps::balance(&<Test as crate::Config>::FeeSink::get(), TestAsset::Native),
        sink
      );
      if paid {
        assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
        assert!(ServiceNodes::<Test>::contains_key(actor_id));
        run_next_idle(Weight::MAX);
        assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 1);
        assert_eq!(MockAssetOps::balance(&sovereign, TestAsset::Local(9)), 320);
      }
      let episode = ParkedBalanceEpisodes::<Test>::get(actor_id).unwrap();
      assert_eq!(episode.owner.actor, successor);
      assert_eq!(episode.watches[0].anchor, if paid { 320 } else { 321 });
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn parked_balance_qualified_review_opens_one_cycle_at_b_plus_one() {
  for actor_type in [ActorType::User, ActorType::System] {
    for trigger in [Trigger::Manual, Trigger::at_time(20)] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let actor_id = Actors::next_actor_id();
        let mut contract = public_parked_balance_contract(trigger.clone());
        contract.parked_balance_activation = Some(
          ParkedBalanceActivationOf::<Test>::try_from_rules(
            (6..=9)
              .map(|asset| ParkedBalanceRule {
                asset: TestAsset::Local(asset),
                authored_min_delta: 100,
              })
              .collect(),
          )
          .unwrap(),
        );
        if actor_type == ActorType::User {
          crate::UserCreationEnabled::<Test>::put(true);
          assert_ok!(Actors::create_user_actor(
            RuntimeOrigin::signed(ALICE),
            Mutability::Mutable,
            Some(contract),
          ));
        } else {
          assert_ok!(Actors::create_system_actor(
            RuntimeOrigin::root(),
            ALICE,
            Mutability::Mutable,
            Some(contract),
          ));
        }
        fund_native(actor_id, 1_000_000_000_000_000_000);
        let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
        let before = MockAssetOps::balance(&BOB, TestAsset::Local(9));
        let sink = <Test as crate::Config>::FeeSink::get();
        let sink_before = MockAssetOps::balance(&sink, TestAsset::Native);
        let quote = Actors::actor_cost_quote(actor_id)
          .unwrap()
          .prospective_trigger_fee
          .unwrap();
        assert_eq!(quote.trigger_family, TriggerFamily::ParkedBalance);
        // A mutation without a causal hook is still covered by the mandatory timed review.
        set_asset_balance(&sovereign, TestAsset::Local(9), 100);
        System::set_block_number(2);
        Actors::on_initialize(2);
        run_prepass();
        Actors::on_idle(2, Weight::MAX);
        let node = ServiceNodes::<Test>::get(actor_id).expect("qualified review publishes Pending");
        assert_eq!(node.kind, ServiceResidenceKind::Pending);
        assert_eq!(node.eligible_from, 3);
        assert!(
          Actors::active_actor_state(actor_id)
            .unwrap()
            .hot
            .pending_signal
        );
        assert_eq!(
          MockAssetOps::balance(&sink, TestAsset::Native),
          sink_before + quote.fee
        );
        assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
          actor_id
        ));
        assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 0);
        assert_eq!(MockAssetOps::balance(&BOB, TestAsset::Local(9)), before);
        System::set_block_number(3);
        Actors::on_initialize(3);
        run_prepass();
        Actors::on_idle(3, Weight::MAX);
        assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 1);
        assert_eq!(MockAssetOps::balance(&BOB, TestAsset::Local(9)), before + 1);
        assert_eq!(
          ParkedBalanceEpisodes::<Test>::get(actor_id)
            .unwrap()
            .watches[3]
            .anchor,
          99
        );
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn parked_balance_positive_review_refusal_preserves_exact_authority() {
  use crate::{DependencyReviewWorkerError, mock::set_fail_fee_sink_transfer};
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    crate::UserCreationEnabled::<Test>::put(true);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      Some(public_parked_balance_contract(Trigger::at_time(20))),
    ));
    let actor = Actors::load_actor_ref(actor_id).unwrap();
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, TestAsset::Local(9), 100);
    System::set_block_number(2);
    let review = || {
      Actors::process_next_due_block_dependency_review(
        &mut WeightMeter::with_limit(Weight::MAX),
        ServiceResidenceKind::Pending,
        2,
        None,
      )
    };
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut WeightMeter::with_limit(Weight::zero()),
        ServiceResidenceKind::Pending,
        2,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight),
    );
    assert_eq!(
      review(),
      Err(DependencyReviewWorkerError::Interpretation(
        DependencyRegistrationError::OccurrenceFailed(Error::<Test>::InsufficientFee.into()),
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    set_fail_fee_sink_transfer(true);
    assert_eq!(
      review(),
      Err(DependencyReviewWorkerError::Interpretation(
        DependencyRegistrationError::OccurrenceFailed(
          Error::<Test>::TriggerFeeCollectionFailed.into()
        ),
      ))
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    assert_eq!(review(), Ok((actor, crate::DependencyReviewMutation::Woke)));
    assert!(
      Actors::active_actor_state(actor_id)
        .unwrap()
        .hot
        .pending_signal
    );
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().eligible_from,
      3
    );
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    run_next_idle(Weight::MAX);
    assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 1);
  });
}

#[test]
fn parked_balance_initial_arming_rolls_back_partial_source_allocation() {
  for entry in 0..3 {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = Actors::next_actor_id();
      let mut contract = public_parked_balance_contract(Trigger::Manual);
      contract.parked_balance_activation = Some(
        ParkedBalanceActivationOf::<Test>::try_from_rules(vec![
          ParkedBalanceRule {
            asset: TestAsset::Native,
            authored_min_delta: 100,
          },
          ParkedBalanceRule {
            asset: TestAsset::Local(9),
            authored_min_delta: 100,
          },
        ])
        .unwrap(),
      );
      if entry != 0 {
        let existing = if entry == 2 {
          let mut existing = contract.clone();
          existing.parked_balance_activation = None;
          Some(existing)
        } else {
          None
        };
        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          existing
        ));
        System::set_block_number(2);
      }
      crate::DependencySourceAllocatorState::<Test>::put(DependencySourceAllocator {
        next: u64::MAX,
        exhausted: false,
      });
      assert_noop!(
        match entry {
          0 => Actors::create_system_actor(
            RuntimeOrigin::root(),
            ALICE,
            Mutability::Mutable,
            Some(contract)
          ),
          1 => Actors::activate_actor(RuntimeOrigin::root(), actor_id, contract),
          _ => Actors::update_contract(RuntimeOrigin::root(), actor_id, contract),
        },
        Error::<Test>::SchedulerIndexExhausted
      );
      assert!(BalanceDependencySources::<Test>::iter().next().is_none());
      assert!(DependencySourceBalances::<Test>::iter().next().is_none());
      assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn parked_balance_system_pause_and_resume_rearm_from_current_custody() {
  for trigger in [Trigger::Manual, Trigger::at_time(100)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        Some(public_parked_balance_contract(trigger.clone()))
      ));
      let actor = Actors::load_actor_ref(actor_id).unwrap();
      let tick = crate::TriggerDeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key);
      System::set_block_number(2);
      assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
      assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
      assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
      assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor_id
      ));
      let sovereign = sovereign_account(actor_id);
      set_asset_balance(&sovereign, TestAsset::Local(9), 500);
      System::set_block_number(3);
      assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
      assert_eq!(Actors::load_actor_ref(actor_id), Some(actor));
      assert_eq!(
        ParkedBalanceEpisodes::<Test>::get(actor_id)
          .unwrap()
          .watches[0]
          .anchor,
        500
      );
      assert!(!ServiceNodes::<Test>::contains_key(actor_id));
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
        tick
      );
      run_next_idle(Weight::MAX);
      assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 0);
      assert_eq!(MockAssetOps::balance(&sovereign, TestAsset::Local(9)), 500);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn parked_dependency_last_source_read_proof_uses_bounded_complete_subtrees() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieLayout, TrieMut,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let partial = vec![0x11; 81];
  let mut fanout = vec![(partial.clone(), vec![0x22; 32])];
  for child in 0..16u8 {
    let mut key = partial.clone();
    key.extend_from_slice(&[child << 4, 0]);
    fanout.push((key, vec![0x33; 32]));
  }
  // This reaches the header, partial, bitmap, inline value and sixteen hashed child maxima.
  assert_eq!(Layout::trie_root_unhashed(fanout).len(), 646);

  // The reference runtime separately checks this foreign-key width from its native key schema.
  let maximum_foreign_key_bytes = 712;
  let partial = vec![0x11; maximum_foreign_key_bytes];
  let mut fanout = vec![(partial.clone(), vec![0x22; 32])];
  for child in 0..16u8 {
    let mut key = partial.clone();
    key.extend_from_slice(&[child << 4, 0]);
    fanout.push((key, vec![0x33; 32]));
  }
  assert_eq!(Layout::trie_root_unhashed(fanout).len(), 1282);

  let maximum_sources =
    <<Test as crate::Config>::MaxParkedBalanceWatches as Get<u32>>::get() as usize;
  assert_eq!(maximum_sources, 4);
  for (reverse, holes, sources) in [
    (false, 0, 1),
    (false, 31, 1),
    (true, 0, 1),
    (true, 31, 1),
    (false, 31, maximum_sources),
    (true, 31, maximum_sources),
  ] {
    let mut pairs = BTreeMap::new();
    let mut queries = Vec::new();
    let mut source_prefixes = Vec::<Vec<u8>>::new();
    for offset in 0..sources {
      let source = u64::MAX - offset as u64;
      let mut source_queries = Vec::new();
      for slot in 0..32u8 {
        let position = DependencyRegistrationPosition {
          page: u64::MAX,
          slot,
        };
        let (key, value) = if reverse {
          (
            crate::DependencyRegistrationFreeIndices::<Test>::hashed_key_for(source, position),
            u64::from(slot).encode(),
          )
        } else {
          (
            DependencyRegistrationFreePositions::<Test>::hashed_key_for(source, u64::from(slot)),
            position.encode(),
          )
        };
        assert_eq!(key.len(), if reverse { 81 } else { 80 });
        assert_eq!(value.len(), if reverse { 8 } else { 9 });
        if slot < holes {
          pairs.insert(key.clone(), value.clone());
        }
        source_queries.push((key, (slot < holes).then_some(value)));
      }
      let source_prefix = source_queries[0].0[..56].to_vec();
      for (key, _) in &source_queries {
        assert_eq!(&key[..56], source_prefix);
      }
      if let Some(first) = source_prefixes.first() {
        assert_eq!(&source_prefix[..32], &first[..32]);
      }
      // Saturate every ancestor nibble with off-path children and large inline branch values.
      for nibble in 0..if holes == 0 { 111 } else { 112 } {
        if nibble % 2 == 0 {
          pairs.insert(source_prefix[..nibble / 2].to_vec(), vec![0x44; 32]);
        }
        let selected = if nibble % 2 == 0 {
          source_prefix[nibble / 2] >> 4
        } else {
          source_prefix[nibble / 2] & 15
        };
        for sibling in 0..16u8 {
          if sibling == selected {
            continue;
          }
          let mut key = source_prefix[..=nibble / 2].to_vec();
          key[nibble / 2] = if nibble % 2 == 0 {
            sibling << 4
          } else {
            (key[nibble / 2] & 0xf0) | sibling
          };
          pairs.insert(key, vec![0x55; 32]);
        }
      }
      if holes == 0 {
        let mut foreign_key = source_prefix.clone();
        foreign_key[55] ^= 1;
        foreign_key.resize(maximum_foreign_key_bytes, 0x66);
        pairs.insert(foreign_key, vec![0x77; 32]);
      }
      source_prefixes.push(source_prefix);
      queries.extend(source_queries);
    }
    // Synthetic siblings must not add entries inside another queried complete subtree.
    for prefix in &source_prefixes {
      assert_eq!(
        pairs.keys().filter(|key| key.starts_with(prefix)).count(),
        holes as usize
      );
    }
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let mut root = Default::default();
    {
      let mut trie = TrieDBMutBuilder::<Layout>::new(&mut database, &mut root).build();
      for (key, value) in &pairs {
        trie.insert(key, value).unwrap();
      }
    }
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for (key, expected) in &queries {
        assert_eq!(trie.get(key).unwrap(), *expected);
      }
    }
    let proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
    if holes == 0 {
      // Absence reaches the divergent foreign leaf rather than a populated source subtree.
      assert!(proof.iter_nodes().any(|node| node.len() > 650));
      assert!(proof.encoded_size() <= 113 * 650 + 1284 + 5);
    }
    let complete_subtree_nodes = 2 * 32 - 1;
    let shared_ancestor_nodes = 2 * 32 + 1;
    let source_ancestor_nodes = 2 * (56 - 32);
    let node_bound =
      shared_ancestor_nodes + sources * (source_ancestor_nodes + complete_subtree_nodes);
    let encoded_bound = node_bound * 650 + 5;
    assert!(proof.encoded_size() <= encoded_bound);
    if holes > 0 {
      assert!(proof.iter_nodes().all(|node| node.encoded_size() <= 650));
      assert!(proof.iter_nodes().count() <= node_bound);
    }
    if sources == maximum_sources {
      assert_eq!(encoded_bound, 330_855);
    }
  }
}

#[test]
fn parked_dependency_last_source_deletion_proof_includes_collapsed_sibling() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieError, TrieMut, delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  for (reverse, branch_sibling) in [(false, false), (false, true), (true, false), (true, true)] {
    let mut source_pairs = Vec::new();
    for slot in 0..31u8 {
      let position = DependencyRegistrationPosition {
        page: u64::MAX,
        slot,
      };
      source_pairs.push(if reverse {
        (
          crate::DependencyRegistrationFreeIndices::<Test>::hashed_key_for(u64::MAX, position),
          u64::from(slot).encode(),
        )
      } else {
        (
          DependencyRegistrationFreePositions::<Test>::hashed_key_for(u64::MAX, u64::from(slot)),
          position.encode(),
        )
      });
    }
    let source_prefix = &source_pairs[0].0[..56];
    let mut foreign_key = source_prefix.to_vec();
    foreign_key[55] ^= 1;
    foreign_key.resize(if branch_sibling { 710 } else { 712 }, 0x66);
    for sibling_value_bytes in [32, 8 * 1024] {
      let mut foreign_pairs = vec![(foreign_key.clone(), vec![0x77; sibling_value_bytes])];
      if branch_sibling {
        // A full hashed fanout survives; collapsing its parent need not read descendants.
        for child in 0..16u8 {
          let mut key = foreign_key.clone();
          key.extend_from_slice(&[child << 4, 0]);
          foreign_pairs.push((key, vec![0x88; 32]));
        }
      }
      assert!(foreign_pairs.iter().all(|(key, _)| key.len() <= 712));
      assert!(
        foreign_pairs
          .iter()
          .all(|(key, _)| !key.starts_with(source_prefix))
      );
      let mut database = MemoryDB::<Blake2Hasher>::default();
      let mut root = Default::default();
      {
        let mut trie = TrieDBMutBuilder::<Layout>::new(&mut database, &mut root).build();
        for (key, value) in source_pairs.iter().chain(&foreign_pairs) {
          trie.insert(key, value).unwrap();
        }
      }
      let mut recorder = Recorder::<Layout>::new();
      {
        let trie = TrieDBBuilder::<Layout>::new(&database, &root)
          .with_recorder(&mut recorder)
          .build();
        for (key, expected) in &source_pairs {
          assert_eq!(trie.get(key).unwrap(), Some(expected.clone()));
        }
      }
      let read_proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
      assert!(
        read_proof
          .iter_nodes()
          .all(|node| node.encoded_size() <= 650)
      );
      let delta = source_pairs
        .iter()
        .map(|(key, _)| (key.clone(), None::<Vec<u8>>))
        .collect::<Vec<_>>();
      let mut incomplete_database = read_proof.to_memory_db::<Blake2Hasher>();
      let error = delta_trie_root::<Layout, _, _, _, _, _>(
        &mut incomplete_database,
        root,
        delta.clone(),
        None,
        None,
      )
      .unwrap_err();
      assert!(matches!(
        error.as_ref(),
        TrieError::<Layout>::IncompleteDatabase(_)
      ));

      let updated_root = delta_trie_root::<Layout, _, _, _, _, _>(
        &mut database,
        root,
        delta.clone(),
        Some(&mut recorder),
        None,
      )
      .unwrap();
      assert_ne!(updated_root, root);
      assert_eq!(updated_root, Layout::trie_root(foreign_pairs));
      let mutation_nodes = recorder
        .drain()
        .into_iter()
        .map(|record| record.data)
        .collect::<BTreeSet<_>>();
      let read_nodes = read_proof.iter_nodes().cloned().collect::<BTreeSet<_>>();
      let extra = mutation_nodes.difference(&read_nodes).collect::<Vec<_>>();
      assert_eq!(extra.len(), 1);
      assert!(extra[0].encoded_size() > 650);
      assert!(extra[0].encoded_size() <= 1284);
      let complete_proof = StorageProof::new(read_nodes.into_iter().chain(mutation_nodes));
      assert!(complete_proof.encoded_size() <= read_proof.encoded_size() + 1284 + 5);
      let mut complete_database = complete_proof.into_memory_db::<Blake2Hasher>();
      assert_eq!(
        delta_trie_root::<Layout, _, _, _, _, _>(&mut complete_database, root, delta, None, None,)
          .unwrap(),
        updated_root,
      );
    }
  }
}

#[test]
fn parked_dependency_sources_reclaim_public_actor_churn_with_fresh_ids() {
  new_test_ext().execute_with(|| {
    let mut retained = Vec::new();
    let sovereign_id = Actors::next_actor_id();
    for index in 0..3u32 {
      let block = 1u64 + u64::from(index) * 4;
      System::set_block_number(block);
      let asset = TestAsset::Local(100);
      let mut contract = system_active_contract(
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      )
      .unwrap();
      contract.parked_balance_activation = Some(
        ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
          asset,
          authored_min_delta: 100,
        }])
        .unwrap(),
      );
      let actor_id = Actors::next_actor_id();
      if index == 0 {
        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          Some(contract)
        ));
      } else {
        assert_ok!(Actors::create_system_actor_at_sovereign_id(
          RuntimeOrigin::root(),
          sovereign_id,
          ALICE,
          Mutability::Mutable,
          Some(contract)
        ));
      }
      assert_eq!(Actors::system_sovereign_count(), 1);
      assert_eq!(Actors::active_actor_count(), 1);
      assert!(ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
      let source = BalanceDependencySources::<Test>::get(asset).unwrap();
      assert_eq!(source, u64::from(index));
      let registration = DependencyRegistrations::<Test>::get(source, actor_id).unwrap();
      let position = DependencyRegistrationPositions::<Test>::get(source, actor_id).unwrap();
      assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 1);

      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert_eq!(Actors::active_actor_count(), 0);
      assert_eq!(Actors::actor_identity_count(), 0);
      assert!(!DependencyRegistrations::<Test>::contains_key(
        source, actor_id
      ));
      assert!(!DependencyRegistrationPositions::<Test>::contains_key(
        source, actor_id
      ));
      retained.push((asset, source, position));
      for (asset, source, position) in &retained {
        assert!(!BalanceDependencySources::<Test>::contains_key(asset));
        assert!(!DependencySourceBalances::<Test>::contains_key(source));
        assert!(!DependencyRegistrationHeaders::<Test>::contains_key(source));
        assert!(!DependencyRevisions::<Test>::contains_key(source));
        assert!(!DependencyScanSourceNodes::<Test>::contains_key(source));
        assert!(!DependencyRegistrationFreePositions::<Test>::contains_key(
          source, 0
        ));
        assert!(!crate::DependencyRegistrationFreeIndices::<Test>::contains_key(source, position));
        assert!(!DependencyRegistrationPages::<Test>::contains_key(
          source,
          position.page
        ));
      }
      assert_eq!(
        Actors::dependency_source_allocator().next,
        u64::from(index + 1)
      );
      assert_eq!(registration.actor.actor_id, actor_id);
    }
  });
}

#[test]
fn parked_dependency_gap_churn_diverges_sources_with_bounded_sovereign_slots() {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};

  fn contract(assets: &[TestAsset]) -> crate::ActorContractOf<Test> {
    let mut contract = system_active_contract(
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    )
    .unwrap();
    contract.parked_balance_activation = Some(
      ParkedBalanceActivationOf::<Test>::try_from_rules(
        assets
          .iter()
          .map(|asset| ParkedBalanceRule {
            asset: *asset,
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    );
    contract
  }

  fn park(contract: crate::ActorContractOf<Test>, slot: Option<u64>) -> (u64, u64) {
    let actor = Actors::next_actor_id();
    if let Some(slot) = slot {
      assert_ok!(Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        slot,
        ALICE,
        Mutability::Mutable,
        Some(contract),
      ));
    } else {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        Some(contract),
      ));
    }
    assert!(ParkedBalanceEpisodes::<Test>::contains_key(actor));
    let ActorClass::System { sovereign_id } = Actors::actor_identity(actor).unwrap().actor_class
    else {
      panic!("System authority")
    };
    (actor, sovereign_id)
  }

  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let assets = [TestAsset::Local(100), TestAsset::Local(101)];
    let retained_pages = [[0, 2, 5], [0, 1, 7]];
    let shared = contract(&assets);
    let mut slots = Vec::new();
    let mut permanent = Vec::new();
    for page in 0..3u64 {
      if page != 0 {
        // Any monotone retained-page choice can insert source-only gaps without growing locators.
        for (source_index, asset) in assets.iter().enumerate() {
          let source = BalanceDependencySources::<Test>::get(asset).unwrap();
          let next_page = DependencyRegistrationHeaders::<Test>::get(source).next_index / 32;
          let desired = retained_pages[source_index][page as usize];
          assert!(next_page <= desired);
          for gap_page in next_page..desired {
            let before = DependencyRegistrationHeaders::<Test>::get(source);
            let holes = (0..before.free_count)
              .map(|index| DependencyRegistrationFreePositions::<Test>::get(source, index).unwrap())
              .collect::<Vec<_>>();
            let gap = contract(&[*asset]);
            let mut transient = Vec::new();
            for index in 0..32 {
              let (actor, slot) = park(gap.clone(), slots.get(index).copied());
              if index == slots.len() {
                slots.push(slot);
              }
              transient.push(actor);
            }
            for actor in transient {
              assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor));
            }
            let after = DependencyRegistrationHeaders::<Test>::get(source);
            assert_eq!(after.next_index, before.next_index + 32);
            assert_eq!(
              (after.count, after.free_count),
              (before.count, before.free_count)
            );
            assert!(!DependencyRegistrationPages::<Test>::contains_key(
              source, gap_page
            ));
            assert!(DependencyScanSourceNodes::<Test>::contains_key(source));
            for (index, position) in holes.into_iter().enumerate() {
              assert_eq!(
                DependencyRegistrationFreePositions::<Test>::get(source, index as u64),
                Some(position)
              );
              assert_eq!(
                crate::DependencyRegistrationFreeIndices::<Test>::get(source, position),
                Some(index as u64)
              );
            }
          }
        }
      }
      let (actor, _) = park(shared.clone(), None);
      permanent.push(actor);
      for (source_index, asset) in assets.iter().enumerate() {
        let source = BalanceDependencySources::<Test>::get(asset).unwrap();
        assert_eq!(
          DependencyRegistrationPositions::<Test>::get(source, actor),
          Some(DependencyRegistrationPosition {
            page: retained_pages[source_index][page as usize],
            slot: 0
          }),
        );
        // Declared host occurrence; this is public lifecycle setup, not a complete producer block.
        assert_ok!(with_transaction_unchecked(|| TransactionOutcome::Commit(
          Actors::publish_balance_dependency_event(*asset),
        )));
      }
      let mut transient = Vec::new();
      for index in 0..31 {
        let (actor, slot) = park(shared.clone(), slots.get(index).copied());
        if index == slots.len() {
          slots.push(slot);
        }
        transient.push(actor);
      }
      for actor in transient {
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor));
      }
      assert_eq!(
        Actors::actor_identity_count(),
        u32::try_from(page + 1).unwrap()
      );
      for (source_index, asset) in assets.iter().enumerate() {
        let source = BalanceDependencySources::<Test>::get(asset).unwrap();
        let header = DependencyRegistrationHeaders::<Test>::get(source);
        assert_eq!(
          (header.count, header.free_count),
          (u32::try_from(page + 1).unwrap(), 31 * (page + 1))
        );
        for index in 0..header.free_count {
          let expected = DependencyRegistrationPosition {
            page: retained_pages[source_index][(index / 31) as usize],
            slot: (index % 31 + 1) as u8,
          };
          assert_eq!(
            DependencyRegistrationFreePositions::<Test>::get(source, index),
            Some(expected)
          );
          assert_eq!(
            crate::DependencyRegistrationFreeIndices::<Test>::get(source, expected),
            Some(index)
          );
        }
      }
    }
    assert_eq!(slots.len(), 32);
    assert_eq!(Actors::system_sovereign_count(), 35);
    assert_eq!(Actors::active_actor_count(), 3);
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().unwrap();
    for actor in permanent {
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor));
    }
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(DependencyScanSourceListState::<Test>::get().count, 0);
  });
}

#[test]
fn parked_dependency_interior_release_replays_complete_committed_root() {
  use polkadot_sdk::{
    frame_support::storage::{TransactionOutcome, with_transaction_unchecked},
    sp_core::{Blake2Hasher, H256},
    sp_runtime::StateVersion,
    sp_trie::{LayoutV1, delta_trie_root, recorder::Recorder},
  };
  use std::collections::BTreeSet;

  fn snapshot() -> BTreeMap<Vec<u8>, Vec<u8>> {
    let mut pairs = BTreeMap::new();
    let mut cursor = Vec::new();
    while let Some(key) = polkadot_sdk::sp_io::storage::next_key(&cursor) {
      pairs.insert(
        key.clone(),
        polkadot_sdk::sp_io::storage::get(&key).unwrap().to_vec(),
      );
      cursor = key;
    }
    pairs
  }

  let mut ext = new_test_ext();
  let (actor, evidence, state, plan, retained, allocator, event_count) = ext.execute_with(|| {
    System::set_block_number(1);
    let watches = <Test as crate::Config>::MaxParkedBalanceWatches::get();
    let mut contract = system_active_contract(
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    )
    .unwrap();
    contract.parked_balance_activation = Some(
      ParkedBalanceActivationOf::<Test>::try_from_rules(
        (0..watches)
          .map(|index| ParkedBalanceRule {
            asset: TestAsset::Local(100 + index),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    );
    let mut actors = Vec::new();
    for _ in 0..128 {
      actors.push(Actors::next_actor_id());
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        Some(contract.clone()),
      ));
    }
    let actor_id = actors[32];
    // Thirty-one local holes precede thirty-two outside holes in the dense stack.
    for index in (33..64).chain(64..95).chain(std::iter::once(96)) {
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actors[index]));
    }
    let actor = Actors::load_actor_ref(actor_id).unwrap();
    let state = Actors::active_actor_state(actor_id).unwrap();
    let process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let Some(ProcessResidence::Parked(evidence)) = process.residence else {
      panic!("public creation arms the Park residence")
    };
    let plan = DependencyPlans::<Test>::get(actor_id);
    assert_eq!(plan.len(), watches as usize);
    for entry in &plan {
      let source = entry.source;
      assert_eq!(
        DependencyRegistrationPositions::<Test>::get(source, actor_id),
        Some(DependencyRegistrationPosition { page: 1, slot: 0 }),
      );
      let header = DependencyRegistrationHeaders::<Test>::get(source);
      assert_eq!((header.count, header.free_count), (65, 63));
      for slot in 1..32u8 {
        assert_eq!(
          crate::DependencyRegistrationFreeIndices::<Test>::get(
            source,
            DependencyRegistrationPosition { page: 1, slot },
          ),
          Some(u64::from(slot - 1)),
        );
      }
      for index in 31..63u64 {
        assert_ne!(
          DependencyRegistrationFreePositions::<Test>::get(source, index)
            .unwrap()
            .page,
          1,
        );
      }
      let asset = DependencySourceBalances::<Test>::get(source).unwrap();
      assert_ok!(with_transaction_unchecked(|| TransactionOutcome::Commit(
        Actors::publish_balance_dependency_event(asset)
      )));
      assert!(DependencyScanSourceNodes::<Test>::contains_key(source));
    }
    let retained = ActorSemanticStates::<Test>::get(actor_id).unwrap();
    (
      actor,
      evidence,
      state,
      plan,
      retained,
      crate::DependencySourceAllocatorState::<Test>::get(),
      System::event_count(),
    )
  });
  ext.commit_all().unwrap();
  let (before, pre_root, allowed_changes, custody) = ext.execute_with(|| {
    let deadline = DeadlineHandles::<Test>::get(actor.actor_id).unwrap();
    let mut allowed_changes = BTreeSet::from([
      DependencyPlans::<Test>::hashed_key_for(actor.actor_id),
      DependencyTimedReviews::<Test>::hashed_key_for(actor.actor_id),
      PendingDependencyEvents::<Test>::hashed_key_for(actor.actor_id),
      PendingDependencyReviews::<Test>::hashed_key_for(actor.actor_id),
      PendingCheckOwners::<Test>::hashed_key_for(actor.actor_id),
      ParkedBalanceEpisodes::<Test>::hashed_key_for(actor.actor_id),
      DeadlineHandles::<Test>::hashed_key_for(actor.actor_id),
      DeadlineHeaders::<Test>::hashed_key_for(deadline.key),
    ]);
    for page in 0..4u64 {
      allowed_changes.insert(DeadlinePages::<Test>::hashed_key_for(deadline.key, page));
    }
    for entry in &plan {
      let source = entry.source;
      allowed_changes.extend([
        DependencyRegistrationHeaders::<Test>::hashed_key_for(source),
        DependencyRegistrations::<Test>::hashed_key_for(source, actor.actor_id),
        DependencyRegistrationPositions::<Test>::hashed_key_for(source, actor.actor_id),
      ]);
      for page in 0..3u64 {
        allowed_changes.insert(DependencyRegistrationPages::<Test>::hashed_key_for(
          source, page,
        ));
      }
      for index in 0..64u64 {
        allowed_changes.insert(DependencyRegistrationFreePositions::<Test>::hashed_key_for(
          source, index,
        ));
      }
      for slot in 0..32u8 {
        allowed_changes.insert(
          crate::DependencyRegistrationFreeIndices::<Test>::hashed_key_for(
            source,
            DependencyRegistrationPosition { page: 1, slot },
          ),
        );
      }
      for index in 32..63u64 {
        let position = DependencyRegistrationFreePositions::<Test>::get(source, index).unwrap();
        allowed_changes.insert(
          crate::DependencyRegistrationFreeIndices::<Test>::hashed_key_for(source, position),
        );
      }
    }
    let custody = plan
      .iter()
      .map(|entry| {
        let asset = DependencySourceBalances::<Test>::get(entry.source).unwrap();
        (
          asset,
          MockAssetOps::total_balance(&state.identity.sovereign_account, asset),
        )
      })
      .collect::<Vec<_>>();
    (
      snapshot(),
      H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1)),
      allowed_changes,
      custody,
    )
  });
  let recorder = Recorder::<Blake2Hasher>::default();
  let post_root = ext.execute_with_recorder(recorder.clone(), || {
    // This is the loaded Parked-release owner, including its primary Deadline removal.
    assert_ok!(with_transaction_unchecked(|| {
      TransactionOutcome::Commit(Actors::release_parked_dependency_authority(actor, evidence))
    }));
    H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1))
  });
  let proof = recorder.to_storage_proof();
  let after = ext.execute_with(|| {
    assert_eq!(
      ActorSemanticStates::<Test>::get(actor.actor_id),
      Some(retained.clone())
    );
    assert_eq!(
      crate::DependencySourceAllocatorState::<Test>::get(),
      allocator
    );
    assert_eq!(System::event_count(), event_count);
    for (asset, balance) in &custody {
      assert_eq!(
        MockAssetOps::total_balance(&state.identity.sovereign_account, *asset),
        *balance
      );
    }
    assert!(!DependencyPlans::<Test>::contains_key(actor.actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
    for entry in &plan {
      let source = entry.source;
      let header = DependencyRegistrationHeaders::<Test>::get(source);
      assert_eq!((header.count, header.free_count), (64, 32));
      assert!(!DependencyRegistrationPages::<Test>::contains_key(
        source, 1
      ));
      assert_eq!(
        DependencyRegistrationPages::<Test>::get(source, 0)
          .unwrap()
          .next,
        Some(2)
      );
      assert_eq!(
        DependencyRegistrationPages::<Test>::get(source, 2)
          .unwrap()
          .previous,
        Some(0)
      );
      assert!(DependencyScanSourceNodes::<Test>::contains_key(source));
      for index in 0..32u64 {
        let position = DependencyRegistrationFreePositions::<Test>::get(source, index).unwrap();
        assert_ne!(position.page, 1);
        assert_eq!(
          crate::DependencyRegistrationFreeIndices::<Test>::get(source, position),
          Some(index)
        );
      }
      for slot in 0..32u8 {
        assert!(
          !crate::DependencyRegistrationFreeIndices::<Test>::contains_key(
            source,
            DependencyRegistrationPosition { page: 1, slot },
          )
        );
      }
    }
    snapshot()
  });
  let delta = before
    .keys()
    .chain(after.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|key| before.get(*key) != after.get(*key))
    .map(|key| (key.clone(), after.get(key).cloned()))
    .collect::<Vec<_>>();
  assert!(delta.iter().all(|(key, _)| allowed_changes.contains(key)));
  for entry in &plan {
    let source = entry.source;
    for prefix in [
      DependencyRegistrationFreePositions::<Test>::hashed_key_for(source, 0),
      crate::DependencyRegistrationFreeIndices::<Test>::hashed_key_for(
        source,
        DependencyRegistrationPosition { page: 0, slot: 0 },
      ),
    ] {
      let changes = delta
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix[..56]))
        .collect::<Vec<_>>();
      assert_eq!(changes.len(), 62);
      assert!(changes.iter().all(|(key, _)| before.contains_key(key)));
      assert_eq!(
        changes.iter().filter(|(_, value)| value.is_none()).count(),
        31
      );
    }
  }
  println!(
    "Parked interior release: watches={} committed_proof_bytes={} net_keys={}",
    plan.len(),
    proof.encoded_size(),
    delta.len(),
  );
  assert_ne!(pre_root, post_root);
  let mut proof_database = proof.into_memory_db::<Blake2Hasher>();
  assert_eq!(
    delta_trie_root::<LayoutV1<Blake2Hasher>, _, _, _, _, _>(
      &mut proof_database,
      pre_root,
      delta,
      None,
      None,
    )
    .unwrap(),
    post_root,
  );
  ext.execute_with(|| {
    let ActorSemanticState::Active(record) = retained else {
      panic!("release retains active semantic authority")
    };
    ActorProcesses::<Test>::remove(actor.actor_id);
    assert_ok!(Actors::finalize_actor_from_consumed_state(
      actor.actor_id,
      state,
      &record.admission,
      CloseReason::OwnerInitiated,
    ));
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().unwrap();
  });
}

#[test]
fn parked_public_close_free_stack_slot_order_preserves_custody_and_caller_root() {
  use polkadot_sdk::{
    frame_support::storage::{TransactionOutcome, with_transaction_unchecked},
    sp_runtime::StateVersion,
  };

  for target_slot in [0u8, 31] {
    for rollback in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let asset = TestAsset::Local(101);
        let mut contract = system_active_contract(
          manual_schedule(),
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        )
        .unwrap();
        contract.parked_balance_activation = Some(
          ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
            asset,
            authored_min_delta: 100,
          }])
          .unwrap(),
        );
        let mut actors = Vec::new();
        for _ in 0..128 {
          actors.push(Actors::next_actor_id());
          assert_ok!(Actors::create_system_actor(
            RuntimeOrigin::root(),
            ALICE,
            Mutability::Mutable,
            Some(contract.clone()),
          ));
        }
        let target = actors[32 + usize::from(target_slot)];
        for index in (32..64)
          .filter(|index| *index != 32 + usize::from(target_slot))
          .chain(64..95)
          .chain(std::iter::once(96))
        {
          assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actors[index]));
        }
        let source = DependencyPlans::<Test>::get(target)[0].source;
        let position = DependencyRegistrationPosition { page: 1, slot: target_slot };
        assert_eq!(
          DependencyRegistrationPositions::<Test>::get(source, target),
          Some(position),
        );
        let header = DependencyRegistrationHeaders::<Test>::get(source);
        assert_eq!((header.count, header.free_count), (65, 63));
        let account = Actors::actor_identity(target).unwrap().sovereign_account;
        assert_ok!(MockAssetOps::mint(&account, TestAsset::Native, 77));
        // The mock ledger does not publish automatically; the ordinary producer owns this ingress.
        assert!(!crate::SovereignIndex::<Test>::contains_key(BOB));
        let revision = DependencyRevisions::<Test>::get(source).revision;
        assert_ok!(MockAssetOps::mint(&BOB, asset, 1));
        assert_ok!(with_transaction_unchecked(|| TransactionOutcome::Commit(
          Actors::publish_balance_dependency_event(asset)
        )));
        assert_eq!(DependencyRevisions::<Test>::get(source).revision, revision + 1);
        let scan = DependencyRevisions::<Test>::get(source);
        assert!(scan.scan_target.is_some());
        let peers = actors.iter().filter_map(|id| {
          (*id != target && Actors::actor_identity(*id).is_some())
            .then(|| (*id, Actors::active_actor_state(*id).unwrap()))
        }).collect::<Vec<_>>();
        assert_eq!(peers.len(), 64);
        let slots = crate::SystemSovereignCount::<Test>::get();

        // Diagnostic sequence only: no production hook/counter or storage injection.
        let mut expected = (0..header.free_count)
          .map(|index| DependencyRegistrationFreePositions::<Test>::get(source, index).unwrap())
          .collect::<Vec<_>>();
        expected.push(position);
        let mut non_tail = 0;
        let mut moved_fresh = false;
        for slot in 0..32u8 {
          let hole = DependencyRegistrationPosition { page: 1, slot };
          let index = expected.iter().position(|entry| *entry == hole).unwrap();
          if index != expected.len() - 1 {
            non_tail += 1;
            moved_fresh |= expected.last() == Some(&position);
          }
          expected.swap_remove(index);
        }
        assert_eq!(non_tail, if target_slot == 0 { 31 } else { 32 });
        assert_eq!(moved_fresh, target_slot != 0);
        assert_eq!(expected.len(), 32);
        let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        assert_ok!(with_transaction_unchecked(|| {
          assert_ok!(Actors::close_actor(RuntimeOrigin::root(), target));
          assert!(Actors::actor_identity(target).is_none());
          let after = DependencyRegistrationHeaders::<Test>::get(source);
          assert_eq!((after.count, after.free_count), (64, 32));
          assert!(!DependencyRegistrationPages::<Test>::contains_key(source, 1));
          assert_eq!(DependencyRegistrationPages::<Test>::get(source, 0).unwrap().next, Some(2));
          assert_eq!(DependencyRegistrationPages::<Test>::get(source, 2).unwrap().previous, Some(0));
          for (index, hole) in expected.iter().enumerate() {
            assert_ne!(hole.page, 1);
            assert_eq!(DependencyRegistrationFreePositions::<Test>::get(source, index as u64), Some(*hole));
            assert_eq!(crate::DependencyRegistrationFreeIndices::<Test>::get(source, *hole), Some(index as u64));
          }
          for slot in 0..32u8 {
            assert!(!crate::DependencyRegistrationFreeIndices::<Test>::contains_key(
              source, DependencyRegistrationPosition { page: 1, slot },
            ));
          }
          assert_eq!(DependencyRevisions::<Test>::get(source), scan);
          assert_eq!(crate::SystemSovereignCount::<Test>::get(), slots);
          assert_eq!(MockAssetOps::total_balance(&account, TestAsset::Native), 77);
          for (id, state) in &peers {
            assert_eq!(Actors::active_actor_state(*id), Some(state.clone()));
          }
          #[cfg(feature = "try-runtime")]
          Actors::do_try_state().unwrap();
          if rollback {
            TransactionOutcome::Rollback(Ok::<(), polkadot_sdk::sp_runtime::DispatchError>(()))
          } else {
            TransactionOutcome::Commit(Ok::<(), polkadot_sdk::sp_runtime::DispatchError>(()))
          }
        }));
        if rollback {
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
          assert!(Actors::actor_identity(target).is_some());
          assert_eq!(DependencyRegistrationHeaders::<Test>::get(source), header);
        } else {
          assert_ne!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
        }
        println!("public free-stack order: target_slot={target_slot} non_tail={non_tail} moved_fresh={moved_fresh} rollback={rollback}");
      });
    }
  }
}

#[test]
fn dependency_reclamation_preserves_sparse_scan_frontiers_holes_and_rollback() {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};
  new_test_ext().execute_with(|| {
    with_transaction_unchecked(|| {
      let asset = TestAsset::Local(101);
      let crate::DependencySourceMutation::Allocated(source) =
        Actors::resolve_balance_dependency_source(asset).unwrap()
      else {
        panic!("fresh source")
      };
      for index in 0..97u64 {
        let owner = PendingCheckOwner {
          actor: actor_ref(4000 + index, 1),
          plan_revision: 1,
        };
        PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
        Actors::install_dependency_registration(source, owner, 0).unwrap();
      }
      Actors::publish_dependency_event_with_source_retention(source).unwrap();
      for index in (32..64u64)
        .chain(0..31)
        .chain(std::iter::once(64))
        .chain(std::iter::once(31))
      {
        let handle = DependencyRegistrations::<Test>::get(source, 4000 + index).unwrap();
        Actors::remove_dependency_registration(source, handle).unwrap();
      }
      assert!(!DependencyRegistrationPages::<Test>::contains_key(
        source, 0
      ));
      assert!(!DependencyRegistrationPages::<Test>::contains_key(
        source, 1
      ));
      let header = DependencyRegistrationHeaders::<Test>::get(source);
      assert_eq!(header.first_page, Some(2));
      assert_eq!(header.last_page, Some(3));
      assert_eq!(header.count, 32);
      assert_eq!(header.free_count, 1);
      let hole = DependencyRegistrationPosition { page: 2, slot: 0 };
      assert_eq!(
        DependencyRegistrationFreePositions::<Test>::get(source, 0),
        Some(hole)
      );
      assert_eq!(
        crate::DependencyRegistrationFreeIndices::<Test>::get(source, hole),
        Some(0)
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 64);
      let appended = PendingCheckOwner {
        actor: actor_ref(4097, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(appended.actor.actor_id, appended);
      Actors::install_dependency_registration(source, appended, 0).unwrap();
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 97);
      assert_eq!(
        DependencyRegistrationPositions::<Test>::get(source, 4097),
        Some(DependencyRegistrationPosition { page: 3, slot: 1 })
      );
      Actors::publish_dependency_event_with_source_retention(source).unwrap();
      while DependencyRevisions::<Test>::get(source).scan_cursor < 97 {
        let state = DependencyRevisions::<Test>::get(source);
        Actors::process_dependency_scan_member(source, 1, state.scan_cursor).unwrap();
      }
      assert!(!PendingDependencyEvents::<Test>::contains_key(4097));
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 97),
        Ok(DependencyScanMutation::HandedOff(2))
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 64);
      let mut meter = WeightMeter::new();
      while DependencyRevisions::<Test>::get(source)
        .scan_target
        .is_some()
      {
        Actors::process_next_dependency_scan_unit(&mut meter).unwrap();
      }
      for index in 65..98u64 {
        assert!(PendingDependencyEvents::<Test>::contains_key(4000 + index));
        assert_eq!(
          DependencyRegistrations::<Test>::get(source, 4000 + index)
            .unwrap()
            .acknowledged_revision,
          2
        );
      }
      let reused = PendingCheckOwner {
        actor: actor_ref(4098, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(reused.actor.actor_id, reused);
      Actors::install_dependency_registration(source, reused, 2).unwrap();
      assert_eq!(
        DependencyRegistrationPositions::<Test>::get(source, 4098),
        Some(hole)
      );
      assert_eq!(
        DependencyRegistrationHeaders::<Test>::get(source).next_index,
        98
      );
      assert_eq!(
        DependencyRegistrationHeaders::<Test>::get(source).free_count,
        0
      );
      for index in 65..98u64 {
        let handle = DependencyRegistrations::<Test>::get(source, 4000 + index).unwrap();
        Actors::remove_dependency_registration(source, handle).unwrap();
      }
      Actors::publish_dependency_event_with_source_retention(source).unwrap();
      let handle = DependencyRegistrations::<Test>::get(source, 4098).unwrap();
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      with_transaction_unchecked(|| {
        Actors::remove_dependency_registration(source, handle).unwrap();
        TransactionOutcome::Rollback(())
      });
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root
      );
      Actors::remove_dependency_registration(source, handle).unwrap();
      assert!(!BalanceDependencySources::<Test>::contains_key(asset));
      assert!(!DependencySourceBalances::<Test>::contains_key(source));
      assert!(!DependencyRevisions::<Test>::contains_key(source));
      assert!(!DependencyRegistrationHeaders::<Test>::contains_key(source));
      assert_eq!(DependencyScanSourceListState::<Test>::get().count, 0);
      assert!(!DependencyScanSourceNodes::<Test>::contains_key(source));
      assert_eq!(
        Actors::resolve_balance_dependency_source(asset).unwrap(),
        crate::DependencySourceMutation::Allocated(source + 1)
      );
      TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn dependency_scan_reclaims_last_stale_page_without_restoring_source_ring() {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};
  new_test_ext().execute_with(|| {
    with_transaction_unchecked(|| {
      let mut sources = Vec::new();
      for index in 0..2u32 {
        let crate::DependencySourceMutation::Allocated(source) =
          Actors::resolve_balance_dependency_source(TestAsset::Local(110 + index)).unwrap()
        else {
          panic!("fresh source")
        };
        let owner = PendingCheckOwner {
          actor: actor_ref(4200 + u64::from(index), 1),
          plan_revision: 1,
        };
        PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
        Actors::install_dependency_registration(source, owner, 0).unwrap();
        Actors::publish_dependency_event_with_source_retention(source).unwrap();
        sources.push(source);
      }
      DependencyRegistrations::<Test>::remove(sources[0], 4200);
      let mut meter = WeightMeter::new();
      assert_eq!(
        Actors::process_next_dependency_scan_unit(&mut meter),
        Ok(Some(DependencyScanMutation::Advanced(1)))
      );
      assert!(!BalanceDependencySources::<Test>::contains_key(
        TestAsset::Local(110)
      ));
      assert!(!DependencyRegistrationHeaders::<Test>::contains_key(
        sources[0]
      ));
      assert!(!DependencyRevisions::<Test>::contains_key(sources[0]));
      assert!(!DependencyScanSourceNodes::<Test>::contains_key(sources[0]));
      assert_eq!(
        DependencyScanSourceListState::<Test>::get(),
        DependencyScanSourceList {
          cursor: Some(sources[1]),
          count: 1
        }
      );
      Actors::process_next_dependency_scan_unit(&mut meter).unwrap();
      assert!(PendingDependencyEvents::<Test>::contains_key(4201));
      TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn completed_authored_contract_captures_anchor_and_enters_park() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let activation = ParkedBalanceActivationOf::<Test>::try_from_rules(vec![
      ParkedBalanceRule {
        asset: TestAsset::Native,
        authored_min_delta: 100,
      },
      ParkedBalanceRule {
        asset: TestAsset::Local(9),
        authored_min_delta: 100,
      },
    ])
    .expect("canonical activation");
    let mut contract = system_active_contract(
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    )
    .expect("active Contract");
    contract.parked_balance_activation = Some(activation);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(contract),
    ));
    let actor = Actors::load_actor_ref(actor_id).expect("Actor reference");
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    let service = Actors::service_canonical_round_head_inner(
      &mut meter,
      2,
      None,
      BlockResourceDomain::ActorControl,
      None,
    );
    assert_eq!(
      service.map(|(encounter, attempt)| (encounter, attempt.is_some())),
      Ok((ServiceRoundEncounter::Eligible(actor), true))
    );

    let episode = ParkedBalanceEpisodes::<Test>::get(actor_id)
      .expect("completed Contract owns one parked-balance episode");
    assert_eq!(episode.owner.actor, actor);
    assert_eq!(episode.owner.plan_revision, 1);
    assert_eq!(episode.watches.len(), 2);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(3))
    );
    assert!(matches!(
      ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence),
      Some(ProcessResidence::Parked(ParkEvidence {
        reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
        review_at: Some(3),
        ..
      }))
    ));

    let native_source = BalanceDependencySources::<Test>::get(TestAsset::Native)
      .expect("native watch owns a causal source");
    let local_source = BalanceDependencySources::<Test>::get(TestAsset::Local(9))
      .expect("non-native watch owns a causal source");
    let sovereign = Actors::actor_identity(actor_id)
      .expect("Actor identity")
      .sovereign_account;
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      set_asset_balance(&sovereign, TestAsset::Native, 25);
      let result = <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(
        TestAsset::Native,
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
    })
    .expect("committed native mutation publishes its causal revision");
    assert_eq!(DependencyRevisions::<Test>::get(native_source).revision, 1);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(native_source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(native_source, 1, 1),
        Ok(DependencyScanMutation::Completed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      PendingDependencyEvents::<Test>::get(actor_id),
      Some(PendingDependencyEvent {
        owner: episode.owner,
        source: native_source,
        revision: 1,
      })
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      set_asset_balance(&sovereign, TestAsset::Local(9), 40);
      let result = <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(
        TestAsset::Local(9),
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
    })
    .expect("committed non-native mutation publishes its causal revision");
    assert_eq!(DependencyRevisions::<Test>::get(local_source).revision, 1);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(local_source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(local_source, 1, 1),
        Ok(DependencyScanMutation::Completed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      PendingDependencyEvents::<Test>::get(actor_id),
      Some(PendingDependencyEvent {
        owner: episode.owner,
        source: native_source,
        revision: 1,
      }),
      "a racing asset revision preserves the first current Pending obligation"
    );
    assert_eq!(
      DependencyRegistrations::<Test>::get(local_source, actor_id)
        .expect("non-native registration remains live")
        .acknowledged_revision,
      1,
      "the later source is acknowledged only after its scan preserves the complete Pending recheck"
    );
    let Some(ProcessResidence::Parked(evidence)) =
      ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence)
    else {
      panic!("completed Actor remains parked");
    };
    let first_pending = PendingDependencyEvents::<Test>::get(actor_id).expect("Pending event");
    let event_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::process_pending_parked_balance_event();
    let mut no_event_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_pending_parked_balance_event(
        &mut no_event_weight,
        first_pending,
        ServiceResidenceKind::Live,
        evidence,
        2,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert_eq!(PendingDependencyEvents::<Test>::get(actor_id), Some(first_pending));
    assert!(ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    let mut negative_event_meter = WeightMeter::with_limit(event_weight);
    assert_eq!(
      Actors::process_pending_parked_balance_event(
        &mut negative_event_meter,
        first_pending,
        ServiceResidenceKind::Live,
        evidence,
        2,
        None,
      ),
      Ok(DependencyReviewMutation::Rearmed(DependencyPlanMutation {
        retained: 2,
        timed_review: DependencyTimedReviewMutation::Retained,
        ..Default::default()
      }))
    );
    assert_eq!(
      ParkedBalanceEpisodes::<Test>::get(actor_id)
        .expect("subthreshold current recheck retains episode")
        .watches,
      episode.watches
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      set_asset_balance(&sovereign, TestAsset::Native, 100);
      let result = <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(
        TestAsset::Native,
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
    })
    .expect("threshold native mutation publishes a second revision");
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(native_source, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(native_source, 2, 1),
        Ok(DependencyScanMutation::Completed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert!(PendingDependencyEvents::<Test>::contains_key(actor_id));
    frame_system::Pallet::<Test>::set_block_number(3);
    let due_weight = Actors::due_dependency_review_weight_upper();
    let mut positive_event_meter = WeightMeter::with_limit(due_weight);
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut positive_event_meter,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Ok((actor, DependencyReviewMutation::Woke))
    );
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).map(|node| node.eligible_from),
      Some(4)
    );
  });
}

#[test]
fn balance_causal_ingress_saturation_rolls_back_revision_publication() {
  new_test_ext().execute_with(|| {
    let asset = TestAsset::Local(77);
    let source = 900;
    BalanceDependencySources::<Test>::insert(asset, source);
    DependencySourceBalances::<Test>::insert(source, asset);
    DependencyScanSourceListState::<Test>::put(DependencyScanSourceList {
      cursor: Some(901),
      count: <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get()
        .saturating_mul(<<Test as crate::Config>::MaxContractSteps as Get<u32>>::get()),
    });
    let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      set_asset_balance(&ALICE, asset, 500);
      match <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(asset) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    });
    assert_eq!(
      outcome,
      Err(DispatchError::Other(
        "dependency scan source capacity reached"
      ))
    );
    assert_eq!(DependencyRevisions::<Test>::get(source).revision, 0);
    assert!(!DependencyScanSourceNodes::<Test>::contains_key(source));
  });
}

#[test]
fn parked_balance_activation_captures_one_atomic_host_owned_anchor_set() {
  new_test_ext().execute_with(|| {
    set_asset_balance(&ALICE, TestAsset::Local(9), 700);
    let activation = ParkedBalanceActivationOf::<Test>::try_from_rules(vec![
      ParkedBalanceRule {
        asset: TestAsset::Local(9),
        authored_min_delta: 200,
      },
      ParkedBalanceRule {
        asset: TestAsset::Native,
        authored_min_delta: 100,
      },
    ])
    .expect("bounded unique activation certifies");
    let watches = Actors::certify_parked_balance_activation(&ALICE, &activation, 11)
      .expect("host minima and total balances certify");
    assert_eq!(watches.len(), 2);
    for watch in watches
      .iter(/* deos-bypass: bounded-iter -- test activation has two watches. */)
    {
      assert_eq!(watch.acknowledged_revision, 11);
      assert_eq!(
        watch.anchor,
        MockAssetOps::total_balance(&ALICE, watch.asset),
        "every anchor uses total ownership from the same certification transition"
      );
      assert_eq!(
        watch.certified_minimum_balance,
        MockAssetOps::minimum_balance(watch.asset)
      );
    }
  });
}

#[test]
fn parked_balance_episode_publishes_atomically_and_positive_review_reclaims_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    set_asset_balance(&sovereign_account(actor_id), TestAsset::Local(9), 1_000);
    let mut contract =
      system_active_contract(manual_schedule(), None, inert_contract_steps()).unwrap();
    contract.parked_balance_activation = Some(
      ParkedBalanceActivationOf::<Test>::try_from_rules(vec![
        ParkedBalanceRule {
          asset: TestAsset::Native,
          authored_min_delta: 100,
        },
        ParkedBalanceRule {
          asset: TestAsset::Local(9),
          authored_min_delta: 200,
        },
      ])
      .unwrap(),
    );
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      contract
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let episode =
      ParkedBalanceEpisodes::<Test>::get(actor_id).expect("installation retains episode");
    let owner = episode.owner;
    assert_eq!(episode.owner, owner);
    assert_eq!(episode.watches.len(), 2);
    let fixed_anchors = episode.watches.clone();
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
      review_at: Some(2),
    };
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    ParkedBalanceEpisodes::<Test>::remove(actor_id);
    let missing_root =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Err(ActorSemanticLoadError::Corrupt)
    );
    assert_eq!(
      Actors::release_parked_dependency_authority(actor, evidence),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      missing_root
    );
    ParkedBalanceEpisodes::<Test>::insert(actor_id, episode.clone());
    let mut stale_episode = episode.clone();
    stale_episode.owner.plan_revision += 1;
    ParkedBalanceEpisodes::<Test>::insert(actor_id, stale_episode);
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Err(ActorSemanticLoadError::Corrupt),
      "an episode cannot borrow another plan revision"
    );
    ParkedBalanceEpisodes::<Test>::insert(actor_id, episode);
    assert!(Actors::load_canonical_actor_semantic_state(actor).is_ok());

    frame_system::Pallet::<Test>::set_block_number(2);
    let review_weight = Actors::due_dependency_review_weight_upper();
    let mut no_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut no_weight,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    set_asset_minimum_balance(2);
    let mut drift_meter = WeightMeter::with_limit(review_weight);
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut drift_meter,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::Interpretation(
        DependencyRegistrationError::ParkedBalanceMinimumChanged
      ))
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      ParkedBalanceEpisodes::<Test>::get(actor_id)
        .expect("minimum drift refusal retains episode")
        .watches,
      fixed_anchors
    );
    set_asset_minimum_balance(1);
    let mut negative_meter = WeightMeter::with_limit(review_weight);
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut negative_meter,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Ok((
        actor,
        DependencyReviewMutation::Rearmed(DependencyPlanMutation {
          retained: 2,
          timed_review: DependencyTimedReviewMutation::Installed,
          ..Default::default()
        })
      ))
    );
    assert_eq!(
      ParkedBalanceEpisodes::<Test>::get(actor_id)
        .expect("negative review retains episode")
        .watches,
      fixed_anchors,
      "negative current checks keep the fixed episode anchors"
    );
    assert!(
      !PendingDependencyReviews::<Test>::contains_key(actor_id),
      "negative review consumes its transient marker before committing"
    );
    let rearmed_evidence = ParkEvidence {
      review_at: Some(3),
      ..evidence
    };
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(rearmed_evidence)))
    );
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(3))
    );

    let source = BalanceDependencySources::<Test>::get(TestAsset::Local(9)).unwrap();
    let acknowledged = DependencyRegistrations::<Test>::get(source, actor_id)
      .unwrap()
      .acknowledged_revision;
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      MockAssetOps::transfer(
        &record.identity.sovereign_account,
        &BOB,
        TestAsset::Local(9),
        200,
      )
      .expect("reverse movement commits");
      assert_ok!(Actors::publish_balance_dependency_event(TestAsset::Local(
        9
      )));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert!(DependencyRevisions::<Test>::get(source).revision > acknowledged);
    assert!(!PendingDependencyEvents::<Test>::contains_key(actor_id));
    frame_system::Pallet::<Test>::set_block_number(3);
    let mut positive_meter = WeightMeter::with_limit(review_weight);
    assert_eq!(
      Actors::process_next_due_block_dependency_review(
        &mut positive_meter,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Ok((actor, DependencyReviewMutation::Woke))
    );
    assert!(
      !PendingDependencyReviews::<Test>::contains_key(actor_id),
      "positive review consumes its transient marker before committing"
    );
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
  });
}

#[test]
fn parked_balance_cleanup_covers_replacement_deactivation_and_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(before) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, before.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let activation = ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
      asset: TestAsset::Native,
      authored_min_delta: 100,
    }])
    .unwrap();
    Actors::transfer_service_member_to_parked_balance(
      actor,
      ServiceResidenceKind::Live,
      8,
      &activation,
      WakeupKey::Block(2),
    )
    .unwrap();

    let mut replacement = Actors::load_actor_contract(actor_id).unwrap();
    replacement.cooldown_blocks = 7;
    assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
      || {
        let result = Actors::store_actor_contract(actor_id, replacement.clone());
        if result.is_ok() {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
        } else {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
        }
      }
    ));
    let ActorSemanticState::Active(after) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("replacement Actor is active");
    };
    assert_eq!(after.generation, before.generation + 1);
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyTimedReviews::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.generation),
      Some(after.generation),
      "replacement publishes only the new generation"
    );

    for deactivate in [true, false] {
      let lifecycle_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let ActorSemanticState::Active(lifecycle_record) =
        ActorSemanticStates::<Test>::get(lifecycle_id).unwrap()
      else {
        panic!("lifecycle Actor is active");
      };
      let lifecycle_actor = actor_ref(lifecycle_id, lifecycle_record.generation);
      ActorControlLocators::<Test>::remove(lifecycle_id);
      ActorUnsignaledControlCells::<Test>::remove(lifecycle_id);
      publish_test_service_member(lifecycle_actor, ServiceResidenceKind::Live, 1).unwrap();
      Actors::transfer_service_member_to_parked_balance(
        lifecycle_actor,
        ServiceResidenceKind::Live,
        9,
        &activation,
        WakeupKey::Block(2),
      )
      .unwrap();
      if deactivate {
        assert_ok!(Actors::deactivate_actor(
          RuntimeOrigin::root(),
          lifecycle_id
        ));
        assert!(matches!(
          ActorSemanticStates::<Test>::get(lifecycle_id),
          Some(ActorSemanticState::Dormant(_))
        ));
      } else {
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), lifecycle_id));
        assert!(!ActorSemanticStates::<Test>::contains_key(lifecycle_id));
      }
      assert!(!ActorProcesses::<Test>::contains_key(lifecycle_id));
      assert!(!ParkedBalanceEpisodes::<Test>::contains_key(lifecycle_id));
      assert!(!PendingCheckOwners::<Test>::contains_key(lifecycle_id));
      assert!(DependencyPlans::<Test>::get(lifecycle_id).is_empty());
      assert!(!DependencyTimedReviews::<Test>::contains_key(lifecycle_id));
      assert!(!DeadlineHandles::<Test>::contains_key(lifecycle_id));
    }
  });
}

#[test]
fn parked_balance_watch_uses_inclusive_absolute_delta_and_keeps_its_anchor() {
  let watch = certify_parked_balance_watch(7u32, 80u128, 1u128, 1_000u128, 4)
    .expect("nonzero representable minimum certifies");
  assert_eq!(watch.anchor, 1_000);
  assert_eq!(watch.acknowledged_revision, 4);
  assert_eq!(
    classify_parked_balance(&watch, 1, 1_099),
    Ok(ParkedBalanceQualification::BelowThreshold)
  );
  assert_eq!(
    classify_parked_balance(&watch, 1, 1_100),
    Ok(ParkedBalanceQualification::Qualified),
    "equality at max(authored delta, 100 * minimum) qualifies"
  );
  assert_eq!(
    classify_parked_balance(&watch, 1, 900),
    Ok(ParkedBalanceQualification::Qualified),
    "movement in the reverse direction uses the same fixed anchor"
  );
  assert_eq!(watch.anchor, 1_000, "classification cannot move the anchor");

  let authored_dominates = certify_parked_balance_watch(7u32, 150u128, 1u128, 1_000u128, 4)
    .expect("authored threshold certifies");
  assert_eq!(
    classify_parked_balance(&authored_dominates, 1, 1_149),
    Ok(ParkedBalanceQualification::BelowThreshold)
  );
  assert_eq!(
    classify_parked_balance(&authored_dominates, 1, 850),
    Ok(ParkedBalanceQualification::Qualified)
  );
}

#[test]
fn parked_balance_plan_checks_every_minimum_and_qualifies_any_absolute_delta() {
  let watches = [
    certify_parked_balance_watch(1u32, 100u128, 1u128, 1_000u128, 4).unwrap(),
    certify_parked_balance_watch(2u32, 500u128, 2u128, 2_000u128, 7).unwrap(),
  ];
  assert_eq!(
    classify_parked_balance_plan(&watches, |asset| match asset {
      1 => (1, 1_100),
      2 => (2, 2_100),
      _ => unreachable!(),
    }),
    Ok(ParkedBalanceQualification::Qualified)
  );
  assert_eq!(
    classify_parked_balance_plan(&watches, |asset| match asset {
      1 => (1, 1_100),
      2 => (3, 2_100),
      _ => unreachable!(),
    }),
    Err(ParkedBalanceClassificationError::MinimumBalanceChanged),
    "a later watch's drift is not hidden by an earlier qualification"
  );
}

#[test]
fn parked_balance_watch_rejects_invalid_or_changed_minimums_without_saturation() {
  assert_eq!(
    certify_parked_balance_watch(7u32, 1u128, 0u128, 1_000u128, 0),
    Err(ParkedBalanceCertificationError::ZeroMinimumBalance)
  );
  assert_eq!(
    certify_parked_balance_watch(7u32, 1u8, 3u8, 10u8, 0),
    Err(ParkedBalanceCertificationError::ThresholdOverflow)
  );
  let watch =
    certify_parked_balance_watch(7u32, 100u128, 1u128, 1_000u128, 0).expect("watch certifies");
  assert_eq!(
    classify_parked_balance(&watch, 2, 1_100),
    Err(ParkedBalanceClassificationError::MinimumBalanceChanged)
  );
}

#[test]
fn semantic_record_owns_only_non_derivable_state_and_projects_execution_geometry() {
  let record = ActorSemanticRecord {
    identity: 11u32,
    generation: 7,
    hot: 12u32,
    admission: 13u32,
  };
  assert_eq!(
    (
      record.identity,
      record.generation,
      record.hot,
      record.admission,
    ),
    (11, 7, 12, 13)
  );

  let resources = ActorStepResourceEnvelope {
    control: Weight::from_parts(17, 19),
    effect: Weight::from_parts(23, 29),
  };
  assert_eq!(
    project_actor_semantic_execution(CycleState::Idle, None::<(u32, u32)>, Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 0,
      eligible_at: None,
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, Some((3, 31u32)), Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 3,
      eligible_at: Some(31),
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Suspended, Some((4, 37u32)), Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 4,
      eligible_at: Some(37),
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Idle, Some((0, 31u32)), Some(resources)),
    Err(ActorSemanticProjectionError::RunStateMismatch)
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, None::<(u32, u32)>, Some(resources)),
    Err(ActorSemanticProjectionError::RunStateMismatch)
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, Some((3, 31u32)), None),
    Err(ActorSemanticProjectionError::CurrentStepMissing)
  );
}

#[test]
fn semantic_generation_is_nonzero_and_fails_closed_at_exhaustion() {
  assert_eq!(next_actor_generation(0), Some(1));
  assert_eq!(next_actor_generation(41), Some(42));
  assert_eq!(next_actor_generation(u64::MAX), None);
}

#[test]
fn semantic_mutation_is_complete_compare_and_replace_without_placement_authority() {
  let dormant = ActorSemanticState::Dormant(DormantActorSemanticRecord {
    identity: 1u32,
    generation: 0,
  });
  let active = ActorSemanticState::Active(ActorSemanticRecord {
    identity: 1u32,
    generation: 1,
    hot: 4u32,
    admission: 5u32,
  });
  let stale = ActorSemanticState::Active(ActorSemanticRecord {
    identity: 9u32,
    generation: 9,
    hot: 9u32,
    admission: 9u32,
  });

  assert_eq!(
    apply_actor_semantic_mutation(None, &ActorSemanticMutation::Publish(dormant.clone()),),
    Ok(Some(dormant.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Publish(dormant.clone()),
    ),
    Err(ActorSemanticMutationError::AlreadyPublished)
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Replace {
        expected: dormant.clone(),
        replacement: active.clone(),
      },
    ),
    Ok(Some(active.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Replace {
        expected: stale.clone(),
        replacement: active.clone(),
      },
    ),
    Err(ActorSemanticMutationError::Stale)
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&active),
      &ActorSemanticMutation::Replace {
        expected: active.clone(),
        replacement: dormant.clone(),
      },
    ),
    Ok(Some(dormant.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Remove {
        expected: dormant.clone(),
      },
    ),
    Ok(None)
  );
  assert_eq!(
    apply_actor_semantic_mutation(None, &ActorSemanticMutation::Remove { expected: dormant },),
    Err(ActorSemanticMutationError::Missing)
  );
}

#[test]
fn legacy_process_compiler_maps_exact_placements_and_refuses_unsignaled_guessing() {
  let disabled = ProcessDisablement {
    cause: ProcessDisableCause::OwnerPaused,
    revival_authority: ProcessRevivalAuthority::Owner,
    basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
  };
  let parked = ParkEvidence {
    plan_identity: [3; 32],
    reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
    review_at: Some(12u32),
  };
  let cases = [
    (
      LegacyProcessPlacement::Ready(ServiceResidenceKind::Live),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
      },
    ),
    (
      LegacyProcessPlacement::Ready(ServiceResidenceKind::Pending),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
      },
    ),
    (
      LegacyProcessPlacement::Waiting {
        key: WakeupKey::Block(9),
        page: 2,
        slot: 3,
      },
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Deadline {
          key: WakeupKey::Block(9),
          page: 2,
          slot: 3,
        }),
      },
    ),
    (
      LegacyProcessPlacement::Waiting {
        key: WakeupKey::Tick(10),
        page: 4,
        slot: 5,
      },
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Deadline {
          key: WakeupKey::Tick(10),
          page: 4,
          slot: 5,
        }),
      },
    ),
    (
      LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Parked(parked))),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Parked(parked)),
      },
    ),
    (
      LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Disabled(disabled))),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Disabled(disabled),
        residence: None,
      },
    ),
  ];
  for (placement, expected) in cases {
    assert_eq!(compile_legacy_process(11, Some(7), placement), Ok(expected));
  }
  assert_eq!(
    compile_legacy_process::<u32>(11, None, LegacyProcessPlacement::Unsignaled(None)),
    Err(ProcessCompileError::AmbiguousUnsignaled)
  );
}

#[test]
fn canonical_occurrence_planner_publishes_idle_pending_and_preserves_busy_residence() {
  let parked = ParkEvidence {
    plan_identity: [3; 32],
    reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
    review_at: Some(12u32),
  };
  let idle = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Parked(parked)),
  };
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, idle, 20),
    Ok(Some(CanonicalOccurrencePlan {
      process: ActorProcess {
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
        ..idle
      },
      pending_signal: true,
      publication: CanonicalOccurrencePublication::PublishPending { eligible_from: 21 },
    }))
  );
  let disabled = ActorProcess {
    status: ProcessStatus::Disabled(ProcessDisablement {
      cause: ProcessDisableCause::Protocol,
      revival_authority: ProcessRevivalAuthority::Protocol,
      basis: SuspendedProcessBasis::Idle,
    }),
    residence: None,
    ..idle
  };
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, disabled, 20),
    Ok(Some(CanonicalOccurrencePlan {
      process: ActorProcess {
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
        ..disabled
      },
      pending_signal: true,
      publication: CanonicalOccurrencePublication::PublishPending { eligible_from: 21 },
    }))
  );

  for (cycle_state, residence) in [
    (
      CycleState::Running,
      ProcessResidence::Service(ServiceResidenceKind::Live),
    ),
    (
      CycleState::Suspended,
      ProcessResidence::Deadline {
        key: WakeupKey::Block(31),
        page: 2,
        slot: 3,
      },
    ),
  ] {
    let busy = ActorProcess {
      residence: Some(residence),
      ..idle
    };
    assert_eq!(
      plan_canonical_occurrence(cycle_state, false, busy, 20),
      Ok(Some(CanonicalOccurrencePlan {
        process: busy,
        pending_signal: true,
        publication: CanonicalOccurrencePublication::PreserveResidence,
      }))
    );
  }

  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, true, idle, 20),
    Ok(None)
  );
  assert_eq!(
    plan_canonical_occurrence(CycleState::Running, false, idle, 20),
    Err(CanonicalOccurrenceError::InvalidResidence)
  );
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, idle, u32::MAX),
    Err(CanonicalOccurrenceError::BlockNumberOverflow)
  );
  assert_eq!(
    plan_canonical_occurrence(
      CycleState::Idle,
      false,
      ActorProcess {
        status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
        residence: None,
        ..idle
      },
      20,
    ),
    Err(CanonicalOccurrenceError::InvalidProcess)
  );
}

#[test]
fn legacy_process_transition_planner_enforces_owner_obligations_and_typed_successors() {
  let current = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
  };
  let waiting = LegacyProcessPlacement::Waiting {
    key: WakeupKey::Block(9u32),
    page: 2,
    slot: 3,
  };
  let disabled = ProcessDisablement {
    cause: ProcessDisableCause::OwnerPaused,
    revival_authority: ProcessRevivalAuthority::Owner,
    basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
  };

  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PublishTypedResidence,
      LegacyProcessTransition::Publish(waiting),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Deadline {
        key: WakeupKey::Block(9),
        page: 2,
        slot: 3,
      }),
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(Some(waiting)),
    ),
    compile_legacy_process(11, Some(7), waiting).map_err(ProcessTransitionError::Compile)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::Preserve,
    ),
    Ok(current)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::CarrierOnly,
      LegacyProcessTransition::CarrierOnly,
    ),
    Ok(current)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(None),
    ),
    Err(ProcessTransitionError::DetachWithoutSuccessor)
  );
  for obligation in [
    ProcessTransitionObligation::PublishTypedResidence,
    ProcessTransitionObligation::AtomicSuccessorOrRemoval,
  ] {
    let transition = match obligation {
      ProcessTransitionObligation::PublishTypedResidence => {
        LegacyProcessTransition::Publish(LegacyProcessPlacement::Unsignaled(None))
      }
      ProcessTransitionObligation::AtomicSuccessorOrRemoval => {
        LegacyProcessTransition::Replace(Some(LegacyProcessPlacement::Unsignaled(None)))
      }
      _ => unreachable!(),
    };
    assert_eq!(
      plan_legacy_process_transition(current, obligation, transition),
      Err(ProcessTransitionError::Compile(
        ProcessCompileError::AmbiguousUnsignaled
      ))
    );
  }
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::RetireOrDisable,
      LegacyProcessTransition::Disable(disabled),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Disabled(disabled),
      residence: None,
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Retire(CloseReason::OwnerInitiated),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
      residence: None,
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::CarrierOnly,
    ),
    Err(ProcessTransitionError::ObligationMismatch)
  );
  assert_eq!(
    plan_legacy_process_transition(
      ActorProcess::<u32> {
        generation: 11,
        last_attempted: None,
        status: ProcessStatus::Serving,
        residence: None,
      },
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::Preserve,
    ),
    Err(ProcessTransitionError::InvalidCurrentProcess)
  );
}

#[test]
fn process_publication_is_transaction_local_single_authority_and_rollback_safe() {
  new_test_ext().execute_with(|| {
    let current = ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
    };
    let waiting = LegacyProcessPlacement::Waiting {
      key: WakeupKey::Block(9),
      page: 2,
      slot: 3,
    };

    assert_eq!(
      Actors::publish_legacy_process_transition(
        900,
        current,
        ProcessTransitionObligation::PublishTypedResidence,
        LegacyProcessTransition::Publish(waiting),
      ),
      Err(ProcessPublicationError::TransactionRequired)
    );

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let dual_authority = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          actor_id,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(waiting),
        ),
      )
    });
    // A canonically created Actor already owns one process carrier. The legacy transition writer
    // must refuse to overwrite that single authority instead of publishing a second carrier.
    assert_eq!(
      dual_authority,
      Err(ProcessPublicationError::ProcessAlreadyExists)
    );
    assert!(ActorProcesses::<Test>::contains_key(actor_id));

    let rejected = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          902,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(LegacyProcessPlacement::Unsignaled(None)),
        ),
      )
    });
    assert_eq!(
      rejected,
      Err(ProcessPublicationError::Transition(
        ProcessTransitionError::Compile(ProcessCompileError::AmbiguousUnsignaled)
      ))
    );
    assert!(!ActorProcesses::<Test>::contains_key(902));

    let published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          900,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(waiting),
        ),
      )
    })
    .expect("typed process publication succeeds");
    assert_eq!(ActorProcesses::<Test>::get(900), Some(published));

    let mismatched = ActorProcess {
      generation: 12,
      ..published
    };
    let mismatch = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          900,
          mismatched,
          ProcessTransitionObligation::PreserveProcess,
          LegacyProcessTransition::Preserve,
        ),
      )
    });
    assert_eq!(
      mismatch,
      Err(ProcessPublicationError::CurrentProcessMismatch)
    );
    assert_eq!(ActorProcesses::<Test>::get(900), Some(published));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::publish_legacy_process_transition(
        901,
        current,
        ProcessTransitionObligation::PublishTypedResidence,
        LegacyProcessTransition::Publish(waiting),
      )
      .expect("transaction-local publication is initially visible");
      assert!(ActorProcesses::<Test>::contains_key(901));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(!ActorProcesses::<Test>::contains_key(901));
  });
}

#[test]
fn service_publication_atomically_owns_process_and_ring_insertion() {
  new_test_ext().execute_with(|| {
    let first = actor_ref(910, 4);
    publish_test_service_member(first, ServiceResidenceKind::Live, 1)
      .expect("empty publication succeeds");
    assert_eq!(ServiceHeader::<Test>::get().count, 1);
    assert_eq!(
      ActorProcesses::<Test>::get(first.actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Service(ServiceResidenceKind::Live)))
    );
    assert!(ServiceNodes::<Test>::contains_key(first.actor_id));

    let second = actor_ref(911, 2);
    publish_test_service_member(second, ServiceResidenceKind::Pending, 1)
      .expect("populated publication succeeds");
    assert_eq!(ServiceHeader::<Test>::get().count, 2);
    assert!(ActorProcesses::<Test>::contains_key(second.actor_id));
    assert!(ServiceNodes::<Test>::contains_key(second.actor_id));

    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    let rejected = actor_ref(912, 1);
    assert_eq!(
      publish_test_service_member(rejected, ServiceResidenceKind::Live, 1),
      Err(ServicePublicationError::Ring(
        ServiceRingMutationError::CorruptRing
      ))
    );
    assert!(!ActorProcesses::<Test>::contains_key(rejected.actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(rejected.actor_id));
  });
}

#[test]
fn canonical_service_semantics_load_and_mutate_without_legacy_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);

    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    assert_eq!(
      Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Live),
      Ok(record.clone())
    );

    let mut replacement = record.hot.clone();
    replacement.unsuccessful_attempt_streak = 7;
    assert_eq!(
      Actors::try_store_service_control_hot(
        actor,
        ServiceResidenceKind::Live,
        replacement.clone(),
      ),
      Ok(())
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(replacement));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorUnsignaledControlCells::<Test>::contains_key(actor_id));
    DeadlineHandles::<Test>::insert(
      actor_id,
      DeadlineHandle {
        actor,
        key: WakeupKey::Block(2),
        page: 0,
        slot: 0,
      },
    );
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Err(ActorSemanticLoadError::Corrupt),
      "a Service process cannot retain a second process carrier"
    );
  });
}

fn create_parked_balance_service_actor() -> (ActorId, ParkedBalanceActivationOf<Test>) {
  let contract = public_parked_balance_contract(crate::Trigger::Manual);
  let activation = contract.parked_balance_activation.clone().unwrap();
  assert_ok!(Actors::create_system_actor(
    RuntimeOrigin::root(),
    ALICE,
    Mutability::Mutable,
    Some(contract)
  ));
  let actor_id = crate::NextActorId::<Test>::get() - 1;
  let actor = Actors::load_actor_ref(actor_id).unwrap();
  let Some(ProcessResidence::Parked(evidence)) =
    ActorProcesses::<Test>::get(actor_id).unwrap().residence
  else {
    panic!("public creation arms parked balance");
  };
  polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    Actors::release_parked_dependency_authority(actor, evidence).unwrap();
    polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
  });
  (actor_id, activation)
}

#[test]
fn idle_placement_park_preparation_and_finish_preserve_successor() {
  use crate::scheduler::{AttemptTransactionError, PreparedIdleServicePlacement};
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let (actor_id, _) = create_parked_balance_service_actor();
    let actor = Actors::load_actor_ref(actor_id).unwrap();
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let state = Actors::active_actor_state(actor_id).unwrap();
    let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let source_root = root();
    let PreparedIdleServicePlacement::Parked {
      kind,
      activation,
      review_at,
    } = Actors::prepare_idle_service_placement(actor, &state, 1).unwrap()
    else {
      panic!("authored Park destination must be retained")
    };
    assert_eq!(kind, ServiceResidenceKind::Live);
    assert_eq!(review_at, 2);
    assert!(::core::ptr::eq(
      activation,
      state.contract.parked_balance_activation.as_ref().unwrap()
    ));
    assert_eq!(root(), source_root);
    assert!(matches!(
      Actors::prepare_idle_service_placement(actor, &state, u64::MAX),
      Err(AttemptTransactionError::Invariant)
    ));
    assert_eq!(root(), source_root);
    with_transaction_unchecked(|| {
      Actors::transfer_service_member_to_parked_balance(
        actor,
        kind,
        state.identity.cycle_nonce,
        activation,
        WakeupKey::Block(review_at),
      )
      .unwrap();
      let placed = ActorProcesses::<Test>::get(actor_id).unwrap();
      assert!(matches!(
        placed.residence,
        Some(ProcessResidence::Parked(_))
      ));
      let placed_root = root();
      with_transaction_unchecked(|| {
        ActorProcesses::<Test>::mutate(actor_id, |process| {
          process.as_mut().unwrap().generation += 1
        });
        let refused_root = root();
        assert_eq!(
          Actors::finish_idle_service_placement(actor, 1, None),
          Err(AttemptTransactionError::Invariant)
        );
        assert_eq!(root(), refused_root);
        TransactionOutcome::Rollback(())
      });
      assert_eq!(root(), placed_root);
      let mut expected = placed;
      expected.last_attempted = Some(1);
      let expected_root = with_transaction_unchecked(|| {
        ActorProcesses::<Test>::insert(actor_id, expected);
        TransactionOutcome::Rollback(root())
      });
      with_transaction_unchecked(|| {
        Actors::finish_idle_service_placement(actor, 1, None).unwrap();
        assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(expected));
        assert_eq!(
          root(),
          expected_root,
          "finishing changes only the current process stamp"
        );
        TransactionOutcome::Commit(())
      });
      TransactionOutcome::Rollback(())
    });
    assert_eq!(root(), source_root);
  });
}

#[test]
fn canonical_service_parking_installs_destination_before_releasing_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, activation) = create_parked_balance_service_actor();
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    let source = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let source = match Actors::resolve_balance_dependency_source(TestAsset::Local(9)).unwrap() {
        crate::DependencySourceMutation::Allocated(source)
        | crate::DependencySourceMutation::Existing(source) => source,
      };
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(source)
    });
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 7,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
      review_at: Some(2),
    };
    let park = |target| {
      Actors::transfer_service_member_to_parked_balance(
        target,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        &activation,
        WakeupKey::Block(2),
      )
      .map_err(|error| match error {
        ParkedBalanceTransitionError::Dependency(error) => error,
        error => panic!("unexpected balance certification error: {error:?}"),
      })
    };
    let node = ServiceNodes::<Test>::get(actor_id).expect("member exists");

    assert_eq!(
      park(actor_ref(actor_id, actor.generation + 1)),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));

    let mut stale_run_record = record.clone();
    stale_run_record.hot.cycle_state = CycleState::Running;
    ActorSemanticStates::<Test>::insert(actor_id, ActorSemanticState::Active(stale_run_record));
    assert_eq!(
      park(actor),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorSemanticStates::<Test>::insert(actor_id, ActorSemanticState::Active(record.clone()));

    let head = ActorContractHeads::<Test>::get(actor_id).expect("Contract head exists");
    let mut stale_step_head = head.clone();
    stale_step_head.first_step = None;
    ActorContractHeads::<Test>::insert(actor_id, stale_step_head);
    assert_eq!(
      park(actor),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorContractHeads::<Test>::insert(actor_id, head.clone());

    let mut stale_resources_head = head.clone();
    stale_resources_head.first_step_resources = None;
    ActorContractHeads::<Test>::insert(actor_id, stale_resources_head);
    assert_eq!(
      park(actor),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorContractHeads::<Test>::insert(actor_id, head);
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        exhausted: true,
        ..Default::default()
      },
    );
    assert_eq!(
      park(actor),
      Err(DependencyRegistrationError::SourceExhausted)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    DependencyRevisions::<Test>::remove(source);

    let occupied_deadline = Actors::plan_deadline_destination(actor, WakeupKey::Block(2)).unwrap();
    DeadlineHandles::<Test>::insert(actor_id, occupied_deadline);
    assert_eq!(
      park(actor),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyTimedReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id),
      Some(occupied_deadline)
    );
    DeadlineHandles::<Test>::remove(actor_id);

    assert_eq!(
      park(actor),
      Ok(DependencyPlanMutation {
        installed: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(PendingCheckOwners::<Test>::get(actor_id), Some(owner));
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Ok((
        record,
        ActorProcess {
          generation: actor.generation,
          last_attempted: None,
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Parked(evidence)),
        },
      ))
    );

    let stale_evidence = ParkEvidence {
      plan_identity: [8; 32],
      ..evidence
    };
    assert_eq!(
      Actors::wake_parked_member_to_service(
        actor,
        ServiceResidenceKind::Live,
        owner,
        stale_evidence,
        2,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(PendingCheckOwners::<Test>::get(actor_id), Some(owner));
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);

    set_asset_balance(&sovereign_account(actor_id), TestAsset::Local(9), 100);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending =
      PendingDependencyEvents::<Test>::get(actor_id).expect("real balance event is pending");
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::process_pending_parked_balance_event(
        &mut meter,
        pending,
        ServiceResidenceKind::Live,
        evidence,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Ok(crate::DependencyReviewMutation::Woke)
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(Actors::actor_hot(actor_id).unwrap().pending_signal);
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
  });
}

#[test]
fn positive_due_review_wakes_only_the_exact_current_park_episode() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, activation) = create_parked_balance_service_actor();
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let source = match Actors::resolve_balance_dependency_source(TestAsset::Local(9)).unwrap() {
        crate::DependencySourceMutation::Allocated(source)
        | crate::DependencySourceMutation::Existing(source) => source,
      };
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(source)
    });
    let observed = [DependencyPlanSource {
      source,
      observed_revision: 0,
    }];
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 9,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_parked_balance(
      actor,
      ServiceResidenceKind::Live,
      owner.plan_revision,
      &activation,
      WakeupKey::Block(2),
    )
    .unwrap();
    frame_system::Pallet::<Test>::set_block_number(2);
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(2),
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(review),
        Ok(DependencyDueReviewMutation::Published)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let stale_review = DependencyTimedReview {
      deadline: WakeupKey::Block(1),
      ..review
    };
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        stale_review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::PendingReviewMismatch)
    );
    let stale_evidence = ParkEvidence {
      plan_identity: [7; 32],
      ..evidence
    };
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        stale_evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.revision += 1);
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(actor_id),
      Some(review)
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    DependencyRevisions::<Test>::mutate(source, |state| {
      state.revision -= 1;
      state.exhausted = true;
    });
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.exhausted = false);

    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Ok(())
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyRegistrations::<Test>::contains_key(
      source, actor_id
    ));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Service(
        ServiceResidenceKind::Pending
      )))
    );
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::PendingReviewMissing)
    );
  });
}

#[test]
fn canonical_service_semantics_reject_stale_generation_and_residence_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");

    let mut rejected = record.hot.clone();
    rejected.unsuccessful_attempt_streak = 9;
    let stale = actor_ref(actor_id, actor.generation + 1);
    for (reference, kind) in [
      (stale, ServiceResidenceKind::Live),
      (actor, ServiceResidenceKind::Pending),
    ] {
      assert_eq!(
        Actors::load_service_actor_semantic_state(reference, kind),
        Err(ActorSemanticLoadError::Corrupt)
      );
      assert_eq!(
        Actors::try_store_service_control_hot(reference, kind, rejected.clone()),
        Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
      );
      assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    }
  });
}

#[test]
fn retained_service_attempt_commits_semantics_before_advance_and_preserves_refusals() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).expect("round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let header_before_refusal = ServiceHeader::<Test>::get();
    let node_before_refusal = ServiceNodes::<Test>::get(actor_id).unwrap();
    let mut replacement = record.hot.clone();
    replacement.unsuccessful_attempt_streak = 11;
    assert_eq!(
      Actors::commit_retained_service_attempt(
        actor,
        ServiceResidenceKind::Pending,
        record.identity.clone(),
        replacement.clone(),
        2,
      ),
      Err(ServiceRoundError::ProcessResidenceMismatch)
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot));
    assert_eq!(ServiceHeader::<Test>::get(), header_before_refusal);
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id),
      Some(node_before_refusal)
    );

    assert_eq!(
      Actors::commit_retained_service_attempt(
        actor,
        ServiceResidenceKind::Live,
        record.identity.clone(),
        replacement.clone(),
        2,
      ),
      Ok(())
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(replacement));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().last_considered,
      2
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(2)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
  });
}

fn assert_passive_manual_semantics(
  actor: ActorRef,
  cycle_nonce: u64,
  block: u64,
) -> crate::ActorSemanticRecordOf<Test> {
  let (stored, process) = Actors::load_canonical_actor_semantic_state(actor)
    .expect("completed Manual Actor retains canonical passive authority");
  assert_eq!(stored.generation, actor.generation);
  assert_eq!(stored.identity.cycle_nonce, cycle_nonce);
  assert_eq!(stored.hot.cycle_state, CycleState::Idle);
  assert_eq!(stored.hot.last_cycle_block, Some(block));
  assert!(!stored.hot.pending_signal);
  assert!(stored.hot.queue_ticket.is_none());
  assert!(stored.hot.wakeup_pointer.is_none());
  assert!(stored.hot.trigger_wakeup_pointer.is_none());
  assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
  assert!(!ServiceNodes::<Test>::contains_key(actor.actor_id));
  assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
  assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
    actor.actor_id
  ));
  assert_eq!(process.generation, actor.generation);
  assert_eq!(process.residence, None);
  assert_eq!(process.last_attempted, Some(block));
  assert!(
    matches!(process.status, ProcessStatus::Disabled(disablement)
    if disablement.cause == ProcessDisableCause::Protocol
      && disablement.revival_authority == ProcessRevivalAuthority::Protocol
      && disablement.basis == SuspendedProcessBasis::Idle)
  );
  let head = ActorContractHeads::<Test>::get(actor.actor_id).expect("Contract head survives");
  assert_eq!(head.header.trigger, Trigger::Manual);
  assert!(head.header.window.is_none());
  assert!(!head.header.has_close_when);
  let contract = Actors::load_actor_contract(actor.actor_id).expect("complete Contract survives");
  assert_eq!(
    contract.semantic_contract_id(),
    stored.admission.semantic_contract_id
  );
  assert_eq!(
    contract.body_commitment().unwrap(),
    stored.admission.body_commitment
  );
  assert_eq!(
    head.header.semantic_contract_id,
    stored.admission.semantic_contract_id
  );
  assert_eq!(
    head.header.admission_identity,
    stored.admission.admission_identity
  );
  stored
}

#[test]
fn canonical_zero_step_service_attempt_commits_before_passive_placement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, Default::default());
    let state = Actors::active_actor_state(actor_id).expect("active zero-Step state");
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).expect("round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    Actors::execute_zero_step_on_service(
      actor,
      ServiceResidenceKind::Live,
      state,
      &record.admission,
      2,
    )
    .expect("canonical zero-Step attempt commits");
    let stored = assert_passive_manual_semantics(actor, 1, 2);
    let mut expected_identity = record.identity;
    expected_identity.cycle_nonce = 1;
    assert_eq!(stored.identity, expected_identity);
    assert_eq!(stored.admission, record.admission);
  });
}

#[test]
fn canonical_effectful_completion_commits_before_passive_placement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).expect("one-Step Contract"),
    );
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    let stored = Actors::active_actor_state(actor_id)
      .expect("retained canonical process authority remains loadable");
    assert_eq!(stored.identity.cycle_nonce, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(now));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    let process = ActorProcesses::<Test>::get(actor_id).unwrap();
    assert_eq!(process.last_attempted, Some(now));
    assert_eq!(process.residence, None);
    assert!(
      matches!(process.status, ProcessStatus::Disabled(disablement)
      if disablement.cause == crate::ProcessDisableCause::Protocol
        && disablement.revival_authority == crate::ProcessRevivalAuthority::Protocol
        && disablement.basis == crate::SuspendedProcessBasis::Idle)
    );
  });
}

#[test]
fn canonical_effectful_later_retry_moves_through_deadline_and_reenters_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).expect("one-Step retry Contract"),
    );
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    Actors::execute_cycle(Weight::MAX);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    let retry_at = now + 3;
    let retry_run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run remains");
    assert_eq!(retry_run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(retry_run.eligible_at, retry_at);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id),
      Some(DeadlineHandle {
        actor,
        key: WakeupKey::Block(retry_at),
        page: 0,
        slot: 0,
      })
    );
    assert!(matches!(
      Actors::return_next_due_block_deadline_to_service(ServiceResidenceKind::Live, now),
      Err(DeadlineMutationError::InvalidDestination)
    ));

    frame_system::Pallet::<Test>::set_block_number(retry_at);
    let selector_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_block_deadline();
    let retry_weight = Actors::deadline_return_weight_upper();
    let deadline_weight = selector_weight.saturating_add(retry_weight);
    let mut no_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut no_weight,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    let mut branch_refused = WeightMeter::with_limit(selector_weight);
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut branch_refused,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    let source_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    for shortfall in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let mut refused = WeightMeter::with_limit(
        deadline_weight
          .checked_sub(&shortfall)
          .expect("each complete Deadline dimension is positive"),
      );
      assert_eq!(
        Actors::process_next_due_block_deadline(
          &mut refused,
          ServiceResidenceKind::Live,
          retry_at,
          None,
        ),
        Err(DependencyReviewWorkerError::InsufficientWeight)
      );
      assert_eq!(refused.consumed(), selector_weight);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        source_root,
        "branch refusal must retain the exact due source and canonical Service state"
      );
    }
    let mut admitted = WeightMeter::with_limit(deadline_weight);
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut admitted,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Ok(DueBlockDeadlineMutation::RetryReturned(actor))
    );
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(matches!(
      Actors::return_next_due_block_deadline_to_service(ServiceResidenceKind::Live, retry_at),
      Err(DeadlineMutationError::MemberMissing)
    ));
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    let stored_run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run stays stored");
    assert_eq!(stored_run.cursor, retry_run.cursor);
    assert_eq!(stored_run.eligible_at, retry_run.eligible_at);
    assert_eq!(
      stored_run.unsuccessful_attempts_at_cursor,
      retry_run.unsuccessful_attempts_at_cursor
    );
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    frame_system::Pallet::<Test>::set_block_number(retry_at + 1);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      1,
      "the consumed due retry cannot execute twice"
    );
  });
}

#[test]
fn canonical_effectful_terminal_attempt_rolls_back_then_cleans_before_service_unlink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_close_when(
        manual_schedule(),
        None,
        contract_steps_with_step(step),
        Some(crate::tests::close_when_action_and_normal_cycle()),
      ),
    ));
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    let service_header = ServiceHeader::<Test>::get();
    assert_eq!(service_header.cursor, Some(actor));

    let recipient_before = asset_balance(&BOB, TestAsset::Local(1));
    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    let refused = Actors::execute_cycle(Weight::MAX);
    assert!(refused.starved);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);
    assert!(ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    ServiceHeader::<Test>::put(service_header);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      recipient_before + 1
    );
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
    assert_eq!(
      ServiceHeader::<Test>::get(),
      ServiceHeaderRecord {
        round_block: Some(3),
        ..ServiceHeaderRecord::default()
      },
      "the paid final discovery opens an empty round, not an Actor residence"
    );
  });
}

#[test]
fn canonical_zero_step_terminal_attempt_cleans_before_service_unlink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut contract = system_active_contract(manual_schedule(), None, Default::default())
      .expect("zero-Step Contract");
    contract.auto_close_at_cycle_nonce = Some(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(contract),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(actor));

    Actors::execute_cycle(Weight::MAX);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
    assert_eq!(
      ServiceHeader::<Test>::get(),
      ServiceHeaderRecord {
        round_block: Some(now),
        ..ServiceHeaderRecord::default()
      },
      "the paid final discovery opens an empty round, not an Actor residence"
    );
  });
}

#[cfg(feature = "try-runtime")]
fn assert_carrier_corruption_rejected(
  label: &str,
  corrupt: impl FnOnce(),
) -> polkadot_sdk::sp_runtime::TryRuntimeError {
  let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
  let error = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    corrupt();
    let corrupted = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let error =
      Actors::do_try_state().expect_err(&format!("{label}: corrupt carrier must be rejected"));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupted,
      "{label}: validation must not repair or mutate state"
    );
    polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(error)
  });
  assert_eq!(
    polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
    before
  );
  assert_ok!(Actors::do_try_state());
  error
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_carrier_service_ring_rejects_orphans_and_broken_topology() {
  new_test_ext().execute_with(|| {
    assert_ok!(
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::begin_service_round(1),
        )
      })
    );
    assert_ok!(Actors::do_try_state()); // A paid empty round may retain its round marker.
    let actors: Vec<_> = (0..4)
      .map(|_| {
        let id = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), id));
        Actors::load_actor_ref(id).unwrap()
      })
      .collect();
    let dormant = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), dormant));
    assert_eq!(ServiceHeader::<Test>::get().count, 4);
    assert_ok!(Actors::do_try_state());
    for owner in [dormant, 99_999] {
      assert_carrier_corruption_rejected("ownerless Service node", || {
        ServiceNodes::<Test>::insert(
          owner,
          ServiceNodes::<Test>::get(actors[0].actor_id).unwrap(),
        );
        ServiceHeader::<Test>::mutate(|header| header.count += 1);
      });
    }
    assert_carrier_corruption_rejected("Service cardinality", || {
      ServiceHeader::<Test>::mutate(|header| header.count += 1);
    });
    assert_carrier_corruption_rejected("missing Service cursor", || {
      ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    });
    assert_carrier_corruption_rejected("stale Service cursor", || {
      ServiceHeader::<Test>::mutate(|header| header.cursor.as_mut().unwrap().generation += 1);
    });
    assert_carrier_corruption_rejected("Service backlink", || {
      ServiceNodes::<Test>::mutate(actors[1].actor_id, |node| {
        node.as_mut().unwrap().previous = actors[1]
      });
    });
    assert_carrier_corruption_rejected("stale Service next generation", || {
      ServiceNodes::<Test>::mutate(actors[0].actor_id, |node| {
        node.as_mut().unwrap().next.generation += 1
      });
    });
    assert_carrier_corruption_rejected("two disconnected Service rings", || {
      for pair in actors.chunks(2) {
        for (actor, peer) in [(pair[0], pair[1]), (pair[1], pair[0])] {
          ServiceNodes::<Test>::mutate(actor.actor_id, |node| {
            let node = node.as_mut().unwrap();
            node.next = peer;
            node.previous = peer;
          });
        }
      }
    });
    // Removing real peers covers pair/singleton/empty rings without fabricating residence.
    for actor in actors {
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor.actor_id));
      assert_ok!(Actors::do_try_state());
    }
    assert_carrier_corruption_rejected("cursor without any Service member", || {
      ServiceHeader::<Test>::mutate(|header| header.cursor = Some(actor_ref(99_999, 1)));
    });
  });
}

#[cfg(feature = "try-runtime")]
fn prepare_carrier_deadlines() -> (u64, u64) {
  System::set_block_number(1);
  setup_temporary_retry_pool();
  let mut schedule = manual_schedule();
  schedule.cooldown_blocks = 4;
  let sleeper = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
  fund_native(sleeper, 100);
  set_temporary_dex_failure(true);
  assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), sleeper));
  run_next_idle(Weight::MAX);
  assert!(DeadlineHandles::<Test>::contains_key(sleeper));
  assert!(Actors::actor_hot(sleeper).unwrap().wakeup_pointer.is_none());
  let temporal = create_system_with(ALICE, at_time_schedule(1_000), None, BoundedVec::default());
  assert!(TriggerDeadlineHandles::<Test>::contains_key(temporal));
  assert!(matches!(
    ActorProcesses::<Test>::get(temporal).unwrap().status,
    ProcessStatus::Disabled(_)
  ));
  assert_ok!(Actors::do_try_state());
  (sleeper, temporal)
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_carrier_deadline_handles_require_exact_live_owners() {
  new_test_ext().execute_with(|| {
    let (sleeper, temporal) = prepare_carrier_deadlines();
    let dormant = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), dormant));
    for trigger in [false, true] {
      let handle = if trigger {
        TriggerDeadlineHandles::<Test>::get(temporal)
      } else {
        DeadlineHandles::<Test>::get(sleeper)
      }
      .unwrap();
      for owner in [dormant, 99_999] {
        for mismatched_key in [false, true] {
          assert_carrier_corruption_rejected("Deadline handle inverse owner", || {
            let mut orphan = handle;
            if !mismatched_key {
              orphan.actor.actor_id = owner;
            }
            if trigger {
              TriggerDeadlineHandles::<Test>::insert(owner, orphan);
            } else {
              DeadlineHandles::<Test>::insert(owner, orphan);
            }
          });
        }
      }
    }
    assert_carrier_corruption_rejected("extra Trigger on non-temporal owner", || {
      let mut handle = TriggerDeadlineHandles::<Test>::get(temporal).unwrap();
      handle.actor = Actors::load_actor_ref(sleeper).unwrap();
      TriggerDeadlineHandles::<Test>::insert(sleeper, handle);
    });
    assert_carrier_corruption_rejected("stale Trigger generation", || {
      TriggerDeadlineHandles::<Test>::mutate(temporal, |handle| {
        handle.as_mut().unwrap().actor.generation += 1
      });
    });
    assert_carrier_corruption_rejected("stale process Deadline generation", || {
      DeadlineHandles::<Test>::mutate(sleeper, |handle| {
        handle.as_mut().unwrap().actor.generation += 1
      });
    });
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_carrier_deadline_pages_and_index_are_bijective() {
  new_test_ext().execute_with(|| {
    let (_, temporal) = prepare_carrier_deadlines();
    // One full and one sparse C32 page; distinct keys also cross an index-page boundary.
    for _ in 0..32 {
      create_system_with(ALICE, at_time_schedule(1_000), None, BoundedVec::default());
    }
    for ticks in 2_000..2_033 {
      create_system_with(ALICE, at_time_schedule(ticks), None, BoundedVec::default());
    }
    let handle = TriggerDeadlineHandles::<Test>::get(temporal).unwrap();
    let header = DeadlineHeaders::<Test>::get(handle.key).unwrap();
    assert_eq!(header.page_count, 2);
    assert_eq!(header.count, 33);
    assert_eq!(
      DeadlineIndexPages::<Test>::iter_prefix(WakeupClock::Tick).count(),
      2
    );
    assert_ok!(Actors::do_try_state());
    assert_carrier_corruption_rejected("orphan occupied Deadline slot", || {
      DeadlinePages::<Test>::mutate(handle.key, header.last_page, |page| {
        let page = page.as_mut().unwrap();
        page.entries[31] = Some(actor_ref(99_999, 1));
        page.live_entries += 1;
      });
      DeadlineHeaders::<Test>::mutate(handle.key, |header| header.as_mut().unwrap().count += 1);
    });
    assert_carrier_corruption_rejected("copied Deadline page without reverse handles", || {
      DeadlinePages::<Test>::insert(
        handle.key,
        header.next_page,
        DeadlinePages::<Test>::get(handle.key, header.last_page).unwrap(),
      );
    });
    assert_eq!(
      assert_carrier_corruption_rejected("disconnected but fully owned Deadline page", || {
        DeadlinePages::<Test>::mutate(handle.key, header.first_page, |page| {
          page.as_mut().unwrap().next_page = None
        });
        DeadlineHeaders::<Test>::mutate(handle.key, |stored| {
          let stored = stored.as_mut().unwrap();
          stored.last_page = stored.first_page;
          stored.page_count = 1;
          stored.count = 32;
          stored.first_vacant_page = None;
        });
      }),
      polkadot_sdk::sp_runtime::TryRuntimeError::Other("Deadline page is outside its header chain")
    );
    assert_carrier_corruption_rejected("orphan Deadline header", || {
      DeadlineHeaders::<Test>::insert(WakeupKey::Tick(99_999), header);
    });
    assert_carrier_corruption_rejected("Deadline live count", || {
      DeadlinePages::<Test>::mutate(handle.key, header.last_page, |page| {
        page.as_mut().unwrap().live_entries += 1
      });
    });
    assert_carrier_corruption_rejected("Deadline page backlink", || {
      DeadlinePages::<Test>::mutate(handle.key, header.last_page, |page| {
        page.as_mut().unwrap().previous_page = None
      });
    });
    assert_carrier_corruption_rejected("Deadline page chain cycle", || {
      DeadlinePages::<Test>::mutate(handle.key, header.last_page, |page| {
        page.as_mut().unwrap().next_page = Some(header.first_page)
      });
    });
    assert_carrier_corruption_rejected("missing Deadline vacancy authority", || {
      DeadlineHeaders::<Test>::mutate(handle.key, |header| {
        header.as_mut().unwrap().first_vacant_page = None
      });
    });
    assert_carrier_corruption_rejected("full page in vacancy list", || {
      DeadlineHeaders::<Test>::mutate(handle.key, |stored| {
        stored.as_mut().unwrap().first_vacant_page = Some(header.first_page)
      });
    });
    assert_carrier_corruption_rejected("Deadline vacancy cycle", || {
      DeadlinePages::<Test>::mutate(handle.key, header.last_page, |page| {
        page.as_mut().unwrap().next_vacant_page = Some(header.last_page)
      });
    });
    assert_carrier_corruption_rejected("orphan Deadline index position", || {
      DeadlineIndexPositions::<Test>::insert(WakeupKey::Tick(99_999), 0);
    });
    assert_carrier_corruption_rejected("orphan Deadline index page", || {
      DeadlineIndexPages::<Test>::insert(
        WakeupClock::Tick,
        99_999,
        DeadlineIndexPages::<Test>::get(WakeupClock::Tick, 0).unwrap(),
      );
    });
    assert_eq!(
      assert_carrier_corruption_rejected("Deadline index page packing", || {
        DeadlineIndexPages::<Test>::mutate(WakeupClock::Tick, 0, |page| {
          assert!(page.as_mut().unwrap().pop().is_some());
        });
      }),
      polkadot_sdk::sp_runtime::TryRuntimeError::Other("Deadline index pages are not packed C32")
    );
    assert_carrier_corruption_rejected("Deadline index length", || {
      DeadlineIndexLen::<Test>::mutate(WakeupClock::Tick, |len| *len += 1);
    });
    assert_carrier_corruption_rejected("Deadline index reverse position", || {
      DeadlineIndexPositions::<Test>::mutate(handle.key, |index| *index = Some(99_999));
    });
    assert_carrier_corruption_rejected("Deadline heap order with correct reverse indices", || {
      DeadlineIndexPages::<Test>::mutate(WakeupClock::Tick, 0, |page| {
        let page = page.as_mut().unwrap();
        page.as_mut().swap(0, 1);
        DeadlineIndexPositions::<Test>::insert(page[0], 0);
        DeadlineIndexPositions::<Test>::insert(page[1], 1);
      });
    });
    let sparse_member = DeadlinePages::<Test>::get(handle.key, header.last_page)
      .unwrap()
      .entries
      .iter()
      .flatten()
      .next()
      .unwrap()
      .actor_id;
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), temporal));
    assert_eq!(
      DeadlineHeaders::<Test>::get(handle.key)
        .unwrap()
        .first_vacant_page,
      Some(header.first_page)
    );
    assert_ok!(Actors::do_try_state());
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), sparse_member));
    assert!(!DeadlinePages::<Test>::contains_key(
      handle.key,
      header.last_page
    ));
    assert_ok!(Actors::do_try_state());
    for _ in 0..2 {
      create_system_with(ALICE, at_time_schedule(1_000), None, BoundedVec::default());
      assert_ok!(Actors::do_try_state());
    }
    assert_eq!(
      DeadlineHeaders::<Test>::get(handle.key).unwrap().last_page,
      header.next_page,
      "reclaimed page ids need not be contiguous"
    );
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_orphan_canonical_processes() {
  new_test_ext().execute_with(|| {
    assert_ok!(Actors::do_try_state());
    ActorProcesses::<Test>::insert(
      999,
      ActorProcess {
        generation: 1,
        last_attempted: Some(1),
        status: ProcessStatus::Retired(CloseReason::AutoCloseNonceReached),
        residence: None,
      },
    );
    assert_eq!(
      Actors::do_try_state(),
      Err(polkadot_sdk::sp_runtime::TryRuntimeError::Other(
        "canonical process has no semantic Actor owner"
      ))
    );
    ActorProcesses::<Test>::remove(999);
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn service_retirement_atomically_unlinks_each_topology_and_reclaims_the_process() {
  new_test_ext().execute_with(|| {
    let members = [
      actor_ref(920, 1),
      actor_ref(921, 1),
      actor_ref(922, 1),
      actor_ref(923, 1),
    ];
    for actor in members {
      publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
        .expect("service publication succeeds");
    }

    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::retire_service_member(actor_ref(members[0].actor_id, 2)),
      Err(ServiceRetirementError::Ring(
        ServiceRingMutationError::StaleGeneration
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    ActorProcesses::<Test>::mutate(members[0].actor_id, |process| {
      process.as_mut().unwrap().residence = None;
    });
    let corrupt_root =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::retire_service_member(members[0]),
      Err(ServiceRetirementError::Ring(
        ServiceRingMutationError::ProcessResidenceMismatch
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupt_root
    );
    ActorProcesses::<Test>::insert(
      members[0].actor_id,
      serving_process(members[0], ServiceResidenceKind::Live),
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );

    for actor in [members[2], members[0], members[3], members[1]] {
      Actors::retire_service_member(actor)
        .expect("interior, cursor, pair, and singleton retirement succeeds");
      assert!(!ServiceNodes::<Test>::contains_key(actor.actor_id));
      assert!(!ActorProcesses::<Test>::contains_key(actor.actor_id));
    }
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());

    let corrupt = actor_ref(924, 1);
    publish_test_service_member(corrupt, ServiceResidenceKind::Live, 2)
      .expect("service publication succeeds");
    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    let corrupt_root =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::retire_service_member(corrupt),
      Err(ServiceRetirementError::Ring(
        ServiceRingMutationError::CorruptRing
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupt_root
    );
    assert!(ServiceNodes::<Test>::contains_key(corrupt.actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(corrupt.actor_id),
      Some(serving_process(corrupt, ServiceResidenceKind::Live))
    );
  });
}

#[test]
fn legacy_control_mutation_inventory_covers_every_raw_storage_owner() {
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum TransactionBoundary {
    CallerTransactional,
    FunctionTransactional,
    InPlaceAtomicWrite,
  }
  type RequiredProcessTransition = ProcessTransitionObligation;
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum PlannerIntent {
    Publish,
    Preserve,
    Replace,
    RetireOrDisable,
    CarrierOnly,
  }
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum AtomicPublicationSite {
    MutationOwner,
    EveryDirectCaller,
    CarrierNoProcessPublication,
  }
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum AtomicOutcome {
    SuccessorOrRollback,
    PreservedOrRollback,
    TerminalOrRollback,
    CarrierOnlyOrRollback,
  }
  type InventoryRow = (
    &'static str,
    &'static str,
    TransactionBoundary,
    RequiredProcessTransition,
    PlannerIntent,
    AtomicPublicationSite,
    AtomicOutcome,
  );

  const INVENTORY: &[InventoryRow] = &[
    (
      "execution.rs",
      "write_run_state",
      TransactionBoundary::FunctionTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::MutationOwner,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "lib.rs",
      "replace_control_admission_for_transition",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PreserveProcess,
      PlannerIntent::Preserve,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::PreservedOrRollback,
    ),
    (
      "scheduler.rs",
      "append_waiting_entry",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::CarrierOnly,
      PlannerIntent::CarrierOnly,
      AtomicPublicationSite::CarrierNoProcessPublication,
      AtomicOutcome::CarrierOnlyOrRollback,
    ),
    (
      "scheduler.rs",
      "consume_waiting_from_supplied_authority",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "control_append_ready",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "control_append_waiting",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "detach_primary_for_successor",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "paged_consume_head_at_inner",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "remove_primary_control_cell_inner",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "remove_waiting_entry",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::CarrierOnly,
      PlannerIntent::CarrierOnly,
      AtomicPublicationSite::CarrierNoProcessPublication,
      AtomicOutcome::CarrierOnlyOrRollback,
    ),
    (
      "scheduler.rs",
      "restore_unsignaled_from_authority",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "store_primary_control_cell",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::PreserveProcess,
      PlannerIntent::Preserve,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::PreservedOrRollback,
    ),
    (
      "scheduler.rs",
      "wakeup_substrate_drain_block_inner",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
  ];

  fn raw_mutation_owners(source: &str) -> BTreeSet<String> {
    let mut owner = None;
    let mut owners = BTreeSet::new();
    for line in source.lines() {
      let trimmed = line.trim_start();
      if let Some(rest) = trimmed
        .strip_prefix("fn ")
        .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
      {
        owner = rest.split('(').next();
      }
      let mutates_control_storage = [
        "ActorUnsignaledControlCells::<T>::insert",
        "ActorUnsignaledControlCells::<T>::remove",
        "ActorReadyFrameChunks::<T>::insert",
        "ActorReadyFrameChunks::<T>::remove",
        "ActorWaitingFrameChunks::<T>::insert",
        "ActorWaitingFrameChunks::<T>::remove",
        "ActorControlLocators::<T>::insert",
        "ActorControlLocators::<T>::remove",
      ]
      .iter()
      .any(|needle| line.contains(needle));
      if mutates_control_storage {
        owners.insert(
          owner
            .expect("raw control mutation must be inside a named function")
            .into(),
        );
      }
    }
    owners
  }

  let sources = [
    ("lib.rs", include_str!("../lib.rs")),
    ("scheduler.rs", include_str!("../scheduler.rs")),
    ("execution.rs", include_str!("../execution.rs")),
  ];
  let actual = sources
    .into_iter()
    .flat_map(|(file, source)| {
      raw_mutation_owners(source)
        .into_iter()
        .map(move |owner| (file, owner))
    })
    .collect::<BTreeSet<_>>();
  let expected = INVENTORY
    .iter()
    .map(|(file, owner, ..)| (*file, (*owner).to_owned()))
    .collect::<BTreeSet<_>>();
  assert_eq!(
    actual, expected,
    "classify every new raw control mutation owner before process cutover"
  );
  assert_eq!(
    INVENTORY
      .iter()
      .map(|(_, _, boundary, ..)| *boundary)
      .collect::<BTreeSet<_>>(),
    BTreeSet::from([
      TransactionBoundary::CallerTransactional,
      TransactionBoundary::FunctionTransactional,
      TransactionBoundary::InPlaceAtomicWrite,
    ])
  );
  assert_eq!(
    INVENTORY
      .iter()
      .map(|(_, _, _, transition, ..)| *transition)
      .collect::<BTreeSet<_>>(),
    BTreeSet::from([
      RequiredProcessTransition::PublishTypedResidence,
      RequiredProcessTransition::PreserveProcess,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      RequiredProcessTransition::CarrierOnly,
    ])
  );

  fn direct_callers(source: &str, callee: &str) -> BTreeSet<String> {
    let mut owner = None;
    let mut callers = BTreeSet::new();
    for line in source.lines() {
      let trimmed = line.trim_start();
      if let Some(rest) = trimmed
        .strip_prefix("fn ")
        .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
      {
        owner = rest.split('(').next();
      }
      if line.contains(&format!("Self::{callee}("))
        && let Some(caller) = owner
        && caller != callee
      {
        callers.insert(caller.to_owned());
      }
    }
    callers
  }

  for (file, owner, boundary, obligation, intent, site, outcome) in INVENTORY {
    let expected_intent = match obligation {
      RequiredProcessTransition::PublishTypedResidence => PlannerIntent::Publish,
      RequiredProcessTransition::PreserveProcess => PlannerIntent::Preserve,
      RequiredProcessTransition::AtomicSuccessorOrRemoval => PlannerIntent::Replace,
      RequiredProcessTransition::RetireOrDisable => PlannerIntent::RetireOrDisable,
      RequiredProcessTransition::CarrierOnly => PlannerIntent::CarrierOnly,
    };
    assert_eq!(
      *intent, expected_intent,
      "planner intent drift for {file}::{owner}"
    );
    let expected_outcome = match obligation {
      RequiredProcessTransition::PublishTypedResidence
      | RequiredProcessTransition::AtomicSuccessorOrRemoval => AtomicOutcome::SuccessorOrRollback,
      RequiredProcessTransition::PreserveProcess => AtomicOutcome::PreservedOrRollback,
      RequiredProcessTransition::RetireOrDisable => AtomicOutcome::TerminalOrRollback,
      RequiredProcessTransition::CarrierOnly => AtomicOutcome::CarrierOnlyOrRollback,
    };
    assert_eq!(
      *outcome, expected_outcome,
      "atomic outcome drift for {file}::{owner}"
    );

    match site {
      AtomicPublicationSite::MutationOwner => {
        assert_eq!(*boundary, TransactionBoundary::FunctionTransactional);
        let source = sources
          .iter()
          .find_map(|(candidate, source)| (*candidate == *file).then_some(*source))
          .expect("inventory source exists");
        assert!(
          source.contains(&format!("fn {owner}(")) && source.contains("storage::with_transaction"),
          "mutation owner must retain its explicit transaction: {file}::{owner}"
        );
      }
      AtomicPublicationSite::EveryDirectCaller => {
        let callers = sources
          .iter()
          .flat_map(|(_, source)| direct_callers(source, owner))
          .collect::<BTreeSet<_>>();
        assert!(
          !callers.is_empty(),
          "caller-transactional owner must have a source-backed publication cohort: {file}::{owner}"
        );
      }
      AtomicPublicationSite::CarrierNoProcessPublication => {
        assert_eq!(*intent, PlannerIntent::CarrierOnly);
        assert_eq!(*outcome, AtomicOutcome::CarrierOnlyOrRollback);
      }
    }
  }
}

#[test]
fn canonical_semantic_publication_is_the_sole_actor_authority() {
  let lib = include_str!("../lib.rs");
  let scheduler = include_str!("../scheduler.rs");
  let pallet_weights = include_str!("../weights.rs");
  let runtime_weights = include_str!("../../../../runtime/src/weights/pallet_deos_actors.rs");

  // The post-cutover creation/activation owner publishes exactly one semantic record and one
  // process/residence carrier instead of mirroring per-field state into a legacy primary cell.
  assert!(lib.contains("fn insert_active_actor("));
  assert!(lib.contains("Self::mutate_actor_semantic_state("));
  assert!(lib.contains("Self::publish_actor_publication("));
  assert!(lib.contains("Self::store_actor_contract(actor_id, contract.clone())?"));
  assert!(lib.contains("fn load_actor_semantic_state("));
  assert!(lib.contains("ActorSemanticStates::<T>::get(actor_id)"));

  // Every dispatch that may terminally finalize prices the shared close upper bound so a canonical
  // close can never underprice its bounded cleanup.
  for terminal_dispatch_owner in [
    "pause_actor",
    "resume_actor",
    "manual_trigger",
    "close_actor",
    "update_contract",
    "permissionless_sweep",
    "permissionless_sweep_many",
    "cancel_run",
  ] {
    let marker = format!("    pub fn {terminal_dispatch_owner}(");
    let start = lib
      .find(&marker)
      .unwrap_or_else(|| panic!("terminal dispatch owner disappeared: {terminal_dispatch_owner}"));
    let prefix = &lib[start.saturating_sub(320)..start];
    assert!(
      prefix.contains("close_dispatch_weight_upper()"),
      "terminal dispatch owner must price shared finalization: {terminal_dispatch_owner}"
    );
  }
  assert!(scheduler.contains("pub fn close_cleanup_weight_upper() -> Weight"));
  assert!(scheduler.contains("T::WeightInfo::close_actor()"));

  for generated_owner in [
    "create_user_actor",
    "create_user_actor_at_slot",
    "create_system_actor",
    "create_system_actor_at_sovereign_id",
    "create_dormant_system_actor",
    "activate_actor",
    "deactivate_actor",
    "close_actor",
  ] {
    assert!(
      pallet_weights.contains(&format!("fn {generated_owner}() -> Weight")),
      "pallet Weight binding disappeared: {generated_owner}"
    );
    assert!(
      runtime_weights.contains(&format!("fn {generated_owner}() -> Weight")),
      "runtime Weight binding disappeared: {generated_owner}"
    );
  }
}

#[test]
fn process_status_separates_park_from_revocation_and_retirement() {
  let parked: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Parked(ParkEvidence {
      plan_identity: [3; 32],
      reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
      review_at: Some(9u32),
    })),
  };
  let disabled: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Disabled(ProcessDisablement {
      cause: ProcessDisableCause::OwnerPaused,
      revival_authority: ProcessRevivalAuthority::Owner,
      basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
    }),
    residence: None,
  };
  let retired: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
    residence: None,
  };

  assert!(matches!(
    parked,
    ActorProcess {
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Parked(_)),
      ..
    }
  ));
  assert!(matches!(
    disabled,
    ActorProcess {
      status: ProcessStatus::Disabled(_),
      residence: None,
      ..
    }
  ));
  assert!(matches!(
    retired,
    ActorProcess {
      status: ProcessStatus::Retired(_),
      residence: None,
      ..
    }
  ));
}

#[derive(Default)]
struct ServiceRingOracle {
  header: ServiceHeaderRecord<u32>,
  nodes: BTreeMap<u64, ServiceNode<u32>>,
}

impl ServiceRingOracle {
  fn insert_tail(&mut self, actor: ActorRef) {
    assert!(!self.nodes.contains_key(&actor.actor_id));
    let cursor = self.header.cursor.unwrap_or(actor);
    let tail = self
      .nodes
      .get(&cursor.actor_id)
      .map_or(actor, |node| node.previous);
    let node = ServiceNode {
      generation: actor.generation,
      previous: tail,
      next: cursor,
      kind: ServiceResidenceKind::Live,
      eligible_from: 1,
      last_considered: 0,
    };
    if let Some(head) = self.nodes.get_mut(&cursor.actor_id) {
      head.previous = actor;
      self
        .nodes
        .get_mut(&tail.actor_id)
        .expect("tail exists")
        .next = actor;
    } else {
      self.header.cursor = Some(actor);
    }
    self.nodes.insert(actor.actor_id, node);
    self.header.count += 1;
    self.assert_valid();
  }

  fn remove(&mut self, actor: ActorRef) -> bool {
    let Some(node) = self.nodes.get(&actor.actor_id).copied() else {
      return false;
    };
    if node.generation != actor.generation {
      return false;
    }
    if self.header.count == 1 {
      self.header.cursor = None;
    } else {
      self
        .nodes
        .get_mut(&node.previous.actor_id)
        .expect("previous exists")
        .next = node.next;
      self
        .nodes
        .get_mut(&node.next.actor_id)
        .expect("next exists")
        .previous = node.previous;
      if self.header.cursor == Some(actor) {
        self.header.cursor = Some(node.next);
      }
    }
    self.nodes.remove(&actor.actor_id);
    self.header.count -= 1;
    self.assert_valid();
    true
  }

  fn assert_valid(&self) {
    assert_eq!(self.header.count as usize, self.nodes.len());
    let Some(start) = self.header.cursor else {
      assert!(self.nodes.is_empty());
      return;
    };
    let mut current = start;
    for _ in 0..self.header.count {
      let node = self.nodes.get(&current.actor_id).expect("member exists");
      assert_eq!(node.generation, current.generation);
      let next = self.nodes.get(&node.next.actor_id).expect("next exists");
      let previous = self
        .nodes
        .get(&node.previous.actor_id)
        .expect("previous exists");
      assert_eq!(next.previous, current);
      assert_eq!(previous.next, current);
      current = node.next;
    }
    assert_eq!(current, start);
  }
}

fn actor_ref(actor_id: u64, generation: u64) -> ActorRef {
  ActorRef {
    actor_id,
    generation,
  }
}

#[test]
fn service_ring_oracle_covers_empty_singleton_interior_cursor_and_wrap() {
  let mut ring = ServiceRingOracle::default();
  ring.assert_valid();
  for id in 1..=4 {
    ring.insert_tail(actor_ref(id, 1));
  }
  assert_eq!(ring.nodes[&4].next, actor_ref(1, 1));
  assert!(ring.remove(actor_ref(3, 1)));
  assert!(ring.remove(actor_ref(1, 1)));
  assert_eq!(ring.header.cursor, Some(actor_ref(2, 1)));
  assert!(ring.remove(actor_ref(4, 1)));
  assert!(ring.remove(actor_ref(2, 1)));
  assert_eq!(ring.header, ServiceHeaderRecord::default());
}

#[test]
fn service_ring_oracle_rejects_stale_generation_without_mutation() {
  let mut ring = ServiceRingOracle::default();
  ring.insert_tail(actor_ref(7, 3));
  assert!(!ring.remove(actor_ref(7, 2)));
  assert_eq!(ring.header.count, 1);
  assert_eq!(ring.header.cursor, Some(actor_ref(7, 3)));
}

fn serving_process(actor: ActorRef, kind: ServiceResidenceKind) -> ActorProcess<u64> {
  ActorProcess {
    generation: actor.generation,
    last_attempted: None,
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Service(kind)),
  }
}

#[test]
fn canonical_service_ring_is_transactional_generation_bound_and_structurally_complete() {
  new_test_ext().execute_with(|| {
    let first = actor_ref(100, 4);
    ActorProcesses::<Test>::insert(
      first.actor_id,
      serving_process(first, ServiceResidenceKind::Live),
    );
    assert_eq!(
      Actors::insert_service_member(first, ServiceResidenceKind::Live, 1),
      Err(ServiceRingMutationError::TransactionRequired)
    );

    // A canonically published Actor carries its own Service residence. Inserting a different
    // residence kind for the same generation must fail closed before the ring is touched.
    let mismatched = actor_ref(104, 9);
    ActorProcesses::<Test>::insert(
      mismatched.actor_id,
      serving_process(mismatched, ServiceResidenceKind::Pending),
    );
    let residence_rejected =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::insert_service_member(mismatched, ServiceResidenceKind::Live, 1),
        )
      });
    assert_eq!(
      residence_rejected,
      Err(ServiceRingMutationError::ProcessResidenceMismatch)
    );

    let members = [
      first,
      actor_ref(101, 5),
      actor_ref(102, 6),
      actor_ref(103, 7),
    ];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::insert_service_member(actor, ServiceResidenceKind::Live, 1),
        )
      })
      .expect("canonical insertion succeeds");
    }
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
    assert_eq!(ServiceHeader::<Test>::get().count, 4);
    assert_eq!(ServiceNodes::<Test>::get(103).expect("tail").next, first);

    let stale = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_service_member(actor_ref(102, 5)),
      )
    });
    assert_eq!(stale, Err(ServiceRingMutationError::StaleGeneration));
    assert_eq!(ServiceHeader::<Test>::get().count, 4);

    for actor in [members[2], members[0], members[3], members[1]] {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::remove_service_member(actor),
        )
      })
      .expect("interior, cursor, pair, and singleton removal succeed");
    }
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(
      members
        .iter()
        .all(|actor| !ServiceNodes::<Test>::contains_key(actor.actor_id))
    );

    let rolled_back = actor_ref(110, 8);
    ActorProcesses::<Test>::insert(
      rolled_back.actor_id,
      serving_process(rolled_back, ServiceResidenceKind::Pending),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(rolled_back, ServiceResidenceKind::Pending, 2)
        .expect("staged insertion is visible");
      assert!(ServiceNodes::<Test>::contains_key(rolled_back.actor_id));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(!ServiceNodes::<Test>::contains_key(rolled_back.actor_id));
  });
}

#[test]
fn dependency_revision_is_nonwrapping_sticky_and_transactional() {
  new_test_ext().execute_with(|| {
    let source = 17;
    assert_eq!(
      Actors::revise_dependency_source(source),
      Err(DependencyRevisionError::TransactionRequired)
    );
    let first = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::revise_dependency_source(source),
      )
    });
    assert_eq!(first, Ok(DependencyRevisionMutation::Advanced(1)));

    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::revise_dependency_source(source);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(rolled_back, Ok(DependencyRevisionMutation::Advanced(2)));
    assert_eq!(DependencyRevisions::<Test>::get(source).revision, 1);

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: u64::MAX,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    for _ in 0..2 {
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::revise_dependency_source(source),
        )
      });
      assert_eq!(outcome, Ok(DependencyRevisionMutation::Exhausted));
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: u64::MAX,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: true,
        }
      );
    }
  });
}

#[test]
fn dependency_event_publication_coalesces_behind_one_fixed_scan() {
  new_test_ext().execute_with(|| {
    let source = 18;
    assert_eq!(
      Actors::publish_dependency_event(source),
      Err(DependencyRevisionError::TransactionRequired)
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: 1,
          scan_target: Some(1),
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        }
      );
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 2,
          active_target: 1,
        })
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 0),
        Ok(DependencyScanMutation::HandedOff(2))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let owner = PendingCheckOwner {
      actor: actor_ref(120, 4),
      plan_revision: 6,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, owner, 2),
        Ok(DependencyRegistrationMutation::Installed)
      );
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 3,
          active_target: 2,
        })
      );
      let fixed = DependencyRevisions::<Test>::get(source);
      assert_eq!(fixed.scan_end, 0);
      assert_eq!(
        Actors::complete_dependency_scan(source, 2, 0),
        Ok(DependencyScanMutation::HandedOff(3))
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 1);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let retained = DependencyRevisions::<Test>::get(source);
    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::publish_dependency_event(source);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(
      rolled_back,
      Ok(DependencyPublicationMutation::Coalesced {
        revision: 4,
        active_target: 3,
      })
    );
    assert_eq!(DependencyRevisions::<Test>::get(source), retained);

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: u64::MAX,
        scan_target: Some(9),
        scan_cursor: 4,
        scan_end: 7,
        exhausted: false,
      },
    );
    for _ in 0..2 {
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::publish_dependency_event(source),
        )
      });
      assert_eq!(outcome, Ok(DependencyPublicationMutation::Exhausted));
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: u64::MAX,
          scan_target: Some(9),
          scan_cursor: 4,
          scan_end: 7,
          exhausted: true,
        }
      );
    }
  });
}

#[test]
fn dependency_publication_retains_empty_scan_source_across_coalescing_and_rollback() {
  new_test_ext().execute_with(|| {
    let source = 18;
    assert_eq!(
      Actors::publish_dependency_event_with_source_retention(source),
      Err(DependencyPublicationError::Revision(
        DependencyRevisionError::TransactionRequired
      ))
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event_with_source_retention(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        Actors::publish_dependency_event_with_source_retention(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 2,
          active_target: 1,
        })
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 0);
    assert_eq!(DependencyScanSourceListState::<Test>::get().count, 1);

    let before_revision = DependencyRevisions::<Test>::get(19);
    let before_list = DependencyScanSourceListState::<Test>::get();
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::publish_dependency_event_with_source_retention(19);
      assert_eq!(outcome, Ok(DependencyPublicationMutation::Begun(1)));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(19), before_revision);
    assert_eq!(DependencyScanSourceListState::<Test>::get(), before_list);
    assert!(!DependencyScanSourceNodes::<Test>::contains_key(19));
  });
}

#[test]
fn dependency_scan_source_carrier_preserves_exact_fair_membership_and_rollback() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::insert_dependency_scan_source(18),
      Err(DependencyScanSourceError::TransactionRequired)
    );

    for source in [18, 19, 20] {
      DependencyRevisions::<Test>::mutate(source, |state| state.scan_target = Some(1));
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      for source in [18, 19, 20] {
        assert_eq!(
          Actors::insert_dependency_scan_source(source),
          Ok(DependencyScanSourceMutation::Inserted)
        );
      }
      assert_eq!(
        Actors::insert_dependency_scan_source(19),
        Ok(DependencyScanSourceMutation::AlreadyActive)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(
      DependencyScanSourceListState::<Test>::get(),
      DependencyScanSourceList {
        cursor: Some(18),
        count: 3,
      }
    );
    assert_eq!(
      DependencyScanSourceNodes::<Test>::get(18),
      Some(DependencyScanSourceNode {
        previous: 20,
        next: 19,
      })
    );

    let before = DependencyScanSourceListState::<Test>::get();
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      DependencyRevisions::<Test>::mutate(18, |state| state.scan_target = None);
      assert_eq!(
        Actors::remove_dependency_scan_source(18),
        Ok(DependencyScanSourceMutation::Removed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyScanSourceListState::<Test>::get(), before);
    assert!(DependencyScanSourceNodes::<Test>::contains_key(18));

    for source in [18, 20, 19] {
      DependencyRevisions::<Test>::mutate(source, |state| state.scan_target = None);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::remove_dependency_scan_source(source),
          Ok(DependencyScanSourceMutation::Removed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    assert_eq!(
      DependencyScanSourceListState::<Test>::get(),
      DependencyScanSourceList::default()
    );
    assert_eq!(DependencyScanSourceNodes::<Test>::iter().count(), 0);

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::insert_dependency_scan_source(21),
        Err(DependencyScanSourceError::ScanInactive)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn dependency_scan_keeps_fixed_target_and_requires_exact_advance_authority() {
  new_test_ext().execute_with(|| {
    let source = 19;
    let owner = PendingCheckOwner {
      actor: actor_ref(121, 3),
      plan_revision: 5,
    };
    let handle = DependencyRegistrationHandle {
      actor: owner.actor,
      plan_revision: owner.plan_revision,
      acknowledged_revision: 1,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(
      Actors::begin_dependency_scan(source),
      Err(DependencyScanError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(1))
      );
      assert_eq!(
        Actors::revise_dependency_source(source),
        Ok(DependencyRevisionMutation::Advanced(2))
      );
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Err(DependencyScanError::ScanAlreadyActive)
      );
      assert_eq!(
        DependencyRevisions::<Test>::get(source).scan_target,
        Some(1)
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(handle)
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 1),
        Ok(DependencyScanMutation::HandedOff(2))
      );
      let state = DependencyRevisions::<Test>::get(source);
      assert_eq!(state.scan_target, Some(2));
      assert_eq!(state.scan_cursor, 0);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let retained = DependencyRevisions::<Test>::get(source);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      DependencyRegistrations::<Test>::remove(source, owner.actor.actor_id);
      assert_eq!(
        Actors::process_dependency_scan_member(source, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source), retained);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(handle)
    );

    let mut exhausted = retained;
    exhausted.exhausted = true;
    DependencyRevisions::<Test>::insert(source, exhausted);
    let before = DependencyRevisions::<Test>::get(source);
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::complete_dependency_scan(source, 2, 0),
      )
    });
    assert_eq!(refused, Err(DependencyScanError::SourceExhausted));
    assert_eq!(DependencyRevisions::<Test>::get(source), before);
  });
}

#[test]
fn dependency_scan_processes_one_destination_before_advancing() {
  new_test_ext().execute_with(|| {
    let source = 29;
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 3,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    let owners: Vec<_> = (0..5)
      .map(|offset| PendingCheckOwner {
        actor: actor_ref(130 + offset, 2),
        plan_revision: 7,
      })
      .collect();
    for (index, owner) in owners.iter().copied().enumerate() {
      PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::install_dependency_registration(
            source,
            owner,
            if index == 0 || index == 3 { 1 } else { 3 },
          ),
          Ok(DependencyRegistrationMutation::Installed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(3))
      );
      assert_eq!(
        Actors::revise_dependency_source(source),
        Ok(DependencyRevisionMutation::Advanced(4))
      );
      let third = DependencyRegistrations::<Test>::get(source, owners[2].actor.actor_id).unwrap();
      assert_eq!(
        Actors::replace_dependency_registration(source, third, owners[2], 4),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let before = DependencyRevisions::<Test>::get(source);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owners[0].actor.actor_id)
          .unwrap()
          .acknowledged_revision,
        3
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source), before);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[0].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owners[0].actor.actor_id
    ));

    let coalesced = PendingDependencyEvent {
      owner: owners[0],
      source: 77,
      revision: 9,
    };
    PendingDependencyEvents::<Test>::insert(owners[0].actor.actor_id, coalesced);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        PendingDependencyEvents::<Test>::get(owners[0].actor.actor_id),
        Some(coalesced)
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 1),
        Ok(DependencyScanMutation::Advanced(2))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 2),
        Ok(DependencyScanMutation::Advanced(3))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[2].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      4
    );

    let wrong_generation = PendingCheckOwner {
      actor: actor_ref(owners[3].actor.actor_id, 3),
      ..owners[3]
    };
    let wrong_plan = PendingCheckOwner {
      plan_revision: 8,
      ..owners[3]
    };
    for wrong_owner in [wrong_generation, wrong_plan] {
      PendingCheckOwners::<Test>::insert(wrong_owner.actor.actor_id, wrong_owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::process_dependency_scan_member(source, 3, 3),
          Err(DependencyScanError::PendingAuthorityMismatch)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 3);
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owners[3].actor.actor_id)
          .unwrap()
          .acknowledged_revision,
        1
      );
    }
    PendingCheckOwners::<Test>::insert(owners[3].actor.actor_id, owners[3]);
    PendingDependencyEvents::<Test>::insert(
      owners[3].actor.actor_id,
      PendingDependencyEvent {
        owner: wrong_plan,
        source,
        revision: 3,
      },
    );
    let destination_refused =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::process_dependency_scan_member(source, 3, 3),
        )
      });
    assert_eq!(
      destination_refused,
      Err(DependencyScanError::PendingDestinationMismatch)
    );
    assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 3);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[3].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    PendingDependencyEvents::<Test>::remove(owners[3].actor.actor_id);

    let stale_position =
      DependencyRegistrationPositions::<Test>::get(source, owners[4].actor.actor_id).unwrap();
    DependencyRegistrations::<Test>::remove(source, owners[4].actor.actor_id);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 3),
        Ok(DependencyScanMutation::Advanced(4))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 4),
        Ok(DependencyScanMutation::Advanced(5))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 4);
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owners[3].actor.actor_id),
      Some(PendingDependencyEvent {
        owner: owners[3],
        source,
        revision: 3,
      })
    );
    assert_eq!(
      DependencyRegistrationPages::<Test>::get(source, stale_position.page)
        .unwrap()
        .entries[stale_position.slot as usize],
      None
    );
    assert!(!DependencyRegistrationPositions::<Test>::contains_key(
      source,
      owners[4].actor.actor_id
    ));
    assert_eq!(
      Actors::complete_dependency_scan(source, 3, 5),
      Err(DependencyScanError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::complete_dependency_scan(source, 3, 5),
        Ok(DependencyScanMutation::HandedOff(4))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn pending_check_owner_is_exactly_generation_and_plan_bound() {
  new_test_ext().execute_with(|| {
    let old = PendingCheckOwner {
      actor: actor_ref(119, 7),
      plan_revision: 3,
    };
    PendingCheckOwners::<Test>::insert(old.actor.actor_id, old);
    assert_eq!(
      PendingCheckOwners::<Test>::get(old.actor.actor_id),
      Some(old)
    );

    let replacement = PendingCheckOwner {
      actor: actor_ref(old.actor.actor_id, 8),
      plan_revision: old.plan_revision,
    };
    let revised_plan = PendingCheckOwner {
      actor: old.actor,
      plan_revision: 4,
    };
    assert_ne!(old, replacement);
    assert_ne!(old, revised_plan);
    assert_eq!(
      PendingCheckOwners::<Test>::get(old.actor.actor_id),
      Some(old)
    );
  });
}

#[test]
fn dependency_registration_boundaries_are_exact_idempotent_and_transactional() {
  new_test_ext().execute_with(|| {
    let source = 23;
    let old_owner = PendingCheckOwner {
      actor: actor_ref(123, 4),
      plan_revision: 7,
    };
    let old_handle = DependencyRegistrationHandle {
      actor: old_owner.actor,
      plan_revision: old_owner.plan_revision,
      acknowledged_revision: 2,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 3,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(old_owner.actor.actor_id, old_owner);

    assert_eq!(
      Actors::install_dependency_registration(source, old_owner, 2),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::install_dependency_registration(source, old_owner, 2),
      )
    });
    assert_eq!(installed, Ok(DependencyRegistrationMutation::Installed));
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      1
    );
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 1);
    assert_eq!(
      DependencyRegistrationPages::<Test>::get(source, 0)
        .unwrap()
        .entries[0],
      Some(old_handle)
    );
    assert!(DependencyRegistrationPositions::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    let overlap = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::install_dependency_registration(source, old_owner, 2),
      )
    });
    assert_eq!(overlap, Ok(DependencyRegistrationMutation::Unchanged));

    let stale_generation = PendingCheckOwner {
      actor: actor_ref(old_owner.actor.actor_id, 5),
      plan_revision: old_owner.plan_revision,
    };
    let stale_plan = PendingCheckOwner {
      actor: old_owner.actor,
      plan_revision: old_owner.plan_revision + 1,
    };
    for owner in [stale_generation, stale_plan] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::replace_dependency_registration(source, old_handle, owner, 3),
        )
      });
      assert_eq!(
        refused,
        Err(DependencyRegistrationError::PendingOwnerMismatch)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
        Some(old_handle)
      );
    }
    let future = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::replace_dependency_registration(source, old_handle, old_owner, 4),
      )
    });
    assert_eq!(future, Err(DependencyRegistrationError::RevisionFromFuture));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
      Some(old_handle)
    );

    let revised_owner = PendingCheckOwner {
      actor: old_owner.actor,
      plan_revision: 8,
    };
    PendingCheckOwners::<Test>::insert(revised_owner.actor.actor_id, revised_owner);
    let replacement = DependencyRegistrationHandle {
      actor: revised_owner.actor,
      plan_revision: revised_owner.plan_revision,
      acknowledged_revision: 3,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::replace_dependency_registration(source, old_handle, revised_owner, 3),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
        Some(replacement)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
      Some(old_handle)
    );

    let removed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_dependency_registration(source, old_handle),
      )
    });
    assert_eq!(removed, Ok(DependencyRegistrationMutation::Removed));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    assert!(!DependencyRegistrationPositions::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    assert!(!DependencyRegistrationHeaders::<Test>::contains_key(source));
    assert!(!DependencyRegistrationFreePositions::<Test>::contains_key(
      source, 0
    ));
    assert!(!DependencyRegistrationPages::<Test>::contains_key(
      source, 0
    ));
  });
}

#[test]
fn negative_dependency_evaluation_commits_only_exact_current_snapshot() {
  new_test_ext().execute_with(|| {
    let source = 37;
    let owner = PendingCheckOwner {
      actor: actor_ref(237, 4),
      plan_revision: 6,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 2,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    assert_eq!(
      Actors::commit_negative_dependency_evaluation(source, owner, 2),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(installed, Ok(DependencyRegistrationMutation::Installed));
    let original = DependencyRegistrationHandle {
      actor: owner.actor,
      plan_revision: owner.plan_revision,
      acknowledged_revision: 2,
    };
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );
    let unchanged = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(unchanged, Ok(DependencyRegistrationMutation::Unchanged));

    for stale_owner in [
      PendingCheckOwner {
        actor: actor_ref(owner.actor.actor_id, owner.actor.generation + 1),
        plan_revision: owner.plan_revision,
      },
      PendingCheckOwner {
        actor: owner.actor,
        plan_revision: owner.plan_revision + 1,
      },
    ] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::commit_negative_dependency_evaluation(source, stale_owner, 2),
        )
      });
      assert_eq!(
        refused,
        Err(DependencyRegistrationError::PendingOwnerMismatch)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(original)
      );
    }

    DependencyRevisions::<Test>::mutate(source, |state| state.revision = 3);
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(raced, Err(DependencyRegistrationError::RevisionMismatch));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: owner.plan_revision + 1,
    };
    PendingCheckOwners::<Test>::insert(revised_owner.actor.actor_id, revised_owner);
    let replacement = DependencyRegistrationHandle {
      actor: revised_owner.actor,
      plan_revision: revised_owner.plan_revision,
      acknowledged_revision: 3,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_evaluation(source, revised_owner, 3),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(replacement)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );

    DependencyRevisions::<Test>::mutate(source, |state| state.exhausted = true);
    let exhausted = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, revised_owner, 3),
      )
    });
    assert_eq!(exhausted, Err(DependencyRegistrationError::SourceExhausted));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );
  });
}

#[test]
fn complete_negative_dependency_plan_replaces_all_sources_atomically() {
  new_test_ext().execute_with(|| {
    let owner = PendingCheckOwner {
      actor: actor_ref(238, 4),
      plan_revision: 6,
    };
    for (source, revision) in [(41, 2), (42, 3), (43, 4), (44, 5)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let initial = [
      DependencyPlanSource {
        source: 41,
        observed_revision: 2,
      },
      DependencyPlanSource {
        source: 42,
        observed_revision: 3,
      },
    ];
    assert_eq!(
      Actors::commit_negative_dependency_plan(owner, &initial, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &initial, None),
      )
    });
    assert_eq!(
      installed,
      Ok(DependencyPlanMutation {
        installed: 2,
        ..Default::default()
      })
    );
    assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, revised_owner);
    let revised = [
      DependencyPlanSource {
        source: 42,
        observed_revision: 3,
      },
      DependencyPlanSource {
        source: 43,
        observed_revision: 4,
      },
    ];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          replaced: 1,
          removed: 1,
          ..Default::default()
        })
      );
      assert!(!DependencyRegistrations::<Test>::contains_key(
        41,
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        43,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      43,
      owner.actor.actor_id
    ));
    assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);

    let duplicate = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[
            DependencyPlanSource {
              source: 41,
              observed_revision: 2,
            },
            DependencyPlanSource {
              source: 41,
              observed_revision: 2,
            },
          ],
          None,
        ),
      )
    });
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 43,
            observed_revision: 3,
          }],
          None,
        ),
      )
    });
    for refused in [duplicate, raced] {
      assert!(matches!(
        refused,
        Err(DependencyRegistrationError::DuplicateSource)
          | Err(DependencyRegistrationError::RevisionMismatch)
      ));
      assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);
    }

    DependencyRegistrationHeaders::<Test>::mutate(44, |header| {
      let capacity: u32 = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 44,
            observed_revision: 5,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert!(DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      42,
      owner.actor.actor_id
    ));

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        installed: 1,
        replaced: 1,
        removed: 1,
        ..Default::default()
      })
    );
    let unchanged = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
      )
    });
    assert_eq!(
      unchanged,
      Ok(DependencyPlanMutation {
        retained: 2,
        ..Default::default()
      })
    );
    assert!(!DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      42,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      43,
      owner.actor.actor_id
    ));
  });
}

#[test]
fn complete_negative_dependency_plan_replaces_block_and_tick_review_atomically() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let source = 45;
    let owner = PendingCheckOwner {
      actor: actor_ref(239, 4),
      plan_revision: 6,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 2,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let desired = [DependencyPlanSource {
      source,
      observed_revision: 2,
    }];
    let block_review = WakeupKey::Block(20);
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &desired, Some(block_review)),
      )
    });
    assert_eq!(
      installed,
      Ok(DependencyPlanMutation {
        installed: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    let original = DependencyTimedReview {
      owner,
      deadline: block_review,
    };
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(original)
    );

    let retained = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &desired, Some(block_review)),
      )
    });
    assert_eq!(
      retained,
      Ok(DependencyPlanMutation {
        retained: 1,
        timed_review: DependencyTimedReviewMutation::Retained,
        ..Default::default()
      })
    );

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, revised_owner);
    let tick_review = WakeupKey::Tick(30);
    let replaced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &desired, Some(tick_review)),
      )
    });
    assert_eq!(
      replaced,
      Ok(DependencyPlanMutation {
        replaced: 1,
        timed_review: DependencyTimedReviewMutation::Replaced,
        ..Default::default()
      })
    );
    let replacement = DependencyTimedReview {
      owner: revised_owner,
      deadline: tick_review,
    };
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    for invalid in [WakeupKey::Block(10), WakeupKey::Tick(10)] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::commit_negative_dependency_plan(revised_owner, &desired, Some(invalid)),
        )
      });
      assert_eq!(refused, Err(DependencyRegistrationError::DeadlineNotFuture));
      assert_eq!(
        DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
        Some(replacement)
      );
    }

    DependencyRevisions::<Test>::insert(
      46,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    DependencyRegistrationHeaders::<Test>::mutate(46, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 46,
            observed_revision: 1,
          }],
          Some(WakeupKey::Block(40)),
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert!(DependencyRegistrations::<Test>::contains_key(
      source,
      owner.actor.actor_id
    ));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(revised_owner, &desired, None),
        Ok(DependencyPlanMutation {
          retained: 1,
          timed_review: DependencyTimedReviewMutation::Removed,
          ..Default::default()
        })
      );
      assert!(!DependencyTimedReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    let stale = PendingCheckOwner {
      actor: actor_ref(owner.actor.actor_id, owner.actor.generation + 1),
      plan_revision: revised_owner.plan_revision,
    };
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(stale, &desired, None),
      )
    });
    assert_eq!(
      refused,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );
  });
}

#[test]
fn due_dependency_review_publishes_exact_pending_authority_atomically() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(240, 4),
      plan_revision: 6,
    };
    let block_review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    DependencyTimedReviews::<Test>::insert(owner.actor.actor_id, block_review);

    assert_eq!(
      Actors::publish_due_dependency_review(block_review),
      Err(DependencyDueReviewError::TransactionRequired)
    );
    let early = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(early, Err(DependencyDueReviewError::NotDue));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(block_review)
    );

    System::set_block_number(20);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(block_review),
        Ok(DependencyDueReviewMutation::Published)
      );
      assert!(!DependencyTimedReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert_eq!(
        PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
        Some(block_review)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(block_review)
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));

    let published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(published, Ok(DependencyDueReviewMutation::Published));
    let duplicate = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(duplicate, Ok(DependencyDueReviewMutation::AlreadyPending));

    let tick_owner = PendingCheckOwner {
      actor: actor_ref(241, 2),
      plan_revision: 3,
    };
    let tick_review = DependencyTimedReview {
      owner: tick_owner,
      deadline: WakeupKey::Tick(10),
    };
    PendingCheckOwners::<Test>::insert(tick_owner.actor.actor_id, tick_owner);
    DependencyTimedReviews::<Test>::insert(tick_owner.actor.actor_id, tick_review);
    let tick_published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(tick_review),
      )
    });
    assert_eq!(tick_published, Ok(DependencyDueReviewMutation::Published));

    let raced_owner = PendingCheckOwner {
      actor: actor_ref(242, 1),
      plan_revision: 1,
    };
    let stale = DependencyTimedReview {
      owner: raced_owner,
      deadline: WakeupKey::Block(19),
    };
    let replacement = DependencyTimedReview {
      owner: PendingCheckOwner {
        plan_revision: 2,
        ..raced_owner
      },
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(raced_owner.actor.actor_id, replacement.owner);
    DependencyTimedReviews::<Test>::insert(raced_owner.actor.actor_id, replacement);
    let stale_deadline = DependencyTimedReview {
      owner: replacement.owner,
      deadline: stale.deadline,
    };
    let replaced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(stale_deadline),
      )
    });
    assert_eq!(replaced, Err(DependencyDueReviewError::ReviewMismatch));
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(stale),
      )
    });
    assert_eq!(raced, Err(DependencyDueReviewError::PendingOwnerMismatch));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(raced_owner.actor.actor_id),
      Some(replacement)
    );

    let occupied_owner = PendingCheckOwner {
      actor: actor_ref(243, 1),
      plan_revision: 1,
    };
    let occupied = DependencyTimedReview {
      owner: occupied_owner,
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(occupied_owner.actor.actor_id, occupied_owner);
    DependencyTimedReviews::<Test>::insert(occupied_owner.actor.actor_id, occupied);
    PendingDependencyReviews::<Test>::insert(
      occupied_owner.actor.actor_id,
      DependencyTimedReview {
        deadline: WakeupKey::Block(19),
        ..occupied
      },
    );
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(occupied),
      )
    });
    assert_eq!(refused, Err(DependencyDueReviewError::DestinationOccupied));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(occupied_owner.actor.actor_id),
      Some(occupied)
    );
  });
}

#[test]
fn dependency_pending_causes_share_one_actor_destination() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(10),
    };
    DependencyTimedReviews::<Test>::insert(owner.actor.actor_id, review);
    let event = PendingDependencyEvent {
      owner,
      source: 57,
      revision: 2,
    };
    PendingDependencyEvents::<Test>::insert(owner.actor.actor_id, event);

    let blocked_review = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(review),
      )
    });
    assert_eq!(
      blocked_review,
      Err(DependencyDueReviewError::DestinationOccupied)
    );
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(review)
    );
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(event)
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));

    PendingDependencyEvents::<Test>::remove(owner.actor.actor_id);
    let published_review = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(review),
      )
    });
    assert_eq!(published_review, Ok(DependencyDueReviewMutation::Published));

    DependencyRevisions::<Test>::insert(
      57,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(
          owner,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 1,
          }],
          None,
        ),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(2))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let blocked_event = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::process_dependency_scan_member(57, 2, 0),
      )
    });
    assert_eq!(
      blocked_event,
      Err(DependencyScanError::PendingDestinationMismatch)
    );
    assert_eq!(DependencyRevisions::<Test>::get(57).scan_cursor, 0);
    assert_eq!(
      DependencyRegistrations::<Test>::get(57, owner.actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(review)
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));

    let consumed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(
          review,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 2,
          }],
          None,
        ),
      )
    });
    assert_eq!(
      consumed,
      Ok(DependencyPlanMutation {
        replaced: 1,
        ..Default::default()
      })
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(57, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(57, 2, 1),
        Ok(DependencyScanMutation::Completed)
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(3))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert!(PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));
  });
}

#[test]
fn pending_dependency_event_installs_complete_successor_before_consumption() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    for (source, revision) in [(57, 2), (58, 4), (59, 1)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(
          owner,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 2,
          }],
          None,
        ),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(3))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(57, 3, 1),
        Ok(DependencyScanMutation::Completed)
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(4))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 4, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending = PendingDependencyEvent {
      owner,
      source: 57,
      revision: 3,
    };
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert_eq!(
      DependencyPlans::<Test>::get(owner.actor.actor_id)[0]
        .handle
        .acknowledged_revision,
      4
    );
    let successor = [
      DependencyPlanSource {
        source: 57,
        observed_revision: 4,
      },
      DependencyPlanSource {
        source: 58,
        observed_revision: 4,
      },
    ];

    assert_eq!(
      Actors::consume_pending_dependency_event(pending, &successor, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consume_pending_dependency_event(pending, &successor, Some(WakeupKey::Tick(20)),),
        Ok(DependencyPlanMutation {
          retained: 1,
          installed: 1,
          timed_review: DependencyTimedReviewMutation::Installed,
          ..Default::default()
        })
      );
      assert!(!PendingDependencyEvents::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        58,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(!DependencyRegistrations::<Test>::contains_key(
      58,
      owner.actor.actor_id
    ));

    let mismatched = PendingDependencyEvent {
      revision: 4,
      ..pending
    };
    let mismatch = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(mismatched, &successor, None),
      )
    });
    assert_eq!(
      mismatch,
      Err(DependencyRegistrationError::PendingEventMismatch)
    );

    let invalid = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, Some(WakeupKey::Block(10))),
      )
    });
    assert_eq!(invalid, Err(DependencyRegistrationError::DeadlineNotFuture));
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );

    let registration = DependencyRegistrations::<Test>::take(57, owner.actor.actor_id).unwrap();
    let registration_race =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::consume_pending_dependency_event(pending, &successor, None),
        )
      });
    assert_eq!(
      registration_race,
      Err(DependencyRegistrationError::RegistrationMissing)
    );
    DependencyRegistrations::<Test>::insert(57, owner.actor.actor_id, registration);

    DependencyRegistrationHeaders::<Test>::mutate(59, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(
          pending,
          &[DependencyPlanSource {
            source: 59,
            observed_revision: 1,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));

    let raced_owner = PendingCheckOwner {
      plan_revision: owner.plan_revision + 1,
      ..owner
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, raced_owner);
    let owner_race = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      owner_race,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        retained: 1,
        installed: 1,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));
    let replay = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      replay,
      Err(DependencyRegistrationError::PendingEventMissing)
    );
  });
}

#[test]
fn pending_dependency_review_installs_complete_successor_before_consumption() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    for (source, revision) in [(47, 2), (48, 4)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let initial = [DependencyPlanSource {
      source: 47,
      observed_revision: 2,
    }];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(owner, &initial, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(10),
    };
    PendingDependencyReviews::<Test>::insert(owner.actor.actor_id, pending);
    let successor = [DependencyPlanSource {
      source: 48,
      observed_revision: 4,
    }];

    assert_eq!(
      Actors::consume_pending_dependency_review(pending, &successor, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consume_pending_dependency_review(pending, &successor, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          removed: 1,
          ..Default::default()
        })
      );
      assert!(!PendingDependencyReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        48,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      47,
      owner.actor.actor_id
    ));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      48,
      owner.actor.actor_id
    ));

    let invalid = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, Some(WakeupKey::Block(10))),
      )
    });
    assert_eq!(invalid, Err(DependencyRegistrationError::DeadlineNotFuture));
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );

    DependencyRevisions::<Test>::mutate(48, |state| state.revision = 5);
    let revision_race = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      revision_race,
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    DependencyRevisions::<Test>::mutate(48, |state| state.revision = 4);

    DependencyRevisions::<Test>::insert(
      49,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    DependencyRegistrationHeaders::<Test>::mutate(49, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(
          pending,
          &[DependencyPlanSource {
            source: 49,
            observed_revision: 1,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      47,
      owner.actor.actor_id
    ));

    let raced_owner = PendingCheckOwner {
      plan_revision: owner.plan_revision + 1,
      ..owner
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, raced_owner);
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      raced,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    let stale = DependencyTimedReview {
      deadline: WakeupKey::Block(9),
      ..pending
    };
    let mismatched = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(stale, &successor, None),
      )
    });
    assert_eq!(
      mismatched,
      Err(DependencyRegistrationError::PendingReviewMismatch)
    );

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, Some(WakeupKey::Tick(20))),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        installed: 1,
        removed: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(DependencyTimedReview {
        owner,
        deadline: WakeupKey::Tick(20),
      })
    );
    let replay = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      replay,
      Err(DependencyRegistrationError::PendingReviewMissing)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      48,
      owner.actor.actor_id
    ));
  });
}

#[test]
fn dependency_pages_keep_fixed_cursor_authority_across_fragmentation_and_new_members() {
  new_test_ext().execute_with(|| {
    let source = 29;
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    let mut handles = Vec::new();
    for actor_id in 200..234 {
      let owner = PendingCheckOwner {
        actor: actor_ref(actor_id, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(actor_id, owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::install_dependency_registration(source, owner, 1),
          Ok(DependencyRegistrationMutation::Installed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
      handles.push(DependencyRegistrationHandle {
        actor: owner.actor,
        plan_revision: 1,
        acknowledged_revision: 1,
      });
    }
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      34
    );
    assert!(DependencyRegistrationPages::<Test>::contains_key(source, 0));
    assert!(DependencyRegistrationPages::<Test>::contains_key(source, 1));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(1))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::remove_dependency_registration(source, handles[0]),
        Ok(DependencyRegistrationMutation::Removed)
      );
      let ahead_owner = PendingCheckOwner {
        actor: handles[33].actor,
        plan_revision: 2,
      };
      PendingCheckOwners::<Test>::insert(ahead_owner.actor.actor_id, ahead_owner);
      assert_eq!(
        Actors::replace_dependency_registration(source, handles[33], ahead_owner, 1),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      handles[33].plan_revision = 2;

      let new_owner = PendingCheckOwner {
        actor: actor_ref(234, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(new_owner.actor.actor_id, new_owner);
      assert_eq!(
        Actors::install_dependency_registration(source, new_owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 34);

      for cursor in 1..34 {
        assert_eq!(
          Actors::process_dependency_scan_member(source, 1, cursor),
          Ok(DependencyScanMutation::Advanced(cursor + 1))
        );
      }
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 34),
        Err(DependencyScanError::ScanComplete)
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 34),
        Ok(DependencyScanMutation::Completed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      35
    );
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 34);
    assert_eq!(
      DependencyRegistrationPositions::<Test>::get(source, handles[33].actor.actor_id)
        .unwrap()
        .page,
      1
    );

    let reused_owner = PendingCheckOwner {
      actor: actor_ref(235, 1),
      plan_revision: 1,
    };
    PendingCheckOwners::<Test>::insert(reused_owner.actor.actor_id, reused_owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, reused_owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrationPositions::<Test>::get(source, reused_owner.actor.actor_id),
      Some(DependencyRegistrationPosition { page: 0, slot: 0 })
    );
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      35
    );
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).free_count,
      0
    );
  });
}

#[test]
fn canonical_service_round_preserves_markers_cursor_and_blocked_head() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(120, 1), actor_ref(121, 1), actor_ref(122, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 4)
          .expect("same-block admission succeeds");
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    assert_eq!(ServiceHeader::<Test>::get().round_block, Some(4));
    assert!(members.iter().all(|actor| {
      let node = ServiceNodes::<Test>::get(actor.actor_id).expect("member");
      node.eligible_from == 5 && node.last_considered == 4
    }));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(5).expect("next round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::advance_service_head(members[0], 5).expect("staged attempt advances");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[0]));
    assert_eq!(
      ActorProcesses::<Test>::get(members[0].actor_id)
        .unwrap()
        .last_attempted,
      None
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[0]))
      );
      Actors::advance_service_head(members[0], 5).expect("first consideration advances");
      assert_eq!(
        ActorProcesses::<Test>::get(members[0].actor_id)
          .unwrap()
          .last_attempted,
        Some(5)
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[1]));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::advance_service_head(members[1], 5).expect("second consideration advances");
      Actors::advance_service_head(members[2], 5).expect("third consideration wraps");
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn canonical_service_idle_no_work_head_advances_without_an_attempt() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(130, 1), actor_ref(131, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 4)
          .expect("same-block admission succeeds");
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(5).expect("next round begins");
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[0]))
      );
      Actors::advance_idle_service_head(members[0], 5)
        .expect("an Idle retained resident advances without executing a Step");
      let node = ServiceNodes::<Test>::get(members[0].actor_id).expect("retained member");
      assert_eq!(node.last_considered, 5);
      assert_eq!(
        ActorProcesses::<Test>::get(members[0].actor_id)
          .unwrap()
          .last_attempted,
        None,
        "a no-work encounter records no attempt"
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[1]));
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[1]))
      );
      Actors::advance_idle_service_head(members[1], 5).expect("second no-work resident advances");
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[0]));
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Closed),
        "the round closes once every resident has been considered exactly once"
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn mandatory_service_frontier_dispatches_the_selected_zero_step_kind() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, Default::default());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let zero_step = <Test as crate::Config>::WeightInfo::scheduler_inner_zero_step_complete()
      .saturating_add(
        <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
          <Test as crate::Config>::WeightInfo::service_member_retire_interior()
            .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
            .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
        ),
      );
    let complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(zero_step);
    let mut refused = WeightMeter::with_limit(complete.saturating_sub(Weight::from_parts(1, 0)));
    assert_eq!(
      Actors::service_canonical_round_head(&mut refused, 5),
      Err(ServiceRoundError::InsufficientWeight)
    );
    assert_eq!(
      refused.consumed(),
      selector.saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
    );
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));

    let mut admitted = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head(&mut admitted, 5),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(admitted.consumed(), complete);
    let stored = assert_passive_manual_semantics(actor, 1, 5);
    let mut expected_identity = record.identity;
    expected_identity.cycle_nonce = 1;
    assert_eq!(stored.identity, expected_identity);
    assert_eq!(stored.admission, record.admission);
    assert_eq!(stored.identity.cycle_nonce, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_frontier_pre_admits_and_executes_one_effectful_head() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let recipient_before = asset_balance(&BOB, TestAsset::Local(1));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let inspection =
      selector.saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe());
    let complete = inspection
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let mut refused = WeightMeter::with_limit(complete.saturating_sub(deficit));
      assert_eq!(
        Actors::service_canonical_round_head(&mut refused, 5),
        Err(ServiceRoundError::InsufficientWeight)
      );
      assert_eq!(refused.consumed(), inspection);
      assert_eq!(ServiceHeader::<Test>::get(), before_header);
      assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);
    }

    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let branch = resources.control.saturating_add(suffix);
      let remaining = inspection.saturating_add(branch).saturating_sub(deficit);
      let consumed_before = budget.limits().actor_control().saturating_sub(remaining);
      let mut resource_refused = resource_state;
      let mut prior = resource_refused
        .reserve(
          budget.limits(),
          crate::BlockResourceDomain::ActorControl,
          consumed_before,
        )
        .expect("test leaves only a component-wise late-refusal boundary");
      assert_ok!(resource_refused.settle(&mut prior, consumed_before));
      let resource_refused_before = resource_refused;
      let mut admitted = WeightMeter::with_limit(complete);
      assert_eq!(
        Actors::service_canonical_round_head_with_resources(
          &mut admitted,
          5,
          &mut resource_refused,
          budget.limits(),
          crate::BlockResourceDomain::ActorDrainEffect,
        ),
        Err(ServiceRoundError::ResourceUnavailable)
      );
      assert_eq!(admitted.consumed(), inspection);
      assert_eq!(
        resource_refused.usage().actor_control_used(),
        resource_refused_before
          .usage()
          .actor_control_used()
          .saturating_add(inspection)
      );
      assert_eq!(resource_refused.outstanding_reservations(), 0);
      assert_eq!(ServiceHeader::<Test>::get(), before_header);
      assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);
    }

    let mut admitted = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut admitted,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert!(admitted.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      recipient_before + 1
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_commits_abort_cycle_failure_and_passive_placement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    assert_eq!(step.on_error, StepErrorPolicy::AbortCycle);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    let stored = Actors::load_canonical_actor_semantic_state(actor)
      .expect("aborted cycle retains passive process authority")
      .0;
    assert_eq!(stored.hot.cycle_state, CycleState::Idle);
    assert_eq!(stored.hot.unsuccessful_attempt_streak, 0);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_commits_permanent_retry_failure_without_parking() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let asset_out = TestAsset::Local(91);
    setup_pool(TestAsset::Native, asset_out, 1_000_000, 1);
    set_asset_balance(&u64::MAX, asset_out, 1);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out,
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    let stored = assert_passive_manual_semantics(actor, 1, 5);
    let mut expected_identity = record.identity;
    expected_identity.cycle_nonce = 1;
    assert_eq!(stored.identity, expected_identity);
    assert_eq!(stored.admission, record.admission);
    assert_eq!(stored.hot.cycle_state, CycleState::Idle);
    assert_eq!(stored.hot.unsuccessful_attempt_streak, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        retry_class: RetryClass::Permanent,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_closes_locally_exhausted_retry_and_removes_residence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass(budget));
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(first_state.outstanding_reservations(), 0);
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor_id)
        .expect("first temporary failure retains its Run")
        .unsuccessful_attempts_at_cursor,
      1
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    let complete = complete.saturating_add(Actors::close_dispatch_weight_upper());
    let inspection =
      selector.saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe());
    let before_header = ServiceHeader::<Test>::get();
    let before_semantic = ActorSemanticStates::<Test>::get(actor_id).unwrap();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let before_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    let before_run_coordinates = (
      before_run.cycle_nonce,
      before_run.cursor,
      before_run.eligible_at,
      before_run.unsuccessful_attempts_at_cursor,
      before_run.cumulative_outcomes,
    );
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let remaining = complete
        .saturating_sub(resources.effect)
        .saturating_sub(deficit);
      let consumed_before = budget.limits().actor_control().saturating_sub(remaining);
      let mut resource_refused = crate::BlockResourceState::new(6);
      assert_ok!(resource_refused.begin_prepass(budget));
      assert_ok!(resource_refused.open_external_phase());
      assert_ok!(resource_refused.begin_drain());
      let mut prior = resource_refused
        .reserve(
          budget.limits(),
          crate::BlockResourceDomain::ActorControl,
          consumed_before,
        )
        .expect("test leaves a one-unit shortfall in complete terminal Control admission");
      assert_ok!(resource_refused.settle(&mut prior, consumed_before));
      let resource_refused_before = resource_refused;
      let mut refused_meter = WeightMeter::with_limit(complete);
      assert_eq!(
        Actors::service_canonical_round_head_with_resources(
          &mut refused_meter,
          6,
          &mut resource_refused,
          budget.limits(),
          crate::BlockResourceDomain::ActorDrainEffect,
        ),
        Err(ServiceRoundError::ResourceUnavailable)
      );
      assert_eq!(refused_meter.consumed(), inspection);
      assert_eq!(
        resource_refused.usage().actor_control_used(),
        resource_refused_before
          .usage()
          .actor_control_used()
          .saturating_add(inspection)
      );
      assert_eq!(resource_refused.outstanding_reservations(), 0);
      assert_eq!(ServiceHeader::<Test>::get(), before_header);
      assert_eq!(
        ActorSemanticStates::<Test>::get(actor_id),
        Some(before_semantic.clone())
      );
      assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
      let retained_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
      assert_eq!(
        (
          retained_run.cycle_nonce,
          retained_run.cursor,
          retained_run.eligible_at,
          retained_run.unsuccessful_attempts_at_cursor,
          retained_run.cumulative_outcomes,
        ),
        before_run_coordinates
      );
      assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    }

    let mut resource_state = crate::BlockResourceState::new(6);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        6,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(ServiceHeader::<Test>::get().count, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_closes_at_global_failure_limit_and_rolls_back_refusal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    set_max_consecutive_failures(2);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass(budget));
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(first_state.outstanding_reservations(), 0);
    let first_run = ActorRunStateStore::<Test>::get(actor_id)
      .expect("first failure remains below the global limit");
    assert_eq!(first_run.unsuccessful_attempts_at_cursor, 1);
    let ActorSemanticState::Active(first_semantic) =
      ActorSemanticStates::<Test>::get(actor_id).expect("first failure retains semantic state")
    else {
      panic!("first failure keeps active semantics");
    };
    assert_eq!(first_semantic.hot.unsuccessful_attempt_streak, 1);

    frame_system::Pallet::<Test>::set_block_number(6);
    let complete = complete.saturating_add(Actors::close_dispatch_weight_upper());
    let before_header = ServiceHeader::<Test>::get();
    let before_semantic = ActorSemanticStates::<Test>::get(actor_id).unwrap();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let before_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    let before_run_coordinates = (
      before_run.cycle_nonce,
      before_run.cursor,
      before_run.eligible_at,
      before_run.unsuccessful_attempts_at_cursor,
      before_run.cumulative_outcomes,
    );
    let mut resource_refused = crate::BlockResourceState::new(6);
    assert_ok!(resource_refused.begin_prepass(budget));
    assert_ok!(resource_refused.open_external_phase());
    assert_ok!(resource_refused.begin_drain());
    let mut exhausted = resource_refused
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::ActorControl,
        budget.limits().actor_control(),
      )
      .expect("test exhausts ActorControl");
    assert_ok!(resource_refused.settle(&mut exhausted, budget.limits().actor_control()));
    let resource_refused_before = resource_refused;
    let mut refused_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut refused_meter,
        6,
        &mut resource_refused,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::DiscoveryUnavailable)
    );
    assert_eq!(refused_meter.consumed(), Weight::zero());
    assert_eq!(resource_refused, resource_refused_before);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(
      ActorSemanticStates::<Test>::get(actor_id),
      Some(before_semantic)
    );
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    let retained_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    assert_eq!(
      (
        retained_run.cycle_nonce,
        retained_run.cursor,
        retained_run.eligible_at,
        retained_run.unsuccessful_attempts_at_cursor,
        retained_run.cumulative_outcomes,
      ),
      before_run_coordinates
    );

    let mut resource_state = crate::BlockResourceState::new(6);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        6,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(ServiceHeader::<Test>::get().count, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_continues_after_failed_step_without_repeating_the_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let failed = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let completed = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(2),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failed, completed]).unwrap(),
    );
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    fund_native(actor_id, 100);
    set_asset_balance(&sovereign, TestAsset::Local(2), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let first_resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let first_complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(first_resources.control)
      .saturating_add(first_resources.effect)
      .saturating_add(suffix);
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass(budget));
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(first_complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(2)), 0);
    assert_eq!(first_state.outstanding_reservations(), 0);
    let running = ActorRunStateStore::<Test>::get(actor_id).expect("cycle remains Running");
    assert_eq!(running.cursor, 1);
    assert_eq!(running.cumulative_outcomes.failed_steps, 1);
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let parked_balance =
      ParkedBalanceActivationOf::<Test>::try_from_rules(vec![ParkedBalanceRule {
        asset: TestAsset::Native,
        authored_min_delta: 100,
      }])
      .unwrap();
    assert_eq!(
      Actors::transfer_service_member_to_parked_balance(
        actor,
        ServiceResidenceKind::Live,
        17,
        &parked_balance,
        WakeupKey::Block(6),
      ),
      Err(ParkedBalanceTransitionError::Dependency(
        DependencyRegistrationError::StoredPlanMismatch
      )),
      "a coherent Running cycle cannot acquire parked-balance authority"
    );
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor_id)
        .expect("busy refusal retains Run")
        .encode(),
      running.encode()
    );
    assert!(!ParkedBalanceEpisodes::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    let semantic = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Live)
      .expect("continued Actor remains in canonical Service");
    let second_resources = Actors::load_actor_service_state_with_control(
      actor_id,
      semantic.identity,
      semantic.hot,
      semantic.admission,
    )
    .and_then(|(_, _, loaded)| loaded)
    .expect("continued Step loads from canonical authority")
    .resources;
    let second_complete = selector
      .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe())
      .saturating_add(second_resources.control)
      .saturating_add(second_resources.effect)
      .saturating_add(suffix);
    let mut second_state = crate::BlockResourceState::new(6);
    assert_ok!(second_state.begin_prepass(budget));
    assert_ok!(second_state.open_external_phase());
    assert_ok!(second_state.begin_drain());
    let mut second_meter = WeightMeter::with_limit(second_complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut second_meter,
        6,
        &mut second_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(2)), 1);
    assert_eq!(second_state.outstanding_reservations(), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(6)
    );
  });
}

#[test]
fn mandatory_service_routes_later_retry_through_preplanned_block_deadline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let known_effect = Weight::from_parts(17, 23);
    set_task_effect_actual_weight_override(Some(known_effect));
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(2);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    fund_native(actor_id, 100);
    let retry_key = WakeupKey::Block(5);
    DeadlineHeaders::<Test>::insert(
      retry_key,
      DeadlineHeader {
        first_page: 0,
        last_page: 0,
        next_page: 1,
        first_vacant_page: Some(0),
        page_count: 1,
        count: 1,
      },
    );
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let inspection =
      selector.saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe());
    let complete = inspection
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix)
      .saturating_add(Actors::close_dispatch_weight_upper());
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(2);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut meter = WeightMeter::with_limit(complete.saturating_add(inspection));
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        2,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::ProcessResidenceMismatch)
    );
    assert_eq!(
      meter.consumed(),
      inspection
        .saturating_add(resources.control)
        .saturating_add(suffix)
        .saturating_add(Actors::close_dispatch_weight_upper())
        .saturating_add(known_effect)
    );
    assert_eq!(resource_state.usage().actor_effect_used(), known_effect);
    assert_eq!(
      meter.consumed(),
      resource_state
        .usage()
        .actor_control_used()
        .saturating_add(known_effect)
    );
    assert!(resource_state.optional_actor_work_halted());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));

    DeadlineHeaders::<Test>::remove(retry_key);
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let branch = resources
        .control
        .saturating_add(suffix)
        .saturating_add(Actors::close_dispatch_weight_upper());
      let remaining = inspection.saturating_add(branch).saturating_sub(deficit);
      let consumed_before = budget.limits().actor_control().saturating_sub(remaining);
      let mut refused_state = crate::BlockResourceState::new(2);
      assert_ok!(refused_state.begin_prepass(budget));
      assert_ok!(refused_state.open_external_phase());
      assert_ok!(refused_state.begin_drain());
      let mut prior = refused_state
        .reserve(
          budget.limits(),
          crate::BlockResourceDomain::ActorControl,
          consumed_before,
        )
        .expect("test leaves only a component-wise late deadline-refusal boundary");
      assert_ok!(refused_state.settle(&mut prior, consumed_before));
      let refused_before = refused_state;
      let mut refused_meter = WeightMeter::with_limit(complete);
      assert_eq!(
        Actors::service_canonical_round_head_with_resources(
          &mut refused_meter,
          2,
          &mut refused_state,
          budget.limits(),
          crate::BlockResourceDomain::ActorDrainEffect,
        ),
        Err(ServiceRoundError::ResourceUnavailable)
      );
      assert_eq!(refused_meter.consumed(), inspection);
      assert_eq!(
        refused_state.usage().actor_control_used(),
        refused_before
          .usage()
          .actor_control_used()
          .saturating_add(inspection)
      );
      assert_eq!(refused_state.outstanding_reservations(), 0);
      assert_eq!(ServiceHeader::<Test>::get(), before_header);
      assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
      assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
      assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    }

    // Repair cannot clear the current-block halt. Only a fresh block reopens service authority.
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        2,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect
      ),
      Err(ServiceRoundError::DiscoveryUnavailable)
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    let retry_key = WakeupKey::Block(6);
    resource_state = crate::BlockResourceState::new(3);
    assert_ok!(resource_state.begin_prepass(budget));
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        3,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(retry_key)
    );
    assert!(matches!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Deadline { key, .. }) if key == retry_key
    ));
    let run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run remains canonical");
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.eligible_at, 6);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
  });
}

#[test]
fn mandatory_prepass_recovers_later_retry_and_completes_once_next_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(2);

    run_prepass();
    assert_ne!(Actors::on_idle(2, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(5))
    );
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor_id)
        .unwrap()
        .unsuccessful_attempts_at_cursor,
      1
    );
    assert_eq!(
      crate::CurrentBlockResourceState::<Test>::get()
        .unwrap()
        .phase(),
      crate::BlockResourcePhase::Finalizable
    );

    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    frame_system::Pallet::<Test>::set_block_number(5);
    run_prepass();
    assert_ne!(Actors::on_idle(5, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    run_prepass();
    assert_ne!(Actors::on_idle(6, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), 9);
    let stored = assert_passive_manual_semantics(actor_ref(actor_id, record.generation), 1, 6);
    let mut expected_identity = record.identity;
    expected_identity.cycle_nonce = 1;
    assert_eq!(stored.identity, expected_identity);
    assert_eq!(stored.admission, record.admission);
    let state = crate::CurrentBlockResourceState::<Test>::get().unwrap();
    assert_eq!(state.phase(), crate::BlockResourcePhase::Finalizable);
    assert_ne!(state.usage(), crate::BlockResourceUsage::default());
    assert_eq!(state.outstanding_reservations(), 0);

    assert_eq!(Actors::on_idle(6, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
  });
}

#[test]
fn canonical_service_round_matches_the_independent_semantic_trace() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(120, 1), actor_ref(121, 1), actor_ref(122, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 0).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    let mut block = 0;
    for row in include_str!("../../tests/fixtures/current_state_round_trace_v1.tsv").lines() {
      if row.starts_with('#') || row.is_empty() {
        continue;
      }
      let mut fields = row.split('\t');
      let next_block = fields.next().unwrap().parse::<u64>().unwrap();
      let expected = fields.next().unwrap().parse::<u64>().unwrap();
      let action = fields.next().unwrap();
      assert!(fields.next().is_none());
      if next_block != block {
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          Actors::begin_service_round(next_block).unwrap();
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
        });
        block = next_block;
      }
      let actor = members[(expected - 120) as usize];
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::consider_service_head(block),
          Ok(ServiceRoundEncounter::Eligible(actor))
        );
        if action == "admit" {
          Actors::advance_service_head(actor, block).unwrap();
        } else {
          assert!(matches!(action, "refuse_ref_time" | "refuse_proof_size"));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
  });
}

#[test]
fn canonical_service_ring_appends_partial_round_admission_behind_current_residents() {
  new_test_ext().execute_with(|| {
    let [a, b, c, d] = [120, 121, 122, 123].map(|id| actor_ref(id, 1));
    for actor in [a, b, c] {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 0).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(1).unwrap();
      assert_eq!(
        Actors::consider_service_head(1),
        Ok(ServiceRoundEncounter::Eligible(a))
      );
      Actors::advance_service_head(a, 1).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    ActorProcesses::<Test>::insert(d.actor_id, serving_process(d, ServiceResidenceKind::Live));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(d, ServiceResidenceKind::Live, 1).unwrap();
      Actors::begin_service_round(2).unwrap();
      for expected in [b, c, a, d] {
        assert_eq!(
          Actors::consider_service_head(2),
          Ok(ServiceRoundEncounter::Eligible(expected))
        );
        Actors::advance_service_head(expected, 2).unwrap();
      }
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn due_deadline_reentry_after_round_open_waits_until_next_block() {
  new_test_ext().execute_with(|| {
    let [a, b, c, d] = [120, 121, 122, 123].map(|id| actor_ref(id, 1));
    for actor in [a, b, c] {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 0).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(1).unwrap();
      Actors::advance_service_head(a, 1).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let handle = Actors::plan_deadline_destination(d, WakeupKey::Block(1)).unwrap();
    let mut sleeping = serving_process(d, ServiceResidenceKind::Live);
    sleeping.residence = Some(ProcessResidence::Deadline {
      key: handle.key,
      page: handle.page,
      slot: handle.slot,
    });
    ActorProcesses::<Test>::insert(d.actor_id, sleeping);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_deadline_member(handle).unwrap();
      Actors::return_due_deadline_member_to_service(d, ServiceResidenceKind::Live, 1).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(ServiceHeader::<Test>::get().round_block, Some(1));
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(b));
    let returned = ServiceNodes::<Test>::get(d.actor_id).expect("D returned to Service");
    assert_eq!(returned.eligible_from, 2);
    assert_eq!(returned.last_considered, 1);
    assert!(!DeadlineHandles::<Test>::contains_key(d.actor_id));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).unwrap();
      for expected in [b, c, a, d] {
        assert_eq!(
          Actors::consider_service_head(2),
          Ok(ServiceRoundEncounter::Eligible(expected))
        );
        Actors::advance_service_head(expected, 2).unwrap();
      }
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn canonical_service_round_handles_empty_removal_interruption_and_faults() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::begin_service_round(1),
      Err(ServiceRoundError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(1).expect("empty round begins");
      assert_eq!(
        Actors::consider_service_head(1),
        Ok(ServiceRoundEncounter::Empty)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let first = actor_ref(130, 2);
    let second = actor_ref(131, 2);
    for actor in [first, second] {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    ActorProcesses::<Test>::mutate(first.actor_id, |process| {
      process.as_mut().unwrap().last_attempted = Some(2);
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::AlreadyAttempted(first))
      );
      Actors::converge_attempted_service_head(first, 2).unwrap();
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(second));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
    assert_eq!(
      ServiceNodes::<Test>::get(first.actor_id)
        .unwrap()
        .last_considered,
      1
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::converge_attempted_service_head(first, 2).unwrap();
      Actors::remove_service_member(second).expect("cursor removal selects successor");
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      let node = node.as_mut().unwrap();
      node.eligible_from = 9;
      node.last_considered = 1;
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::FutureMemberUnmarked)
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      node.as_mut().unwrap().generation = 3;
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::StaleGeneration)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      let node = node.as_mut().unwrap();
      node.generation = 2;
      node.eligible_from = 2;
    });
    ActorProcesses::<Test>::mutate(first.actor_id, |process| {
      process.as_mut().unwrap().last_attempted = Some(3);
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::AttemptFromFuture)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
  });
}

#[test]
fn canonical_service_admission_rejects_overflow_and_defers_replacement() {
  new_test_ext().execute_with(|| {
    let actor = actor_ref(140, 1);
    ActorProcesses::<Test>::insert(
      actor.actor_id,
      serving_process(actor, ServiceResidenceKind::Live),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, u64::MAX),
        Err(ServiceRingMutationError::BlockNumberOverflow)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(!ServiceNodes::<Test>::contains_key(actor.actor_id));

    let replacement = actor_ref(actor.actor_id, 2);
    ActorProcesses::<Test>::insert(
      replacement.actor_id,
      serving_process(replacement, ServiceResidenceKind::Live),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(replacement, ServiceResidenceKind::Live, 8).unwrap();
      assert_eq!(
        Actors::consider_service_head(8),
        Ok(ServiceRoundEncounter::Closed)
      );
      let node = ServiceNodes::<Test>::get(replacement.actor_id).unwrap();
      assert_eq!(
        (node.generation, node.eligible_from, node.last_considered),
        (2, 9, 8)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

fn deadline_process(handle: DeadlineHandle<u64>) -> ActorProcess<u64> {
  ActorProcess {
    generation: handle.actor.generation,
    last_attempted: None,
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Deadline {
      key: handle.key,
      page: handle.page,
      slot: handle.slot,
    }),
  }
}

#[test]
fn canonical_deadline_carrier_covers_fragmentation_full_pages_move_and_rollback() {
  new_test_ext().execute_with(|| {
    let key = WakeupKey::Block(20);
    let first = DeadlineHandle {
      actor: actor_ref(200, 3),
      key,
      page: 0,
      slot: 0,
    };
    ActorProcesses::<Test>::insert(first.actor.actor_id, deadline_process(first));
    assert_eq!(
      Actors::insert_deadline_member(first),
      Err(DeadlineMutationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_member(first),
      )
    })
    .expect("first deadline insertion succeeds");

    for slot in 1..32u8 {
      let handle = DeadlineHandle {
        actor: actor_ref(200 + u64::from(slot), 3),
        key,
        page: 0,
        slot,
      };
      ActorProcesses::<Test>::insert(handle.actor.actor_id, deadline_process(handle));
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::insert_deadline_member(handle),
        )
      })
      .expect("page fill succeeds");
    }
    assert_eq!(
      DeadlinePages::<Test>::get(key, 0)
        .expect("page")
        .live_entries,
      32
    );
    let planned_actor = actor_ref(240, 4);
    assert_eq!(
      Actors::plan_deadline_destination(planned_actor, key),
      Ok(DeadlineHandle {
        actor: planned_actor,
        key,
        page: 1,
        slot: 0,
      })
    );
    let page_one = DeadlineHandle {
      actor: actor_ref(239, 4),
      key,
      page: 1,
      slot: 0,
    };
    ActorProcesses::<Test>::insert(page_one.actor.actor_id, deadline_process(page_one));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_member(page_one),
      )
    })
    .expect("second page insertion succeeds");
    assert_eq!(
      Actors::plan_deadline_destination(planned_actor, key),
      Ok(DeadlineHandle {
        actor: planned_actor,
        key,
        page: 1,
        slot: 1,
      })
    );

    let removed = DeadlineHandle {
      actor: actor_ref(207, 3),
      key,
      page: 0,
      slot: 7,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_member(removed.actor),
      )
    })
    .expect("interior removal succeeds");
    assert_eq!(
      Actors::plan_deadline_destination(planned_actor, key),
      Ok(DeadlineHandle {
        actor: planned_actor,
        key,
        page: 0,
        slot: 7,
      })
    );
    let replacement = DeadlineHandle {
      actor: planned_actor,
      key,
      page: 0,
      slot: 7,
    };
    ActorProcesses::<Test>::insert(replacement.actor.actor_id, deadline_process(replacement));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_member(replacement),
      )
    })
    .expect("fragmented slot is reusable");

    let destination = DeadlineHandle {
      actor: first.actor,
      key: WakeupKey::Tick(30),
      page: 0,
      slot: 4,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::move_deadline_member(first.actor, destination),
      )
    })
    .expect("cross-bucket move succeeds");
    assert_eq!(
      DeadlineHandles::<Test>::get(first.actor.actor_id),
      Some(destination)
    );
    assert_eq!(
      DeadlineHeaders::<Test>::get(key)
        .expect("source header")
        .count,
      32
    );

    let before = DeadlineHandles::<Test>::get(first.actor.actor_id);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let invalid = DeadlineHandle {
        actor: first.actor,
        key: WakeupKey::Tick(40),
        page: 9,
        slot: 0,
      };
      assert_eq!(
        Actors::move_deadline_member(first.actor, invalid),
        Err(DeadlineMutationError::InvalidDestination)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DeadlineHandles::<Test>::get(first.actor.actor_id), before);
  });
}

fn storage_root() -> Vec<u8> {
  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1)
}

fn deadline_mutation<R>(
  apply: impl FnOnce() -> Result<R, DeadlineMutationError>,
) -> Result<R, DeadlineMutationError> {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};
  with_transaction_unchecked(|| match apply() {
    Ok(value) => TransactionOutcome::Commit(Ok(value)),
    Err(error) => TransactionOutcome::Rollback(Err(error)),
  })
}

fn assert_deadline_vacancy_topology(
  key: WakeupKey<u64>,
  expected: &BTreeMap<u64, DeadlineHandle<u64>>,
) {
  let pages: BTreeMap<_, _> = DeadlinePages::<Test>::iter_prefix(key).collect();
  let Some(header) = DeadlineHeaders::<Test>::get(key) else {
    assert!(expected.is_empty());
    assert!(pages.is_empty());
    assert!(!DeadlineIndexPositions::<Test>::contains_key(key));
    return;
  };
  assert_eq!(header.count as usize, expected.len());
  assert_eq!(header.page_count as usize, pages.len());
  assert!(header.page_count <= Actors::deadline_destination_page_bound());
  assert!(header.last_page < header.next_page);
  assert!(DeadlineIndexPositions::<Test>::contains_key(key));

  let mut physical = BTreeSet::new();
  let mut nonfull = BTreeSet::new();
  let mut actual = BTreeMap::new();
  let mut previous = None;
  let mut cursor = Some(header.first_page);
  while let Some(id) = cursor {
    assert!(physical.insert(id), "physical page cycle");
    let page = pages.get(&id).expect("physical page exists");
    assert_eq!(page.previous_page, previous);
    assert_eq!(page.entries.len(), 32);
    assert_eq!(
      usize::from(page.live_entries),
      page.entries.iter().filter(|entry| entry.is_some()).count()
    );
    assert!(page.live_entries > 0);
    if page.live_entries < 32 {
      nonfull.insert(id);
    } else {
      assert_eq!(page.previous_vacant_page, None);
      assert_eq!(page.next_vacant_page, None);
    }
    for (slot, actor) in page.entries.iter().enumerate() {
      if let Some(actor) = actor {
        let handle = DeadlineHandle {
          actor: *actor,
          key,
          page: id,
          slot: slot as u8,
        };
        assert!(actual.insert(actor.actor_id, handle).is_none());
        assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), Some(handle));
        assert_eq!(
          ActorProcesses::<Test>::get(actor.actor_id),
          Some(deadline_process(handle))
        );
      }
    }
    previous = Some(id);
    cursor = page.next_page;
  }
  assert_eq!(previous, Some(header.last_page));
  assert_eq!(physical, pages.keys().copied().collect());
  assert_eq!(&actual, expected);

  let mut vacancies = BTreeSet::new();
  previous = None;
  cursor = header.first_vacant_page;
  while let Some(id) = cursor {
    assert!(vacancies.insert(id), "vacancy page cycle");
    let page = pages.get(&id).expect("vacancy page exists");
    assert_eq!(page.previous_vacant_page, previous);
    assert!(page.live_entries < 32);
    previous = Some(id);
    cursor = page.next_vacant_page;
  }
  assert_eq!(
    vacancies, nonfull,
    "every nonfull page is reachable exactly once"
  );
}

fn insert_planned_deadline(actor: ActorRef, key: WakeupKey<u64>) -> DeadlineHandle<u64> {
  let before = storage_root();
  let handle = Actors::plan_deadline_destination(actor, key).expect("destination exists");
  assert_eq!(storage_root(), before, "planning is read-only");
  deadline_mutation(|| {
    ActorProcesses::<Test>::insert(actor.actor_id, deadline_process(handle));
    Actors::insert_deadline_member(handle)
  })
  .expect("planned insertion succeeds");
  handle
}

#[test]
fn deadline_vacancy_membership_survives_all_page_removal_orders_and_refills() {
  for order in [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
  ] {
    new_test_ext().execute_with(|| {
      let key = WakeupKey::Block(77);
      let mut expected = BTreeMap::new();
      for id in 0..96 {
        let handle = insert_planned_deadline(actor_ref(10_000 + id, 1), key);
        expected.insert(handle.actor.actor_id, handle);
        assert_deadline_vacancy_topology(key, &expected);
      }
      let mut holes = Vec::new();
      for page in order {
        let id = 10_000 + page * 32 + 15;
        let handle = expected.remove(&id).unwrap();
        assert_eq!(
          deadline_mutation(|| Actors::remove_deadline_member(handle.actor)),
          Ok(handle)
        );
        holes.push(handle);
        assert_deadline_vacancy_topology(key, &expected);
      }
      // Fill retained holes from vacancy tail to head, including physical/vacancy neighbor overlap.
      for old in holes {
        let handle = DeadlineHandle {
          actor: actor_ref(old.actor.actor_id, 2),
          ..old
        };
        deadline_mutation(|| {
          ActorProcesses::<Test>::insert(handle.actor.actor_id, deadline_process(handle));
          Actors::insert_deadline_member(handle)
        })
        .unwrap();
        expected.insert(handle.actor.actor_id, handle);
        assert_deadline_vacancy_topology(key, &expected);
        let before = storage_root();
        assert_eq!(
          deadline_mutation(|| Actors::remove_deadline_member(old.actor)),
          Err(DeadlineMutationError::StaleGeneration)
        );
        assert_eq!(storage_root(), before);
      }
      for page in order {
        for turn in 0..32 {
          let id = 10_000 + page * 32 + (turn * 17) % 32;
          let handle = expected.remove(&id).unwrap();
          assert_eq!(
            deadline_mutation(|| Actors::remove_deadline_member(handle.actor)),
            Ok(handle)
          );
          assert!(!DeadlineHandles::<Test>::contains_key(id));
          assert_deadline_vacancy_topology(key, &expected);
        }
      }
      let replacement = insert_planned_deadline(actor_ref(10_000, 3), key);
      assert_eq!((replacement.page, replacement.slot), (0, 0));
      expected.insert(replacement.actor.actor_id, replacement);
      assert_deadline_vacancy_topology(key, &expected);
    });
  }
}

#[test]
fn deadline_vacancy_moves_preserve_exact_survivors_and_rollback_late_refusal() {
  new_test_ext().execute_with(|| {
    let source = WakeupKey::Block(77);
    let target = WakeupKey::Tick(91);
    let mut expected = BTreeMap::new();
    for id in 0..65 {
      let handle = insert_planned_deadline(actor_ref(20_000 + id, 1), source);
      expected.insert(handle.actor.actor_id, handle);
    }
    let original = expected[&20_000];
    let destination = Actors::plan_deadline_destination(original.actor, source).unwrap();
    Actors::move_deadline_member(original.actor, destination).unwrap();
    expected.insert(original.actor.actor_id, destination);
    assert_deadline_vacancy_topology(source, &expected);

    let before = storage_root();
    assert_eq!(
      Actors::move_deadline_member(
        original.actor,
        DeadlineHandle {
          key: target,
          page: 9,
          ..destination
        }
      ),
      Err(DeadlineMutationError::InvalidDestination)
    );
    assert_eq!(
      storage_root(),
      before,
      "late insertion refusal restores source and index"
    );

    let target_handle = Actors::plan_deadline_destination(original.actor, target).unwrap();
    Actors::move_deadline_member(original.actor, target_handle).unwrap();
    expected.remove(&original.actor.actor_id);
    assert_deadline_vacancy_topology(source, &expected);
    assert_deadline_vacancy_topology(
      target,
      &BTreeMap::from([(original.actor.actor_id, target_handle)]),
    );

    let before = storage_root();
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let back = Actors::plan_deadline_destination(original.actor, source).unwrap();
      Actors::move_deadline_member(original.actor, back).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      storage_root(),
      before,
      "outer refusal restores both memberships"
    );
  });
}

#[test]
fn deadline_vacancy_neighbor_refusal_rolls_back_earlier_link_writes() {
  new_test_ext().execute_with(|| {
    let key = WakeupKey::Block(77);
    let mut expected = BTreeMap::new();
    for id in 0..65 {
      let handle = insert_planned_deadline(actor_ref(25_000 + id, 1), key);
      expected.insert(handle.actor.actor_id, handle);
    }
    let mut holes = Vec::new();
    for id in [25_015, 25_047] {
      let handle = expected.remove(&id).unwrap();
      deadline_mutation(|| Actors::remove_deadline_member(handle.actor)).unwrap();
      holes.push(handle);
    }
    assert_deadline_vacancy_topology(key, &expected);
    // Filling page 0 first writes its vacancy predecessor (1), then checks successor (2).
    DeadlinePages::<Test>::mutate(key, 2, |page| {
      page.as_mut().unwrap().previous_vacant_page = Some(99);
    });
    let before = storage_root();
    assert_eq!(
      deadline_mutation(|| Actors::insert_deadline_member(holes[0])),
      Err(DeadlineMutationError::CorruptCarrier)
    );
    assert_eq!(storage_root(), before);
    DeadlinePages::<Test>::mutate(key, 2, |page| {
      page.as_mut().unwrap().previous_vacant_page = Some(0);
    });
    deadline_mutation(|| Actors::insert_deadline_member(holes[0])).unwrap();
    expected.insert(holes[0].actor.actor_id, holes[0]);
    assert_deadline_vacancy_topology(key, &expected);
  });
}

#[test]
fn deadline_vacancy_rejects_truncated_pages_without_panicking() {
  new_test_ext().execute_with(|| {
    let key = WakeupKey::Block(77);
    let existing = insert_planned_deadline(actor_ref(30_000, 1), key);
    let handle = DeadlineHandle {
      actor: actor_ref(30_001, 1),
      slot: 7,
      ..existing
    };
    ActorProcesses::<Test>::insert(handle.actor.actor_id, deadline_process(handle));
    DeadlinePages::<Test>::mutate(key, 0, |page| {
      page.as_mut().unwrap().entries.truncate(1);
    });
    let before = storage_root();
    assert_eq!(
      deadline_mutation(|| Actors::insert_deadline_member(handle)),
      Err(DeadlineMutationError::CorruptCarrier)
    );
    assert_eq!(storage_root(), before);
    assert_eq!(
      Actors::move_deadline_member(
        existing.actor,
        DeadlineHandle {
          actor: existing.actor,
          ..handle
        }
      ),
      Err(DeadlineMutationError::CorruptCarrier)
    );
    assert_eq!(storage_root(), before);
  });
}

#[test]
fn deadline_vacancy_rejects_a_missing_head_without_stranding_free_slots() {
  new_test_ext().execute_with(|| {
    let key = WakeupKey::Block(77);
    let first = insert_planned_deadline(actor_ref(31_000, 1), key);
    let next = DeadlineHandle {
      actor: actor_ref(31_001, 1),
      slot: 1,
      ..first
    };
    ActorProcesses::<Test>::insert(next.actor.actor_id, deadline_process(next));
    DeadlineHeaders::<Test>::mutate(key, |header| {
      header.as_mut().unwrap().first_vacant_page = None;
    });
    let before = storage_root();
    assert_eq!(
      Actors::plan_deadline_destination(next.actor, key),
      Err(DeadlineMutationError::CorruptCarrier)
    );
    assert_eq!(
      deadline_mutation(|| Actors::insert_deadline_member(next)),
      Err(DeadlineMutationError::CorruptCarrier)
    );
    assert_eq!(storage_root(), before);
  });
}

#[test]
fn deadline_branch_envelopes_combine_independent_resource_dimensions() {
  type W = <Test as crate::Config>::WeightInfo;
  let populated_return = <W as crate::weights::WeightInfo>::return_due_block_deadline_to_service();
  let deep_return =
    <W as crate::weights::WeightInfo>::return_due_block_deadline_to_service_deep_index();
  let returning = Actors::deadline_return_weight_upper();
  assert_eq!(
    returning.ref_time(),
    populated_return.ref_time().max(deep_return.ref_time())
  );
  assert_eq!(
    returning.proof_size(),
    populated_return.proof_size().max(deep_return.proof_size())
  );
  assert!(deep_return.ref_time() > populated_return.ref_time());
  assert!(populated_return.proof_size() > deep_return.proof_size());

  let retry = <W as crate::weights::WeightInfo>::scheduler_service_retry_to_deadline_new_key();
  let due = <W as crate::weights::WeightInfo>::scheduler_due_deadline_to_service_deep_index();
  let outer = Actors::scheduler_complete_outer_weight_upper();
  assert!(retry.all_lte(outer));
  assert!(due.all_lte(outer));
  assert!(retry.ref_time() > due.ref_time());
  assert!(due.proof_size() > retry.proof_size());
}

#[test]
fn deadline_destination_search_is_bounded_by_reachable_active_population() {
  new_test_ext().execute_with(|| {
    let page_count = Actors::deadline_destination_page_bound();
    assert_eq!(page_count, 313);
    let key = WakeupKey::Block(77);
    let maximum: u32 = <Test as crate::Config>::MaxActiveActors::get();
    let mut count = 0u32;
    for page_index in 0..page_count {
      let preceding = page_index * 32;
      let live = if page_index + 1 < page_count {
        32
      } else {
        maximum.saturating_sub(preceding).saturating_sub(1)
      };
      let mut entries = vec![None; 32];
      for slot in 0..live {
        entries[slot as usize] = Some(actor_ref(u64::from(preceding + slot), 1));
      }
      DeadlinePages::<Test>::insert(
        key,
        u64::from(page_index),
        DeadlinePage {
          previous_page: page_index.checked_sub(1).map(u64::from),
          next_page: (page_index + 1 < page_count).then(|| u64::from(page_index + 1)),
          previous_vacant_page: None,
          next_vacant_page: None,
          live_entries: live as u8,
          entries: BoundedVec::try_from(entries).expect("C32 page fits"),
        },
      );
      count += live;
    }
    DeadlineHeaders::<Test>::insert(
      key,
      DeadlineHeader {
        first_page: 0,
        last_page: u64::from(page_count - 1),
        next_page: u64::from(page_count),
        first_vacant_page: Some(u64::from(page_count - 1)),
        page_count,
        count,
      },
    );
    let actor = actor_ref(u64::from(count), 1);
    assert_eq!(
      Actors::plan_deadline_destination(actor, key),
      Ok(DeadlineHandle {
        actor,
        key,
        page: u64::from(page_count - 1),
        slot: (count % 32) as u8,
      })
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_deadline_index(key).expect("maximum bucket is indexed");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let append = DeadlineHandle {
      actor,
      key,
      page: u64::from(page_count),
      slot: 0,
    };
    let before = storage_root();
    assert_eq!(
      deadline_mutation(|| {
        ActorProcesses::<Test>::insert(actor.actor_id, deadline_process(append));
        Actors::insert_deadline_member(append)
      }),
      Err(DeadlineMutationError::CapacityExceeded)
    );
    assert_eq!(
      storage_root(),
      before,
      "page saturation cannot grow the carrier"
    );

    insert_planned_deadline(actor, key);
    assert_eq!(DeadlineHeaders::<Test>::get(key).unwrap().count, maximum);
    let extra = Actors::plan_deadline_destination(actor_ref(u64::from(maximum), 1), key).unwrap();
    let before = storage_root();
    assert_eq!(
      deadline_mutation(|| {
        ActorProcesses::<Test>::insert(extra.actor.actor_id, deadline_process(extra));
        Actors::insert_deadline_member(extra)
      }),
      Err(DeadlineMutationError::CapacityExceeded)
    );
    assert_eq!(
      storage_root(),
      before,
      "member saturation preserves every existing obligation"
    );

    DeadlineHeaders::<Test>::mutate(key, |header| {
      header.as_mut().expect("header exists").page_count = page_count + 1;
    });
    assert_eq!(
      Actors::plan_deadline_destination(actor, key),
      Err(DeadlineMutationError::CorruptCarrier)
    );
  });
}

fn install_indexed_deadline_bucket(key: WakeupKey<u64>, actor_id: u64) -> DeadlineHandle<u64> {
  let handle = DeadlineHandle {
    actor: actor_ref(actor_id, 1),
    key,
    page: 0,
    slot: 0,
  };
  ActorProcesses::<Test>::insert(actor_id, deadline_process(handle));
  polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    Actors::insert_deadline_member(handle).expect("deadline bucket and index insertion succeeds");
    polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
  });
  handle
}

#[test]
fn canonical_deadline_index_covers_order_pages_stale_state_and_rollback() {
  new_test_ext().execute_with(|| {
    let mut handles = BTreeMap::new();
    for deadline in (1..=65u64).rev() {
      let key = WakeupKey::Block(deadline);
      handles.insert(key, install_indexed_deadline_bucket(key, 1_000 + deadline));
    }
    for deadline in [9, 3, 12] {
      let key = WakeupKey::Tick(deadline);
      handles.insert(key, install_indexed_deadline_bucket(key, 2_000 + deadline));
    }

    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Block), 65);
    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Tick), 3);
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0)
        .unwrap()
        .len(),
      32
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 1)
        .unwrap()
        .len(),
      32
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 2)
        .unwrap()
        .len(),
      1
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(1)
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Tick, 0).unwrap()[0],
      WakeupKey::Tick(3)
    );

    for (key, index) in DeadlineIndexPositions::<Test>::iter() {
      let page = DeadlineIndexPages::<Test>::get(key.clock(), u64::from(index / 32)).unwrap();
      assert_eq!(page[(index % 32) as usize], key);
      if index > 0 {
        let parent = (index - 1) / 2;
        let parent_page =
          DeadlineIndexPages::<Test>::get(key.clock(), u64::from(parent / 32)).unwrap();
        assert!(parent_page[(parent % 32) as usize] <= key);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::update_deadline_index(key),
        )
      })
      .expect("clustered and overdue key remains structurally valid");
    }

    let removed_key = WakeupKey::Block(1);
    let removed = handles[&removed_key];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::remove_deadline_member(removed.actor)
        .expect("empty bucket removal repairs the deadline index");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Block), 64);
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(2)
    );
    assert!(!DeadlineIndexPages::<Test>::contains_key(
      WakeupClock::Block,
      2
    ));

    let stale_key = WakeupKey::Block(10);
    let original = DeadlineIndexPositions::<Test>::get(stale_key).unwrap();
    DeadlineIndexPositions::<Test>::insert(stale_key, 999);
    let stale = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::update_deadline_index(stale_key),
      )
    });
    assert_eq!(stale, Err(DeadlineIndexMutationError::StaleIndex));
    DeadlineIndexPositions::<Test>::insert(stale_key, original);

    let rollback_key = WakeupKey::Block(0);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      install_indexed_deadline_bucket(rollback_key, 3_000);
      assert_eq!(DeadlineIndexPositions::<Test>::get(rollback_key), Some(0));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(!DeadlineHeaders::<Test>::contains_key(rollback_key));
    assert!(!DeadlineIndexPositions::<Test>::contains_key(rollback_key));
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(2)
    );
  });
}

#[test]
fn canonical_deadline_index_rejects_missing_headers_legacy_authority_and_early_remove() {
  new_test_ext().execute_with(|| {
    let missing = WakeupKey::Block(40);
    assert_eq!(
      Actors::insert_deadline_index(missing),
      Err(DeadlineIndexMutationError::TransactionRequired)
    );
    let missing_result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_index(missing),
      )
    });
    assert_eq!(
      missing_result,
      Err(DeadlineIndexMutationError::HeaderMissing)
    );

    let key = WakeupKey::Block(41);
    let handle = install_indexed_deadline_bucket(key, 4_000);
    let header = DeadlineHeaders::<Test>::get(key).unwrap();
    DeadlineHeaders::<Test>::mutate(key, |stored| stored.as_mut().unwrap().count = 0);
    let corrupt = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::update_deadline_index(key),
      )
    });
    assert_eq!(corrupt, Err(DeadlineIndexMutationError::CorruptHeader));
    DeadlineHeaders::<Test>::insert(key, header);

    let early_remove = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_index(key),
      )
    });
    assert_eq!(early_remove, Err(DeadlineIndexMutationError::CorruptHeader));

    ActorWaitingOccupancies::<Test>::insert(key, 1);
    let legacy = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_index(key),
      )
    });
    assert_eq!(
      legacy,
      Err(DeadlineIndexMutationError::LegacyAuthorityPresent)
    );
    assert!(DeadlineIndexPositions::<Test>::contains_key(key));
    ActorWaitingOccupancies::<Test>::remove(key);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::remove_deadline_member(handle.actor).expect("bucket and index remove together");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

fn test_pipeline_machine_envelope() -> crate::PipelineMachineEnvelope<Balance> {
  crate::PipelineMachineEnvelope {
    pipeline_machine_fee_upper: 11,
  }
}

fn geometry_certificate(
  contract: &RuntimeActorContract,
) -> crate::ActorAdmissionCertificateOf<Test> {
  crate::ActorAdmissionCertificate::new(
    contract.semantic_contract_id(),
    contract.body_commitment().expect("body commitment"),
    contract.trigger.wake_qualification(&contract.window),
    1,
    [4u8; 32],
    1,
    [6u8; 32],
    Weight::from_parts(77, 88),
  )
}

#[test]
fn actor_identity_resolves_lifecycle_owner_and_fails_closed_on_missing_primary() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let identity = Actors::actor_identity(actor_id).expect("active identity exists");
    let semantic = crate::ActorSemanticStates::<Test>::take(actor_id)
      .expect("Manual Actor starts with one semantic authority");
    assert!(Actors::actor_identity(actor_id).is_none());
    crate::ActorSemanticStates::<Test>::insert(actor_id, semantic);
    assert_eq!(Actors::actor_identity(actor_id), Some(identity.clone()));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let dormant = Actors::actor_identity(actor_id).expect("dormant identity exists");
    assert_eq!(dormant.sovereign_account, identity.sovereign_account);
    assert_eq!(dormant.owner, identity.owner);
    assert!(Actors::actor_identity(actor_id + 1).is_none());
  });
}

#[test]
fn public_api_error_signatures_use_shared_typed_cores() {
  let _: fn(ActorId) -> Result<ActorEligibility<u64, Balance>, ActorClassificationError> =
    Actors::actor_eligibility;
  let _: fn(ActorId) -> Result<crate::ActorCostQuote<Balance>, ActorCostQuoteError> =
    Actors::actor_cost_quote;
  let _: fn(
    ActorId,
    ActorType,
    Mutability,
    RuntimeActorContract,
    SimulationMode,
    crate::SimulationBudget,
  ) -> Result<crate::SimulationResult, SimulationError> = Actors::simulate_current_contract;

  let classification_cases = [
    (
      ActorClassificationError::ActorInvariant,
      Error::<Test>::ActorInvariant,
    ),
    (
      ActorClassificationError::RunInvariant,
      Error::<Test>::ActorRunInvariant,
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
fn state_hold_encoding_consumers_separate_actual_body_from_control_capacity() {
  use codec::MaxEncodedLen;

  new_test_ext().execute_with(|| {
    for count in [0, 1, 5] {
      let step = transfer_contract_steps(BOB, 2)[0].clone();
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        vec![step; count].try_into().unwrap(),
      );
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let head = ActorContractHeads::<Test>::get(actor_id).unwrap();
      let admission = Actors::load_control_admission(actor_id).unwrap();
      let hold = crate::ActorStateHolds::<Test>::get(actor_id)
        .unwrap()
        .breakdown;
      let per_byte: Balance = <Test as crate::Config>::ActorStateHoldPerByte::get();
      let base: Balance = <Test as crate::Config>::ActorStateHoldBase::get();
      let price = |bytes: usize| {
        if bytes == 0 {
          0
        } else {
          base + bytes as Balance * per_byte
        }
      };
      let capacity_without_resources =
        crate::ActorControlHotState::<MockBlockNumber>::max_encoded_len()
          + u32::max_encoded_len()
          + Option::<MockBlockNumber>::max_encoded_len();
      let actual_head = head.encoded_size() + admission.encoded_size();
      let declared_resource = ActorStepResourceEnvelope::max_encoded_len();
      assert_eq!(
        hold.contract_head,
        price(capacity_without_resources + declared_resource + actual_head)
      );
      let actual_tails: usize = ActorContractTailChunks::<Test>::iter_prefix(actor_id)
        .map(|(_, chunk)| chunk.encoded_size())
        .sum();
      assert_eq!(hold.contract_body, price(actual_tails));
      assert_eq!(
        hold.run,
        price(crate::ActorRunStateOf::<Test>::max_encoded_len())
      );

      // The reserved slot covers both compact Weight values without a per-Step surcharge.
      let compact_resource = 4 * codec::Compact::<u64>::max_encoded_len();
      assert_eq!(declared_resource, compact_resource);
      assert_eq!(
        price(capacity_without_resources + compact_resource + actual_head),
        hold.contract_head,
      );
      let quote = Actors::actor_cost_quote(actor_id).unwrap();
      assert_eq!(
        quote.actor_state_hold.total,
        [
          hold.identity,
          hold.contract_head,
          hold.contract_body,
          hold.detector,
          hold.run
        ]
        .into_iter()
        .sum::<Balance>()
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before,
      );
    }
  });
}

#[test]
fn actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let quote = Actors::actor_cost_quote(actor_id).expect("active User cost quote exists");
    assert_eq!(quote.actor_type, ActorType::User);
    assert_eq!(quote.creation_fee, 10);
    let trigger = quote
      .prospective_trigger_fee
      .expect("active Trigger quote exists");
    assert_eq!(trigger.trigger_family, TriggerFamily::Manual);
    assert_eq!(
      trigger.maximum_weight,
      <TestWeightInfo as crate::WeightInfo>::manual_trigger()
    );
    assert_eq!(trigger.fee, manual_trigger_fee());
    assert_ne!(trigger.production_weight_identity, [0; 32]);
    let pipeline = quote
      .prospective_pipeline_fee
      .expect("active Pipeline quote exists");
    assert_eq!(
      pipeline.strategy,
      PipelineMachineFeeStrategy::UpfrontBounded
    );
    assert_eq!(pipeline.total_fee, pipeline.pipeline_machine_fee);
    assert_eq!(
      pipeline.production_weight_identity,
      crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
    );
    let loaded =
      Actors::load_current_step_from_storage(actor_id, 0).expect("current Step resources exist");
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_weight,
      loaded.resources.effect
    );
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_fee,
      TestWeightToFee::weight_to_fee(&loaded.resources.effect)
    );
    let hold = crate::ActorStateHolds::<Test>::get(actor_id)
      .expect("User hold record exists")
      .breakdown;
    let expected_hold_total = [
      hold.identity,
      hold.contract_head,
      hold.contract_body,
      hold.detector,
      hold.run,
    ]
    .into_iter()
    .sum::<Balance>();
    assert_eq!(quote.actor_state_hold.total, expected_hold_total);
    assert!(!quote.actor_state_hold.exempt);
    assert_eq!(quote.actor_state_hold.base_per_component, 1);
    assert_eq!(quote.actor_state_hold.per_encoded_byte, 1);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(BOB),
      Mutability::Mutable,
      None,
    ));
    let dormant_id = Actors::next_actor_id() - 1;
    let dormant = Actors::actor_cost_quote(dormant_id).expect("Dormant quote exists");
    assert!(dormant.prospective_trigger_fee.is_none());
    assert!(dormant.prospective_pipeline_fee.is_none());
    assert_eq!(dormant.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(dormant.actor_state_hold.total > 0);

    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let system = Actors::actor_cost_quote(system_id).expect("System quote exists");
    assert_eq!(system.actor_type, ActorType::System);
    assert_eq!(system.creation_fee, 0);
    assert_eq!(system.prospective_trigger_fee.expect("Trigger").fee, 0);
    assert_eq!(
      system.prospective_pipeline_fee.expect("Pipeline").total_fee,
      0
    );
    assert_eq!(system.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(system.actor_state_hold.exempt);
    assert_eq!(system.actor_state_hold.total, 0);

    assert_eq!(
      Actors::actor_cost_quote(u64::MAX),
      Err(ActorCostQuoteError::ActorNotFound)
    );
  });
}

#[test]
fn contract_header_extracts_only_hot_metadata_and_runtime_bindings() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  let semantic_contract_id = [1u8; 32];
  let body_commitment = [2u8; 32];
  let admission_identity = [3u8; 32];

  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded Contract produces a header");

  assert_eq!(header.trigger, contract.trigger);
  assert_eq!(header.cooldown_blocks, contract.cooldown_blocks);
  assert_eq!(header.window, contract.window);
  assert_eq!(header.funding, contract.funding);
  assert_eq!(header.has_close_when, contract.close_when.is_some());
  assert_eq!(header.on_unfunded, contract.on_unfunded);
  assert_eq!(
    header.auto_close_at_cycle_nonce,
    contract.auto_close_at_cycle_nonce
  );
  assert_eq!(
    header.step_count,
    u32::try_from(contract.steps.len()).expect("bounded Step count fits u32")
  );
  assert_eq!(header.semantic_contract_id, semantic_contract_id);
  assert_eq!(header.body_commitment, body_commitment);
  assert_eq!(header.admission_identity, admission_identity);
  assert_eq!(
    header.pipeline_machine_envelope,
    test_pipeline_machine_envelope()
  );
}

#[test]
fn semantic_contract_id_uses_fixed_domain_and_authored_field_order() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  assert_eq!(crate::ACTOR_CONTRACT_HASH_DOMAIN, *b"DEOS_ACTOR_CONTRACT");
  let expected = (
    crate::ACTOR_CONTRACT_HASH_DOMAIN,
    (
      &contract.trigger,
      contract.cooldown_blocks,
      &contract.window,
      &contract.funding,
      &contract.close_when,
      contract.on_unfunded,
      &contract.parked_balance_activation,
      contract.auto_close_at_cycle_nonce,
    ),
    &contract.steps,
  )
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.semantic_contract_id(), expected);

  let mut changed = contract.clone();
  changed.cooldown_blocks = changed.cooldown_blocks.saturating_add(1);
  assert_ne!(
    contract.semantic_contract_id(),
    changed.semantic_contract_id()
  );
}

#[test]
fn body_commitment_uses_fixed_domain_and_exact_ordered_indexes() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  assert_eq!(crate::ACTOR_BODY_HASH_DOMAIN, *b"DEOS_ACTOR_BODY");
  let indexed_steps = vec![(0u32, &contract.steps[0]), (1u32, &contract.steps[1])];
  let expected = (crate::ACTOR_BODY_HASH_DOMAIN, indexed_steps.as_slice())
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.body_commitment(), Some(expected));

  let mut reordered = contract.clone();
  reordered.steps =
    BoundedVec::try_from(vec![contract.steps[1].clone(), contract.steps[0].clone()])
      .expect("reordered Steps fit");
  assert_ne!(contract.body_commitment(), reordered.body_commitment());
}

#[test]
fn contract_commitment_hashing_visits_each_step_once() {
  use std::cell::Cell;

  // This probes the generic codec owners, not admission or Actor history.
  struct CountedStep<'a> {
    value: u32,
    visits: &'a Cell<usize>,
  }
  impl Encode for CountedStep<'_> {
    fn size_hint(&self) -> usize {
      4
    }

    fn encode_to<O: codec::Output + ?Sized>(&self, output: &mut O) {
      self.visits.set(self.visits.get() + 1);
      self.value.encode_to(output);
    }
  }

  for count in [0usize, 1, 12, 63, 64] {
    let visits = Cell::new(0);
    let contract = crate::ActorContract {
      trigger: (),
      cooldown_blocks: 0,
      window: None::<ScheduleWindow<u32>>,
      steps: (0..count)
        .map(|index| CountedStep {
          value: index as u32,
          visits: &visits,
        })
        .collect::<Vec<_>>(),
      funding: (),
      on_unfunded: crate::OnUnfunded::Close,
      close_when: None::<crate::CloseWhenOf<Test>>,
      parked_balance_activation: None::<()>,
      auto_close_at_cycle_nonce: None,
    };
    let authored = (
      &contract.trigger,
      contract.cooldown_blocks,
      &contract.window,
      &contract.funding,
      &contract.close_when,
      contract.on_unfunded,
      &contract.parked_balance_activation,
      contract.auto_close_at_cycle_nonce,
    );
    let semantic_input = (crate::ACTOR_CONTRACT_HASH_DOMAIN, authored, &contract.steps).encode();
    let prefix = codec::Compact(count as u32).encoded_size();
    assert_eq!(
      semantic_input.len(),
      19 + authored.encoded_size() + prefix + 4 * count
    );
    visits.set(0);
    assert_eq!(
      contract.semantic_contract_id(),
      frame::hashing::blake2_256(&semantic_input)
    );
    assert_eq!(visits.get(), count);

    let indexed = contract
      .steps
      .iter()
      .enumerate()
      .map(|(index, step)| (index as u32, step))
      .collect::<Vec<_>>();
    let body_input = (crate::ACTOR_BODY_HASH_DOMAIN, indexed).encode();
    assert_eq!(body_input.len(), 15 + prefix + (4 + 4) * count);
    visits.set(0);
    assert_eq!(
      contract.body_commitment(),
      Some(frame::hashing::blake2_256(&body_input))
    );
    assert_eq!(visits.get(), count);
  }
}

#[test]
fn current_step_service_state_does_not_load_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let tail = ActorContractTailChunks::<Test>::take(actor_id, 0).expect("authored tail exists");

    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("Step 0 needs no tail chunk");
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert_eq!(state.contract.steps.len(), 8);
    assert_eq!(loaded_step.cursor, 0);
    assert_eq!(
      loaded_step.resources,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources"),
      "the loaded Step keeps exact fragment authority"
    );
    assert!(admission.has_valid_identity());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    System::reset_events();
    // Removing an unreachable tail is a read-isolation falsifier, not a valid published Contract.
    // Opening needs the authenticated header/current Step and stored Pipeline envelope only.
    run_canonical_round_at(2, Weight::MAX);
    assert!(
      Actors::actor_hot(actor_id)
        .is_some_and(|hot| !hot.pending_signal && hot.cycle_state == CycleState::Idle)
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, cycle_nonce: 1 } if *id == actor_id
    )));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    ActorContractTailChunks::<Test>::insert(actor_id, 0, tail);
    assert_eq!(Actors::actor_identity(actor_id).unwrap().cycle_nonce, 1);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn run_state_authority_remains_coherent_across_progress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = inert_contract_steps()[0].clone();
    let steps =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_next_idle(Weight::MAX);
    let first = ActorRunStateStore::<Test>::get(actor_id).expect("canonical run state exists");
    assert_eq!(first.cursor, 1);
    let authority = first.contract_authority;

    run_next_idle(Weight::MAX);
    let second = ActorRunStateStore::<Test>::get(actor_id).expect("canonical run state persists");
    assert_eq!(second.cursor, 2);
    assert_eq!(second.contract_authority, authority);

    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("canonical run state exists")
        .contract_authority
        .body_commitment[0] ^= 1;
    });
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn current_fragment_resources_are_authoritative_without_certificate_duplication() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      let head = maybe.as_mut().expect("canonical head exists");
      let resources = head
        .first_step_resources
        .as_mut()
        .expect("nonempty Contract has inline Step resources");
      resources.control = resources.control.saturating_add(Weight::from_parts(1, 0));
    });
    let (_, _, loaded) = Actors::load_current_step_service_state(actor_id)
      .expect("current fragment remains authoritative");
    assert_eq!(
      loaded.resources.control,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources")
        .control
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
  });
}

#[test]
fn running_execution_and_post_placement_ignore_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let inert = inert_contract_steps()[0].clone();
    let steps = BoundedVec::try_from((0..8).map(|_| inert.clone()).collect::<Vec<_>>())
      .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_next_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|run| run.cursor),
      Some(1)
    );

    ActorContractTailChunks::<Test>::remove(actor_id, 1);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    // The running suffix is republished as a canonical Pending Service for the following block;
    // its current Step lives in tail chunk 0, so the unreached tail chunk 1 is never loaded.
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(Actors::load_current_step_service_state(actor_id).is_some());
    run_next_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped { actor_id: id, step_index: 1, .. } if *id == actor_id
    )));
    let run = ActorRunStateStore::<Test>::get(actor_id).expect("Running suffix remains live");
    assert_eq!(run.cursor, 2);
    assert_eq!(run.last_committed_step_block, Some(3));
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.cycle_state == CycleState::Running));
  });
}

#[test]
fn step_ticket_binds_run_cursor_fifo_eligibility_and_contract_commitment() {
  let ticket = crate::ActorStepTicket {
    actor_id: 7,
    cycle_nonce: 8,
    cursor: 2,
    ticket: 9,
    eligible_at: 10u64,
    contract_commitment: crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  };
  let decoded =
    crate::ActorStepTicket::decode(&mut ticket.encode().as_slice()).expect("Step ticket decodes");
  assert_eq!(decoded, ticket);
  assert_eq!(decoded.ticket, 9);
  assert_eq!(decoded.cursor, 2);
  assert_eq!(decoded.eligible_at, 10);
  assert!(decoded.matches(
    7,
    8,
    2,
    9,
    &10,
    &crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  ));
  assert!(!decoded.matches(7, 8, 3, 9, &10, &decoded.contract_commitment,));
}

#[test]
fn actor_storage_metadata_declares_current_capacity_owners() {
  use polkadot_sdk::frame_support::traits::{ConstU32, StorageInfoTrait};

  // No externalities: these are configured bounds, never a mutable population or active limit.
  let metadata = Actors::storage_info();
  for name in [
    "ActorIdentities",
    "ActorSemanticStates",
    "ActorStateHolds",
    "OwnerSlotBitmaps",
    "SovereignIndex",
  ] {
    let info = metadata
      .iter()
      .find(|info| info.storage_name == name.as_bytes())
      .unwrap();
    assert_eq!(
      info.max_values,
      Some(<Test as crate::Config>::MaxActorIdentities::get())
    );
  }
  for name in [
    "ActorContractHead",
    "ActorBehaviorPolicies",
    "ActorBehaviorMonitors",
    "ActorFinancialWaits",
    "ActorRunState",
    "ActorProcesses",
    "ServiceNodes",
  ] {
    let info = metadata
      .iter()
      .find(|info| info.storage_name == name.as_bytes())
      .unwrap();
    assert_eq!(
      info.max_values,
      Some(<Test as crate::Config>::MaxActiveActors::get())
    );
  }
  let tails = metadata
    .iter()
    .find(|info| info.storage_name == b"ActorContractTailChunk")
    .unwrap();
  assert_eq!(tails.max_values, Some(30_000));

  for name in [
    "BalanceDependencySources",
    "DependencySourceBalances",
    "DependencyRevisions",
    "DependencyScanSourceNodes",
    "DependencyRegistrationHeaders",
    "DependencyRegistrationPages",
    "DependencyRegistrationPositions",
    "DependencyRegistrations",
  ] {
    let info = metadata
      .iter()
      .find(|info| info.storage_name == name.as_bytes())
      .unwrap();
    assert_eq!(info.max_values, Some(40_000), "{name}");
  }
  for name in [
    "DependencyRegistrationFreePositions",
    "DependencyRegistrationFreeIndices",
  ] {
    let info = metadata
      .iter()
      .find(|info| info.storage_name == name.as_bytes())
      .unwrap();
    assert_eq!(info.max_values, Some(1_240_000), "{name}");
  }
  assert_eq!(
    crate::DependencyStorageValues::<ConstU32<10>, ConstU32<0>, 1>::get(),
    Some(0)
  );
  assert_eq!(
    crate::DependencyStorageValues::<ConstU32<10>, ConstU32<6>, 31>::get(),
    Some(1860)
  );
  assert_eq!(
    crate::DependencyStorageValues::<ConstU32<{ u32::MAX }>, ConstU32<1>, 31>::get(),
    None
  );
  assert_eq!(
    crate::DependencyStorageValues::<ConstU32<{ u32::MAX }>, ConstU32<2>, 1>::get(),
    None
  );

  assert_eq!(
    crate::ContractTailStorageValues::<ConstU32<10>, ConstU32<0>>::get(),
    Some(0)
  );
  assert_eq!(
    crate::ContractTailStorageValues::<ConstU32<10>, ConstU32<1>>::get(),
    Some(0)
  );
  assert_eq!(
    crate::ContractTailStorageValues::<ConstU32<10>, ConstU32<6>>::get(),
    Some(20)
  );
  assert_eq!(
    crate::ContractTailStorageValues::<ConstU32<{ u32::MAX }>, ConstU32<5>>::get(),
    Some(u32::MAX)
  );
  assert_eq!(
    crate::ContractTailStorageValues::<ConstU32<{ u32::MAX }>, ConstU32<6>>::get(),
    None,
    "overflow must not advertise a smaller saturated population bound"
  );

  new_test_ext().execute_with(|| {
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 1));
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(Actors::storage_info(), metadata);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
  });
}

#[test]
fn resource_storage_metadata_uses_compact_aware_parent_bounds() {
  use codec::MaxEncodedLen;
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;

  assert_eq!(
    crate::CurrentBlockResourceState::<Test>::storage_info()[0].max_size,
    Some(crate::BlockResourceState::<MockBlockNumber>::max_encoded_len() as u32),
  );
  assert_eq!(
    crate::FinalizedBlockResourceTelemetry::<Test>::storage_info()[0].max_size,
    Some(crate::FinalizedBlockResourceSnapshot::<MockBlockNumber>::max_encoded_len() as u32),
  );
}

#[test]
fn quote_and_host_metadata_bounds_cover_compact_weights() {
  use codec::MaxEncodedLen;

  let authority = crate::AdmissionCertificateAuthority {
    runtime_actor_semantics_version: 1,
    production_weight_identity: [0; 32],
    body_geometry_version: 1,
    configured_bounds_commitment: [0; 32],
    maximum_lifecycle_weight: Weight::MAX,
  };
  assert_eq!(
    authority.encoded_size(),
    crate::AdmissionCertificateAuthority::max_encoded_len()
  );
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let mut quote = Actors::actor_cost_quote(actor_id).unwrap();
    // Alter a detached codec value only; these are not host-admitted execution weights.
    quote
      .prospective_trigger_fee
      .as_mut()
      .unwrap()
      .maximum_weight = Weight::MAX;
    quote.maximum_next_action_fee.maximum_effect_weight = Weight::MAX;
    assert_eq!(
      quote.prospective_trigger_fee.unwrap().encoded_size(),
      crate::ActorTriggerFeeQuote::<Balance>::max_encoded_len(),
    );
    assert_eq!(
      quote.maximum_next_action_fee.encoded_size(),
      crate::ActorActionFeeQuote::<Balance>::max_encoded_len(),
    );
    assert_eq!(
      quote.encoded_size(),
      crate::ActorCostQuote::<Balance>::max_encoded_len()
    );
  });
}

#[test]
fn admission_identity_hash_input_has_bounded_encoding() {
  use codec::MaxEncodedLen;
  use polkadot_sdk::sp_io;

  let trigger = manual_schedule().trigger;
  let window = None::<crate::ScheduleWindow<MockBlockNumber>>;
  let qualification = trigger.wake_qualification(&window);
  let selector_input = (*b"DEOS_ACTOR_WAKE_SELECTOR", &trigger).encode();
  let schedule_input = (*b"DEOS_ACTOR_WAKE_SCHEDULE", &window).encode();
  assert_eq!(selector_input.len(), 24 + trigger.encoded_size());
  assert_eq!(schedule_input.len(), 24 + window.encoded_size());
  assert_eq!(
    qualification.selector_commitment,
    sp_io::hashing::blake2_256(&selector_input)
  );
  assert_eq!(
    qualification.schedule_commitment,
    sp_io::hashing::blake2_256(&schedule_input)
  );
  assert_eq!(qualification.encoded_size(), 65);

  for component in [
    0,
    63,
    64,
    (1 << 14) - 1,
    1 << 14,
    (1 << 30) - 1,
    1 << 30,
    u64::MAX,
  ] {
    let weight = Weight::from_parts(component, component);
    let certificate: crate::ActorAdmissionCertificateOf<Test> =
      crate::ActorAdmissionCertificate::new(
        [1; 32],
        [2; 32],
        qualification,
        3,
        [4; 32],
        5,
        [6; 32],
        weight,
      );
    let input = (
      *b"DEOS_ACTOR_ADMISSION",
      [1u8; 32],
      [2u8; 32],
      qualification,
      3u32,
      [4u8; 32],
      5u32,
      [6u8; 32],
      weight,
    )
      .encode();
    assert_eq!(input.len(), 221 + weight.encoded_size());
    let weight_encoding_bound = 2 * codec::Compact::<u64>::max_encoded_len();
    assert!(input.len() <= 221 + weight_encoding_bound);
    assert_eq!(
      certificate.admission_identity,
      sp_io::hashing::blake2_256(&input)
    );
    assert!(certificate.has_valid_identity());
    assert!(certificate.authorizes_wake(qualification));
    if component == u64::MAX {
      assert_eq!(input.len(), 221 + weight_encoding_bound);
      // The pinned derive ignores #[codec(compact)] when computing MaxEncodedLen.
      // This is a codec counterexample, not an admitted Actor or host authority fixture.
      assert_eq!(weight.encoded_size(), 18);
      assert_eq!(Weight::max_encoded_len(), 16);
      assert_eq!(
        certificate.encoded_size(),
        crate::ActorAdmissionCertificateOf::<Test>::max_encoded_len()
      );
      let resources = ActorStepResourceEnvelope {
        control: weight,
        effect: weight,
      };
      assert_eq!(
        resources.encoded_size(),
        ActorStepResourceEnvelope::max_encoded_len()
      );
    }
  }
}

#[test]
fn admission_identity_binds_every_runtime_owned_domain_field() {
  let qualification = manual_schedule()
    .trigger
    .wake_qualification(&None::<crate::ScheduleWindow<MockBlockNumber>>);
  let certificate: crate::ActorAdmissionCertificateOf<Test> = crate::ActorAdmissionCertificate::new(
    [1u8; 32],
    [2u8; 32],
    qualification,
    3,
    [4u8; 32],
    5,
    [6u8; 32],
    Weight::from_parts(77, 88),
  );
  assert!(certificate.has_valid_identity());
  assert!(certificate.authorizes_wake(qualification));
  let mut wrong_family = qualification;
  wrong_family.family = crate::TriggerFamily::AddressEvent;
  assert!(!certificate.authorizes_wake(wrong_family));
  let mut wrong_selector = qualification;
  wrong_selector.selector_commitment[0] ^= 1;
  assert!(!certificate.authorizes_wake(wrong_selector));
  let mut stale = certificate.clone();
  stale.wake_qualification = wrong_family;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.wake_qualification = wrong_selector;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.body_geometry_version = 6;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.production_weight_identity[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.maximum_lifecycle_weight = Weight::from_parts(78, 88);
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.maximum_lifecycle_weight = Weight::from_parts(77, 89);
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.semantic_contract_id[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.body_commitment[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.runtime_actor_semantics_version += 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.configured_bounds_commitment[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate;
  stale.wake_qualification.schedule_commitment[0] ^= 1;
  assert!(!stale.has_valid_identity());
}

#[test]
fn admission_certificate_encoding_is_independent_of_resource_ceiling() {
  type SmallResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<1>>;
  type LargeResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<32>>;
  let qualification = manual_schedule()
    .trigger
    .wake_qualification(&None::<crate::ScheduleWindow<MockBlockNumber>>);
  let small: crate::ActorAdmissionCertificate<SmallResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      qualification,
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  let large: crate::ActorAdmissionCertificate<LargeResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      qualification,
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  assert_eq!(small.encode(), large.encode());
  assert_eq!(small.admission_identity, large.admission_identity);
}

#[test]
fn admission_certificate_builder_composes_compact_host_authority() {
  new_test_ext().execute_with(|| {
    // This case documents the unprepared-host branch of the certificate composition, so the
    // production-faithful default has to be disabled explicitly.
    crate::mock::set_prepared_behavior_lifecycle_weight(false);
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle), make_step(Task::StopCycle)])
        .expect("two Steps fit"),
    )
    .expect("active Contract");
    let certificate =
      Actors::build_admission_certificate(&contract).expect("host authority exists");
    assert!(certificate.has_valid_identity());
    assert_eq!(
      certificate.semantic_contract_id,
      contract.semantic_contract_id()
    );
    assert_eq!(
      certificate.body_commitment,
      contract.body_commitment().expect("body commitment")
    );
    assert_eq!(certificate.runtime_actor_semantics_version, 1);
    assert_eq!(
      certificate.production_weight_identity,
      crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
    );
    assert_eq!(certificate.body_geometry_version, 1);
    assert_eq!(certificate.configured_bounds_commitment, [6; 32]);
    assert_eq!(
      certificate.maximum_lifecycle_weight,
      Weight::from_parts(77, 88)
    );
    assert_eq!(
      certificate.marker,
      std::marker::PhantomData::<crate::ActorAdmissionResourcesOf<Test>>
    );
  });
}

#[test]
fn production_contract_load_requires_its_certified_wake_qualification() {
  new_test_ext().execute_with(|| {
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("active Contract");
    let matching = Actors::build_admission_certificate(&contract).expect("host authority exists");
    assert!(Actors::admission_authorizes_contract_wake(
      &matching, &contract
    ));

    let temporal_contract = system_active_contract(
      at_time_schedule(10),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("temporal Contract");
    assert!(
      !Actors::admission_authorizes_contract_wake(&matching, &temporal_contract),
      "a valid Manual certificate cannot authorize a temporal production Contract load",
    );

    let windowed_contract = system_active_contract(
      manual_schedule(),
      Some(crate::ScheduleWindow { start: 2, end: 20 }),
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("windowed Contract");
    assert!(
      !Actors::admission_authorizes_contract_wake(&matching, &windowed_contract),
      "a valid unwindowed certificate cannot authorize different schedule boundaries",
    );

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |stored| {
      let crate::ActorSemanticState::Active(record) = stored
        .as_mut()
        .expect("canonical semantic authority exists")
      else {
        panic!("created Actor is active");
      };
      let old = record.admission.clone();
      record.admission = crate::ActorAdmissionCertificate::new(
        old.semantic_contract_id,
        old.body_commitment,
        temporal_contract
          .trigger
          .wake_qualification(&temporal_contract.window),
        old.runtime_actor_semantics_version,
        old.production_weight_identity,
        old.body_geometry_version,
        old.configured_bounds_commitment,
        old.maximum_lifecycle_weight,
      );
    });
    let crate::ActorSemanticState::Active(record) =
      crate::ActorSemanticStates::<Test>::get(actor_id).expect("mutated authority exists")
    else {
      panic!("mutated Actor is active");
    };
    let replacement_identity = record.admission.admission_identity;
    crate::ActorContractHeads::<Test>::mutate(actor_id, |stored| {
      stored
        .as_mut()
        .expect("Contract head exists")
        .header
        .admission_identity = replacement_identity;
    });
    assert!(matches!(
      Actors::load_actor_state_for_frame_control(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn step_resource_derivation_binds_current_predicate_and_amount_geometry() {
  let steps = BoundedVec::try_from(vec![StepOf::<Test> {
    precondition: all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1,
    }]),
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Percent(Perbill::from_percent(50)),
    },
    on_error: StepErrorPolicy::AbortCycle,
  }])
  .expect("one Step fits");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let resources = Actors::derive_step_resource_envelopes(&contract)
    .expect("configured resource providers admit Contract");
  assert_eq!(resources.len(), 1);
  assert_eq!(
    resources[0].control,
    Weight::from_parts(100_000_012, 100_023),
    "control base plus current predicate and amount evaluation units",
  );
}

#[test]
fn opening_control_envelope_charges_only_authored_tail_chunks() {
  let one_step = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
  )
  .expect("one-Step Contract exists");
  let eight_steps = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit"),
  )
  .expect("eight-Step Contract exists");

  let one =
    Actors::derive_step_resource_envelopes(&one_step).expect("one-Step resources exist")[0].control;
  let eight = Actors::derive_step_resource_envelopes(&eight_steps)
    .expect("eight-Step resources exist")[0]
    .control;

  assert_eq!(eight.ref_time().saturating_sub(one.ref_time()), 2);
  assert_eq!(eight.proof_size(), one.proof_size());
}

#[test]
fn unconfigured_resource_weight_ports_fail_closed() {
  assert_eq!(
    <() as crate::AdmissionCertificateAuthorityProvider>::current(),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::maximum_behavior_service_inspection_weight(
      Actors::behavior_service_inspection_bounds().unwrap(),
    ),
    None
  );
  let step = make_step(Task::StopCycle);
  let context = Actors::step_control_weight_context(1, 0, 0).unwrap();
  let behavior = crate::BehaviorControlWeightContext {
    policy_bytes: 55,
    monitor_bytes: 0,
    clauses: 1,
    atoms: 1,
    metrics: 1,
    counter_slots: 0,
    requires_clean_cycle: false,
  };
  assert_eq!(
    <() as crate::AdmissionCertificateAuthorityProvider>::behavior_close_additional_weight(
      behavior
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::maximum_behavior_zero_step_control_weight(behavior),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::actual_behavior_zero_step_control_weight(
      behavior,
      Weight::from_parts(1, 1),
      false,
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<_>>::maximum_behavior_control_weight(
      context, &step, behavior,
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<_>>::actual_behavior_control_weight(
      context,
      &step,
      behavior,
      Weight::from_parts(1, 1),
      crate::StepControlExecution {
        task_effect: crate::TaskEffectExecution::NotInvoked,
        action_fee_collected: false,
        phase: crate::StepControlPhase::Opening,
        outcome: crate::StepControlOutcome::Completed,
        placement: crate::StepControlPlacement::None,
      },
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<_>>::maximum_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
      },
      &step,
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::actual_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
      },
      &step,
      Weight::from_parts(1, 1),
      crate::StepControlExecution {
        task_effect: crate::TaskEffectExecution::NotInvoked,
        action_fee_collected: false,
        phase: crate::StepControlPhase::Opening,
        outcome: crate::StepControlOutcome::Completed,
        placement: crate::StepControlPlacement::None,
      },
    ),
    None
  );
  let task = Task::<TestAsset, Balance, AccountId>::StopCycle;
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<RuntimeTask>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::maximum_effect_weight(&task),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::actual_effect_weight(
      &task,
      crate::TaskEffectExecution::NotInvoked,
    ),
    None
  );
}

#[test]
fn step_control_weight_context_matches_c6_head_and_tail_geometry() {
  assert_eq!(
    Actors::step_control_weight_context(1, 0, 7),
    Some(crate::StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 0,
      predicate_evaluation_units: 7,
    })
  );
  assert_eq!(
    Actors::step_control_weight_context(12, 0, 7)
      .expect("twelve-Step head context exists")
      .opening_tail_chunks,
    3,
  );
  for (cursor, steps_in_fragment) in [(1, 4), (4, 4), (5, 4), (8, 4), (9, 3), (11, 3)] {
    assert_eq!(
      Actors::step_control_weight_context(12, cursor, 7),
      Some(crate::StepControlWeightContext {
        cursor,
        steps_in_fragment,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 7,
      })
    );
  }
  assert_eq!(Actors::step_control_weight_context(0, 0, 0), None);
  assert_eq!(Actors::step_control_weight_context(12, 12, 0), None);
  assert_eq!(Actors::step_control_weight_context(13, 0, 0), None);
}

#[test]
fn current_step_resource_admission_requires_both_weight_components() {
  let resources = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let control = WeightMeter::with_limit(resources.control);
  let effect = WeightMeter::with_limit(resources.effect);
  assert!(Actors::current_step_resources_fit(
    &control, &effect, resources
  ));
  let short_control = WeightMeter::with_limit(Weight::from_parts(10, 22));
  assert!(!Actors::current_step_resources_fit(
    &short_control,
    &effect,
    resources,
  ));
  let short_effect_proof = WeightMeter::with_limit(Weight::from_parts(33, 43));
  assert!(!Actors::current_step_resources_fit(
    &control,
    &short_effect_proof,
    resources,
  ));
}

#[test]
fn step_resource_envelope_keeps_control_and_effect_weight_separate() {
  let envelope = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  assert_eq!(envelope.control, Weight::from_parts(11, 22));
  assert_eq!(envelope.effect, Weight::from_parts(33, 44));
  assert_eq!(
    crate::ActorStepResourceEnvelope::decode(&mut envelope.encode().as_slice()),
    Ok(envelope)
  );
}

#[test]
fn user_pipeline_machine_envelope_prices_control_retries_without_destruction() {
  new_test_ext().execute_with(|| {
    let mut steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two Steps fit");
    steps[1].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let first_fee = Actors::maximum_current_step_fee(ActorType::User, resources[0])
      .expect("first Action fee fits");
    let second_fee = Actors::maximum_current_step_fee(ActorType::User, resources[1])
      .expect("second Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(
      ActorType::User,
      crate::OnUnfunded::Close,
      &steps,
      &resources,
      TestWeightInfo::scheduler_inner_zero_step_complete(),
      Weight::zero(),
    )
    .expect("Pipeline Machine envelope fits");

    let service = Actors::service_turn_control_weight_upper().unwrap();
    let first = resources[0].control.checked_add(&service).unwrap();
    let retry = resources[1].control.checked_add(&service).unwrap();
    let aggregate = (0..3).fold(first, |total, _| total.checked_add(&retry).unwrap());
    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&aggregate),
      "Convert one complete declared Control Weight, not each visit separately"
    );
    assert_eq!(first_fee.control_fee, 0);
    assert_eq!(second_fee.control_fee, 0);
    assert!(first_fee.effect_fee > 0);
    assert!(second_fee.effect_fee > 0);
    let quote = Actors::pipeline_fee_breakdown(ActorType::User, envelope);
    assert_eq!(quote.total_fee, envelope.pipeline_machine_fee_upper);
    assert_eq!(
      Actors::close_cleanup_weight_upper(),
      <TestWeightInfo as crate::WeightInfo>::close_actor()
    );
  });
}

#[test]
fn pipeline_repeat_cost_checked_work_is_not_maximized_by_retry_ceiling() {
  use polkadot_sdk::sp_runtime::traits::{CheckedAdd, CheckedMul, Zero};
  use std::cell::Cell;
  std::thread_local! {
    static CHECKED_WORK: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
  }
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  struct Counted(u64);
  impl std::ops::Add for Counted {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
      Self(self.0 + rhs.0)
    }
  }
  impl std::ops::Mul for Counted {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
      Self(self.0 * rhs.0)
    }
  }
  impl Zero for Counted {
    fn zero() -> Self {
      Self(0)
    }
    fn is_zero(&self) -> bool {
      self.0 == 0
    }
  }
  impl From<u32> for Counted {
    fn from(value: u32) -> Self {
      Self(u64::from(value))
    }
  }
  impl CheckedAdd for Counted {
    fn checked_add(&self, rhs: &Self) -> Option<Self> {
      CHECKED_WORK.with(|work| {
        let (a, m) = work.get();
        work.set((a + 1, m));
      });
      self.0.checked_add(rhs.0).map(Self)
    }
  }
  impl CheckedMul for Counted {
    fn checked_mul(&self, rhs: &Self) -> Option<Self> {
      CHECKED_WORK.with(|work| {
        let (a, m) = work.get();
        work.set((a, m + 1));
      });
      self.0.checked_mul(rhs.0).map(Self)
    }
  }
  // Instrument the real helper through its existing generic contract, never a production hook.
  // Counts describe this diagnostic instantiation, not optimized u64/u128 timing or sort work.
  for cost in [0, 7] {
    for (retry_ceiling, expected_choices) in [(10, 1), (2, 9)] {
      CHECKED_WORK.with(|work| work.set((0, 0)));
      let result =
        Actors::maximum_pipeline_repeat_cost(vec![(Counted(cost), retry_ceiling - 1); 12], 9)
          .unwrap();
      assert_eq!(result, Counted(cost * 9));
      assert_eq!(
        CHECKED_WORK.with(Cell::get),
        (expected_choices, expected_choices)
      );
    }
  }
}

#[test]
fn pipeline_repeat_cost_matches_independent_bounded_allocations() {
  fn exhaustive(choices: &[(u64, u32)], remaining: u32) -> u128 {
    let Some(((cost, capacity), tail)) = choices.split_first() else {
      return 0;
    };
    (0..=(*capacity).min(remaining))
      .map(|count| u128::from(*cost) * u128::from(count) + exhaustive(tail, remaining - count))
      .max()
      .unwrap()
  }
  let cases = [
    vec![],
    vec![(0u64, 3)],
    vec![(7, 2), (7, 3)],
    vec![(2, 3), (11, 1), (5, 2)],
    vec![(u64::MAX, 0), (3, 2)],
    vec![(u64::MAX, 1), (1, 1)],
    vec![(u64::MAX - 5, 1), (5, 1)],
    vec![(u64::MAX, 2)],
  ];
  for choices in cases {
    for budget in 0..=6 {
      let expected = exhaustive(&choices, budget);
      let mut reversed = choices.clone();
      reversed.reverse();
      for ordering in [choices.clone(), reversed] {
        assert_eq!(
          Actors::maximum_pipeline_repeat_cost(ordering.clone(), budget),
          u64::try_from(expected).ok()
        );
        assert_eq!(
          Actors::maximum_pipeline_repeat_cost(
            ordering
              .into_iter()
              .map(|(cost, count)| (u128::from(cost), count))
              .collect(),
            budget,
          ),
          Some(expected)
        );
      }
    }
  }
  assert_eq!(
    Actors::maximum_pipeline_repeat_cost(vec![(u128::MAX, 1)], 1),
    Some(u128::MAX)
  );
  assert_eq!(
    Actors::maximum_pipeline_repeat_cost(vec![(u128::MAX, 2)], 2),
    None
  );
}

#[test]
fn pipeline_machine_envelope_checks_both_cumulative_weight_dimensions() {
  new_test_ext().execute_with(|| {
    let steps = crate::ContractSteps::<Test>::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      });
      2
    ])
    .unwrap();
    let service = Actors::service_turn_control_weight_upper().unwrap();
    let maximum = Weight::from_parts(u64::MAX, u64::MAX);
    let twice_service = service.checked_add(&service).unwrap();
    // Pure arithmetic inputs, not qualified physical prices or admitted Contract resources.
    for (control, fits) in [
      ([Weight::from_parts(u64::MAX / 2 + 1, 0); 2], false),
      ([Weight::from_parts(0, u64::MAX / 2 + 1); 2], false),
      (
        [maximum.checked_sub(&twice_service).unwrap(), Weight::zero()],
        true,
      ),
    ] {
      let visits = control.map(|weight| weight.checked_add(&service).unwrap());
      let aggregate = visits[0].checked_add(&visits[1]);
      assert_eq!(aggregate.is_some(), fits);
      let resources = crate::ActorAdmissionResourcesOf::<Test>::try_from(
        control
          .into_iter()
          .map(|control| crate::ActorStepResourceEnvelope {
            control,
            effect: Weight::zero(),
          })
          .collect::<Vec<_>>(),
      )
      .unwrap();
      let result = Actors::derive_pipeline_machine_envelope(
        ActorType::User,
        crate::OnUnfunded::Close,
        &steps,
        &resources,
        Weight::zero(),
        Weight::zero(),
      );
      if let Some(weight) = aggregate {
        assert_eq!(weight, maximum);
        assert_eq!(
          result.unwrap().pipeline_machine_fee_upper,
          TestWeightToFee::weight_to_fee(&weight)
        );
      } else {
        assert!(matches!(result, Err(Error::<Test>::AdmissionBoundOverflow)));
        let mut retry_step = steps[0].clone();
        retry_step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
        let retry_steps = crate::ContractSteps::<Test>::try_from(vec![retry_step]).unwrap();
        let retry_resources =
          crate::ActorAdmissionResourcesOf::<Test>::try_from(vec![resources[0]]).unwrap();
        assert!(matches!(
          Actors::derive_pipeline_machine_envelope(
            ActorType::User,
            crate::OnUnfunded::Close,
            &retry_steps,
            &retry_resources,
            Weight::zero(),
            Weight::zero(),
          ),
          Err(Error::<Test>::AdmissionBoundOverflow)
        ));
      }
    }
  });
}

#[test]
fn pipeline_machine_envelope_prices_only_reachable_stop_cycle_visits() {
  for (stop_at_head, conditional, close_fee, waiting_fee) in [
    (true, false, 100, 100),
    (true, true, 100, 1_600),
    (false, false, 100, 1_600),
    (false, true, 100, 2_100),
  ] {
    new_test_ext().execute_with(|| {
      set_max_consecutive_failures(3);
      set_prepared_financial_wait_weights(true);
      set_prepared_financial_wait_due_weight(true);
      set_prepared_financial_wait_service_weight(true);
      let mut action = make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      });
      action.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
      let mut stop = make_step(Task::StopCycle);
      stop.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
      if conditional {
        stop.precondition = all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 1_000,
        }]);
      }
      let steps = crate::ContractSteps::<Test>::try_from(if stop_at_head {
        vec![stop, action]
      } else {
        vec![action.clone(), stop, action]
      })
      .unwrap();
      let contract = system_active_contract(manual_schedule(), None, steps.clone()).unwrap();
      let resources = Actors::derive_step_resource_envelopes(&contract).unwrap();
      assert_eq!(
        resources.len(),
        steps.len(),
        "Retain full authored resource geometry"
      );
      for (policy, expected) in [
        (crate::OnUnfunded::Close, close_fee),
        (crate::OnUnfunded::AwaitFunding, waiting_fee),
      ] {
        let envelope = Actors::derive_pipeline_machine_envelope(
          ActorType::User,
          policy,
          &steps,
          &resources,
          Weight::zero(),
          Weight::zero(),
        )
        .unwrap();
        assert_eq!(
          envelope.pipeline_machine_fee_upper, expected,
          "StopCycle has one visit; only a conditional return permits suffix attempts"
        );
      }
      let system = Actors::derive_pipeline_machine_envelope(
        ActorType::System,
        crate::OnUnfunded::Close,
        &steps,
        &resources,
        Weight::zero(),
        Weight::zero(),
      )
      .unwrap();
      assert_eq!(system.pipeline_machine_fee_upper, 0);
    });
  }
}

#[test]
fn zero_step_pipeline_machine_envelope_prices_generated_control_without_destruction() {
  new_test_ext().execute_with(|| {
    let steps = crate::ContractSteps::<Test>::default();
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("zero-Step Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("empty resources derive");
    let user = Actors::derive_pipeline_machine_envelope(
      ActorType::User,
      crate::OnUnfunded::Close,
      &steps,
      &resources,
      TestWeightInfo::scheduler_inner_zero_step_complete(),
      Weight::zero(),
    )
    .expect("zero-Step User Pipeline Machine envelope fits");
    assert_eq!(
      user.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(
        &<TestWeightInfo as crate::WeightInfo>::scheduler_inner_zero_step_complete()
          .checked_add(&Actors::service_turn_control_weight_upper().unwrap())
          .unwrap(),
      )
    );
    assert!(user.pipeline_machine_fee_upper > 0);
    let quote = Actors::pipeline_fee_breakdown(ActorType::User, user);
    assert_eq!(quote.total_fee, user.pipeline_machine_fee_upper);
    let system = Actors::derive_pipeline_machine_envelope(
      ActorType::System,
      crate::OnUnfunded::Close,
      &steps,
      &resources,
      TestWeightInfo::scheduler_inner_zero_step_complete(),
      Weight::zero(),
    )
    .expect("zero-Step System envelope fits");
    assert_eq!(system.pipeline_machine_fee_upper, 0);
    assert_eq!(
      Actors::pipeline_fee_breakdown(ActorType::System, user).total_fee,
      0
    );
  });
}

#[test]
fn pipeline_wait_backing_counts_initial_inspections_and_economic_attempts() {
  new_test_ext().execute_with(|| {
    set_prepared_financial_wait_weights(true);
    set_prepared_financial_wait_due_weight(true);
    set_prepared_financial_wait_service_weight(true);
    let mut steps = crate::ContractSteps::<Test>::try_from(vec![
      make_step(Task::Transfer { to: BOB, asset: TestAsset::Native, amount: AmountResolution::Fixed(1) }),
      make_step(Task::Transfer { to: CHARLIE, asset: TestAsset::Native, amount: AmountResolution::Fixed(1) }),
      make_step(Task::StopCycle),
    ]).unwrap();
    steps[1].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract = system_active_contract(manual_schedule(), None, steps.clone()).unwrap();
    let resources = Actors::derive_step_resource_envelopes(&contract).unwrap();
    let base = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::Close, &steps, &resources, Weight::zero(), Weight::zero()).unwrap();
    let waiting = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::AwaitFunding, &steps, &resources, Weight::zero(), Weight::zero()).unwrap();
    let bytes = u32::try_from(crate::FinancialWaitEpisode::<Balance>::max_encoded_len()).unwrap();
    let entry = <MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::financial_wait_entry_weight(bytes).unwrap();
    let due = <MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::financial_wait_due_weight(bytes).unwrap();
    let recovery = <MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::financial_wait_service_weight(bytes).unwrap();
    let phase_fee = TestWeightToFee::weight_to_fee(&entry) + TestWeightToFee::weight_to_fee(&due) + TestWeightToFee::weight_to_fee(&recovery);
    let inspection_fee = TestWeightToFee::weight_to_fee(&recovery);
    // Readiness backs the first Opening inspection; Pipeline backs the other four inspections,
    // including StopCycle. Only four economic Attempts can enter an Action wait.
    let extra = TestWeightToFee::weight_to_fee(&resources[0].control) + TestWeightToFee::weight_to_fee(&resources[1].control) * 3 + phase_fee * 4 + inspection_fee * 4;
    assert_eq!(waiting.pipeline_machine_fee_upper, base.pipeline_machine_fee_upper + extra);
    assert_eq!(Actors::maximum_current_action_fee(ActorType::User, &steps[0], resources[0]).unwrap().total_fee, 100);
    assert_eq!(Actors::maximum_current_action_fee(ActorType::User, &steps[2], resources[2]).unwrap().total_fee, 0);
    for missing in 0..3 {
      set_prepared_financial_wait_weights(missing != 0);
      set_prepared_financial_wait_due_weight(missing != 1);
      set_prepared_financial_wait_service_weight(missing != 2);
      assert!(matches!(Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::AwaitFunding, &steps, &resources, Weight::zero(), Weight::zero()), Err(Error::<Test>::AdmissionBoundOverflow)));
      assert_eq!(Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::Close, &steps, &resources, Weight::zero(), Weight::zero()).unwrap(), base);
    }
    set_prepared_financial_wait_weights(true);
    set_prepared_financial_wait_due_weight(true);
    set_prepared_financial_wait_service_weight(true);
    let stop = crate::ContractSteps::<Test>::try_from(vec![make_step(Task::StopCycle)]).unwrap();
    let contract = system_active_contract(manual_schedule(), None, stop.clone()).unwrap();
    let resources = Actors::derive_step_resource_envelopes(&contract).unwrap();
    let stop_wait = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::AwaitFunding, &stop, &resources, Weight::zero(), Weight::zero()).unwrap();
    let stop_close = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::Close, &stop, &resources, Weight::zero(), Weight::zero()).unwrap();
    assert_eq!(stop_wait.pipeline_machine_fee_upper, stop_close.pipeline_machine_fee_upper);
    assert_eq!(Actors::maximum_current_action_fee(ActorType::User, &stop[0], resources[0]).unwrap().total_fee, 0);
    let mut head_retry = crate::ContractSteps::<Test>::try_from(vec![steps[0].clone()]).unwrap();
    head_retry[0].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract = system_active_contract(manual_schedule(), None, head_retry.clone()).unwrap();
    let resources = Actors::derive_step_resource_envelopes(&contract).unwrap();
    let retry_base = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::Close, &head_retry, &resources, Weight::zero(), Weight::zero()).unwrap();
    let retry_wait = Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::AwaitFunding, &head_retry, &resources, Weight::zero(), Weight::zero()).unwrap();
    let machine = TestWeightToFee::weight_to_fee(&resources[0].control.checked_add(&Actors::service_turn_control_weight_upper().unwrap()).unwrap());
    assert_eq!(retry_wait.pipeline_machine_fee_upper, retry_base.pipeline_machine_fee_upper + (machine + phase_fee) * 3 + inspection_fee * 2, "Only the first head inspection belongs to readiness; both retries remain Pipeline-backed");
    let empty = crate::ContractSteps::<Test>::default();
    let resources = crate::ActorAdmissionResourcesOf::<Test>::default();
    assert_eq!(Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::AwaitFunding, &empty, &resources, Weight::from_parts(1, 1), Weight::zero()).unwrap(), Actors::derive_pipeline_machine_envelope(ActorType::User, crate::OnUnfunded::Close, &empty, &resources, Weight::from_parts(1, 1), Weight::zero()).unwrap());
  });
}

#[test]
fn pipeline_machine_envelope_absorbs_stop_cycle_control_effect() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits");
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let action_fee = Actors::maximum_current_action_fee(ActorType::User, &steps[0], resources[0])
      .expect("StopCycle Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(
      ActorType::User,
      crate::OnUnfunded::Close,
      &steps,
      &resources,
      TestWeightInfo::scheduler_inner_zero_step_complete(),
      Weight::zero(),
    )
    .expect("Pipeline Machine envelope fits");

    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&resources[0].control.saturating_add(resources[0].effect))
    );
    assert_eq!(action_fee.total_fee, 0);
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_pipeline_machine_envelope_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("C6 head exists")
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper = 1;
    });

    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_actor_state_hold_geometry_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ActorStateHolds::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("User Actor hold exists")
        .breakdown
        .contract_head = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn step_chunk_binds_authority_and_contiguous_first_index() {
  let chunk = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
      admission_identity: [3u8; 32],
    },
    first_step_index: 1,
    steps: vec![11u8, 12u8, 13u8, 14u8],
    step_resources: vec![
      crate::ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: Weight::zero(),
      };
      4
    ],
  };
  assert!(chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&8, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 2));
  assert_eq!(chunk.steps.len(), 4);
}

#[test]
fn contract_geometry_decomposition_is_gap_free_and_head_only_for_one_step() {
  new_test_ext().execute_with(|| {
    for step_count in [1u32, 8] {
      let steps = BoundedVec::try_from(
        (0..step_count)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("bounded Steps fit");
      let contract =
        system_active_contract(manual_schedule(), None, steps).expect("active Contract");
      let certificate = crate::ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment().expect("body commitment"),
        contract.trigger.wake_qualification(&contract.window),
        1,
        [4u8; 32],
        1,
        [6u8; 32],
        Weight::from_parts(77, 88),
      );
      let (head, chunks) =
        Actors::decompose_admitted_contract_geometry(7, ActorType::System, &contract, &certificate)
          .expect("admitted Contract geometry decomposes");
      assert_eq!(
        head.header.admission_identity,
        certificate.admission_identity
      );
      assert_eq!(
        head.header.pipeline_machine_envelope,
        crate::PipelineMachineEnvelope {
          pipeline_machine_fee_upper: 0,
        }
      );
      let loaded_first = Actors::load_current_step_from_geometry(7, &head, &certificate, 0, None)
        .expect("Step 0 loads from the head only");
      assert_eq!(loaded_first.step, contract.steps[0]);
      if step_count > 1 {
        let cursor = step_count - 1;
        let chunk_index = cursor.saturating_sub(1) / 4;
        let chunk = chunks
          .iter(/* deos-bypass: bounded-iter */)
          .find(|(index, _)| *index == chunk_index)
          .expect("current tail chunk exists");
        let loaded = Actors::load_current_step_from_geometry(
          7,
          &head,
          &certificate,
          cursor,
          Some((chunk.0, &chunk.1)),
        )
        .expect("tail Step loads from exactly one chunk");
        assert_eq!(loaded.step, contract.steps[cursor as usize]);
        assert!(
          Actors::load_current_step_from_geometry(
            8,
            &head,
            &certificate,
            cursor,
            Some((chunk.0, &chunk.1)),
          )
          .is_none()
        );
      }
      let mut stale_certificate = certificate.clone();
      stale_certificate.body_geometry_version += 1;
      assert!(
        Actors::decompose_admitted_contract_geometry(
          7,
          ActorType::User,
          &contract,
          &stale_certificate,
        )
        .is_none()
      );
      assert_eq!(head.first_step, Some(contract.steps[0].clone()));
      assert_eq!(
        Actors::reconstruct_contract_geometry(7, head.clone(), &chunks),
        Some(contract.clone())
      );
      let maximum: u32 = <Test as crate::Config>::MaxContractSteps::get();
      for count in [maximum + 1, u32::MAX] {
        let mut oversized = head.clone();
        oversized.header.step_count = count;
        assert!(Actors::reconstruct_contract_geometry(7, oversized, &chunks).is_none());
      }
      if !chunks.is_empty() {
        assert!(
          Actors::reconstruct_contract_geometry(7, head.clone(), &chunks[..chunks.len() - 1])
            .is_none()
        );
        let mut extra = chunks.clone();
        extra.push(chunks[0].clone());
        assert!(Actors::reconstruct_contract_geometry(7, head.clone(), &extra).is_none());
        assert_eq!(
          Actors::reconstruct_contract_geometry(8, head.clone(), &chunks),
          None
        );
        let mut stale_range = chunks.clone();
        stale_range[0].1.first_step_index = 2;
        assert_eq!(
          Actors::reconstruct_contract_geometry(7, head.clone(), &stale_range),
          None
        );
      }
      assert_eq!(
        chunks.len(),
        usize::try_from(step_count.saturating_sub(1).div_ceil(4)).unwrap()
      );
      let reconstructed = head
        .first_step
        .into_iter()
        .chain(
          chunks
            .iter()
            .flat_map(|(_, chunk)| chunk.steps.iter().cloned()),
        )
        .collect::<Vec<_>>();
      assert_eq!(reconstructed.as_slice(), contract.steps.as_slice());
      for (chunk_index, chunk) in chunks {
        assert_eq!(chunk.first_step_index, 1 + chunk_index * 4);
        assert!(!chunk.steps.is_empty());
        assert!(chunk.steps.len() <= 4);
        assert!(chunk.matches(
          &7,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          chunk.first_step_index,
        ));
      }
    }
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn contract_replacement_updates_frame_admission_and_current_step_resources_in_place() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let crate::ActorSemanticState::Active(before_record) =
      crate::ActorSemanticStates::<Test>::get(actor_id).expect("initial semantic authority exists")
    else {
      panic!("created Actor is active");
    };
    let replacement =
      system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 7))
        .expect("replacement Contract");

    assert_ok!(Actors::store_actor_contract(actor_id, replacement));

    let crate::ActorSemanticState::Active(after_record) =
      crate::ActorSemanticStates::<Test>::get(actor_id).expect("replacement semantic authority")
    else {
      panic!("replaced Actor is active");
    };
    let head =
      ActorContractHeads::<Test>::get(actor_id).expect("replacement canonical head exists");
    let loaded_step =
      Actors::load_current_step_from_storage(actor_id, 0).expect("replacement current Step loads");
    assert_ne!(
      after_record.admission.admission_identity,
      before_record.admission.admission_identity
    );
    assert_eq!(
      after_record.admission.admission_identity,
      head.header.admission_identity
    );
    assert_eq!(
      head
        .first_step_resources
        .expect("replacement Step resources"),
      loaded_step.resources
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn dormant_activation_and_trigger_replacements_preserve_strict_frame_loading() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("Manual Contract"),
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      timer_schedule(2),
      None,
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(5);
    let address_schedule = Schedule {
      trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
      cooldown_blocks: 0,
    };
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      address_schedule,
      None,
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
  });
}

#[test]
fn admitted_contract_storage_loads_one_current_fragment_and_replaces_exact_tail() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(
        (0..8)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("eight Steps fit"),
    )
    .expect("active Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract.steps.clone());
    let crate::ActorSemanticState::Active(record) =
      crate::ActorSemanticStates::<Test>::get(actor_id).expect("live semantic authority exists")
    else {
      panic!("created Actor is active");
    };
    let certificate = record.admission;
    assert_eq!(
      Actors::remove_admitted_contract_geometry(actor_id),
      Some(contract.clone())
    );
    assert!(Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert_eq!(
      Actors::load_admitted_contract_geometry(actor_id),
      Some((contract.clone(), certificate.clone()))
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 0)
        .expect("head Step loads")
        .step,
      contract.steps[0]
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 7)
        .expect("one tail chunk loads current Step")
        .step,
      contract.steps[7]
    );
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 1));

    let replacement = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("replacement Contract");
    // The canonical storage owner rotates geometry, admission and the generation-bound carrier
    // together instead of mirroring a legacy primary cell.
    assert_ok!(Actors::store_actor_contract(actor_id, replacement.clone()));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 1));
    assert!(Actors::load_current_step_from_storage(actor_id, 1).is_none());
    let process_before = ActorProcesses::<Test>::get(actor_id);
    assert_eq!(
      Actors::remove_admitted_contract_geometry(actor_id),
      Some(replacement)
    );
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    // Removing the geometry leaves the canonical process/residence carrier untouched.
    assert_eq!(ActorProcesses::<Test>::get(actor_id), process_before);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(Actors::remove_admitted_contract_geometry(actor_id).is_none());
  });
}

#[test]
fn admitted_contract_storage_fails_closed_before_mutation_on_stale_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = 78;
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("active Contract");
    let mut stale_certificate = geometry_certificate(&contract);
    stale_certificate.body_geometry_version += 1;
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &stale_certificate,
    ));
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!Actors::actor_control_cell(actor_id).is_some());
  });
}

#[test]
fn inline_first_head_and_lazy_tail_have_unique_ordered_ownership() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let semantic_contract_id = contract.semantic_contract_id();
  let body_commitment = contract.body_commitment().expect("bounded body commitment");
  let admission_identity = [3u8; 32];
  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded header");
  let resource = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let head = crate::ActorContractHead {
    header,
    first_step: Some(contract.steps[0].clone()),
    first_step_resources: Some(resource),
  };
  let tail = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    },
    first_step_index: 1,
    steps: BoundedVec::<_, ConstU32<4>>::try_from(vec![contract.steps[1].clone()])
      .expect("one tail Step fits"),
    step_resources: BoundedVec::<_, ConstU32<4>>::try_from(vec![resource])
      .expect("one tail resource fits"),
  };

  assert_eq!(head.header.step_count, 2);
  assert_eq!(head.first_step, Some(contract.steps[0].clone()));
  assert_eq!(tail.steps[0], contract.steps[1]);
  assert!(tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &8,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    0,
  ));
  let reconstructed = BoundedVec::<_, <Test as crate::Config>::MaxContractSteps>::try_from(vec![
    head.first_step.expect("nonempty Contract has inline Step"),
    tail.steps[0].clone(),
  ])
  .expect("reconstructed Steps fit");
  assert_eq!(reconstructed, contract.steps);
}

#[test]
#[ignore = "retained-state Contract geometry profile"]
fn profile_contract_geometry_state_footprint() {
  for step_count in [1u32, 8] {
    let steps = BoundedVec::try_from(
      (0..step_count)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("profile Steps fit");
    let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
    let semantic_contract_id = contract.semantic_contract_id();
    let body_commitment = contract.body_commitment().expect("bounded body commitment");
    let admission_identity = [3u8; 32];
    let header = contract
      .try_header(
        semantic_contract_id,
        body_commitment,
        admission_identity,
        test_pipeline_machine_envelope(),
      )
      .expect("bounded header");
    let authority = crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    };
    let resource = crate::ActorStepResourceEnvelope {
      control: Weight::zero(),
      effect: Weight::zero(),
    };
    let head = crate::ActorContractHead {
      header,
      first_step: Some(contract.steps[0].clone()),
      first_step_resources: Some(resource),
    };
    let chunks = contract.steps.as_slice()[1..]
      .chunks(4)
      .enumerate()
      .map(|(chunk_index, steps)| crate::ActorStepChunk {
        authority: authority.clone(),
        first_step_index: 1u32.saturating_add(
          u32::try_from(chunk_index)
            .expect("bounded chunk index fits u32")
            .saturating_mul(4),
        ),
        steps: steps.to_vec(),
        step_resources: vec![resource; steps.len()],
      })
      .collect::<Vec<_>>();
    let b0_bytes = contract.encode().len();
    let head_bytes = head.encode().len();
    let chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .sum::<usize>();
    let max_chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .max()
      .unwrap_or_default();
    println!(
      "CONTRACT_GEOMETRY_FOOTPRINT step_count={step_count} monolithic_bytes={b0_bytes} chunked_total_bytes={} head_bytes={head_bytes} max_chunk_bytes={max_chunk_bytes}",
      head_bytes.saturating_add(chunk_bytes),
    );
  }
}

#[test]
fn removed_amount_resolution_scale_forms_are_rejected() {
  for discriminant in [2_u8, 3, 4] {
    assert!(AmountResolution::<u128>::decode(&mut &[discriminant][..]).is_err());
  }
}

#[test]
fn public_reachability_inventory_is_closed_and_canonical() {
  assert_variant_names::<RuntimeTask>(&[
    "Transfer",
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
    "Percent",
    "CaptureCycleBalance",
    "CycleBalancePercent",
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
  assert_variant_names::<crate::PredicateError>(&["InvalidObservation"]);
  assert_variant_names::<RuntimeSourceFilter>(&["Any", "OwnerOnly", "Whitelist"]);
  assert_variant_names::<RuntimeAssetFilter>(&["Any", "Whitelist"]);
  assert_variant_names::<RuntimeTrigger>(&["Manual", "AddressEvent", "AtTime", "Cadenced"]);
  assert_variant_names::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &["Manual", "AddressEvent", "AtTime", "Cadenced"],
  );
  assert_variant_names::<ActorType>(&["User", "System"]);
  assert_variant_names::<ActorClass>(&["User", "System"]);
  assert_variant_names::<Mutability>(&["Mutable", "Immutable"]);
  assert_variant_names::<Option<crate::CloseWhenOf<Test>>>(&["None", "Some"]);
  assert_variant_names::<ActiveLifecycle>(&["Active", "Paused"]);
  assert_variant_names::<CycleState>(&["Idle", "Running", "Suspended"]);
  assert_variant_names::<AttemptDisposition>(&[
    "Completed",
    "Continued",
    "Failed",
    "Suspended",
    "Closed",
  ]);
  assert_variant_names::<StepOutcome>(&[
    "Executed",
    "Stopped",
    "Skipped",
    "FundingUnavailable",
    "Failed",
  ]);
  assert_variant_names::<CloseReason>(&[
    "OwnerInitiated",
    "CycleAdmissionInsufficient",
    "TriggerAdmissionInsufficient",
    "ConsecutiveFailures",
    "WindowExpired",
    "CycleNonceExhausted",
    "AutoCloseNonceReached",
    "RetryAttemptsExhausted",
    "BehaviorPolicySatisfied",
    "FundingGraceExpired",
    "SchedulerIndexExhausted",
  ]);
  assert_variant_names::<StepErrorPolicy>(&["AbortCycle", "ContinueNextStep", "RetryLater"]);
  assert_variant_names::<SuspensionReason>(&["FundingUnavailable", "Temporary"]);
  assert_variant_names::<crate::FundingUnavailableReason>(&["ServiceFee", "TaskInput"]);
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
  assert_variant_names::<ActorEligibility<u64, Balance>>(&["NotRegistered", "Dormant", "Active"]);
  assert_variant_names::<SimulationMode>(&["FreshCurrentPlan", "CurrentRun"]);
  assert_variant_names::<SimulationError>(&[
    "TransactionDepthExceeded",
    "Classification",
    "ActorNotFound",
    "TypeMismatch",
    "MutabilityMismatch",
    "InvalidContract",
    "InvalidBudget",
    "ContractMismatch",
    "ModeCycleStateMismatch",
    "GlobalCircuitBreaker",
    "Paused",
    "NotReady",
    "ResourceDeferred",
    "FeeCollectionFailed",
  ]);
}

#[test]
fn actor_storage_schema_is_explicit() {
  assert_eq!(crate::SuspensionReason::max_encoded_len(), 1);
  assert_eq!(
    crate::StepOutcome::max_encoded_len(),
    1 + crate::TaskFailure::max_encoded_len(),
    "funding provenance must not increase the dominant Run outcome bound"
  );
  let storage_info = Actors::storage_info();
  assert!(
    storage_info
      .iter()
      .all(|entry| entry.pallet_name == b"Actors")
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
      ("ActorContractHead", true, true),
      ("ActorBehaviorPolicies", true, true),
      ("ActorBehaviorMonitors", true, true),
      ("ActorFinancialWaits", true, true),
      ("ActorContractTailChunk", true, true),
      ("ActorRunState", true, true),
      ("ActorIdentities", true, true),
      ("ActorSemanticStates", true, true),
      ("ActorProcesses", true, true),
      ("ServiceHeader", false, false),
      ("ServiceNodes", true, true),
      ("DependencySourceAllocatorState", false, false),
      ("BalanceDependencySources", true, true),
      ("DependencySourceBalances", true, true),
      ("DependencyRevisions", false, true),
      ("DependencyScanSourceListState", false, false),
      ("DependencyScanSourceNodes", true, true),
      ("PendingCheckOwners", true, true),
      ("DependencyRegistrationHeaders", false, true),
      ("DependencyRegistrationPages", true, true),
      ("DependencyRegistrationFreePositions", true, true),
      ("DependencyRegistrationFreeIndices", true, true),
      ("DependencyRegistrationPositions", true, true),
      ("DependencyRegistrations", true, true),
      ("DependencyPlans", false, true),
      ("ParkedBalanceEpisodes", true, true),
      ("DependencyTimedReviews", true, true),
      ("PendingDependencyEvents", true, true),
      ("PendingDependencyReviews", true, true),
      ("DeadlineHeaders", true, true),
      ("DeadlinePages", true, true),
      ("DeadlineHandles", true, true),
      ("TriggerDeadlineHandles", true, true),
      ("DeadlineIndexPages", true, true),
      ("DeadlineIndexPositions", true, true),
      ("DeadlineIndexLen", false, true),
      ("ActorIdentityCount", false, false),
      ("ActorStateHolds", true, true),
      ("ActiveActorCount", false, false),
      ("SystemSovereigns", true, true),
      ("SystemSovereignCount", false, false),
      ("PrepassExecutionCutoff", true, false),
      ("CurrentBlockResourceState", true, false),
      ("FinalizedBlockResourceTelemetry", true, false),
      ("ActorUnsignaledControlCells", true, true),
      ("ActorReadyFrameChunks", true, true),
      ("ActorWaitingFrameChunks", true, true),
      ("ActorControlLocators", true, true),
      ("ActorReadyHead", false, false),
      ("ActorReadyTail", false, false),
      ("ActorReadyOccupancy", false, false),
      ("ActorWaitingHeads", false, true),
      ("ActorWaitingTails", false, true),
      ("ActorWaitingOccupancies", false, true),
      ("ActorWaitingCursorIndices", true, true),
      ("WakeupCursorPages", true, true),
      ("WakeupCursorLen", false, true),
      ("NextWakeupClock", false, false),
      ("OwnerSlotBitmaps", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("UserCreationEnabled", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("IdleStarvationState", false, false),
    ]
  );

  assert_eq!(
    storage_info
      .iter()
      .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
      .collect::<Vec<_>>(),
    actual_shapes
      .iter()
      .map(|(name, _, _)| *name)
      .collect::<Vec<_>>(),
    "storage info and complete canonical metadata must agree without exclusions"
  );

  let entry = |name: &str| {
    metadata
      .entries
      .iter()
      .find(|entry| entry.name == name)
      .expect("declared storage entry exists")
  };
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entry("ActorContractHead"));
  assert_map_storage_types::<(u64, u32), crate::ActorStepChunkOf<Test>>(entry(
    "ActorContractTailChunk",
  ));
  assert_map_storage_types::<u64, crate::ActorStateHoldRecordOf<Test>>(entry("ActorStateHolds"));

  assert_plain_storage_type::<u64>(entry("NextActorId"));
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entry("ActorContractHead"));

  assert_map_storage_types::<u64, crate::ActorControlCellOf<Test>>(entry(
    "ActorUnsignaledControlCells",
  ));
  assert_map_storage_types::<u64, crate::ActorControlChunkOf<Test>>(entry("ActorReadyFrameChunks"));
  assert_map_storage_types::<(WakeupKey<MockBlockNumber>, u64), crate::ActorWaitingPageOf<Test>>(
    entry("ActorWaitingFrameChunks"),
  );
  assert_map_storage_types::<u64, crate::ActorControlLocation<MockBlockNumber>>(entry(
    "ActorControlLocators",
  ));
  assert_plain_storage_type::<u64>(entry("ActorReadyHead"));
  assert_plain_storage_type::<u64>(entry("ActorReadyTail"));
  assert_plain_storage_type::<u32>(entry("ActorReadyOccupancy"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u64>(entry("ActorWaitingHeads"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u64>(entry("ActorWaitingTails"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u32>(entry("ActorWaitingOccupancies"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u32>(entry("ActorWaitingCursorIndices"));
  assert_plain_storage_type::<(MockBlockNumber, u64)>(entry("PrepassExecutionCutoff"));
  assert_plain_storage_type::<crate::BlockResourceState<MockBlockNumber>>(entry(
    "CurrentBlockResourceState",
  ));
  assert_plain_storage_type::<crate::FinalizedBlockResourceSnapshot<MockBlockNumber>>(entry(
    "FinalizedBlockResourceTelemetry",
  ));

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
      "close_when",
      "on_unfunded",
      "parked_balance_activation",
      "auto_close_at_cycle_nonce"
    ]
  );
  assert_map_storage_types::<u64, RuntimeActorRunState>(entry("ActorRunState"));
  let run_type = registry.register_type(&scale_info::meta_type::<RuntimeActorRunState>());
  let (_, run_state) = registry
    .types()
    .find(|(symbol, _)| symbol.id == run_type.id)
    .expect("Actor run type is registered");
  let scale_info::TypeDef::Composite(run_fields) = &run_state.type_def else {
    panic!("Actor run metadata must be composite");
  };
  assert_eq!(
    run_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor run field"))
      .collect::<Vec<_>>(),
    [
      "contract_authority",
      "cycle_nonce",
      "balance_snapshot",
      "cursor",
      "unsuccessful_attempts_at_cursor",
      "last_attempt_block",
      "last_committed_step_block",
      "eligible_at",
      "cumulative_outcomes",
      "last_step_outcome",
      "suspension"
    ]
  );
  assert_map_storage_types::<u64, crate::ActorIdentityOf<Test>>(entry("ActorIdentities"));
  assert_plain_storage_type::<DependencySourceAllocator>(entry("DependencySourceAllocatorState"));
  assert_map_storage_types::<TestAsset, u64>(entry("BalanceDependencySources"));
  assert_map_storage_types::<u64, TestAsset>(entry("DependencySourceBalances"));
  assert_plain_storage_type::<u32>(entry("ActorIdentityCount"));
  assert_plain_storage_type::<u32>(entry("ActiveActorCount"));
  assert_map_storage_types::<u64, SystemSovereignState>(entry("SystemSovereigns"));
  assert_plain_storage_type::<u32>(entry("SystemSovereignCount"));
  assert_map_storage_types::<(WakeupClock, u64), crate::WakeupCursorPageOf<Test>>(entry(
    "WakeupCursorPages",
  ));
  assert_map_storage_types::<WakeupClock, u32>(entry("WakeupCursorLen"));
  assert_plain_storage_type::<WakeupClock>(entry("NextWakeupClock"));
  assert_map_storage_types::<AccountId, [u8; 32]>(entry("OwnerSlotBitmaps"));
  assert_map_storage_types::<AccountId, u64>(entry("SovereignIndex"));
  assert_plain_storage_type::<u32>(entry("ActiveActorLimit"));
  assert_plain_storage_type::<bool>(entry("GlobalCircuitBreaker"));
  assert_plain_storage_type::<IdleStarvationPhase>(entry("IdleStarvationState"));
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
  let excessive_span = u64::from(<<Test as crate::Config>::MaxQueueLength as Get<u32>>::get()) + 1;
  for (tail, occupancy) in [(1u64, 0u32), (excessive_span, 1u32)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(enqueue_latched_actor(actor_id));
      crate::ActorReadyTail::<Test>::put(tail);
      crate::ActorReadyOccupancy::<Test>::put(occupancy);
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
        let identity = Actors::actor_identity(actor_id).expect("system identity");
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
      let identity = Actors::actor_identity(actor_id).expect("user identity");
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
      mutate_actor_hot_coherent(actor_id, |hot| hot.terminal_at = Some(999));
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorHot terminal_at disagrees with schedule window terminal membership\")"
            .into()
        )
      );
      mutate_actor_hot_coherent(actor_id, |hot| hot.terminal_at = None);
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      mutate_actor_hot_coherent(actor_id, |hot| hot.terminal_at = Some(102));
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_loader_rejects_contract_admission_disagreement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let crate::LoadedActorStateOf::Active(frame_state) =
      Actors::load_actor_state_for_frame_control(actor_id)
    else {
      panic!("canonical frame state must remain active");
    };
    assert!(!frame_state.hot.pending_signal);
    assert_eq!(frame_state.hot.unsuccessful_attempt_streak, 0);
    assert!(matches!(
      frame_state.hot.trigger_runtime_state,
      TriggerRuntimeState::Stateless
    ));
    assert_eq!(frame_state.identity.cycle_nonce, 0);
    let projected = Actors::active_actor_state(actor_id).expect("frame-owned projection is active");
    assert!(!projected.hot.pending_signal);
    assert_eq!(projected.hot.unsuccessful_attempt_streak, 0);
    assert_eq!(projected.identity.cycle_nonce, 0);

    let (service_state, service_admission, loaded_step) =
      Actors::load_frame_actor_service_state(actor_id).expect("frame service state remains live");
    assert_eq!(service_state.identity.cycle_nonce, 0);
    assert!(!service_state.hot.pending_signal);
    assert!(service_admission.has_valid_identity());
    assert_eq!(loaded_step.expect("current Step remains loaded").cursor, 0);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Contract head remains live")
        .header
        .body_commitment[0] ^= 1;
    });
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    assert!(Actors::active_actor_state(actor_id).is_none());
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
    assert!(!Actors::pending_signal(actor_id));
    ActorContractHeads::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_noop!(
      Actors::write_run_state(actor_id, None),
      Error::<Test>::ActorInvariant
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );

    crate::ActorControlLocators::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn canonical_loader_classifies_every_primary_partition_presence_mask() {
  for mask in 0u8..8 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      // The canonical partition presence domain is the semantic owner, the Contract geometry and
      // the generation-bound process/residence carrier; a fresh genesis never publishes a legacy
      // primary cell or locator.
      let semantic =
        crate::ActorSemanticStates::<Test>::take(actor_id).expect("semantic authority fixture");
      let head = ActorContractHeads::<Test>::take(actor_id).expect("Contract head fixture");
      let process = ActorProcesses::<Test>::take(actor_id).expect("process residence fixture");
      if mask & 0b001 != 0 {
        crate::ActorSemanticStates::<Test>::insert(actor_id, semantic);
      }
      if mask & 0b010 != 0 {
        ActorContractHeads::<Test>::insert(actor_id, head);
      }
      if mask & 0b100 != 0 {
        ActorProcesses::<Test>::insert(actor_id, process);
      }
      let loaded = Actors::load_actor_state(actor_id);
      match mask {
        0 => assert!(matches!(loaded, LoadedActorStateOf::NotRegistered)),
        7 => assert!(matches!(loaded, LoadedActorStateOf::Active(_))),
        _ => assert!(
          matches!(loaded, LoadedActorStateOf::Corrupt),
          "mask {mask:03b}"
        ),
      }
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_contract_tail_keys_outside_the_admitted_geometry() {
  for orphan in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let step = transfer_contract_steps(BOB, 1)[0].clone();
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        BoundedVec::try_from(vec![step.clone(), step]).expect("two Steps fit"),
      );
      assert_ok!(Actors::do_try_state());
      let chunk = ActorContractTailChunks::<Test>::get(actor_id, 0).expect("admitted tail");
      let key = if orphan {
        (actor_id + 100, 0)
      } else {
        (actor_id, 1)
      };
      ActorContractTailChunks::<Test>::insert(key.0, key.1, chunk);
      assert!(
        Actors::do_try_state().is_err(),
        "unowned tail {key:?} was accepted"
      );
      ActorContractTailChunks::<Test>::remove(key.0, key.1);
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_system_reverse_index_outside_derived_account() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let original = Actors::actor_identity(actor_id).expect("System identity fixture");
    let replacement_account = 999_998;
    crate::SovereignIndex::<Test>::remove(original.sovereign_account);
    crate::SovereignIndex::<Test>::insert(replacement_account, actor_id);
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("identity derives its account")
        .sovereign_account,
      original.sovereign_account
    );
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
    ActorContractHeads::<Test>::remove(actor_id);
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
        ample_simulation_budget(),
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
  });
}
