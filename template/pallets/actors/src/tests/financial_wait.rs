use crate::{
  ActorRef, ActorRunAuthority, ActorType, FUNDING_GRACE_MILLIS, FinancialWaitDecision,
  FinancialWaitEpisode, FinancialWaitError, FinancialWaitRecord, FinancialWaitScope,
  UnpaidActorService,
};
use codec::{Decode, Encode, MaxEncodedLen};

const TICK_MILLIS: u64 = 500;

fn scope() -> FinancialWaitScope {
  FinancialWaitScope {
    actor: ActorRef {
      actor_id: 12,
      generation: 3,
    },
    actor_type: ActorType::User,
    contract_authority: ActorRunAuthority {
      semantic_contract_id: [1; 32],
      body_commitment: [2; 32],
      admission_identity: [3; 32],
      pipeline_service_identity: [4; 32],
    },
    behavior_episode: Some([5; 32]),
  }
}

fn boundaries() -> [UnpaidActorService; 4] {
  [
    UnpaidActorService::AtTimeTrigger { due_tick: 1 },
    UnpaidActorService::PipelineOpening { cycle_nonce: 7 },
    UnpaidActorService::RunningAction {
      cycle_nonce: 7,
      cursor: 2,
    },
    UnpaidActorService::SuspendedAction {
      cycle_nonce: 7,
      cursor: 2,
      unsuccessful_attempts_at_cursor: 3,
    },
  ]
}

fn episode(boundary: UnpaidActorService) -> FinancialWaitRecord<u128> {
  FinancialWaitRecord::new(scope(), boundary, 10, 501, TICK_MILLIS, u64::MAX)
    .expect("valid financial episode")
}

#[test]
fn financial_wait_recertification_preserves_original_identity_and_rejects_replacement() {
  for boundary in boundaries() {
    let original = FinancialWaitEpisode::new(episode(boundary));
    let mut current = original;
    for version in [6, 7] {
      let mut rebound_scope = scope();
      rebound_scope.contract_authority.admission_identity = [version; 32];
      rebound_scope.contract_authority.pipeline_service_identity = [version + 1; 32];
      current = current
        .rebind(rebound_scope, TICK_MILLIS, u64::MAX)
        .unwrap();
      assert_eq!(current.original_identity(), original.original_identity());
      assert_eq!(
        current.record().entered_at_millis(),
        original.record().entered_at_millis()
      );
      assert_eq!(
        current.record().expiry_tick(),
        original.record().expiry_tick()
      );
      assert_eq!(current.record().unpaid_service(), boundary);
      assert_eq!(current.record().unpaid_liability(), 10);
      assert!(
        current
          .bind(rebound_scope, boundary, 10, TICK_MILLIS, u64::MAX)
          .is_ok()
      );
      assert!(
        current
          .bind(scope(), boundary, 10, TICK_MILLIS, u64::MAX)
          .is_err()
      );
    }
    for mutation in 0..5 {
      let mut changed = current.record().scope();
      match mutation {
        0 => changed.actor.actor_id += 1,
        1 => changed.actor.generation += 1,
        2 => changed.contract_authority.semantic_contract_id = [8; 32],
        3 => changed.contract_authority.body_commitment = [8; 32],
        4 => changed.behavior_episode = None,
        _ => unreachable!(),
      }
      assert_eq!(
        current.rebind(changed, TICK_MILLIS, u64::MAX),
        Err(FinancialWaitError::BindingMismatch)
      );
    }
    let mut encoded = current.encode();
    *encoded.last_mut().unwrap() ^= 1;
    let forged = FinancialWaitEpisode::<u128>::decode(&mut &encoded[..]).unwrap();
    assert!(
      forged
        .bind(
          current.record().scope(),
          boundary,
          10,
          TICK_MILLIS,
          u64::MAX
        )
        .is_err()
    );
  }
}

#[test]
fn financial_wait_recertification_cannot_reinterpret_the_original_clock() {
  for boundary in boundaries() {
    let original = FinancialWaitEpisode::new(episode(boundary));
    let mut authority = scope();
    authority.contract_authority.admission_identity = [6; 32];
    authority.contract_authority.pipeline_service_identity = [7; 32];
    for tick_millis in [250, 1_000] {
      let error =
        if matches!(boundary, UnpaidActorService::AtTimeTrigger { .. }) && tick_millis == 1_000 {
          FinancialWaitError::InvalidBoundary
        } else {
          FinancialWaitError::InvalidExpiry
        };
      assert_eq!(
        original.rebind(authority, tick_millis, u64::MAX),
        Err(error)
      );
    }
    let delay =
      original.record().expiry_tick() - original.record().entered_at_millis() / TICK_MILLIS;
    assert_eq!(
      original.rebind(authority, TICK_MILLIS, delay - 1),
      Err(FinancialWaitError::Clock(
        crate::FinancialWaitClockError::TemporalDelayExceeded
      ))
    );
    assert_eq!(original.original_identity(), original.record().identity());
    assert!(
      original
        .bind(scope(), boundary, 10, TICK_MILLIS, u64::MAX)
        .is_ok()
    );
  }
}

#[test]
fn financial_wait_clock_binding_rejects_equal_tick_endpoints_in_different_clocks() {
  for boundary in boundaries() {
    let record = FinancialWaitRecord::new(scope(), boundary, 10_u128, 120_000, 60_000, u64::MAX)
      .expect("valid coarse-clock episode");
    let other_clock =
      FinancialWaitRecord::new(scope(), boundary, 10_u128, 120_000, 60_001, u64::MAX)
        .expect("valid distinct clock with the same Tick endpoint");
    assert_eq!(record.expiry_tick(), other_clock.expiry_tick());
    assert_ne!(
      record.expiry_tick() * 60_000,
      other_clock.expiry_tick() * 60_001
    );
    assert!(
      record
        .bind(scope(), boundary, 10, record.identity(), 60_000, u64::MAX)
        .is_ok()
    );
    assert!(matches!(
      record.bind(scope(), boundary, 10, record.identity(), 60_001, u64::MAX),
      Err(FinancialWaitError::InvalidExpiry)
    ));
    assert_ne!(record.identity(), other_clock.identity());
    let mut encoded = record.encode();
    let clock_offset = encoded.len() - 16;
    encoded[clock_offset..clock_offset + 8].copy_from_slice(&60_001_u64.to_le_bytes());
    let forged = FinancialWaitRecord::<u128>::decode(&mut &encoded[..]).unwrap();
    assert!(matches!(
      forged.bind(scope(), boundary, 10, record.identity(), 60_001, u64::MAX),
      Err(FinancialWaitError::BindingMismatch)
    ));
    let original = FinancialWaitEpisode::new(record);
    let mut authority = scope();
    authority.contract_authority.admission_identity = [6; 32];
    authority.contract_authority.pipeline_service_identity = [7; 32];
    assert_eq!(
      original.rebind(authority, 60_001, u64::MAX),
      Err(FinancialWaitError::InvalidExpiry)
    );
  }
}

#[test]
fn financial_wait_current_solvency_owns_recovery_at_and_after_expiry() {
  for boundary in boundaries() {
    let record = episode(boundary);
    let bound = record
      .bind(
        scope(),
        boundary,
        10,
        record.identity(),
        TICK_MILLIS,
        u64::MAX,
      )
      .expect("current source binds");
    let due_millis = record.expiry_tick() * TICK_MILLIS;
    assert!(due_millis >= record.entered_at_millis() + FUNDING_GRACE_MILLIS);
    for now in [501, due_millis - 1, due_millis, due_millis + 1_000] {
      for spendability in [0, 1, 9, 10, 11] {
        let expected = if spendability >= 10 {
          FinancialWaitDecision::Funded
        } else if now >= due_millis {
          FinancialWaitDecision::GraceExpired
        } else {
          FinancialWaitDecision::Awaiting
        };
        assert_eq!(bound.classify(spendability, now), Ok(expected));
      }
    }
    assert_eq!(
      bound.classify(10, 0),
      Err(FinancialWaitError::ClockReversed)
    );
  }
}

#[test]
fn financial_wait_balance_oscillation_and_dust_do_not_replace_the_episode() {
  let boundary = boundaries()[3];
  let record = episode(boundary);
  let original = record.encode();
  let bound = record
    .bind(
      scope(),
      boundary,
      10,
      record.identity(),
      TICK_MILLIS,
      u64::MAX,
    )
    .expect("current source binds");
  for (now, spendability) in [(501, 1), (502, 10), (503, 0), (504, 11), (505, 9)] {
    let expected = if spendability >= 10 {
      FinancialWaitDecision::Funded
    } else {
      FinancialWaitDecision::Awaiting
    };
    assert_eq!(bound.classify(spendability, now), Ok(expected));
    assert_eq!(record.encode(), original);
  }
  assert_eq!(record.scope(), scope());
  assert_eq!(record.unpaid_service(), boundary);
  assert_eq!(record.unpaid_liability(), 10);
  assert_eq!(record.entered_at_millis(), 501);
}

#[test]
fn financial_wait_rejects_stale_scope_job_and_owed_liability() {
  let original_scope = scope();
  let boundary = boundaries()[3];
  let record = episode(boundary);
  let mut scopes = vec![original_scope; 9];
  scopes[0].actor.actor_id += 1;
  scopes[1].actor.generation += 1;
  scopes[2].contract_authority.semantic_contract_id = [11; 32];
  scopes[3].contract_authority.body_commitment = [12; 32];
  scopes[4].contract_authority.admission_identity = [13; 32];
  scopes[5].contract_authority.pipeline_service_identity = [14; 32];
  scopes[6].behavior_episode = Some([15; 32]);
  scopes[7].behavior_episode = None;
  scopes[8].actor.generation -= 1;
  for changed_scope in scopes {
    assert!(matches!(
      record.bind(
        changed_scope,
        boundary,
        10,
        record.identity(),
        TICK_MILLIS,
        u64::MAX
      ),
      Err(FinancialWaitError::BindingMismatch)
    ));
  }
  for changed_boundary in boundaries().into_iter().chain([
    UnpaidActorService::SuspendedAction {
      cycle_nonce: 8,
      cursor: 2,
      unsuccessful_attempts_at_cursor: 3,
    },
    UnpaidActorService::SuspendedAction {
      cycle_nonce: 7,
      cursor: 3,
      unsuccessful_attempts_at_cursor: 3,
    },
    UnpaidActorService::SuspendedAction {
      cycle_nonce: 7,
      cursor: 2,
      unsuccessful_attempts_at_cursor: 4,
    },
  ]) {
    if changed_boundary == boundary {
      continue;
    }
    assert!(matches!(
      record.bind(
        original_scope,
        changed_boundary,
        10,
        record.identity(),
        TICK_MILLIS,
        u64::MAX
      ),
      Err(FinancialWaitError::BindingMismatch)
    ));
  }
  assert!(matches!(
    record.bind(
      original_scope,
      boundary,
      11,
      record.identity(),
      TICK_MILLIS,
      u64::MAX
    ),
    Err(FinancialWaitError::BindingMismatch)
  ));
}

#[test]
fn financial_wait_decoded_deadline_and_rebased_start_require_original_authority() {
  let boundary = boundaries()[0];
  let record = episode(boundary);
  let original_identity = record.identity();
  let mut encoded = record.encode();
  let expiry_offset = encoded.len() - 8;
  encoded[expiry_offset..].copy_from_slice(&(record.expiry_tick() - 1).to_le_bytes());
  let early = FinancialWaitRecord::<u128>::decode(&mut &encoded[..]).expect("fixed-width record");
  assert!(matches!(
    early.bind(
      scope(),
      boundary,
      10,
      original_identity,
      TICK_MILLIS,
      u64::MAX
    ),
    Err(FinancialWaitError::InvalidExpiry)
  ));
  let rebased = FinancialWaitRecord::new(scope(), boundary, 10, 1_001, TICK_MILLIS, u64::MAX)
    .expect("later endpoint is representable");
  assert!(matches!(
    rebased.bind(
      scope(),
      boundary,
      10,
      original_identity,
      TICK_MILLIS,
      u64::MAX
    ),
    Err(FinancialWaitError::BindingMismatch)
  ));
}

#[test]
fn financial_wait_invalid_or_unowed_sources_cannot_form_an_episode() {
  let boundary = boundaries()[0];
  let mut system_scope = scope();
  system_scope.actor_type = ActorType::System;
  assert_eq!(
    FinancialWaitRecord::new(system_scope, boundary, 10_u128, 1, TICK_MILLIS, u64::MAX),
    Err(FinancialWaitError::UserRequired)
  );
  let mut zero_generation = scope();
  zero_generation.actor.generation = 0;
  assert_eq!(
    FinancialWaitRecord::new(zero_generation, boundary, 10_u128, 1, TICK_MILLIS, u64::MAX),
    Err(FinancialWaitError::InvalidScope)
  );
  assert_eq!(
    FinancialWaitRecord::new(scope(), boundary, 0_u128, 1, TICK_MILLIS, u64::MAX),
    Err(FinancialWaitError::ZeroLiability)
  );
  for invalid in [
    UnpaidActorService::AtTimeTrigger { due_tick: 0 },
    UnpaidActorService::AtTimeTrigger { due_tick: 2 },
    UnpaidActorService::PipelineOpening { cycle_nonce: 0 },
    UnpaidActorService::RunningAction {
      cycle_nonce: 0,
      cursor: 0,
    },
    UnpaidActorService::SuspendedAction {
      cycle_nonce: 0,
      cursor: 0,
      unsuccessful_attempts_at_cursor: 0,
    },
  ] {
    assert_eq!(
      FinancialWaitRecord::new(scope(), invalid, 10_u128, 501, TICK_MILLIS, u64::MAX),
      Err(FinancialWaitError::InvalidBoundary)
    );
  }
}

#[test]
fn financial_wait_codec_bound_is_attained_without_a_payment_ticket_or_history() {
  let record = FinancialWaitRecord::new(
    scope(),
    UnpaidActorService::SuspendedAction {
      cycle_nonce: u64::MAX,
      cursor: u32::MAX,
      unsuccessful_attempts_at_cursor: u32::MAX,
    },
    u128::MAX,
    1,
    TICK_MILLIS,
    u64::MAX,
  )
  .expect("fixed-width maxima fit the codec");
  assert_eq!(
    record.encoded_size(),
    FinancialWaitRecord::<u128>::max_encoded_len()
  );
  let decoded =
    FinancialWaitRecord::<u128>::decode(&mut &record.encode()[..]).expect("record round trip");
  assert_eq!(decoded, record);
  let episode = FinancialWaitEpisode::new(record);
  assert_eq!(
    episode.encoded_size(),
    FinancialWaitEpisode::<u128>::max_encoded_len()
  );
  assert_eq!(episode.encoded_size(), record.encoded_size() + 96);
  assert!(
    decoded
      .bind(
        record.scope(),
        record.unpaid_service(),
        record.unpaid_liability(),
        record.identity(),
        TICK_MILLIS,
        u64::MAX
      )
      .is_ok()
  );
}
