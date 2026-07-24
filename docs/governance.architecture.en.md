# Governance: Bounded Reward-Memory and Proposal Lifecycle Architecture

> Contract layer: [`governance.specification.en.md`](./governance.specification.en.md)
>
> **On-Chain Namespace**
>
> - Pallet: `pallet-governance`
> - This pallet is state-only in the current runtime: it does not own a pallet-derived sovereign account
> - Current runtime type bindings:
>   - `DomainId = AssetId`
>   - `WinningVoteItemId = u32`
>   - `Epoch = BlockNumber`
> - Admin origin: `Root`

## Executive Summary

`pallet-governance` is the live bounded governance kernel in the current DEOS reference runtime. In the shipped reference line, it feeds the TMCTOL standard's governance-conditioned staking reward path.
It serves two coupled but distinct roles:

1. `reward-memory kernel`
   It tracks bounded rolling winning-vote memory per `(domain, account)`, cumulative participation plus proposal-authorship totals for GovXP inputs, and exports a normalized `reward_coefficient(domain, account)` plus raw GovXP counter inputs

2. `proposal lifecycle kernel`
   It provides a bounded active proposal, ballot, resolution, rejection, auto-finalization, and recent finalized-outcome surface

The pallet is intentionally not a maximal governance platform.
It is a bounded runtime component whose main architectural job is to turn governance outcomes into sparse, queryable reward memory without unbounded storage growth.

## Current Divergence from the Target Specification

The new contract layer in [`governance.specification.en.md`](./governance.specification.en.md) is intentionally broader than the runtime slice described here.
The most important current gaps are:

**1. First dual-mode protection-track slice exists, but it remains narrower than the target contract.**

The runtime exposes a separate protection track with `Veto` and protection-track `Pass`. Each account may cast one ordinary-track vote and one protection-track vote on the same proposal, with bounded replacement of its protection-track side for the same item. The path remains universal across today's protected proposal set.

The backing surface is domain-specific: the `$VETO` asset protects protocol / network governance, while native `$NTVE` stake protects `$BLDR` governance. Raw `Veto` turnout must clear a `1%` dust floor before the final fail-closed gate can activate. Protection remains admissible until the configured protection-window close instead of the older first-touch activation deadline. Richer class families and broader track menus remain future work.

**2. The Declining Power slice is closer to the target contract, with one narrowly scoped urgent exception.**

Ballot weight follows the shipped piecewise `7x -> 1x` curve through a bounded vote-time epoch stored per ballot. The pallet separates ordinary-track and protection-track weighting windows internally so those clocks can diverge honestly later.

An explicit urgent-policy query surface per proposal makes the expeditable contract visible. Current policy remains narrow: only strategic `L1RootAction` proposals are expeditable, and the fast path requires unanimous raw protection-track `Pass` over `100%` of eligible `$VETO` supply.

A tighter runtime-upgrade-only exception sits above the generic urgent path. Once unanimous protection passes, the pallet immediately resolves and executes runtime-upgrade authorization without waiting for a separate primary-track ballot.

Outside that exception, urgent handling remains disabled until constitutional/runtime policy opts more `(domain, payload_kind)` pairs in. The remaining gap concerns the narrow policy surface, not missing urgent mechanics or an absent implementation path.

**3. Proposal control remains admin-heavy.**

Signed users can cast votes. The browser exposes the bounded advisory submit path and the first minimal tactical treasury invoice submit path. The runtime also opts tactical `$BLDR` `L2TreasurySpend` into signed submission alongside the earlier advisory combinations.

Manual resolution/rejection, requeue, and policy-aware early finalization otherwise remain `Root`-gated in the current runtime.

**4. A pallet-side payload executor scaffold and the first executable runtime slices are live.**

The governance kernel has a canonical bounded enactment scaffold. Finalized proposals can stay as approved-only outcomes, enter bounded pending-enactment buckets, finalize advisory payloads without dispatch, or record generic execution success / failure when a runtime payload executor is enabled.

The current runtime enables one bounded `L1RootAction` slice. In the strategic domain, it may dispatch preimage-backed `RuntimeCall::System(authorize_upgrade { code_hash })` under Root-equivalent authority.

The bounded `L2ParameterChange` slice supports two narrow domain-local paths. Preimage-backed `RuntimeCall::AxialRouter(add_tracked_asset { asset })` and bounded `RuntimeCall::AxialRouter(update_router_fee { new_fee })` apply through governance-owned internal setters instead of Root dispatch.

Tactical `L2TreasurySpend` decodes a bounded invoice payload with `beneficiary`, `payout_asset`, `base_amount`, and explicit funding source. It reads the resolved winning primary option from bounded governance state, applies the on-chain invoice scalar, and transfers from the designated BLDR Treasury sovereign account rather than as Root.

The treasury authority topology remains explicit. The tactical `$BLDR` domain declares exactly one executable funding source, `BldrTreasury`, which resolves to the domain treasury sovereign account only for that domain. Wider source families or native payout topologies remain future opt-in work, not hidden rights of the invoice payload.

Launch-line `$BLDR` referenda are live invoice-shaped rather than merely invoice-centric in concept. Changes to System AAA behavior still require `L2SignalToL1` or explicit delegation of those control surfaces into the domain.

A scan of Root-only custom-pallet controls narrows the next truthful `L2ParameterChange` search space. TMC launch-physics mutation remains out of contract; staking onboarding, recovery, and admin reward-bootstrap paths remain system-owned; AAA global breaker and actor-limit controls remain system-owned; asset-registry registration and migration remain L1-owned.

The remaining runtime gap is narrower than a generic "more setters" wishlist. The next valid slice must expose a genuinely delegated, domain-owned parameter surface rather than opportunistically reuse unrelated Root setters.

**5. Runtime-upgrade execution closely matches the intended post-bootstrap line.**

Governance can drive the first parachain-safe step through a bounded `L1RootAction` payload carrying only `code_hash` and dispatching `System::authorize_upgrade { code_hash }`. `ProposalRuntimeUpgradeAuthorized` reports this success separately from generic execution success.

The bounded `authorized_runtime_upgrade()` governance/runtime view exposes pending relay without requiring browser code to read raw `System.AuthorizedUpgrade` storage. The browser uses that view to show when authorization still awaits `apply_authorized_upgrade { code }`, while stating that it does not expose that live write path.

Integration coverage proves the paired system-level second step works after authorization. Any origin may submit matching bytes to `System.apply_authorized_upgrade { code }`. Invalid authorized code clears the pending authorization with an explicit system rejection event instead of silently wedging state.

One constitutional acceleration rule overlays that path. Unanimous raw protection-track `Pass` over the full eligible `$VETO` supply immediately resolves and executes authorization without a separate primary ballot. Ordinary and urgent machinery for other payload kinds remains untouched.

The post-bootstrap role split stays explicit: governance authorizes the code hash, while `apply_authorized_upgrade` only transports already-authorized bytes rather than making a second governance decision.

The repository ships `/scripts/authorized-upgrade-local.sh check` as a plan-only verifier for local WASM against the pending hash and `/scripts/authorized-upgrade-local.sh apply` as the explicit relay submit surface. Local dev bootstrap no longer depends on Sudo for the wallet/swap seeding surfaces that previously blocked this handoff.

**6. Invoice-family storage, queries, resolution, and runtime activation exist.**

Outside the protection path, storage-level primary voting no longer hardcodes only `Aye / Nay`. The vote enum and bounded ballots represent invoice-family `Amplify / Approve / Reduce / Nay`, and family-aware cast validation rejects mismatched primary vote kinds.

The family-aware `proposal_primary_track_tally(domain, item_id)` query reports deterministic lowest-scalar tie-breaking for the leading positive invoice option. The kernel resolves invoice families into `PassingAmplify / PassingApprove / PassingReduce` instead of collapsing them into binary approval.

The reference runtime marks tactical `L2TreasurySpend` in the canonical `$BLDR` domain as `Invoice`, making those semantics live rather than latent.

**7. The target public cadence is shipped, while urgent policy remains narrow.**

The pallet has additive timing scaffolding, pending-enactment status handling, split ordinary/protection weighting windows, a generic lead-in gate for ordinary ballots, bounded servicing for due pending-enactment buckets, and a generic urgent fast-track mechanism.

Runtime policy opens protection at submission and ordinary primary after a `3 day` lead-in. Ordinary protection and primary each run for `7 days`; successful ordinary approvals receive a default `3 day` enactment delay.

The urgent line opts in exactly one strategic exception. Protocol `L1RootAction` is expeditable and uses unanimous raw protection-track `Pass` for immediate runtime-upgrade authorization; the rest of the launch line remains deny-by-default.

Confirm-period machinery remains disabled. The remaining gap concerns broader call-matrix expansion, richer execution observability, and later urgent-policy opt-ins rather than public cadence policy.

**8. Payload readiness is observable, but admission remains soft.**

The runtime persists `payload_hash` metadata, exposes derived execution authority, and reports `proposal_payload_availability(domain, item_id)` through the canonical preimage provider. It does not yet hard-enforce a payload-kind-specific admission/execution gate beyond that readiness scaffold.

The target contract permits executable payload submission before the full preimage is noted when the canonical request/readiness surface remains honest. Actual enactment still requires runtime-visible payload availability.

**9. GovXP matches the counters-first v1 contract.**

The runtime exports only bounded GovXP input counters. No separate live GovXP multiplier remains in the canonical runtime query contract, keeping v1 aligned with the specification's counters-first / defer-multiplier policy.

This architecture doc now matches that narrowed contract directly.

## Architecture Overview

### Design Principles

1. `Reward coefficient stays winning-memory-first`
   The exported staking reward coefficient is still based on counted winning outcomes rather than raw vote count or mere turnout, even though the pallet now also accumulates monotonic participation/authorship totals for GovXP inputs

2. `Bounded everywhere`
   live memory, per-epoch item sets, per-call account sets, active proposals, maturity buckets, finalized-outcome retention, and expiry servicing are all bounded

3. `Runtime-as-Config`
   lookback width, per-epoch caps, vote weight, voting period, threshold, turnout floor, and retention are runtime policy, not pallet constants

4. `Proposal lifecycle as ingress path`
   proposal handling exists mainly to feed reward memory through a real lifecycle rather than leaving governance reward accounting at manual admin injection forever

5. `Recent observability over archival accumulation`
   active tallies, resolution-state, unified status, and recent finalized outcomes are queryable, but finalized history expires

### System Architecture

```mermaid
graph TD
    Admin[Root / Governance] -->|submit_proposal| Active[Active proposals]
    User[Signed voter] -->|cast_vote| Ballots[Bounded ordinary + protection-track sets]
    Admin -->|resolve_proposal / reject_proposal| Active
    Admin -->|record_winning_vote*| Memory[Winning-vote memory]

    Active -->|maturity epoch| Buckets[ProposalMaturityBuckets]
    Buckets -->|on_initialize| Resolve[Resolve or reject from current votes]
    Resolve --> Outcomes[Finalized outcomes]
    Resolve --> Memory

    Memory --> Coeff[reward_coefficient(domain, account)]
    Coeff --> Staking[pallet-staking reward snapshots]

    Expiry[Expiry buckets] -->|on_initialize| Memory
    OutcomeExpiry[Finalized outcome expiry buckets] -->|on_initialize| Outcomes
```

## Architectural Layers

### 1. Winning-vote memory

This is the original core of the pallet.
Per `(domain, account)` the pallet stores a bounded sliding window of winning items:

- Indexed by epoch slots
- Each slot stores bounded `item_id`s
- The window keeps a rolling sum
- Zero-sum windows are evicted from storage

This is the source of exported governance reward weight.

### 2. Resolution-once memory

The pallet also keeps a domain-level `WinningVoteResolutionWindow`.
Its job is different:

- Stop the same `(domain, item_id)` from being ingested twice within the live lookback horizon
- Support bounded non-origin ingestion helpers safely
- Keep manual and proposal-driven reward-memory ingress consistent

This is the "count one resolved item once" guardrail.

### 3. Active proposal lifecycle

A later layer adds real proposal state:

- `submit_proposal(domain, item_id, proposer, cadence_mode, payload_kind, payload_hash)`
- `cast_vote(domain, item_id, vote)`
- `resolve_proposal(domain, item_id, winners)`
- `resolve_proposal_from_votes(domain, item_id)`
- `reject_proposal(domain, item_id)`
- `force_resolve_proposal_from_votes(domain, item_id)`
- `requeue_proposal_for_auto_finalization(domain, item_id)`

This layer is deliberately bounded and narrow, but it is now sufficient to drive governance reward memory from actual proposal outcomes instead of only admin item injection.

### 4. Recent finalized-outcome retention

The newest layer retains recent finalized proposal outcomes in bounded storage:

- Resolved outcome -> epoch + winner count
- Rejected outcome -> epoch + rejection reason
- Bounded expiry buckets delete them later

This gives UI/admin observability without turning the pallet into an unbounded archival store.

## Core Data Structures

### Winning-vote window

`WinningVoteWindow` stores:

- `last_epoch`
- Fixed-width bounded epoch slots
- `rolling_sum`

Each epoch slot holds bounded `item_id`s.
The implementation rotates the ring forward on access and clears expired slots as epochs move.

### Participation counters, proposal tally, and status surfaces

Public read helpers now expose these governance-observability layers, and the same bounded set is exported through FRAME view functions for direct client/light-client consumption without forcing browser-side reconstruction:

1. `govxp_counters(domain, account)`
   - Rolling winning participation inside the bounded live tail
   - Cumulative total participation
   - Cumulative total winning-side participation
   - Cumulative total authored proposals
   - Cumulative total successful authored proposals

2. `recent_finalized_proposals(domain)`
   - Bounded per-domain recent-finalized proposal discovery
   - Newest-first summary entries carrying `item_id + FinalizedProposalOutcome`

3. `proposal_vote_tally(domain, item_id)`
   - Voter counts
   - Weighted primary-track `Aye / Nay / Amplify / Approve / Reduce`
   - Weighted protection-track `Veto / Pass`
   - Total primary turnout plus total protection turnout

4. `proposal_vote_power_profile(domain, item_id, vote_kind)`
   - Runtime-declared power-profile identity for the live track family behind that vote kind
   - Current runtime returns `DecliningDirectStake` for ordinary `Aye / Nay`
   - Current runtime returns `DecliningVetoAsset` for protocol / network protection-track `Veto / Pass`
   - Current runtime returns `DecliningNativeStake` for `$BLDR`-domain protection-track `Veto / Pass`, backed by locked `$NTVE`, locked `stNTVE`, and locked `NTVE/stNTVE` LP-derived `NativeVotePower`

5. `proposal_resolution_state(domain, item_id)`
   - `VotingWindowOpen { current_epoch, maturity_epoch }`
   - `PassingAye`
   - `PassingAmplify / PassingApprove / PassingReduce` when family policy enables invoice-style primary resolution
   - `PassingNay`
   - `Rejected { reason }`

6. `proposal_status(domain, item_id)`
   - `Active(ProposalResolutionState)` while proposal storage still exists
   - `PendingEnactment { outcome, enactment_epoch }` after successful finalization when a positive enactment delay is scheduled and not yet elapsed
   - `Finalized(FinalizedProposalOutcome)` once proposal state is gone and either no enactment delay exists or the pending window has elapsed

7. `proposal_metadata(domain, item_id)`
   - Additive proposal-meaning scaffold over one item
   - `CadenceMode + ProposalPayloadKind + payload_hash`
   - Proposal submission now requires explicit cadence mode, payload kind, and payload hash at the extrinsic boundary; tests/helpers that want a shorthand must provide their own local wrapper above that contract

8. `proposal_execution_authority(domain, item_id)`
   - Additive execution-scope scaffold derived from payload kind
   - Currently resolves to one of `Root`, `DomainTreasury`, `DomainParameters`, or `NonExecutable`

9. `authorized_runtime_upgrade()`
   - Additive bounded runtime-upgrade authorization scaffold over the current chain state
   - Reports whether governance (via the system pallet) has already authorized one runtime code hash for later relay/application and whether the later apply step still requires version checking
   - The current operator/tooling line interprets that bounded view through three truthful phases: `awaiting-governance-authorization`, `authorized-hash-mismatch`, and `ready-to-relay-code`

10. `proposal_submission_authority(domain, payload_kind)`

- Additive submission-scope scaffold over one `(domain, payload_kind)` pair
- Reports whether the current runtime treats that combination as `Signed` or `AdminOnly`
- The current launch line now opts the first bounded public submission slices in: `Intent` is `Signed` across domains, tactical `$BLDR` `L2SignalToL1` is also `Signed`, tactical `$BLDR` `L2TreasurySpend` is now `Signed` too, and the remaining executable kinds plus the remaining advisory combinations stay `AdminOnly`

11. `proposal_opening_fee(domain, payload_kind)`

- Additive public-submission cost scaffold over one `(domain, payload_kind)` pair
- Reports the Fee Sink-collected opening fee only for combinations that are actually `Signed`
- The current launch line now returns the configured native opening fee for `Intent`, tactical `$BLDR` `L2SignalToL1`, and tactical `$BLDR` `L2TreasurySpend`, and `None` for admin-only combinations

12. `proposal_payload_availability(domain, item_id)`

- Additive payload-readiness scaffold over one item
- Reports whether the stored `payload_hash` currently has a registered preimage and whether that preimage is requested in the canonical runtime preimage subsystem

13. `payload_hash_preimage_status(payload_hash)`

- Additive preimage-status scaffold over one payload hash before proposal submission exists
- Reports whether that exact hash already has a noted preimage, whether it is requested but not yet noted, and the noted payload length when available, so browser-side advisory composition can stay on canonical governance query surfaces instead of reading raw preimage storage layout directly

14. `payload_preimage_note_cost(payload_len)`

- Additive bounded preimage-note cost scaffold over one byte length
- Reports the current runtime's generic preimage note deposit for that payload length so browser-side signed advisory composition can quote the optional `Preimage.note_preimage` path honestly without hardcoding runtime pricing constants

15. `proposal_primary_track_family(domain, item_id)`

- Additive primary-track contract scaffold over one proposal item
- Reports whether the current runtime treats that proposal's primary lane as `Binary` or `Invoice`
- The current reference runtime now returns `Invoice` for tactical `L2TreasurySpend` in the canonical `$BLDR` domain and `Binary` elsewhere on the current launch line

`proposal_primary_track_tally(domain, item_id)` is the companion family-aware primary-lane summary.
For binary families it reports `Aye / Nay` weights plus the current leading side.
For invoice families it reports `Amplify / Approve / Reduce / Nay` weights, aggregate positive weight, and deterministic lowest-scalar tie-breaking for the current leading positive option.

`retained_proposal_winning_primary_option(domain, item_id)` is the retained finalized-outcome companion.
It reports the already-selected winning primary option (`Aye / Nay / Amplify / Approve / Reduce`) while bounded finalized retention still exists, so delayed enactment, clients, and audits do not need to reconstruct winner identity from internal executor paths or raw tallies alone.

13. `proposal_timing(domain, item_id)`

- Additive timing scaffold over one active proposal
- Submitted epoch, protection open/close, ordinary primary open/close
- Optional urgent-open override and optional pending-enactment epoch

14. `proposal_urgent_eligibility(domain, item_id)`

- Additive urgent-policy scaffold over one proposal item
- Reports whether that proposal's current `(domain, payload_kind)` combination is configured as expeditable by the runtime policy surface
- The current reference runtime explicitly returns `false` for all launch-line combinations until a later urgent-policy rollout opts some in

That last distinction is important: a matured proposal may already be logically failing policy and still be `Active(Rejected { ... })` until explicit/manual/automatic finalization executes.

Active proposer identity is also chain-native today through the bounded `ProposalAuthorsByItem` storage getter, even though the current implementation has not promoted that tiny active-only surface into a dedicated view helper yet.

## Storage Topology

- `WinningVoteWindows[(domain, account)]`: per-account rolling winning-memory tail; sparse, zero-sum evicted
- `ParticipationTotalsByAccount[(domain, account)]`: cumulative participation totals
- `ProposalAuthorshipTotalsByAccount[(domain, account)]`: cumulative authorship totals
- `WinningVoteResolutionWindows[domain]`: resolution-once memory; prevents duplicate live-horizon ingestion
- `ActiveProposals[(domain, item_id)]`: live proposal registry storing `submitted_epoch`
- `ProposalAuthorsByItem[(domain, item_id)]`: explicit logical proposer / sponsor per active proposal
- `ProposalMetadataByItem[(domain, item_id)]`: `CadenceMode`, payload kind, and payload hash scaffold
- `ProposalVotesByItem[(domain, item_id)]`: bounded primary/protection ballot sets with frozen weight/raw power
- `GovernanceLocks[account]`: aggregate `lock_until` extended to the maximum touched enactment horizon
- `ProposalUrgentAuthorizedAt[(domain, item_id)]`: written once when expeditable `Pass` crosses raw threshold
- `ProposalPendingEnactmentAt[(domain, item_id)]`: approval scheduling state when enactment delay is positive
- `PendingEnactmentBuckets[epoch]`: epoch-keyed bounded servicing for pending enactment attempts
- `ActiveProposalCounts[domain]`: domain-local active cap tracking
- `ActiveProposalIdsByDomain[domain]`: canonical bounded live list for active item ids in one domain
- `ProposalMaturityBuckets[epoch]`: epoch-keyed auto-finalization schedule, no global active scan
- `FinalizedProposalOutcomes[(domain, item_id)]`: queryable but temporary recent finalized result
- `ProposalWinningPrimaryOptionByItem[(domain, item_id)]`: retained resolved primary-side winner for enactment
- `FinalizedProposalOutcomeExpiryBuckets[epoch]`: finalized-outcome retention control
- `ExpiryBuckets[epoch]`: winning-vote expiry schedule for accounts whose memory may decay
- `LastProcessedEpoch`: `on_initialize` service cursor preventing repeated work

`Migration state`:
The current pre-fork storage baseline is `3` in this repository line. The active ballot schema stores vote-time epoch plus frozen computed weight and raw protection power directly in `ProposalVotesByItem`, and `GovernanceLocks` stores account-level lock horizons; downstream live forks must own explicit migrations if they carry an older deployed governance schema.

## Core Execution Flows

### 1. Winning-vote ingestion

Low-level ingress surfaces:

- `record_winning_vote(domain, item_id, account)`
- `record_winning_vote_batch(domain, item_id, accounts)`
- `ingest_winning_vote_resolution(domain, item_id, account)`
- `ingest_winning_vote_resolution_batch(domain, item_id, accounts)`

Implementation behavior:

1. ensure lookback > 0
2. load current epoch
3. record the item once in the domain-level resolution window
4. increment cumulative winning participation, and when the helper is the only available participation ingress also increment cumulative total participation for the provided accounts
5. record the item for each account in the account-level winning window
6. schedule account expiry at `current_epoch + lookback`
7. emit `WinningVoteRecorded`

The batch helpers are transactional, so late failures do not strand partial reward memory.

### 2. Reward coefficient calculation

`reward_coefficient(domain, account)`:

- Reads the account's winning-vote window
- Rotates it to the current epoch
- Returns `rolling_sum / (lookback * max_votes_per_epoch)` as `FixedU128`
- Returns zero if the window is absent or empty

So the coefficient is normalized against the runtime's own configured capacity, not against an unbounded historical total.

### 3. Proposal submission and voting

`submit_proposal(domain, item_id, proposer, cadence_mode, payload_kind, payload_hash)`:

- Admin-only explicit submit path
- Checks duplicate active proposal for the same item
- Checks domain active-cap bound
- Records `submitted_epoch`
- Records one explicit logical `proposer` / sponsor for that active item
- Records additive proposal metadata (`CadenceMode + ProposalPayloadKind + payload_hash`)
- Increments the proposer's cumulative authored-proposal GovXP counter
- Computes `maturity_epoch = submitted_epoch + ProposalLeadInPeriod + ProposalVotingPeriod`
- Inserts a maturity touch into `ProposalMaturityBuckets[maturity_epoch]`
- Emits `ProposalSubmitted`

`submit_signed_proposal(domain, item_id, cadence_mode, payload_kind, payload_hash)`:

- Signed submit path for runtime-approved public combinations only
- Derives proposer identity from the signer rather than an admin-supplied sponsor field
- Transfers the runtime-configured native opening fee into Fee Sink before proposal creation
- Uses transactional semantics so duplicate/cap failures do not strand a collected fee
- Reuses the same bounded active-proposal insertion path and GovXP authorship accounting once admitted
- Emits `ProposalOpeningFeeCollected` when the opening fee is non-zero, then `ProposalSubmitted`

`cast_vote(domain, item_id, vote)`:

- Requires an active proposal
- Stores one ordinary-track vote per account in `ayes / nays`
- Stores one protection-track vote per account in `vetoes / passes`
- Allows the same account to participate once in the ordinary track and once in the protection track for the same item
- Increments cumulative total participation exactly once on the first proposal-level ballot that account casts for that item
- Rejects duplicate voting inside the ordinary track family
- Treats a later protection-track `Veto` or `Pass` ballot as bounded replacement of that account's earlier protection-track side for the same item
- Rejects protection-track ballots once the configured protection window has closed for that proposal
- Rejects over-cap voter sets
- If a newly updated raw `Veto` tally becomes **strictly greater** than the runtime threshold against the domain's total eligible protection supply, the extrinsic finalizes the proposal immediately as `VetoCancelled`
- The later maturity-time protection gate only becomes active once raw `Veto` turnout reaches the runtime dust floor against that same protection supply
- Emits `ProposalVoteCast`

### 4. Resolution from weighted votes

`proposal_resolution_state(domain, item_id)` is the shared policy evaluator.
Its order is now:

1. check immediate-threshold cancellation against the domain's live protection supply
2. if the voting window is still open, return `VotingWindowOpen`
3. at/after maturity, check whether raw `Veto` turnout reaches the runtime dust floor and, if it does, evaluate the separate protection-track final gate (`Veto` vs `Pass`) before ordinary ballot policy
4. if protection does not block, evaluate ordinary turnout / approval policy

The shipped protection-track final gate is currently fail-closed once that raw `Veto` floor is met: if `Pass` does not strictly outweigh `Veto`, the proposal resolves through the protection branch instead of ordinary `Aye / Nay` approval. Sub-percent dust `Veto` turnout is intentionally ignored for final-gate activation.

Then:

- `resolve_proposal_from_votes(...)` applies that policy and enforces the voting-window guard
- `force_resolve_proposal_from_votes(...)` applies the same policy but bypasses only the timing guard

That means the admin override is policy-aware, not winner-injection by hand.

### 5. Manual resolve / reject paths

The pallet still keeps narrow admin tools:

- `resolve_proposal(domain, item_id, winners)`
- `reject_proposal(domain, item_id)`

`resolve_proposal(...)` bypasses the stored ballot policy and credits the provided winner set directly, but remains bounded by `MaxWinningVoteAccountsPerCall`.
This is intentionally retained as a recovery / narrow-control surface.

### 6. Auto-finalization on `on_initialize`

The pallet's only hook is `on_initialize`, which calls `service_current_epoch(current_epoch)`.
Today that routine services four bounded bucket families in order:

1. `service_maturing_proposals(...)`
2. `service_pending_enactments(...)`
3. `service_finalized_proposal_outcomes(...)`
4. `service_expiring_accounts(...)`

That means pending enactment is no longer merely query/status scaffolding inside the pallet itself. The remaining gap sits one layer higher: executable payload kinds still need real runtime dispatch wiring before those bounded due-enactment attempts can do useful work on the reference runtime.

#### Maturity servicing

For each due proposal touch:

- If the proposal is already gone, skip it
- Try `resolve_active_proposal_from_votes(...)`
- If success, proposal resolves or rejects normally
- If failure, attempt to reschedule into the next epoch and emit `ProposalAutoFinalizationDeferred`

This is the key architectural choice: no full scan of all active proposals every block.

#### Account-expiry servicing

For each due `(domain, account)` touch:

- Rotate the account window to the current epoch
- If `rolling_sum == 0`, delete storage and emit `WinningVoteWindowEvicted`

This keeps reward memory sparse.

#### Finalized-outcome servicing

For each due finalized-outcome touch:

- Delete `FinalizedProposalOutcomes[(domain, item_id)]`

This enforces bounded recent-history retention.

## Finalized Outcome Retention

Finalized outcomes are recorded from both final paths:

- `resolve_active_proposal(...)` first records `Resolved { epoch, winner_count }`
- `reject_active_proposal(...)` records `Rejected { epoch, reason }`
- `veto_cancel_active_proposal(...)` records `VetoCancelled { ... }`

After approval, the newer enactment scaffold may later overwrite that initial resolved outcome with one of:

- `Enacted { approved_epoch, executed_epoch, winner_count }`
- `ExecutionFailed { approved_epoch, failed_epoch, winner_count }`
- `AdvisoryFinalized { approved_epoch, finalized_epoch, winner_count }`

The retention schedule still keys off the original finalized-approval insertion epoch rather than extending history indefinitely after enactment attempts.

They are inserted immediately and scheduled to expire at:

```text
current_epoch + FinalizedProposalOutcomeRetentionEpochs
```

This is a deliberate product/engineering compromise:

- Enough retained state for runtime queries and UI recovery
- No permanent on-chain archive inside the kernel pallet

## Identity, Uniqueness, and Retention Semantics

The current pallet does **not** treat `(domain, item_id)` as a permanent archival identity.
Its uniqueness guarantees are bounded by the currently relevant horizons:

1. `ActiveProposals[(domain, item_id)]`
   prevents duplicate live active proposals for the same item

2. `WinningVoteResolutionWindows[domain]`
   prevents the same resolved item from being credited twice while it remains inside the live reward-memory lookback horizon

3. `FinalizedProposalOutcomes[(domain, item_id)]`
   retains recent finalized status for a bounded post-finalization horizon only

So the real implementation contract is:

> `(domain, item_id)` is unique while it is live or still economically relevant, not forever as chain-archival identity

That means consumers should not assume this pallet alone provides eternal governance history.
If long-lived archival identity is needed, indexers, events, or a future dedicated history surface must carry that burden.

The related state distinction is also important:

- `proposal_resolution_state(...)` describes the current policy result of an **active** proposal
- `proposal_status(...)` returns active state first and only falls back to retained finalized outcome once active proposal storage is gone
- `FinalizedProposalOutcomes` describes a concluded proposal, but only while retention has not yet expired

## Public Call Surface

| Call | Extrinsic | Role |
| --- | --- | --- |
| `0` | `record_winning_vote` | low-level admin ingress |
| `1` | `record_winning_vote_batch` | bounded admin batch ingress |
| `2` | `submit_proposal` | admin proposal create path |
| `3` | `submit_signed_proposal` | signed create with collected fee |
| `4` | `resolve_proposal` | manual bounded resolution |
| `5` | `reject_proposal` | manual rejection |
| `6` | `cast_vote` | signed ballot |
| `7` | `resolve_proposal_from_votes` | maturity resolution |
| `8` | `requeue_proposal_for_auto_finalization` | deferred-item recovery |
| `9` | `force_resolve_proposal_from_votes` | policy-aware early finalization |

## Events and Errors

### Events that form the live operational surface

- `ProposalOpeningFeeCollected`: signed public submission paid the opening fee into Fee Sink
- `ProposalSubmitted`: active proposal creation, proposer identity, payload metadata, active-count pressure
- `ProposalVoteCast`: bounded ballot ingress with vote epoch, replacement state, and stored track counts
- `GovernanceLockExtended`: account-level lock horizon extension after an accepted ballot
- `ProposalUrgentAuthorized`: expeditable `Pass` threshold crossed, with epoch and raw pass/supply context
- `ProposalResolved`: proposal closure and winner count credited into reward memory
- `ProposalEnactmentScheduled`: approval moved into bounded pending-enactment servicing
- `ProposalExecuted`: executable payload handling succeeded and names the successful payload kind
- `ProposalRuntimeUpgradeAuthorized`: current `L1RootAction` success slice and authorized code hash
- `ProposalParameterChangeExecuted`: current `L2ParameterChange` success slice and bounded parameter identity
- `ProposalTreasurySpendExecuted`: treasury spend funding source, beneficiary, asset, scalar, payout, settlement
- `ProposalExecutionFailed`: bounded enactment attempted but failed, with payload kind and failure category
- `ProposalAdvisoryFinalized`: non-dispatch advisory finalization as `Intent` or `L2SignalToL1`
- `ProposalRejected`: proposal closure and explicit rejection reason
- `ProposalVetoCancelled`: separate protection track cancelled the proposal without reward credit
- `ProposalAutoFinalizationDeferred`: maturity servicing did not finish cleanly and may need requeue
- `ProposalAutoFinalizationRequeued`: manual recovery of deferred maturity scheduling
- `WinningVoteRecorded`: reward-memory credit for one winner account
- `WinningVoteWindowEvicted`: zero-sum reward-memory eviction after expiry

### Errors that expose real runtime boundaries

- `ProposalAlreadyActive` / `ProposalNotActive`: active-state identity guard
- `ProposalVotingWindowStillOpen`: vote-derived resolution attempted before maturity
- `ProposalVoteAlreadyCast` / `ProposalVoteSetFull` / `ProposalProtectionTrackClosed`: bounded ballot and protection-window guards
- `ProposalWinnerSetEmpty`: manual resolution cannot inject an empty winner set
- `ActiveProposalCapReached`: domain-local active proposal budget exhausted
- `ProposalMaturityBucketFull`: auto-finalization scheduling cap hit for one epoch
- `DuplicateWinningVoteItem` / `DuplicateWinningVoteResolutionItem`: live memory uniqueness violation
- `FinalizedProposalOutcomeExpiryBucketFull`: finalized-outcome retention expiry cap hit
- `ExpiryBucketFull`: account-expiry scheduling hit its bounded service bucket cap

## Runtime Binding

Current runtime wiring in `template/runtime/src/configs/governance_config.rs`:

- `AdminOrigin = Root`
- `EpochProvider = System::block_number()`
- `ProposalVoteWeightProvider = RuntimeProposalVoteWeightProvider`
- `ProposalTrackPowerProfileProvider = RuntimeProposalTrackPowerProfileProvider`
- `VetoVotePowerProvider = RuntimeVetoVotePowerProvider`
- `WeightInfo = runtime weight bridge`

Current runtime policy values:

- `WinningVoteLookbackEpochs = 3`
- `MaxWinningVotesPerEpoch = 4`
- `MaxWinningVoteItemsPerEpoch = 4`
- `MaxWinningVoteResolutionItemsPerEpoch = 64`
- `MaxWinningVoteAccountsPerCall = 256`
- `MaxActiveProposalsPerDomain = 128`
- `MaxMaturingProposalsPerEpoch = 4`
- `ProposalVotingPeriod = 7 days` (`100,800` blocks)
- `ProposalLeadInPeriod = 3 days` (`43,200` blocks)
- `ProposalProtectionPeriod = 7 days` (`100,800` blocks)
- `ProposalUrgentVotingPeriod = 1 day` (`14,400` blocks)
- `ProposalEnactmentDelay = 3 days` (`43,200` blocks)
- `ProposalFastTrackPassThreshold = 100%` of eligible protection supply on the current upgrade line
- `ProposalApprovalThreshold = 60%`
- `ProposalVetoThreshold = 50%` of eligible protection supply, strict `>` for immediate cancellation
- `ProposalVetoMinimumVetoTurnout = 1%` of eligible protection supply
- `ProposalMinimumTurnout = 200` weighted units
- `FinalizedProposalOutcomeRetentionEpochs = 16`
- `MaxFinalizedProposalOutcomesPerEpoch = 1024`
- `MaxExpiringAccountsPerEpoch = 1024`

### Vote weight providers

For ordinary `Aye / Nay` voting in normal runtime builds, base balance is still:

```text
Staking::stake_value(domain, account)
```

and the runtime provider transforms that base through the shipped piecewise ballot-time Declining Power curve using bounded proposal-time context (`item_id`, `current_epoch`, `submitted_epoch`, `maturity_epoch`, `vote_epoch`), clamping the final result to `u32`.

For the live protection track, the base surface is domain-specific:

```text
protocol / network governance => Assets::balance(VETO_ASSET_ID, account)
$BLDR tactical governance => Staking::native_stake_value(account)
```

and the runtime applies the same shipped piecewise ballot-time Declining Power rule to the stored protection-track ballot epoch in both cases.

Immediate-threshold cancellation compares frozen raw protection power from recorded protection ballots against total eligible protection supply:

```text
protocol / network governance => Assets::total_issuance(VETO_ASSET_ID)
$BLDR tactical governance => Staking::pool(native_asset_id).accounted_balance
```

using a strict `>` comparison against the runtime threshold, while the end-of-window protection gate first requires raw `Veto` turnout to reach the runtime `1%` dust floor and then compares decline-weighted `Veto` vs decline-weighted `Pass` tallies.

In `runtime-benchmarks` builds, the ordinary provider deliberately falls back to equal weight `1`, while the protection-power provider now falls back to `1 / 1` vote-weight-plus-total-issuance so benchmarking can exercise the immediate-cancellation worst case deterministically.

This demonstrates the project's `Runtime-as-Config` discipline. The pallet does not hardcode one-account-one-vote or an ordinary-ballot staking formula. It keeps ordinary and protection vote-power surfaces runtime-wired rather than baking asset lookup or temporal policy into pallet logic.

The runtime centralizes domain hierarchy, profile identity, and weight behavior through one typed `GovernanceDomainPolicy` declaration and shared consumers in `runtime/src/configs/governance_config.rs`. Protocol `$NTVE + $VETO` governance and tactical `$BLDR + $NTVE` governance therefore stay aligned across tally logic, query identity, and exported domain policy.

The current public surface is intentionally narrow: `governance_domain_policy(domain)` exposes the launch-line ordinary/protection power profiles, but it does not yet attempt to encode richer future class families, execution authorities, or broader constitutional topology beyond the current bounded query contract.

## Current Launch Policy

For the current launch line, the runtime policy is now intentionally frozen to the simplest bounded rule set that already exists in code:

**1. Ballot-time Declining Power.**

Normal runtime builds apply the shipped piecewise `7x -> 1x` curve to ordinary and protection-track ballots. Ordinary `Aye / Nay` derive their base from same-domain `Staking::stake_value(domain, account)`; protection-track `Veto / Pass` use the runtime-declared protection surface for that domain.

For `$BLDR`, the native protection surface adds locked `$NTVE`, locked `stNTVE` converted through the staking exchange rate, and account-level locked `NTVE/stNTVE` LP converted into conservative native-equivalent `NativeVotePower`.

**2. Domain-scoped hierarchy.**

Protocol / network governance runs as `$NTVE` primary + `$VETO` protection, while `$BLDR` tactical governance runs as `$BLDR` primary + `$NTVE` protection. Both use the same bounded `Veto / Pass` cancellation lane with different base-weight surfaces.

**3. Protection-track cancellation.**

Domain-specific backing enables the first live protection slice. Protocol governance uses the well-known `$VETO` asset class, created at genesis with deterministic metadata and an Asset Registry-owned admin surface. `$BLDR` governance uses locked `$NTVE` / `stNTVE` / `NTVE/stNTVE` LP-derived native `NativeVotePower`.

One account may vote once in each track on the same item. Later protection replacements use the later ballot epoch, and protection ballots remain admissible until the configured close.

Immediate cancellation requires frozen raw protection power to be **strictly greater** than the threshold against eligible protection supply. Raw `Veto` turnout below `1%` of supply counts as dust; otherwise the gate stays fail-closed unless decline-weighted `Pass` strictly outweighs decline-weighted `Veto`. Veto-cancelled items receive no governance reward-memory credit.

**4. Override and recovery.**

Admin control stays intentionally narrow. `reject_proposal(...)`, policy-aware `force_resolve_proposal_from_votes(...)`, and `requeue_proposal_for_auto_finalization(...)` form the recovery/override surface, with no broader arbitrary override contract.

**5. Finalized history.**

Bounded finalized-outcome retention is sufficient for the kernel pallet. Durable archival history belongs to events, indexers, or a future dedicated history surface rather than unbounded in-kernel storage growth.

Richer vote-power formulas, broader emergency policy, or permanent on-chain history remain future opt-in choices, not unresolved debt in the current launch baseline.

## Query and Computation Semantics

### Tally and resolution are derived, not cached

The current pallet does not keep a precomputed weighted tally per proposal.
Instead:

- `ProposalVotesByItem[(domain, item_id)]` now stores bounded ballot sets for primary-track `Aye`, `Nay`, `Amplify`, `Approve`, `Reduce` plus protection-track `Veto`, `Pass`, with each ballot carrying the account, vote-time epoch, frozen computed weight, and frozen raw protection power
- `cast_vote(...)` computes ordinary ballot weight through `ProposalVoteWeightProvider` and protection-track ballot weight/raw power through `VetoVotePowerProvider` exactly once at vote time; `proposal_vote_tally(...)` and resolution then sum the stored ballot weights rather than re-reading live balances
- `cast_vote(...)` extends `GovernanceLocks[account].lock_until` to the maximum of its current value and the proposal's effective primary close plus enactment delay; runtime staking integration now uses that horizon to refuse collator-LP, standalone-governance-LP, `$NTVE`, and `stNTVE` unlock requests while the locked position is still custody-backing frozen `NativeVotePower`
- `cast_vote(...)` now already enforces the generic rule that ordinary ballots cannot enter before `primary_open`, while protection-track ballots remain admissible during any configured lead-in
- `proposal_resolution_state(...)` first checks whether frozen raw protection-majority triggers the immediate threshold. After maturity, raw `Veto` turnout must clear the `1%` dust floor before the stored-weight `Veto` versus `Pass` gate applies; only then does the evaluator derive primary state from the family-aware tally.
- Binary families use weighted `Aye / Nay`. Invoice families use `weighted_positive` versus `weighted_nay` with deterministic lowest-scalar tie-breaking across `Amplify / Approve / Reduce`. The launch-line runtime still reports only `Binary`, so invoice resolution remains kernel-ready rather than live reference-line policy.
- Vote-derived finalization paths reuse the same logic rather than carrying a second hidden policy engine

This keeps the pallet simpler and more honest, but it means tally/resolution cost scales with the bounded ballot-set size rather than O(1) cached counters.

### Status precedence matters

`proposal_status(domain, item_id)` is intentionally a two-step query:

1. if active proposal storage exists, return `Active(...)`
2. otherwise, if bounded retained finalized state exists, return `Finalized(...)`

So a mature proposal that is currently failing policy but has not yet been explicitly finalized still reports as active state, not finalized history.

### Hook cost scales with due buckets, not global state

`on_initialize` services:

- Matured proposals due now
- Finalized outcomes due for expiry now
- Accounts due for winning-vote expiry now

This is the core boundedness win of the pallet: service cost tracks due bucket entries rather than the full active or historical state.

## Governance Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](./read-model.contract.en.md) split.

### Canonical on-chain governance projections

The current pallet already provides chain-native reads for known `(domain, item_id)` identities through:

- `active_proposal(domain, item_id)`
- `proposal_votes(domain, item_id)` as low-level ballot storage
- `proposal_vote_tally(domain, item_id)`
- `proposal_resolution_state(domain, item_id)`
- `active_proposal_ids(domain)`
- `proposal_status(domain, item_id)`
- `proposal_metadata(domain, item_id)`
- `proposal_execution_authority(domain, item_id)`
- `authorized_runtime_upgrade()`
- `proposal_submission_authority(domain, payload_kind)`
- `proposal_opening_fee(domain, payload_kind)`
- `proposal_payload_availability(domain, item_id)`
- `payload_hash_preimage_status(payload_hash)`
- `payload_preimage_note_cost(payload_len)`
- `proposal_primary_track_family(domain, item_id)`
- `proposal_timing(domain, item_id)`
- `proposal_urgent_eligibility(domain, item_id)`
- `proposal_execution_detail(domain, item_id)` while bounded retention still exists
- `proposal_primary_track_tally(domain, item_id)`
- `retained_proposal_winning_primary_option(domain, item_id)` while bounded retention still exists
- `proposal_vote_power_profile(domain, item_id, vote_kind)`
- `finalized_proposal_outcome(domain, item_id)` while bounded retention still exists
- `reward_coefficient(domain, account)`
- `govxp_counters(domain, account)`

These authoritative bounded surfaces cover live proposal detail and meaning, submission authority, opening-fee cost, payload readiness, primary-track family and tally interpretation, retained winner identity, urgent eligibility, status and timing, enactment and execution detail, staking reward memory, and GovXP inputs.

The helper/query contract no longer serves only as internal pallet convenience. Canonical governance view functions export these bounded projections, while raw storage remains an implementation detail except where explicitly named as a stable discovery surface.

### Indexed / materialized governance views

The current pallet intentionally does **not** promise these as permanent or canonical on-chain surfaces:

- full referendum archive
- proposal search/filter across expired items
- historical ballot timelines
- long-range participation analytics
- operator dashboards beyond bounded recent state

Those belong to events plus external indexing/materialization rather than permanent in-kernel storage.

### Current discovery boundary

The live pallet now exposes both bounded active-proposal discovery and bounded recent-finalized discovery for one domain:

- `active_proposal_ids(domain)` returns the current live proposal id set
- `recent_finalized_proposals(domain)` returns newest-first bounded recent-finalized summaries for retained outcomes through one canonical runtime view instead of asking clients to sort raw retained-outcome storage themselves
- `ActiveProposals` and `FinalizedProposalOutcomes` remain keyed by `(domain, item_id)` underneath those surfaces

That means live proposal discovery and bounded recent-finalized discovery are chain-native today, while full archive/search/filter UX across expired history still belongs to explicit indexed/materialized views. Consumers SHOULD NOT treat ad-hoc iteration over current raw storage topology as the stable product contract.

### Current runtime-upgrade operator path

The current line now has one explicit bounded off-browser operator flow for already-authorized runtime code:

1. governance authorizes one `code_hash` through the `L1RootAction -> System.authorize_upgrade { code_hash }` path
2. operators read `authorized_runtime_upgrade()` to learn whether authorization exists and whether version checking remains enabled
3. `/scripts/authorized-upgrade-local.sh check` compares a local WASM blob against that authorized hash and now classifies the result as `awaiting-governance-authorization`, `authorized-hash-mismatch`, or `ready-to-relay-code`
4. once the phase reaches `ready-to-relay-code`, `/scripts/authorized-upgrade-local.sh apply` provides the dedicated operator-facing submit surface for the external relay step and stays plan-only unless `--submit` is passed explicitly
5. only after that readiness check should an external system-origin path relay matching bytes through `System.apply_authorized_upgrade { code }`

This is intentionally still an off-browser operator flow rather than a browser action. The browser governance surface remains read-only for that second step, while the verifier and relay helper both default to non-submitting behavior.

### Current post-bootstrap relay contract

There is no longer a separate bootstrap superuser owner for the external relay step.
The current contract is narrower and cleaner:

- governance decides _which_ `code_hash` is authorized
- any operator MAY relay the matching code bytes after the verifier reaches `ready-to-relay-code`
- that relay step MUST remain ministerial rather than becoming a second governance veto or reinterpretation surface

This keeps the current line honest: governance owns authorization of upgrade intent, while the later `apply_authorized_upgrade` call is only a transport step for already-authorized bytes.

## Integration Boundary with Staking

The key exported staking surface is:

- `reward_coefficient(domain, account)`

The pallet also now exports GovXP input observability through:

- `govxp_counters(domain, account)`

In the current runtime, staking maps:

- `reward_governance_domain(asset_id) = asset_id`
- `reward_coefficient(asset_id, account) = Governance::reward_coefficient(asset_id, account)`

So the governance pallet is already live inside the staking reward-weight pipeline.

## Validation Surface

The implementation is covered by:

- Pallet tests in `template/pallets/governance/src/tests.rs`
- Runtime integration tests in `template/runtime/src/tests/staking_integration_tests.rs`
- FRAME v2 benchmarks in `template/pallets/governance/src/benchmarking.rs`
- Runtime weight bridge in `template/runtime/src/weights/pallet_governance.rs`

Coverage includes:

- Duplicate item protection
- Transactional rollback of late batch failures
- Weighted vote-derived outcomes
- Turnout and approval-threshold rejection
- Auto-finalization and retry deferral
- Early force resolution
- Finalized outcome retention and expiry
- Governance -> staking reward-coefficient propagation

## Integrator Checklist

### Canonical read path

For most consumers, query in this order:

1. `active_proposal_ids(domain)` when the product needs the current live proposal list for one domain
2. `recent_finalized_proposals(domain)` when the product needs the bounded retained recent-finalized list for one domain
3. `proposal_status(domain, item_id)` for one known item
4. `proposal_metadata(domain, item_id)` when the product needs the additive payload/cadence scaffold for one item
5. `proposal_execution_authority(domain, item_id)` when the product needs the currently derived execution scope for that item
6. `proposal_payload_availability(domain, item_id)` when the product needs to know whether the stored payload hash is actually backed by a canonical preimage
7. `proposal_timing(domain, item_id)` when the product needs the additive timing scaffold for one live item
8. `proposal_execution_detail(domain, item_id)` when the product needs the retained bounded enactment/advisory detail for one known item
9. `proposal_vote_tally(domain, item_id)` when active-state tally detail is needed
10. `proposal_vote_power_profile(domain, item_id, vote_kind)` when UI or operators need the declared live power-profile identity behind a track
11. `finalized_proposal_outcome(domain, item_id)` only when a consumer explicitly wants the retained finalized record for one known item rather than the bounded list surface or unified status surface

If the product needs archive/search/filter beyond that bounded per-domain recent-finalized surface, keep it explicitly indexed/materialized rather than pretending expired history is still a canonical chain-native list.

### Interpret the state correctly

- `Active(VotingWindowOpen { ... })` -> still inside configured voting period
- `Active(VetoPassing { ... })` -> either the separate protection track already exceeds the immediate-threshold contract or the matured protection-track gate has cleared the raw veto floor and is currently blocking ordinary resolution, but explicit/manual/automatic finalization has not removed proposal storage yet
- `Active(PassingAye | PassingNay)` -> mature and currently passing by policy, but not yet finalized
- `Active(Rejected { ... })` -> mature and currently failing by policy, but not yet finalized
- `PendingEnactment { ... }` -> proposal storage is gone, approval is finalized, and a positive enactment delay is still counting down
- `Finalized(...)` -> proposal storage is gone and a retained finalized outcome still exists without an active pending-enactment delay
- `None` -> no active proposal and no retained finalized outcome remain on-chain

### Watch the deferral surface

If `ProposalAutoFinalizationDeferred` appears, consumers should not treat the proposal as finalized yet.
The proposal may still be active and need either later automatic retry or explicit `requeue_proposal_for_auto_finalization(...)`.

### Do not mistake this pallet for archival history

If UI or analytics need permanent proposal history, index events or maintain an external history store.
The pallet intentionally retains only recent finalized outcomes.

## Current Watchpoints

**1. Launch policy is intentionally narrow and frozen.**

Ordinary `Aye / Nay` applies ballot-time Declining Power to same-domain `Staking::stake_value(...)`. Protocol / network protection-track `Veto / Pass` applies the same curve to the `$VETO` asset. `$BLDR` protection applies it to locked `$NTVE` / `stNTVE` / `NTVE/stNTVE` LP-derived native `NativeVotePower`.

The policy includes a raw-supply immediate-cancellation gate and a raw `1%` veto dust floor before final protection can activate. It admits protection ballots until the configured close, retains bounded recent outcomes, and exposes a deliberately narrow admin recovery surface. Broader models remain future opt-ins, not hidden implementation debt.

**2. Epochs are block numbers today.**

The launch line runs the public ordinary timing policy directly: `ProposalLeadInPeriod = 3 days`, `ProposalVotingPeriod = 7 days`, `ProposalProtectionPeriod = 7 days`, and `ProposalEnactmentDelay = 3 days`. Urgent handling remains tightly scoped: only protocol `L1RootAction` is opted in, using unanimous raw protection-track `Pass` as the live acceleration path.

**3. Auto-finalization is bucket-bounded.**

This avoids global scans, but overloaded maturity epochs can defer and may need explicit requeue by admin.

**4. Finalized outcomes are recent history, not archive history.**

Consumers that need durable historical indexing should not rely on this pallet as permanent storage.

**5. Item identity is bounded, not eternal.**

`(domain, item_id)` is protected across active state and the live reward-memory horizon, but the pallet does not promise permanent archival uniqueness after bounded retention/expiry windows expire.

**6. Ballot cardinality is bounded.**

The pallet targets bounded runtime safety, not large open referendum sets with unbounded voter storage. Adding the protection track widens one proposal's bounded vote-set shape without removing cap discipline.

**7. The first protection-track slice is universal across today's proposals, but still narrow.**

TMCTOL treats the live proposal set as protected, so the separate dual-mode protection track remains universally available today. Richer class families and broader multi-track policy remain future work.

**8. Unified status is deliberately two-phase.**

`proposal_status(...)` returns `Active(Rejected { ... })` or `Active(VetoPassing { ... })` before finalization. Consumers should distinguish policy state from finalized state.

**9. GovXP identity layers beyond the counters-first v1 slice remain out of scope.**

The pallet ships only bounded GovXP input counters. Richer identity layers, any later bounded multiplier policy, delegation semantics, and soulbound reputation policy remain future work.

## Conclusion

`pallet-governance` has evolved from a narrow reward-memory helper into a bounded governance kernel with a real proposal lifecycle.
What makes the implementation strong is not breadth, but shape:

- It stays sparse
- It stays bounded
- It keeps policy in runtime wiring
- It gives staking a real governance-derived coefficient surface
- It exposes enough recent state for observability without becoming an archival subsystem

That is exactly the role this pallet is supposed to play inside TMCTOL's two-pallet reward architecture.

---

- `Last Updated`: April 2026
