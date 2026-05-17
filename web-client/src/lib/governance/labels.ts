/*
Domain: Governance presentation helpers
Owns: Governance-specific label, status, vote, execution, and receipt projections for UI surfaces.
Excludes: Governance store mutation, adapter transport, and reusable UI Kit primitives.
Zone: Product domain support; may depend on governance contracts but not widgets or concrete adapters.
*/
import type {
  GovernanceFrozenBallot,
  GovernancePanelProposal,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernanceProposalExecutionDetail,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPayloadKind,
  GovernanceProposalPrimaryTrackTally,
  GovernanceRetainedFinalizedProposal,
  GovernanceVoteKind,
} from '$lib/governance';

export type FinalizedExecutionDetailRow = {
  label: string;
  value: string;
  mono?: boolean;
};

export function profileLabel(profile?: string) {
  switch (profile) {
    case 'DecliningDirectStake':
      return 'Declining domain stake';
    case 'DecliningVetoAsset':
      return 'Declining VETO asset';
    case 'DecliningNativeStake':
      return 'Declining native stake';
    case 'FlatUrgentDirectStake':
      return 'Urgent flat direct stake';
    default:
      return 'Unavailable';
  }
}

export function statusLabel(proposal: Pick<GovernancePanelProposal, 'status'>) {
  const status = proposal.status;
  if (!status) return 'Unknown';
  if (status.kind === 'PendingEnactment')
    return `Pending enactment · epoch ${status.enactmentEpoch}`;
  if (status.kind === 'Finalized') return status.outcome.kind;
  switch (status.resolution.kind) {
    case 'VotingWindowOpen':
      return `Voting · epoch ${status.resolution.currentEpoch}/${status.resolution.maturityEpoch}`;
    case 'VetoPassing':
      return 'Protection track deciding';
    case 'PassingAye':
      return 'Aye passing';
    case 'PassingAmplify':
      return 'Amplify passing';
    case 'PassingApprove':
      return 'Approve passing';
    case 'PassingReduce':
      return 'Reduce passing';
    case 'PassingNay':
      return 'Nay passing';
    case 'Confirming':
      return `Confirming · ${status.resolution.confirmStartedEpoch}-${status.resolution.confirmEndEpoch}`;
    case 'Rejected':
      return `Rejected · ${rejectionReasonLabel(status.resolution.reason)}`;
  }
}

export function rejectionReasonLabel(reason?: string | null) {
  switch (reason) {
    case 'AdminRejected':
      return 'Admin rejected';
    case 'NoVotes':
      return 'No votes recorded';
    case 'VoteTie':
      return 'Vote tie';
    case 'TurnoutBelowMinimum':
      return 'Turnout below minimum';
    case 'ApprovalThresholdNotMet':
      return 'Approval threshold not met';
    default:
      return 'Unavailable';
  }
}

export function executionFailureReasonLabel(reason?: string | null) {
  switch (reason) {
    case 'MissingPreimage':
      return 'Missing preimage';
    case 'InvalidPreimage':
      return 'Invalid preimage';
    case 'UnsupportedDomain':
      return 'Unsupported governance domain';
    case 'UnsupportedCall':
      return 'Unsupported runtime call';
    case 'UnsupportedTarget':
      return 'Unsupported delegated target';
    case 'UnsupportedPayloadKind':
      return 'Unsupported payload kind';
    case 'MissingWinningPrimaryOption':
      return 'Missing winning primary option';
    case 'DispatchFailed':
      return 'Dispatch failed';
    default:
      return 'Unavailable';
  }
}

export function payloadKindLabel(kind?: string | null) {
  switch (kind) {
    case 'L1RootAction':
      return 'L1 root action';
    case 'L2TreasurySpend':
      return 'L2 treasury spend';
    case 'L2ParameterChange':
      return 'L2 parameter change';
    case 'Intent':
      return 'Intent';
    case 'L2SignalToL1':
      return 'L2 signal to L1';
    default:
      return 'Unavailable';
  }
}

export function cadenceModeLabel(cadenceMode?: string | null) {
  return cadenceMode === 'Ordinary'
    ? 'Ordinary'
    : cadenceMode === 'Fast'
      ? 'Fast'
      : 'Unavailable';
}

export function executionAuthorityLabel(authority?: string | null) {
  switch (authority) {
    case 'Root':
      return 'Root';
    case 'DomainTreasury':
      return 'Domain treasury';
    case 'DomainParameters':
      return 'Domain parameters';
    case 'NonExecutable':
      return 'Advisory only';
    default:
      return 'Unavailable';
  }
}

export function submissionAuthorityLabel(authority?: string | null) {
  return authority === 'Signed'
    ? 'Signed public path'
    : authority === 'AdminOnly'
      ? 'Admin only'
      : 'Unavailable';
}

export function openingFeeLabel(
  authority?: string | null,
  openingFee?: bigint | null,
) {
  if (authority !== 'Signed') return 'No public fee';
  return openingFee == null
    ? 'Unavailable'
    : `${openingFee.toLocaleString()} native units`;
}

export function payloadFamilyLabel(kind?: string | null) {
  switch (kind) {
    case 'L1RootAction':
      return 'Executable · Strategic runtime-upgrade authorization';
    case 'L2TreasurySpend':
      return 'Executable · Tactical invoice treasury spend';
    case 'L2ParameterChange':
      return 'Executable · Delegated router-only parameter change';
    case 'Intent':
      return 'Advisory · Domain-local intent';
    case 'L2SignalToL1':
      return 'Advisory · Upward tactical signal';
    default:
      return 'Unavailable';
  }
}

export function advisoryScopeLabel(kind?: string | null) {
  switch (kind) {
    case 'Intent':
      return 'Same governance domain only';
    case 'L2SignalToL1':
      return 'Upward signal toward L1 only';
    default:
      return null;
  }
}

export function primaryTrackFamilyLabel(primaryTrackFamily?: string | null) {
  return primaryTrackFamily === 'Binary'
    ? 'Binary Aye / Nay'
    : primaryTrackFamily === 'Invoice'
      ? 'Invoice Amplify / Approve / Reduce / Nay'
      : 'Unavailable';
}

export function primaryTrackLeaderLabel(
  primaryTrackTally?: GovernanceProposalPrimaryTrackTally | null,
) {
  if (!primaryTrackTally) return 'Unavailable';
  if (primaryTrackTally.kind === 'Binary')
    return primaryTrackTally.leadingOption ?? 'Tie / none';
  if (!primaryTrackTally.leadingPositiveOption) return 'None';
  return `${primaryTrackTally.leadingPositiveOption} · ${primaryTrackTally.leadingPositiveWeight.toLocaleString()}`;
}

export function primaryTrackPositiveWeightLabel(
  primaryTrackTally?: GovernanceProposalPrimaryTrackTally | null,
) {
  if (!primaryTrackTally) return 'Unavailable';
  return primaryTrackTally.kind === 'Binary'
    ? primaryTrackTally.ayeWeight.toLocaleString()
    : primaryTrackTally.positiveWeight.toLocaleString();
}

export function payloadAvailabilityLabel(
  payloadKind?: string | null,
  availability?: GovernanceProposalPayloadAvailability | null,
) {
  if (!availability) return 'Unavailable';
  if (availability.havePreimage) return 'Ready';
  if (availability.preimageRequested) return 'Requested';
  if (payloadKind === 'Intent' || payloadKind === 'L2SignalToL1')
    return 'Hash only';
  return 'Missing';
}

export function advisoryPayloadHashPreimageStatusLabel(
  status: GovernancePayloadHashPreimageStatus | null,
  loading: boolean,
) {
  if (loading) return 'Checking';
  if (!status) return 'Unavailable';
  if (status.havePreimage)
    return status.byteLength == null
      ? 'Already noted'
      : `Already noted · ${status.byteLength} bytes`;
  if (status.preimageRequested) return 'Requested · not yet noted';
  return 'Not noted';
}

export function advisoryPayloadPreimageNoteCostLabel(
  noteCost: GovernancePayloadPreimageNoteCost | null,
  loading: boolean,
) {
  if (loading) return 'Checking';
  return noteCost == null
    ? 'Unavailable'
    : `${noteCost.toLocaleString()} native units`;
}

export function executionPathLabel(
  payloadKind?: string | null,
  primaryTrackFamily?: string | null,
) {
  switch (payloadKind) {
    case 'L1RootAction':
      return 'Authorizes `System.authorize_upgrade { code_hash }` only';
    case 'L2TreasurySpend':
      return primaryTrackFamily === 'Invoice'
        ? 'Executes BLDR-treasury scalar invoice payout from retained winning option'
        : 'Executes bounded tactical treasury payout';
    case 'L2ParameterChange':
      return 'Executes router fee or tracked-asset change only';
    case 'Intent':
      return 'No dispatch, same-domain advisory only';
    case 'L2SignalToL1':
      return 'No dispatch, upward tactical signal only';
    default:
      return 'Unavailable';
  }
}

export function treasurySettlementLabel(
  payloadKind?: string | null,
  primaryTrackFamily?: string | null,
) {
  if (payloadKind !== 'L2TreasurySpend') return null;
  return primaryTrackFamily === 'Invoice'
    ? 'Scalar invoice transfer'
    : 'Direct transfer only';
}

export function parameterChangeSurfaceLabel(surface?: string | null) {
  return surface === 'RouterFee'
    ? 'Axial Router fee'
    : surface === 'TrackedAsset'
      ? 'Axial Router tracked asset'
      : 'Unavailable';
}

export function treasurySpendScalarLabel(scalar?: string | null) {
  switch (scalar) {
    case 'Amplify':
      return 'Amplify';
    case 'Approve':
      return 'Approve';
    case 'Reduce':
      return 'Reduce';
    default:
      return 'Unavailable';
  }
}

export function treasurySpendSettlementKindLabel(kind?: string | null) {
  return kind === 'InvoiceScalarTransfer'
    ? 'Scalar invoice transfer'
    : kind === 'DirectTransfer'
      ? 'Direct transfer'
      : 'Unavailable';
}

export function accountIdentifierLabel(accountId?: string | null) {
  return accountId ? `Account ${accountId}` : 'Unavailable';
}

export function assetIdentifierLabel(
  assetId?: bigint | number | string | null,
) {
  return assetId == null ? 'Unavailable' : `Asset #${assetId.toString()}`;
}

export function finalizedExecutionDetailRows(
  detail: GovernanceProposalExecutionDetail | null,
): FinalizedExecutionDetailRow[] {
  if (!detail) return [];
  switch (detail.kind) {
    case 'Executed': {
      const rows: FinalizedExecutionDetailRow[] = [
        {
          label: 'Execution receipt',
          value:
            finalizedExecutionDetailLabel(detail) ??
            `${payloadKindLabel(detail.payloadKind)} executed`,
        },
        { label: 'Executed epoch', value: detail.executedEpoch.toString() },
      ];
      switch (detail.detail.kind) {
        case 'Generic':
          return rows;
        case 'RuntimeUpgradeAuthorized':
          rows.push({
            label: 'Authorized code hash',
            value: detail.detail.codeHash,
            mono: true,
          });
          return rows;
        case 'ParameterChangeExecuted':
          rows.push({
            label: 'Parameter surface',
            value: parameterChangeSurfaceLabel(detail.detail.surface),
          });
          return rows;
        case 'TreasurySpendExecuted':
          rows.push(
            {
              label: 'Funding source',
              value: accountIdentifierLabel(detail.detail.fundingSource),
              mono: true,
            },
            {
              label: 'Beneficiary',
              value: accountIdentifierLabel(detail.detail.beneficiary),
              mono: true,
            },
            {
              label: 'Payout asset',
              value: assetIdentifierLabel(detail.detail.payoutAsset),
              mono: true,
            },
            {
              label: 'Base amount',
              value: detail.detail.baseAmount.toLocaleString(),
            },
            {
              label: 'Winning scalar',
              value: treasurySpendScalarLabel(detail.detail.scalar),
            },
            {
              label: 'Final amount',
              value: detail.detail.finalAmount.toLocaleString(),
            },
            {
              label: 'Settlement kind',
              value: treasurySpendSettlementKindLabel(
                detail.detail.settlementKind,
              ),
            },
          );
          return rows;
      }
      return rows;
    }
    case 'ExecutionFailed':
      return [
        {
          label: 'Execution receipt',
          value:
            finalizedExecutionDetailLabel(detail) ??
            `Failed · ${executionFailureReasonLabel(detail.reason)}`,
        },
        { label: 'Failed epoch', value: detail.failedEpoch.toString() },
        {
          label: 'Failure reason',
          value: executionFailureReasonLabel(detail.reason),
        },
      ];
    case 'AdvisoryFinalized':
      return [
        {
          label: 'Execution receipt',
          value:
            finalizedExecutionDetailLabel(detail) ??
            `Advisory · ${payloadKindLabel(detail.payloadKind)}`,
        },
        { label: 'Finalized epoch', value: detail.finalizedEpoch.toString() },
      ];
  }
  return [];
}

export function primaryTrackPowerLabel(
  proposal: Pick<
    GovernancePanelProposal,
    'primaryTrackFamily' | 'votePowerProfiles'
  >,
) {
  if (proposal.primaryTrackFamily === 'Invoice')
    return profileLabel(
      proposal.votePowerProfiles.approve ?? proposal.votePowerProfiles.nay,
    );
  return profileLabel(proposal.votePowerProfiles.aye);
}

export function voteKindLabel(voteKind?: GovernanceVoteKind | null) {
  switch (voteKind) {
    case 'aye':
      return 'Aye';
    case 'nay':
      return 'Nay';
    case 'amplify':
      return 'Amplify';
    case 'approve':
      return 'Approve';
    case 'reduce':
      return 'Reduce';
    case 'veto':
      return 'Veto';
    case 'pass':
      return 'Pass';
    default:
      return 'Unavailable';
  }
}

export function governanceLockLabel(epoch?: number | null) {
  return epoch == null ? 'None' : `Until epoch ${epoch}`;
}

export function frozenBallotLabel(ballot?: GovernanceFrozenBallot | null) {
  if (!ballot) return 'No frozen ballot';
  return `${voteKindLabel(ballot.voteKind)} · ${ballot.weight.toLocaleString()} at epoch ${ballot.voteEpoch}`;
}

export function voteButtons(
  proposal: Pick<GovernancePanelProposal, 'primaryTrackFamily'>,
): Array<{ voteKind: GovernanceVoteKind; label: string }> {
  const primaryButtons: Array<{ voteKind: GovernanceVoteKind; label: string }> =
    proposal.primaryTrackFamily === 'Invoice'
      ? [
          { voteKind: 'amplify', label: 'Amplify' },
          { voteKind: 'approve', label: 'Approve' },
          { voteKind: 'reduce', label: 'Reduce' },
          { voteKind: 'nay', label: 'Nay' },
        ]
      : [
          { voteKind: 'aye', label: 'Aye' },
          { voteKind: 'nay', label: 'Nay' },
        ];
  return [
    ...primaryButtons,
    { voteKind: 'veto', label: 'Veto' },
    { voteKind: 'pass', label: 'Pass' },
  ];
}

export function urgentEligibilityLabel(
  urgentEligibility?: boolean | null,
  payloadKind?: string | null,
) {
  if (urgentEligibility == null) return 'Unavailable';
  if (!urgentEligibility) return 'Disabled on current line';
  if (payloadKind === 'L1RootAction')
    return 'Unanimous PASS runtime-upgrade fast path';
  return 'Expeditable';
}

export function urgentExecutionContractLabel(payloadKind?: string | null) {
  if (payloadKind === 'L1RootAction')
    return 'Unanimous raw VETO PASS immediately authorizes the runtime upgrade without waiting for a separate primary ballot';
  return null;
}

export function timingWindowLabel(
  openEpoch?: number | null,
  closeEpoch?: number | null,
) {
  return openEpoch == null || closeEpoch == null
    ? 'Unavailable'
    : `${openEpoch}-${closeEpoch}`;
}

export function pendingEnactmentLabel(pendingEnactmentEpoch?: number | null) {
  return pendingEnactmentEpoch == null
    ? 'None'
    : `Epoch ${pendingEnactmentEpoch}`;
}

export function primaryTrackOptionLabel(option?: string | null) {
  switch (option) {
    case 'Aye':
      return 'Aye';
    case 'Nay':
      return 'Nay';
    case 'Amplify':
      return 'Amplify';
    case 'Approve':
      return 'Approve';
    case 'Reduce':
      return 'Reduce';
    default:
      return 'Unavailable';
  }
}

export function retainedWinningPrimaryOptionLabel(
  proposal: Pick<GovernanceRetainedFinalizedProposal, 'winningPrimaryOption'>,
) {
  if (proposal.winningPrimaryOption == null) return null;
  return primaryTrackOptionLabel(proposal.winningPrimaryOption);
}

export function executedStateLabel(
  detail: GovernanceProposalExecutionDetail | null,
) {
  if (!detail || detail.kind !== 'Executed') return 'Executed';
  switch (detail.detail.kind) {
    case 'Generic':
      return `${payloadKindLabel(detail.payloadKind)} executed`;
    case 'RuntimeUpgradeAuthorized':
      return 'Executed · Runtime upgrade authorized';
    case 'ParameterChangeExecuted':
      return `Executed · Parameter change · ${parameterChangeSurfaceLabel(detail.detail.surface)}`;
    case 'TreasurySpendExecuted':
      return `Executed · Treasury payout · ${treasurySpendScalarLabel(detail.detail.scalar)}`;
  }
  return 'Executed';
}

export function finalizedExecutionStateLabel(
  proposal: Pick<
    GovernanceRetainedFinalizedProposal,
    'outcome' | 'executionDetail'
  >,
) {
  switch (proposal.outcome.kind) {
    case 'Resolved':
      return 'Approved only, no retained execution receipt';
    case 'Rejected':
      return 'Not executed · Rejected before enactment';
    case 'VetoCancelled':
      return 'Not executed · Blocked by protection';
    case 'Enacted':
      return executedStateLabel(proposal.executionDetail);
    case 'ExecutionFailed':
      return proposal.executionDetail?.kind === 'ExecutionFailed'
        ? `Execution failed · ${executionFailureReasonLabel(proposal.executionDetail.reason)}`
        : 'Execution attempted and failed';
    case 'AdvisoryFinalized':
      return proposal.executionDetail?.kind === 'AdvisoryFinalized'
        ? `Advisory finalization · ${payloadKindLabel(proposal.executionDetail.payloadKind)}`
        : 'Advisory finalization only';
  }
  return 'Unavailable';
}

export function hasDecliningPower(
  proposal: Pick<GovernancePanelProposal, 'votePowerProfiles'>,
) {
  return Object.values(proposal.votePowerProfiles).some(
    (profile) => typeof profile === 'string' && profile.startsWith('Declining'),
  );
}

export function finalizedOutcomeLabel(
  proposal: Pick<
    GovernanceRetainedFinalizedProposal,
    'outcome' | 'executionDetail'
  >,
) {
  const outcome = proposal.outcome;
  switch (outcome.kind) {
    case 'Resolved':
      return `Resolved · epoch ${outcome.epoch}`;
    case 'Rejected':
      return `Rejected · ${rejectionReasonLabel(outcome.reason)}`;
    case 'VetoCancelled':
      return `Veto cancelled · epoch ${outcome.epoch}`;
    case 'Enacted':
      return `Enacted · epoch ${outcome.executedEpoch}`;
    case 'ExecutionFailed':
      return proposal.executionDetail?.kind === 'ExecutionFailed'
        ? `Execution failed · ${executionFailureReasonLabel(proposal.executionDetail.reason)} · epoch ${outcome.failedEpoch}`
        : `Execution failed · epoch ${outcome.failedEpoch}`;
    case 'AdvisoryFinalized':
      return proposal.executionDetail?.kind === 'AdvisoryFinalized'
        ? `Advisory finalized · ${payloadKindLabel(proposal.executionDetail.payloadKind)} · epoch ${outcome.finalizedEpoch}`
        : `Advisory finalized · epoch ${outcome.finalizedEpoch}`;
  }
  return 'Unavailable';
}

export function publicSubmissionPayloadLabel(
  payloadKind: GovernanceProposalPayloadKind,
) {
  switch (payloadKind) {
    case 'Intent':
      return 'Intent';
    case 'L2SignalToL1':
      return 'L2 signal to L1';
    case 'L1RootAction':
      return 'L1 root action';
    case 'L2TreasurySpend':
      return 'L2 treasury spend';
    case 'L2ParameterChange':
      return 'L2 parameter change';
  }
}

export function publicSubmissionSummaryPlaceholder(
  payloadKind: GovernanceProposalPayloadKind | null,
) {
  switch (payloadKind) {
    case 'Intent':
      return 'State the domain-local intent';
    case 'L2SignalToL1':
      return 'State the tactical signal that should be visible to L1';
    default:
      return 'Non-empty advisory summary';
  }
}

export function publicSubmissionButtonLabel(
  payloadKind: GovernanceProposalPayloadKind | null,
) {
  switch (payloadKind) {
    case 'Intent':
      return 'Submit intent';
    case 'L2SignalToL1':
      return 'Submit L2 signal';
    default:
      return 'Submit advisory proposal';
  }
}

export function publicSubmissionPurposeNotice(
  payloadKind: GovernanceProposalPayloadKind | null,
) {
  switch (payloadKind) {
    case 'Intent':
      return 'Use Intent when the current governance domain wants to record its own non-executable position without escalating authority toward another domain';
    case 'L2SignalToL1':
      return 'Use L2 signal to L1 when the current tactical domain wants to record an upward non-executable signal for L1 rather than mutate strategic control surfaces directly';
    default:
      return null;
  }
}

export function finalizedExecutionDetailLabel(
  detail: GovernanceProposalExecutionDetail | null,
) {
  if (!detail) return null;
  switch (detail.kind) {
    case 'Executed':
      switch (detail.detail.kind) {
        case 'Generic':
          return `${payloadKindLabel(detail.payloadKind)} executed`;
        case 'RuntimeUpgradeAuthorized':
          return `Runtime upgrade authorized · ${detail.detail.codeHash}`;
        case 'ParameterChangeExecuted':
          return `Parameter change · ${parameterChangeSurfaceLabel(detail.detail.surface)}`;
        case 'TreasurySpendExecuted':
          return `Treasury payout · ${treasurySpendScalarLabel(detail.detail.scalar)} · ${detail.detail.baseAmount.toLocaleString()} → ${detail.detail.finalAmount.toLocaleString()} · ${accountIdentifierLabel(detail.detail.fundingSource)}`;
      }
      break;
    case 'ExecutionFailed':
      return `Failed · ${executionFailureReasonLabel(detail.reason)}`;
    case 'AdvisoryFinalized':
      return `Advisory · ${payloadKindLabel(detail.payloadKind)}`;
  }
  return null;
}

export function runtimeUpgradeOperatorPathLabel() {
  return 'authorized-upgrade-local.sh check → authorized-upgrade-local.sh apply --submit';
}
