export type ReadModelContractClass = "canonical-chain" | "materialized";

export type ReadModelRealization =
  | "direct"
  | "session-cache"
  | "session-derived"
  | "provider";

export type ChainDirectSourceKind = "storage" | "runtime-view";
export type ChainDerivedSourceKind =
  | "finalized-events"
  | "bounded-block-sampler"
  | "client-bounded-projection";
export type MaterializedSourceKind =
  | "indexer"
  | "archive-api"
  | "analytics-api";

export type ReadModelScope =
  | "live"
  | "snapshot"
  | "bounded-recent"
  | "session"
  | "historical"
  | "archive"
  | "search";

export type CanonicalDirectProvenance = {
  contractClass: "canonical-chain";
  realization: "direct" | "session-cache";
  sourceKind: ChainDirectSourceKind;
  scope: "live" | "snapshot" | "bounded-recent";
  bounded: true;
  sourceRef: string;
};

export type CanonicalDerivedProvenance = {
  contractClass: "canonical-chain";
  realization: "session-derived";
  sourceKind: ChainDerivedSourceKind;
  scope: "live" | "snapshot" | "bounded-recent" | "session";
  bounded: true;
  sourceRef: string;
};

export type MaterializedProvenance = {
  contractClass: "materialized";
  realization: "provider" | "session-cache";
  sourceKind: MaterializedSourceKind;
  scope: "historical" | "archive" | "search";
  bounded: false;
  sourceRef: string;
};

export type ReadModelProvenance =
  | CanonicalDirectProvenance
  | CanonicalDerivedProvenance
  | MaterializedProvenance;

export type ReadModelValue<T> = {
  value: T;
  provenance: Readonly<ReadModelProvenance>;
  fetchedAt: number;
  asOfBlock?: number;
  asOfHash?: string;
};

export type ReadModelStamp = {
  fetchedAt?: number;
  asOfBlock?: number;
  asOfHash?: string;
};

export type SurfaceState<T> =
  | {
      status: "idle" | "loading";
      data: null;
      error: null;
    }
  | {
      status: "ready";
      data: ReadModelValue<T>;
      error: null;
    }
  | {
      status: "error";
      data: null;
      error: string;
    };

function stamp<T>(
  value: T,
  provenance: ReadModelProvenance,
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return {
    value,
    provenance,
    fetchedAt: stampInfo?.fetchedAt ?? Date.now(),
    asOfBlock: stampInfo?.asOfBlock,
    asOfHash: stampInfo?.asOfHash,
  };
}

export function fromChainStorage<T>(
  value: T,
  sourceRef: string,
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return stamp(
    value,
    {
      contractClass: "canonical-chain",
      realization: "direct",
      sourceKind: "storage",
      scope: "live",
      bounded: true,
      sourceRef,
    },
    stampInfo,
  );
}

export function fromRuntimeView<T>(
  value: T,
  sourceRef: string,
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return stamp(
    value,
    {
      contractClass: "canonical-chain",
      realization: "direct",
      sourceKind: "runtime-view",
      scope: "live",
      bounded: true,
      sourceRef,
    },
    stampInfo,
  );
}

export function fromSessionDerivedChain<T>(
  value: T,
  sourceKind: ChainDerivedSourceKind,
  sourceRef: string,
  scope: CanonicalDerivedProvenance["scope"] = "session",
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return stamp(
    value,
    {
      contractClass: "canonical-chain",
      realization: "session-derived",
      sourceKind,
      scope,
      bounded: true,
      sourceRef,
    },
    stampInfo,
  );
}

export function fromClientBoundedProjection<T>(
  value: T,
  sourceRef: string,
  scope: Extract<CanonicalDerivedProvenance["scope"], "live" | "snapshot" | "bounded-recent" | "session"> = "live",
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return fromSessionDerivedChain(
    value,
    "client-bounded-projection",
    sourceRef,
    scope,
    stampInfo,
  );
}

export function fromMaterialized<T>(
  value: T,
  sourceKind: MaterializedSourceKind,
  sourceRef: string,
  scope: MaterializedProvenance["scope"] = "historical",
  stampInfo?: ReadModelStamp,
): ReadModelValue<T> {
  return stamp(
    value,
    {
      contractClass: "materialized",
      realization: "provider",
      sourceKind,
      scope,
      bounded: false,
      sourceRef,
    },
    stampInfo,
  );
}

export function isCanonicalChain(
  value: ReadModelValue<unknown> | ReadModelProvenance,
): boolean {
  const provenance = "provenance" in value ? value.provenance : value;
  return provenance.contractClass === "canonical-chain";
}

export function isMaterialized(
  value: ReadModelValue<unknown> | ReadModelProvenance,
): boolean {
  const provenance = "provenance" in value ? value.provenance : value;
  return provenance.contractClass === "materialized";
}

export function isSessionDerivedCanonical(
  value: ReadModelValue<unknown> | ReadModelProvenance,
): boolean {
  const provenance = "provenance" in value ? value.provenance : value;
  return (
    provenance.contractClass === "canonical-chain" &&
    provenance.realization === "session-derived"
  );
}

export function getReadModelLabel(provenance: ReadModelProvenance): string {
  if (provenance.contractClass === "materialized") {
    if (provenance.scope === "archive") {
      return "Archive view";
    }
    if (provenance.scope === "search") {
      return "Indexed search";
    }
    return "Materialized view";
  }
  if (provenance.realization === "session-derived") {
    switch (provenance.sourceKind) {
      case "finalized-events":
        return "Live session feed";
      case "bounded-block-sampler":
        return "Recent chain sample";
      case "client-bounded-projection":
        return "Client-bounded projection";
    }
  }
  return provenance.realization === "session-cache"
    ? "Cached live chain"
    : "Live chain";
}

export function getReadModelDescription(
  provenance: ReadModelProvenance,
): string {
  if (provenance.contractClass === "materialized") {
    return `Derived by ${provenance.sourceKind} from ${provenance.sourceRef}`;
  }
  if (provenance.realization === "session-derived") {
    return `Built in this browser session from ${provenance.sourceKind} via ${provenance.sourceRef}`;
  }
  return `Read from ${provenance.sourceKind} via ${provenance.sourceRef}`;
}
