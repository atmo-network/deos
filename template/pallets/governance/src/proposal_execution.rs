use crate::*;
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::sp_runtime::traits::SaturatedConversion;
impl<T: Config> Pallet<T> {
  pub(crate) fn proposal_execution_authority_for_payload_kind(
    payload_kind: ProposalPayloadKind,
  ) -> ProposalExecutionAuthority {
    match payload_kind {
      ProposalPayloadKind::L1RootAction => ProposalExecutionAuthority::Root,
      ProposalPayloadKind::L2TreasurySpend => ProposalExecutionAuthority::DomainTreasury,
      ProposalPayloadKind::L2ParameterChange => ProposalExecutionAuthority::DomainParameters,
      ProposalPayloadKind::Intent => ProposalExecutionAuthority::NonExecutable,
      ProposalPayloadKind::L2SignalToL1 => ProposalExecutionAuthority::NonExecutable,
    }
  }
  pub(crate) fn do_proposal_execution_authority(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalExecutionAuthority> {
    ProposalMetadataByItem::<T>::get(domain, item_id)
      .map(|metadata| Self::proposal_execution_authority_for_payload_kind(metadata.payload_kind))
  }
  pub(crate) fn do_proposal_payload_availability(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<ProposalPayloadAvailability> {
    let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)?;
    Some(ProposalPayloadAvailability {
      have_preimage: T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash),
      preimage_requested: T::ProposalPayloadPreimageProvider::preimage_requested(
        &metadata.payload_hash,
      ),
    })
  }
  pub(crate) fn do_proposal_primary_track_family(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<crate::ProposalPrimaryTrackFamily> {
    ProposalMetadataByItem::<T>::get(domain, item_id)
      .map(|metadata| T::ProposalPrimaryTrackFamilyProvider::family(domain, metadata.payload_kind))
  }
  pub(crate) fn do_proposal_urgent_eligibility(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
  ) -> Option<bool> {
    ProposalMetadataByItem::<T>::get(domain, item_id).map(|metadata| {
      T::ProposalUrgentPolicyProvider::is_expeditable(domain, metadata.payload_kind)
    })
  }
  /// Voting-window progress clamped to `[0, Perbill::one()]`.
  pub(crate) fn record_finalized_proposal_outcome(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    outcome: FinalizedProposalOutcome<T::Epoch>,
    current_epoch: T::Epoch,
  ) -> DispatchResult {
    FinalizedProposalOutcomes::<T>::insert(domain, item_id, outcome);
    let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
    let expiry_epoch_u32 = current_epoch
      .saturated_into::<u32>()
      .checked_add(retention)
      .ok_or(Error::<T>::EpochArithmeticOverflow)?;
    let expiry_epoch: T::Epoch = expiry_epoch_u32.saturated_into();
    FinalizedProposalOutcomeExpiryBuckets::<T>::try_mutate(
      expiry_epoch,
      |bucket| -> DispatchResult {
        let exists = bucket
          .iter()
          .any(|entry| entry.domain == domain && entry.item_id == item_id);
        if !exists {
          bucket
            .try_push(FinalizedProposalTouch { domain, item_id })
            .map_err(|_| Error::<T>::FinalizedProposalOutcomeExpiryBucketFull)?;
        }
        Ok(())
      },
    )
  }
  pub(crate) fn schedule_pending_enactment_if_needed(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    finalized_epoch: T::Epoch,
  ) -> Result<bool, DispatchError> {
    let enactment_delay = T::ProposalEnactmentDelay::get();
    if enactment_delay.saturated_into::<u32>() == 0 {
      ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
      return Ok(false);
    }
    let enactment_epoch = Self::add_epochs(finalized_epoch, enactment_delay)?;
    let touch = FinalizedProposalTouch { domain, item_id };
    ProposalPendingEnactmentAt::<T>::insert(domain, item_id, enactment_epoch);
    PendingEnactmentBuckets::<T>::try_mutate(enactment_epoch, |bucket| -> DispatchResult {
      if !bucket.contains(&touch) {
        bucket
          .try_push(touch.clone())
          .map_err(|_| Error::<T>::PendingEnactmentBucketFull)?;
      }
      Ok(())
    })?;
    Self::deposit_event(Event::ProposalEnactmentScheduled {
      domain,
      item_id,
      finalized_epoch,
      enactment_epoch,
    });
    Ok(true)
  }
  pub(crate) fn set_finalized_proposal_outcome(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    outcome: FinalizedProposalOutcome<T::Epoch>,
  ) {
    FinalizedProposalOutcomes::<T>::insert(domain, item_id, outcome);
  }
  pub(crate) fn set_proposal_execution_detail(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    detail: crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash, T::Epoch>,
  ) {
    ProposalExecutionDetails::<T>::insert(domain, item_id, detail);
  }
  pub(crate) fn maybe_execute_proposal_payload(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    approved_epoch: T::Epoch,
    winner_count: u32,
    execution_epoch: T::Epoch,
  ) -> DispatchResult {
    let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)
      .ok_or(Error::<T>::ProposalMetadataMissing)?;
    match metadata.payload_kind {
      ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1 => {
        Self::set_finalized_proposal_outcome(
          domain,
          item_id,
          FinalizedProposalOutcome::AdvisoryFinalized {
            approved_epoch,
            finalized_epoch: execution_epoch,
            winner_count,
          },
        );
        ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
        Self::set_proposal_execution_detail(
          domain,
          item_id,
          crate::ProposalExecutionDetail::AdvisoryFinalized {
            payload_kind: metadata.payload_kind,
            finalized_epoch: execution_epoch,
          },
        );
        Self::deposit_event(Event::ProposalAdvisoryFinalized {
          domain,
          item_id,
          approved_epoch,
          finalized_epoch: execution_epoch,
          payload_kind: metadata.payload_kind,
        });
        Ok(())
      }
      _ => {
        if !T::ProposalPayloadExecutor::can_execute(metadata.payload_kind) {
          return Ok(());
        }
        let authority = Self::proposal_execution_authority_for_payload_kind(metadata.payload_kind);
        if !T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash) {
          Self::set_finalized_proposal_outcome(
            domain,
            item_id,
            FinalizedProposalOutcome::ExecutionFailed {
              approved_epoch,
              failed_epoch: execution_epoch,
              winner_count,
            },
          );
          ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
          Self::set_proposal_execution_detail(
            domain,
            item_id,
            crate::ProposalExecutionDetail::ExecutionFailed {
              payload_kind: metadata.payload_kind,
              authority,
              failed_epoch: execution_epoch,
              reason: crate::ProposalExecutionFailureReason::MissingPreimage,
            },
          );
          Self::deposit_event(Event::ProposalExecutionFailed {
            domain,
            item_id,
            approved_epoch,
            failed_epoch: execution_epoch,
            authority,
            payload_kind: metadata.payload_kind,
            reason: crate::ProposalExecutionFailureReason::MissingPreimage,
          });
          return Ok(());
        }
        match T::ProposalPayloadExecutor::execute(
          domain,
          item_id,
          metadata.payload_kind,
          metadata.payload_hash,
        ) {
          Ok(receipt) => {
            Self::set_finalized_proposal_outcome(
              domain,
              item_id,
              FinalizedProposalOutcome::Enacted {
                approved_epoch,
                executed_epoch: execution_epoch,
                winner_count,
              },
            );
            ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
            let execution_detail = match receipt {
              crate::ProposalExecutionReceipt::Generic => {
                crate::ProposalExecutionSuccessDetail::Generic
              }
              crate::ProposalExecutionReceipt::RuntimeUpgradeAuthorized { code_hash } => {
                Self::deposit_event(Event::ProposalRuntimeUpgradeAuthorized {
                  domain,
                  item_id,
                  approved_epoch,
                  executed_epoch: execution_epoch,
                  code_hash,
                });
                crate::ProposalExecutionSuccessDetail::RuntimeUpgradeAuthorized { code_hash }
              }
              crate::ProposalExecutionReceipt::ParameterChangeExecuted { surface } => {
                Self::deposit_event(Event::ProposalParameterChangeExecuted {
                  domain,
                  item_id,
                  approved_epoch,
                  executed_epoch: execution_epoch,
                  surface,
                });
                crate::ProposalExecutionSuccessDetail::ParameterChangeExecuted { surface }
              }
              crate::ProposalExecutionReceipt::TreasurySpendExecuted {
                funding_source,
                beneficiary,
                payout_asset,
                base_amount,
                scalar,
                final_amount,
                settlement_kind,
              } => {
                Self::deposit_event(Event::ProposalTreasurySpendExecuted {
                  domain,
                  item_id,
                  approved_epoch,
                  executed_epoch: execution_epoch,
                  funding_source: funding_source.clone(),
                  beneficiary: beneficiary.clone(),
                  payout_asset,
                  base_amount,
                  scalar,
                  final_amount,
                  settlement_kind,
                });
                crate::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
                  funding_source,
                  beneficiary,
                  payout_asset,
                  base_amount,
                  scalar,
                  final_amount,
                  settlement_kind,
                }
              }
            };
            Self::set_proposal_execution_detail(
              domain,
              item_id,
              crate::ProposalExecutionDetail::Executed {
                payload_kind: metadata.payload_kind,
                authority,
                executed_epoch: execution_epoch,
                detail: execution_detail,
              },
            );
            Self::deposit_event(Event::ProposalExecuted {
              domain,
              item_id,
              approved_epoch,
              executed_epoch: execution_epoch,
              authority,
              payload_kind: metadata.payload_kind,
            });
            Ok(())
          }
          Err(reason) => {
            Self::set_finalized_proposal_outcome(
              domain,
              item_id,
              FinalizedProposalOutcome::ExecutionFailed {
                approved_epoch,
                failed_epoch: execution_epoch,
                winner_count,
              },
            );
            ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
            Self::set_proposal_execution_detail(
              domain,
              item_id,
              crate::ProposalExecutionDetail::ExecutionFailed {
                payload_kind: metadata.payload_kind,
                authority,
                failed_epoch: execution_epoch,
                reason,
              },
            );
            Self::deposit_event(Event::ProposalExecutionFailed {
              domain,
              item_id,
              approved_epoch,
              failed_epoch: execution_epoch,
              authority,
              payload_kind: metadata.payload_kind,
              reason,
            });
            Ok(())
          }
        }
      }
    }
  }
  pub(crate) fn finalized_outcome_epoch(outcome: &FinalizedProposalOutcome<T::Epoch>) -> T::Epoch {
    match outcome {
      FinalizedProposalOutcome::Resolved { epoch, .. }
      | FinalizedProposalOutcome::Rejected { epoch, .. }
      | FinalizedProposalOutcome::VetoCancelled { epoch, .. } => *epoch,
      FinalizedProposalOutcome::Enacted { executed_epoch, .. } => *executed_epoch,
      FinalizedProposalOutcome::ExecutionFailed { failed_epoch, .. } => *failed_epoch,
      FinalizedProposalOutcome::AdvisoryFinalized {
        finalized_epoch, ..
      } => *finalized_epoch,
    }
  }
  pub(crate) fn do_recent_finalized_proposals(
    domain: T::DomainId,
  ) -> BoundedVec<
    RecentFinalizedProposal<T::WinningVoteItemId, T::Epoch>,
    T::MaxRecentFinalizedProposalsPerDomain,
  > {
    let mut proposals = FinalizedProposalOutcomes::<T>::iter_prefix(domain)
      .map(|(item_id, outcome)| RecentFinalizedProposal { item_id, outcome })
      .collect::<Vec<_>>();
    proposals.sort_by(|left, right| {
      Self::finalized_outcome_epoch(&right.outcome)
        .cmp(&Self::finalized_outcome_epoch(&left.outcome))
        .then_with(|| left.item_id.cmp(&right.item_id))
    });
    proposals
      .into_iter()
      .fold(BoundedVec::default(), |mut bounded, proposal| {
        let push_result = bounded.try_push(proposal);
        if push_result.is_err() {
          panic!("recent finalized proposal query bound must cover retained outcomes")
        }
        bounded
      })
  }
}
