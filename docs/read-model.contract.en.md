# DEOS Read-Model Contract

> Project-wide rule for separating authoritative on-chain client data from externally indexed materializations

## 1. Purpose

DEOS treats frontend and operator data needs through an explicit two-class read model.
This split is part of the project protocol, not only a UI implementation detail.

Every public datum SHOULD be classified as one of these:

1. `On-Chain Canonical Projection`
   bounded authoritative state or a bounded derived projection stored directly in chain state and intended for raw client or light-client consumption
2. `Indexed / Materialized View`
   externally reconstructed, aggregated, searched, or archived data intended for dashboards, analytics, long-lived history, or operator convenience

The default rule is:

- If a bounded on-chain projection is the real protocol contract, clients SHOULD read it directly from chain state
- If a view is unbounded, historical, search-heavy, or analytics-oriented, it SHOULD stay off-chain instead of being pushed into consensus storage

## 2. Why the split exists

Without this discipline, projects drift into two bad failure modes:

1. `Silent indexer dependency`
   the product pretends to be chain-native, but canonical UX only works if an off-chain indexer is online
2. `Consensus-state dashboard creep`
   unbounded history and heavy analytics are pushed on-chain just to avoid off-chain tooling, increasing proof size, weight, and long-term state growth

DEOS explicitly rejects both defaults.

## 3. Classification Rules

### 3.1 Put data on-chain when all of these are true

- It is part of the live protocol contract or current bounded control surface
- Canonical clients need it without replaying unbounded history
- It can stay bounded in storage, proof size, and servicing cost
- Ambiguity would otherwise force every client to reconstruct the same view ad hoc

### 3.2 Prefer external indexing when any of these are true

- The view is archival or effectively unbounded
- The view is mainly for search, ranking, dashboards, or time-series analysis
- Correctness does not require the runtime to preserve it as canonical state
- Replaying events or batching off-chain aggregation is acceptable

### 3.3 Product honesty rule

If a user-facing screen depends on indexed data rather than canonical on-chain projections, the product contract SHOULD treat that as a derived/materialized view, not as indistinguishable protocol truth.

## 4. Subsystem Audit Matrix

### 4.1 TMC (Curve)

### On-Chain Canonical Projection

- Curve parameters and enablement
- Minted/collateral asset binding
- Live supply-dependent pricing state
- Balances and downstream token-flow effects produced by mint execution

### Indexed / Materialized View

- Mint history timelines
- Issuance analytics
- Cohort or wallet behavior analysis
- Long-horizon curve usage dashboards

### 4.2 Asset Conversion + Axial Router

### On-Chain Canonical Projection

- Current pool reserves
- LP issuance / balances
- Tracked-asset configuration
- Live router fee parameters
- Any bounded runtime-facing routing configuration needed for execution

### Indexed / Materialized View

- Volume charts
- TVL history
- Route-quality dashboards
- Fee-revenue time series
- Historical execution traces per swap path

### 4.3 Asset Registry

### On-Chain Canonical Projection

- Bidirectional `Location <-> AssetId` mapping
- Deterministic foreign-asset identity and metadata
- Governance-controlled registration/migration state
- Well-known protocol asset identity surfaces

### Indexed / Materialized View

- Registration history timelines
- Migration history timelines
- Ecosystem catalog browsing/search beyond the bounded reverse projection
- Operational asset onboarding dashboards

### 4.4 AAA / Automation Kernel

### On-Chain Canonical Projection

- Actor configuration and lifecycle state
- Schedule / trigger state needed for execution
- Bounded readiness, queue, wakeup, and overflow surfaces
- Account ownership/control slots
- Live balances and execution-side effects
- Bounded operational telemetry surfaces already exposed as storage/events when they are part of runtime observability

### Indexed / Materialized View

- Long-lived execution history per actor
- Per-step timeline replay across many cycles
- Fleet dashboards, rankings, and operator analytics
- Archived run logs beyond bounded on-chain observability

### 4.5 Staking

### On-Chain Canonical Projection

- Pool state, shares, receipts, and live ownership surface
- Native binding / operator linkage
- Reward epoch accruals, liability state, and claim state
- Current reward snapshot state needed for settlement
- Any bounded client-facing helper that determines live claimability or position state

### Indexed / Materialized View

- APY charts
- Wallet PnL history
- Claim history timelines
- Leaderboards and comparative analytics
- Archival receipt-transfer narratives

### 4.6 Governance

### On-Chain Canonical Projection

- Active proposal registry
- Bounded active proposal discovery index
- Bounded ballot/tally/status surfaces
- Current resolution state
- Bounded recent finalized proposal discovery and outcomes
- Reward coefficient export
- GovXP input counters
- Any bounded per-proposal or per-account projection needed for canonical live voting UX

### Indexed / Materialized View

- Full referendum archive
- Ballot timelines across expired proposals
- Governance search/filter dashboards
- Long-range participation analytics
- Proposal feeds that outlive bounded retention windows

### 4.7 DEOS Product Layer / Web Client

### On-Chain Canonical Projection

- Current balances, positions, live statuses, and bounded helper surfaces required for canonical flows
- Any runtime-exposed projection whose absence would make the product silently depend on indexers

### Indexed / Materialized View

- Portfolio history
- Cross-subsystem dashboards
- Search
- Notifications/inbox aggregation
- Deep historical explorer views
- DEX/TMC historical chart series beyond the current session

Current web-client note:

- `ChartWidget` may honestly show a bounded recent on-chain historical sampler over finalized blocks as an intermediate product slice before any longer-range retained provider exists
- once chart history is meant to provide deeper retention, search, or archive semantics beyond that bounded recent on-chain window, it MUST be treated explicitly as either a stronger bounded on-chain projection or an indexed/materialized view rather than silently remaining browser-local state
- any future materialized/archive provider in the browser MUST declare its provenance explicitly enough that the user can distinguish it from live chain truth: at minimum the surface must identify `(a)` that it is `materialized`, `(b)` which provider family produced it (`indexer`, `archive-api`, or `analytics-api`), `(c)` whether the scope is `historical`, `archive`, or `search`, and `(d)` the provider/source reference backing that data
- future materialized/archive screens SHOULD keep their fallback behavior honest too: if the provider is unavailable, the client may degrade to no data, a smaller bounded canonical-chain slice, or an explicitly different widget mode, but it MUST NOT silently preserve stale materialized semantics under a live-chain presentation label

## 5. Design Checklist for New Work

For every new subsystem, pallet feature, runtime API, or product-facing query surface:

1. Classify each datum as `On-Chain Canonical Projection` or `Indexed / Materialized View`
2. If canonical, make the on-chain contract explicit and bounded
3. If indexed, keep protocol safety and correctness independent from that indexer
4. Avoid forcing every client to reverse-engineer raw storage topology when one bounded projection is the true contract
5. Avoid introducing permanent on-chain archival state when events plus indexing are enough

## 6. DEOS-Specific Heuristic

When in doubt, ask:

> `Is this datum needed to safely understand or execute the protocol as it exists now, or is it mainly needed to understand everything that happened before?`

If the answer is:

- `needed to safely understand or execute now` -> bias toward a bounded on-chain projection
- `mainly needed to understand history/analytics/search` -> bias toward external indexing

## 7. Consequence for Repository Work

This document means future DEOS work SHOULD not phrase frontend data needs vaguely as just "the UI needs data".
Instead, each such need should be framed as one of:

- `add/keep a bounded on-chain canonical projection`
- `materialize/index this externally`

That vocabulary is now the default project mental model.
