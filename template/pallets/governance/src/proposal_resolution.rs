use crate::*;

use alloc::{collections::BTreeSet, vec::Vec};
use frame::prelude::*;
use polkadot_sdk::frame_support::transactional;
use polkadot_sdk::sp_runtime::{Perbill, traits::SaturatedConversion};

impl<T: Config> Pallet<T> {
  pub(crate) fn voting_progress(
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
  ) -> Perbill {
    let current = current_epoch.saturated_into::<u32>();
    let start = submitted_epoch.saturated_into::<u32>();
    let end = maturity_epoch.saturated_into::<u32>();
    let window = end.saturating_sub(start);
    if window == 0 {
      return Perbill::one();
    }
    let elapsed = current.saturating_sub(start).min(window);
    Perbill::from_rational(elapsed, window)
  }

  /// Adaptive approval threshold: decays linearly from ceiling to floor over the voting window.
  pub(crate) fn approval_threshold_at(
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
  ) -> Perbill {
    let floor = T::ProposalApprovalThreshold::get();
    let ceiling = T::ProposalApprovalCeiling::get();
    if ceiling <= floor {
      return floor;
    }
    let progress = Self::voting_progress(current_epoch, submitted_epoch, maturity_epoch);
    let spread = ceiling.saturating_sub(floor);
    let decay = progress.mul_floor(spread.deconstruct());
    Perbill::from_parts(ceiling.deconstruct().saturating_sub(decay))
  }

  /// Adaptive turnout threshold: decays linearly from ceiling to floor over the voting window.
  pub(crate) fn turnout_threshold_at(
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
  ) -> u64 {
    let floor = T::ProposalMinimumTurnout::get();
    let ceiling = T::ProposalTurnoutCeiling::get();
    if ceiling <= floor {
      return floor;
    }
    let progress = Self::voting_progress(current_epoch, submitted_epoch, maturity_epoch);
    let spread = ceiling.saturating_sub(floor);
    let decay = progress.mul_floor(spread);
    ceiling.saturating_sub(decay)
  }

  pub fn cast_active_proposal_vote(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    account: T::AccountId,
    vote: ProposalVoteKind,
  ) -> DispatchResult {
    let proposal =
      ActiveProposals::<T>::get(domain, item_id).ok_or(Error::<T>::ProposalNotActive)?;
    let current_epoch = T::EpochProvider::current_epoch();
    let protection_close_epoch = Self::proposal_protection_close_epoch(proposal.submitted_epoch)?;
    let primary_track_family = Self::do_proposal_primary_track_family(domain, item_id)
      .ok_or(Error::<T>::ProposalNotActive)?;
    let (
      aye_count,
      nay_count,
      veto_count,
      pass_count,
      replaced_vote,
      immediate_cancellation,
      urgent_authorization,
      urgent_executes_immediately,
      is_first_participation,
    ) = ProposalVotesByItem::<T>::try_mutate(
      domain,
      item_id,
      |maybe_votes| -> Result<
        (
          u32,
          u32,
          u32,
          u32,
          Option<ProposalVoteKind>,
          Option<VetoCancellation>,
          Option<UrgentAuthorization>,
          bool,
          bool,
        ),
        DispatchError,
      > {
        let mut votes = maybe_votes.take().unwrap_or(ProposalVotes {
          ayes: BoundedVec::default(),
          nays: BoundedVec::default(),
          amplifies: BoundedVec::default(),
          approves: BoundedVec::default(),
          reduces: BoundedVec::default(),
          vetoes: BoundedVec::default(),
          passes: BoundedVec::default(),
        });
        let ballot = ProposalBallot {
          account: account.clone(),
          vote_epoch: current_epoch,
        };
        let ordinary_track_vote_exists =
          Self::proposal_ballots_contain_account(&votes.ayes, &account)
            || Self::proposal_ballots_contain_account(&votes.nays, &account)
            || Self::proposal_ballots_contain_account(&votes.amplifies, &account)
            || Self::proposal_ballots_contain_account(&votes.approves, &account)
            || Self::proposal_ballots_contain_account(&votes.reduces, &account);
        let veto_track_vote_exists =
          Self::proposal_ballots_contain_account(&votes.vetoes, &account)
            || Self::proposal_ballots_contain_account(&votes.passes, &account);
        let is_first_participation = !ordinary_track_vote_exists && !veto_track_vote_exists;
        let mut replaced_vote = None;
        match vote {
          ProposalVoteKind::Aye => {
            ensure!(
              primary_track_family == crate::ProposalPrimaryTrackFamily::Binary,
              Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
            );
            ensure!(
              current_epoch.saturated_into::<u32>()
                >= Self::proposal_effective_primary_open_epoch(
                  domain,
                  item_id,
                  proposal.submitted_epoch,
                )?
                .saturated_into::<u32>(),
              Error::<T>::ProposalPrimaryTrackNotOpen
            );
            ensure!(
              !ordinary_track_vote_exists,
              Error::<T>::ProposalVoteAlreadyCast
            );
            votes
              .ayes
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Nay => {
            ensure!(
              current_epoch.saturated_into::<u32>()
                >= Self::proposal_effective_primary_open_epoch(
                  domain,
                  item_id,
                  proposal.submitted_epoch,
                )?
                .saturated_into::<u32>(),
              Error::<T>::ProposalPrimaryTrackNotOpen
            );
            ensure!(
              !ordinary_track_vote_exists,
              Error::<T>::ProposalVoteAlreadyCast
            );
            votes
              .nays
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Amplify => {
            ensure!(
              primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
              Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
            );
            ensure!(
              current_epoch.saturated_into::<u32>()
                >= Self::proposal_effective_primary_open_epoch(
                  domain,
                  item_id,
                  proposal.submitted_epoch,
                )?
                .saturated_into::<u32>(),
              Error::<T>::ProposalPrimaryTrackNotOpen
            );
            ensure!(
              !ordinary_track_vote_exists,
              Error::<T>::ProposalVoteAlreadyCast
            );
            votes
              .amplifies
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Approve => {
            ensure!(
              primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
              Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
            );
            ensure!(
              current_epoch.saturated_into::<u32>()
                >= Self::proposal_effective_primary_open_epoch(
                  domain,
                  item_id,
                  proposal.submitted_epoch,
                )?
                .saturated_into::<u32>(),
              Error::<T>::ProposalPrimaryTrackNotOpen
            );
            ensure!(
              !ordinary_track_vote_exists,
              Error::<T>::ProposalVoteAlreadyCast
            );
            votes
              .approves
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Reduce => {
            ensure!(
              primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
              Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
            );
            ensure!(
              current_epoch.saturated_into::<u32>()
                >= Self::proposal_effective_primary_open_epoch(
                  domain,
                  item_id,
                  proposal.submitted_epoch,
                )?
                .saturated_into::<u32>(),
              Error::<T>::ProposalPrimaryTrackNotOpen
            );
            ensure!(
              !ordinary_track_vote_exists,
              Error::<T>::ProposalVoteAlreadyCast
            );
            votes
              .reduces
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Veto => {
            ensure!(
              !Self::proposal_protection_track_is_closed(current_epoch, protection_close_epoch,),
              Error::<T>::ProposalProtectionTrackClosed
            );
            if let Some(position) = votes
              .passes
              .iter()
              .position(|existing| existing.account == account)
            {
              votes.passes.remove(position);
              replaced_vote = Some(ProposalVoteKind::Pass);
            } else {
              ensure!(
                !votes
                  .vetoes
                  .iter()
                  .any(|existing| existing.account == account),
                Error::<T>::ProposalVoteAlreadyCast
              );
            }
            votes
              .vetoes
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
          ProposalVoteKind::Pass => {
            ensure!(
              !Self::proposal_protection_track_is_closed(current_epoch, protection_close_epoch,),
              Error::<T>::ProposalProtectionTrackClosed
            );
            if let Some(position) = votes
              .vetoes
              .iter()
              .position(|existing| existing.account == account)
            {
              votes.vetoes.remove(position);
              replaced_vote = Some(ProposalVoteKind::Veto);
            } else {
              ensure!(
                !votes
                  .passes
                  .iter()
                  .any(|existing| existing.account == account),
                Error::<T>::ProposalVoteAlreadyCast
              );
            }
            votes
              .passes
              .try_push(ballot)
              .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
          }
        }
        let aye_count = votes.ayes.len() as u32;
        let nay_count = votes.nays.len() as u32;
        let veto_count = votes.vetoes.len() as u32;
        let pass_count = votes.passes.len() as u32;
        let immediate_cancellation =
          Self::current_veto_cancellation(domain, item_id, &votes, false);
        let urgent_authorization = if immediate_cancellation.is_none() {
          Self::current_urgent_authorization(domain, item_id, &votes)
        } else {
          None
        };
        let urgent_executes_immediately = if urgent_authorization.is_some() {
          Self::urgent_fast_track_executes_immediately(domain, item_id, &votes)
        } else {
          false
        };
        *maybe_votes = Some(votes);
        Ok((
          aye_count,
          nay_count,
          veto_count,
          pass_count,
          replaced_vote,
          immediate_cancellation,
          urgent_authorization,
          urgent_executes_immediately,
          is_first_participation,
        ))
      },
    )?;
    if is_first_participation {
      Self::note_total_participation(domain, &account);
    }
    Self::deposit_event(Event::ProposalVoteCast {
      domain,
      item_id,
      account,
      vote,
      replaced_vote,
      vote_epoch: current_epoch,
      aye_count,
      nay_count,
      veto_count,
      pass_count,
    });
    if let Some(cancellation) = immediate_cancellation {
      return Self::veto_cancel_active_proposal(domain, item_id, cancellation);
    }
    if let Some(urgent_authorization) = urgent_authorization {
      Self::authorize_urgent_fast_track(domain, item_id, current_epoch, urgent_authorization);
      if urgent_executes_immediately {
        return Self::resolve_active_proposal_without_winners(domain, item_id);
      }
    }
    // Reset confirm timer if a new vote changes the passing state during confirm
    if T::ProposalConfirmPeriod::get().saturated_into::<u32>() > 0
      && ProposalConfirmStartedAt::<T>::contains_key(domain, item_id)
    {
      match Self::proposal_resolution_state(domain, item_id) {
        Some(ProposalResolutionState::Confirming { .. }) => {
          // Still passing — keep confirm timer running
        }
        _ => {
          // No longer passing — reset confirm, re-schedule maturity
          let reset_epoch = T::EpochProvider::current_epoch();
          ProposalConfirmStartedAt::<T>::remove(domain, item_id);
          let next_epoch_u32 = reset_epoch.saturated_into::<u32>().saturating_add(1);
          let _ =
            Self::schedule_proposal_maturity_at(next_epoch_u32.saturated_into(), domain, item_id);
          Self::deposit_event(Event::ProposalConfirmReset {
            domain,
            item_id,
            epoch: reset_epoch,
          });
        }
      }
    }
    Ok(())
  }

  pub(crate) fn proposal_vote_context(
    item_id: T::WinningVoteItemId,
    submitted_epoch: T::Epoch,
    current_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
    vote_epoch: T::Epoch,
  ) -> crate::ProposalVoteContext<T::WinningVoteItemId, T::Epoch> {
    crate::ProposalVoteContext {
      item_id,
      current_epoch,
      submitted_epoch,
      maturity_epoch,
      vote_epoch,
    }
  }

  pub(crate) fn proposal_ordinary_weighting_window(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<(T::Epoch, T::Epoch, T::Epoch)> {
    let proposal = ActiveProposals::<T>::get(domain, item_id)?;
    let current_epoch = T::EpochProvider::current_epoch();
    let primary_open_epoch =
      Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
        .ok()?;
    let primary_close_epoch =
      Self::proposal_effective_primary_close_epoch(domain, item_id, proposal.submitted_epoch)
        .ok()?;
    Some((current_epoch, primary_open_epoch, primary_close_epoch))
  }

  pub(crate) fn proposal_protection_weighting_window(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<(T::Epoch, T::Epoch, T::Epoch)> {
    let proposal = ActiveProposals::<T>::get(domain, item_id)?;
    let current_epoch = T::EpochProvider::current_epoch();
    let protection_close_epoch =
      Self::proposal_protection_close_epoch(proposal.submitted_epoch).ok()?;
    Some((
      current_epoch,
      proposal.submitted_epoch,
      protection_close_epoch,
    ))
  }

  pub(crate) fn proposal_vote_weight_sum(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
    ballots: &BoundedVec<ProposalBallot<T::AccountId, T::Epoch>, T::MaxWinningVoteAccountsPerCall>,
  ) -> u64 {
    ballots.iter().fold(0u64, |sum, ballot| {
      let context = Self::proposal_vote_context(
        item_id,
        submitted_epoch,
        current_epoch,
        maturity_epoch,
        ballot.vote_epoch,
      );
      sum.saturating_add(u64::from(T::ProposalVoteWeightProvider::vote_weight(
        domain,
        &context,
        &ballot.account,
      )))
    })
  }

  pub(crate) fn proposal_veto_weight_sum(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
    ballots: &BoundedVec<ProposalBallot<T::AccountId, T::Epoch>, T::MaxWinningVoteAccountsPerCall>,
  ) -> u64 {
    ballots.iter().fold(0u64, |sum, ballot| {
      let context = Self::proposal_vote_context(
        item_id,
        submitted_epoch,
        current_epoch,
        maturity_epoch,
        ballot.vote_epoch,
      );
      sum.saturating_add(T::VetoVotePowerProvider::vote_weight(
        domain,
        &context,
        &ballot.account,
      ))
    })
  }

  pub(crate) fn proposal_raw_protection_weight_sum(
    domain: T::DomainId,
    ballots: &BoundedVec<ProposalBallot<T::AccountId, T::Epoch>, T::MaxWinningVoteAccountsPerCall>,
  ) -> u64 {
    ballots.iter().fold(0u64, |sum, ballot| {
      sum.saturating_add(T::VetoVotePowerProvider::raw_vote_weight(
        domain,
        &ballot.account,
      ))
    })
  }

  pub(crate) fn proposal_raw_veto_weight_sum(
    domain: T::DomainId,
    ballots: &BoundedVec<ProposalBallot<T::AccountId, T::Epoch>, T::MaxWinningVoteAccountsPerCall>,
  ) -> u64 {
    Self::proposal_raw_protection_weight_sum(domain, ballots)
  }

  pub(crate) fn proposal_veto_track_weights(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    current_epoch: T::Epoch,
    submitted_epoch: T::Epoch,
    maturity_epoch: T::Epoch,
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) -> (u64, u64) {
    (
      Self::proposal_veto_weight_sum(
        domain,
        item_id,
        current_epoch,
        submitted_epoch,
        maturity_epoch,
        &votes.vetoes,
      ),
      Self::proposal_veto_weight_sum(
        domain,
        item_id,
        current_epoch,
        submitted_epoch,
        maturity_epoch,
        &votes.passes,
      ),
    )
  }

  pub(crate) fn proposal_protection_track_is_closed(
    current_epoch: T::Epoch,
    protection_close_epoch: T::Epoch,
  ) -> bool {
    current_epoch.saturated_into::<u32>() >= protection_close_epoch.saturated_into::<u32>()
  }

  pub(crate) fn veto_weight_strictly_exceeds_threshold(
    veto_weight: u64,
    total_veto_issuance: u64,
  ) -> bool {
    if veto_weight == 0 || total_veto_issuance == 0 {
      return false;
    }
    let threshold_parts = u128::from(T::ProposalVetoThreshold::get().deconstruct());
    let veto_parts = u128::from(veto_weight).saturating_mul(1_000_000_000u128);
    let threshold_weight = u128::from(total_veto_issuance).saturating_mul(threshold_parts);
    veto_parts > threshold_weight
  }

  pub(crate) fn veto_weight_meets_minimum_turnout(
    veto_weight: u64,
    total_veto_issuance: u64,
  ) -> bool {
    if veto_weight == 0 || total_veto_issuance == 0 {
      return false;
    }
    let threshold_parts = u128::from(T::ProposalVetoMinimumVetoTurnout::get().deconstruct());
    if threshold_parts == 0 {
      return true;
    }
    let veto_parts = u128::from(veto_weight).saturating_mul(1_000_000_000u128);
    let threshold_weight = u128::from(total_veto_issuance).saturating_mul(threshold_parts);
    veto_parts >= threshold_weight
  }

  pub(crate) fn pass_weight_meets_fast_track_threshold(
    pass_weight: u64,
    total_protection_supply: u64,
  ) -> bool {
    if pass_weight == 0 || total_protection_supply == 0 {
      return false;
    }
    let threshold = T::ProposalFastTrackPassThreshold::get();
    if threshold == Perbill::one() {
      return pass_weight == total_protection_supply;
    }
    let threshold_parts = u128::from(threshold.deconstruct());
    let pass_parts = u128::from(pass_weight).saturating_mul(1_000_000_000u128);
    let threshold_weight = u128::from(total_protection_supply).saturating_mul(threshold_parts);
    pass_parts > threshold_weight
  }

  pub(crate) fn current_veto_cancellation(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    allow_track_outcome: bool,
  ) -> Option<VetoCancellation> {
    let raw_veto_weight = Self::proposal_raw_veto_weight_sum(domain, &votes.vetoes);
    let total_veto_issuance = T::VetoVotePowerProvider::total_issuance(domain);
    let (current_epoch, protection_open_epoch, protection_close_epoch) =
      Self::proposal_protection_weighting_window(domain, item_id)?;
    let (veto_weight, pass_weight) = Self::proposal_veto_track_weights(
      domain,
      item_id,
      current_epoch,
      protection_open_epoch,
      protection_close_epoch,
      votes,
    );
    if Self::veto_weight_strictly_exceeds_threshold(raw_veto_weight, total_veto_issuance) {
      return Some(VetoCancellation {
        veto_weight,
        pass_weight,
        mode: VetoCancellationMode::ImmediateThreshold,
      });
    }
    if allow_track_outcome
      && Self::veto_weight_meets_minimum_turnout(raw_veto_weight, total_veto_issuance)
      && veto_weight >= pass_weight
    {
      return Some(VetoCancellation {
        veto_weight,
        pass_weight,
        mode: VetoCancellationMode::TrackOutcome,
      });
    }
    None
  }

  pub(crate) fn proposal_is_urgent_authorized(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> bool {
    ProposalUrgentAuthorizedAt::<T>::contains_key(domain, item_id)
  }

  pub(crate) fn current_urgent_authorization(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) -> Option<UrgentAuthorization> {
    if Self::proposal_is_urgent_authorized(domain, item_id) {
      return None;
    }
    let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)?;
    if !T::ProposalUrgentPolicyProvider::is_expeditable(domain, metadata.payload_kind) {
      return None;
    }
    let pass_weight = Self::proposal_raw_protection_weight_sum(domain, &votes.passes);
    let total_protection_supply = T::VetoVotePowerProvider::total_issuance(domain);
    if !Self::pass_weight_meets_fast_track_threshold(pass_weight, total_protection_supply) {
      return None;
    }
    Some(UrgentAuthorization {
      pass_weight,
      total_protection_supply,
    })
  }

  pub(crate) fn urgent_fast_track_executes_immediately(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) -> bool {
    let Some(metadata) = ProposalMetadataByItem::<T>::get(domain, item_id) else {
      return false;
    };
    if !T::ProposalUrgentPolicyProvider::executes_immediately_on_unanimous_pass(
      domain,
      metadata.payload_kind,
    ) {
      return false;
    }
    let pass_weight = Self::proposal_raw_protection_weight_sum(domain, &votes.passes);
    let total_protection_supply = T::VetoVotePowerProvider::total_issuance(domain);
    total_protection_supply > 0 && pass_weight == total_protection_supply
  }

  pub(crate) fn authorize_urgent_fast_track(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    authorization_epoch: T::Epoch,
    urgent_authorization: UrgentAuthorization,
  ) {
    ProposalUrgentAuthorizedAt::<T>::insert(domain, item_id, authorization_epoch);
    Self::deposit_event(Event::ProposalUrgentAuthorized {
      domain,
      item_id,
      authorization_epoch,
      pass_weight: urgent_authorization.pass_weight,
      total_protection_supply: urgent_authorization.total_protection_supply,
    });
  }

  pub(crate) fn proposal_has_any_votes(
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) -> bool {
    !votes.ayes.is_empty()
      || !votes.nays.is_empty()
      || !votes.amplifies.is_empty()
      || !votes.approves.is_empty()
      || !votes.reduces.is_empty()
      || !votes.vetoes.is_empty()
      || !votes.passes.is_empty()
  }

  pub(crate) fn proposal_ballots_contain_account(
    ballots: &BoundedVec<ProposalBallot<T::AccountId, T::Epoch>, T::MaxWinningVoteAccountsPerCall>,
    account: &T::AccountId,
  ) -> bool {
    for ballot in ballots {
      if ballot.account == *account {
        return true;
      }
    }
    false
  }

  pub(crate) fn collect_unique_accounts<I>(accounts: I) -> Vec<T::AccountId>
  where
    I: IntoIterator<Item = T::AccountId>,
  {
    let mut seen_accounts = BTreeSet::new();
    let mut unique_accounts = Vec::new();
    for account in accounts {
      if seen_accounts.insert(account.encode()) {
        unique_accounts.push(account);
      }
    }
    unique_accounts
  }

  pub(crate) fn note_winning_participation_batch(
    domain: T::DomainId,
    accounts: impl IntoIterator<Item = T::AccountId>,
  ) {
    for account in Self::collect_unique_accounts(accounts) {
      Self::note_winning_participation(domain, &account);
    }
  }

  pub(crate) fn note_pass_winning_participation(
    domain: T::DomainId,
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) {
    for ballot in &votes.passes {
      Self::note_winning_participation(domain, &ballot.account);
    }
  }

  pub(crate) fn infer_winning_primary_option_from_winners(
    votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    winners: &BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
  ) -> Option<ProposalPrimaryTrackOption> {
    if winners.is_empty() {
      return None;
    }
    let winners_match = |ballots: &BoundedVec<
      ProposalBallot<T::AccountId, T::Epoch>,
      T::MaxWinningVoteAccountsPerCall,
    >| {
      for winner in winners {
        if !Self::proposal_ballots_contain_account(ballots, winner) {
          return false;
        }
      }
      true
    };
    if winners_match(&votes.ayes) {
      return Some(ProposalPrimaryTrackOption::Aye);
    }
    if winners_match(&votes.amplifies) {
      return Some(ProposalPrimaryTrackOption::Amplify);
    }
    if winners_match(&votes.approves) {
      return Some(ProposalPrimaryTrackOption::Approve);
    }
    if winners_match(&votes.reduces) {
      return Some(ProposalPrimaryTrackOption::Reduce);
    }
    if winners_match(&votes.nays) {
      return Some(ProposalPrimaryTrackOption::Nay);
    }
    None
  }

  pub(crate) fn resolve_or_reject_from_current_votes(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    votes: ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
  ) -> DispatchResult {
    let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
    if let Some(cancellation) =
      Self::current_veto_cancellation(domain, item_id, &votes, !urgent_authorized)
    {
      return Self::veto_cancel_active_proposal(domain, item_id, cancellation);
    }
    let (current_epoch, primary_open_epoch, primary_close_epoch) =
      Self::proposal_ordinary_weighting_window(domain, item_id)
        .ok_or(Error::<T>::ProposalNotActive)?;
    let (protection_current_epoch, protection_open_epoch, protection_close_epoch) =
      Self::proposal_protection_weighting_window(domain, item_id)
        .ok_or(Error::<T>::ProposalNotActive)?;
    let family = Self::do_proposal_primary_track_family(domain, item_id)
      .ok_or(Error::<T>::ProposalNotActive)?;
    let aye_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.ayes,
    );
    let nay_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.nays,
    );
    let amplify_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.amplifies,
    );
    let approve_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.approves,
    );
    let reduce_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.reduces,
    );
    let veto_weight = Self::proposal_veto_weight_sum(
      domain,
      item_id,
      protection_current_epoch,
      protection_open_epoch,
      protection_close_epoch,
      &votes.vetoes,
    );
    let pass_weight = Self::proposal_veto_weight_sum(
      domain,
      item_id,
      protection_current_epoch,
      protection_open_epoch,
      protection_close_epoch,
      &votes.passes,
    );
    let turnout = match family {
      crate::ProposalPrimaryTrackFamily::Binary => aye_weight.saturating_add(nay_weight),
      crate::ProposalPrimaryTrackFamily::Invoice => amplify_weight
        .saturating_add(approve_weight)
        .saturating_add(reduce_weight)
        .saturating_add(nay_weight),
    };
    if turnout == 0 {
      return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::NoVotes);
    }
    if turnout < Self::turnout_threshold_at(current_epoch, primary_open_epoch, primary_close_epoch)
    {
      return Self::reject_active_proposal(
        domain,
        item_id,
        ProposalRejectionReason::TurnoutBelowMinimum,
      );
    }
    let approval_threshold =
      Self::approval_threshold_at(current_epoch, primary_open_epoch, primary_close_epoch);
    match family {
      crate::ProposalPrimaryTrackFamily::Binary => {
        if aye_weight == nay_weight {
          return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::VoteTie);
        }
        let aye_approval = Perbill::from_rational(aye_weight, turnout);
        let nay_approval = Perbill::from_rational(nay_weight, turnout);
        if aye_approval >= approval_threshold {
          if pass_weight > veto_weight {
            Self::note_pass_winning_participation(domain, &votes);
          }
          let winners = votes.ayes.into_iter().fold(
            BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
            |mut winners, ballot| {
              let push_result = winners.try_push(ballot.account);
              if push_result.is_err() {
                panic!("winner projection must preserve the bounded vote set")
              }
              winners
            },
          );
          return Self::resolve_active_proposal(
            domain,
            item_id,
            winners,
            Some(ProposalPrimaryTrackOption::Aye),
          );
        }
        if nay_approval >= approval_threshold {
          if pass_weight > veto_weight {
            Self::note_pass_winning_participation(domain, &votes);
          }
          let winners = votes.nays.into_iter().fold(
            BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
            |mut winners, ballot| {
              let push_result = winners.try_push(ballot.account);
              if push_result.is_err() {
                panic!("winner projection must preserve the bounded vote set")
              }
              winners
            },
          );
          return Self::resolve_active_proposal(
            domain,
            item_id,
            winners,
            Some(ProposalPrimaryTrackOption::Nay),
          );
        }
        Self::reject_active_proposal(
          domain,
          item_id,
          ProposalRejectionReason::ApprovalThresholdNotMet,
        )
      }
      crate::ProposalPrimaryTrackFamily::Invoice => {
        let positive_weight = amplify_weight
          .saturating_add(approve_weight)
          .saturating_add(reduce_weight);
        if positive_weight == nay_weight {
          return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::VoteTie);
        }
        if positive_weight < nay_weight {
          return Self::reject_active_proposal(
            domain,
            item_id,
            ProposalRejectionReason::ApprovalThresholdNotMet,
          );
        }
        let positive_approval = Perbill::from_rational(positive_weight, turnout);
        if positive_approval < approval_threshold {
          return Self::reject_active_proposal(
            domain,
            item_id,
            ProposalRejectionReason::ApprovalThresholdNotMet,
          );
        }
        if pass_weight > veto_weight {
          Self::note_pass_winning_participation(domain, &votes);
        }
        let (leading_positive_option, _) =
          Self::invoice_leading_positive_weights(amplify_weight, approve_weight, reduce_weight);
        let winners = match leading_positive_option {
          Some(ProposalPrimaryTrackOption::Amplify) => votes.amplifies.into_iter().fold(
            BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
            |mut winners, ballot| {
              let push_result = winners.try_push(ballot.account);
              if push_result.is_err() {
                panic!("winner projection must preserve the bounded vote set")
              }
              winners
            },
          ),
          Some(ProposalPrimaryTrackOption::Approve) => votes.approves.into_iter().fold(
            BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
            |mut winners, ballot| {
              let push_result = winners.try_push(ballot.account);
              if push_result.is_err() {
                panic!("winner projection must preserve the bounded vote set")
              }
              winners
            },
          ),
          Some(ProposalPrimaryTrackOption::Reduce) => votes.reduces.into_iter().fold(
            BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
            |mut winners, ballot| {
              let push_result = winners.try_push(ballot.account);
              if push_result.is_err() {
                panic!("winner projection must preserve the bounded vote set")
              }
              winners
            },
          ),
          _ => {
            return Self::reject_active_proposal(
              domain,
              item_id,
              ProposalRejectionReason::ApprovalThresholdNotMet,
            );
          }
        };
        Self::resolve_active_proposal(domain, item_id, winners, leading_positive_option)
      }
    }
  }

  #[transactional]
  pub(crate) fn resolve_active_proposal_without_winners(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    ensure!(
      ActiveProposals::<T>::contains_key(domain, item_id),
      Error::<T>::ProposalNotActive
    );
    let winner_count = 0;
    let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
    ActiveProposals::<T>::remove(domain, item_id);
    ProposalConfirmStartedAt::<T>::remove(domain, item_id);
    ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
    let proposer = ProposalAuthorsByItem::<T>::take(domain, item_id);
    Self::remove_active_proposal_id(domain, item_id);
    ProposalVotesByItem::<T>::remove(domain, item_id);
    ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
    let current_epoch = T::EpochProvider::current_epoch();
    if let Some(proposer) = proposer {
      Self::note_successful_authored_proposal(domain, &proposer);
    }
    Self::record_finalized_proposal_outcome(
      domain,
      item_id,
      FinalizedProposalOutcome::Resolved {
        epoch: current_epoch,
        winner_count,
      },
      current_epoch,
    )?;
    let enactment_scheduled = if urgent_authorized {
      ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
      false
    } else {
      Self::schedule_pending_enactment_if_needed(domain, item_id, current_epoch)?
    };
    if !enactment_scheduled {
      Self::maybe_execute_proposal_payload(
        domain,
        item_id,
        current_epoch,
        winner_count,
        current_epoch,
      )?;
    }
    let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
      *active_count = active_count.saturating_sub(1);
      *active_count
    });
    Self::deposit_event(Event::ProposalResolved {
      domain,
      item_id,
      epoch: current_epoch,
      winner_count,
      active_count,
    });
    Ok(())
  }

  #[transactional]
  pub fn resolve_active_proposal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    winners: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    winning_primary_option: Option<ProposalPrimaryTrackOption>,
  ) -> DispatchResult {
    ensure!(!winners.is_empty(), Error::<T>::ProposalWinnerSetEmpty);
    ensure!(
      ActiveProposals::<T>::contains_key(domain, item_id),
      Error::<T>::ProposalNotActive
    );
    let winner_count = winners.len() as u32;
    let votes = ProposalVotesByItem::<T>::get(domain, item_id);
    let count_total_participation = votes
      .as_ref()
      .map(Self::proposal_has_any_votes)
      .map(|has_votes| !has_votes)
      .unwrap_or(true);
    Self::ingest_winning_vote_resolution_batch_internal(
      domain,
      item_id,
      winners,
      count_total_participation,
    )?;
    let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
    ActiveProposals::<T>::remove(domain, item_id);
    ProposalConfirmStartedAt::<T>::remove(domain, item_id);
    ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
    let proposer = ProposalAuthorsByItem::<T>::take(domain, item_id);
    Self::remove_active_proposal_id(domain, item_id);
    ProposalVotesByItem::<T>::remove(domain, item_id);
    if let Some(winning_primary_option) = winning_primary_option {
      ProposalWinningPrimaryOptionByItem::<T>::insert(domain, item_id, winning_primary_option);
    } else {
      ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
    }
    let current_epoch = T::EpochProvider::current_epoch();
    if let Some(proposer) = proposer {
      Self::note_successful_authored_proposal(domain, &proposer);
    }
    Self::record_finalized_proposal_outcome(
      domain,
      item_id,
      FinalizedProposalOutcome::Resolved {
        epoch: current_epoch,
        winner_count,
      },
      current_epoch,
    )?;
    let enactment_scheduled = if urgent_authorized {
      ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
      false
    } else {
      Self::schedule_pending_enactment_if_needed(domain, item_id, current_epoch)?
    };
    if !enactment_scheduled {
      Self::maybe_execute_proposal_payload(
        domain,
        item_id,
        current_epoch,
        winner_count,
        current_epoch,
      )?;
    }
    let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
      *active_count = active_count.saturating_sub(1);
      *active_count
    });
    Self::deposit_event(Event::ProposalResolved {
      domain,
      item_id,
      epoch: current_epoch,
      winner_count,
      active_count,
    });
    Ok(())
  }

  #[transactional]
  pub fn resolve_active_proposal_from_votes(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    Self::resolve_active_proposal_from_votes_with_policy(domain, item_id, true)
  }

  #[transactional]
  pub(crate) fn resolve_active_proposal_from_votes_with_policy(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    enforce_voting_window: bool,
  ) -> DispatchResult {
    match Self::proposal_resolution_state(domain, item_id) {
      None => Err(Error::<T>::ProposalNotActive.into()),
      Some(ProposalResolutionState::VotingWindowOpen { .. }) if enforce_voting_window => {
        Err(Error::<T>::ProposalVotingWindowStillOpen.into())
      }
      Some(ProposalResolutionState::VotingWindowOpen { .. }) => {
        let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
          ayes: BoundedVec::default(),
          nays: BoundedVec::default(),
          amplifies: BoundedVec::default(),
          approves: BoundedVec::default(),
          reduces: BoundedVec::default(),
          vetoes: BoundedVec::default(),
          passes: BoundedVec::default(),
        });
        Self::resolve_or_reject_from_current_votes(domain, item_id, votes)
      }
      Some(ProposalResolutionState::VetoPassing {
        veto_weight,
        pass_weight,
        mode,
      }) => Self::veto_cancel_active_proposal(
        domain,
        item_id,
        VetoCancellation {
          veto_weight,
          pass_weight,
          mode,
        },
      ),
      Some(ProposalResolutionState::PassingAye)
      | Some(ProposalResolutionState::PassingAmplify)
      | Some(ProposalResolutionState::PassingApprove)
      | Some(ProposalResolutionState::PassingReduce)
      | Some(ProposalResolutionState::PassingNay)
      | Some(ProposalResolutionState::Confirming { .. })
      | Some(ProposalResolutionState::Rejected { .. }) => {
        let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
          ayes: BoundedVec::default(),
          nays: BoundedVec::default(),
          amplifies: BoundedVec::default(),
          approves: BoundedVec::default(),
          reduces: BoundedVec::default(),
          vetoes: BoundedVec::default(),
          passes: BoundedVec::default(),
        });
        Self::resolve_or_reject_from_current_votes(domain, item_id, votes)
      }
    }
  }

  #[transactional]
  pub fn reject_active_proposal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    reason: ProposalRejectionReason,
  ) -> DispatchResult {
    ensure!(
      ActiveProposals::<T>::take(domain, item_id).is_some(),
      Error::<T>::ProposalNotActive
    );
    ProposalAuthorsByItem::<T>::remove(domain, item_id);
    ProposalConfirmStartedAt::<T>::remove(domain, item_id);
    ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
    ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
    ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
    Self::remove_active_proposal_id(domain, item_id);
    ProposalVotesByItem::<T>::remove(domain, item_id);
    let current_epoch = T::EpochProvider::current_epoch();
    Self::record_finalized_proposal_outcome(
      domain,
      item_id,
      FinalizedProposalOutcome::Rejected {
        epoch: current_epoch,
        reason,
      },
      current_epoch,
    )?;
    let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
      *active_count = active_count.saturating_sub(1);
      *active_count
    });
    Self::deposit_event(Event::ProposalRejected {
      domain,
      item_id,
      epoch: current_epoch,
      reason,
      active_count,
    });
    Ok(())
  }

  #[transactional]
  pub(crate) fn veto_cancel_active_proposal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    cancellation: VetoCancellation,
  ) -> DispatchResult {
    ensure!(
      ActiveProposals::<T>::take(domain, item_id).is_some(),
      Error::<T>::ProposalNotActive
    );
    ProposalAuthorsByItem::<T>::remove(domain, item_id);
    ProposalConfirmStartedAt::<T>::remove(domain, item_id);
    ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
    ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
    ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
    Self::remove_active_proposal_id(domain, item_id);
    let votes = ProposalVotesByItem::<T>::get(domain, item_id);
    if let Some(votes) = &votes {
      Self::note_winning_participation_batch(
        domain,
        votes.vetoes.iter().map(|ballot| ballot.account.clone()),
      );
    }
    ProposalVotesByItem::<T>::remove(domain, item_id);
    let current_epoch = T::EpochProvider::current_epoch();
    Self::record_finalized_proposal_outcome(
      domain,
      item_id,
      FinalizedProposalOutcome::VetoCancelled {
        epoch: current_epoch,
        veto_weight: cancellation.veto_weight,
      },
      current_epoch,
    )?;
    let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
      *active_count = active_count.saturating_sub(1);
      *active_count
    });
    Self::deposit_event(Event::ProposalVetoCancelled {
      domain,
      item_id,
      epoch: current_epoch,
      veto_weight: cancellation.veto_weight,
      pass_weight: cancellation.pass_weight,
      mode: cancellation.mode,
      active_count,
    });
    Ok(())
  }

  pub(crate) fn track_family_for_vote_kind(vote: ProposalVoteKind) -> crate::ProposalTrackFamily {
    match vote {
      ProposalVoteKind::Aye
      | ProposalVoteKind::Nay
      | ProposalVoteKind::Amplify
      | ProposalVoteKind::Approve
      | ProposalVoteKind::Reduce => crate::ProposalTrackFamily::Ordinary,
      ProposalVoteKind::Veto | ProposalVoteKind::Pass => crate::ProposalTrackFamily::Veto,
    }
  }

  pub(crate) fn do_proposal_vote_power_profile(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    vote: ProposalVoteKind,
  ) -> Option<crate::ProposalVotePowerProfile> {
    ActiveProposals::<T>::contains_key(domain, item_id).then(|| {
      T::ProposalTrackPowerProfileProvider::power_profile(
        domain,
        item_id,
        Self::track_family_for_vote_kind(vote),
      )
    })
  }

  pub(crate) fn invoice_leading_positive_weights(
    amplify_weight: u64,
    approve_weight: u64,
    reduce_weight: u64,
  ) -> (Option<ProposalPrimaryTrackOption>, u64) {
    let mut leading_option = None;
    let mut leading_weight = 0u64;
    if amplify_weight > 0 {
      leading_option = Some(ProposalPrimaryTrackOption::Amplify);
      leading_weight = amplify_weight;
    }
    if approve_weight > 0 && approve_weight >= leading_weight {
      leading_option = Some(ProposalPrimaryTrackOption::Approve);
      leading_weight = approve_weight;
    }
    if reduce_weight > 0 && reduce_weight >= leading_weight {
      leading_option = Some(ProposalPrimaryTrackOption::Reduce);
      leading_weight = reduce_weight;
    }
    (leading_option, leading_weight)
  }

  pub(crate) fn invoice_leading_positive_option(
    tally: &ProposalVoteTally,
  ) -> (Option<ProposalPrimaryTrackOption>, u64) {
    Self::invoice_leading_positive_weights(
      tally.amplify_weight,
      tally.approve_weight,
      tally.reduce_weight,
    )
  }

  pub(crate) fn do_proposal_primary_track_tally(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalPrimaryTrackTally> {
    let family = Self::do_proposal_primary_track_family(domain, item_id)?;
    let tally = Self::do_proposal_vote_tally(domain, item_id)?;
    match family {
      crate::ProposalPrimaryTrackFamily::Binary => {
        let leading_option = if tally.aye_weight > tally.nay_weight {
          Some(ProposalPrimaryTrackOption::Aye)
        } else if tally.nay_weight > tally.aye_weight {
          Some(ProposalPrimaryTrackOption::Nay)
        } else {
          None
        };
        Some(ProposalPrimaryTrackTally::Binary {
          aye_voters: tally.aye_voters,
          nay_voters: tally.nay_voters,
          aye_weight: tally.aye_weight,
          nay_weight: tally.nay_weight,
          turnout_weight: tally.turnout_weight,
          leading_option,
        })
      }
      crate::ProposalPrimaryTrackFamily::Invoice => {
        let positive_weight = tally
          .amplify_weight
          .saturating_add(tally.approve_weight)
          .saturating_add(tally.reduce_weight);
        let (leading_positive_option, leading_positive_weight) =
          Self::invoice_leading_positive_option(&tally);
        Some(ProposalPrimaryTrackTally::Invoice {
          amplify_voters: tally.amplify_voters,
          approve_voters: tally.approve_voters,
          reduce_voters: tally.reduce_voters,
          nay_voters: tally.nay_voters,
          amplify_weight: tally.amplify_weight,
          approve_weight: tally.approve_weight,
          reduce_weight: tally.reduce_weight,
          nay_weight: tally.nay_weight,
          positive_weight,
          turnout_weight: tally.turnout_weight,
          leading_positive_option,
          leading_positive_weight,
        })
      }
    }
  }

  pub(crate) fn do_proposal_vote_tally(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalVoteTally> {
    let (current_epoch, primary_open_epoch, primary_close_epoch) =
      Self::proposal_ordinary_weighting_window(domain, item_id)?;
    let (protection_current_epoch, protection_open_epoch, protection_close_epoch) =
      Self::proposal_protection_weighting_window(domain, item_id)?;
    let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
      ayes: BoundedVec::default(),
      nays: BoundedVec::default(),
      amplifies: BoundedVec::default(),
      approves: BoundedVec::default(),
      reduces: BoundedVec::default(),
      vetoes: BoundedVec::default(),
      passes: BoundedVec::default(),
    });
    let aye_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.ayes,
    );
    let nay_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.nays,
    );
    let amplify_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.amplifies,
    );
    let approve_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.approves,
    );
    let reduce_weight = Self::proposal_vote_weight_sum(
      domain,
      item_id,
      current_epoch,
      primary_open_epoch,
      primary_close_epoch,
      &votes.reduces,
    );
    let veto_weight = Self::proposal_veto_weight_sum(
      domain,
      item_id,
      protection_current_epoch,
      protection_open_epoch,
      protection_close_epoch,
      &votes.vetoes,
    );
    let pass_weight = Self::proposal_veto_weight_sum(
      domain,
      item_id,
      protection_current_epoch,
      protection_open_epoch,
      protection_close_epoch,
      &votes.passes,
    );
    let turnout_weight = aye_weight
      .saturating_add(nay_weight)
      .saturating_add(amplify_weight)
      .saturating_add(approve_weight)
      .saturating_add(reduce_weight);
    let veto_turnout_weight = veto_weight.saturating_add(pass_weight);
    Some(ProposalVoteTally {
      aye_voters: votes.ayes.len() as u32,
      nay_voters: votes.nays.len() as u32,
      amplify_voters: votes.amplifies.len() as u32,
      approve_voters: votes.approves.len() as u32,
      reduce_voters: votes.reduces.len() as u32,
      veto_voters: votes.vetoes.len() as u32,
      pass_voters: votes.passes.len() as u32,
      aye_weight,
      nay_weight,
      amplify_weight,
      approve_weight,
      reduce_weight,
      veto_weight,
      pass_weight,
      turnout_weight,
      veto_turnout_weight,
    })
  }

  pub fn proposal_resolution_state(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalResolutionState<T::Epoch>> {
    let proposal = ActiveProposals::<T>::get(domain, item_id)?;
    let current_epoch = T::EpochProvider::current_epoch();
    let maturity_epoch =
      Self::proposal_effective_primary_close_epoch(domain, item_id, proposal.submitted_epoch)
        .ok()?;
    let tally = Self::do_proposal_vote_tally(domain, item_id)?;
    let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
      ayes: BoundedVec::default(),
      nays: BoundedVec::default(),
      amplifies: BoundedVec::default(),
      approves: BoundedVec::default(),
      reduces: BoundedVec::default(),
      vetoes: BoundedVec::default(),
      passes: BoundedVec::default(),
    });
    if let Some(cancellation) = Self::current_veto_cancellation(domain, item_id, &votes, false) {
      return Some(ProposalResolutionState::VetoPassing {
        veto_weight: cancellation.veto_weight,
        pass_weight: cancellation.pass_weight,
        mode: cancellation.mode,
      });
    }
    if current_epoch.saturated_into::<u32>() < maturity_epoch.saturated_into::<u32>() {
      return Some(ProposalResolutionState::VotingWindowOpen {
        current_epoch,
        maturity_epoch,
      });
    }
    let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
    if let Some(cancellation) =
      Self::current_veto_cancellation(domain, item_id, &votes, !urgent_authorized)
    {
      return Some(ProposalResolutionState::VetoPassing {
        veto_weight: cancellation.veto_weight,
        pass_weight: cancellation.pass_weight,
        mode: cancellation.mode,
      });
    }
    if tally.turnout_weight == 0 {
      return Some(ProposalResolutionState::Rejected {
        reason: ProposalRejectionReason::NoVotes,
      });
    }
    if tally.aye_weight == tally.nay_weight {
      return Some(ProposalResolutionState::Rejected {
        reason: ProposalRejectionReason::VoteTie,
      });
    }
    if tally.turnout_weight
      < Self::turnout_threshold_at(
        current_epoch,
        Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
          .ok()?,
        maturity_epoch,
      )
    {
      return Some(ProposalResolutionState::Rejected {
        reason: ProposalRejectionReason::TurnoutBelowMinimum,
      });
    }
    let approval_threshold = Self::approval_threshold_at(
      current_epoch,
      Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
        .ok()?,
      maturity_epoch,
    );
    let passing_state = match Self::do_proposal_primary_track_family(domain, item_id)? {
      crate::ProposalPrimaryTrackFamily::Binary => {
        if tally.aye_weight == tally.nay_weight {
          return Some(ProposalResolutionState::Rejected {
            reason: ProposalRejectionReason::VoteTie,
          });
        }
        let aye_approval = Perbill::from_rational(tally.aye_weight, tally.turnout_weight);
        let nay_approval = Perbill::from_rational(tally.nay_weight, tally.turnout_weight);
        if aye_approval >= approval_threshold {
          ProposalResolutionState::PassingAye
        } else if nay_approval >= approval_threshold {
          ProposalResolutionState::PassingNay
        } else {
          return Some(ProposalResolutionState::Rejected {
            reason: ProposalRejectionReason::ApprovalThresholdNotMet,
          });
        }
      }
      crate::ProposalPrimaryTrackFamily::Invoice => {
        let positive_weight = tally
          .amplify_weight
          .saturating_add(tally.approve_weight)
          .saturating_add(tally.reduce_weight);
        if positive_weight == tally.nay_weight {
          return Some(ProposalResolutionState::Rejected {
            reason: ProposalRejectionReason::VoteTie,
          });
        }
        if positive_weight < tally.nay_weight {
          return Some(ProposalResolutionState::Rejected {
            reason: ProposalRejectionReason::ApprovalThresholdNotMet,
          });
        }
        let positive_approval = Perbill::from_rational(positive_weight, tally.turnout_weight);
        if positive_approval < approval_threshold {
          return Some(ProposalResolutionState::Rejected {
            reason: ProposalRejectionReason::ApprovalThresholdNotMet,
          });
        }
        match Self::invoice_leading_positive_option(&tally).0 {
          Some(ProposalPrimaryTrackOption::Amplify) => ProposalResolutionState::PassingAmplify,
          Some(ProposalPrimaryTrackOption::Approve) => ProposalResolutionState::PassingApprove,
          Some(ProposalPrimaryTrackOption::Reduce) => ProposalResolutionState::PassingReduce,
          _ => {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::ApprovalThresholdNotMet,
            });
          }
        }
      }
    };
    if let Some(confirm_started) = ProposalConfirmStartedAt::<T>::get(domain, item_id) {
      let confirm_end_u32 = confirm_started
        .saturated_into::<u32>()
        .saturating_add(T::ProposalConfirmPeriod::get().saturated_into::<u32>());
      return Some(ProposalResolutionState::Confirming {
        confirm_started_epoch: confirm_started,
        confirm_end_epoch: confirm_end_u32.saturated_into(),
      });
    }
    Some(passing_state)
  }

  pub(crate) fn do_proposal_status(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalStatus<T::Epoch>> {
    if let Some(resolution_state) = Self::proposal_resolution_state(domain, item_id) {
      return Some(ProposalStatus::Active(resolution_state));
    }
    let outcome = Self::finalized_proposal_outcome(domain, item_id)?;
    if let Some(enactment_epoch) = ProposalPendingEnactmentAt::<T>::get(domain, item_id) {
      if T::EpochProvider::current_epoch().saturated_into::<u32>()
        < enactment_epoch.saturated_into::<u32>()
      {
        return Some(ProposalStatus::PendingEnactment {
          outcome,
          enactment_epoch,
        });
      }
    }
    Some(ProposalStatus::Finalized(outcome))
  }
}
