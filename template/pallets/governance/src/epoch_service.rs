use crate::*;

use frame::prelude::*;
use polkadot_sdk::frame_support::transactional;
use polkadot_sdk::sp_runtime::traits::SaturatedConversion;

pub(crate) struct ProposalAdmission<Epoch, Balance> {
  pub active_count_after: u32,
  pub maturity_epoch: Epoch,
  pub opening_fee: Option<Balance>,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn proposal_preimage_admission_error(
    error: ProposalPreimageAdmissionError,
  ) -> Error<T> {
    match error {
      ProposalPreimageAdmissionError::Missing => Error::<T>::ProposalPreimageMissing,
      ProposalPreimageAdmissionError::Oversized => Error::<T>::ProposalPreimageOversized,
      ProposalPreimageAdmissionError::Invalid => Error::<T>::ProposalPreimageInvalid,
      ProposalPreimageAdmissionError::Incompatible => Error::<T>::ProposalPreimageIncompatible,
    }
  }

  pub(crate) fn proposal_admission_policy(
    domain: T::DomainId,
    payload_kind: ProposalPayloadKind,
  ) -> ProposalAdmissionPolicyView<BalanceOf<T>> {
    let authority = T::ProposalSubmissionAuthorityProvider::authority(domain, payload_kind);
    let capacity_lane = if payload_kind == ProposalPayloadKind::L1RootAction
      && authority == ProposalSubmissionAuthority::PrimaryEligibleSigned
    {
      ProposalCapacityLane::Strategic
    } else {
      ProposalCapacityLane::General
    };
    let maximum = T::MaxActiveProposalsPerDomain::get();
    ProposalAdmissionPolicyView {
      authority,
      capacity_lane,
      capacity_limit: match capacity_lane {
        ProposalCapacityLane::Strategic => maximum,
        ProposalCapacityLane::General => maximum.saturating_sub(T::StrategicProposalReserve::get()),
      },
      author_limit: T::MaxActiveProposalsPerAuthor::get(),
      signed_preimage_required: authority != ProposalSubmissionAuthority::AdminOnly,
      opening_fee: (authority != ProposalSubmissionAuthority::AdminOnly)
        .then(T::ProposalOpeningFee::get),
    }
  }

  fn active_proposal_count_for_author(
    domain: T::DomainId,
    proposer: &T::AccountId,
  ) -> Result<u32, DispatchError> {
    ActiveProposalIdsByDomain::<T>::get(domain)
      .iter() // deos-bypass: bounded-iter — MaxActiveProposalsPerDomain ids
      .try_fold(0u32, |count, item_id| {
        let author = ProposalAuthorsByItem::<T>::get(domain, item_id)
          .ok_or(Error::<T>::ActiveProposalAuthorMissing)?;
        Ok(count.saturating_add(u32::from(author == *proposer)))
      })
  }

  pub(crate) fn classify_proposal_admission(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    proposer: &T::AccountId,
    payload_kind: ProposalPayloadKind,
    payload_hash: &T::Hash,
    signed: bool,
  ) -> Result<ProposalAdmission<T::Epoch, BalanceOf<T>>, DispatchError> {
    let policy = Self::proposal_admission_policy(domain, payload_kind);
    if signed {
      ensure!(
        policy.authority != ProposalSubmissionAuthority::AdminOnly,
        Error::<T>::ProposalSubmissionNotAllowedForSignedOrigin
      );
      if policy.authority == ProposalSubmissionAuthority::PrimaryEligibleSigned {
        ensure!(
          T::ProposalSubmissionEligibilityProvider::has_primary_governance_power(domain, proposer,),
          Error::<T>::ProposalSubmitterNotPrimaryEligible
        );
      }
      T::ProposalPayloadPreimageProvider::validate_for_submission(
        domain,
        payload_kind,
        payload_hash,
      )
      .map_err(Self::proposal_preimage_admission_error)?;
    }
    ensure!(
      !ActiveProposals::<T>::contains_key(domain, item_id),
      Error::<T>::ProposalAlreadyActive
    );
    let active_count = ActiveProposalIdsByDomain::<T>::decode_len(domain).unwrap_or(0) as u32;
    ensure!(
      active_count < policy.capacity_limit,
      Error::<T>::ActiveProposalCapReached
    );
    ensure!(
      Self::active_proposal_count_for_author(domain, proposer)?
        < T::MaxActiveProposalsPerAuthor::get(),
      Error::<T>::ActiveProposalAuthorCapReached
    );
    let maturity_epoch = Self::proposal_maturity_epoch(T::EpochProvider::current_epoch())?;
    Self::ensure_proposal_maturity_capacity(maturity_epoch, domain, item_id)?;
    Ok(ProposalAdmission {
      active_count_after: active_count.saturating_add(1),
      maturity_epoch,
      opening_fee: signed.then_some(policy.opening_fee).flatten(),
    })
  }

  #[transactional]
  pub fn submit_active_proposal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    proposer: T::AccountId,
    metadata: ProposalMetadata<T::Hash>,
    admission: ProposalAdmission<T::Epoch, BalanceOf<T>>,
  ) -> DispatchResult {
    let current_epoch = T::EpochProvider::current_epoch();
    let maturity_epoch = admission.maturity_epoch;
    let active_count = admission.active_count_after;
    ActiveProposals::<T>::insert(
      domain,
      item_id,
      ActiveProposal {
        submitted_epoch: current_epoch,
      },
    );
    ProposalAuthorsByItem::<T>::insert(domain, item_id, proposer.clone());
    ProposalMetadataByItem::<T>::insert(domain, item_id, metadata.clone());
    Self::note_authored_proposal(domain, &proposer);
    Self::insert_active_proposal_id(domain, item_id)?;
    Self::schedule_proposal_maturity_at(maturity_epoch, domain, item_id)?;
    Self::deposit_event(Event::ProposalSubmitted {
      domain,
      item_id,
      proposer,
      cadence_mode: metadata.cadence_mode,
      payload_kind: metadata.payload_kind,
      payload_hash: metadata.payload_hash,
      epoch: current_epoch,
      active_count,
    });
    Ok(())
  }

  pub(crate) fn add_epochs(
    base_epoch: T::Epoch,
    delta: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    let result_epoch_u32 = base_epoch
      .saturated_into::<u32>()
      .checked_add(delta.saturated_into::<u32>())
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    Ok(result_epoch_u32.saturated_into())
  }

  pub(crate) fn proposal_maturity_epoch(
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    let voting_period = T::ProposalVotingPeriod::get().saturated_into::<u32>();
    ensure!(voting_period > 0, Error::<T>::ZeroProposalVotingPeriod);
    Self::proposal_ordinary_primary_close_epoch(submitted_epoch)
  }

  pub(crate) fn proposal_ordinary_primary_open_epoch(
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    Self::add_epochs(submitted_epoch, T::ProposalLeadInPeriod::get())
  }

  pub(crate) fn proposal_ordinary_primary_close_epoch(
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    let primary_open_epoch = Self::proposal_ordinary_primary_open_epoch(submitted_epoch)?;
    Self::add_epochs(primary_open_epoch, T::ProposalVotingPeriod::get())
  }

  pub(crate) fn proposal_effective_primary_open_epoch(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    Ok(
      ProposalUrgentAuthorizedAt::<T>::get(domain, item_id)
        .unwrap_or(Self::proposal_ordinary_primary_open_epoch(submitted_epoch)?),
    )
  }

  pub(crate) fn proposal_effective_primary_close_epoch(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    if let Some(urgent_primary_open_epoch) = ProposalUrgentAuthorizedAt::<T>::get(domain, item_id) {
      return Self::proposal_urgent_primary_close_epoch(urgent_primary_open_epoch);
    }
    Self::proposal_ordinary_primary_close_epoch(submitted_epoch)
  }

  pub(crate) fn proposal_protection_close_epoch(
    submitted_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    Self::add_epochs(submitted_epoch, T::ProposalProtectionPeriod::get())
  }

  pub(crate) fn proposal_urgent_primary_close_epoch(
    urgent_primary_open_epoch: T::Epoch,
  ) -> Result<T::Epoch, DispatchError> {
    Self::add_epochs(
      urgent_primary_open_epoch,
      T::ProposalUrgentVotingPeriod::get(),
    )
  }

  pub(crate) fn do_proposal_timing(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalTiming<T::Epoch>> {
    let proposal = ActiveProposals::<T>::get(domain, item_id)?;
    let submitted_epoch = proposal.submitted_epoch;
    let protection_open_epoch = submitted_epoch;
    let protection_close_epoch = Self::proposal_protection_close_epoch(submitted_epoch).ok()?;
    let ordinary_primary_open_epoch =
      Self::proposal_ordinary_primary_open_epoch(submitted_epoch).ok()?;
    let ordinary_primary_close_epoch =
      Self::proposal_ordinary_primary_close_epoch(submitted_epoch).ok()?;
    let urgent_primary_open_epoch = ProposalUrgentAuthorizedAt::<T>::get(domain, item_id);
    let urgent_primary_close_epoch = urgent_primary_open_epoch.and_then(|urgent_open_epoch| {
      Self::proposal_urgent_primary_close_epoch(urgent_open_epoch).ok()
    });
    let effective_primary_open_epoch =
      urgent_primary_open_epoch.unwrap_or(ordinary_primary_open_epoch);
    let effective_primary_close_epoch =
      urgent_primary_close_epoch.unwrap_or(ordinary_primary_close_epoch);
    let pending_enactment_epoch = ProposalPendingEnactmentAt::<T>::get(domain, item_id);
    Some(ProposalTiming {
      submitted_epoch,
      protection_open_epoch,
      protection_close_epoch,
      ordinary_primary_open_epoch,
      ordinary_primary_close_epoch,
      urgent_primary_open_epoch,
      urgent_primary_close_epoch,
      effective_primary_open_epoch,
      effective_primary_close_epoch,
      pending_enactment_epoch,
    })
  }

  fn ensure_proposal_maturity_capacity(
    maturity_epoch: T::Epoch,
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    let bucket = ProposalMaturityBuckets::<T>::get(maturity_epoch);
    let exists = bucket
      .iter() // deos-bypass: bounded-iter — MaxMaturitiesPerEpoch bucket
      .any(|entry| entry.domain == domain && entry.item_id == item_id);
    ensure!(
      exists || (bucket.len() as u32) < T::MaxMaturingProposalsPerEpoch::get(),
      Error::<T>::ProposalMaturityBucketFull
    );
    Ok(())
  }

  pub(crate) fn schedule_proposal_maturity_at(
    maturity_epoch: T::Epoch,
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    ProposalMaturityBuckets::<T>::try_mutate(maturity_epoch, |bucket| -> DispatchResult {
      let exists = bucket
        .iter() // deos-bypass: bounded-iter — MaxMaturitiesPerEpoch bucket
        .any(|entry| entry.domain == domain && entry.item_id == item_id);
      if !exists {
        bucket
          .try_push(MaturingProposalTouch { domain, item_id })
          .map_err(|_| Error::<T>::ProposalMaturityBucketFull)?;
      }
      Ok(())
    })
  }

  pub(crate) fn insert_active_proposal_id(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    ActiveProposalIdsByDomain::<T>::try_mutate(domain, |item_ids| -> DispatchResult {
      if item_ids
        .iter() // deos-bypass: bounded-iter — MaxActiveProposalsPerDomain ids
        .all(|existing| *existing != item_id)
      {
        item_ids
          .try_push(item_id)
          .map_err(|_| Error::<T>::ActiveProposalCapReached)?;
      }
      Ok(())
    })
  }

  pub(crate) fn remove_active_proposal_id(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Result<u32, DispatchError> {
    ActiveProposalIdsByDomain::<T>::try_mutate(domain, |item_ids| {
      let position = item_ids
        .iter() // deos-bypass: bounded-iter — MaxActiveProposalsPerDomain ids
        .position(|existing| *existing == item_id)
        .ok_or(Error::<T>::ActiveProposalCountUnderflow)?;
      item_ids.remove(position);
      Ok(item_ids.len() as u32)
    })
  }

  pub fn requeue_active_proposal_for_auto_finalization(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> DispatchResult {
    let proposal =
      ActiveProposals::<T>::get(domain, item_id).ok_or(Error::<T>::ProposalNotActive)?;
    let current_epoch = T::EpochProvider::current_epoch();
    let natural_maturity_epoch = Self::proposal_maturity_epoch(proposal.submitted_epoch)?;
    let target_epoch_u32 = current_epoch
      .saturated_into::<u32>()
      .saturating_add(1)
      .max(natural_maturity_epoch.saturated_into::<u32>());
    let maturity_epoch: T::Epoch = target_epoch_u32.saturated_into();
    Self::schedule_proposal_maturity_at(maturity_epoch, domain, item_id)?;
    Self::deposit_event(Event::ProposalAutoFinalizationRequeued {
      domain,
      item_id,
      epoch: current_epoch,
      maturity_epoch,
    });
    Ok(())
  }

  #[transactional]
  pub(crate) fn schedule_expiry(
    domain: T::DomainId,
    account: &T::AccountId,
    current_epoch: T::Epoch,
  ) -> DispatchResult {
    let lookback = T::WinningVoteLookbackEpochs::get();
    ensure!(lookback > 0, Error::<T>::ZeroLookbackWindow);
    let expiry_epoch_u32 = current_epoch
      .saturated_into::<u32>()
      .checked_add(lookback)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    let expiry_epoch: T::Epoch = expiry_epoch_u32.saturated_into();
    ExpiryBuckets::<T>::try_mutate(expiry_epoch, |bucket| -> DispatchResult {
      let exists = bucket
        .iter() // deos-bypass: bounded-iter — MaxExpiriesPerEpoch bucket
        .any(|entry| entry.domain == domain && entry.account == *account);
      if !exists {
        bucket
          .try_push(ExpiringAccountTouch {
            domain,
            account: account.clone(),
          })
          .map_err(|_| Error::<T>::ExpiryBucketFull)?;
      }
      Ok(())
    })
  }

  pub(crate) fn service_current_epoch(current_epoch: T::Epoch) -> Weight {
    let last_processed_epoch = LastProcessedEpoch::<T>::get().saturated_into::<u32>();
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    if current_epoch_u32 <= last_processed_epoch {
      return Weight::zero();
    }
    let maturing_weight = Self::service_maturing_proposals(last_processed_epoch, current_epoch);
    let pending_enactment_weight =
      Self::service_pending_enactments(last_processed_epoch, current_epoch);
    let finalized_weight =
      Self::service_finalized_proposal_outcomes(last_processed_epoch, current_epoch);
    let expiring_weight = Self::service_expiring_accounts(last_processed_epoch, current_epoch);
    LastProcessedEpoch::<T>::put(current_epoch);
    maturing_weight
      .saturating_add(pending_enactment_weight)
      .saturating_add(finalized_weight)
      .saturating_add(expiring_weight)
  }

  pub(crate) fn service_maturing_proposals(
    last_processed_epoch: u32,
    current_epoch: T::Epoch,
  ) -> Weight {
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    let confirm_period = T::ProposalConfirmPeriod::get().saturated_into::<u32>();
    let mut processed_entries = 0u32;
    for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
      let epoch: T::Epoch = epoch_u32.saturated_into();
      let bucket = ProposalMaturityBuckets::<T>::take(epoch);
      processed_entries = processed_entries.saturating_add(bucket.len() as u32);
      for touch in bucket {
        if !ActiveProposals::<T>::contains_key(touch.domain, touch.item_id) {
          ProposalConfirmStartedAt::<T>::remove(touch.domain, touch.item_id);
          continue;
        }
        if confirm_period == 0 {
          if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
            continue;
          }
        } else if ProposalConfirmStartedAt::<T>::contains_key(touch.domain, touch.item_id) {
          // Confirm-end: proposal sustained approval for the full confirm period
          ProposalConfirmStartedAt::<T>::remove(touch.domain, touch.item_id);
          if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
            continue;
          }
        } else {
          // First maturity: check if currently passing → enter confirm
          match Self::proposal_resolution_state(touch.domain, touch.item_id) {
            Some(ProposalResolutionState::PassingAye)
            | Some(ProposalResolutionState::PassingAmplify)
            | Some(ProposalResolutionState::PassingApprove)
            | Some(ProposalResolutionState::PassingReduce)
            | Some(ProposalResolutionState::PassingNay) => {
              let confirm_end_epoch_u32 = epoch_u32.saturating_add(confirm_period);
              ProposalConfirmStartedAt::<T>::insert(touch.domain, touch.item_id, epoch);
              let rescheduled = Self::schedule_proposal_maturity_at(
                confirm_end_epoch_u32.saturated_into(),
                touch.domain,
                touch.item_id,
              )
              .is_ok();
              if rescheduled {
                Self::deposit_event(Event::ProposalConfirmStarted {
                  domain: touch.domain,
                  item_id: touch.item_id,
                  confirm_started_epoch: epoch,
                  confirm_end_epoch: confirm_end_epoch_u32.saturated_into(),
                });
              }
              continue;
            }
            Some(ProposalResolutionState::VetoPassing { .. }) => {
              if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
                continue;
              }
            }
            _ => {
              if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
                continue;
              }
            }
          }
        }
        let next_epoch_u32 = epoch_u32.saturating_add(1);
        let rescheduled = Self::schedule_proposal_maturity_at(
          next_epoch_u32.saturated_into(),
          touch.domain,
          touch.item_id,
        )
        .is_ok();
        Self::deposit_event(Event::ProposalAutoFinalizationDeferred {
          domain: touch.domain,
          item_id: touch.item_id,
          epoch: current_epoch,
          rescheduled,
        });
      }
    }
    T::WeightInfo::service_maturing_proposals(processed_entries)
  }

  pub(crate) fn service_pending_enactments(
    last_processed_epoch: u32,
    current_epoch: T::Epoch,
  ) -> Weight {
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    let mut processed_entries = 0u32;
    for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
      let epoch: T::Epoch = epoch_u32.saturated_into();
      let bucket = PendingEnactmentBuckets::<T>::take(epoch);
      processed_entries = processed_entries.saturating_add(bucket.len() as u32);
      for touch in bucket {
        let Some(enactment_epoch) =
          ProposalPendingEnactmentAt::<T>::get(touch.domain, touch.item_id)
        else {
          continue;
        };
        if enactment_epoch != epoch {
          continue;
        }
        let Some(finalization) = FinalizedProposals::<T>::get(touch.domain, touch.item_id) else {
          ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
          continue;
        };
        let FinalizedProposalOutcome::Approved {
          approval,
          enactment: ProposalEnactmentOutcome::NotAttempted,
        } = finalization.outcome
        else {
          ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
          continue;
        };
        let approved_epoch = approval.approved_epoch;
        let winner_count = approval.winner_count;
        let execution_attempt = Self::maybe_execute_proposal_payload(
          touch.domain,
          touch.item_id,
          approved_epoch,
          winner_count,
          current_epoch,
        );
        if execution_attempt.is_err()
          || ProposalPendingEnactmentAt::<T>::contains_key(touch.domain, touch.item_id)
        {
          let next_epoch_u32 = epoch_u32.saturating_add(1);
          let next_epoch: T::Epoch = next_epoch_u32.saturated_into();
          let next_touch = FinalizedProposalTouch {
            domain: touch.domain,
            item_id: touch.item_id,
          };
          let _ =
            PendingEnactmentBuckets::<T>::try_mutate(next_epoch, |next_bucket| -> DispatchResult {
              if !next_bucket.contains(&next_touch) {
                next_bucket
                  .try_push(next_touch.clone())
                  .map_err(|_| Error::<T>::PendingEnactmentBucketFull)?;
              }
              Ok(())
            });
        }
      }
    }
    T::WeightInfo::service_finalized_proposal_outcomes(processed_entries)
  }

  pub(crate) fn service_finalized_proposal_outcomes(
    last_processed_epoch: u32,
    current_epoch: T::Epoch,
  ) -> Weight {
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    let mut processed_entries = 0u32;
    for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
      let epoch: T::Epoch = epoch_u32.saturated_into();
      let bucket = FinalizedProposalOutcomeExpiryBuckets::<T>::take(epoch);
      processed_entries = processed_entries.saturating_add(bucket.len() as u32);
      for touch in bucket {
        FinalizedProposals::<T>::remove(touch.domain, touch.item_id);
        ProposalMetadataByItem::<T>::remove(touch.domain, touch.item_id);
        ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
        ProposalWinningPrimaryOptionByItem::<T>::remove(touch.domain, touch.item_id);
        ProposalUrgentAuthorizedAt::<T>::remove(touch.domain, touch.item_id);
      }
    }
    T::WeightInfo::service_finalized_proposal_outcomes(processed_entries)
  }

  pub(crate) fn service_expiring_accounts(
    last_processed_epoch: u32,
    current_epoch: T::Epoch,
  ) -> Weight {
    let current_epoch_u32 = current_epoch.saturated_into::<u32>();
    let mut processed_entries = 0u32;
    for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
      let epoch: T::Epoch = epoch_u32.saturated_into();
      let bucket = ExpiryBuckets::<T>::take(epoch);
      processed_entries = processed_entries.saturating_add(bucket.len() as u32);
      for touch in bucket {
        let evicted =
          WinningVoteWindows::<T>::mutate_exists(touch.domain, &touch.account, |maybe_window| {
            let Some(window) = maybe_window.as_mut() else {
              return false;
            };
            Self::rotate_window_to(window, current_epoch);
            if window.rolling_sum == 0 {
              *maybe_window = None;
              return true;
            }
            false
          });
        if evicted {
          Self::deposit_event(Event::WinningVoteWindowEvicted {
            domain: touch.domain,
            account: touch.account,
            epoch: current_epoch,
          });
        }
      }
    }
    T::WeightInfo::service_expiring_accounts(processed_entries)
  }
}
