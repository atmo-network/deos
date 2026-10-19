#![cfg(feature = "runtime-benchmarks")]

use crate::*;
use frame::prelude::*;
use polkadot_sdk::frame_benchmarking::{account, v2::*};
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::traits::SaturatedConversion;

fn benchmark_domain_id<T: Config>() -> T::DomainId
where
  T::DomainId: From<u32>,
{
  7u32.into()
}

fn benchmark_item_id<T: Config>(index: u32) -> T::WinningVoteItemId
where
  T::WinningVoteItemId: From<u32>,
{
  index.into()
}

fn expiry_epoch<T: Config>(current_epoch: T::Epoch) -> T::Epoch {
  current_epoch
    .saturated_into::<u32>()
    .saturating_add(T::WinningVoteLookbackEpochs::get())
    .into()
}

fn benchmark_proposal_metadata<T: Config>(
  payload_kind: ProposalPayloadKind,
) -> ProposalMetadata<T::Hash> {
  ProposalMetadata {
    cadence_mode: ProposalCadenceMode::Ordinary,
    payload_kind,
    payload_hash: T::Hash::default(),
  }
}

fn seed_window<T: Config>(
  domain: T::DomainId,
  account: &T::AccountId,
  vote_epoch: T::Epoch,
  count: u16,
) where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let lookback = T::WinningVoteLookbackEpochs::get();
  let mut epochs = BoundedVec::default();
  for _ in 0..lookback {
    let push_result = epochs.try_push(WinningVoteEpochSlot {
      item_ids: BoundedVec::default(),
    });
    if push_result.is_err() {
      panic!("benchmark vote-window seed must fit configured lookback")
    }
  }
  if lookback > 0 {
    let slot_index = (vote_epoch.saturated_into::<u32>() % lookback) as usize;
    let slot = epochs
      .get_mut(slot_index)
      .expect("benchmark slot must stay inside configured lookback");
    for item_index in 0..u32::from(count) {
      let push_result = slot
        .item_ids
        .try_push(benchmark_item_id::<T>(item_index.saturating_add(1)));
      if push_result.is_err() {
        panic!("benchmark item seed must fit configured per-epoch limit")
      }
    }
  }
  WinningVoteWindows::<T>::insert(
    domain,
    account,
    WinningVoteWindow::<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteItemsPerEpoch,
    > {
      last_epoch: vote_epoch,
      epochs,
      rolling_sum: u32::from(count),
    },
  );
}

fn seed_active_proposals<T: Config>(domain: T::DomainId, count: u32)
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let current_epoch = T::EpochProvider::current_epoch();
  let mut item_ids = BoundedVec::default();
  for index in 0..count {
    let item_id = benchmark_item_id::<T>(1_000u32.saturating_add(index));
    ActiveProposals::<T>::insert(
      domain,
      item_id,
      ActiveProposal {
        submitted_epoch: current_epoch,
      },
    );
    ProposalAuthorsByItem::<T>::insert(
      domain,
      item_id,
      account::<T::AccountId>("proposer", index, 0),
    );
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
    );
    let push_result = item_ids.try_push(item_id);
    if push_result.is_err() {
      panic!("benchmark active proposal index must fit configured domain cap")
    }
  }
  ActiveProposalIdsByDomain::<T>::insert(domain, item_ids);
}

fn seed_proposal_votes<T: Config>(
  domain: T::DomainId,
  item_id: T::WinningVoteItemId,
  submitted_epoch: T::Epoch,
  voter_seed: u32,
  ayes: u32,
  nays: u32,
) where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  ActiveProposals::<T>::insert(domain, item_id, ActiveProposal { submitted_epoch });
  ProposalAuthorsByItem::<T>::insert(
    domain,
    item_id,
    account::<T::AccountId>("proposer", voter_seed, 0),
  );
  ProposalMetadataByItem::<T>::insert(
    domain,
    item_id,
    benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
  );
  ActiveProposalIdsByDomain::<T>::insert(
    domain,
    BoundedVec::try_from(alloc::vec![item_id])
      .expect("benchmark active proposal index must fit configured domain cap"),
  );
  let mut vote_state = ProposalVotes {
    ayes: BoundedVec::default(),
    nays: BoundedVec::default(),
    amplifies: BoundedVec::default(),
    approves: BoundedVec::default(),
    reduces: BoundedVec::default(),
    vetoes: BoundedVec::default(),
    passes: BoundedVec::default(),
  };
  for index in 0..ayes {
    let push_result = vote_state.ayes.try_push(ProposalBallot {
      account: account("aye", index, voter_seed),
      vote_epoch: submitted_epoch,
      weight: 1,
      raw_power: 1,
    });
    if push_result.is_err() {
      panic!("benchmark aye vote seed must fit configured max")
    }
  }
  for index in 0..nays {
    let push_result = vote_state.nays.try_push(ProposalBallot {
      account: account("nay", index, voter_seed),
      vote_epoch: submitted_epoch,
      weight: 1,
      raw_power: 1,
    });
    if push_result.is_err() {
      panic!("benchmark nay vote seed must fit configured max")
    }
  }
  ProposalVotesByItem::<T>::insert(domain, item_id, vote_state);
}

fn seed_maturing_proposals<T: Config>(domain: T::DomainId, current_epoch: T::Epoch, n: u32)
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let voting_period = T::ProposalVotingPeriod::get().saturated_into::<u32>();
  let lead_in_period = T::ProposalLeadInPeriod::get().saturated_into::<u32>();
  let submitted_epoch: T::Epoch = current_epoch_u32::<T>(current_epoch)
    .saturating_sub(voting_period)
    .saturating_sub(lead_in_period)
    .into();
  let mut bucket = BoundedVec::default();
  for index in 0..n {
    let item_id = benchmark_item_id::<T>(index.saturating_add(1));
    let push_result = bucket.try_push(MaturingProposalTouch { domain, item_id });
    if push_result.is_err() {
      panic!("benchmark proposal maturity bucket must fit configured max")
    }
    seed_proposal_votes::<T>(
      domain,
      item_id,
      submitted_epoch,
      index,
      T::MaxWinningVoteAccountsPerCall::get(),
      0,
    );
  }
  let active_ids = bucket
    .iter()
    .map(|touch| touch.item_id)
    .collect::<alloc::vec::Vec<_>>();
  ActiveProposalIdsByDomain::<T>::insert(
    domain,
    BoundedVec::try_from(active_ids)
      .expect("benchmark active proposal index must fit configured domain cap"),
  );
  ProposalMaturityBuckets::<T>::insert(current_epoch, bucket);
}

fn seed_finalized_outcomes<T: Config>(
  domain: T::DomainId,
  current_epoch: T::Epoch,
  n: u32,
  maximum: (
    ProposalMetadata<T::Hash>,
    ProposalExecutionSuccessDetail<T::AccountId, T::DomainId, T::Hash, CidBytesOf<T>>,
  ),
) -> Result<(), BenchmarkError>
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let per_epoch = T::MaxFinalizedProposalOutcomesPerEpoch::get();
  let discovery_capacity = T::MaxRecentFinalizedProposalsPerDomain::get();
  let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
  if per_epoch == 0
    || n == 0
    || n > per_epoch
    || n > T::MaxFinalizedProposalOutcomesPerBlock::get()
    || n > discovery_capacity
    || current_epoch
      != Pallet::<T>::add_epochs(
        Pallet::<T>::add_epochs(1u32.into(), T::ProposalEnactmentDelay::get())?,
        retention.into(),
      )?
    || (discovery_capacity - n).div_ceil(per_epoch) > retention
  {
    return Err(BenchmarkError::Skip);
  }
  for index in 0..discovery_capacity {
    let item_id = benchmark_item_id::<T>(
      2_000u32
        .checked_add(index)
        .expect("benchmark finalized identities must fit u32"),
    );
    ProposalMetadataByItem::<T>::insert(domain, item_id, maximum.0.clone());
    let approved_epoch: T::Epoch = if index < n {
      1u32.into()
    } else {
      2u32
        .checked_add((index - n) / per_epoch)
        .expect("benchmark retained epochs must fit u32")
        .into()
    };
    let finalized_epoch =
      Pallet::<T>::add_epochs(approved_epoch, T::ProposalEnactmentDelay::get())?;
    // Retained winning-option state is historical data, never pending execution authority.
    let winning_option = match &maximum.1 {
      ProposalExecutionSuccessDetail::TreasurySpendExecuted { scalar, .. } => match scalar {
        ProposalTreasurySpendScalar::Amplify => ProposalPrimaryTrackOption::Amplify,
        ProposalTreasurySpendScalar::Approve => ProposalPrimaryTrackOption::Approve,
        ProposalTreasurySpendScalar::Reduce => ProposalPrimaryTrackOption::Reduce,
      },
      _ => ProposalPrimaryTrackOption::Aye,
    };
    ProposalWinningPrimaryOptionByItem::<T>::insert(domain, item_id, winning_option);
    let outcome = FinalizedProposalOutcome::Approved {
      approval: ProposalApproval {
        approved_epoch,
        winner_count: 1,
      },
      enactment: ProposalEnactmentOutcome::Enacted {
        epoch: finalized_epoch,
      },
    };
    Pallet::<T>::record_finalized_proposal_outcome(
      domain,
      item_id,
      outcome.clone(),
      finalized_epoch,
    )?;
    Pallet::<T>::update_finalized_proposal(
      domain,
      item_id,
      outcome,
      Some(ProposalExecutionDetail::Succeeded(maximum.1.clone())),
    )?;
  }
  Ok(())
}

fn seed_maximum_finalized_outcomes<T: Config>(
  n: u32,
) -> Result<(T::DomainId, T::Epoch), BenchmarkError>
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let first_finalized_epoch =
    Pallet::<T>::add_epochs(1u32.into(), T::ProposalEnactmentDelay::get())?;
  let current_epoch = Pallet::<T>::add_epochs(
    first_finalized_epoch,
    T::FinalizedProposalOutcomeRetentionEpochs::get().into(),
  )?;
  let beneficiary = account::<T::AccountId>("finalized-beneficiary", 0, 0);
  let (domain, payload_kind, payload_hash, detail) =
    T::BenchmarkHelper::prepare_maximum_finalized_execution(&beneficiary)?;
  if detail.encoded_size() != ProposalExecutionSuccessDetail::<T::AccountId, T::DomainId, T::Hash, CidBytesOf<T>>::max_encoded_len() {
    return Err(BenchmarkError::Stop("finalized execution fixture must reach its declared encoded maximum"));
  }
  let metadata = ProposalMetadata {
    cadence_mode: ProposalCadenceMode::Ordinary,
    payload_kind,
    payload_hash,
  };
  seed_finalized_outcomes::<T>(domain, current_epoch, n, (metadata, detail))?;
  let record = FinalizedProposals::<T>::get(domain, benchmark_item_id::<T>(2_000))
    .ok_or(BenchmarkError::Stop("finalized fixture record missing"))?;
  if record.encoded_size() != FinalizedProposalRecord::<T::AccountId, T::DomainId, T::Hash, T::Epoch, CidBytesOf<T>>::max_encoded_len() {
    return Err(BenchmarkError::Stop("finalized fixture must reach its declared record maximum"));
  }
  Ok((domain, current_epoch))
}

fn prepare_finalized_cleanup<T: Config>(
  n: u32,
) -> Result<(T::Epoch, impl FnOnce() -> Result<(), BenchmarkError>), BenchmarkError>
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
  T::DomainId: From<u32>,
{
  let (domain, current_epoch) = seed_maximum_finalized_outcomes::<T>(n)?;
  let current_epoch_u32 = current_epoch_u32::<T>(current_epoch);
  let ids_before = FinalizedProposalIdsByDomain::<T>::get(domain);
  let expiry_before = FinalizedProposalOutcomeExpiryBuckets::<T>::get(current_epoch);
  assert_eq!(
    ids_before.len(),
    T::MaxRecentFinalizedProposalsPerDomain::get() as usize
  );
  assert_eq!(expiry_before.len(), n as usize);
  let peers_before = ids_before[n as usize..]
    .iter()
    .map(|item_id| {
      (
        *item_id,
        FinalizedProposals::<T>::get(domain, item_id),
        ProposalMetadataByItem::<T>::get(domain, item_id),
        ProposalWinningPrimaryOptionByItem::<T>::get(domain, item_id),
      )
    })
    .collect::<Vec<_>>();
  let future_expiry_before = (1..=T::FinalizedProposalOutcomeRetentionEpochs::get())
    .map(|offset| {
      let epoch = Pallet::<T>::add_epochs(current_epoch, offset.into())?;
      Ok((
        epoch,
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(epoch),
      ))
    })
    .collect::<Result<Vec<_>, DispatchError>>()?;
  let pending_before = PendingEnactments::<T>::get();
  let pending_cursor_before = PendingEnactmentCursor::<T>::get();
  let previous_epoch: T::Epoch = current_epoch_u32.saturating_sub(1).into();
  LastProcessedEpoch::<T>::put(previous_epoch);
  frame_system::Pallet::<T>::set_block_number(current_epoch_u32.into());
  Ok((current_epoch, move || {
    for index in 0..n {
      assert!(!FinalizedProposals::<T>::contains_key(
        domain,
        benchmark_item_id::<T>(2_000u32.saturating_add(index))
      ));
      assert!(!ProposalMetadataByItem::<T>::contains_key(
        domain,
        benchmark_item_id::<T>(2_000u32.saturating_add(index))
      ));
    }
    for index in 0..n {
      assert!(!ProposalWinningPrimaryOptionByItem::<T>::contains_key(
        domain,
        benchmark_item_id::<T>(2_000u32.saturating_add(index))
      ));
    }
    assert_eq!(
      FinalizedProposalIdsByDomain::<T>::get(domain).as_slice(),
      &ids_before[n as usize..],
    );
    assert!(!FinalizedProposalOutcomeExpiryBuckets::<T>::contains_key(
      current_epoch
    ));
    for (item_id, record, metadata, winning_option) in peers_before {
      assert_eq!(FinalizedProposals::<T>::get(domain, item_id), record);
      assert_eq!(ProposalMetadataByItem::<T>::get(domain, item_id), metadata);
      assert_eq!(
        ProposalWinningPrimaryOptionByItem::<T>::get(domain, item_id),
        winning_option
      );
    }
    for (epoch, bucket) in future_expiry_before {
      assert_eq!(
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(epoch),
        bucket
      );
    }
    assert_eq!(PendingEnactments::<T>::get(), pending_before);
    assert_eq!(PendingEnactmentCursor::<T>::get(), pending_cursor_before);
    Ok(())
  }))
}

/// One shared fixture population across n distinct due domains, not a global-state bound or
/// a Cartesian per-domain maximum.
fn seed_diverse_finalized_outcomes<T: Config>(n: u32) -> Result<(T::Epoch, u32), BenchmarkError>
where
  T::DomainId: From<u32>,
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let total = T::MaxRecentFinalizedProposalsPerDomain::get();
  let per_epoch = T::MaxFinalizedProposalOutcomesPerEpoch::get();
  let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
  if n < 2
    || n > total
    || n > per_epoch
    || n > T::MaxFinalizedProposalOutcomesPerBlock::get()
    || per_epoch == 0
    || 101u32.checked_add(n).is_none()
    || (total - n).div_ceil(per_epoch) > retention
  {
    return Err(BenchmarkError::Skip);
  }
  let expiry = Pallet::<T>::add_epochs(1u32.into(), retention.into())?;
  for index in 0..total {
    let finalized_epoch: T::Epoch = if index < n {
      1
    } else {
      2 + (index - n) / per_epoch
    }
    .into();
    Pallet::<T>::record_finalized_proposal_outcome(
      (101 + index % n).into(),
      benchmark_item_id::<T>(index),
      FinalizedProposalOutcome::Rejected {
        finalized_epoch,
        reason: ProposalRejectionReason::NoVotes,
      },
      finalized_epoch,
    )?;
  }
  Ok((expiry, total))
}

fn prepare_diverse_finalized_cleanup<T: Config>(
  n: u32,
) -> Result<(T::Epoch, impl FnOnce() -> Result<(), BenchmarkError>), BenchmarkError>
where
  T::DomainId: From<u32>,
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let (expiry, total) = seed_diverse_finalized_outcomes::<T>(n)?;
  let domains = (0..n)
    .map(|offset| T::DomainId::from(101 + offset))
    .collect::<Vec<_>>();
  let indices = domains
    .iter()
    .map(FinalizedProposalIdsByDomain::<T>::get)
    .collect::<Vec<_>>();
  assert_eq!(
    indices.iter().map(|ids| ids.len()).sum::<usize>(),
    total as usize
  );
  let due = FinalizedProposalOutcomeExpiryBuckets::<T>::get(expiry);
  assert_eq!(due.len(), n as usize);
  assert!(
    domains
      .iter()
      .all(|domain| due.iter().any(|touch| &touch.domain == domain))
  );
  let future_records = (n..total)
    .map(|index| {
      let domain: T::DomainId = (101 + index % n).into();
      let item = benchmark_item_id::<T>(index);
      (domain, item, FinalizedProposals::<T>::get(domain, item))
    })
    .collect::<Vec<_>>();
  let future_buckets = (1..=T::FinalizedProposalOutcomeRetentionEpochs::get())
    .map(|offset| {
      let epoch = Pallet::<T>::add_epochs(expiry, offset.into())?;
      Ok((
        epoch,
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(epoch),
      ))
    })
    .collect::<Result<Vec<_>, DispatchError>>()?;
  let now = current_epoch_u32::<T>(expiry);
  let previous_epoch: T::Epoch = (now - 1).into();
  LastProcessedEpoch::<T>::put(previous_epoch);
  frame_system::Pallet::<T>::set_block_number(now.into());
  let pending_before = PendingEnactments::<T>::get();
  let pending_cursor_before = PendingEnactmentCursor::<T>::get();
  Ok((expiry, move || {
    for touch in &due {
      assert!(!FinalizedProposals::<T>::contains_key(
        touch.domain,
        touch.item_id
      ));
    }
    for (position, domain) in domains.into_iter().enumerate() {
      let expected = indices[position]
        .iter()
        .copied()
        .filter(|id| {
          !due
            .iter()
            .any(|touch| touch.domain == domain && touch.item_id == *id)
        })
        .collect::<Vec<_>>();
      assert_eq!(
        FinalizedProposalIdsByDomain::<T>::get(domain).as_slice(),
        expected.as_slice()
      );
    }
    assert!(FinalizedProposalOutcomeExpiryBuckets::<T>::get(expiry).is_empty());
    for (domain, item, record) in future_records {
      assert_eq!(FinalizedProposals::<T>::get(domain, item), record);
    }
    for (epoch, bucket) in future_buckets {
      assert_eq!(
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(epoch),
        bucket
      );
    }
    assert_eq!(PendingEnactments::<T>::get(), pending_before);
    assert_eq!(PendingEnactmentCursor::<T>::get(), pending_cursor_before);
    Ok(())
  }))
}

/// Two complete indices, including all peers; a prepared state, not admitted chronology.
fn prepare_independent_full_indices_cleanup<T: Config>(
  distinct_values: bool,
) -> Result<
  (
    T::Epoch,
    T::Epoch,
    impl FnOnce() -> Result<(), BenchmarkError>,
  ),
  BenchmarkError,
>
where
  T::DomainId: From<u32>,
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let members = T::MaxRecentFinalizedProposalsPerDomain::get();
  let per_epoch = T::MaxFinalizedProposalOutcomesPerEpoch::get();
  let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
  let Some(total) = members.checked_mul(2) else {
    return Err(BenchmarkError::Skip);
  };
  if members == 0
    || per_epoch < 2
    || retention == 0
    || T::MaxFinalizedProposalOutcomesPerBlock::get() < 2
    || T::MaxEpochCatchUpPerBlock::get() == 0
    || 2000u32.checked_add(members).is_none()
    || !PendingEnactments::<T>::get().is_empty()
  {
    return Err(BenchmarkError::Skip);
  }
  let old_expiry = Pallet::<T>::add_epochs(1u32.into(), retention.into())?;
  if T::Epoch::from(current_epoch_u32::<T>(old_expiry)) != old_expiry {
    return Err(BenchmarkError::Skip);
  }
  let latest_terminal = if total == 2 {
    1
  } else {
    2 + (total - 3) / per_epoch
  };
  let clock = latest_terminal.max(current_epoch_u32::<T>(old_expiry));
  let now: T::Epoch = clock.into();
  let domains = [T::DomainId::from(101), T::DomainId::from(102)];
  assert_ne!(domains[0], domains[1]);
  for flat in 0..total {
    let terminal: T::Epoch = if flat < 2 {
      1
    } else {
      2 + (flat - 2) / per_epoch
    }
    .into();
    let item =
      benchmark_item_id::<T>(2000 + flat / 2 + u32::from(distinct_values && flat == total - 1));
    Pallet::<T>::record_finalized_proposal_outcome(
      domains[(flat % 2) as usize],
      item,
      FinalizedProposalOutcome::Rejected {
        finalized_epoch: terminal,
        reason: ProposalRejectionReason::NoVotes,
      },
      terminal,
    )?;
  }
  frame_system::Pallet::<T>::set_block_number(clock.into());
  if T::EpochProvider::current_epoch() != now {
    return Err(BenchmarkError::Skip);
  }
  let previous: T::Epoch = (current_epoch_u32::<T>(old_expiry) - 1).into();
  LastProcessedEpoch::<T>::put(previous);
  CurrentEpochServicePhase::<T>::put(EpochServicePhase::FinalizedOutcome);
  assert!(ProposalMaturityBuckets::<T>::get(old_expiry).is_empty());
  let indices = domains.map(FinalizedProposalIdsByDomain::<T>::get);
  assert_eq!(indices[0].len(), members as usize);
  assert_eq!(indices[1].len(), members as usize);
  if distinct_values {
    assert_eq!(
      &indices[0][..members as usize - 1],
      &indices[1][..members as usize - 1]
    );
    assert_ne!(indices[0].last(), indices[1].last());
  } else {
    assert_eq!(indices[0], indices[1]);
  }
  let due = FinalizedProposalOutcomeExpiryBuckets::<T>::get(old_expiry);
  assert_eq!(due.len(), 2);
  for ((touch, domain), index) in due.iter().zip(domains).zip(&indices) {
    assert_eq!(touch.domain, domain);
    assert_eq!(touch.item_id, index[0]);
  }
  let peers = domains
    .iter()
    .copied()
    .zip(&indices)
    .flat_map(|(domain, ids)| {
      ids
        .iter()
        .skip(1)
        .map(move |item| (domain, *item, FinalizedProposals::<T>::get(domain, item)))
    })
    .collect::<Vec<_>>();
  let future = (2..=latest_terminal)
    .map(|terminal| {
      let expiry = Pallet::<T>::add_epochs(terminal.into(), retention.into())?;
      Ok((
        expiry,
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(expiry),
      ))
    })
    .collect::<Result<Vec<_>, DispatchError>>()?;
  Ok((now, old_expiry, move || {
    for (domain, index) in domains.into_iter().zip(indices) {
      assert_eq!(
        FinalizedProposalIdsByDomain::<T>::get(domain).as_slice(),
        &index[1..]
      );
      assert!(!FinalizedProposals::<T>::contains_key(domain, index[0]));
    }
    assert!(!FinalizedProposalOutcomeExpiryBuckets::<T>::contains_key(
      old_expiry
    ));
    for (domain, item, record) in peers {
      assert!(record.is_some());
      assert_eq!(FinalizedProposals::<T>::get(domain, item), record);
    }
    for (epoch, bucket) in future {
      assert_eq!(
        FinalizedProposalOutcomeExpiryBuckets::<T>::get(epoch),
        bucket
      );
    }
    assert_eq!(
      CurrentEpochServicePhase::<T>::get(),
      EpochServicePhase::RewardExpiry
    );
    assert_eq!(LastProcessedEpoch::<T>::get(), previous);
    Ok(())
  }))
}

fn prepare_full_index_value_classes<T: Config>(
  distinct_values: bool,
) -> Result<
  (
    T::Epoch,
    T::Epoch,
    impl FnOnce() -> Result<(), BenchmarkError>,
  ),
  BenchmarkError,
>
where
  T::DomainId: From<u32>,
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let (now, expiry, check_cleanup) =
    prepare_independent_full_indices_cleanup::<T>(distinct_values)?;
  let pending = PendingEnactments::<T>::get();
  let cursor = PendingEnactmentCursor::<T>::get();
  Ok((now, expiry, move || {
    check_cleanup()?;
    assert_eq!(PendingEnactments::<T>::get(), pending);
    assert_eq!(PendingEnactmentCursor::<T>::get(), cursor);
    Ok(())
  }))
}

fn current_epoch_u32<T: Config>(epoch: T::Epoch) -> u32 {
  epoch.saturated_into::<u32>()
}

fn prepare_full_pending_enactment_topology<T: Config>(
  current_epoch: T::Epoch,
  preparation_epoch: T::Epoch,
  cleanup_refusal: bool,
  payload_source: Option<(
    T::DomainId,
    ProposalPayloadKind,
    T::Hash,
    Option<ProposalPrimaryTrackOption>,
  )>,
) -> Result<
  impl FnOnce(&[(T::WinningVoteItemId, ProposalExecutionFailureReason)]) -> Result<(), BenchmarkError>,
  BenchmarkError,
>
where
  T::DomainId: From<u32>,
  T::WinningVoteItemId: From<u32>,
  T::Epoch: From<u32>,
  BlockNumberFor<T>: From<u32>,
{
  let f = u32::from(cleanup_refusal);
  let current_block = current_epoch_u32::<T>(current_epoch);
  let preparation_block = current_epoch_u32::<T>(preparation_epoch);
  if T::Epoch::from(current_block) != current_epoch
    || T::Epoch::from(preparation_block) != preparation_epoch
    || preparation_epoch > current_epoch
    || (cleanup_refusal && preparation_epoch != current_epoch)
  {
    return Err(BenchmarkError::Skip);
  }
  let visits = T::MaxPendingEnactmentsPerBlock::get();
  let pending_capacity = T::MaxPendingEnactments::get();
  let cleanup_capacity = T::MaxFinalizedProposalOutcomesPerEpoch::get();
  if visits == 0 || visits > pending_capacity || visits > cleanup_capacity {
    return Err(BenchmarkError::Skip);
  }
  let first_due = pending_capacity - visits;
  let delay = T::ProposalEnactmentDelay::get();
  let delay_u32 = current_epoch_u32::<T>(delay);
  if T::Epoch::from(delay_u32) != delay {
    return Err(BenchmarkError::Skip);
  }
  let Some(approved_u32) = current_block
    .checked_sub(delay_u32)
    .filter(|epoch| *epoch > 0)
  else {
    return Err(BenchmarkError::Skip);
  };
  let due_approved_epoch = T::Epoch::from(approved_u32);
  let future_approved_epoch = Pallet::<T>::add_epochs(due_approved_epoch, 1u32.into())?;
  // Every approval must already exist in the prepared caller state.
  if due_approved_epoch > preparation_epoch
    || (first_due > 0 && future_approved_epoch > preparation_epoch)
  {
    return Err(BenchmarkError::Skip);
  }
  let future_due = Pallet::<T>::add_epochs(current_epoch, 1u32.into())?;
  let cleanup_epoch = Pallet::<T>::add_epochs(
    current_epoch,
    T::FinalizedProposalOutcomeRetentionEpochs::get().into(),
  )?;
  let peer_cleanup_epoch = Pallet::<T>::add_epochs(
    preparation_epoch,
    T::FinalizedProposalOutcomeRetentionEpochs::get().into(),
  )?;
  if preparation_epoch < current_epoch
    && (T::FinalizedProposalOutcomeRetentionEpochs::get() == 0
      || !FinalizedProposalOutcomeExpiryBuckets::<T>::get(cleanup_epoch).is_empty())
  {
    return Err(BenchmarkError::Skip);
  }
  let (domain, payload_kind, payload_hash, winning_option) = match payload_source {
    Some(source) => source,
    None => {
      let proposer = account("pending-full", 0, 0);
      let (domain, kind, hash) = T::BenchmarkHelper::prepare_pending_enactment(&proposer)?;
      (domain, kind, hash, None)
    }
  };
  if matches!(
    payload_kind,
    ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1
  ) || !T::ProposalPayloadExecutor::can_execute(payload_kind)
    || !T::ProposalPayloadPreimageProvider::have_preimage(&payload_hash)
  {
    return Err(BenchmarkError::Skip);
  }
  let peer_domain = benchmark_domain_id::<T>();
  let cleanup_peers = cleanup_capacity - if f == 0 { visits } else { 0 };
  let discovery_required = if peer_domain == domain {
    pending_capacity.checked_add(cleanup_peers)
  } else {
    Some(pending_capacity.max(cleanup_peers))
  };
  if discovery_required.is_none_or(|count| count > T::MaxRecentFinalizedProposalsPerDomain::get()) {
    return Err(BenchmarkError::Skip);
  }
  for index in 0..cleanup_peers {
    let item_id = benchmark_item_id::<T>(index);
    ProposalMetadataByItem::<T>::insert(
      peer_domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::Intent),
    );
    Pallet::<T>::record_finalized_proposal_outcome(
      peer_domain,
      item_id,
      FinalizedProposalOutcome::Rejected {
        finalized_epoch: preparation_epoch,
        reason: ProposalRejectionReason::AdminRejected,
      },
      preparation_epoch,
    )?;
  }
  let mut due_ids = Vec::new();
  for index in 0..pending_capacity {
    let item_id = benchmark_item_id::<T>(
      cleanup_capacity
        .checked_add(index)
        .expect("benchmark pending identities must fit u32"),
    );
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      ProposalMetadata {
        cadence_mode: ProposalCadenceMode::Ordinary,
        payload_kind,
        payload_hash,
      },
    );
    let approved_epoch = if index >= first_due {
      due_approved_epoch
    } else {
      future_approved_epoch
    };
    if let Some(option) = winning_option {
      ProposalWinningPrimaryOptionByItem::<T>::insert(domain, item_id, option);
    }
    Pallet::<T>::record_finalized_proposal_outcome(
      domain,
      item_id,
      FinalizedProposalOutcome::Approved {
        approval: ProposalApproval {
          approved_epoch,
          winner_count: 1,
        },
        enactment: ProposalEnactmentOutcome::NotAttempted,
      },
      approved_epoch,
    )?;
    let due = if index >= first_due {
      due_ids.push(item_id);
      current_epoch
    } else {
      future_due
    };
    assert_eq!(
      due,
      Pallet::<T>::add_epochs(approved_epoch, T::ProposalEnactmentDelay::get())?
    );
    Pallet::<T>::admit_pending_enactment(domain, item_id, due)?;
  }
  PendingEnactmentCursor::<T>::put(first_due);
  frame_system::Pallet::<T>::set_block_number(preparation_block.into());
  let pending_before = PendingEnactments::<T>::get();
  let peers_before = pending_before[..first_due as usize]
    .iter()
    .map(|touch| {
      (
        touch.clone(),
        FinalizedProposals::<T>::get(touch.domain, touch.item_id),
        ProposalMetadataByItem::<T>::get(touch.domain, touch.item_id),
        ProposalPendingEnactmentAt::<T>::get(touch.domain, touch.item_id),
        ProposalWinningPrimaryOptionByItem::<T>::get(touch.domain, touch.item_id),
      )
    })
    .collect::<Vec<_>>();
  let cleanup_before = FinalizedProposalOutcomeExpiryBuckets::<T>::get(cleanup_epoch);
  let peer_cleanup_before = (peer_cleanup_epoch != cleanup_epoch)
    .then(|| FinalizedProposalOutcomeExpiryBuckets::<T>::get(peer_cleanup_epoch));
  assert_eq!(pending_before.len(), pending_capacity as usize);
  assert_eq!(
    cleanup_before.len(),
    if peer_cleanup_before.is_some() {
      0
    } else {
      cleanup_peers as usize
    }
  );
  if let Some(bucket) = &peer_cleanup_before {
    assert_eq!(bucket.len(), cleanup_peers as usize);
  }
  // Normalize FRAME's level metadata as a prior public transaction would.
  let initialized_transaction: Result<(), DispatchError> =
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    });
  initialized_transaction?;
  let root_before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
  Ok(
    move |expected_failures: &[(T::WinningVoteItemId, ProposalExecutionFailureReason)]| {
      assert!(expected_failures.len() <= due_ids.len());
      assert!(f == 0 || expected_failures.is_empty());
      for (index, (item_id, _)) in expected_failures.iter().enumerate() {
        assert!(
          due_ids.contains(item_id),
          "failure expectation must name a due item"
        );
        assert!(
          !expected_failures[..index]
            .iter()
            .any(|(prior, _)| prior == item_id),
          "failure expectations must be unique"
        );
      }
      assert_eq!(PendingEnactmentCursor::<T>::get(), 0);
      let pending_after = PendingEnactments::<T>::get();
      let cleanup_after = FinalizedProposalOutcomeExpiryBuckets::<T>::get(cleanup_epoch);
      if f == 1 {
        assert_eq!(pending_after, pending_before);
        assert_eq!(cleanup_after, cleanup_before);
        // The verification overlay must not change FRAME's level metadata.
        polkadot_sdk::sp_io::storage::start_transaction();
        PendingEnactmentCursor::<T>::put(first_due);
        let restored_root =
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        polkadot_sdk::sp_io::storage::rollback_transaction();
        assert_eq!(restored_root, root_before);
      } else {
        assert_eq!(
          pending_after.as_slice(),
          &pending_before[..first_due as usize]
        );
        assert_eq!(
          &cleanup_after[..cleanup_before.len()],
          cleanup_before.as_slice()
        );
        assert_eq!(cleanup_after.len(), cleanup_before.len() + visits as usize);
      }
      if let Some(bucket) = peer_cleanup_before {
        assert_eq!(
          FinalizedProposalOutcomeExpiryBuckets::<T>::get(peer_cleanup_epoch),
          bucket
        );
      }
      for (touch, record, metadata, due, winning_option) in peers_before {
        assert_eq!(
          FinalizedProposals::<T>::get(touch.domain, touch.item_id),
          record
        );
        assert_eq!(
          ProposalMetadataByItem::<T>::get(touch.domain, touch.item_id),
          metadata
        );
        assert_eq!(
          ProposalPendingEnactmentAt::<T>::get(touch.domain, touch.item_id),
          due
        );
        assert_eq!(
          ProposalWinningPrimaryOptionByItem::<T>::get(touch.domain, touch.item_id),
          winning_option
        );
      }
      for item_id in due_ids {
        let failure = expected_failures
          .iter()
          .find_map(|(expected_id, reason)| (*expected_id == item_id).then_some(*reason));
        let finalized = FinalizedProposals::<T>::get(domain, item_id)
          .expect("benchmark service retains the approval record");
        assert_eq!(
          finalized.outcome,
          FinalizedProposalOutcome::Approved {
            approval: ProposalApproval {
              approved_epoch: due_approved_epoch,
              winner_count: 1
            },
            enactment: if f == 0 {
              if failure.is_some() {
                ProposalEnactmentOutcome::ExecutionFailed {
                  epoch: current_epoch,
                }
              } else {
                ProposalEnactmentOutcome::Enacted {
                  epoch: current_epoch,
                }
              }
            } else {
              ProposalEnactmentOutcome::NotAttempted
            },
          }
        );
        assert_eq!(
          ProposalPendingEnactmentAt::<T>::get(domain, item_id),
          if f == 0 { None } else { Some(current_epoch) }
        );
        if f == 0 {
          if let Some(reason) = failure {
            assert_eq!(
              finalized.execution_detail,
              Some(ProposalExecutionDetail::Failed(reason))
            );
          } else {
            assert!(matches!(
              finalized.execution_detail,
              Some(ProposalExecutionDetail::Succeeded(_))
            ));
          }
        } else {
          assert!(finalized.execution_detail.is_none());
        }
      }
      Ok(())
    },
  )
}

#[benchmarks(where
  T::DomainId: From<u32>,
  T::WinningVoteItemId: From<u32>,
  T::Epoch: From<u32>,
  BlockNumberFor<T>: From<u32>,
)]
mod benches {
  use super::*;

  #[benchmark]
  fn record_winning_vote() {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let caller: T::AccountId = whitelisted_caller();
    let current_epoch = T::EpochProvider::current_epoch();
    let expiry_epoch = expiry_epoch::<T>(current_epoch);
    let occupancy = T::MaxExpiringAccountsPerEpoch::get().saturating_sub(1);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: account("expiring-account", index, 0),
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(expiry_epoch, bucket);
    #[extrinsic_call]
    record_winning_vote(RawOrigin::Root, domain, item_id, caller.clone());
    assert!(WinningVoteWindows::<T>::contains_key(
      domain,
      caller.clone()
    ));
  }

  #[benchmark]
  fn record_winning_vote_batch(n: Linear<1, 256>) {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let current_epoch = T::EpochProvider::current_epoch();
    let expiry_epoch = expiry_epoch::<T>(current_epoch);
    let occupancy = T::MaxExpiringAccountsPerEpoch::get().saturating_sub(n);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: account("expiring-account", index, 0),
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(expiry_epoch, bucket);
    let mut accounts = BoundedVec::default();
    for index in 0..n {
      let push_result = accounts.try_push(account("winner", index, 0));
      if push_result.is_err() {
        panic!("benchmark winning-vote batch must fit configured max")
      }
    }
    let verify_accounts = accounts.clone();
    #[extrinsic_call]
    record_winning_vote_batch(RawOrigin::Root, domain, item_id, accounts);
    for account in verify_accounts {
      assert!(WinningVoteWindows::<T>::contains_key(domain, account));
    }
  }

  #[benchmark]
  fn submit_proposal() {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let proposer: T::AccountId = whitelisted_caller();
    let occupancy = T::MaxActiveProposalsPerDomain::get()
      .saturating_sub(T::StrategicProposalReserve::get())
      .saturating_sub(1);
    seed_active_proposals::<T>(domain, occupancy);
    #[extrinsic_call]
    submit_proposal(
      RawOrigin::Root,
      domain,
      item_id,
      proposer.clone(),
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L2ParameterChange,
      T::Hash::default(),
    );
    assert!(ActiveProposals::<T>::contains_key(domain, item_id));
    assert_eq!(
      ProposalAuthorsByItem::<T>::get(domain, item_id),
      Some(proposer)
    );
  }

  #[benchmark]
  fn submit_signed_proposal() {
    let proposer: T::AccountId = whitelisted_caller();
    let (domain, payload_hash, payload) =
      T::BenchmarkHelper::prepare_primary_eligible_submitter(&proposer)
        .expect("benchmark helper must prepare a primary-eligible submitter and signed preimage");
    let payload = ProposalPayloadBytesOf::<T>::try_from(payload)
      .expect("benchmark payload must fit the Governance call bound");
    let item_id = benchmark_item_id::<T>(1);
    let occupancy = T::MaxActiveProposalsPerDomain::get().saturating_sub(1);
    seed_active_proposals::<T>(domain, occupancy);
    Pallet::<T>::prepare_payload_admission_witness(
      RawOrigin::Signed(proposer.clone()).into(),
      domain,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
      payload,
    )
    .expect("benchmark payload witness preparation must succeed");
    #[extrinsic_call]
    submit_signed_proposal(
      RawOrigin::Signed(proposer.clone()),
      domain,
      item_id,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    );
    assert!(ActiveProposals::<T>::contains_key(domain, item_id));
    assert_eq!(
      ProposalAuthorsByItem::<T>::get(domain, item_id),
      Some(proposer)
    );
  }

  #[benchmark]
  fn prepare_payload_admission_witness() {
    let proposer: T::AccountId = whitelisted_caller();
    let (domain, payload_kind, payload_hash, payload) =
      T::BenchmarkHelper::prepare_maximum_payload_witness(&proposer)
        .expect("benchmark helper must prepare the maximum valid signed payload preimage");
    let payload = ProposalPayloadBytesOf::<T>::try_from(payload)
      .expect("benchmark payload must fit the Governance call bound");
    #[extrinsic_call]
    prepare_payload_admission_witness(
      RawOrigin::Signed(proposer),
      domain,
      payload_kind,
      payload_hash,
      payload,
    );
    assert!(PayloadAdmissionWitnesses::<T>::contains_key(
      payload_hash,
      (domain, payload_kind),
    ));
  }

  #[benchmark]
  fn cast_vote() {
    let voter: T::AccountId = whitelisted_caller();
    let domain = T::BenchmarkHelper::prepare_protection_voter(&voter)
      .expect("benchmark helper must prepare a protection voter");
    let item_id = benchmark_item_id::<T>(1);
    let submitted_epoch: T::Epoch = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let current_epoch: T::Epoch = 1u32.into();
    ActiveProposals::<T>::insert(domain, item_id, ActiveProposal { submitted_epoch });
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
    );
    ActiveProposalIdsByDomain::<T>::insert(
      domain,
      BoundedVec::try_from(alloc::vec![item_id]).expect("benchmark active proposal index fits"),
    );
    let mut votes = ProposalVotes {
      ayes: BoundedVec::default(),
      nays: BoundedVec::default(),
      amplifies: BoundedVec::default(),
      approves: BoundedVec::default(),
      reduces: BoundedVec::default(),
      vetoes: BoundedVec::default(),
      passes: BoundedVec::default(),
    };
    for index in 0..T::MaxWinningVoteAccountsPerCall::get().saturating_sub(1) {
      votes
        .vetoes
        .try_push(ProposalBallot {
          account: account("prior-veto", index, 0),
          vote_epoch: current_epoch,
          weight: 1,
          raw_power: 1,
        })
        .expect("benchmark veto set must fit configured maximum");
    }
    ProposalVotesByItem::<T>::insert(domain, item_id, votes);
    let retention_epoch: T::Epoch = current_epoch_u32::<T>(current_epoch)
      .saturating_add(T::FinalizedProposalOutcomeRetentionEpochs::get())
      .into();
    let occupancy = T::MaxFinalizedProposalOutcomesPerEpoch::get().saturating_sub(1);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(FinalizedProposalTouch {
        domain,
        item_id: benchmark_item_id::<T>(10_000u32.saturating_add(index)),
      });
      if push_result.is_err() {
        panic!("benchmark finalized outcome bucket seed must fit configured max")
      }
    }
    FinalizedProposalOutcomeExpiryBuckets::<T>::insert(retention_epoch, bucket);
    #[extrinsic_call]
    cast_vote(
      RawOrigin::Signed(voter.clone()),
      domain,
      item_id,
      ProposalVoteKind::Veto,
    );
    assert!(!ActiveProposals::<T>::contains_key(domain, item_id));
    assert!(FinalizedProposals::<T>::contains_key(domain, item_id));
  }

  #[benchmark]
  fn unlock_vote_power() {
    let caller: T::AccountId = whitelisted_caller();
    let (lock_id, amount) = T::BenchmarkHelper::prepare_vote_power_custody(&caller)
      .expect("runtime must prepare transferable vote-power custody");
    let current_epoch = T::EpochProvider::current_epoch();
    VotePowerCustodyByAccount::<T>::insert(
      &caller,
      lock_id,
      VotePowerCustodyPosition {
        amount,
        lock_until: current_epoch,
      },
    );
    #[extrinsic_call]
    unlock_vote_power(RawOrigin::Signed(caller.clone()), lock_id);
    assert!(!VotePowerCustodyByAccount::<T>::contains_key(
      caller, lock_id
    ));
  }

  #[benchmark]
  fn resolve_proposal(n: Linear<1, 256>) {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let current_epoch = T::EpochProvider::current_epoch();
    ActiveProposals::<T>::insert(
      domain,
      item_id,
      ActiveProposal {
        submitted_epoch: current_epoch,
      },
    );
    ProposalAuthorsByItem::<T>::insert(domain, item_id, account::<T::AccountId>("proposer", 0, 0));
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
    );
    ActiveProposalIdsByDomain::<T>::insert(
      domain,
      BoundedVec::try_from(alloc::vec![item_id]).expect("benchmark active proposal index fits"),
    );
    let expiry_epoch = expiry_epoch::<T>(current_epoch);
    let occupancy = T::MaxExpiringAccountsPerEpoch::get().saturating_sub(n);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: account("expiring-account", index, 0),
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(expiry_epoch, bucket);
    let mut winners = BoundedVec::default();
    for index in 0..n {
      let push_result = winners.try_push(account("winner", index, 0));
      if push_result.is_err() {
        panic!("benchmark proposal winner batch must fit configured max")
      }
    }
    #[extrinsic_call]
    resolve_proposal(RawOrigin::Root, domain, item_id, winners);
    assert!(!ActiveProposals::<T>::contains_key(domain, item_id));
  }

  #[benchmark]
  fn resolve_proposal_from_votes(n: Linear<1, 256>) {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let current_epoch_u32 = T::ProposalLeadInPeriod::get()
      .saturated_into::<u32>()
      .saturating_add(T::ProposalVotingPeriod::get().saturated_into::<u32>())
      .saturating_add(1);
    frame_system::Pallet::<T>::set_block_number(current_epoch_u32.into());
    let current_epoch: T::Epoch = current_epoch_u32.into();
    let expiry_epoch = expiry_epoch::<T>(current_epoch);
    let occupancy = T::MaxExpiringAccountsPerEpoch::get().saturating_sub(n);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: account("expiring-account", index, 0),
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(expiry_epoch, bucket);
    seed_proposal_votes::<T>(domain, item_id, 1u32.into(), 0, n, n.saturating_sub(1));
    #[extrinsic_call]
    resolve_proposal_from_votes(RawOrigin::Root, domain, item_id);
    assert!(!ActiveProposals::<T>::contains_key(domain, item_id));
  }

  #[benchmark]
  fn reject_proposal() {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let current_epoch = T::EpochProvider::current_epoch();
    ActiveProposals::<T>::insert(
      domain,
      item_id,
      ActiveProposal {
        submitted_epoch: current_epoch,
      },
    );
    ProposalAuthorsByItem::<T>::insert(domain, item_id, account::<T::AccountId>("proposer", 0, 0));
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
    );
    ActiveProposalIdsByDomain::<T>::insert(
      domain,
      BoundedVec::try_from(alloc::vec![item_id]).expect("benchmark active proposal index fits"),
    );
    #[extrinsic_call]
    reject_proposal(RawOrigin::Root, domain, item_id);
    assert!(!ActiveProposals::<T>::contains_key(domain, item_id));
  }

  #[benchmark]
  fn force_resolve_proposal_from_votes(n: Linear<1, 256>) {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let current_epoch = T::EpochProvider::current_epoch();
    let expiry_epoch = expiry_epoch::<T>(current_epoch);
    let occupancy = T::MaxExpiringAccountsPerEpoch::get().saturating_sub(n);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: account("expiring-account", index, 0),
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(expiry_epoch, bucket);
    seed_proposal_votes::<T>(domain, item_id, current_epoch, 0, n, n.saturating_sub(1));
    #[extrinsic_call]
    force_resolve_proposal_from_votes(RawOrigin::Root, domain, item_id);
    assert!(!ActiveProposals::<T>::contains_key(domain, item_id));
  }

  #[benchmark]
  fn requeue_proposal_for_auto_finalization() {
    let domain = benchmark_domain_id::<T>();
    let item_id = benchmark_item_id::<T>(1);
    let submitted_epoch_u32 = 1u32;
    let maturity_epoch_u32 = submitted_epoch_u32
      .saturating_add(T::ProposalLeadInPeriod::get().saturated_into::<u32>())
      .saturating_add(T::ProposalVotingPeriod::get().saturated_into::<u32>());
    let current_epoch_u32 = maturity_epoch_u32.saturating_sub(1);
    frame_system::Pallet::<T>::set_block_number(current_epoch_u32.into());
    ActiveProposals::<T>::insert(
      domain,
      item_id,
      ActiveProposal {
        submitted_epoch: submitted_epoch_u32.into(),
      },
    );
    ProposalMetadataByItem::<T>::insert(
      domain,
      item_id,
      benchmark_proposal_metadata::<T>(ProposalPayloadKind::L2ParameterChange),
    );
    ActiveProposalIdsByDomain::<T>::insert(
      domain,
      BoundedVec::try_from(alloc::vec![item_id]).expect("benchmark active proposal index fits"),
    );
    let maturity_epoch: T::Epoch = maturity_epoch_u32.into();
    let occupancy = T::MaxMaturingProposalsPerEpoch::get().saturating_sub(1);
    let mut bucket = BoundedVec::default();
    for index in 0..occupancy {
      let push_result = bucket.try_push(MaturingProposalTouch {
        domain,
        item_id: benchmark_item_id::<T>(10u32.saturating_add(index)),
      });
      if push_result.is_err() {
        panic!("benchmark proposal maturity bucket must fit configured max")
      }
    }
    ProposalMaturityBuckets::<T>::insert(maturity_epoch, bucket);
    #[extrinsic_call]
    requeue_proposal_for_auto_finalization(RawOrigin::Root, domain, item_id);
    assert!(
      ProposalMaturityBuckets::<T>::get(maturity_epoch)
        .iter()
        .any(|entry| entry.domain == domain && entry.item_id == item_id)
    );
  }

  #[benchmark]
  fn service_epoch_catch_up() {
    let current_epoch: T::Epoch = 1u32.into();
    LastProcessedEpoch::<T>::put(T::Epoch::zero());
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      let _ = Pallet::<T>::service_current_epoch(current_epoch);
    }
    assert_eq!(LastProcessedEpoch::<T>::get(), current_epoch);
  }

  #[benchmark]
  fn service_maturing_proposals(n: Linear<1, 3>) {
    let domain = benchmark_domain_id::<T>();
    let current_epoch_u32 = T::ProposalLeadInPeriod::get()
      .saturated_into::<u32>()
      .saturating_add(T::ProposalVotingPeriod::get().saturated_into::<u32>())
      .saturating_add(1);
    let current_epoch: T::Epoch = current_epoch_u32.into();
    seed_maturing_proposals::<T>(domain, current_epoch, n);
    let previous_epoch: T::Epoch = current_epoch_u32.saturating_sub(1).into();
    LastProcessedEpoch::<T>::put(previous_epoch);
    frame_system::Pallet::<T>::set_block_number(current_epoch_u32.into());
    #[block]
    {
      let _ =
        Pallet::<T>::service_maturing_proposals(current_epoch_u32.saturating_sub(1), current_epoch);
    }
    for index in 0..n {
      assert!(!ActiveProposals::<T>::contains_key(
        domain,
        benchmark_item_id::<T>(index.saturating_add(1))
      ));
    }
  }
  #[benchmark]
  fn service_pending_enactments(n: Linear<1, 4>) -> Result<(), BenchmarkError> {
    let current_epoch = Pallet::<T>::add_epochs(1u32.into(), T::ProposalEnactmentDelay::get())?;
    let current_block = current_epoch_u32::<T>(current_epoch);
    if T::Epoch::from(current_block) != current_epoch {
      return Err(BenchmarkError::Skip);
    }
    let mut identities = Vec::new();
    for index in 0..n {
      let proposer = account("pending", index, 0);
      let (domain, payload_kind, payload_hash) =
        T::BenchmarkHelper::prepare_pending_enactment(&proposer)
          .expect("benchmark pending payload must be prepared by the host");
      assert!(!matches!(
        payload_kind,
        ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1
      ));
      assert!(T::ProposalPayloadExecutor::can_execute(payload_kind));
      assert!(T::ProposalPayloadPreimageProvider::have_preimage(
        &payload_hash
      ));
      let item_id = benchmark_item_id::<T>(index);
      identities.push((domain, item_id));
      ProposalMetadataByItem::<T>::insert(
        domain,
        item_id,
        ProposalMetadata {
          cadence_mode: ProposalCadenceMode::Ordinary,
          payload_kind,
          payload_hash,
        },
      );
      Pallet::<T>::record_finalized_proposal_outcome(
        domain,
        item_id,
        FinalizedProposalOutcome::Approved {
          approval: ProposalApproval {
            approved_epoch: 1u32.into(),
            winner_count: 1,
          },
          enactment: ProposalEnactmentOutcome::NotAttempted,
        },
        1u32.into(),
      )
      .expect("benchmark discovery capacity is available");
      Pallet::<T>::admit_pending_enactment(domain, item_id, current_epoch)
        .expect("benchmark global pending capacity is available");
    }
    frame_system::Pallet::<T>::set_block_number(current_block.into());
    #[block]
    {
      let _ = Pallet::<T>::service_pending_enactments(current_epoch);
    }
    assert!(PendingEnactments::<T>::get().is_empty());
    for (domain, item_id) in identities {
      let finalized = FinalizedProposals::<T>::get(domain, item_id)
        .expect("benchmark execution retains the finalized proposal");
      assert_eq!(
        finalized.outcome,
        FinalizedProposalOutcome::Approved {
          approval: ProposalApproval {
            approved_epoch: 1u32.into(),
            winner_count: 1
          },
          enactment: ProposalEnactmentOutcome::Enacted {
            epoch: current_epoch
          },
        },
      );
      assert!(matches!(
        finalized.execution_detail,
        Some(ProposalExecutionDetail::Succeeded(_))
      ));
      assert!(!ProposalPendingEnactmentAt::<T>::contains_key(
        domain, item_id
      ));
    }
    Ok(())
  }

  #[benchmark(extra)]
  fn service_pending_enactments_full_topology(f: Linear<0, 1>) -> Result<(), BenchmarkError> {
    let current_epoch = Pallet::<T>::add_epochs(1u32.into(), T::ProposalEnactmentDelay::get())?;
    let verify_pending_topology =
      prepare_full_pending_enactment_topology::<T>(current_epoch, current_epoch, f == 1, None)?;
    #[block]
    {
      let _ = Pallet::<T>::service_pending_enactments(current_epoch);
    }
    verify_pending_topology(&[])?;
    Ok(())
  }

  #[cfg(feature = "std")]
  impl<T: Config> Pallet<T>
  where
    T::DomainId: From<u32>,
    T::WinningVoteItemId: From<u32>,
    T::Epoch: From<u32>,
    BlockNumberFor<T>: From<u32>,
  {
    /// Verify pending setup and execution without timing or ProofSize recording.
    /// None selects the simple profile; Some selects full topology with cleanup refusal.
    pub fn verify_pending_enactment_benchmark(
      cleanup_refusal: Option<bool>,
      before_service: &dyn Fn(),
    ) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      if let Some(refused) = cleanup_refusal {
        return <service_pending_enactments_full_topology as BenchmarkingSetup<T>>::test_instance(
          &service_pending_enactments_full_topology,
          &[(BenchmarkParameter::f, u32::from(refused))],
          before_service,
        );
      }
      let n = T::MaxPendingEnactmentsPerBlock::get();
      let components = <service_pending_enactments as BenchmarkingSetup<T>>::components(
        &service_pending_enactments,
      );
      if !components.iter().any(|(parameter, low, high)| {
        *parameter == BenchmarkParameter::n && n >= *low && n <= *high
      }) {
        return Err(BenchmarkError::Stop(
          "host pending maximum is outside benchmark component bounds",
        ));
      }
      <service_pending_enactments as BenchmarkingSetup<T>>::test_instance(
        &service_pending_enactments,
        &[(BenchmarkParameter::n, n)],
        before_service,
      )
    }

    /// Prepare without dispatching. The returned pending-only verifier requires no unrelated
    /// state changes when checking a refused destination's exact root (apart from its cursor).
    /// None reuses the host's default helper. Some supplies an already-noted executable
    /// payload and optional primary winner, without invoking that default helper.
    pub fn prepare_full_pending_enactment_hook(
      current_epoch: T::Epoch,
      cleanup_refusal: bool,
      payload_source: Option<(
        T::DomainId,
        ProposalPayloadKind,
        T::Hash,
        Option<ProposalPrimaryTrackOption>,
      )>,
    ) -> Result<
      impl FnOnce(
        &[(T::WinningVoteItemId, ProposalExecutionFailureReason)],
      ) -> Result<(), BenchmarkError>,
      BenchmarkError,
    > {
      prepare_full_pending_enactment_topology::<T>(
        current_epoch,
        current_epoch,
        cleanup_refusal,
        payload_source,
      )
    }

    /// Prepare a predecessor-clock source for a child hook. This does not construct a header,
    /// public history, full runtime prefix or PVF input; the same-epoch diagnostic stays separate.
    pub fn prepare_full_pending_enactment_parent(
      current_epoch: T::Epoch,
      payload_source: Option<(
        T::DomainId,
        ProposalPayloadKind,
        T::Hash,
        Option<ProposalPrimaryTrackOption>,
      )>,
    ) -> Result<
      impl FnOnce(
        &[(T::WinningVoteItemId, ProposalExecutionFailureReason)],
      ) -> Result<(), BenchmarkError>,
      BenchmarkError,
    > {
      let child_block = current_epoch_u32::<T>(current_epoch);
      if T::Epoch::from(child_block) != current_epoch || child_block == 0 {
        return Err(BenchmarkError::Skip);
      }
      let parent_epoch = T::Epoch::from(child_block - 1);
      prepare_full_pending_enactment_topology::<T>(
        current_epoch,
        parent_epoch,
        false,
        payload_source,
      )
    }

    /// Reuse the bounded benchmark state for a host's complete initialization-hook witness.
    pub fn prepare_maximum_finalized_cleanup_hook()
    -> Result<(T::DomainId, T::Epoch), BenchmarkError> {
      seed_maximum_finalized_outcomes::<T>(T::MaxFinalizedProposalOutcomesPerBlock::get())
    }

    /// Verify the full hook's finite empty-pending source without collecting physical samples.
    pub fn verify_complete_finalized_cleanup_benchmark(
      n: u32,
      phase: u32,
    ) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      <on_initialize_finalized_cleanup as BenchmarkingSetup<T>>::unit_test_instance(
        &on_initialize_finalized_cleanup,
        &[(BenchmarkParameter::n, n), (BenchmarkParameter::p, phase)],
      )
    }

    /// Run the extra's generated v2 boundary with caller-owned recording.
    pub fn verify_full_index_value_classes_benchmark(
      q: u32,
      recording: &mut impl polkadot_sdk::frame_benchmarking::Recording,
    ) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      <on_initialize_full_index_value_classes as BenchmarkingSetup<T>>::instance(
        &on_initialize_full_index_value_classes,
        recording,
        &[(BenchmarkParameter::q, q)],
        true,
      )
    }

    /// Prepare two complete indices at a consistent service clock/debt; no physical sampling.
    pub fn prepare_independent_full_indices_cleanup_hook() -> Result<
      (
        T::Epoch,
        T::Epoch,
        impl FnOnce() -> Result<(), BenchmarkError>,
      ),
      BenchmarkError,
    > {
      Self::prepare_independent_full_indices_value_contrast(false)
    }

    /// Native source contrast only; true changes one last peer ID and its lifecycle data.
    pub fn prepare_independent_full_indices_value_contrast(
      distinct_values: bool,
    ) -> Result<
      (
        T::Epoch,
        T::Epoch,
        impl FnOnce() -> Result<(), BenchmarkError>,
      ),
      BenchmarkError,
    > {
      prepare_full_index_value_classes::<T>(distinct_values)
    }

    /// Prepare before other components. The verifier covers only cleanup-owned state;
    /// the caller independently verifies the composed pending pass and its effects.
    pub fn prepare_independent_full_indices_cleanup_component() -> Result<
      (
        T::Epoch,
        T::Epoch,
        impl FnOnce() -> Result<(), BenchmarkError>,
      ),
      BenchmarkError,
    > {
      prepare_independent_full_indices_cleanup::<T>(false)
    }

    /// Verify complete-hook domain diversity without collecting physical samples.
    pub fn verify_complete_diverse_finalized_cleanup_benchmark(
      n: u32,
      phase: u32,
    ) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      <on_initialize_finalized_cleanup_diverse as BenchmarkingSetup<T>>::unit_test_instance(
        &on_initialize_finalized_cleanup_diverse,
        &[(BenchmarkParameter::n, n), (BenchmarkParameter::p, phase)],
      )
    }

    /// Verify a finite distinct-domain source without collecting physical samples.
    pub fn verify_diverse_finalized_cleanup_benchmark(n: u32) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      <service_finalized_proposal_outcomes_diverse as BenchmarkingSetup<T>>::unit_test_instance(
        &service_finalized_proposal_outcomes_diverse,
        &[(BenchmarkParameter::n, n)],
      )
    }

    /// Verify this host's configured maximum with caller-owned SDK boundary recording.
    pub fn verify_maximum_finalized_cleanup_benchmark(
      recording: &mut impl polkadot_sdk::frame_benchmarking::Recording,
    ) -> Result<(), BenchmarkError> {
      use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
      let n = T::MaxFinalizedProposalOutcomesPerBlock::get();
      let components = <service_finalized_proposal_outcomes as BenchmarkingSetup<T>>::components(
        &service_finalized_proposal_outcomes,
      );
      if !components.iter().any(|(parameter, low, high)| {
        *parameter == BenchmarkParameter::n && n >= *low && n <= *high
      }) {
        return Err(BenchmarkError::Stop(
          "host cleanup maximum is outside benchmark component bounds",
        ));
      }
      <service_finalized_proposal_outcomes as BenchmarkingSetup<T>>::instance(
        &service_finalized_proposal_outcomes,
        recording,
        &[(BenchmarkParameter::n, n)],
        true,
      )
    }
  }

  #[benchmark]
  fn service_finalized_proposal_outcomes(n: Linear<1, 1024>) -> Result<(), BenchmarkError> {
    let (current_epoch, check_cleanup) = prepare_finalized_cleanup::<T>(n)?;
    let current_epoch_u32 = current_epoch_u32::<T>(current_epoch);
    #[block]
    {
      let _ = Pallet::<T>::service_finalized_proposal_outcomes(
        current_epoch_u32.saturating_sub(1),
        current_epoch,
      );
    }
    // V2 places the Result tail outside optional verification; only return Ok there.
    check_cleanup()?;
    Ok(())
  }

  #[benchmark(extra)]
  fn on_initialize_finalized_cleanup(
    n: Linear<1, 1024>,
    p: Linear<0, 1>,
  ) -> Result<(), BenchmarkError> {
    let (epoch, check_cleanup) = prepare_finalized_cleanup::<T>(n)?;
    if T::EpochProvider::current_epoch() != epoch {
      return Err(BenchmarkError::Skip);
    }
    assert!(PendingEnactments::<T>::get().is_empty());
    assert!(ProposalMaturityBuckets::<T>::get(epoch).is_empty());
    CurrentEpochServicePhase::<T>::put(if p == 0 {
      EpochServicePhase::Maturing
    } else {
      EpochServicePhase::FinalizedOutcome
    });
    let number = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      let _ = <Pallet<T> as Hooks<BlockNumberFor<T>>>::on_initialize(number);
    }
    check_cleanup()?;
    assert_eq!(
      CurrentEpochServicePhase::<T>::get(),
      EpochServicePhase::RewardExpiry
    );
    assert_eq!(
      LastProcessedEpoch::<T>::get(),
      T::Epoch::from(current_epoch_u32::<T>(epoch) - 1),
    );
    Ok(())
  }

  #[benchmark(extra)]
  fn on_initialize_full_index_value_classes(q: Linear<0, 1>) -> Result<(), BenchmarkError> {
    let (_, _, check_hook) = prepare_full_index_value_classes::<T>(q == 1)?;
    // Absolute body-proof diagnostic: do not prefill or discount any original key.
    polkadot_sdk::frame_benchmarking::benchmarking::set_whitelist(Vec::new());
    let number = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      let _ = <Pallet<T> as Hooks<BlockNumberFor<T>>>::on_initialize(number);
    }
    check_hook()?;
    Ok(())
  }

  #[benchmark(extra)]
  fn service_finalized_proposal_outcomes_diverse(n: Linear<2, 1024>) -> Result<(), BenchmarkError> {
    let (expiry, check_cleanup) = prepare_diverse_finalized_cleanup::<T>(n)?;
    let now = current_epoch_u32::<T>(expiry);
    #[block]
    {
      let _ = Pallet::<T>::service_finalized_proposal_outcomes(now - 1, expiry);
    }
    check_cleanup()?;
    Ok(())
  }

  #[benchmark(extra)]
  fn on_initialize_finalized_cleanup_diverse(
    n: Linear<2, 1024>,
    p: Linear<0, 1>,
  ) -> Result<(), BenchmarkError> {
    let (epoch, check_cleanup) = prepare_diverse_finalized_cleanup::<T>(n)?;
    if T::EpochProvider::current_epoch() != epoch {
      return Err(BenchmarkError::Skip);
    }
    assert!(PendingEnactments::<T>::get().is_empty());
    assert!(ProposalMaturityBuckets::<T>::get(epoch).is_empty());
    CurrentEpochServicePhase::<T>::put(if p == 0 {
      EpochServicePhase::Maturing
    } else {
      EpochServicePhase::FinalizedOutcome
    });
    let number = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      let _ = <Pallet<T> as Hooks<BlockNumberFor<T>>>::on_initialize(number);
    }
    check_cleanup()?;
    assert_eq!(
      CurrentEpochServicePhase::<T>::get(),
      EpochServicePhase::RewardExpiry
    );
    assert_eq!(
      LastProcessedEpoch::<T>::get(),
      T::Epoch::from(current_epoch_u32::<T>(epoch) - 1)
    );
    Ok(())
  }

  #[benchmark]
  fn service_expiring_accounts(n: Linear<1, 512>) {
    let domain = benchmark_domain_id::<T>();
    let current_epoch_u32 = T::WinningVoteLookbackEpochs::get().saturating_add(1);
    let current_epoch: T::Epoch = current_epoch_u32.into();
    let vote_epoch: T::Epoch = 1u32.into();
    let mut bucket = BoundedVec::default();
    for index in 0..n {
      let voter: T::AccountId = account("voter", index, 0);
      seed_window::<T>(domain, &voter, vote_epoch, 1);
      let push_result = bucket.try_push(ExpiringAccountTouch {
        domain,
        account: voter,
      });
      if push_result.is_err() {
        panic!("benchmark expiry bucket seed must fit configured max")
      }
    }
    ExpiryBuckets::<T>::insert(current_epoch, bucket);
    let previous_epoch: T::Epoch = current_epoch_u32.saturating_sub(1).into();
    LastProcessedEpoch::<T>::put(previous_epoch);
    frame_system::Pallet::<T>::set_block_number(current_epoch_u32.into());
    #[block]
    {
      let _ =
        Pallet::<T>::service_expiring_accounts(current_epoch_u32.saturating_sub(1), current_epoch);
    }
    for index in 0..n {
      assert!(!WinningVoteWindows::<T>::contains_key(
        domain,
        account::<T::AccountId>("voter", index, 0)
      ));
    }
  }

  #[cfg(test)]
  mod verification_switch_tests {
    use super::*;
    use crate::mock::{Test, new_test_ext};
    use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup, Recording};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Default)]
    struct FaultAfterStop {
      starts: u32,
      stops: u32,
      faulted: bool,
    }
    impl Recording for FaultAfterStop {
      fn start(&mut self) {
        self.starts += 1;
      }
      fn stop(&mut self) {
        self.stops += 1;
        let domain = FinalizedProposalIdsByDomain::<Test>::iter_keys()
          .next()
          .unwrap();
        FinalizedProposalIdsByDomain::<Test>::mutate(domain, |ids| {
          assert!(!ids.is_empty());
          ids.remove(0);
        });
        self.faulted = true;
      }
    }

    fn assert_switch(
      benchmark: &impl BenchmarkingSetup<Test>,
      components: &[(BenchmarkParameter, u32)],
    ) {
      for verify in [false, true] {
        new_test_ext().execute_with(|| {
          let mut recording = FaultAfterStop::default();
          let result = catch_unwind(AssertUnwindSafe(|| {
            benchmark.instance(&mut recording, components, verify)
          }));
          assert_eq!(
            (recording.starts, recording.stops, recording.faulted),
            (1, 1, true)
          );
          if verify {
            assert!(
              result.is_err(),
              "verification must reject post-stop index corruption"
            );
          } else {
            result
              .expect("verify=false must skip the post-stop verifier")
              .unwrap();
          }
        });
      }
    }

    #[test]
    fn cleanup_helper_respects_verification_switch() {
      assert_switch(
        &service_finalized_proposal_outcomes,
        &[(BenchmarkParameter::n, 1)],
      );
    }

    #[test]
    fn diverse_cleanup_helper_respects_verification_switch() {
      assert_switch(
        &service_finalized_proposal_outcomes_diverse,
        &[(BenchmarkParameter::n, 2)],
      );
    }

    #[test]
    fn full_index_value_classes_respect_verification_switch() {
      for q in [0, 1] {
        assert_switch(
          &on_initialize_full_index_value_classes,
          &[(BenchmarkParameter::q, q)],
        );
      }
    }
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}

#[cfg(test)]
mod fixture_tests {
  use super::*;
  use crate::mock::{Test, new_test_ext};

  #[test]
  fn pending_parent_source_binds_predecessor_dates_and_distinct_deadlines() {
    for (child, delay, accepted) in [
      (0u64, 2u64, false),
      (1, 0, false),
      (2, 1, false),
      (3, 2, true),
    ] {
      new_test_ext().execute_with(|| {
        crate::mock::ProposalEnactmentDelay::set(delay);
        let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let before = root();
        let result = Pallet::<Test>::prepare_full_pending_enactment_parent(child, None);
        if !accepted {
          assert!(matches!(result, Err(BenchmarkError::Skip)));
          assert_eq!(root(), before);
          return;
        }
        let verify_pending_topology = result.unwrap();
        let parent = child - 1;
        assert_eq!(frame_system::Pallet::<Test>::block_number(), parent);
        let retention = u64::from(<<Test as Config>::FinalizedProposalOutcomeRetentionEpochs as Get<u32>>::get());
        let peers = FinalizedProposalOutcomeExpiryBuckets::<Test>::get(parent + retention);
        assert!(!peers.is_empty());
        for touch in &peers {
          assert!(matches!(FinalizedProposals::<Test>::get(touch.domain, touch.item_id).unwrap().outcome, FinalizedProposalOutcome::Rejected { finalized_epoch, .. } if finalized_epoch == parent));
        }
        assert!(FinalizedProposalOutcomeExpiryBuckets::<Test>::get(child + retention).is_empty());
        for touch in PendingEnactments::<Test>::get() {
          let record = FinalizedProposals::<Test>::get(touch.domain, touch.item_id).unwrap();
          assert!(matches!(record.outcome, FinalizedProposalOutcome::Approved { approval, .. } if approval.approved_epoch <= parent));
        }
        frame_system::Pallet::<Test>::set_block_number(child);
        Pallet::<Test>::service_pending_enactments(child);
        verify_pending_topology(&[]).unwrap();
        assert_eq!(FinalizedProposalOutcomeExpiryBuckets::<Test>::get(parent + retention), peers);
        assert_eq!(FinalizedProposalOutcomeExpiryBuckets::<Test>::get(child + retention).len(), <<Test as Config>::MaxPendingEnactmentsPerBlock as Get<u32>>::get() as usize);
      });
    }
  }

  #[test]
  fn pending_failure_expectations_preserve_default_success_and_exact_causes() {
    for mode in 0..3 {
      new_test_ext().execute_with(|| {
        crate::mock::ProposalEnactmentDelay::set(2);
        let verify_pending_topology =
          Pallet::<Test>::prepare_full_pending_enactment_hook(3, false, None).unwrap();
        let visits = <<Test as Config>::MaxPendingEnactmentsPerBlock as Get<u32>>::get() as usize;
        let members = PendingEnactments::<Test>::get();
        let due = &members[members.len() - visits..];
        for touch in due {
          let metadata = ProposalMetadataByItem::<Test>::get(touch.domain, touch.item_id).unwrap();
          crate::mock::set_payload_execution_result(metadata.payload_hash, false);
        }
        Pallet::<Test>::service_pending_enactments(3);
        let reason = if mode == 1 {
          ProposalExecutionFailureReason::MissingPreimage
        } else {
          ProposalExecutionFailureReason::DispatchFailed
        };
        let expected = if mode == 0 {
          Vec::new()
        } else {
          due.iter().map(|touch| (touch.item_id, reason)).collect()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          verify_pending_topology(&expected)
        }));
        assert_eq!(
          matches!(result, Ok(Ok(()))),
          mode == 2,
          "only exact explicitly expected failures may pass; an empty map still requires success"
        );
      });
    }
  }

  #[test]
  fn pending_full_topology_requires_reachable_future_approval() {
    for delay in [0u64, 1] {
      for refused in [false, true] {
        new_test_ext().execute_with(|| {
          crate::mock::ProposalEnactmentDelay::set(delay);
          let root =
            || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          let before = root();
          let observer = || {
            let now = frame_system::Pallet::<Test>::block_number();
            assert_eq!(now, 1 + delay);
            for member in PendingEnactments::<Test>::get() {
              let record = FinalizedProposals::<Test>::get(member.domain, member.item_id).unwrap();
              let FinalizedProposalOutcome::Approved { approval, .. } = record.outcome else {
                panic!("fixture requires approval")
              };
              assert!(approval.approved_epoch <= now);
              assert_eq!(
                ProposalPendingEnactmentAt::<Test>::get(member.domain, member.item_id),
                Some(approval.approved_epoch + delay)
              );
            }
          };
          let result = Pallet::<Test>::verify_pending_enactment_benchmark(Some(refused), &observer);
          if delay == 0 {
            assert!(matches!(result, Err(BenchmarkError::Skip)));
            assert_eq!(root(), before);
          } else {
            result.unwrap();
          }
        });
      }
    }
  }

  #[test]
  fn complete_cleanup_fixture_verifies_both_hook_entry_phases() {
    for phase in 0..=1 {
      for n in [
        1,
        <<Test as Config>::MaxFinalizedProposalOutcomesPerBlock as Get<u32>>::get(),
      ] {
        new_test_ext().execute_with(|| {
          Pallet::<Test>::verify_complete_finalized_cleanup_benchmark(n, phase).unwrap();
        });
      }
    }
  }

  #[test]
  fn diverse_cleanup_fixture_visits_each_due_domain() {
    new_test_ext().execute_with(|| {
      let n = 3;
      let (expiry, total) = seed_diverse_finalized_outcomes::<Test>(n).unwrap();
      let due = FinalizedProposalOutcomeExpiryBuckets::<Test>::get(expiry);
      let domains = due
        .iter()
        .map(|touch| touch.domain)
        .collect::<alloc::collections::BTreeSet<_>>();
      assert_eq!(domains.len(), n as usize);
      assert_eq!(due.len(), n as usize);
      assert_eq!(
        domains
          .iter()
          .map(|domain| FinalizedProposalIdsByDomain::<Test>::get(domain).len())
          .sum::<usize>(),
        total as usize
      );
      for touch in &due {
        assert!(FinalizedProposals::<Test>::contains_key(
          touch.domain,
          touch.item_id
        ));
        assert!(FinalizedProposalIdsByDomain::<Test>::get(touch.domain).contains(&touch.item_id));
      }
      Pallet::<Test>::service_finalized_proposal_outcomes(
        current_epoch_u32::<Test>(expiry) - 1,
        expiry,
      );
      for touch in due {
        assert!(!FinalizedProposals::<Test>::contains_key(
          touch.domain,
          touch.item_id
        ));
        assert!(!FinalizedProposalIdsByDomain::<Test>::get(touch.domain).contains(&touch.item_id));
      }
    });
  }

  #[test]
  fn independent_full_indices_cleanup_fixture_reconciles_both_entry_phases() {
    for phase in [
      EpochServicePhase::Maturing,
      EpochServicePhase::FinalizedOutcome,
    ] {
      new_test_ext().execute_with(|| {
        let (now, expiry, check_cleanup) =
          Pallet::<Test>::prepare_independent_full_indices_cleanup_hook().unwrap();
        assert!(now >= expiry);
        let first = FinalizedProposalIdsByDomain::<Test>::get(101);
        assert_eq!(first, FinalizedProposalIdsByDomain::<Test>::get(102));
        assert_eq!(
          first.len(),
          <<Test as Config>::MaxRecentFinalizedProposalsPerDomain as Get<u32>>::get() as usize
        );
        CurrentEpochServicePhase::<Test>::put(phase);
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().unwrap();
        let _ = <Pallet<Test> as Hooks<BlockNumberFor<Test>>>::on_initialize(
          frame_system::Pallet::<Test>::block_number(),
        );
        check_cleanup().unwrap();
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().unwrap();
      });
    }
  }

  #[test]
  fn full_index_value_classes_extra_verifies_both_cases() {
    for q in [0, 1] {
      new_test_ext().execute_with(|| {
        struct NoClock;
        impl polkadot_sdk::frame_benchmarking::Recording for NoClock {}
        Pallet::<Test>::verify_full_index_value_classes_benchmark(q, &mut NoClock).unwrap();
      });
    }
  }

  #[test]
  fn independent_full_indices_distinct_values_reconcile_hook() {
    new_test_ext().execute_with(|| {
      let (_, _, check_hook) =
        Pallet::<Test>::prepare_independent_full_indices_value_contrast(true).unwrap();
      assert_ne!(
        FinalizedProposalIdsByDomain::<Test>::get(101),
        FinalizedProposalIdsByDomain::<Test>::get(102)
      );
      #[cfg(feature = "try-runtime")]
      Pallet::<Test>::do_try_state().unwrap();
      let _ = <Pallet<Test> as Hooks<BlockNumberFor<Test>>>::on_initialize(frame_system::Pallet::<
        Test,
      >::block_number());
      check_hook().unwrap();
      #[cfg(feature = "try-runtime")]
      Pallet::<Test>::do_try_state().unwrap();
    });
  }

  #[test]
  fn independent_full_indices_hook_verifier_rejects_pending_cursor_mutation() {
    new_test_ext().execute_with(|| {
      let (_, _, check_hook) =
        Pallet::<Test>::prepare_independent_full_indices_cleanup_hook().unwrap();
      let _ = <Pallet<Test> as Hooks<BlockNumberFor<Test>>>::on_initialize(frame_system::Pallet::<
        Test,
      >::block_number());
      PendingEnactmentCursor::<Test>::put(1);
      assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(check_hook)).is_err());
    });
  }

  #[test]
  fn complete_diverse_cleanup_fixture_verifies_both_hook_entry_phases() {
    for n in [2, 3] {
      for phase in 0..=1 {
        new_test_ext().execute_with(|| {
          Pallet::<Test>::verify_complete_diverse_finalized_cleanup_benchmark(n, phase).unwrap();
        });
      }
    }
  }

  #[test]
  fn finalized_cleanup_fixture_has_maximum_record() {
    new_test_ext().execute_with(|| {
      let (domain, epoch) = seed_maximum_finalized_outcomes::<Test>(1).unwrap();
      let record = FinalizedProposals::<Test>::get(domain, 2_000).unwrap();
      assert_eq!(
        record.encoded_size(),
        FinalizedProposalRecord::<
          crate::mock::AccountId,
          crate::mock::DomainId,
          polkadot_sdk::sp_core::H256,
          <Test as Config>::Epoch,
          CidBytesOf<Test>,
        >::max_encoded_len()
      );
      assert!(matches!(&record.execution_detail,
        Some(ProposalExecutionDetail::Succeeded(ProposalExecutionSuccessDetail::TreasurySpendExecuted { invoice_cid, target_amount: 200, actual_amount: 150, capacity_limited: true, .. }))
          if invoice_cid.len() == 96));
      assert_eq!(ProposalMetadataByItem::<Test>::get(domain, 2_000).unwrap().payload_kind, ProposalPayloadKind::L2TreasurySpend);
      let index = FinalizedProposalIdsByDomain::<Test>::get(domain);
      let peer = FinalizedProposals::<Test>::get(domain, index[1]);
      let future = FinalizedProposalOutcomeExpiryBuckets::<Test>::get(epoch + 1);
      assert!(!future.is_empty());
      let pending = PendingEnactments::<Test>::get();
      let cursor = PendingEnactmentCursor::<Test>::get();
      Pallet::<Test>::service_finalized_proposal_outcomes(current_epoch_u32::<Test>(epoch) - 1, epoch);
      assert!(!FinalizedProposals::<Test>::contains_key(domain, 2_000));
      assert!(!ProposalMetadataByItem::<Test>::contains_key(domain, 2_000));
      assert!(!ProposalWinningPrimaryOptionByItem::<Test>::contains_key(domain, 2_000));
      assert_eq!(ProposalWinningPrimaryOptionByItem::<Test>::get(domain, index[1]), Some(ProposalPrimaryTrackOption::Amplify));
      assert_eq!(FinalizedProposalIdsByDomain::<Test>::get(domain).as_slice(), &index[1..]);
      assert_eq!(FinalizedProposals::<Test>::get(domain, index[1]), peer);
      assert_eq!(FinalizedProposalOutcomeExpiryBuckets::<Test>::get(epoch + 1), future);
      assert_eq!(PendingEnactments::<Test>::get(), pending);
      assert_eq!(PendingEnactmentCursor::<Test>::get(), cursor);
    });
  }
}
