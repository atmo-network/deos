import { Enum as PapiEnum, type Enum as PapiRuntimeEnum } from "polkadot-api";

import type {
  DeosChainSnapshot,
  DeosPapiConnection,
} from "../blockchain/deos";
import {
  connectDeosSigner,
  DEFAULT_DEOS_DAPP_NAME,
  hasBuiltInDevSigner,
  injectedSignerAvailability,
} from "../blockchain/signer";

import type {
  GovernanceAccountId,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceFinalizedProposalOutcome,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernancePrimaryTrackOption,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernanceProposalExecutionDetail,
  GovernanceProposalExecutionFailureReason,
  GovernanceProposalExecutionSuccessDetail,
  GovernanceProposalMetadata,
  GovernanceProposalOpeningFee,
  GovernanceProposalParameterChangeSurface,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalPayloadKind,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalPrimaryTrackTally,
  GovernanceProposalRejectionReason,
  GovernanceProposalResolutionState,
  GovernanceProposalStatus,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalTiming,
  GovernanceProposalTreasurySpendScalar,
  GovernanceProposalTreasurySpendSettlementKind,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceQuerySurfaceAvailability,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVetoCancellationMode,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from "$lib/governance";
import {
  buildWriteSurfaceAvailability,
  DEFAULT_GOVERNANCE_WS_ENDPOINT,
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
} from "$lib/governance";
import type { GovernanceBlockchainProvider } from "./provider";

const FIXED_U128_SCALE = 10n ** 18n;

type GovernanceSnapshot = DeosChainSnapshot;

type GovernanceEnum<T extends Record<string, unknown>> = PapiRuntimeEnum<T>;

function unavailableWriteSurface(
  reason: string,
): GovernanceWriteSurfaceAvailability {
  return buildWriteSurfaceAvailability({
    castVote: { reason },
    submitProposal: { reason },
    noteProposalPreimage: { reason },
    resolveProposal: { reason },
    rejectProposal: { reason },
    resolveProposalFromVotes: { reason },
    forceResolveProposalFromVotes: { reason },
    requeueProposalForAutoFinalization: { reason },
  });
}

function fixedU128String(value: bigint): GovernanceRewardCoefficient {
  if (value <= 0n) {
    return "0.000000000000000000";
  }
  const whole = value / FIXED_U128_SCALE;
  const fraction = (value % FIXED_U128_SCALE).toString().padStart(18, "0");
  return `${whole}.${fraction}`;
}

function mapVotePowerProfile(
  profile:
    | GovernanceEnum<{
        DecliningDirectStake: undefined;
        DecliningVetoAsset: undefined;
        DecliningNativeStake: undefined;
        ConstantProtectedActor: undefined;
        ProxyGovernedActor: undefined;
        FlatUrgentDirectStake: undefined;
      }>
    | undefined,
): GovernanceVotePowerProfile | null {
  if (!profile) {
    return null;
  }
  switch (profile.type) {
    case "DecliningDirectStake":
    case "DecliningVetoAsset":
    case "DecliningNativeStake":
    case "FlatUrgentDirectStake":
      return profile.type;
    case "ConstantProtectedActor":
    case "ProxyGovernedActor":
      return null;
  }
}

function proposalVoteKindEnum(voteKind: GovernanceVoteKind) {
  switch (voteKind) {
    case "aye":
      return PapiEnum("Aye");
    case "nay":
      return PapiEnum("Nay");
    case "amplify":
      return PapiEnum("Amplify");
    case "approve":
      return PapiEnum("Approve");
    case "reduce":
      return PapiEnum("Reduce");
    case "veto":
      return PapiEnum("Veto");
    case "pass":
      return PapiEnum("Pass");
  }
}

function papiWriteSurfaceAvailability(options: {
  signedWriteAvailable: boolean;
  signedWriteReason: string;
  adminReason: string;
}): GovernanceWriteSurfaceAvailability {
  const signedStatus = options.signedWriteAvailable
    ? "available"
    : "unavailable";
  return buildWriteSurfaceAvailability({
    castVote: {
      providerStatus: signedStatus,
      reason: options.signedWriteReason,
    },
    submitProposal: {
      providerStatus: signedStatus,
      reason: options.signedWriteReason,
    },
    noteProposalPreimage: {
      providerStatus: signedStatus,
      reason: options.signedWriteReason,
    },
    resolveProposal: { reason: options.adminReason },
    rejectProposal: { reason: options.adminReason },
    resolveProposalFromVotes: { reason: options.adminReason },
    forceResolveProposalFromVotes: { reason: options.adminReason },
    requeueProposalForAutoFinalization: { reason: options.adminReason },
  });
}

function mapRejectionReason(
  reason: GovernanceEnum<{
    AdminRejected: undefined;
    NoVotes: undefined;
    VoteTie: undefined;
    TurnoutBelowMinimum: undefined;
    ApprovalThresholdNotMet: undefined;
  }>,
): GovernanceProposalRejectionReason {
  switch (reason.type) {
    case "AdminRejected":
    case "NoVotes":
    case "VoteTie":
    case "TurnoutBelowMinimum":
    case "ApprovalThresholdNotMet":
      return reason.type;
  }
}

function mapVetoCancellationMode(
  mode: GovernanceEnum<{
    ImmediateThreshold: undefined;
    TrackOutcome: undefined;
  }>,
): GovernanceVetoCancellationMode {
  switch (mode.type) {
    case "ImmediateThreshold":
    case "TrackOutcome":
      return mode.type;
  }
}

function mapProposalCadenceMode(
  cadenceMode: GovernanceEnum<{
    Ordinary: undefined;
    Fast: undefined;
  }>,
): GovernanceProposalCadenceMode {
  return cadenceMode.type;
}

function mapProposalPayloadKind(
  payloadKind: GovernanceEnum<{
    L1RootAction: undefined;
    L2TreasurySpend: undefined;
    L2ParameterChange: undefined;
    Intent: undefined;
    L2SignalToL1: undefined;
  }>,
): GovernanceProposalPayloadKind {
  return payloadKind.type;
}

function mapProposalExecutionAuthority(
  authority: GovernanceEnum<{
    Root: undefined;
    DomainTreasury: undefined;
    DomainParameters: undefined;
    NonExecutable: undefined;
  }>,
): GovernanceProposalExecutionAuthority {
  return authority.type;
}

function mapProposalSubmissionAuthority(
  authority: GovernanceEnum<{
    Signed: undefined;
    AdminOnly: undefined;
  }>,
): GovernanceProposalSubmissionAuthority {
  return authority.type;
}

function mapProposalExecutionFailureReason(
  reason: GovernanceEnum<{
    MissingPreimage: undefined;
    InvalidPreimage: undefined;
    UnsupportedDomain: undefined;
    UnsupportedCall: undefined;
    UnsupportedTarget: undefined;
    UnsupportedPayloadKind: undefined;
    MissingWinningPrimaryOption: undefined;
    DispatchFailed: undefined;
  }>,
): GovernanceProposalExecutionFailureReason {
  return reason.type;
}

function mapProposalPrimaryTrackFamily(
  family:
    | GovernanceEnum<{
        Binary: undefined;
        Invoice: undefined;
      }>
    | undefined,
): GovernanceProposalPrimaryTrackFamily | null {
  if (!family) {
    return null;
  }
  return family.type;
}

function mapPrimaryTrackOption(
  option:
    | GovernanceEnum<{
        Aye: undefined;
        Nay: undefined;
        Amplify: undefined;
        Approve: undefined;
        Reduce: undefined;
      }>
    | undefined,
): GovernancePrimaryTrackOption | null {
  if (!option) {
    return null;
  }
  return option.type;
}

function mapProposalTiming(
  timing:
    | {
        submitted_epoch: number;
        protection_open_epoch: number;
        protection_close_epoch: number;
        ordinary_primary_open_epoch: number;
        ordinary_primary_close_epoch: number;
        urgent_primary_open_epoch: number | undefined;
        urgent_primary_close_epoch: number | undefined;
        effective_primary_open_epoch: number;
        effective_primary_close_epoch: number;
        pending_enactment_epoch: number | undefined;
      }
    | undefined,
): GovernanceProposalTiming | null {
  if (!timing) {
    return null;
  }
  return {
    submittedEpoch: timing.submitted_epoch,
    protectionOpenEpoch: timing.protection_open_epoch,
    protectionCloseEpoch: timing.protection_close_epoch,
    ordinaryPrimaryOpenEpoch: timing.ordinary_primary_open_epoch,
    ordinaryPrimaryCloseEpoch: timing.ordinary_primary_close_epoch,
    urgentPrimaryOpenEpoch: timing.urgent_primary_open_epoch ?? null,
    urgentPrimaryCloseEpoch: timing.urgent_primary_close_epoch ?? null,
    effectivePrimaryOpenEpoch: timing.effective_primary_open_epoch,
    effectivePrimaryCloseEpoch: timing.effective_primary_close_epoch,
    pendingEnactmentEpoch: timing.pending_enactment_epoch ?? null,
  };
}

function mapProposalParameterChangeSurface(
  surface: GovernanceEnum<{
    RouterFee: undefined;
    TrackedAsset: undefined;
  }>,
): GovernanceProposalParameterChangeSurface {
  return surface.type;
}

function mapProposalTreasurySpendSettlementKind(
  settlementKind: GovernanceEnum<{
    DirectTransfer: undefined;
    InvoiceScalarTransfer: undefined;
  }>,
): GovernanceProposalTreasurySpendSettlementKind {
  return settlementKind.type;
}

function mapProposalTreasurySpendScalar(
  scalar: GovernanceEnum<{
    Amplify: undefined;
    Approve: undefined;
    Reduce: undefined;
  }>,
): GovernanceProposalTreasurySpendScalar {
  return scalar.type;
}

function mapProposalExecutionSuccessDetail(
  detail: GovernanceEnum<{
    Generic: undefined;
    RuntimeUpgradeAuthorized: {
      code_hash: string;
    };
    ParameterChangeExecuted: {
      surface: GovernanceEnum<{
        RouterFee: undefined;
        TrackedAsset: undefined;
      }>;
    };
    TreasurySpendExecuted: {
      funding_source: GovernanceAccountId;
      beneficiary: GovernanceAccountId;
      payout_asset: GovernanceDomainId;
      base_amount: bigint;
      scalar: GovernanceEnum<{
        Amplify: undefined;
        Approve: undefined;
        Reduce: undefined;
      }>;
      final_amount: bigint;
      settlement_kind: GovernanceEnum<{
        DirectTransfer: undefined;
        InvoiceScalarTransfer: undefined;
      }>;
    };
  }>,
): GovernanceProposalExecutionSuccessDetail {
  switch (detail.type) {
    case "Generic":
      return { kind: "Generic" };
    case "RuntimeUpgradeAuthorized":
      return {
        kind: "RuntimeUpgradeAuthorized",
        codeHash: detail.value.code_hash,
      };
    case "ParameterChangeExecuted":
      return {
        kind: "ParameterChangeExecuted",
        surface: mapProposalParameterChangeSurface(detail.value.surface),
      };
    case "TreasurySpendExecuted":
      return {
        kind: "TreasurySpendExecuted",
        fundingSource: detail.value.funding_source,
        beneficiary: detail.value.beneficiary,
        payoutAsset: detail.value.payout_asset,
        baseAmount: detail.value.base_amount,
        scalar: mapProposalTreasurySpendScalar(detail.value.scalar),
        finalAmount: detail.value.final_amount,
        settlementKind: mapProposalTreasurySpendSettlementKind(
          detail.value.settlement_kind,
        ),
      };
  }
}

function mapProposalExecutionDetail(
  detail:
    | GovernanceEnum<{
        Executed: {
          payload_kind: GovernanceEnum<{
            L1RootAction: undefined;
            L2TreasurySpend: undefined;
            L2ParameterChange: undefined;
            Intent: undefined;
            L2SignalToL1: undefined;
          }>;
          authority: GovernanceEnum<{
            Root: undefined;
            DomainTreasury: undefined;
            DomainParameters: undefined;
            NonExecutable: undefined;
          }>;
          executed_epoch: number;
          detail: GovernanceEnum<{
            Generic: undefined;
            RuntimeUpgradeAuthorized: { code_hash: string };
            ParameterChangeExecuted: {
              surface: GovernanceEnum<{
                RouterFee: undefined;
                TrackedAsset: undefined;
              }>;
            };
            TreasurySpendExecuted: {
              funding_source: GovernanceAccountId;
              beneficiary: GovernanceAccountId;
              payout_asset: GovernanceDomainId;
              base_amount: bigint;
              scalar: GovernanceEnum<{
                Amplify: undefined;
                Approve: undefined;
                Reduce: undefined;
              }>;
              final_amount: bigint;
              settlement_kind: GovernanceEnum<{
                DirectTransfer: undefined;
                InvoiceScalarTransfer: undefined;
              }>;
            };
          }>;
        };
        ExecutionFailed: {
          payload_kind: GovernanceEnum<{
            L1RootAction: undefined;
            L2TreasurySpend: undefined;
            L2ParameterChange: undefined;
            Intent: undefined;
            L2SignalToL1: undefined;
          }>;
          authority: GovernanceEnum<{
            Root: undefined;
            DomainTreasury: undefined;
            DomainParameters: undefined;
            NonExecutable: undefined;
          }>;
          failed_epoch: number;
          reason: GovernanceEnum<{
            MissingPreimage: undefined;
            InvalidPreimage: undefined;
            UnsupportedDomain: undefined;
            UnsupportedCall: undefined;
            UnsupportedTarget: undefined;
            UnsupportedPayloadKind: undefined;
            MissingWinningPrimaryOption: undefined;
            DispatchFailed: undefined;
          }>;
        };
        AdvisoryFinalized: {
          payload_kind: GovernanceEnum<{
            L1RootAction: undefined;
            L2TreasurySpend: undefined;
            L2ParameterChange: undefined;
            Intent: undefined;
            L2SignalToL1: undefined;
          }>;
          finalized_epoch: number;
        };
      }>
    | undefined,
): GovernanceProposalExecutionDetail | null {
  if (!detail) {
    return null;
  }
  switch (detail.type) {
    case "Executed":
      return {
        kind: "Executed",
        payloadKind: mapProposalPayloadKind(detail.value.payload_kind),
        authority: mapProposalExecutionAuthority(detail.value.authority),
        executedEpoch: detail.value.executed_epoch,
        detail: mapProposalExecutionSuccessDetail(detail.value.detail),
      };
    case "ExecutionFailed":
      return {
        kind: "ExecutionFailed",
        payloadKind: mapProposalPayloadKind(detail.value.payload_kind),
        authority: mapProposalExecutionAuthority(detail.value.authority),
        failedEpoch: detail.value.failed_epoch,
        reason: mapProposalExecutionFailureReason(detail.value.reason),
      };
    case "AdvisoryFinalized":
      return {
        kind: "AdvisoryFinalized",
        payloadKind: mapProposalPayloadKind(detail.value.payload_kind),
        finalizedEpoch: detail.value.finalized_epoch,
      };
  }
}

function mapFinalizedProposalOutcome(
  outcome: GovernanceEnum<{
    Resolved: {
      epoch: number;
      winner_count: number;
    };
    Rejected: {
      epoch: number;
      reason: GovernanceEnum<{
        AdminRejected: undefined;
        NoVotes: undefined;
        VoteTie: undefined;
        TurnoutBelowMinimum: undefined;
        ApprovalThresholdNotMet: undefined;
      }>;
    };
    VetoCancelled: {
      epoch: number;
      veto_weight: bigint;
    };
    Enacted: {
      approved_epoch: number;
      executed_epoch: number;
      winner_count: number;
    };
    ExecutionFailed: {
      approved_epoch: number;
      failed_epoch: number;
      winner_count: number;
    };
    AdvisoryFinalized: {
      approved_epoch: number;
      finalized_epoch: number;
      winner_count: number;
    };
  }>,
): GovernanceFinalizedProposalOutcome {
  switch (outcome.type) {
    case "Resolved":
      return {
        kind: "Resolved",
        epoch: outcome.value.epoch,
        winnerCount: outcome.value.winner_count,
      };
    case "Rejected":
      return {
        kind: "Rejected",
        epoch: outcome.value.epoch,
        reason: mapRejectionReason(outcome.value.reason),
      };
    case "VetoCancelled":
      return {
        kind: "VetoCancelled",
        epoch: outcome.value.epoch,
        vetoWeight: outcome.value.veto_weight,
      };
    case "Enacted":
      return {
        kind: "Enacted",
        approvedEpoch: outcome.value.approved_epoch,
        executedEpoch: outcome.value.executed_epoch,
        winnerCount: outcome.value.winner_count,
      };
    case "ExecutionFailed":
      return {
        kind: "ExecutionFailed",
        approvedEpoch: outcome.value.approved_epoch,
        failedEpoch: outcome.value.failed_epoch,
        winnerCount: outcome.value.winner_count,
      };
    case "AdvisoryFinalized":
      return {
        kind: "AdvisoryFinalized",
        approvedEpoch: outcome.value.approved_epoch,
        finalizedEpoch: outcome.value.finalized_epoch,
        winnerCount: outcome.value.winner_count,
      };
  }
}

function mapProposalVoteTally(
  tally:
    | {
        aye_voters: number;
        nay_voters: number;
        veto_voters: number;
        pass_voters: number;
        aye_weight: bigint;
        nay_weight: bigint;
        veto_weight: bigint;
        pass_weight: bigint;
        turnout_weight: bigint;
        veto_turnout_weight: bigint;
      }
    | undefined,
): GovernanceProposalVoteTally | null {
  if (!tally) {
    return null;
  }
  const maybeExtended = tally as {
    amplify_voters?: number;
    approve_voters?: number;
    reduce_voters?: number;
    amplify_weight?: bigint;
    approve_weight?: bigint;
    reduce_weight?: bigint;
  };
  return {
    ayeVoters: tally.aye_voters,
    nayVoters: tally.nay_voters,
    amplifyVoters: maybeExtended.amplify_voters ?? 0,
    approveVoters: maybeExtended.approve_voters ?? 0,
    reduceVoters: maybeExtended.reduce_voters ?? 0,
    vetoVoters: tally.veto_voters,
    passVoters: tally.pass_voters,
    ayeWeight: tally.aye_weight,
    nayWeight: tally.nay_weight,
    amplifyWeight: maybeExtended.amplify_weight ?? 0n,
    approveWeight: maybeExtended.approve_weight ?? 0n,
    reduceWeight: maybeExtended.reduce_weight ?? 0n,
    vetoWeight: tally.veto_weight,
    passWeight: tally.pass_weight,
    turnoutWeight: tally.turnout_weight,
    vetoTurnoutWeight: tally.veto_turnout_weight,
  };
}

function mapProposalPrimaryTrackTally(
  tally:
    | GovernanceEnum<{
        Binary: {
          aye_voters: number;
          nay_voters: number;
          aye_weight: bigint;
          nay_weight: bigint;
          turnout_weight: bigint;
          leading_option?:
            | GovernanceEnum<{
                Aye: undefined;
                Nay: undefined;
                Amplify: undefined;
                Approve: undefined;
                Reduce: undefined;
              }>
            | undefined;
        };
        Invoice: {
          amplify_voters: number;
          approve_voters: number;
          reduce_voters: number;
          nay_voters: number;
          amplify_weight: bigint;
          approve_weight: bigint;
          reduce_weight: bigint;
          nay_weight: bigint;
          positive_weight: bigint;
          turnout_weight: bigint;
          leading_positive_option?:
            | GovernanceEnum<{
                Aye: undefined;
                Nay: undefined;
                Amplify: undefined;
                Approve: undefined;
                Reduce: undefined;
              }>
            | undefined;
          leading_positive_weight: bigint;
        };
      }>
    | undefined,
): GovernanceProposalPrimaryTrackTally | null {
  if (!tally) {
    return null;
  }
  switch (tally.type) {
    case "Binary":
      return {
        kind: "Binary",
        ayeVoters: tally.value.aye_voters,
        nayVoters: tally.value.nay_voters,
        ayeWeight: tally.value.aye_weight,
        nayWeight: tally.value.nay_weight,
        turnoutWeight: tally.value.turnout_weight,
        leadingOption: mapPrimaryTrackOption(tally.value.leading_option),
      };
    case "Invoice":
      return {
        kind: "Invoice",
        amplifyVoters: tally.value.amplify_voters,
        approveVoters: tally.value.approve_voters,
        reduceVoters: tally.value.reduce_voters,
        nayVoters: tally.value.nay_voters,
        amplifyWeight: tally.value.amplify_weight,
        approveWeight: tally.value.approve_weight,
        reduceWeight: tally.value.reduce_weight,
        nayWeight: tally.value.nay_weight,
        positiveWeight: tally.value.positive_weight,
        turnoutWeight: tally.value.turnout_weight,
        leadingPositiveOption: mapPrimaryTrackOption(
          tally.value.leading_positive_option,
        ),
        leadingPositiveWeight: tally.value.leading_positive_weight,
      };
  }
}

function mapProposalResolutionState(
  resolution: GovernanceEnum<{
    VotingWindowOpen: {
      current_epoch: number;
      maturity_epoch: number;
    };
    VetoPassing: {
      veto_weight: bigint;
      pass_weight: bigint;
      mode: GovernanceEnum<{
        ImmediateThreshold: undefined;
        TrackOutcome: undefined;
      }>;
    };
    PassingAye: undefined;
    PassingAmplify: undefined;
    PassingApprove: undefined;
    PassingReduce: undefined;
    PassingNay: undefined;
    Confirming: {
      confirm_started_epoch: number;
      confirm_end_epoch: number;
    };
    Rejected: {
      reason: GovernanceEnum<{
        AdminRejected: undefined;
        NoVotes: undefined;
        VoteTie: undefined;
        TurnoutBelowMinimum: undefined;
        ApprovalThresholdNotMet: undefined;
      }>;
    };
  }>,
): GovernanceProposalResolutionState {
  switch (resolution.type) {
    case "VotingWindowOpen":
      return {
        kind: "VotingWindowOpen",
        currentEpoch: resolution.value.current_epoch,
        maturityEpoch: resolution.value.maturity_epoch,
      };
    case "VetoPassing":
      return {
        kind: "VetoPassing",
        vetoWeight: resolution.value.veto_weight,
        passWeight: resolution.value.pass_weight,
        mode: mapVetoCancellationMode(resolution.value.mode),
      };
    case "PassingAye":
      return { kind: "PassingAye" };
    case "PassingAmplify":
      return { kind: "PassingAmplify" };
    case "PassingApprove":
      return { kind: "PassingApprove" };
    case "PassingReduce":
      return { kind: "PassingReduce" };
    case "PassingNay":
      return { kind: "PassingNay" };
    case "Confirming":
      return {
        kind: "Confirming",
        confirmStartedEpoch: resolution.value.confirm_started_epoch,
        confirmEndEpoch: resolution.value.confirm_end_epoch,
      };
    case "Rejected":
      return {
        kind: "Rejected",
        reason: mapRejectionReason(resolution.value.reason),
      };
  }
}

function mapProposalStatus(
  status:
    | GovernanceEnum<{
        Active: GovernanceEnum<{
          VotingWindowOpen: {
            current_epoch: number;
            maturity_epoch: number;
          };
          VetoPassing: {
            veto_weight: bigint;
            pass_weight: bigint;
            mode: GovernanceEnum<{
              ImmediateThreshold: undefined;
              TrackOutcome: undefined;
            }>;
          };
          PassingAye: undefined;
          PassingAmplify: undefined;
          PassingApprove: undefined;
          PassingReduce: undefined;
          PassingNay: undefined;
          Confirming: {
            confirm_started_epoch: number;
            confirm_end_epoch: number;
          };
          Rejected: {
            reason: GovernanceEnum<{
              AdminRejected: undefined;
              NoVotes: undefined;
              VoteTie: undefined;
              TurnoutBelowMinimum: undefined;
              ApprovalThresholdNotMet: undefined;
            }>;
          };
        }>;
        PendingEnactment: {
          outcome: GovernanceEnum<{
            Resolved: {
              epoch: number;
              winner_count: number;
            };
            Rejected: {
              epoch: number;
              reason: GovernanceEnum<{
                AdminRejected: undefined;
                NoVotes: undefined;
                VoteTie: undefined;
                TurnoutBelowMinimum: undefined;
                ApprovalThresholdNotMet: undefined;
              }>;
            };
            VetoCancelled: {
              epoch: number;
              veto_weight: bigint;
            };
            Enacted: {
              approved_epoch: number;
              executed_epoch: number;
              winner_count: number;
            };
            ExecutionFailed: {
              approved_epoch: number;
              failed_epoch: number;
              winner_count: number;
            };
            AdvisoryFinalized: {
              approved_epoch: number;
              finalized_epoch: number;
              winner_count: number;
            };
          }>;
          enactment_epoch: number;
        };
        Finalized: GovernanceEnum<{
          Resolved: {
            epoch: number;
            winner_count: number;
          };
          Rejected: {
            epoch: number;
            reason: GovernanceEnum<{
              AdminRejected: undefined;
              NoVotes: undefined;
              VoteTie: undefined;
              TurnoutBelowMinimum: undefined;
              ApprovalThresholdNotMet: undefined;
            }>;
          };
          VetoCancelled: {
            epoch: number;
            veto_weight: bigint;
          };
          Enacted: {
            approved_epoch: number;
            executed_epoch: number;
            winner_count: number;
          };
          ExecutionFailed: {
            approved_epoch: number;
            failed_epoch: number;
            winner_count: number;
          };
          AdvisoryFinalized: {
            approved_epoch: number;
            finalized_epoch: number;
            winner_count: number;
          };
        }>;
      }>
    | undefined,
): GovernanceProposalStatus | null {
  if (!status) {
    return null;
  }
  switch (status.type) {
    case "Active":
      return {
        kind: "Active",
        resolution: mapProposalResolutionState(status.value),
      };
    case "PendingEnactment":
      return {
        kind: "PendingEnactment",
        outcome: mapFinalizedProposalOutcome(status.value.outcome),
        enactmentEpoch: status.value.enactment_epoch,
      };
    case "Finalized":
      return {
        kind: "Finalized",
        outcome: mapFinalizedProposalOutcome(status.value),
      };
  }
}

export class GovernancePapiProvider implements GovernanceBlockchainProvider {
  private connection: DeosPapiConnection | null = null;
  private connectionLoading: Promise<DeosPapiConnection> | null = null;

  constructor(
    private readonly endpoint: string = DEFAULT_GOVERNANCE_WS_ENDPOINT,
  ) {}

  private uninitializedProviderState(): GovernanceProviderState {
    return {
      status: "unconfigured",
      label: "Governance PAPI provider",
      endpoint: this.endpoint.trim().length > 0 ? this.endpoint.trim() : null,
      chainName: null,
      nodeName: null,
      nodeVersion: null,
      genesisHash: null,
      finalizedBlockHash: null,
      finalizedBlockNumber: null,
      message: "Governance PAPI connection not checked yet",
    };
  }

  private async ensureConnection(): Promise<DeosPapiConnection> {
    if (this.connection) {
      return this.connection;
    }
    if (this.connectionLoading) {
      return this.connectionLoading;
    }
    this.connectionLoading = import("../blockchain/deos").then(
      ({ DeosPapiConnection }) => {
        const connection = new DeosPapiConnection(this.endpoint);
        this.connection = connection;
        this.connectionLoading = null;
        return connection;
      },
    );
    return this.connectionLoading;
  }

  private governanceProviderState(): GovernanceProviderState {
    const state =
      this.connection?.connectionState() ?? this.uninitializedProviderState();
    if (state.status !== "connected") {
      return state;
    }
    return {
      ...state,
      message:
        "PAPI connected; live governance runtime views now provide canonical recent-finalized discovery, retained execution detail, proposal status/tally/profile, submission-authority, reward coefficient, and GovXP counter reads, cast_vote is available when the selected account matches either an injected wallet or a built-in local dev signer, materialized history stays external, and admin signing is not implemented yet",
    };
  }

  private async snapshot(): Promise<GovernanceSnapshot> {
    const connection = await this.ensureConnection();
    return connection.snapshot();
  }

  async syncProviderState(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.syncConnectionState();
  }

  subscribeToUpdates(onUpdate: () => void): () => void {
    let unsubscribe = () => {};
    let disposed = false;
    void this.ensureConnection()
      .then((connection) => {
        if (disposed) {
          return;
        }
        unsubscribe = connection.subscribeToFinalizedBlocks(onUpdate);
      })
      .catch(() => {
        // Ignore eager subscription failures; syncProviderState/refresh will surface them
      });
    return () => {
      disposed = true;
      unsubscribe();
    };
  }

  getProviderState(): GovernanceProviderState {
    return this.governanceProviderState();
  }

  getProviderLabel(): string {
    return this.governanceProviderState().label;
  }

  getQuerySurfaceAvailability(): GovernanceQuerySurfaceAvailability {
    return GOVERNANCE_QUERY_SURFACE_AVAILABILITY;
  }

  getWriteSurfaceAvailability(
    accountId?: string | null,
  ): GovernanceWriteSurfaceAvailability {
    const state = this.governanceProviderState();
    if (state.status !== "connected") {
      return unavailableWriteSurface(
        state.message ?? "PAPI provider unavailable",
      );
    }
    const signerSupport = injectedSignerAvailability();
    const isDevSigner = hasBuiltInDevSigner(accountId ?? null);
    return papiWriteSurfaceAvailability({
      signedWriteAvailable:
        signerSupport.status === "available" || isDevSigner,
      signedWriteReason:
        signerSupport.status === "available"
          ? `${signerSupport.message}; signed governance writes are enabled when the selected account matches either an injected signer account or a built-in Zombienet dev identity`
          : isDevSigner
            ? "Built-in Zombienet dev signer is active for the selected account; signed governance writes are available for local testing even without an injected extension"
            : `${signerSupport.message}; signed governance writes still work for built-in Zombienet dev identities, while custom addresses need a matching injected signer account`,
      adminReason:
        "Runtime admin governance extrinsics remain unavailable in the browser provider until an explicit admin signing/origin flow exists",
    });
  }

  async getActiveProposalIds(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceItemId[]> {
    const snapshot = await this.snapshot();
    return snapshot.typedApi.query.Governance.ActiveProposalIdsByDomain.getValue(
      domainId,
      {
        at: snapshot.at,
      },
    );
  }

  async getRecentFinalizedProposals(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceRecentFinalizedProposal[]> {
    const snapshot = await this.snapshot();
    const proposals =
      await snapshot.typedApi.view.Governance.recent_finalized_proposals(
        domainId,
        { at: snapshot.at },
      );
    return Promise.all(
      proposals.map(async (proposal) => ({
        itemId: proposal.item_id,
        outcome: mapFinalizedProposalOutcome(proposal.outcome),
        executionDetail: mapProposalExecutionDetail(
          await snapshot.typedApi.view.Governance.proposal_execution_detail(
            domainId,
            proposal.item_id,
            { at: snapshot.at },
          ),
        ),
      })),
    );
  }

  async getProposalStatus(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalStatus | null> {
    const snapshot = await this.snapshot();
    return mapProposalStatus(
      await snapshot.typedApi.view.Governance.proposal_status(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalMetadata(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalMetadata | null> {
    const snapshot = await this.snapshot();
    const metadata =
      await snapshot.typedApi.query.Governance.ProposalMetadataByItem.getValue(
        domainId,
        itemId,
        { at: snapshot.at },
      );
    if (!metadata) {
      return null;
    }
    return {
      cadenceMode: mapProposalCadenceMode(metadata.cadence_mode),
      payloadKind: mapProposalPayloadKind(metadata.payload_kind),
      payloadHash: metadata.payload_hash,
    };
  }

  async getProposalExecutionAuthority(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionAuthority | null> {
    const snapshot = await this.snapshot();
    const authority =
      await snapshot.typedApi.view.Governance.proposal_execution_authority(
        domainId,
        itemId,
        { at: snapshot.at },
      );
    return authority ? mapProposalExecutionAuthority(authority) : null;
  }

  async getAuthorizedRuntimeUpgrade(): Promise<GovernanceAuthorizedRuntimeUpgrade | null> {
    const snapshot = await this.snapshot();
    const authorization =
      await snapshot.typedApi.view.Governance.authorized_runtime_upgrade({
        at: snapshot.at,
      });
    if (!authorization) {
      return null;
    }
    return {
      codeHash: authorization.code_hash,
      checkVersion: authorization.check_version,
    };
  }

  async getProposalSubmissionAuthority(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalSubmissionAuthority | null> {
    const snapshot = await this.snapshot();
    return mapProposalSubmissionAuthority(
      await snapshot.typedApi.view.Governance.proposal_submission_authority(
        domainId,
        PapiEnum(payloadKind),
        { at: snapshot.at },
      ),
    );
  }

  async getProposalOpeningFee(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalOpeningFee | null> {
    const snapshot = await this.snapshot();
    const fee = await snapshot.typedApi.view.Governance.proposal_opening_fee(
      domainId,
      PapiEnum(payloadKind),
      { at: snapshot.at },
    );
    return fee ?? null;
  }

  async getProposalPayloadAvailability(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPayloadAvailability | null> {
    const snapshot = await this.snapshot();
    const availability =
      await snapshot.typedApi.view.Governance.proposal_payload_availability(
        domainId,
        itemId,
        { at: snapshot.at },
      );
    if (!availability) {
      return null;
    }
    return {
      havePreimage: availability.have_preimage,
      preimageRequested: availability.preimage_requested,
    };
  }

  async getPayloadHashPreimageStatus(
    payloadHash: string,
  ): Promise<GovernancePayloadHashPreimageStatus | null> {
    const snapshot = await this.snapshot();
    const status =
      await snapshot.typedApi.view.Governance.payload_hash_preimage_status(
        payloadHash,
        { at: snapshot.at },
      );
    return {
      havePreimage: status.have_preimage,
      preimageRequested: status.preimage_requested,
      byteLength: status.payload_len ?? null,
    };
  }

  async getPayloadPreimageNoteCost(
    payloadLen: number,
  ): Promise<GovernancePayloadPreimageNoteCost | null> {
    const snapshot = await this.snapshot();
    const cost =
      await snapshot.typedApi.view.Governance.payload_preimage_note_cost(
        payloadLen,
        {
          at: snapshot.at,
        },
      );
    return cost ?? null;
  }

  async getProposalPrimaryTrackFamily(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackFamily | null> {
    const snapshot = await this.snapshot();
    return mapProposalPrimaryTrackFamily(
      await snapshot.typedApi.view.Governance.proposal_primary_track_family(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalTiming(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalTiming | null> {
    const snapshot = await this.snapshot();
    return mapProposalTiming(
      await snapshot.typedApi.view.Governance.proposal_timing(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalUrgentEligibility(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<boolean | null> {
    const snapshot = await this.snapshot();
    const eligibility =
      await snapshot.typedApi.view.Governance.proposal_urgent_eligibility(
        domainId,
        itemId,
        { at: snapshot.at },
      );
    return eligibility ?? null;
  }

  async getProposalPrimaryTrackTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackTally | null> {
    const snapshot = await this.snapshot();
    return mapProposalPrimaryTrackTally(
      await snapshot.typedApi.view.Governance.proposal_primary_track_tally(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalWinningPrimaryOption(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernancePrimaryTrackOption | null> {
    const snapshot = await this.snapshot();
    return mapPrimaryTrackOption(
      await snapshot.typedApi.view.Governance.retained_proposal_winning_primary_option(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalVoteTally | null> {
    const snapshot = await this.snapshot();
    return mapProposalVoteTally(
      await snapshot.typedApi.view.Governance.proposal_vote_tally(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalExecutionDetail(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionDetail | null> {
    const snapshot = await this.snapshot();
    return mapProposalExecutionDetail(
      await snapshot.typedApi.view.Governance.proposal_execution_detail(
        domainId,
        itemId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getProposalVotePowerProfile(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
    voteKind: GovernanceVoteKind,
  ): Promise<GovernanceVotePowerProfile | null> {
    const snapshot = await this.snapshot();
    return mapVotePowerProfile(
      await snapshot.typedApi.view.Governance.proposal_vote_power_profile(
        domainId,
        itemId,
        proposalVoteKindEnum(voteKind),
        { at: snapshot.at },
      ),
    );
  }

  async getRewardCoefficient(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceRewardCoefficient | null> {
    const snapshot = await this.snapshot();
    return fixedU128String(
      await snapshot.typedApi.view.Governance.reward_coefficient(
        domainId,
        accountId,
        {
          at: snapshot.at,
        },
      ),
    );
  }

  async getGovXpCounters(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceGovXpCounters> {
    const snapshot = await this.snapshot();
    const counters = await snapshot.typedApi.view.Governance.govxp_counters(
      domainId,
      accountId,
      { at: snapshot.at },
    );
    return {
      rollingWinningParticipation: counters.rolling_winning_participation,
      totalParticipations: counters.total_participations,
      totalWinningParticipations: counters.total_winning_participations,
      totalAuthoredProposals: counters.total_authored_proposals,
      totalSuccessfulAuthoredProposals:
        counters.total_successful_authored_proposals,
    };
  }

  async castVote(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    voteKind: GovernanceVoteKind;
  }): Promise<void> {
    const signer = await connectDeosSigner(
      input.accountId,
      DEFAULT_DEOS_DAPP_NAME,
    );
    if (!signer) {
      throw new Error(
        `No signer is available for ${input.accountId}. Use an injected wallet account or a built-in Zombienet dev identity before casting a live governance vote.`,
      );
    }
    try {
      const connection = await this.ensureConnection();
      const { typedApi } = await connection.ensureConnected();
      await typedApi.tx.Governance.cast_vote({
        domain: input.domainId,
        item_id: input.itemId,
        vote: proposalVoteKindEnum(input.voteKind),
      }).signAndSubmit(signer.signer);
      await connection.syncConnectionState();
    } finally {
      signer.disconnect();
    }
  }

  async submitProposal(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    cadenceMode: GovernanceProposalCadenceMode;
    payloadKind: GovernanceProposalPayloadKind;
    payloadHash: string;
  }): Promise<void> {
    const authority = await this.getProposalSubmissionAuthority(
      input.domainId,
      input.payloadKind,
    );
    if (authority !== "Signed") {
      throw new Error(
        `Browser submission currently supports only runtime-signed public proposal kinds; ${input.payloadKind} is still admin-only for domain ${input.domainId}.`,
      );
    }
    const signer = await connectDeosSigner(
      input.accountId,
      DEFAULT_DEOS_DAPP_NAME,
    );
    if (!signer) {
      throw new Error(
        `No signer is available for ${input.accountId}. Use an injected wallet account or a built-in Zombienet dev identity before submitting a live governance proposal.`,
      );
    }
    try {
      const connection = await this.ensureConnection();
      const { typedApi } = await connection.ensureConnected();
      await typedApi.tx.Governance.submit_signed_proposal({
        domain: input.domainId,
        item_id: input.itemId,
        cadence_mode: PapiEnum(input.cadenceMode),
        payload_kind: PapiEnum(input.payloadKind),
        payload_hash: input.payloadHash,
      }).signAndSubmit(signer.signer);
      await connection.syncConnectionState();
    } finally {
      signer.disconnect();
    }
  }

  async noteProposalPreimage(input: {
    accountId: GovernanceAccountId;
    payloadBytes: Uint8Array;
  }): Promise<void> {
    const signer = await connectDeosSigner(
      input.accountId,
      DEFAULT_DEOS_DAPP_NAME,
    );
    if (!signer) {
      throw new Error(
        `No signer is available for ${input.accountId}. Use an injected wallet account or a built-in Zombienet dev identity before noting a live governance preimage.`,
      );
    }
    try {
      const connection = await this.ensureConnection();
      const { typedApi } = await connection.ensureConnected();
      await typedApi.tx.Preimage.note_preimage({
        bytes: input.payloadBytes,
      }).signAndSubmit(signer.signer);
      await connection.syncConnectionState();
    } finally {
      signer.disconnect();
    }
  }

  async resolveProposal(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.ensureConnected();
    throw new Error(
      "Browser-side admin governance extrinsic submission is not implemented yet",
    );
  }

  async rejectProposal(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.ensureConnected();
    throw new Error(
      "Browser-side admin governance extrinsic submission is not implemented yet",
    );
  }

  async resolveProposalFromVotes(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.ensureConnected();
    throw new Error(
      "Browser-side admin governance extrinsic submission is not implemented yet",
    );
  }

  async forceResolveProposalFromVotes(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.ensureConnected();
    throw new Error(
      "Browser-side admin governance extrinsic submission is not implemented yet",
    );
  }

  async requeueProposalForAutoFinalization(): Promise<void> {
    const connection = await this.ensureConnection();
    await connection.ensureConnected();
    throw new Error(
      "Browser-side admin governance extrinsic submission is not implemented yet",
    );
  }
}
