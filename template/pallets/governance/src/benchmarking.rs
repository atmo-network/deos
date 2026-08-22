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

fn seed_finalized_outcomes<T: Config>(domain: T::DomainId, current_epoch: T::Epoch, n: u32)
where
  T::Epoch: From<u32>,
  T::WinningVoteItemId: From<u32>,
{
  let mut bucket = BoundedVec::default();
  for index in 0..n {
    let item_id = benchmark_item_id::<T>(2_000u32.saturating_add(index));
    let push_result = bucket.try_push(FinalizedProposalTouch { domain, item_id });
    if push_result.is_err() {
      panic!("benchmark finalized outcome bucket must fit configured max")
    }
    FinalizedProposals::<T>::insert(
      domain,
      item_id,
      FinalizedProposalRecord {
        outcome: FinalizedProposalOutcome::Approved {
          approval: ProposalApproval {
            approved_epoch: current_epoch,
            winner_count: 1,
          },
          enactment: ProposalEnactmentOutcome::NotAttempted,
        },
        execution_detail: None,
      },
    );
  }
  FinalizedProposalOutcomeExpiryBuckets::<T>::insert(current_epoch, bucket);
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
  fn service_pending_enactments(n: Linear<1, 4>) {
    let domain = benchmark_domain_id::<T>();
    let current_epoch_u32 = 2u32;
    let current_epoch: T::Epoch = current_epoch_u32.into();
    let mut bucket = BoundedVec::default();
    for index in 0..n {
      let item_id = benchmark_item_id::<T>(3_000u32.saturating_add(index));
      bucket
        .try_push(FinalizedProposalTouch { domain, item_id })
        .expect("benchmark pending enactment bucket must fit");
      ProposalPendingEnactmentAt::<T>::insert(domain, item_id, current_epoch);
      FinalizedProposals::<T>::insert(
        domain,
        item_id,
        FinalizedProposalRecord {
          outcome: FinalizedProposalOutcome::Approved {
            approval: ProposalApproval {
              approved_epoch: 1u32.into(),
              winner_count: 1,
            },
            enactment: ProposalEnactmentOutcome::NotAttempted,
          },
          execution_detail: None,
        },
      );
    }
    PendingEnactmentBuckets::<T>::insert(current_epoch, bucket);
    #[block]
    {
      let _ =
        Pallet::<T>::service_pending_enactments(current_epoch_u32.saturating_sub(1), current_epoch);
    }
    assert!(PendingEnactmentBuckets::<T>::get(current_epoch).is_empty());
  }

  #[benchmark]
  fn service_finalized_proposal_outcomes(n: Linear<1, 1024>) {
    let domain = benchmark_domain_id::<T>();
    let current_epoch_u32 = T::FinalizedProposalOutcomeRetentionEpochs::get().saturating_add(1);
    let current_epoch: T::Epoch = current_epoch_u32.into();
    seed_finalized_outcomes::<T>(domain, current_epoch, n);
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
    }
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
