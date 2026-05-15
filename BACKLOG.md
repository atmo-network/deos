# DEOS Backlog

> Canonical open backlog for the framework.
> Pair with `AGENTS.md` for durable protocol/context rules and `CHANGELOG.md` for completed delivery history.

## Open Backlog

> This roadmap tracks only remaining open work.
> Completed delivery history belongs in `CHANGELOG.md`, not here.

> Current baseline assumptions:
>
> - local `pallet-vrf` commit/reveal is retired
> - native `$NTVE`-weighted collator security remains only across the trusted team-operated invulnerable set for the current line
> - same-block randomness is not required
> - until relay randomness exists, deterministic previous-block-hash sampling plus the trusted collator set is accepted
> - the preferred future randomness path is relay-beacon replacement only if upstream ships a parachain-consumable per-block protocol beacon
> - permissionless collators stay deferred until that relay-beacon path exists

> Current shipped baseline includes:
>
> - multi-asset share-vault staking with `stXXX` receipts
> - native `$NTVE -> stNTVE` liquid staking plus locked `NTVE/stNTVE` LP nomination
> - AAA staking automation through portable `Stake`, `Unstake`, and `DonateLiquidity` tasks
> - bounded governance reward-memory and proposal lifecycle
> - domain-scoped `primary + protection` governance for the current launch policy
> - reserve-aware Zap slippage frozen to execution-plan build time for the current launch line
> - same-asset auto-compound reward settlement
> - Polkadot SDK `2603` / node `1.22.2` runtime line

## Open Product / Client Work

### Wallet and portfolio boundary

- [~] `Extend the Wallet widget beyond the bounded tracked-asset contract:` any expansion to a full portfolio UX remains blocked until a materialized/indexed asset-discovery surface exists beyond live chain storage

### Web-client UI architecture simplification

- [ ] `Continue evolving the reserved edge-lane layout model:` define the first concrete left/right lane growth slice if product pressure creates another reserved lane, then extend `RESERVED_LANE_SPECS` without reintroducing user-reorderable edge-lane state
- [ ] `Continue organic customization passes across the web-client:` run only focused highest-pain responsive passes instead of reopening broad layout-polish umbrellas
- [ ] `Continue decomposing oversized web-client slices:` if another frontend store or widget regrows into a hotspot, extract one named sub-slice with explicit ownership
- [ ] `Watch governance state for the next real separation boundary as proposal composition or archive work grows`

## Collator Economics & Fee Routing

> Phase 1 uses trusted, permissioned collators and only pool-level reward flows.
> Phase 2 introduces permissionless collators, LP nomination, and claimable LP-staking nomination rewards.

- [~] `Implement the unified 20/80 collection rule:` transaction fees and AAA fees already use the split; block reward routing remains gated on the explicit block reward source/amount contract
  - [ ] `Block reward routing:` define the reward source/amount contract, then route 20% to collator and 80% to Fee Sink
- [~] `Prepare Phase 2 reward routing without activating it at launch:` keep Phase 2 as a runtime-upgrade boundary, not a launch-time parameter
  - [~] `Claimable LP nomination flow:` activate explicit LP-nomination reward-weight provider only when permissionless collators ship
  - [ ] `LP nomination activation:` expose LP-point nomination to specific collators only when permissionless collator selection is enabled

## Conditional / Externally Gated Work

### Governance execution expansion policy

> Only actionable when a concrete domain-owned control surface, payload family, or failure-state slice is selected beyond the current baseline.

- [ ] `Only after a genuinely delegated/domain-owned parameter surface exists, add the next L2ParameterChange path beyond the Axial Router pair`
- [ ] `Only when a new payload family or failure-state slice ships, broaden per-kind execution observability beyond the current bounded detail/events`
- [ ] `Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next browser composition surface`
- [ ] `Only when a materialized/indexed governance backend is selected, replace the archive-boundary placeholder with live archive search and ballot timelines`

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Only if pool-ratio divergence makes deterministic acquisition insufficient, add swap/mixed-route acquisition:` route selection should include router quote comparison, slippage bounds, and fallback behavior

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Only if a new parachain-consumable per-block protocol beacon exists, define the relay-beacon replacement contract against that actual surface`
- [ ] `Only if that future per-block beacon exists, design the runtime proof-ingestion path:` prefer a weight-accounted `ConsensusHook` snapshot finalized against the real upstream beacon surface
- [ ] `Only if that future per-block beacon exists, wire AAA to it and measure the proof-size / weight impact`
- [ ] `Only after a production-ready per-block relay/protocol beacon exists, design and prototype any permissionless-collator activation path instead of reviving a local threshold line`

### External dependency watch

> The simplified randomness/security roadmap depends on upstream delivery rather than local cryptographic ambition.

- [ ] `Track Safrole/Sassafras release readiness and parachain-consumable randomness availability in paritytech/polkadot-sdk`
- [ ] `Treat the current Polkadot/JAM post-quantum roadmap as a directional beacon-over-VRF signal, not as proof of a shipped parachain-consumable API`
- [ ] `Watch the current upstream signals only in the live polkadot-sdk monorepo:` Sassafras (`#41`, `#1336`, `#7669`) and BLS stabilization (`#10327`, `#11149`)
- [ ] `Only if the relay-beacon path stalls or proves unusable, reassess whether any local threshold runtime work should survive as fallback research`

## Operator-local validation entrypoints

- `./scripts/aaa-release-gate.sh`: AAA scheduler release gate
- `./scripts/try-runtime-local.sh --prepare`: local try-runtime dry-run wrapper
