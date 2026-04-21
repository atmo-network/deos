import { blake2AsHex } from "@polkadot/util-crypto";

import type {
  GovernanceAccountId,
  GovernanceAdapter,
  GovernanceAuthorizedRuntimeUpgrade,
  GovernanceDomainId,
  GovernanceFinalizedProposalOutcome,
  GovernanceGovXpCounters,
  GovernanceItemId,
  GovernancePrimaryTrackOption,
  GovernanceProposalCadenceMode,
  GovernanceProposalExecutionAuthority,
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
  GovernanceProposalOpeningFee,
  GovernanceProposalPayloadKind,
  GovernanceProposalSubmissionAuthority,
  GovernanceProposalExecutionDetail,
  GovernanceProposalMetadata,
  GovernanceProposalPrimaryTrackFamily,
  GovernanceProposalPrimaryTrackTally,
  GovernanceProposalTreasurySpendScalar,
  GovernanceProposalTiming,
  GovernanceProposalPayloadAvailability,
  GovernanceProposalRejectionReason,
  GovernanceProposalResolutionState,
  GovernanceProposalStatus,
  GovernanceProposalVoteTally,
  GovernanceProviderState,
  GovernanceRecentFinalizedProposal,
  GovernanceRewardCoefficient,
  GovernanceVoteKind,
  GovernanceVotePowerProfile,
  GovernanceWriteSurfaceAvailability,
} from "$lib/governance";
import {
  GOVERNANCE_QUERY_SURFACE_AVAILABILITY,
  buildWriteSurfaceAvailability,
} from "$lib/governance";

type OrdinaryVoteKind = Extract<
  GovernanceVoteKind,
  "aye" | "nay" | "amplify" | "approve" | "reduce"
>;
type VetoVoteKind = Extract<GovernanceVoteKind, "veto" | "pass">;
type MockProposal = {
  submittedEpoch: number;
  maturityEpoch: number;
  proposerAccountId: GovernanceAccountId;
  metadata: GovernanceProposalMetadata;
  executionAuthority: GovernanceProposalExecutionAuthority;
  payloadAvailability: GovernanceProposalPayloadAvailability;
  tally: GovernanceProposalVoteTally;
  profiles: Record<GovernanceVoteKind, GovernanceVotePowerProfile>;
  ordinaryVotes: Partial<Record<GovernanceAccountId, OrdinaryVoteKind>>;
  vetoVotes: Partial<Record<GovernanceAccountId, VetoVoteKind>>;
  participants: Partial<Record<GovernanceAccountId, true>>;
};
type MockRetainedFinalizedProposal = GovernanceRecentFinalizedProposal & {
  metadata: GovernanceProposalMetadata | null;
  executionAuthority: GovernanceProposalExecutionAuthority | null;
  payloadAvailability: GovernanceProposalPayloadAvailability | null;
  winningPrimaryOption: GovernancePrimaryTrackOption | null;
};

type MockDomainState = {
  currentEpoch: number;
  activeProposalIds: GovernanceItemId[];
  activeProposals: Record<GovernanceItemId, MockProposal>;
  recentFinalized: MockRetainedFinalizedProposal[];
  govxpCounters: Record<GovernanceAccountId, GovernanceGovXpCounters>;
};

const MAX_RECENT_FINALIZED = 8;
const DEFAULT_VOTING_PERIOD = 6;
const DEFAULT_LEAD_IN_PERIOD = 2;
const DEFAULT_ORDINARY_WEIGHT = 1_800n;
const DEFAULT_VETO_WEIGHT = 900n;
const ORDINARY_MINIMUM_TURNOUT = 2_000n;
const IMMEDIATE_VETO_THRESHOLD = 6_000n;
const FINAL_VETO_MINIMUM = 1_000n;
const ACCOUNT_WEIGHT_OVERRIDES: Record<
  GovernanceAccountId,
  { ordinary: bigint; veto: bigint }
> = {
  alice: { ordinary: 2_100n, veto: 1_000n },
  bob: { ordinary: 1_200n, veto: 700n },
  charlie: { ordinary: 1_500n, veto: 800n },
  dave: { ordinary: 1_300n, veto: 650n },
};
const DEFAULT_PROFILES: Record<GovernanceVoteKind, GovernanceVotePowerProfile> =
  {
    aye: "DecliningDirectStake",
    nay: "DecliningDirectStake",
    amplify: "DecliningDirectStake",
    approve: "DecliningDirectStake",
    reduce: "DecliningDirectStake",
    veto: "DecliningVetoAsset",
    pass: "DecliningVetoAsset",
  };

function createMockProposal(input: {
  submittedEpoch: number;
  maturityEpoch: number;
  proposerAccountId: GovernanceAccountId;
  metadata: GovernanceProposalMetadata;
  executionAuthority: GovernanceProposalExecutionAuthority;
  payloadAvailability: GovernanceProposalPayloadAvailability;
  tally: GovernanceProposalVoteTally;
}): MockProposal {
  return {
    submittedEpoch: input.submittedEpoch,
    maturityEpoch: input.maturityEpoch,
    proposerAccountId: input.proposerAccountId,
    metadata: { ...input.metadata },
    executionAuthority: input.executionAuthority,
    payloadAvailability: { ...input.payloadAvailability },
    tally: { ...input.tally },
    profiles: { ...DEFAULT_PROFILES },
    ordinaryVotes: {},
    vetoVotes: {},
    participants: {},
  };
}

function seededDomainState(): MockDomainState {
  return {
    currentEpoch: 42,
    activeProposalIds: [310, 311],
    activeProposals: {
      310: createMockProposal({
        submittedEpoch: 42,
        maturityEpoch: 48,
        proposerAccountId: "alice",
        metadata: {
          cadenceMode: "Ordinary",
          payloadKind: "L2TreasurySpend",
          payloadHash: "0x" + "11".repeat(32),
        },
        executionAuthority: "DomainTreasury",
        payloadAvailability: {
          havePreimage: true,
          preimageRequested: false,
        },
        tally: {
          ayeVoters: 3,
          nayVoters: 1,
          amplifyVoters: 0,
          approveVoters: 0,
          reduceVoters: 0,
          vetoVoters: 1,
          passVoters: 2,
          ayeWeight: 8_400n,
          nayWeight: 1_200n,
          amplifyWeight: 0n,
          approveWeight: 0n,
          reduceWeight: 0n,
          vetoWeight: 1_900n,
          passWeight: 3_000n,
          turnoutWeight: 9_600n,
          vetoTurnoutWeight: 4_900n,
        },
      }),
      311: createMockProposal({
        submittedEpoch: 38,
        maturityEpoch: 40,
        proposerAccountId: "bob",
        metadata: {
          cadenceMode: "Fast",
          payloadKind: "L2SignalToL1",
          payloadHash: "0x" + "22".repeat(32),
        },
        executionAuthority: "NonExecutable",
        payloadAvailability: {
          havePreimage: false,
          preimageRequested: false,
        },
        tally: {
          ayeVoters: 2,
          nayVoters: 2,
          amplifyVoters: 0,
          approveVoters: 0,
          reduceVoters: 0,
          vetoVoters: 2,
          passVoters: 1,
          ayeWeight: 4_300n,
          nayWeight: 3_800n,
          amplifyWeight: 0n,
          approveWeight: 0n,
          reduceWeight: 0n,
          vetoWeight: 5_400n,
          passWeight: 2_100n,
          turnoutWeight: 8_100n,
          vetoTurnoutWeight: 7_500n,
        },
      }),
    },
    recentFinalized: [
      {
        itemId: 309,
        outcome: {
          kind: "ExecutionFailed",
          approvedEpoch: 41,
          failedEpoch: 41,
          winnerCount: 2,
        },
        executionDetail: {
          kind: "ExecutionFailed",
          payloadKind: "L1RootAction",
          authority: "Root",
          failedEpoch: 41,
          reason: "UnsupportedCall",
        },
        metadata: {
          cadenceMode: "Fast",
          payloadKind: "L1RootAction",
          payloadHash: "0x" + "33".repeat(32),
        },
        executionAuthority: "Root",
        payloadAvailability: {
          havePreimage: true,
          preimageRequested: false,
        },
        winningPrimaryOption: null,
      },
      {
        itemId: 308,
        outcome: {
          kind: "Enacted",
          approvedEpoch: 40,
          executedEpoch: 40,
          winnerCount: 2,
        },
        executionDetail: {
          kind: "Executed",
          payloadKind: "L2TreasurySpend",
          authority: "DomainTreasury",
          executedEpoch: 40,
          detail: {
            kind: "TreasurySpendExecuted",
            fundingSource: "bldr-treasury",
            beneficiary: "alice",
            payoutAsset: 1000,
            baseAmount: 25000000000000n,
            scalar: "Approve",
            finalAmount: 25000000000000n,
            settlementKind: "InvoiceScalarTransfer",
          },
        },
        metadata: {
          cadenceMode: "Ordinary",
          payloadKind: "L2TreasurySpend",
          payloadHash: "0x" + "44".repeat(32),
        },
        executionAuthority: "DomainTreasury",
        payloadAvailability: {
          havePreimage: true,
          preimageRequested: false,
        },
        winningPrimaryOption: "Approve",
      },
      {
        itemId: 307,
        outcome: {
          kind: "VetoCancelled",
          epoch: 39,
          vetoWeight: 6_000n,
        },
        executionDetail: null,
        metadata: {
          cadenceMode: "Ordinary",
          payloadKind: "L2SignalToL1",
          payloadHash: "0x" + "55".repeat(32),
        },
        executionAuthority: "NonExecutable",
        payloadAvailability: {
          havePreimage: false,
          preimageRequested: false,
        },
        winningPrimaryOption: null,
      },
    ],
    govxpCounters: {
      alice: {
        rollingWinningParticipation: 5,
        totalParticipations: 11n,
        totalWinningParticipations: 7n,
        totalAuthoredProposals: 3n,
        totalSuccessfulAuthoredProposals: 2n,
      },
      bob: {
        rollingWinningParticipation: 2,
        totalParticipations: 5n,
        totalWinningParticipations: 2n,
        totalAuthoredProposals: 1n,
        totalSuccessfulAuthoredProposals: 1n,
      },
    },
  };
}

function emptyDomainState(): MockDomainState {
  return {
    currentEpoch: 1,
    activeProposalIds: [],
    activeProposals: {},
    recentFinalized: [],
    govxpCounters: {},
  };
}

function cloneProposal(proposal: MockProposal): MockProposal {
  return {
    submittedEpoch: proposal.submittedEpoch,
    maturityEpoch: proposal.maturityEpoch,
    proposerAccountId: proposal.proposerAccountId,
    metadata: { ...proposal.metadata },
    executionAuthority: proposal.executionAuthority,
    payloadAvailability: { ...proposal.payloadAvailability },
    tally: { ...proposal.tally },
    profiles: { ...proposal.profiles },
    ordinaryVotes: { ...proposal.ordinaryVotes },
    vetoVotes: { ...proposal.vetoVotes },
    participants: { ...proposal.participants },
  };
}

function cloneDomainState(domain: MockDomainState): MockDomainState {
  return {
    currentEpoch: domain.currentEpoch,
    activeProposalIds: [...domain.activeProposalIds],
    activeProposals: Object.fromEntries(
      Object.entries(domain.activeProposals).map(([itemId, proposal]) => [
        Number(itemId),
        cloneProposal(proposal),
      ]),
    ),
    recentFinalized: domain.recentFinalized.map((proposal) => ({
      itemId: proposal.itemId,
      outcome: { ...proposal.outcome },
      executionDetail: proposal.executionDetail
        ? structuredClone(proposal.executionDetail)
        : null,
      metadata: proposal.metadata ? { ...proposal.metadata } : null,
      executionAuthority: proposal.executionAuthority,
      payloadAvailability: proposal.payloadAvailability
        ? { ...proposal.payloadAvailability }
        : null,
      winningPrimaryOption: proposal.winningPrimaryOption,
    })),
    govxpCounters: Object.fromEntries(
      Object.entries(domain.govxpCounters).map(([accountId, counters]) => [
        accountId,
        { ...counters },
      ]),
    ),
  };
}

function ratioString(numerator: bigint, denominator: bigint): string {
  const scale = 10n ** 18n;
  const scaled = (numerator * scale) / denominator;
  const whole = scaled / scale;
  const fraction = (scaled % scale).toString().padStart(18, "0");
  return `${whole}.${fraction}`;
}

function rewardCoefficientForCounters(
  counters: GovernanceGovXpCounters,
): GovernanceRewardCoefficient {
  return ratioString(
    BigInt(Math.min(counters.rollingWinningParticipation, 12)),
    12n,
  );
}

function ensureCounters(
  domain: MockDomainState,
  accountId: GovernanceAccountId,
): GovernanceGovXpCounters {
  domain.govxpCounters[accountId] ??= {
    rollingWinningParticipation: 0,
    totalParticipations: 0n,
    totalWinningParticipations: 0n,
    totalAuthoredProposals: 0n,
    totalSuccessfulAuthoredProposals: 0n,
  };
  return domain.govxpCounters[accountId];
}

function accountWeights(accountId: GovernanceAccountId) {
  return (
    ACCOUNT_WEIGHT_OVERRIDES[accountId] ?? {
      ordinary: DEFAULT_ORDINARY_WEIGHT,
      veto: DEFAULT_VETO_WEIGHT,
    }
  );
}

function voteWeight(
  accountId: GovernanceAccountId,
  voteKind: GovernanceVoteKind,
): bigint {
  const weights = accountWeights(accountId);
  return voteKind === "aye" || voteKind === "nay"
    ? weights.ordinary
    : weights.veto;
}

function protectionTrackCloseEpoch(proposal: MockProposal): number {
  return proposal.submittedEpoch + DEFAULT_VOTING_PERIOD;
}

function protectionTrackIsClosed(
  domain: MockDomainState,
  proposal: MockProposal,
): boolean {
  return domain.currentEpoch > protectionTrackCloseEpoch(proposal);
}

function proposalTimingForProposal(proposal: MockProposal): GovernanceProposalTiming {
  const ordinaryPrimaryOpenEpoch = proposal.metadata.cadenceMode === "Fast"
    ? proposal.submittedEpoch
    : proposal.submittedEpoch + DEFAULT_LEAD_IN_PERIOD;
  const ordinaryPrimaryCloseEpoch = proposal.maturityEpoch;
  const urgentPrimaryOpenEpoch = proposal.metadata.cadenceMode === "Fast"
    ? proposal.submittedEpoch
    : null;
  const urgentPrimaryCloseEpoch = proposal.metadata.cadenceMode === "Fast"
    ? proposal.maturityEpoch
    : null;
  return {
    submittedEpoch: proposal.submittedEpoch,
    protectionOpenEpoch: proposal.submittedEpoch,
    protectionCloseEpoch: proposal.submittedEpoch + DEFAULT_VOTING_PERIOD,
    ordinaryPrimaryOpenEpoch,
    ordinaryPrimaryCloseEpoch,
    urgentPrimaryOpenEpoch,
    urgentPrimaryCloseEpoch,
    effectivePrimaryOpenEpoch: urgentPrimaryOpenEpoch ?? ordinaryPrimaryOpenEpoch,
    effectivePrimaryCloseEpoch: urgentPrimaryCloseEpoch ?? ordinaryPrimaryCloseEpoch,
    pendingEnactmentEpoch: null,
  };
}

function proposalSubmissionAuthority(
  domainId: GovernanceDomainId,
  payloadKind: GovernanceProposalPayloadKind,
): GovernanceProposalSubmissionAuthority {
  if (payloadKind === "Intent") {
    return "Signed";
  }
  if (domainId === 1000 && payloadKind === "L2SignalToL1") {
    return "Signed";
  }
  return "AdminOnly";
}

function proposalOpeningFee(
  domainId: GovernanceDomainId,
  payloadKind: GovernanceProposalPayloadKind,
): GovernanceProposalOpeningFee | null {
  return proposalSubmissionAuthority(domainId, payloadKind) === "Signed" ? 10n : null;
}

function proposalPrimaryTrackFamily(
  domainId: GovernanceDomainId,
  proposal: MockProposal,
): GovernanceProposalPrimaryTrackFamily {
  if (domainId === 43 && proposal.metadata.payloadKind === "L2TreasurySpend") {
    return "Invoice";
  }
  return "Binary";
}

function invoiceLeadingPositiveOption(
  proposal: MockProposal,
): { option: GovernancePrimaryTrackOption | null; weight: bigint } {
  let option: GovernancePrimaryTrackOption | null = null;
  let weight = 0n;
  if (proposal.tally.amplifyWeight > 0n) {
    option = "Amplify";
    weight = proposal.tally.amplifyWeight;
  }
  if (proposal.tally.approveWeight > 0n && proposal.tally.approveWeight >= weight) {
    option = "Approve";
    weight = proposal.tally.approveWeight;
  }
  if (proposal.tally.reduceWeight > 0n && proposal.tally.reduceWeight >= weight) {
    option = "Reduce";
    weight = proposal.tally.reduceWeight;
  }
  return { option, weight };
}

function proposalPrimaryTrackTally(
  domainId: GovernanceDomainId,
  proposal: MockProposal,
): GovernanceProposalPrimaryTrackTally {
  const family = proposalPrimaryTrackFamily(domainId, proposal);
  if (family === "Binary") {
    return {
      kind: "Binary",
      ayeVoters: proposal.tally.ayeVoters,
      nayVoters: proposal.tally.nayVoters,
      ayeWeight: proposal.tally.ayeWeight,
      nayWeight: proposal.tally.nayWeight,
      turnoutWeight: proposal.tally.turnoutWeight,
      leadingOption:
        proposal.tally.ayeWeight > proposal.tally.nayWeight
          ? "Aye"
          : proposal.tally.nayWeight > proposal.tally.ayeWeight
            ? "Nay"
            : null,
    };
  }
  const leader = invoiceLeadingPositiveOption(proposal);
  return {
    kind: "Invoice",
    amplifyVoters: proposal.tally.amplifyVoters,
    approveVoters: proposal.tally.approveVoters,
    reduceVoters: proposal.tally.reduceVoters,
    nayVoters: proposal.tally.nayVoters,
    amplifyWeight: proposal.tally.amplifyWeight,
    approveWeight: proposal.tally.approveWeight,
    reduceWeight: proposal.tally.reduceWeight,
    nayWeight: proposal.tally.nayWeight,
    positiveWeight:
      proposal.tally.amplifyWeight +
      proposal.tally.approveWeight +
      proposal.tally.reduceWeight,
    turnoutWeight: proposal.tally.turnoutWeight,
    leadingPositiveOption: leader.option,
    leadingPositiveWeight: leader.weight,
  };
}

function proposalUrgentEligibility(
  domainId: GovernanceDomainId,
  proposal: MockProposal,
): boolean {
  return domainId === 42 && proposal.metadata.payloadKind === "L1RootAction";
}

function resolutionForProposal(
  domainId: GovernanceDomainId,
  domain: MockDomainState,
  proposal: MockProposal,
  enforceVotingWindow: boolean,
): GovernanceProposalResolutionState {
  const family = proposalPrimaryTrackFamily(domainId, proposal);
  if (
    proposal.tally.vetoWeight > IMMEDIATE_VETO_THRESHOLD &&
    proposal.tally.vetoWeight > proposal.tally.passWeight
  ) {
    return {
      kind: "VetoPassing",
      vetoWeight: proposal.tally.vetoWeight,
      passWeight: proposal.tally.passWeight,
      mode: "ImmediateThreshold",
    };
  }
  if (enforceVotingWindow && domain.currentEpoch < proposal.maturityEpoch) {
    return {
      kind: "VotingWindowOpen",
      currentEpoch: domain.currentEpoch,
      maturityEpoch: proposal.maturityEpoch,
    };
  }
  if (
    proposal.tally.vetoWeight >= FINAL_VETO_MINIMUM &&
    proposal.tally.vetoWeight >= proposal.tally.passWeight &&
    proposal.tally.vetoTurnoutWeight > 0n
  ) {
    return {
      kind: "VetoPassing",
      vetoWeight: proposal.tally.vetoWeight,
      passWeight: proposal.tally.passWeight,
      mode: "TrackOutcome",
    };
  }
  if (proposal.tally.turnoutWeight === 0n) {
    return {
      kind: "Rejected",
      reason: "NoVotes",
    };
  }
  if (family === "Invoice") {
    const positiveWeight =
      proposal.tally.amplifyWeight +
      proposal.tally.approveWeight +
      proposal.tally.reduceWeight;
    if (positiveWeight === proposal.tally.nayWeight) {
      return {
        kind: "Rejected",
        reason: "VoteTie",
      };
    }
    if (proposal.tally.turnoutWeight < ORDINARY_MINIMUM_TURNOUT) {
      return {
        kind: "Rejected",
        reason: "TurnoutBelowMinimum",
      };
    }
    if (positiveWeight <= proposal.tally.nayWeight) {
      return {
        kind: "Rejected",
        reason: "ApprovalThresholdNotMet",
      };
    }
    if (positiveWeight * 100n < proposal.tally.turnoutWeight * 60n) {
      return {
        kind: "Rejected",
        reason: "ApprovalThresholdNotMet",
      };
    }
    switch (invoiceLeadingPositiveOption(proposal).option) {
      case "Amplify":
        return { kind: "PassingAmplify" };
      case "Approve":
        return { kind: "PassingApprove" };
      case "Reduce":
        return { kind: "PassingReduce" };
      default:
        return {
          kind: "Rejected",
          reason: "ApprovalThresholdNotMet",
        };
    }
  }
  if (proposal.tally.ayeWeight === proposal.tally.nayWeight) {
    return {
      kind: "Rejected",
      reason: "VoteTie",
    };
  }
  if (proposal.tally.turnoutWeight < ORDINARY_MINIMUM_TURNOUT) {
    return {
      kind: "Rejected",
      reason: "TurnoutBelowMinimum",
    };
  }
  if (proposal.tally.ayeWeight * 100n >= proposal.tally.turnoutWeight * 60n) {
    return { kind: "PassingAye" };
  }
  if (proposal.tally.nayWeight * 100n >= proposal.tally.turnoutWeight * 60n) {
    return { kind: "PassingNay" };
  }
  return {
    kind: "Rejected",
    reason: "ApprovalThresholdNotMet",
  };
}

function decrementVote(
  tally: GovernanceProposalVoteTally,
  voteKind: GovernanceVoteKind,
  weight: bigint,
) {
  switch (voteKind) {
    case "aye":
      tally.ayeVoters = Math.max(0, tally.ayeVoters - 1);
      tally.ayeWeight =
        tally.ayeWeight > weight ? tally.ayeWeight - weight : 0n;
      tally.turnoutWeight =
        tally.turnoutWeight > weight ? tally.turnoutWeight - weight : 0n;
      return;
    case "nay":
      tally.nayVoters = Math.max(0, tally.nayVoters - 1);
      tally.nayWeight =
        tally.nayWeight > weight ? tally.nayWeight - weight : 0n;
      tally.turnoutWeight =
        tally.turnoutWeight > weight ? tally.turnoutWeight - weight : 0n;
      return;
    case "amplify":
      tally.amplifyVoters = Math.max(0, tally.amplifyVoters - 1);
      tally.amplifyWeight =
        tally.amplifyWeight > weight ? tally.amplifyWeight - weight : 0n;
      tally.turnoutWeight =
        tally.turnoutWeight > weight ? tally.turnoutWeight - weight : 0n;
      return;
    case "approve":
      tally.approveVoters = Math.max(0, tally.approveVoters - 1);
      tally.approveWeight =
        tally.approveWeight > weight ? tally.approveWeight - weight : 0n;
      tally.turnoutWeight =
        tally.turnoutWeight > weight ? tally.turnoutWeight - weight : 0n;
      return;
    case "reduce":
      tally.reduceVoters = Math.max(0, tally.reduceVoters - 1);
      tally.reduceWeight =
        tally.reduceWeight > weight ? tally.reduceWeight - weight : 0n;
      tally.turnoutWeight =
        tally.turnoutWeight > weight ? tally.turnoutWeight - weight : 0n;
      return;
    case "veto":
      tally.vetoVoters = Math.max(0, tally.vetoVoters - 1);
      tally.vetoWeight =
        tally.vetoWeight > weight ? tally.vetoWeight - weight : 0n;
      tally.vetoTurnoutWeight =
        tally.vetoTurnoutWeight > weight
          ? tally.vetoTurnoutWeight - weight
          : 0n;
      return;
    case "pass":
      tally.passVoters = Math.max(0, tally.passVoters - 1);
      tally.passWeight =
        tally.passWeight > weight ? tally.passWeight - weight : 0n;
      tally.vetoTurnoutWeight =
        tally.vetoTurnoutWeight > weight
          ? tally.vetoTurnoutWeight - weight
          : 0n;
  }
}

function incrementVote(
  tally: GovernanceProposalVoteTally,
  voteKind: GovernanceVoteKind,
  weight: bigint,
) {
  switch (voteKind) {
    case "aye":
      tally.ayeVoters += 1;
      tally.ayeWeight += weight;
      tally.turnoutWeight += weight;
      return;
    case "nay":
      tally.nayVoters += 1;
      tally.nayWeight += weight;
      tally.turnoutWeight += weight;
      return;
    case "amplify":
      tally.amplifyVoters += 1;
      tally.amplifyWeight += weight;
      tally.turnoutWeight += weight;
      return;
    case "approve":
      tally.approveVoters += 1;
      tally.approveWeight += weight;
      tally.turnoutWeight += weight;
      return;
    case "reduce":
      tally.reduceVoters += 1;
      tally.reduceWeight += weight;
      tally.turnoutWeight += weight;
      return;
    case "veto":
      tally.vetoVoters += 1;
      tally.vetoWeight += weight;
      tally.vetoTurnoutWeight += weight;
      return;
    case "pass":
      tally.passVoters += 1;
      tally.passWeight += weight;
      tally.vetoTurnoutWeight += weight;
  }
}

function winningAccounts(
  proposal: MockProposal,
  resolution: GovernanceProposalResolutionState,
): GovernanceAccountId[] {
  switch (resolution.kind) {
    case "PassingAye":
      return Object.entries(proposal.ordinaryVotes)
        .filter(([, voteKind]) => voteKind === "aye")
        .map(([accountId]) => accountId);
    case "PassingAmplify":
      return Object.entries(proposal.ordinaryVotes)
        .filter(([, voteKind]) => voteKind === "amplify")
        .map(([accountId]) => accountId);
    case "PassingApprove":
      return Object.entries(proposal.ordinaryVotes)
        .filter(([, voteKind]) => voteKind === "approve")
        .map(([accountId]) => accountId);
    case "PassingReduce":
      return Object.entries(proposal.ordinaryVotes)
        .filter(([, voteKind]) => voteKind === "reduce")
        .map(([accountId]) => accountId);
    case "PassingNay":
      return Object.entries(proposal.ordinaryVotes)
        .filter(([, voteKind]) => voteKind === "nay")
        .map(([accountId]) => accountId);
    case "VetoPassing":
      return Object.entries(proposal.vetoVotes)
        .filter(([, voteKind]) => voteKind === "veto")
        .map(([accountId]) => accountId);
    case "Confirming":
    case "Rejected":
    case "VotingWindowOpen":
      return [];
  }
}

function applyWinningParticipation(
  domain: MockDomainState,
  proposal: MockProposal,
  resolution: GovernanceProposalResolutionState,
) {
  const winners = winningAccounts(proposal, resolution);
  for (const accountId of winners) {
    const counters = ensureCounters(domain, accountId);
    counters.rollingWinningParticipation += 1;
    counters.totalWinningParticipations += 1n;
  }
}

function winningPrimaryOptionForResolution(
  resolution: GovernanceProposalResolutionState,
): GovernancePrimaryTrackOption | null {
  switch (resolution.kind) {
    case "PassingAye":
      return "Aye";
    case "PassingAmplify":
      return "Amplify";
    case "PassingApprove":
      return "Approve";
    case "PassingReduce":
      return "Reduce";
    case "PassingNay":
      return "Nay";
    default:
      return null;
  }
}

function finalizeProposal(
  domain: MockDomainState,
  itemId: GovernanceItemId,
  outcome: GovernanceFinalizedProposalOutcome,
  winningPrimaryOption: GovernancePrimaryTrackOption | null = null,
) {
  const proposal = domain.activeProposals[itemId];
  if (proposal && (outcome.kind === "Resolved" || outcome.kind === "Enacted")) {
    ensureCounters(
      domain,
      proposal.proposerAccountId,
    ).totalSuccessfulAuthoredProposals += 1n;
  }
  delete domain.activeProposals[itemId];
  domain.activeProposalIds = domain.activeProposalIds.filter(
    (activeItemId) => activeItemId !== itemId,
  );
  domain.recentFinalized = [
    {
      itemId,
      outcome,
      executionDetail: null,
      metadata: proposal ? { ...proposal.metadata } : null,
      executionAuthority: proposal?.executionAuthority ?? null,
      payloadAvailability: proposal ? { ...proposal.payloadAvailability } : null,
      winningPrimaryOption,
    },
    ...domain.recentFinalized.filter(
      (proposalEntry) => proposalEntry.itemId !== itemId,
    ),
  ].slice(0, MAX_RECENT_FINALIZED);
}

function adminRejectedOutcome(
  epoch: number,
): GovernanceFinalizedProposalOutcome {
  return {
    kind: "Rejected",
    epoch,
    reason: "AdminRejected",
  };
}

function settlementOutcome(
  domain: MockDomainState,
  proposal: MockProposal,
  resolution: GovernanceProposalResolutionState,
): GovernanceFinalizedProposalOutcome {
  switch (resolution.kind) {
    case "PassingAye":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.ayeVoters,
      };
    case "PassingAmplify":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.amplifyVoters,
      };
    case "PassingApprove":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.approveVoters,
      };
    case "PassingReduce":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.reduceVoters,
      };
    case "PassingNay":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.nayVoters,
      };
    case "Confirming":
      return {
        kind: "Resolved",
        epoch: domain.currentEpoch,
        winnerCount: proposal.tally.ayeVoters,
      };
    case "Rejected":
      return {
        kind: "Rejected",
        epoch: domain.currentEpoch,
        reason: resolution.reason,
      };
    case "VetoPassing":
      return {
        kind: "VetoCancelled",
        epoch: domain.currentEpoch,
        vetoWeight: proposal.tally.vetoWeight,
      };
    case "VotingWindowOpen":
      throw new Error("Voting window is still open");
  }
}

function availableWriteSurface(): GovernanceWriteSurfaceAvailability {
  return buildWriteSurfaceAvailability({
    castVote: { providerStatus: "available", reason: "Stateful mock provider allows interactive vote previews" },
    submitProposal: { providerStatus: "available", reason: "Stateful mock provider allows interactive signed public submission previews" },
    noteProposalPreimage: { providerStatus: "available", reason: "Stateful mock provider allows interactive preimage-note previews" },
    resolveProposal: { providerStatus: "available", reason: "Stateful mock provider allows manual resolution previews" },
    rejectProposal: { providerStatus: "available", reason: "Stateful mock provider allows admin rejection previews" },
    resolveProposalFromVotes: { providerStatus: "available", reason: "Stateful mock provider allows maturity-time resolution previews" },
    forceResolveProposalFromVotes: { providerStatus: "available", reason: "Stateful mock provider allows early-finalization previews" },
    requeueProposalForAutoFinalization: { providerStatus: "available", reason: "Stateful mock provider allows auto-finalization recovery previews" },
  });
}

export class GovernanceMockAdapter implements GovernanceAdapter {
  private readonly domains = new Map<GovernanceDomainId, MockDomainState>([
    [1000, cloneDomainState(seededDomainState())],
  ]);
  private readonly notedPreimageHashes = new Map<string, number>();
  private readonly state: GovernanceProviderState = {
    status: "mock",
    label: "Mock governance provider",
    endpoint: null,
    chainName: "Mock DEOS preview",
    nodeName: "In-memory adapter",
    nodeVersion: "preview",
    genesisHash: null,
    finalizedBlockHash: null,
    finalizedBlockNumber: null,
    message: "Local interactive governance preview",
  };

  private domain(domainId: GovernanceDomainId): MockDomainState {
    const existing = this.domains.get(domainId);
    if (existing) {
      return existing;
    }
    const created = emptyDomainState();
    this.domains.set(domainId, created);
    return created;
  }

  async syncProviderState(): Promise<void> {}

  subscribeToUpdates(): () => void {
    return () => {};
  }

  getProviderState(): GovernanceProviderState {
    return this.state;
  }

  getProviderLabel(): string {
    return this.state.label;
  }

  getQuerySurfaceAvailability() {
    return GOVERNANCE_QUERY_SURFACE_AVAILABILITY;
  }

  getWriteSurfaceAvailability(): GovernanceWriteSurfaceAvailability {
    return availableWriteSurface();
  }

  async getActiveProposalIds(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceItemId[]> {
    return [...this.domain(domainId).activeProposalIds];
  }

  async getRecentFinalizedProposals(
    domainId: GovernanceDomainId,
  ): Promise<GovernanceRecentFinalizedProposal[]> {
    return [...this.domain(domainId).recentFinalized];
  }

  async getProposalStatus(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalStatus | null> {
    const domain = this.domain(domainId);
    const proposal = domain.activeProposals[itemId];
    if (proposal) {
      return {
        kind: "Active",
        resolution: resolutionForProposal(domainId, domain, proposal, true),
      };
    }
    const finalizedProposal = domain.recentFinalized.find(
      (recentProposal) => recentProposal.itemId === itemId,
    );
    if (!finalizedProposal) {
      return null;
    }
    return {
      kind: "Finalized",
      outcome: finalizedProposal.outcome,
    };
  }

  async getProposalMetadata(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalMetadata | null> {
    const domain = this.domain(domainId);
    const proposal = domain.activeProposals[itemId];
    if (proposal) {
      return { ...proposal.metadata };
    }
    const retainedProposal = domain.recentFinalized.find(
      (recentProposal) => recentProposal.itemId === itemId,
    );
    return retainedProposal?.metadata ? { ...retainedProposal.metadata } : null;
  }

  async getProposalExecutionAuthority(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionAuthority | null> {
    const domain = this.domain(domainId);
    return (
      domain.activeProposals[itemId]?.executionAuthority ??
      domain.recentFinalized.find((proposal) => proposal.itemId === itemId)
        ?.executionAuthority ??
      null
    );
  }

  async getAuthorizedRuntimeUpgrade(): Promise<GovernanceAuthorizedRuntimeUpgrade | null> {
    return null;
  }

  async getProposalSubmissionAuthority(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalSubmissionAuthority | null> {
    return proposalSubmissionAuthority(domainId, payloadKind);
  }

  async getProposalOpeningFee(
    domainId: GovernanceDomainId,
    payloadKind: GovernanceProposalPayloadKind,
  ): Promise<GovernanceProposalOpeningFee | null> {
    return proposalOpeningFee(domainId, payloadKind);
  }

  async getProposalPayloadAvailability(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPayloadAvailability | null> {
    const domain = this.domain(domainId);
    const proposal = domain.activeProposals[itemId];
    if (proposal) {
      return { ...proposal.payloadAvailability };
    }
    const retainedProposal = domain.recentFinalized.find(
      (recentProposal) => recentProposal.itemId === itemId,
    );
    return retainedProposal?.payloadAvailability
      ? { ...retainedProposal.payloadAvailability }
      : null;
  }

  async getPayloadHashPreimageStatus(
    payloadHash: string,
  ): Promise<GovernancePayloadHashPreimageStatus | null> {
    const byteLength = this.notedPreimageHashes.get(payloadHash) ?? null;
    return {
      havePreimage: byteLength !== null,
      preimageRequested: false,
      byteLength,
    };
  }

  async getPayloadPreimageNoteCost(
    payloadLen: number,
  ): Promise<GovernancePayloadPreimageNoteCost | null> {
    return 2n + BigInt(payloadLen);
  }

  async getProposalPrimaryTrackFamily(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackFamily | null> {
    const domain = this.domain(domainId);
    const proposal = domain.activeProposals[itemId];
    if (proposal) {
      return proposalPrimaryTrackFamily(domainId, proposal);
    }
    const retainedProposal = domain.recentFinalized.find(
      (recentProposal) => recentProposal.itemId === itemId,
    );
    if (!retainedProposal?.metadata) {
      return null;
    }
    return proposalPrimaryTrackFamily(domainId, {
      submittedEpoch: 0,
      maturityEpoch: 0,
      proposerAccountId: "mock",
      metadata: retainedProposal.metadata,
      executionAuthority: retainedProposal.executionAuthority ?? "NonExecutable",
      payloadAvailability: retainedProposal.payloadAvailability ?? {
        havePreimage: false,
        preimageRequested: false,
      },
      tally: {
        ayeVoters: 0,
        nayVoters: 0,
        amplifyVoters: 0,
        approveVoters: 0,
        reduceVoters: 0,
        vetoVoters: 0,
        passVoters: 0,
        ayeWeight: 0n,
        nayWeight: 0n,
        amplifyWeight: 0n,
        approveWeight: 0n,
        reduceWeight: 0n,
        vetoWeight: 0n,
        passWeight: 0n,
        turnoutWeight: 0n,
        vetoTurnoutWeight: 0n,
      },
      profiles: { ...DEFAULT_PROFILES },
      ordinaryVotes: {},
      vetoVotes: {},
      participants: {},
    });
  }

  async getProposalTiming(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalTiming | null> {
    const proposal = this.domain(domainId).activeProposals[itemId];
    return proposal ? proposalTimingForProposal(proposal) : null;
  }

  async getProposalUrgentEligibility(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<boolean | null> {
    const domain = this.domain(domainId);
    const proposal = domain.activeProposals[itemId];
    if (proposal) {
      return proposalUrgentEligibility(domainId, proposal);
    }
    const retainedProposal = domain.recentFinalized.find(
      (recentProposal) => recentProposal.itemId === itemId,
    );
    if (!retainedProposal?.metadata) {
      return null;
    }
    return proposalUrgentEligibility(domainId, {
      submittedEpoch: 0,
      maturityEpoch: 0,
      proposerAccountId: "mock",
      metadata: retainedProposal.metadata,
      executionAuthority: retainedProposal.executionAuthority ?? "NonExecutable",
      payloadAvailability: retainedProposal.payloadAvailability ?? {
        havePreimage: false,
        preimageRequested: false,
      },
      tally: {
        ayeVoters: 0,
        nayVoters: 0,
        amplifyVoters: 0,
        approveVoters: 0,
        reduceVoters: 0,
        vetoVoters: 0,
        passVoters: 0,
        ayeWeight: 0n,
        nayWeight: 0n,
        amplifyWeight: 0n,
        approveWeight: 0n,
        reduceWeight: 0n,
        vetoWeight: 0n,
        passWeight: 0n,
        turnoutWeight: 0n,
        vetoTurnoutWeight: 0n,
      },
      profiles: { ...DEFAULT_PROFILES },
      ordinaryVotes: {},
      vetoVotes: {},
      participants: {},
    });
  }

  async getProposalPrimaryTrackTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalPrimaryTrackTally | null> {
    const proposal = this.domain(domainId).activeProposals[itemId];
    return proposal ? proposalPrimaryTrackTally(domainId, proposal) : null;
  }

  async getProposalTally(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalVoteTally | null> {
    const proposal = this.domain(domainId).activeProposals[itemId];
    return proposal ? { ...proposal.tally } : null;
  }

  async getProposalWinningPrimaryOption(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernancePrimaryTrackOption | null> {
    return (
      this.domain(domainId).recentFinalized.find(
        (proposal) => proposal.itemId === itemId,
      )?.winningPrimaryOption ?? null
    );
  }

  async getProposalExecutionDetail(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceProposalExecutionDetail | null> {
    return (
      this.domain(domainId).recentFinalized.find(
        (proposal) => proposal.itemId === itemId,
      )?.executionDetail ?? null
    );
  }

  async getProposalVotePowerProfile(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
    voteKind: GovernanceVoteKind,
  ): Promise<GovernanceVotePowerProfile | null> {
    return (
      this.domain(domainId).activeProposals[itemId]?.profiles[voteKind] ?? null
    );
  }

  async getRewardCoefficient(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceRewardCoefficient | null> {
    const counters = this.domain(domainId).govxpCounters[accountId];
    return counters ? rewardCoefficientForCounters(counters) : null;
  }

  async getGovXpCounters(
    domainId: GovernanceDomainId,
    accountId: GovernanceAccountId,
  ): Promise<GovernanceGovXpCounters> {
    return {
      ...ensureCounters(this.domain(domainId), accountId),
    };
  }

  async castVote(input: {
    accountId: GovernanceAccountId;
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    voteKind: GovernanceVoteKind;
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    const proposal = domain.activeProposals[input.itemId];
    if (!proposal) {
      throw new Error(
        `Mock proposal #${input.itemId} is not active in domain ${input.domainId}`,
      );
    }
    const weight = voteWeight(input.accountId, input.voteKind);
    if (!proposal.participants[input.accountId]) {
      proposal.participants[input.accountId] = true;
      ensureCounters(domain, input.accountId).totalParticipations += 1n;
    }
    if (
      input.voteKind === "aye" ||
      input.voteKind === "nay" ||
      input.voteKind === "amplify" ||
      input.voteKind === "approve" ||
      input.voteKind === "reduce"
    ) {
      const existingVote = proposal.ordinaryVotes[input.accountId];
      if (existingVote) {
        throw new Error(
          "Mock preview keeps one immutable ordinary vote per account/proposal",
        );
      }
      proposal.ordinaryVotes[input.accountId] = input.voteKind;
      incrementVote(proposal.tally, input.voteKind, weight);
    } else {
      if (protectionTrackIsClosed(domain, proposal)) {
        throw new Error(
          "Protection track is closed because the mock protection window has ended",
        );
      }
      const existingVote = proposal.vetoVotes[input.accountId];
      if (existingVote === input.voteKind) {
        throw new Error("Duplicate veto-track vote in mock preview");
      }
      if (existingVote) {
        decrementVote(
          proposal.tally,
          existingVote,
          voteWeight(input.accountId, existingVote),
        );
      }
      proposal.vetoVotes[input.accountId] = input.voteKind;
      incrementVote(proposal.tally, input.voteKind, weight);
    }
    const resolution = resolutionForProposal(
      input.domainId,
      domain,
      proposal,
      true,
    );
    if (
      resolution.kind === "VetoPassing" &&
      resolution.mode === "ImmediateThreshold"
    ) {
      applyWinningParticipation(domain, proposal, resolution);
      finalizeProposal(
        domain,
        input.itemId,
        settlementOutcome(domain, proposal, resolution),
        winningPrimaryOptionForResolution(resolution),
      );
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
    const domain = this.domain(input.domainId);
    if (proposalSubmissionAuthority(input.domainId, input.payloadKind) !== "Signed") {
      throw new Error(
        `${input.payloadKind} is not publicly submittable in mock governance for domain ${input.domainId}`,
      );
    }
    if (domain.activeProposals[input.itemId]) {
      throw new Error(`Mock proposal #${input.itemId} is already active`);
    }
    domain.currentEpoch += 1;
    domain.activeProposalIds = [input.itemId, ...domain.activeProposalIds];
    ensureCounters(domain, input.accountId).totalAuthoredProposals += 1n;
    domain.activeProposals[input.itemId] = createMockProposal({
      submittedEpoch: domain.currentEpoch,
      maturityEpoch: domain.currentEpoch + DEFAULT_VOTING_PERIOD,
      proposerAccountId: input.accountId,
      metadata: {
        cadenceMode: input.cadenceMode,
        payloadKind: input.payloadKind,
        payloadHash: input.payloadHash,
      },
      executionAuthority:
        input.payloadKind === "Intent" || input.payloadKind === "L2SignalToL1"
          ? "NonExecutable"
          : "DomainParameters",
      payloadAvailability: {
        havePreimage: this.notedPreimageHashes.has(input.payloadHash),
        preimageRequested: false,
      },
      tally: {
        ayeVoters: 0,
        nayVoters: 0,
        amplifyVoters: 0,
        approveVoters: 0,
        reduceVoters: 0,
        vetoVoters: 0,
        passVoters: 0,
        ayeWeight: 0n,
        nayWeight: 0n,
        amplifyWeight: 0n,
        approveWeight: 0n,
        reduceWeight: 0n,
        vetoWeight: 0n,
        passWeight: 0n,
        turnoutWeight: 0n,
        vetoTurnoutWeight: 0n,
      },
    });
    domain.recentFinalized = domain.recentFinalized.filter(
      (proposal) => proposal.itemId !== input.itemId,
    );
  }

  async noteProposalPreimage(input: {
    accountId: GovernanceAccountId;
    payloadBytes: Uint8Array;
  }): Promise<void> {
    const payloadHash = blake2AsHex(input.payloadBytes, 256);
    this.notedPreimageHashes.set(payloadHash, input.payloadBytes.length);
    for (const domain of this.domains.values()) {
      for (const proposal of Object.values(domain.activeProposals)) {
        if (proposal.metadata.payloadHash === payloadHash) {
          proposal.payloadAvailability = {
            havePreimage: true,
            preimageRequested: false,
          };
        }
      }
      for (const proposal of domain.recentFinalized) {
        if (proposal.metadata?.payloadHash === payloadHash) {
          proposal.payloadAvailability = {
            havePreimage: true,
            preimageRequested: false,
          };
        }
      }
    }
  }

  async resolveProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
    winners: GovernanceAccountId[];
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    const proposal = domain.activeProposals[input.itemId];
    if (!proposal) {
      throw new Error(`Mock proposal #${input.itemId} is not active`);
    }
    finalizeProposal(domain, input.itemId, {
      kind: "Resolved",
      epoch: domain.currentEpoch,
      winnerCount: Math.max(input.winners.length, 1),
    });
  }

  async rejectProposal(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    if (!domain.activeProposals[input.itemId]) {
      throw new Error(`Mock proposal #${input.itemId} is not active`);
    }
    finalizeProposal(
      domain,
      input.itemId,
      adminRejectedOutcome(domain.currentEpoch),
    );
  }

  async resolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    const proposal = domain.activeProposals[input.itemId];
    if (!proposal) {
      throw new Error(`Mock proposal #${input.itemId} is not active`);
    }
    const resolution = resolutionForProposal(
      input.domainId,
      domain,
      proposal,
      true,
    );
    if (resolution.kind === "VotingWindowOpen") {
      throw new Error(
        `Proposal #${input.itemId} is still inside its voting window`,
      );
    }
    applyWinningParticipation(domain, proposal, resolution);
    finalizeProposal(
      domain,
      input.itemId,
      settlementOutcome(domain, proposal, resolution),
      winningPrimaryOptionForResolution(resolution),
    );
  }

  async forceResolveProposalFromVotes(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    const proposal = domain.activeProposals[input.itemId];
    if (!proposal) {
      throw new Error(`Mock proposal #${input.itemId} is not active`);
    }
    const resolution = resolutionForProposal(
      input.domainId,
      domain,
      proposal,
      false,
    );
    if (resolution.kind === "VotingWindowOpen") {
      throw new Error(`Unable to force-resolve proposal #${input.itemId}`);
    }
    applyWinningParticipation(domain, proposal, resolution);
    finalizeProposal(
      domain,
      input.itemId,
      settlementOutcome(domain, proposal, resolution),
      winningPrimaryOptionForResolution(resolution),
    );
  }

  async requeueProposalForAutoFinalization(input: {
    domainId: GovernanceDomainId;
    itemId: GovernanceItemId;
  }): Promise<void> {
    const domain = this.domain(input.domainId);
    const proposal = domain.activeProposals[input.itemId];
    if (!proposal) {
      throw new Error(`Mock proposal #${input.itemId} is not active`);
    }
    domain.currentEpoch = Math.max(domain.currentEpoch, proposal.maturityEpoch);
  }
}
