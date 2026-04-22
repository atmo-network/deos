# DEOS Backlog

> Canonical open backlog for the framework.
> Pair with `AGENTS.md` for durable protocol/context rules and `CHANGELOG.md` for completed delivery history.

## Open Backlog

> This roadmap now tracks only remaining open work.
> Fully delivered sections were rotated out of this file so the backlog stays focused on unresolved implementation, rollout, policy, and dependency items.

> Current baseline assumptions:
>
> - the local `pallet-vrf` commit/reveal line is retired
> - native `$NTVE`-weighted collator security remains, but only across an explicitly trusted, team-operated invulnerable collator set for the current line
> - same-block randomness is not required
> - until relay randomness exists, the accepted fallback is deterministic previous-block-hash sampling plus that trusted team-operated collator set
> - the preferred future randomness path is **relay-beacon replacement** only if Safrole/Sassafras or another upstream line actually delivers a new parachain-consumable **per-block** protocol beacon
> - permissionless collators stay deferred until that kind of relay-beacon path exists
>
> Current shipped baseline already includes:
>
> - multi-asset share-vault staking with `stXXX` receipts
> - native `$NTVE -> stNTVE` binding inside one `pallet-staking`
> - AAA staking automation for both generic staking and explicit operator-aware native staking through `StakeNative { amount, operator }`
> - bounded governance reward-memory + proposal lifecycle in `pallet-governance`, including exported GovXP participation/authorship counters
> - current launch governance policy frozen to a domain-scoped `primary + protection` hierarchy (`Native + $VETO` for protocol governance, `$BLDR + Native` for the canonical tactical domain), narrow admin recovery, bounded recent finalized-outcome retention, and the public ordinary cadence/enactment schedule now shipped on the current line
> - reserve-aware Zap slippage intentionally frozen to execution-plan build time for the current launch line
> - same-asset auto-compound reward settlement
> - mirror implementation-architecture docs for staking and governance
> - Polkadot SDK `2603` / node `1.22.0` runtime line with `system_version >= 3` and staged `:pending_code -> :code` runtime-upgrade behavior

## Open Product / Client Work

### 0. Governance v1 contract rollout (spec target, not yet shipped)

- [x] `Implement the ordinary public referendum cadence in pallet-governance:` the reference runtime now uses the public ordinary cadence contract directly: protection opens at submission, ordinary classes wait through a `3 day` lead-in before primary voting, ordinary protection stays open for `7 days`, ordinary primary runs for `7 days`, the canonical public Declining Power curve decays `7x -> 1x` by the end of day 6 with day 7 flat at `1x`, and successful ordinary referenda now enter a default `3 day` enactment delay with bounded query visibility.
  - [x] `Add proposal timing scaffolding:` runtime config plus bounded storage/query state for lead-in, protection-window close, primary open/close, urgent-open override, and enactment scheduling are now present through additive config hooks, `ProposalUrgentAuthorizedAt` / `ProposalPendingEnactmentAt` storage, and the new canonical `proposal_timing(domain, item_id)` runtime view.
  - [x] `Retarget ordinary lifecycle/resolution to the new timing model:` the generic plumbing for enactment scheduling, lead-in-gated primary admission, and protection-window admission is now matched by runtime policy too, so the reference line now runs the target public cadence rather than the older tiny launch-line timings.
    - [x] `Implement enactment-delay finalization/status handling:` successful proposals now schedule bounded pending enactment, expose that state through `proposal_status`, and clear the additive timing markers when finalized-outcome retention expires.
    - [x] `Implement lead-in-gated primary voting:` primary ballots now stay closed until the configured lead-in ends, while protection-track ballots remain admissible during that earlier window.
    - [x] `Open protection from submission and remove the old first-\`2/3\` activation rule:` protection votes are now simply admissible until the configured protection close instead of relying on late first-touch activation semantics.
  - [x] `Retarget ordinary vote weighting to the target public Declining Power curve:` ordinary tracks now use `7x -> 1x` by end of day 6 with day 7 flat at `1x`, while urgent primary handling stays flat `1x`.
    - [x] `Split ordinary vs protection weighting windows:` the pallet now feeds ordinary and protection ballots through separate timing windows so future lead-in / protection-window divergence can be enabled honestly.
    - [x] `Retarget the shipped declining-power formula to the canonical 7x curve:` the runtime now uses the agreed `7x -> 1x` public temporal weighting shape instead of the older `10x -> 1x` line.
- [ ] `Implement the minimal domain + cadence + payload-kind surface needed for governance v1 rollout:` the contract now chooses `GovernanceDomain + CadenceMode + ProposalPayloadKind`, so the remaining work is landing the smallest explicit pallet/runtime hook that can carry ordinary and fast cadence plus the minimal payload kinds (`L1RootAction`, `L2TreasurySpend`, `L2ParameterChange`, `Intent`, `L2SignalToL1`).
  - [x] `Add proposal metadata scaffolding:` active proposals now persist additive `CadenceMode + PayloadKind + payload hash` metadata through `ProposalMetadataByItem`, the pallet exposes proposal metadata, derived execution authority, and payload-preimage availability query surfaces, the runtime now includes canonical `pallet_preimage` support, and submission now requires explicit cadence mode, payload kind, and payload hash at the pallet boundary.
  - [ ] `Bind domain + cadence + payload kind to execution authority:` each proposal item needs one domain, one cadence mode, one executable payload identity, and one declared execution origin/authority.
    - [x] `Expose additive execution-authority scaffolding:` proposals now expose a derived `proposal_execution_authority(domain, item_id)` query surface based on payload kind.
    - [x] `Define preimage admission policy per payload kind:` the governance contract now fixes the v1 rule that executable payload kinds may be submitted before their full preimage is noted as long as readiness stays queryable through the canonical preimage surface, while advisory payload kinds may remain hash-only and actual enactment still requires runtime-visible payload availability.
    - [ ] `Bind payload hashes to deterministic dispatch semantics:` land one canonical bounded executor that maps `(domain, payload_kind, payload_hash)` into runtime behavior instead of stopping at metadata/readiness scaffolding.
      - [x] `Add the canonical payload-executor scaffold:` the governance pallet now has one bounded enactment scaffold that consumes proposal metadata plus payload readiness and can decide `dispatch`, `non-executable advisory finalization`, or explicit execution failure, even though the reference runtime still wires a no-op executor for executable payload kinds.
      - [x] `Add bounded enactment servicing for due pending proposals:` due pending items now flow through an epoch-keyed bounded enactment service path instead of remaining pure status scaffolding until retention expiry.
      - [x] `Wire executable payload kinds into real enactment behavior:` each executable payload kind now has at least one bounded runtime path on the reference line: `L1RootAction -> RuntimeCall::System(authorize_upgrade { code_hash })` in the strategic domain, `L2ParameterChange -> RuntimeCall::AxialRouter(update_router_fee { new_fee })` through a governance-owned internal setter, and `L2TreasurySpend -> RuntimeCall::Assets(transfer { id = $BLDR, ... })` executed as the designated BLDR Treasury sovereign with non-Root transactional transfer semantics.
      - [x] `Wire advisory payload kinds into explicit non-dispatch finalization:` `Intent` and `L2SignalToL1` now finalize as honest non-dispatch advisory outcomes instead of pretending to be missing execution work.
      - [ ] `Broaden executable payload call matrices beyond the first bounded slices:` `L2ParameterChange` now has two truthful runtime paths (`AxialRouter.add_tracked_asset` and `AxialRouter.update_router_fee`), while `L2TreasurySpend` now supports bounded asset transfers from the designated BLDR treasury sovereign for any asset class held there. The remaining work is no longer one generic "make the matrix bigger" umbrella:
        - [x] `Publish the current rejected-surface inventory for L2 parameter delegation:` the current line now records that TMC launch-physics mutation, staking onboarding/recovery/admin reward bootstrap, AAA global-control surfaces, and asset-registry registration/migration remain L1/system-owned rather than truthful domain-owned `L2ParameterChange` targets.
        - [ ] `Add the first explicitly delegated/domain-owned L2 parameter surface beyond the Axial Router pair:` the next slice is to introduce a genuinely delegated/domain-owned parameter surface rather than widening legacy Root setters that still belong to launch physics, system control, staking administration, or asset registration.
        - [ ] `Define the next truthful L2 treasury authority topology before widening payout semantics further:` make funding source, asset scope, and domain-local treasury authority explicit before adding native payout or richer treasury-spend families.
      - [ ] `Broaden execution-side observability beyond the first bounded slices:` generic success/failure/advisory-finalization events now exist and each now names the bounded payload kind directly, execution failures now also expose bounded failure-reason categories (including invoice-side `MissingWinningPrimaryOption`), the runtime now also retains bounded `proposal_execution_detail(domain, item_id)` query projections for recent enacted/failed/advisory items, and treasury-spend execution details now carry scalar-aware `base_amount + scalar + final_amount` invoice truth. The remaining observability gap is richer per-kind reporting beyond the current bounded slices rather than missing invoice-scalar execution detail itself.
    - [x] `Publish one canonical payload-kind matrix across docs/runtime/events:` the docs, runtime enum, query surfaces, tests, and submission/vote events now consistently use only `L1RootAction`, `L2TreasurySpend`, `L2ParameterChange`, `Intent`, and `L2SignalToL1` as the active payload-kind vocabulary.
  - [x] `Define advisory payload semantics:` the governance specification now fixes bounded `Intent` / `L2SignalToL1` payload shape (`summary`, optional `doc_cid`, optional referenced payload hash), explicit non-executable handling, and the default no winner-memory / GovXP credit contract.
  - [x] `Define invoice execution payload semantics:` the governance specification now fixes beneficiary, payout asset, base amount, funding source, discrete scalar application, caps, and transactional failure handling for `L2TreasurySpend` invoice payloads.
  - [x] `Define and implement the minimum governance event contract:` submission, vote/replacement, fast-track authorization, finalized outcomes, enactment scheduling, execution success/failure, invoice execution, and runtime-upgrade execution now all have explicit bounded event coverage on the current launch line.
    - [x] `Enrich proposal submission events with payload semantics:` `ProposalSubmitted` now carries cadence mode, payload kind, and payload hash so indexers no longer need to infer proposal meaning from storage alone.
    - [x] `Enrich vote-cast events with replacement/time semantics:` `ProposalVoteCast` now carries `vote_epoch` plus the replaced protection-side vote, so `Pass -> Veto` / `Veto -> Pass` rewrites are externally visible.
    - [x] `Emit first payload-execution observability events:` the runtime now emits generic execution success/failure/advisory-finalization plus typed `ProposalRuntimeUpgradeAuthorized`, `ProposalParameterChangeExecuted`, and `ProposalTreasurySpendExecuted` events for the first bounded executable slices.
    - [x] `Emit urgent-authorization observability for the fast-track path:` the pallet now emits `ProposalUrgentAuthorized` when an expeditable proposal crosses the raw protection-track `Pass` threshold and records its authorization epoch.
- [x] `Implement first-class tactical invoice voting:` canonical `$BLDR` governance now ships `L2TreasurySpend` payloads with live `Amplify / Approve / Reduce / Nay` primary evaluation, `Veto / Pass` protection, discrete payout-scalar resolution, scalar-aware treasury execution receipts, and browser-visible invoice-family voting/execution truth on the current launch line.
  - [x] `Split primary-track vote shape by proposal family:` the kernel no longer hardcodes primary voting as storage-level `Aye / Nay` only: `ProposalVoteKind` / `ProposalVotesByItem` can now represent invoice-family `Amplify / Approve / Reduce / Nay` alongside protection `Veto / Pass`, binary families reject invoice-only vote kinds, invoice families reject binary-only `Aye`, and the current launch line still truthfully reports `Binary` for all runtime combinations until a later policy/resolution rollout opts invoice behavior in.
  - [x] `Retarget tally and query surfaces for invoice-family primary options:` `proposal_vote_tally(domain, item_id)` now carries the widened invoice-family primary weights, and the new canonical `proposal_primary_track_tally(domain, item_id)` view now exposes a family-aware primary-lane summary with deterministic lowest-scalar tie-breaking for the leading positive invoice option so clients/executors no longer have to reconstruct that identity off-chain.
  - [x] `Activate the discrete invoice resolution rule on the launch line:` the reference runtime now marks tactical `L2TreasurySpend` proposals in the canonical `$BLDR` domain as `Invoice`, so the spec's invoice rule is no longer kernel-only: live proposals now use `Amplify / Approve / Reduce / Nay` primary semantics, explicit `PassingAmplify / PassingApprove / PassingReduce` states, and the existing `Veto / Pass` protection lane.
  - [x] `Retarget treasury execution receipts/events/detail from transfer-only to scalar-aware invoice execution:` tactical treasury enactment now decodes a dedicated bounded invoice payload (`beneficiary`, `payout_asset`, `base_amount`, explicit funding source), applies the winning scalar on-chain, executes the final transfer from the designated BLDR treasury sovereign, and carries `base_amount + scalar + final_amount + funding_source` through bounded treasury execution events/detail as `InvoiceScalarTransfer` truth.
- [x] `Retarget GovXP and public submission to the v1 contract:` counters-first GovXP, signed advisory submission, opening-fee/preimage quoting, the next public payload-kind opt-in, and the first signed tactical treasury browser composition slice are now all landed on the current line.
- [ ] `Add governance product UX for proposal semantics and execution state:` active/retained proposal semantics, invoice/runtime-upgrade execution detail, and bounded advisory submission review are broadly landed. Remaining work:
  - [ ] `Extend active/retained execution-state UX only when a new payload family or failure-state slice actually ships`
  - [ ] `Design the next richer browser composition surface beyond the current advisory + minimal tactical treasury forms`
  - [ ] `Define a separate materialized/archive governance UX instead of stretching bounded retention cards`

### 1. Web-client wallet and execution UX hardening

- [x] `Finish transaction-grade wallet UX in web-client:` account/signer ownership, bounded asset projection, send-surface provenance, native safe-max honesty, tracked-asset transfer coverage, and draft-keyed in-flight send behavior across watch-only/signer transitions are now landed on the current line.
- [ ] `Extend the Wallet widget beyond the bounded tracked-asset contract:` the wallet now discovers the bounded tracked-asset set with live balances for the selected account and the send surface can already transfer Native plus those bounded tracked assets, but any future expansion beyond that bounded runtime-facing contract still needs a wider authoritative asset-presentation / discovery surface before the wallet should pretend to expose a full portfolio UX.
- [x] `Harden the Swap widget execution path:` signer guidance, safe-max, minimum-buy, quote gating, submit self-validation, slippage-aligned quote eligibility, and buy/sell execution log wording are now all aligned on the current line.
- [x] `Keep web-client bundle growth honest after local dev signing landed:` the first lazy-loading, bootstrap deferral, and bundle-report slices are already in place. The remaining shared `deos` / metadata bootstrap cost is now treated as consciously accepted startup weight under the current on-chain-first/eager product contract rather than as hidden optimization debt.
  - [x] `Trace the current bootstrap importer graph and choose the next concrete boundary to cut:` the current bundle report now points to governance advisory payload hashing as the next honest cut, because `GovernanceWidget` still statically pulls the heavy `@polkadot/util-crypto` path through advisory-payload derivation.
  - [x] `Move governance advisory payload hashing behind an existing action-path lazy edge:` the governance advisory review/submit path now computes the payload hash through an on-demand dynamic `@polkadot/util-crypto` import instead of pinning that crypto chunk on the base governance viewer path.
  - [x] `Choose the next bootstrap fan-out boundary after the advisory hashing cut:` the next honest choice is `(a)` for the current line: accept that startup still pays for bounded live chain bootstrap while the canonical default workspace remains on-chain-first/eager, and defer any further typed-API/metadata lazy boundary until that product contract changes.

### 2. Web-client UI architecture simplification

- [ ] `Continue evolving the reserved edge-lane layout model:` the first header/footer/sidebar lane-spec line is already landed. Remaining work:
  - [ ] `Define the first concrete left/right lane growth slice if product pressure creates another reserved lane`
  - [ ] `Extend RESERVED_LANE_SPECS to that next lane without reintroducing user-reorderable edge-lane state`
- [ ] `Continue organic customization passes across the web-client:` the shared pane chrome and the current highest-traffic widgets already adapt much better across widths. Remaining work:
  - [x] `Identify the next highest-pain widget under extreme width/height constraints and split it into a concrete adaptation slice:` the next clear slices were `WikiWidget` after the metadata expansion, the reserved sidebar lane widgets, the dense `StatisticsWidget` route-mix surface, and the narrow `AutomationWidget` actor-card header.
  - [x] `Run the next focused extreme-pane audit instead of reopening one umbrella layout-polish pass:` the current passes narrowed that audit first to `WikiWidget` reader mode, then to the reserved sidebar lane widgets, then to the dense `StatisticsWidget` route-mix block, and then to the narrow `AutomationWidget` actor cards.
  - [x] `Keep WikiWidget content-first in narrow reader panes after the metadata expansion:` in non-dual reader mode the widget now moves secondary discovery/provenance cards behind the main markdown body so content stays first when space is tight.
  - [x] `Make AccountWidget adapt more honestly in narrow sidebar widths:` the sidebar account surface now shortens the selected address with full-detail fallback in dense lanes, uses earlier two-column preset/injected-account grids once width supports them, and keeps the custom-account action row tighter without wasting vertical space.
  - [x] `Make SettingsWidget adapt more honestly in narrow sidebar widths:` the sidebar settings surface now keeps the endpoint field full-width, reuses a two-column row for the smaller controls once the lane is wide enough, and lets the apply action stop wasting vertical space in wider sidebars while still collapsing back to full width in tight lanes.
  - [x] `Let StatisticsWidget route-mix cards stack in the densest panes:` the route-mix detail cards now drop back to one column before those labels/details become cramped mini-columns.
  - [x] `Let AutomationWidget actor-card headers stack earlier in narrow panes:` dense panes now stop forcing the actor label/role and status badge to compete in one horizontal header row, and the header actor-count badge also shortens in compact panes.
- [ ] `Continue decomposing oversized web-client slices:` log, market, portfolio, system, and the current governance viewer boundaries are already much cleaner than before. Remaining work:
  - [ ] `Watch governance state for the next real separation boundary as proposal composition or archive work grows`
  - [ ] `If another frontend store regrows into a hotspot, extract one named sub-slice with explicit ownership instead of reopening a generic refactor umbrella`
- [ ] `Prepare web-client provenance for future materialized/archive providers:` the shared provenance vocabulary and the current active bounded query surfaces now all expose explicit provenance where that browser-side realization would otherwise be visually ambiguous. Remaining work:
  - [x] `Extend provenance labels to the next active bounded query surface that is still visually ambiguous`
  - [x] `Define the provenance contract for any future materialized/archive provider before shipping it`

## Conditional / Externally Gated Work

### 5. Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Only if a new parachain-consumable per-block protocol beacon exists, define the relay-beacon replacement contract against that actual surface.`
- [ ] `Only if that future per-block beacon exists, design the runtime proof-ingestion path:` a weight-accounted `ConsensusHook` snapshot remains the leading pattern, but it should be finalized against the real upstream beacon surface rather than today's epoch-scale items.
- [ ] `Only if that future per-block beacon exists, wire AAA to it and measure the proof-size / weight impact.`
- [ ] `Only after a production-ready per-block relay/protocol beacon exists, design and prototype any permissionless-collator activation path instead of reviving a local threshold line.`

### 6. External dependency watch

> The simplified randomness/security roadmap depends on upstream delivery rather than local cryptographic ambition.

- [ ] `Track Safrole/Sassafras release readiness and parachain-consumable randomness availability in paritytech/polkadot-sdk.`
- [ ] `Treat the current Polkadot/JAM post-quantum roadmap as a directional beacon-over-VRF signal, not as proof of a shipped parachain-consumable API.`
- [ ] `Watch the current upstream signals only in the live polkadot-sdk monorepo:` Sassafras (`#41`, `#1336`, `#7669`) and BLS stabilization (`#10327`, `#11149`).
- [ ] `Only if the relay-beacon path stalls or proves unusable, reassess whether any local threshold runtime work should survive as fallback research.`

## Operator-local validation entrypoints

- `./scripts/aaa-release-gate.sh`
  Runs the documented AAA scheduler release gate (fairness matrix, topology matrix, sparse long-run liveness, 10k stress, occupancy profile).

- `./scripts/try-runtime-local.sh --prepare`
  Builds the runtime with `try-runtime`, prepares the local Zombienet dev chain, and runs the canonical `on-runtime-upgrade live` plus `execute-block live` dry-runs against the parachain RPC.
