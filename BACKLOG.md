# DEOS Backlog

> Canonical open backlog for the framework.
> Pair with `AGENTS.md` for durable protocol/context rules and `CHANGELOG.md` for completed delivery history.

## Open Backlog

> This roadmap tracks only remaining open work.
> Completed delivery history belongs in `CHANGELOG.md`, not here.

> Current baseline assumptions:
>
> - Local `pallet-vrf` commit/reveal is retired
> - Native `$NTVE`-weighted collator security remains only across the trusted team-operated invulnerable set for the current line
> - Same-block randomness is not required
> - Until relay randomness exists, deterministic previous-block-hash sampling plus the trusted collator set is accepted
> - The preferred future randomness path is relay-beacon replacement only if upstream ships a parachain-consumable per-block protocol beacon
> - Permissionless collators stay deferred until that relay-beacon path exists

> Current shipped baseline includes:
>
> - Multi-asset share-vault staking with `stXXX` receipts
> - Native `$NTVE -> stNTVE` liquid staking plus locked `NTVE/stNTVE` LP nomination
> - AAA staking automation through portable `Stake`, `Unstake`, and `DonateLiquidity` tasks
> - Bounded governance reward-memory and proposal lifecycle
> - Domain-scoped `primary + protection` governance for the current launch policy
> - Reserve-aware liquidity-actor slippage frozen to execution-plan build time for the current launch line
> - Same-asset auto-compound reward settlement
> - Unified 20/80 transaction-fee and AAA-fee routing to collator / Fee Sink when an author exists, with 100% to Fee Sink when no author is resolved
> - Economic-claim inventory covers 10 anchored runtime claims with proof-kind, tautology-risk, and falsification-note metadata
> - No active local runtime/doc anti-rot slices remain; open items are conditional, externally gated, watch-only, or product-pressure-triggered
> - Polkadot SDK `2603` / node `1.22.3` runtime line

## Open Product / Client Work

### Wallet and portfolio boundary

- [~] `Extend the Wallet widget beyond the bounded tracked-asset contract`: any expansion to a full portfolio UX remains blocked until a materialized/indexed asset-discovery surface exists beyond live chain storage

### Web-client product stabilization

- [ ] `Reserved edge-lane growth slice`: only if product pressure creates another reserved left/right lane, define the concrete lane role and extend `RESERVED_LANE_SPECS` without reintroducing user-reorderable edge-lane state.
- [ ] `Governance state separation slice`: only if proposal composition or archive work grows enough to create a named ownership conflict, split the state boundary at that concrete seam.
- [ ] `Materialized provider boundary slice`: only when a second indexed/archive provider family exists, decide whether `adapters/materialized-history/` should become a first-class `materialized/` or `providers/` slice.

## Collator Economics & Fee Routing

> Phase 1 uses trusted, permissioned collators and only pool-level reward flows.
> Phase 2 introduces permissionless collators, LP nomination, and claimable LP-staking nomination rewards.

- [~] `Implement the unified 20/80 collection rule`: transaction fees and AAA fees already use the split; block reward routing remains gated until an explicit block reward issuance/source policy exists
- [~] `Prepare Phase 2 reward routing without activating it at launch`: keep Phase 2 as a runtime-upgrade boundary, not a launch-time parameter
  - [~] `Claimable LP nomination flow`: activate explicit LP-nomination reward-weight provider only when permissionless collators ship
  - [ ] `LP nomination activation`: expose LP-point nomination to specific collators only when permissionless collator selection is enabled

## Conditional / Externally Gated Work

### Governance execution expansion policy

> Only actionable when a concrete domain-owned control surface, payload family, or failure-state slice is selected beyond the current baseline.

- [ ] `Only after a genuinely delegated/domain-owned parameter surface exists, add the next L2ParameterChange path beyond the Axial Router pair`
- [ ] `Only when a new payload family or failure-state slice ships, broaden per-kind execution observability beyond the current bounded detail/events`
- [ ] `Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next browser composition surface`
- [ ] `Only when a materialized/indexed governance backend is selected, connect the reserved archive boundary to live archive search and ballot timelines`

### Block reward source policy

> Only actionable when the launch economy selects a concrete block subsidy / issuance source instead of assuming one exists.

- [ ] `Only after a concrete block reward source/amount policy exists, route block rewards through the unified 20/80 collection rule`: route 20% to author/collator and 80% to Fee Sink, with explicit tests for unresolved authors and no hidden inflation source.

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Only if pool-ratio divergence makes deterministic acquisition insufficient, add swap/mixed-route acquisition`: route selection should include router quote comparison, slippage bounds, and fallback behavior

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Only if a new parachain-consumable per-block protocol beacon exists, define the relay-beacon replacement contract against that actual surface`
- [ ] `Only if that future per-block beacon exists, design the runtime proof-ingestion path`: prefer a weight-accounted `ConsensusHook` snapshot finalized against the real upstream beacon surface
- [ ] `Only if that future per-block beacon exists, wire AAA to it and measure the proof-size / weight impact`
- [ ] `Only after a production-ready per-block relay/protocol beacon exists, design and prototype any permissionless-collator activation path instead of reviving a local threshold line`

### External dependency watch

> The simplified randomness/security roadmap and dependency posture depend on upstream delivery rather than local surgery.

- [ ] `Low-severity npm audit follow-up`: monitor the current npm audit report for SvelteKit/cookie and bits-ui/runed advisories; do not apply suggested semver-major/downgrade fixes unless upstream publishes a compatible non-regressive path.
- [ ] `Formatter peer compatibility watch`: no active formatter problem exists; keep `prettier-plugin-svelte` on the current 3.x line while `@trivago/prettier-plugin-sort-imports 6.0.2` declares optional peer compatibility with `prettier-plugin-svelte 3.x`, and revisit only when the sort-imports peer range supports 4.x or the formatter stack is intentionally changed.
- [ ] `Template cargo update blocker`: `cargo update --manifest-path template/Cargo.toml --dry-run` currently fails because SDK `polkadot-sdk v2603.0.0 -> sc-service -> sc-network -> litep2p v0.13.3 -> multihash v0.17.0` requires yanked `core2 v0.4.0`; revisit only through an upstream-compatible Polkadot SDK/litep2p path rather than local dependency surgery.
- [ ] `Track Safrole/Sassafras release readiness and parachain-consumable randomness availability in paritytech/polkadot-sdk`
- [ ] `Treat the current Polkadot/JAM post-quantum roadmap as a directional beacon-over-VRF signal, not as proof of a shipped parachain-consumable API`
- [ ] `Watch the current upstream signals only in the live polkadot-sdk monorepo`: Sassafras (`#41`, `#1336`, `#7669`) and BLS stabilization (`#10327`, `#11149`)
- [ ] `Only if the relay-beacon path stalls or proves unusable, reassess whether any local threshold runtime work should survive as fallback research`
