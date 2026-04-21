use crate::*;

use alloc::{collections::BTreeSet, vec::Vec};
use frame::prelude::*;
use polkadot_sdk::frame_support::transactional;
use polkadot_sdk::sp_runtime::{FixedU128, traits::SaturatedConversion};

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
        Self::note_total_participation(domain, &account);
      }
      Self::note_winning_participation(domain, &account);
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
    let accounts = BoundedVec::try_from(Vec::from([account]))
      .expect("single winning-vote account must fit configured bound");
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

  pub(crate) fn do_reward_coefficient(domain: T::DomainId, account: &T::AccountId) -> FixedU128 {
    let Some(mut window) = WinningVoteWindows::<T>::get(domain, account) else {
      return FixedU128::from_inner(0);
    };
    let lookback = T::WinningVoteLookbackEpochs::get();
    let max_votes = T::MaxWinningVotesPerEpoch::get();
    if lookback == 0 || max_votes == 0 {
      return FixedU128::from_inner(0);
    }
    Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch());
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
        Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch());
        window.rolling_sum
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
      Self::rotate_resolution_window_to(&mut window, current_epoch);
      let item_already_resolved = window.epochs.iter().any(|slot| {
        slot
          .item_ids
          .iter()
          .any(|existing_item_id| *existing_item_id == item_id)
      });
      ensure!(
        !item_already_resolved,
        Error::<T>::DuplicateWinningVoteResolutionItem
      );
      let slot_index = Self::slot_index(current_epoch);
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .expect("fresh resolution window always has full lookback width");
      epoch_slot
        .item_ids
        .try_push(item_id)
        .map_err(|_| Error::<T>::WinningVoteResolutionItemSetFull)?;
      *maybe_window = Some(window);
      Ok(())
    })
  }

  pub(crate) fn note_total_participation(domain: T::DomainId, account: &T::AccountId) {
    ParticipationTotalsByAccount::<T>::mutate(domain, account, |totals| {
      totals.total_participations = totals.total_participations.saturating_add(1);
    });
  }

  pub(crate) fn note_winning_participation(domain: T::DomainId, account: &T::AccountId) {
    ParticipationTotalsByAccount::<T>::mutate(domain, account, |totals| {
      totals.winning_participations = totals.winning_participations.saturating_add(1);
    });
  }

  pub(crate) fn note_authored_proposal(domain: T::DomainId, account: &T::AccountId) {
    ProposalAuthorshipTotalsByAccount::<T>::mutate(domain, account, |totals| {
      totals.authored_proposals = totals.authored_proposals.saturating_add(1);
    });
  }

  pub(crate) fn note_successful_authored_proposal(domain: T::DomainId, account: &T::AccountId) {
    ProposalAuthorshipTotalsByAccount::<T>::mutate(domain, account, |totals| {
      totals.successful_authored_proposals = totals.successful_authored_proposals.saturating_add(1);
    });
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
      Self::rotate_window_to(&mut window, current_epoch);
      let item_already_counted = window.epochs.iter().any(|slot| {
        slot
          .item_ids
          .iter()
          .any(|existing_item_id| *existing_item_id == item_id)
      });
      ensure!(!item_already_counted, Error::<T>::DuplicateWinningVoteItem);
      let slot_index = Self::slot_index(current_epoch);
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .expect("fresh window always has full lookback width");
      ensure!(
        epoch_slot.item_ids.len() < usize::from(T::MaxWinningVotesPerEpoch::get()),
        Error::<T>::EpochVoteCapReached
      );
      epoch_slot
        .item_ids
        .try_push(item_id)
        .map_err(|_| Error::<T>::WinningVoteItemSetFull)?;
      epoch_count = epoch_slot.item_ids.len() as u16;
      window.rolling_sum = window.rolling_sum.saturating_add(1);
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
    let mut epochs = BoundedVec::default();
    for _ in 0..T::WinningVoteLookbackEpochs::get() {
      let push_result = epochs.try_push(WinningVoteEpochSlot {
        item_ids: BoundedVec::default(),
      });
      if push_result.is_err() {
        panic!("fresh window slot vector fits configured lookback")
      }
    }
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
    let mut epochs = BoundedVec::default();
    for _ in 0..T::WinningVoteLookbackEpochs::get() {
      let push_result = epochs.try_push(WinningVoteEpochSlot {
        item_ids: BoundedVec::default(),
      });
      if push_result.is_err() {
        panic!("fresh resolution window slot vector fits configured lookback")
      }
    }
    WinningVoteResolutionWindow {
      last_epoch: current_epoch,
      epochs,
    }
  }

  pub(crate) fn slot_index(epoch: T::Epoch) -> usize {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      return 0;
    }
    (epoch.saturated_into::<u32>() % lookback) as usize
  }

  pub(crate) fn rotate_window_to(
    window: &mut WinningVoteWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteItemsPerEpoch,
    >,
    current_epoch: T::Epoch,
  ) {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      window.last_epoch = current_epoch;
      window.rolling_sum = 0;
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      return;
    }
    let last_epoch = window.last_epoch.saturated_into::<u32>();
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    if current_epoch_u32 <= last_epoch {
      window.last_epoch = current_epoch;
      return;
    }
    let delta = current_epoch_u32.saturating_sub(last_epoch);
    if delta >= lookback {
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      window.rolling_sum = 0;
      window.last_epoch = current_epoch;
      return;
    }
    let old_expired_epoch = last_epoch.saturating_sub(lookback);
    let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
    for expired_epoch in old_expired_epoch.saturating_add(1)..=new_expired_epoch {
      let slot_index = (expired_epoch % lookback) as usize;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .expect("slot index always stays within lookback width");
      window.rolling_sum = window
        .rolling_sum
        .saturating_sub(epoch_slot.item_ids.len() as u32);
      epoch_slot.item_ids.clear();
    }
    window.last_epoch = current_epoch;
  }

  pub(crate) fn rotate_resolution_window_to(
    window: &mut WinningVoteResolutionWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteResolutionItemsPerEpoch,
    >,
    current_epoch: T::Epoch,
  ) {
    let lookback = T::WinningVoteLookbackEpochs::get();
    if lookback == 0 {
      window.last_epoch = current_epoch;
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      return;
    }
    let last_epoch = window.last_epoch.saturated_into::<u32>();
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    if current_epoch_u32 <= last_epoch {
      window.last_epoch = current_epoch;
      return;
    }
    let delta = current_epoch_u32.saturating_sub(last_epoch);
    if delta >= lookback {
      for epoch_slot in window.epochs.iter_mut() {
        epoch_slot.item_ids.clear();
      }
      window.last_epoch = current_epoch;
      return;
    }
    let old_expired_epoch = last_epoch.saturating_sub(lookback);
    let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
    for expired_epoch in old_expired_epoch.saturating_add(1)..=new_expired_epoch {
      let slot_index = (expired_epoch % lookback) as usize;
      let epoch_slot = window
        .epochs
        .get_mut(slot_index)
        .expect("slot index always stays within lookback width");
      epoch_slot.item_ids.clear();
    }
    window.last_epoch = current_epoch;
  }
}
