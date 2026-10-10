use crate::*;
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::frame_support::transactional;
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
  ) -> Option<PayloadHashPreimageStatus> {
    let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)?;
    Some(PayloadHashPreimageStatus {
      have_preimage: T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash),
      preimage_requested: T::ProposalPayloadPreimageProvider::preimage_requested(
        &metadata.payload_hash,
      ),
      payload_len: T::ProposalPayloadPreimageProvider::preimage_len(&metadata.payload_hash),
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
    ensure!(
      !FinalizedProposals::<T>::contains_key(domain, item_id),
      Error::<T>::ProposalAlreadyFinalized
    );
    FinalizedProposalIdsByDomain::<T>::try_mutate(domain, |ids| -> DispatchResult {
      ensure!(
        !ids.contains(&item_id),
        Error::<T>::ProposalAlreadyFinalized
      );
      ids
        .try_push(item_id)
        .map_err(|_| Error::<T>::FinalizedProposalIndexFull)?;
      Ok(())
    })?;
    FinalizedProposals::<T>::insert(
      domain,
      item_id,
      FinalizedProposalRecord {
        outcome: outcome.clone(),
        execution_detail: None,
      },
    );
    if matches!(
      outcome,
      FinalizedProposalOutcome::Approved {
        enactment: ProposalEnactmentOutcome::NotAttempted,
        ..
      }
    ) {
      return Ok(());
    }
    Self::admit_terminal_cleanup(domain, item_id, current_epoch)
  }

  fn admit_terminal_cleanup(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    current_epoch: T::Epoch,
  ) -> DispatchResult {
    let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
    let expiry_epoch = Self::add_epochs(current_epoch, retention.into())?;
    FinalizedProposalOutcomeExpiryBuckets::<T>::try_mutate(
      expiry_epoch,
      |bucket| -> DispatchResult {
        let exists = bucket
          .iter() // deos-bypass: bounded-iter — finalized-outcome expiry bucket
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
    if enactment_delay.is_zero() {
      Self::release_pending_enactment(domain, item_id);
      return Ok(false);
    }
    let enactment_epoch = Self::add_epochs(finalized_epoch, enactment_delay)?;
    Self::admit_pending_enactment(domain, item_id, enactment_epoch)?;
    Self::deposit_event(Event::ProposalEnactmentScheduled {
      domain,
      item_id,
      finalized_epoch,
      enactment_epoch,
    });
    Ok(true)
  }
  pub(crate) fn admit_pending_enactment(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    due: T::Epoch,
  ) -> DispatchResult {
    let touch = FinalizedProposalTouch { domain, item_id };
    PendingEnactments::<T>::try_mutate(|members| -> DispatchResult {
      ensure!(
        !members.contains(&touch),
        Error::<T>::ProposalAlreadyFinalized
      );
      members
        .try_push(touch)
        .map_err(|_| Error::<T>::PendingEnactmentsFull)?;
      Ok(())
    })?;
    ProposalPendingEnactmentAt::<T>::insert(domain, item_id, due);
    Ok(())
  }

  fn release_pending_enactment(domain: T::DomainId, item_id: T::WinningVoteItemId) {
    ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
    PendingEnactments::<T>::mutate_exists(|maybe_members| {
      if let Some(members) = maybe_members {
        if let Some(index) = members
          .iter() // deos-bypass: bounded-iter — MaxPendingEnactments members
          .position(|touch| touch.domain == domain && touch.item_id == item_id)
        {
          members.remove(index);
          let cursor = PendingEnactmentCursor::<T>::get() as usize;
          if members.is_empty() {
            *maybe_members = None;
            PendingEnactmentCursor::<T>::kill();
          } else {
            PendingEnactmentCursor::<T>::put(
              (cursor.saturating_sub(usize::from(index < cursor)) % members.len()) as u32,
            );
          }
        }
      }
    });
  }

  pub(crate) fn update_finalized_proposal(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    outcome: FinalizedProposalOutcome<T::Epoch>,
    execution_detail: Option<
      crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash, crate::CidBytesOf<T>>,
    >,
  ) -> DispatchResult {
    FinalizedProposals::<T>::try_mutate(domain, item_id, |maybe_record| -> DispatchResult {
      let record = maybe_record
        .as_mut()
        .ok_or(Error::<T>::FinalizedProposalMissing)?;
      record.outcome = outcome;
      record.execution_detail = execution_detail;
      Ok(())
    })
  }
  #[transactional]
  pub(crate) fn maybe_execute_proposal_payload(
    domain: T::DomainId,
    item_id: T::WinningVoteItemId,
    approved_epoch: T::Epoch,
    winner_count: u32,
    execution_epoch: T::Epoch,
  ) -> DispatchResult {
    let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)
      .ok_or(Error::<T>::ProposalMetadataMissing)?;
    ensure!(
      matches!(FinalizedProposals::<T>::get(domain, item_id).map(|record| record.outcome),
        Some(FinalizedProposalOutcome::Approved {
          approval,
          enactment: ProposalEnactmentOutcome::NotAttempted,
        }) if approval.approved_epoch == approved_epoch && approval.winner_count == winner_count),
      Error::<T>::ProposalExecutionNotPending,
    );
    if !matches!(
      metadata.payload_kind,
      ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1
    ) && !T::ProposalPayloadExecutor::can_execute(metadata.payload_kind)
    {
      return Ok(());
    }
    Self::admit_terminal_cleanup(domain, item_id, execution_epoch)?;
    match metadata.payload_kind {
      ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1 => {
        Self::update_finalized_proposal(
          domain,
          item_id,
          FinalizedProposalOutcome::Approved {
            approval: ProposalApproval {
              approved_epoch,
              winner_count,
            },
            enactment: ProposalEnactmentOutcome::AdvisoryFinalized {
              epoch: execution_epoch,
            },
          },
          None,
        )?;
        Self::release_pending_enactment(domain, item_id);
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
        let authority = Self::proposal_execution_authority_for_payload_kind(metadata.payload_kind);
        if !T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash) {
          Self::update_finalized_proposal(
            domain,
            item_id,
            FinalizedProposalOutcome::Approved {
              approval: ProposalApproval {
                approved_epoch,
                winner_count,
              },
              enactment: ProposalEnactmentOutcome::ExecutionFailed {
                epoch: execution_epoch,
              },
            },
            Some(crate::ProposalExecutionDetail::Failed(
              crate::ProposalExecutionFailureReason::MissingPreimage,
            )),
          )?;
          Self::release_pending_enactment(domain, item_id);
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
        let execution: Result<_, DispatchError> =
          polkadot_sdk::frame_support::storage::with_transaction(|| {
            match T::ProposalPayloadExecutor::execute(
              domain,
              item_id,
              metadata.payload_kind,
              metadata.payload_hash,
            ) {
              Ok(receipt) => {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(Ok(receipt)))
              }
              Err(reason) => {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(Err(reason)))
              }
            }
          });
        match execution? {
          Ok(receipt) => {
            Self::release_pending_enactment(domain, item_id);
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
                invoice_cid,
                treasury_account,
                beneficiary,
                payout_asset,
                base_amount,
                scalar,
                target_amount,
                actual_amount,
                capacity_limited,
                settlement_kind,
              } => {
                Self::deposit_event(Event::ProposalTreasurySpendExecuted {
                  domain,
                  item_id,
                  approved_epoch,
                  executed_epoch: execution_epoch,
                  invoice_cid: invoice_cid.clone(),
                  treasury_account: treasury_account.clone(),
                  beneficiary: beneficiary.clone(),
                  payout_asset,
                  base_amount,
                  scalar,
                  target_amount,
                  actual_amount,
                  capacity_limited,
                  settlement_kind,
                });
                crate::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
                  invoice_cid,
                  treasury_account,
                  beneficiary,
                  payout_asset,
                  base_amount,
                  scalar,
                  target_amount,
                  actual_amount,
                  capacity_limited,
                  settlement_kind,
                }
              }
            };
            Self::update_finalized_proposal(
              domain,
              item_id,
              FinalizedProposalOutcome::Approved {
                approval: ProposalApproval {
                  approved_epoch,
                  winner_count,
                },
                enactment: ProposalEnactmentOutcome::Enacted {
                  epoch: execution_epoch,
                },
              },
              Some(crate::ProposalExecutionDetail::Succeeded(execution_detail)),
            )?;
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
            Self::update_finalized_proposal(
              domain,
              item_id,
              FinalizedProposalOutcome::Approved {
                approval: ProposalApproval {
                  approved_epoch,
                  winner_count,
                },
                enactment: ProposalEnactmentOutcome::ExecutionFailed {
                  epoch: execution_epoch,
                },
              },
              Some(crate::ProposalExecutionDetail::Failed(reason)),
            )?;
            Self::release_pending_enactment(domain, item_id);
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
      FinalizedProposalOutcome::Approved {
        approval,
        enactment: ProposalEnactmentOutcome::NotAttempted,
      } => approval.approved_epoch,
      FinalizedProposalOutcome::Approved {
        enactment:
          ProposalEnactmentOutcome::Enacted { epoch }
          | ProposalEnactmentOutcome::ExecutionFailed { epoch }
          | ProposalEnactmentOutcome::AdvisoryFinalized { epoch },
        ..
      }
      | FinalizedProposalOutcome::Rejected {
        finalized_epoch: epoch,
        ..
      }
      | FinalizedProposalOutcome::VetoCancelled {
        finalized_epoch: epoch,
        ..
      } => *epoch,
    }
  }
  pub(crate) fn do_recent_finalized_proposals(
    domain: T::DomainId,
  ) -> BoundedVec<
    RecentFinalizedProposal<
      T::AccountId,
      T::DomainId,
      T::WinningVoteItemId,
      T::Hash,
      T::Epoch,
      crate::CidBytesOf<T>,
    >,
    T::MaxRecentFinalizedProposalsPerDomain,
  > {
    let mut proposals = FinalizedProposalIdsByDomain::<T>::get(domain)
      .into_iter()
      .filter_map(|item_id| {
        FinalizedProposals::<T>::get(domain, item_id).map(|finalization| RecentFinalizedProposal {
          identity: ProposalIdentity { domain, item_id },
          finalization,
        })
      })
      .collect::<Vec<_>>();
    proposals.sort_by(|left, right| {
      Self::finalized_outcome_epoch(&right.finalization.outcome)
        .cmp(&Self::finalized_outcome_epoch(&left.finalization.outcome))
        .then_with(|| left.identity.item_id.cmp(&right.identity.item_id))
    });
    BoundedVec::truncate_from(proposals)
  }
}
