import type {
  GovernanceDomainId,
  GovernanceItemId,
  GovernanceVoteKind,
} from "./types";

export type GovernanceMaterializedArchiveEntry = {
  itemId: GovernanceItemId;
  title: string;
  summary: string;
  finalizedAtEpoch: number;
  outcomeLabel: string;
};

export type GovernanceMaterializedBallotTimelineEntry = {
  atEpoch: number;
  accountId: string;
  voteKind: GovernanceVoteKind;
  track: "ordinary" | "veto";
  weight: bigint;
  note: string;
};

export type GovernanceMaterializedProvider = {
  label(): string;
  message(): string | null;
  searchProposals(
    domainId: GovernanceDomainId,
    query: string,
  ): Promise<GovernanceMaterializedArchiveEntry[]>;
  ballotTimeline(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceMaterializedBallotTimelineEntry[]>;
};

const ARCHIVE_BY_DOMAIN: Record<
  GovernanceDomainId,
  GovernanceMaterializedArchiveEntry[]
> = {
  1000: [
    {
      itemId: 311,
      title: "Treasury buffer risk response",
      summary:
        "Protected referendum with live veto pressure and split ordinary turnout",
      finalizedAtEpoch: 42,
      outcomeLabel: "Active / veto pressure",
    },
    {
      itemId: 309,
      title: "Router fee retune",
      summary:
        "Proposal failed approval threshold after bounded voting window closed",
      finalizedAtEpoch: 41,
      outcomeLabel: "Rejected",
    },
    {
      itemId: 308,
      title: "Reward memory parameter tune",
      summary: "Resolved with manual winner set before expiry horizon",
      finalizedAtEpoch: 40,
      outcomeLabel: "Resolved",
    },
    {
      itemId: 307,
      title: "Emergency protected veto test",
      summary:
        "Finalized through veto cancellation path after protected-track intervention",
      finalizedAtEpoch: 39,
      outcomeLabel: "Veto cancelled",
    },
  ],
};

const BALLOT_TIMELINES: Record<
  GovernanceDomainId,
  Record<GovernanceItemId, GovernanceMaterializedBallotTimelineEntry[]>
> = {
  1000: {
    310: [
      {
        atEpoch: 42,
        accountId: "alice",
        voteKind: "aye",
        track: "ordinary",
        weight: 2100n,
        note: "Early ordinary vote captured under stronger declining-power multiplier",
      },
      {
        atEpoch: 43,
        accountId: "bob",
        voteKind: "pass",
        track: "veto",
        weight: 700n,
        note: "Protected-track participant explicitly allowed ordinary track to decide",
      },
      {
        atEpoch: 44,
        accountId: "charlie",
        voteKind: "veto",
        track: "veto",
        weight: 800n,
        note: "Later veto-track intervention repriced at weaker ballot-time weight",
      },
    ],
    311: [
      {
        atEpoch: 38,
        accountId: "alice",
        voteKind: "aye",
        track: "ordinary",
        weight: 2100n,
        note: "Ordinary approval started strong",
      },
      {
        atEpoch: 39,
        accountId: "dave",
        voteKind: "veto",
        track: "veto",
        weight: 650n,
        note: "Veto-track pressure entered before maturity",
      },
      {
        atEpoch: 40,
        accountId: "bob",
        voteKind: "pass",
        track: "veto",
        weight: 700n,
        note: "Protected-track pass attempted to unblock ordinary result",
      },
    ],
    309: [
      {
        atEpoch: 40,
        accountId: "alice",
        voteKind: "aye",
        track: "ordinary",
        weight: 1600n,
        note: "Later ordinary ballot after most of the voting window elapsed",
      },
      {
        atEpoch: 41,
        accountId: "bob",
        voteKind: "nay",
        track: "ordinary",
        weight: 1200n,
        note: "Counterweight prevented approval threshold from clearing",
      },
    ],
  },
};

export class GovernanceMockMaterializedProvider implements GovernanceMaterializedProvider {
  label(): string {
    return "Mock materialized governance view";
  }

  message(): string | null {
    return "Mock archive/timeline data for contract-preview UX";
  }

  async searchProposals(
    domainId: GovernanceDomainId,
    query: string,
  ): Promise<GovernanceMaterializedArchiveEntry[]> {
    const normalized = query.trim().toLowerCase();
    const entries = ARCHIVE_BY_DOMAIN[domainId] ?? [];
    if (normalized.length === 0) {
      return [...entries];
    }
    return entries.filter(
      (entry) =>
        entry.title.toLowerCase().includes(normalized) ||
        entry.summary.toLowerCase().includes(normalized) ||
        String(entry.itemId).includes(normalized),
    );
  }

  async ballotTimeline(
    domainId: GovernanceDomainId,
    itemId: GovernanceItemId,
  ): Promise<GovernanceMaterializedBallotTimelineEntry[]> {
    return [...(BALLOT_TIMELINES[domainId]?.[itemId] ?? [])];
  }
}

export class GovernanceUnavailableMaterializedProvider implements GovernanceMaterializedProvider {
  constructor(
    private readonly unavailableReason = "No indexed governance backend configured for archive search or ballot timelines",
  ) {}

  label(): string {
    return "Materialized governance backend unavailable";
  }

  message(): string | null {
    return this.unavailableReason;
  }

  async searchProposals(): Promise<GovernanceMaterializedArchiveEntry[]> {
    return [];
  }

  async ballotTimeline(): Promise<GovernanceMaterializedBallotTimelineEntry[]> {
    return [];
  }
}
