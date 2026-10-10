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

fn current_epoch_u32<T: Config>(epoch: T::Epoch) -> u32 {
  epoch.saturated_into::<u32>()
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
    let current_block = current_epoch_u32::<T>(current_epoch);
    if T::Epoch::from(current_block) != current_epoch {
      return Err(BenchmarkError::Skip);
    }
    let visits = T::MaxPendingEnactmentsPerBlock::get();
    let pending_capacity = T::MaxPendingEnactments::get();
    let cleanup_capacity = T::MaxFinalizedProposalOutcomesPerEpoch::get();
    if visits == 0 || visits > pending_capacity || visits > cleanup_capacity {
      return Err(BenchmarkError::Skip);
    }
    let first_due = pending_capacity - visits;
    // Future ordinary obligations require an already-approved source at this service epoch.
    if first_due > 0 && current_epoch < T::Epoch::from(2u32) {
      return Err(BenchmarkError::Skip);
    }
    let future_due = Pallet::<T>::add_epochs(current_epoch, 1u32.into())?;
    let proposer = account("pending-full", 0, 0);
    let (domain, payload_kind, payload_hash) =
      T::BenchmarkHelper::prepare_pending_enactment(&proposer)?;
    assert!(!matches!(
      payload_kind,
      ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1
    ));
    assert!(T::ProposalPayloadExecutor::can_execute(payload_kind));
    assert!(T::ProposalPayloadPreimageProvider::have_preimage(
      &payload_hash
    ));
    let peer_domain = benchmark_domain_id::<T>();
    let cleanup_peers = cleanup_capacity - if f == 0 { visits } else { 0 };
    let discovery_required = if peer_domain == domain {
      pending_capacity.checked_add(cleanup_peers)
    } else {
      Some(pending_capacity.max(cleanup_peers))
    };
    if discovery_required.is_none_or(|count| count > T::MaxRecentFinalizedProposalsPerDomain::get())
    {
      return Err(BenchmarkError::Skip);
    }
    let cleanup_epoch = Pallet::<T>::add_epochs(
      current_epoch,
      T::FinalizedProposalOutcomeRetentionEpochs::get().into(),
    )?;
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
          finalized_epoch: current_epoch,
          reason: ProposalRejectionReason::AdminRejected,
        },
        current_epoch,
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
      let approved_epoch: T::Epoch = if index >= first_due { 1u32 } else { 2u32 }.into();
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
    frame_system::Pallet::<T>::set_block_number(current_block.into());
    let pending_before = PendingEnactments::<T>::get();
    let peers_before = pending_before[..first_due as usize]
      .iter()
      .map(|touch| {
        (
          touch.clone(),
          FinalizedProposals::<T>::get(touch.domain, touch.item_id),
          ProposalMetadataByItem::<T>::get(touch.domain, touch.item_id),
          ProposalPendingEnactmentAt::<T>::get(touch.domain, touch.item_id),
        )
      })
      .collect::<Vec<_>>();
    let cleanup_before = FinalizedProposalOutcomeExpiryBuckets::<T>::get(cleanup_epoch);
    assert_eq!(pending_before.len(), pending_capacity as usize);
    assert_eq!(cleanup_before.len(), cleanup_peers as usize);
    // Normalize FRAME's level metadata as a prior public transaction would.
    let initialized_transaction: Result<(), DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      });
    initialized_transaction?;
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    #[block]
    {
      let _ = Pallet::<T>::service_pending_enactments(current_epoch);
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
        &cleanup_after[..cleanup_peers as usize],
        cleanup_before.as_slice()
      );
      assert_eq!(cleanup_after.len(), cleanup_capacity as usize);
    }
    for (touch, record, metadata, due) in peers_before {
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
    }
    for item_id in due_ids {
      let finalized = FinalizedProposals::<T>::get(domain, item_id)
        .expect("benchmark service retains the approval record");
      assert_eq!(
        finalized.outcome,
        FinalizedProposalOutcome::Approved {
          approval: ProposalApproval {
            approved_epoch: 1u32.into(),
            winner_count: 1
          },
          enactment: if f == 0 {
            ProposalEnactmentOutcome::Enacted {
              epoch: current_epoch,
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
        assert!(matches!(
          finalized.execution_detail,
          Some(ProposalExecutionDetail::Succeeded(_))
        ));
      } else {
        assert!(finalized.execution_detail.is_none());
      }
    }
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

    /// Verify this host's configured maximum without recording benchmark measurements.
    pub fn verify_maximum_finalized_cleanup_benchmark() -> Result<(), BenchmarkError> {
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
      <service_finalized_proposal_outcomes as BenchmarkingSetup<T>>::unit_test_instance(
        &service_finalized_proposal_outcomes,
        &[(BenchmarkParameter::n, n)],
      )
    }
  }

  #[benchmark]
  fn service_finalized_proposal_outcomes(n: Linear<1, 1024>) -> Result<(), BenchmarkError> {
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
    #[block]
    {
      let _ = Pallet::<T>::service_finalized_proposal_outcomes(
        current_epoch_u32.saturating_sub(1),
        current_epoch,
      );
    }
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
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}

#[cfg(test)]
mod fixture_tests {
  use super::*;
  use crate::mock::{Test, new_test_ext};

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
