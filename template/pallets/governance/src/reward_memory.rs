use crate::*;

use alloc::{collections::BTreeSet, vec::Vec};
use frame::prelude::*;
use polkadot_sdk::frame_support::transactional;
use polkadot_sdk::sp_runtime::FixedU128;

impl<T: Config> Pallet<T> {
  pub(crate) fn ingest_winning_vote_resolution_batch_internal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    count_total_participation: bool,
  ) -> DispatchResult {
    let lookback = T::WinningVoteLookbackEpochs::get();
    ensure!(lookback > 0, Error::<T>::ZeroLookbackWindow);
    let current_epoch = T::EpochProvider::current_epoch();
    let mut seen_accounts = BTreeSet::new();
    for account in &accounts {
      ensure!(
        seen_accounts.insert(account.encode()),
        Error::<T>::DuplicateWinningVoteAccount
      );
    }
    Self::record_winning_vote_resolution_item(domain, item_id, current_epoch)?;
    for account in accounts {
      if count_total_participation {
        Self::note_total_participation(domain, &account)?;
      }
      Self::note_winning_participation(domain, &account)?;
      Self::record_winning_vote_for_account(domain, item_id, account, current_epoch)?;
    }
    Ok(())
  }

  #[transactional]
  pub fn ingest_winning_vote_resolution(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    account: T::AccountId,
  ) -> DispatchResult {
    let accounts = BoundedVec::truncate_from(Vec::from([account]));
    Self::ingest_winning_vote_resolution_batch_internal(domain, item_id, accounts, true)
  }

  #[transactional]
  pub fn ingest_winning_vote_resolution_batch(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
  ) -> DispatchResult {
    Self::ingest_winning_vote_resolution_batch_internal(domain, item_id, accounts, true)
  }

  pub(crate) fn do_governance_participation_coefficient(
    domain: T::DomainId,
    account: &T::AccountId,
  ) -> FixedU128 {
    let Some(mut window) = WinningVoteWindows::<T>::get(domain, account) else {
      return FixedU128::from_inner(0);
    };
    let lookback = T::WinningVoteLookbackEpochs::get();
    let max_votes = T::MaxWinningVotesPerEpoch::get();
    if lookback == 0 || max_votes == 0 {
      return FixedU128::from_inner(0);
    }
    if Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch()).is_err() {
      return FixedU128::from_inner(0);
    }
    if window.rolling_sum == 0 {
      return FixedU128::from_inner(0);
    }
    FixedU128::from_rational(
      u128::from(window.rolling_sum),
      u128::from(lookback) * u128::from(max_votes),
    )
  }

  pub(crate) fn do_govxp_counters(domain: T::DomainId, account: &T::AccountId) -> GovXpCounters {
    let rolling_winning_participation = WinningVoteWindows::<T>::get(domain, account)
      .map(|mut window| {
        Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch())
          .map(|()| window.rolling_sum)
          .unwrap_or(0)
      })
      .unwrap_or(0);
    let participation_totals = ParticipationTotalsByAccount::<T>::get(domain, account);
    let authorship_totals = ProposalAuthorshipTotalsByAccount::<T>::get(domain, account);
    GovXpCounters {
      rolling_winning_participation,
      total_participations: participation_totals.total_participations,
      total_winning_participations: participation_totals.winning_participations,
      total_authored_proposals: authorship_totals.authored_proposals,
      total_successful_authored_proposals: authorship_totals.successful_authored_proposals,
    }
  }

  pub(crate) fn record_winning_vote_resolution_item(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    current_epoch: T::Epoch,
  ) -> DispatchResult {
    WinningVoteResolutionWindows::<T>::try_mutate(domain, |maybe_window| -> DispatchResult {
      let mut window = maybe_window
        .take()
        .unwrap_or_else(|| Self::fresh_resolution_window(current_epoch));
      Self::rotate_resolution_window_to(&mut window, current_epoch)?;
      let item_already_resolved = window
        .epochs
        .iter() // deos-bypass: bounded-iter — WinningVoteLookbackEpochs window
        .any(|slot| {
          slot
            .item_ids
            .iter() // deos-bypass: bounded-iter — MaxWinningItemsPerEpoch ids
            .any(|existing_item_id| *existing_item_id == item_id)
        });
      ensure!(
        !item_already_resolved,
        Error::<T>::DuplicateWinningVoteResolutionItem
      );
      let slot_index = Self::slot_index(current_epoch)?;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .ok_or(Error::<T>::RewardWindowInvariant)?;
      epoch_slot
        .item_ids
        .try_push(item_id)
        .map_err(|_| Error::<T>::WinningVoteResolutionItemSetFull)?;
      *maybe_window = Some(window);
      Ok(())
    })
  }

  pub(crate) fn note_total_participation(
    domain: T::DomainId,
    account: &T::AccountId,
  ) -> DispatchResult {
    ParticipationTotalsByAccount::<T>::try_mutate(domain, account, |totals| {
      totals.total_participations = totals
        .total_participations
        .checked_add(1)
        .ok_or(Error::<T>::RewardCounterOverflow)?;
      Ok(())
    })
  }

  pub(crate) fn note_winning_participation(
    domain: T::DomainId,
    account: &T::AccountId,
  ) -> DispatchResult {
    ParticipationTotalsByAccount::<T>::try_mutate(domain, account, |totals| {
      totals.winning_participations = totals
        .winning_participations
        .checked_add(1)
        .ok_or(Error::<T>::RewardCounterOverflow)?;
      Ok(())
    })
  }

  pub(crate) fn note_authored_proposal(
    domain: T::DomainId,
    account: &T::AccountId,
  ) -> DispatchResult {
    ProposalAuthorshipTotalsByAccount::<T>::try_mutate(domain, account, |totals| {
      totals.authored_proposals = totals
        .authored_proposals
        .checked_add(1)
        .ok_or(Error::<T>::RewardCounterOverflow)?;
      Ok(())
    })
  }

  pub(crate) fn note_successful_authored_proposal(
    domain: T::DomainId,
    account: &T::AccountId,
  ) -> DispatchResult {
    ProposalAuthorshipTotalsByAccount::<T>::try_mutate(domain, account, |totals| {
      totals.successful_authored_proposals = totals
        .successful_authored_proposals
        .checked_add(1)
        .ok_or(Error::<T>::RewardCounterOverflow)?;
      Ok(())
    })
  }

  pub(crate) fn record_winning_vote_for_account(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    account: T::AccountId,
    current_epoch: T::Epoch,
  ) -> DispatchResult {
    let mut epoch_count = 0u16;
    let mut rolling_sum = 0u32;
    WinningVoteWindows::<T>::try_mutate(domain, &account, |maybe_window| -> DispatchResult {
      let mut window = maybe_window
        .take()
        .unwrap_or_else(|| Self::fresh_window(current_epoch));
      Self::rotate_window_to(&mut window, current_epoch)?;
      let item_already_counted = window
        .epochs
        .iter() // deos-bypass: bounded-iter — WinningVoteLookbackEpochs window
        .any(|slot| {
          slot
            .item_ids
            .iter() // deos-bypass: bounded-iter — MaxWinningItemsPerEpoch ids
            .any(|existing_item_id| *existing_item_id == item_id)
        });
      ensure!(!item_already_counted, Error::<T>::DuplicateWinningVoteItem);
      let slot_index = Self::slot_index(current_epoch)?;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .ok_or(Error::<T>::RewardWindowInvariant)?;
      ensure!(
        epoch_slot.item_ids.len() < usize::from(T::MaxWinningVotesPerEpoch::get()),
        Error::<T>::EpochVoteCapReached
      );
      epoch_slot
        .item_ids
        .try_push(item_id)
        .map_err(|_| Error::<T>::WinningVoteItemSetFull)?;
      epoch_count = epoch_slot.item_ids.len() as u16;
      window.rolling_sum = window
        .rolling_sum
        .checked_add(1)
        .ok_or(Error::<T>::RewardCounterOverflow)?;
      rolling_sum = window.rolling_sum;
      *maybe_window = Some(window);
      Ok(())
    })?;
    Self::schedule_expiry(domain, &account, current_epoch)?;
    Self::deposit_event(Event::WinningVoteRecorded {
      domain,
      item_id,
      account,
      epoch: current_epoch,
      epoch_count,
      rolling_sum,
    });
    Ok(())
  }

  pub(crate) fn fresh_window(
    current_epoch: T::Epoch,
  ) -> WinningVoteWindow<
    T::Epoch,
    T::WinningVoteItemId,
    T::WinningVoteLookbackEpochs,
    T::MaxWinningVoteItemsPerEpoch,
  > {
    let epochs = BoundedVec::truncate_from(
      (0..T::WinningVoteLookbackEpochs::get())
        .map(|_| WinningVoteEpochSlot {
          item_ids: BoundedVec::default(),
        })
        .collect(),
    );
    WinningVoteWindow {
      last_epoch: current_epoch,
      epochs,
      rolling_sum: 0,
    }
  }

  pub(crate) fn fresh_resolution_window(
    current_epoch: T::Epoch,
  ) -> WinningVoteResolutionWindow<
    T::Epoch,
    T::WinningVoteItemId,
    T::WinningVoteLookbackEpochs,
    T::MaxWinningVoteResolutionItemsPerEpoch,
  > {
    let epochs = BoundedVec::truncate_from(
      (0..T::WinningVoteLookbackEpochs::get())
        .map(|_| WinningVoteEpochSlot {
          item_ids: BoundedVec::default(),
        })
        .collect(),
    );
    WinningVoteResolutionWindow {
      last_epoch: current_epoch,
      epochs,
    }
  }

  pub(crate) fn slot_index(epoch: T::Epoch) -> Result<usize, Error<T>> {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      return Ok(0);
    }
    let epoch = Self::epoch_to_u32(epoch).map_err(|_| Error::<T>::EpochArithmeticOverflow)?;
    Ok((epoch % lookback) as usize)
  }

  pub(crate) fn rotate_window_to(
    window: &mut WinningVoteWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteItemsPerEpoch,
    >,
    current_epoch: T::Epoch,
  ) -> Result<(), Error<T>> {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      window.last_epoch = current_epoch;
      window.rolling_sum = 0;
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      return Ok(());
    }
    ensure!(
      window.epochs.len() == lookback as usize,
      Error::<T>::RewardWindowInvariant
    );
    let last_epoch =
      Self::epoch_to_u32(window.last_epoch).map_err(|_| Error::<T>::EpochArithmeticOverflow)?;
    let current_epoch_u32 =
      Self::epoch_to_u32(current_epoch).map_err(|_| Error::<T>::EpochArithmeticOverflow)?;
    if current_epoch_u32 <= last_epoch {
      window.last_epoch = current_epoch;
      return Ok(());
    }
    let delta = current_epoch_u32
      .checked_sub(last_epoch)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    if delta >= lookback {
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      window.rolling_sum = 0;
      window.last_epoch = current_epoch;
      return Ok(());
    }
    let old_expired_epoch = last_epoch.saturating_sub(lookback);
    let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
    let first_expired_epoch = old_expired_epoch
      .checked_add(1)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    for expired_epoch in first_expired_epoch..=new_expired_epoch {
      let slot_index = (expired_epoch % lookback) as usize;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .ok_or(Error::<T>::RewardWindowInvariant)?;
      let expired_items =
        u32::try_from(epoch_slot.item_ids.len()).map_err(|_| Error::<T>::RewardWindowInvariant)?;
      window.rolling_sum = window
        .rolling_sum
        .checked_sub(expired_items)
        .ok_or(Error::<T>::RewardWindowInvariant)?;
      epoch_slot.item_ids.clear();
    }
    window.last_epoch = current_epoch;
    Ok(())
  }

  #[cfg(feature = "try-runtime")]
  pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
    use alloc::collections::BTreeMap;
    use polkadot_sdk::sp_runtime::TryRuntimeError;

    let expected_width = T::WinningVoteLookbackEpochs::get() as usize;
    let account_windows = WinningVoteWindows::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
    for (_, _, window) in account_windows {
      if window.epochs.len() != expected_width {
        return Err(TryRuntimeError::Other(
          "Winning-vote window width disagrees with configured lookback",
        ));
      }
      let mut expected_sum = 0u32;
      for slot in &window.epochs {
        if slot.item_ids.len() > T::MaxWinningVotesPerEpoch::get() as usize {
          return Err(TryRuntimeError::Other(
            "Winning-vote window slot exceeds the epoch vote cap",
          ));
        }
        let slot_len = u32::try_from(slot.item_ids.len()).map_err(|_| {
          TryRuntimeError::Other("Winning-vote slot length does not fit its configured type")
        })?;
        expected_sum = expected_sum
          .checked_add(slot_len)
          .ok_or(TryRuntimeError::Other(
            "Winning-vote rolling sum overflowed",
          ))?;
      }
      if window.rolling_sum != expected_sum {
        return Err(TryRuntimeError::Other(
          "Winning-vote rolling sum disagrees with retained slots",
        ));
      }
    }

    let resolution_windows = WinningVoteResolutionWindows::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
    for (_, window) in resolution_windows {
      if window.epochs.len() != expected_width {
        return Err(TryRuntimeError::Other(
          "Winning-vote resolution window width disagrees with configured lookback",
        ));
      }
    }

    let mut finalized_by_domain = BTreeMap::<T::DomainId, u32>::new();
    let finalized = FinalizedProposals::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
    for (domain, _, _) in finalized {
      let count = finalized_by_domain.entry(domain).or_default();
      *count = count.checked_add(1).ok_or(TryRuntimeError::Other(
        "Finalized proposal count overflowed",
      ))?;
      if *count > T::MaxRecentFinalizedProposalsPerDomain::get() {
        return Err(TryRuntimeError::Other(
          "Finalized proposals exceed the bounded recent projection",
        ));
      }
    }

    let mut aggregate_custody = BTreeMap::<T::VotePowerLockId, BalanceOf<T>>::new();
    let custody_positions = VotePowerCustodyByAccount::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
    for (account, lock_id, position) in custody_positions {
      if position.amount.is_zero() {
        return Err(TryRuntimeError::Other(
          "vote-power custody position retains a zero amount",
        ));
      }
      let aggregate = aggregate_custody.entry(lock_id).or_default();
      *aggregate = aggregate
        .checked_add(&position.amount)
        .ok_or(TryRuntimeError::Other(
          "aggregate vote-power custody amount overflowed",
        ))?;
      if !GovernanceLocks::<T>::get(&account)
        .is_some_and(|lock| lock.lock_until >= position.lock_until)
      {
        return Err(TryRuntimeError::Other(
          "governance lock does not cover its vote-power custody horizon",
        ));
      }
    }
    for (lock_id, expected_amount) in aggregate_custody {
      if T::VotePowerCustody::custodied_amount(lock_id)
        .is_some_and(|actual_amount| actual_amount != expected_amount)
      {
        return Err(TryRuntimeError::Other(
          "host custody balance disagrees with aggregate governance positions",
        ));
      }
    }

    let active_proposals = ActiveProposals::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
    for (domain, item_id, proposal) in active_proposals {
      let Some(votes) = ProposalVotesByItem::<T>::get(domain, item_id) else {
        continue;
      };
      let lock_until =
        Self::proposal_governance_lock_until(domain, item_id, proposal.submitted_epoch)
          .map_err(|_| TryRuntimeError::Other("active proposal custody horizon is invalid"))?;
      let track_ballots = [
        (
          ProposalTrackFamily::Ordinary,
          votes
            .ayes
            .iter() // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .chain(votes.nays.iter()) // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .chain(votes.amplifies.iter()) // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .chain(votes.approves.iter()) // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .chain(votes.reduces.iter()) // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .collect::<alloc::vec::Vec<_>>(),
        ),
        (
          ProposalTrackFamily::Veto,
          votes
            .vetoes
            .iter() // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .chain(votes.passes.iter()) // deos-bypass: bounded-iter — MaxVotesPerProposal ballots
            .collect::<alloc::vec::Vec<_>>(),
        ),
      ];
      for (track, ballots) in track_ballots {
        let Some(lock_id) = T::VotePowerCustody::lock_id(domain, track) else {
          continue;
        };
        for ballot in ballots {
          let Some(position) = VotePowerCustodyByAccount::<T>::get(&ballot.account, lock_id) else {
            return Err(TryRuntimeError::Other(
              "live transferable ballot has no aggregate custody position",
            ));
          };
          if position.lock_until < lock_until {
            return Err(TryRuntimeError::Other(
              "aggregate custody horizon does not cover a live ballot",
            ));
          }
        }
      }
    }

    let phase = CurrentEpochServicePhase::<T>::get();
    if phase != EpochServicePhase::Maturing {
      let last_processed = Self::epoch_to_u32(LastProcessedEpoch::<T>::get())
        .map_err(|_| TryRuntimeError::Other("LastProcessedEpoch is not exactly representable"))?;
      let next_epoch_u32 = last_processed.checked_add(1).ok_or(TryRuntimeError::Other(
        "noninitial epoch service phase exists beyond the epoch horizon",
      ))?;
      let next_epoch = Self::epoch_from_u32(next_epoch_u32)
        .map_err(|_| TryRuntimeError::Other("next epoch service key is not representable"))?;
      if !ProposalMaturityBuckets::<T>::get(next_epoch).is_empty() {
        return Err(TryRuntimeError::Other(
          "epoch service advanced past a nonempty maturity bucket",
        ));
      }
      if matches!(
        phase,
        EpochServicePhase::FinalizedOutcome | EpochServicePhase::RewardExpiry
      ) && !PendingEnactmentBuckets::<T>::get(next_epoch).is_empty()
      {
        return Err(TryRuntimeError::Other(
          "epoch service advanced past a nonempty enactment bucket",
        ));
      }
      if phase == EpochServicePhase::RewardExpiry
        && !FinalizedProposalOutcomeExpiryBuckets::<T>::get(next_epoch).is_empty()
      {
        return Err(TryRuntimeError::Other(
          "epoch service advanced past a nonempty finalized-outcome bucket",
        ));
      }
    }
    Ok(())
  }

  pub(crate) fn rotate_resolution_window_to(
    window: &mut WinningVoteResolutionWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteResolutionItemsPerEpoch,
    >,
    current_epoch: T::Epoch,
  ) -> Result<(), Error<T>> {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      window.last_epoch = current_epoch;
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      return Ok(());
    }
    ensure!(
      window.epochs.len() == lookback as usize,
      Error::<T>::RewardWindowInvariant
    );
    let last_epoch =
      Self::epoch_to_u32(window.last_epoch).map_err(|_| Error::<T>::EpochArithmeticOverflow)?;
    let current_epoch_u32 =
      Self::epoch_to_u32(current_epoch).map_err(|_| Error::<T>::EpochArithmeticOverflow)?;
    if current_epoch_u32 <= last_epoch {
      window.last_epoch = current_epoch;
      return Ok(());
    }
    let delta = current_epoch_u32
      .checked_sub(last_epoch)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    if delta >= lookback {
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      window.last_epoch = current_epoch;
      return Ok(());
    }
    let old_expired_epoch = last_epoch.saturating_sub(lookback);
    let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
    let first_expired_epoch = old_expired_epoch
      .checked_add(1)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    for expired_epoch in first_expired_epoch..=new_expired_epoch {
      let slot_index = (expired_epoch % lookback) as usize;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .ok_or(Error::<T>::RewardWindowInvariant)?;
      epoch_slot.item_ids.clear();
    }
    window.last_epoch = current_epoch;
    Ok(())
  }
}
