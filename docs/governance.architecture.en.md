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

1. `First dual-mode protection-track slice exists, but it is still narrower than the target contract`
   the runtime now exposes a separate protection track with `Veto` plus protection-track `Pass`, supports one ordinary-track vote plus one protection-track vote per account on the same proposal, allows bounded replacement of the protection-track side for the same account/item, and keeps the path universal across today's protected proposal set; the backing protection surface is now domain-specific (`$VETO` asset for protocol / network governance, native `$NTVE` stake for `$BLDR` governance), requires raw `Veto` turnout to clear a `1%` dust floor before the final fail-closed gate can activate, and now simply keeps protection admissible until the configured protection-window close instead of relying on the older first-touch activation deadline; richer class families and broader track menus still remain future work

2. `The Declining Power slice is now closer to the target contract, and urgent handling now has one narrowly scoped live exception on the current line`
   ballot weight now follows the shipped piecewise `7x -> 1x` curve through a bounded vote-time epoch stored per ballot, and the pallet already separates ordinary-track and protection-track weighting windows internally so those clocks can diverge honestly later. The runtime now also exposes an explicit urgent-policy query surface per proposal so the expeditable contract is no longer implicit; on the current launch line that policy is still narrow rather than universal: only strategic `L1RootAction` proposals are expeditable, and they now require unanimous raw protection-track `Pass` (`100%` of eligible `$VETO` supply) before the fast path triggers. The current line also uses one tighter runtime-upgrade-only exception above the generic urgent path: once that unanimous protection condition is met, the pallet immediately resolves and executes the runtime-upgrade authorization path without waiting for a separate primary-track ballot. Outside that strategic runtime-upgrade exception, urgent handling still remains disabled until a later constitutional/runtime policy opts more `(domain, payload_kind)` pairs in. The remaining gap is no longer missing urgent mechanics; it is the still-narrow policy surface around them rather than the absence of an implementation path.

3. `Admin-heavy proposal control`
   signed users can cast votes, the browser now exposes both the bounded advisory submit path and the first minimal tactical treasury invoice submit path, and the current runtime now also opts tactical `$BLDR` `L2TreasurySpend` into signed submission alongside the earlier advisory combinations; manual resolution/rejection, requeue, and policy-aware early finalization otherwise remain `Root`-gated in the current runtime

4. `Pallet-side payload executor scaffold now exists, and the first executable runtime slices are live`
   the governance kernel now has a canonical bounded enactment scaffold: finalized proposals can either stay as approved-only outcomes, enter bounded pending-enactment buckets, finalize advisory payloads explicitly without dispatch, or record generic execution success / execution failure once a runtime payload executor is enabled. The current reference runtime now enables one bounded slice for each executable payload kind: `(a)` `L1RootAction` may dispatch preimage-backed `RuntimeCall::System(authorize_upgrade { code_hash })` under Root-equivalent authority in the strategic domain, `(b)` `L2ParameterChange` may now dispatch two narrow domain-local parameter paths by applying preimage-backed `RuntimeCall::AxialRouter(add_tracked_asset { asset })` and `RuntimeCall::AxialRouter(update_router_fee { new_fee })` through governance-owned internal setters instead of Root dispatch, and `(c)` tactical `L2TreasurySpend` now decodes a dedicated bounded invoice payload (`beneficiary`, `payout_asset`, `base_amount`, explicit funding source), reads the resolved winning primary option from bounded governance state, applies the on-chain invoice scalar, and executes the final transfer from the designated BLDR Treasury sovereign account rather than as Root. Launch-line `$BLDR` referenda are now not just conceptually invoice-centric but live invoice-shaped on the reference line, while changes to System AAA behavior still require `L2SignalToL1` or an explicit delegation of those control surfaces into the domain. A current scan of Root-only custom-pallet control surfaces also narrows the next truthful `L2ParameterChange` search space: TMC launch-physics mutation remains out of contract, staking onboarding/recovery/admin reward-bootstrap paths remain system-owned, AAA global breaker/actor-limit controls remain system-owned, and asset-registry registration/migration remains L1-owned. The remaining runtime gap is therefore narrower than a generic "more setters" wishlist: the next valid slice is either a genuinely delegated/domain-owned parameter surface or a richer treasury authority topology, not opportunistic reuse of unrelated Root setters.

5. `Runtime-upgrade execution now matches the intended post-bootstrap line closely`
   governance can now drive the first parachain-safe runtime-upgrade step through a dedicated bounded `L1RootAction` payload that carries only `code_hash` and dispatches `System::authorize_upgrade { code_hash }`, and the current line now also emits `ProposalRuntimeUpgradeAuthorized` as a runtime-upgrade-specific success event distinct from generic execution success. Pending relay of that authorized code is now exposed canonically through the bounded governance/runtime view `authorized_runtime_upgrade()` rather than requiring browser/product code to read raw `System.AuthorizedUpgrade` storage layout directly, and the browser governance surface uses that view so product UX can say when a governance-authorized upgrade is still awaiting `apply_authorized_upgrade { code }` while also stating that the browser does not expose that live write path. Integration coverage now also proves the paired system-level second step itself is already live after authorization: any origin may submit the matching code bytes to `System.apply_authorized_upgrade { code }`, and invalid authorized code clears the pending authorization with an explicit system rejection event instead of silently wedging state. The current line now also adds one constitutional acceleration rule on top of that path: unanimous raw protection-track `Pass` over the full eligible `$VETO` supply immediately resolves and executes the runtime-upgrade authorization step without waiting for a separate primary-track ballot, while the ordinary/urgent machinery for other payload kinds remains untouched. That means the current role split is now explicit and post-bootstrap: governance owns authorization of the code hash, while the later `apply_authorized_upgrade` call is only a transport/relay step for already-authorized bytes rather than a second governance decision. The repo now also ships `/scripts/check-authorized-upgrade-local.sh` as a plan-only operator verifier for local WASM vs pending authorized hash, `/scripts/apply-authorized-upgrade-local.sh` as the explicit relay submit surface, and the local dev bootstrap no longer depends on Sudo for the wallet/swap seeding surfaces that previously blocked this handoff.

6. `Invoice-family vote storage, primary-lane query shape, kernel-side resolution, and runtime activation now exist`
   outside the new protection path, the pallet no longer hardcodes storage-level primary voting to only `Aye / Nay`: the vote enum and bounded ballot storage can now also represent invoice-family `Amplify / Approve / Reduce / Nay`, family-aware cast validation rejects mismatched primary vote kinds, the runtime now also exposes a family-aware `proposal_primary_track_tally(domain, item_id)` query that reports deterministic lowest-scalar tie-breaking for the leading positive invoice option, the pallet kernel can now resolve invoice families into explicit `PassingAmplify / PassingApprove / PassingReduce` states instead of collapsing them back into binary approval, and the reference runtime now marks tactical `L2TreasurySpend` in the canonical `$BLDR` domain as `Invoice` so those semantics are live rather than latent

7. `The target public cadence is now shipped, while urgent policy remains narrow on the current line`
   the pallet has additive timing scaffolding, pending-enactment status handling, split ordinary-vs-protection weighting windows, a generic lead-in admission gate for ordinary ballots, bounded servicing for due pending-enactment buckets, and a generic urgent fast-track mechanism. The current runtime now configures those surfaces to the target public ordinary cadence: protection opens at submit, ordinary primary opens after a `3 day` lead-in, both ordinary protection and ordinary primary run for `7 days`, and successful ordinary approvals enter a default `3 day` enactment delay. The current urgent line now opts in exactly one strategic exception: protocol `L1RootAction` is expeditable and uses unanimous raw protection-track `Pass` as an immediate runtime-upgrade authorization path, while the rest of the launch line still remains deny-by-default. Confirm-period machinery remains disabled, and the remaining gap is therefore no longer public cadence policy but broader call-matrix expansion, richer execution observability, and any later urgent-policy opt-in rollout.

8. `Payload readiness is observable, but admission is still soft`
   the runtime already persists `payload_hash` metadata, exposes derived execution authority, and reports `proposal_payload_availability(domain, item_id)` through the canonical preimage provider, but it does not yet hard-enforce a payload-kind-specific admission/execution gate beyond that readiness scaffold. The target contract now says executable payload kinds may be submitted before the full preimage is noted as long as the canonical request/readiness surface stays honest, while actual enactment still requires runtime-visible payload availability.

9. `GovXP now matches the counters-first v1 contract`
   the runtime exports bounded GovXP input counters only; no separate live GovXP multiplier surface remains in the canonical runtime query contract, which keeps v1 aligned with the specification's counters-first / defer-multiplier policy

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
   - Current runtime returns `DecliningNativeStake` for `$BLDR`-domain protection-track `Veto / Pass`

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
- Reports the burned opening fee only for combinations that are actually `Signed`
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

| Storage                                                 | Role                                           | Notes                                                                                                                                                                  |
| :------------------------------------------------------ | :--------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `WinningVoteWindows[(domain, account)]`                 | Per-account rolling winning-memory tail        | Sparse, zero-sum evicted                                                                                                                                               |
| `ParticipationTotalsByAccount[(domain, account)]`       | Per-account cumulative participation totals    | Monotonic `total_participations` + `winning_participations`                                                                                                            |
| `ProposalAuthorshipTotalsByAccount[(domain, account)]`  | Per-account cumulative authorship totals       | Monotonic `authored_proposals` + `successful_authored_proposals`                                                                                                       |
| `WinningVoteResolutionWindows[domain]`                  | Resolution-once memory                         | Prevents duplicate item ingestion within live horizon                                                                                                                  |
| `ActiveProposals[(domain, item_id)]`                    | Live proposal registry                         | Stores `submitted_epoch`                                                                                                                                               |
| `ProposalAuthorsByItem[(domain, item_id)]`              | Live proposal proposer identity                | One explicit logical proposer / sponsor per active proposal                                                                                                            |
| `ProposalMetadataByItem[(domain, item_id)]`             | Additive proposal-meaning scaffold             | `CadenceMode + ProposalPayloadKind + payload_hash`                                                                                                                     |
| `ProposalVotesByItem[(domain, item_id)]`                | Bounded primary + protection-track ballot sets | One account, one stored vote per track family, plus `vote_epoch`; primary storage can now hold binary `Aye / Nay` or invoice-family `Amplify / Approve / Reduce / Nay` |
| `ProposalUrgentAuthorizedAt[(domain, item_id)]`         | Additive urgent-open timing scaffold           | Written exactly once when expeditable protection-track `Pass` crosses the configured raw threshold                                                                     |
| `ProposalPendingEnactmentAt[(domain, item_id)]`         | Additive enactment scheduling state            | Written on successful approval when enactment delay is positive                                                                                                        |
| `PendingEnactmentBuckets[epoch]`                        | Due enactment service index                    | Epoch-keyed bounded servicing for pending enactment attempts                                                                                                           |
| `ActiveProposalCounts[domain]`                          | Domain-local active cap tracking               | Enforces bounded active set                                                                                                                                            |
| `ActiveProposalIdsByDomain[domain]`                     | Bounded active proposal discovery              | Canonical live list for active item ids in one domain                                                                                                                  |
| `ProposalMaturityBuckets[epoch]`                        | Auto-finalization schedule                     | Epoch-keyed, no global active scan                                                                                                                                     |
| `FinalizedProposalOutcomes[(domain, item_id)]`          | Recent finalized result                        | Queryable but temporary                                                                                                                                                |
| `ProposalWinningPrimaryOptionByItem[(domain, item_id)]` | Retained resolved primary-side winner          | Preserves the already-selected `Aye / Nay / Amplify / Approve / Reduce` outcome for later enactment/execution                                                          |
| `FinalizedProposalOutcomeExpiryBuckets[epoch]`          | Finalized-outcome retention control            | Deletes recent history later                                                                                                                                           |
| `ExpiryBuckets[epoch]`                                  | Winning-vote expiry schedule                   | Touches only accounts whose rolling winner memory may decay                                                                                                            |
| `LastProcessedEpoch`                                    | on_initialize service cursor                   | Prevents repeated work                                                                                                                                                 |

`Migration state`:
The current storage version is `11`. Earlier upgrades translated legacy `ProposalVotesByItem` layouts first from the three-vector ballot shape into the four-vector `Veto / Pass` form, then into ballot records carrying `vote_epoch`; later additive slices backfilled `ActiveProposalIdsByDomain` from the live `ActiveProposals` set so bounded active-proposal discovery became part of the explicit on-chain contract, introduced explicit proposer storage plus cumulative proposal-authorship totals without retroactive authorship reconstruction for already-live legacy proposals, added timing-scaffold storage for urgent authorization / pending enactment, backfilled an epoch-keyed pending-enactment service index so due enactment attempts no longer require ad-hoc scans, widened primary ballot storage with empty default invoice-option buckets (`Amplify / Approve / Reduce`) so later invoice voting could evolve without another legacy vote-shape cliff, and now also add retained winning-primary-option storage so delayed enactment can still apply the already-resolved invoice scalar without trying to rediscover proposal outcome after ballot teardown.

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
- Burns the runtime-configured native opening fee before proposal creation
- Uses transactional semantics so duplicate/cap failures do not strand a burned fee
- Reuses the same bounded active-proposal insertion path and GovXP authorship accounting once admitted
- Emits `ProposalOpeningFeeBurned` when the opening fee is non-zero, then `ProposalSubmitted`

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

| Call Index | Extrinsic                                                                              | Role                                                                                 |
| :--------- | :------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------- |
| `0`        | `record_winning_vote(domain, item_id, account)`                                        | Low-level admin ingress                                                              |
| `1`        | `record_winning_vote_batch(domain, item_id, accounts)`                                 | Bounded admin batch ingress                                                          |
| `2`        | `submit_proposal(domain, item_id, proposer, cadence_mode, payload_kind, payload_hash)` | Admin create active proposal with explicit proposer and payload metadata             |
| `3`        | `submit_signed_proposal(domain, item_id, cadence_mode, payload_kind, payload_hash)`    | Signed create path for runtime-approved public combinations, with burned opening fee |
| `4`        | `resolve_proposal(domain, item_id, winners)`                                           | Manual bounded resolution                                                            |
| `5`        | `reject_proposal(domain, item_id)`                                                     | Manual rejection                                                                     |
| `6`        | `cast_vote(domain, item_id, vote)`                                                     | Signed ballot (`Aye`, `Nay`, `Amplify`, `Approve`, `Reduce`, `Veto`, `Pass`)         |
| `7`        | `resolve_proposal_from_votes(domain, item_id)`                                         | Policy-driven resolution after maturity                                              |
| `8`        | `requeue_proposal_for_auto_finalization(domain, item_id)`                              | Recovery if a deferred item needs re-scheduling                                      |
| `9`        | `force_resolve_proposal_from_votes(domain, item_id)`                                   | Policy-aware early finalization                                                      |

## Events and Errors

### Events that form the live operational surface

| Event                              | Why it matters                                                                                                                                                                                                               |
| :--------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ProposalOpeningFeeBurned`         | Confirms that a signed public submission actually paid and burned the configured opening fee before proposal admission                                                                                                       |
| `ProposalSubmitted`                | Confirms active proposal creation, explicit proposer identity, cadence/payload metadata, and current active-count pressure                                                                                                   |
| `ProposalVoteCast`                 | Confirms bounded ballot ingress, the recorded vote epoch, any protection-side replacement, and the current stored primary/protection vote counts across `Aye / Nay / Amplify / Approve / Reduce / Veto / Pass` as applicable |
| `ProposalUrgentAuthorized`         | Confirms that an expeditable proposal crossed the raw protection-track `Pass` threshold and records the authorization epoch plus the triggering raw pass/supply context                                                      |
| `ProposalResolved`                 | Confirms proposal closure and winner count credited into reward memory                                                                                                                                                       |
| `ProposalEnactmentScheduled`       | Confirms that approval moved into bounded pending-enactment servicing rather than stopping at a pure status marker                                                                                                           |
| `ProposalExecuted`                 | Confirms that the canonical enactment scaffold recorded successful executable-payload handling and names which payload kind succeeded                                                                                        |
| `ProposalRuntimeUpgradeAuthorized` | Confirms the current `L1RootAction` success slice with the concrete authorized code hash                                                                                                                                     |
| `ProposalParameterChangeExecuted`  | Confirms the current `L2ParameterChange` success slices with an explicit bounded parameter surface identity                                                                                                                  |
| `ProposalTreasurySpendExecuted`    | Confirms the current `L2TreasurySpend` success slice with funding source, beneficiary, payout asset, base amount, winning scalar, final payout amount, and settlement kind so scalar invoice transfer is explicit on-chain   |
| `ProposalExecutionFailed`          | Confirms that bounded enactment attempted execution but the payload did not enact successfully, and names which payload kind failed plus the bounded failure-reason category                                                 |
| `ProposalAdvisoryFinalized`        | Confirms explicit non-dispatch finalization for advisory payload kinds and names whether the outcome was `Intent` or `L2SignalToL1`                                                                                          |
| `ProposalRejected`                 | Confirms proposal closure and explicit rejection reason                                                                                                                                                                      |
| `ProposalVetoCancelled`            | Confirms that the separate protection track cancelled the proposal without reward credit                                                                                                                                     |
| `ProposalAutoFinalizationDeferred` | Signals that maturity servicing did not finish cleanly and requeue may be needed                                                                                                                                             |
| `ProposalAutoFinalizationRequeued` | Signals manual recovery of deferred maturity scheduling                                                                                                                                                                      |
| `WinningVoteRecorded`              | Confirms reward-memory credit for one winner account                                                                                                                                                                         |
| `WinningVoteWindowEvicted`         | Confirms zero-sum reward-memory eviction after expiry                                                                                                                                                                        |

### Errors that expose real runtime boundaries

| Error                                                                               | Meaning                                                                                                                                                              |
| :---------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ProposalAlreadyActive` / `ProposalNotActive`                                       | Active-state identity guard for the current bounded lifecycle                                                                                                        |
| `ProposalVotingWindowStillOpen`                                                     | Vote-derived resolution was attempted before maturity on the normal path                                                                                             |
| `ProposalVoteAlreadyCast` / `ProposalVoteSetFull` / `ProposalProtectionTrackClosed` | Ballot storage is bounded, one-account-one-vote per track family applies, and protection-track ballots are rejected once the configured protection window has closed |
| `ProposalWinnerSetEmpty`                                                            | Manual resolution cannot inject an empty winner set                                                                                                                  |
| `ActiveProposalCapReached`                                                          | Domain-local active proposal budget is exhausted                                                                                                                     |
| `ProposalMaturityBucketFull`                                                        | Auto-finalization scheduling cap for one epoch was hit                                                                                                               |
| `DuplicateWinningVoteItem` / `DuplicateWinningVoteResolutionItem`                   | Winner ingress uniqueness was violated inside the live memory horizon                                                                                                |
| `FinalizedProposalOutcomeExpiryBucketFull`                                          | Recent finalized-outcome retention hit its bounded expiry scheduling cap                                                                                             |
| `ExpiryBucketFull`                                                                  | Account-expiry scheduling hit its bounded service bucket cap                                                                                                         |

## Runtime Binding

Current runtime wiring in `template/runtime/src/configs/governance_config.rs`:

- `AdminOrigin = Root`
- `EpochProvider = System::block_number()`
- `ProposalVoteWeightProvider = RuntimeProposalVoteWeightProvider`
- `ProposalTrackPowerProfileProvider = RuntimeProposalTrackPowerProfileProvider`
- `VetoVotePowerProvider = RuntimeVetoVotePowerProvider`
- `WeightInfo = runtime weight bridge`

Current runtime policy values:

| Constant                                  | Value                                                                                                                                                                                  |
| :---------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `WinningVoteLookbackEpochs`               | `3`                                                                                                                                                                                    |
| `MaxWinningVotesPerEpoch`                 | `4`                                                                                                                                                                                    |
| `MaxWinningVoteItemsPerEpoch`             | `4`                                                                                                                                                                                    |
| `MaxWinningVoteResolutionItemsPerEpoch`   | `64`                                                                                                                                                                                   |
| `MaxWinningVoteAccountsPerCall`           | `256`                                                                                                                                                                                  |
| `MaxActiveProposalsPerDomain`             | `128`                                                                                                                                                                                  |
| `MaxMaturingProposalsPerEpoch`            | `4`                                                                                                                                                                                    |
| `ProposalVotingPeriod`                    | `7 days` (`7 * 24 * HOURS = 100,800` blocks)                                                                                                                                           |
| `ProposalLeadInPeriod`                    | `3 days` (`3 * 24 * HOURS = 43,200` blocks)                                                                                                                                            |
| `ProposalProtectionPeriod`                | `7 days` (`7 * 24 * HOURS = 100,800` blocks)                                                                                                                                           |
| `ProposalUrgentVotingPeriod`              | `1 day` (`24 * HOURS = 14,400` blocks)                                                                                                                                                 |
| `ProposalEnactmentDelay`                  | `3 days` (`3 * 24 * HOURS = 43,200` blocks)                                                                                                                                            |
| `ProposalFastTrackPassThreshold`          | `100%` of eligible protection supply on the current strategic runtime-upgrade line, so unanimous raw protection-track `Pass` is required before the urgent authorization path triggers |
| `ProposalApprovalThreshold`               | `60%`                                                                                                                                                                                  |
| `ProposalVetoThreshold`                   | `50%` of eligible protection supply (strict `>` for immediate cancellation)                                                                                                            |
| `ProposalVetoMinimumVetoTurnout`          | `1%` of eligible protection supply (raw veto dust floor)                                                                                                                               |
| `ProposalMinimumTurnout`                  | `200` weighted units                                                                                                                                                                   |
| `FinalizedProposalOutcomeRetentionEpochs` | `16`                                                                                                                                                                                   |
| `MaxFinalizedProposalOutcomesPerEpoch`    | `1024`                                                                                                                                                                                 |
| `MaxExpiringAccountsPerEpoch`             | `1024`                                                                                                                                                                                 |

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

Immediate-threshold cancellation still compares raw live protection weight against total eligible protection supply:

```text
protocol / network governance => Assets::total_issuance(VETO_ASSET_ID)
$BLDR tactical governance => Staking::pool(native_asset_id).accounted_balance
```

using a strict `>` comparison against the runtime threshold, while the end-of-window protection gate first requires raw `Veto` turnout to reach the runtime `1%` dust floor and then compares decline-weighted `Veto` vs decline-weighted `Pass` tallies.

In `runtime-benchmarks` builds, the ordinary provider deliberately falls back to equal weight `1`, while the protection-power provider now falls back to `1 / 1` vote-weight-plus-total-issuance so benchmarking can exercise the immediate-cancellation worst case deterministically.

This is an important example of the project's `Runtime-as-Config` discipline: the pallet does not hardcode one-account-one-vote, does not hardcode a staking formula for ordinary ballots, and keeps both the ordinary and protection vote-power surfaces runtime-wired rather than baking asset lookup or temporal policy directly into pallet logic. The current runtime now centralizes domain hierarchy, profile identity, and weight behavior through one typed `GovernanceDomainPolicy` declaration plus shared helper consumers in `runtime/src/configs/governance_config.rs`, so protocol `$NTVE + $VETO` governance and canonical tactical `$BLDR + $NTVE` governance stay aligned between tally logic, query identity, and exported domain policy.

The current public surface is intentionally narrow: `governance_domain_policy(domain)` exposes the launch-line ordinary/protection power profiles, but it does not yet attempt to encode richer future class families, execution authorities, or broader constitutional topology beyond the current bounded query contract.

## Current Launch Policy

For the current launch line, the runtime policy is now intentionally frozen to the simplest bounded rule set that already exists in code:

1. `Ballot-time Declining Power`
   normal runtime builds now use the shipped piecewise `7x -> 1x` Declining Power curve across both ordinary and protection-track ballots: ordinary `Aye / Nay` derive their base from same-domain `Staking::stake_value(domain, account)`, while protection-track `Veto / Pass` derive their base from the runtime-declared protection surface for that domain

2. `Domain-scoped hierarchy`
   protocol / network governance currently runs as `$NTVE` primary + `$VETO` protection, while `$BLDR` tactical governance currently runs as `$BLDR` primary + `$NTVE` protection; both use the same bounded `Veto / Pass` cancellation lane, but with different base-weight surfaces

3. `Protection-track cancellation`
   the first live protection slice is now enabled through domain-specific backing: the runtime creates the well-known `$VETO` asset class at genesis with deterministic metadata and an Asset Registry-owned admin surface for protocol governance, uses native staking weight for `$BLDR` governance, lets one account vote once in the ordinary track and once in the protection track on the same item, reprices later protection-track replacements to the later ballot epoch, keeps protection-track ballots admissible until the configured protection close, cancels immediately only when raw live protection weight is **strictly greater** than the runtime threshold against total eligible protection supply, ignores raw `Veto` turnout below `1%` of that supply as dust protection, and otherwise stays fail-closed unless decline-weighted `Pass` strictly outweighs decline-weighted `Veto`; veto-cancelled items do not credit governance reward memory

4. `Override and recovery`
   admin control stays intentionally narrow: `reject_proposal(...)`, policy-aware `force_resolve_proposal_from_votes(...)`, and `requeue_proposal_for_auto_finalization(...)` are the recovery/override surface, with no broader arbitrary override contract added on top

5. `Finalized history`
   bounded finalized-outcome retention is treated as sufficient for the kernel pallet; durable archival history belongs to events, indexers, or a future dedicated history surface rather than unbounded in-kernel storage growth

Richer vote-power formulas, broader emergency policy, or permanent on-chain history remain future opt-in choices, not unresolved debt in the current launch baseline.

## Query and Computation Semantics

### Tally and resolution are derived, not cached

The current pallet does not keep a precomputed weighted tally per proposal.
Instead:

- `ProposalVotesByItem[(domain, item_id)]` now stores bounded ballot sets for primary-track `Aye`, `Nay`, `Amplify`, `Approve`, `Reduce` plus protection-track `Veto`, `Pass`, with each ballot carrying both the account and its vote-time epoch
- `proposal_vote_tally(...)` re-weights ordinary ballots through `ProposalVoteWeightProvider` and protection-track ballots through `VetoVotePowerProvider`, and those two paths now already consume separate internal timing windows even though the current launch-line defaults still make them coincide
- `cast_vote(...)` now already enforces the generic rule that ordinary ballots cannot enter before `primary_open`, while protection-track ballots remain admissible during any configured lead-in
- `proposal_resolution_state(...)` first checks whether raw live protection-majority already triggers the separate immediate threshold, then after maturity requires raw `Veto` turnout to clear the `1%` dust floor before applying the decline-weighted protection-track `Veto` vs `Pass` gate, and only then derives primary passing/rejected state from the family-aware primary tally: binary families still use weighted `Aye / Nay`, while invoice families now use `weighted_positive` vs `weighted_nay` plus deterministic lowest-scalar tie-breaking across `Amplify / Approve / Reduce`; the current launch-line runtime still reports only `Binary`, so those invoice resolution branches remain kernel-ready rather than live policy on the reference line
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

These are the authoritative bounded surfaces for live proposal detail, proposal meaning, submission authority, public opening-fee cost, payload readiness, primary-track family identity, primary-lane leader/tally interpretation, retained winning-primary-option identity, urgent-policy eligibility, live tally/status interpretation, additive timing/enactment scaffolding, recent execution detail, staking reward-memory export, and GovXP input observability. The helper/query contract is no longer merely internal pallet convenience: the runtime now exports these bounded projections through canonical governance view functions, while raw storage remains an implementation detail except where explicitly named as a stable discovery surface.

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
3. `/scripts/check-authorized-upgrade-local.sh` compares a local WASM blob against that authorized hash and now classifies the result as `awaiting-governance-authorization`, `authorized-hash-mismatch`, or `ready-to-relay-code`
4. once the phase reaches `ready-to-relay-code`, `/scripts/apply-authorized-upgrade-local.sh` provides the dedicated operator-facing submit surface for the external relay step and stays plan-only unless `--submit` is passed explicitly
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

1. `Launch policy is intentionally narrow and frozen`
   the runtime currently uses ballot-time Declining Power on top of same-domain `Staking::stake_value(...)` for ordinary `Aye / Nay`, the same ballot-time Declining Power on top of the `$VETO` protocol asset for protocol / network protection-track `Veto / Pass`, the same ballot-time Declining Power on top of native `$NTVE` stake for `$BLDR` protection-track `Veto / Pass`, a raw-supply immediate-cancellation gate plus a raw `1%` veto dust floor before the final protection gate can activate, protection-track admission until the configured protection close, bounded recent finalized-outcome retention, and a narrow admin recovery surface by design; broader governance models remain future opt-in, not hidden implementation debt

2. `Epochs are block numbers today`
   the current launch line now runs the public ordinary timing policy directly (`ProposalLeadInPeriod = 3 days`, `ProposalVotingPeriod = 7 days`, `ProposalProtectionPeriod = 7 days`, `ProposalEnactmentDelay = 3 days`), while urgent handling is still tightly scoped: only protocol `L1RootAction` is opted in, and it now uses unanimous raw protection-track `Pass` as the live acceleration path

3. `Auto-finalization is bucket-bounded`
   this avoids global scans, but overloaded maturity epochs can defer and may need explicit requeue by admin

4. `Finalized outcomes are recent history, not archive history`
   consumers that need durable historical indexing should not rely on this pallet as permanent storage

5. `Item identity is bounded, not eternal`
   `(domain, item_id)` is protected across active state and the live reward-memory horizon, but the pallet does not promise permanent archival uniqueness after bounded retention/expiry windows are gone

6. `Ballot cardinality is bounded`
   the current pallet is built for bounded runtime safety, not large open referendum sets with unbounded voter storage; adding the protection track widens one proposal's bounded vote-set shape, but does not remove the cap discipline

7. `The first protection-track slice is universal across today's proposals, but still narrow`
   TMCTOL currently treats the live proposal set as protected, so the separate dual-mode protection track remains universally available today; richer class families and broader multi-track policy still remain future work

8. `Unified status is deliberately two-phase`
   `proposal_status(...)` returns `Active(Rejected { ... })` or `Active(VetoPassing { ... })` before finalization; consumers should distinguish policy state from finalized state

9. `GovXP identity layers beyond the counters-first v1 slice are still out of scope`
   the pallet now ships bounded GovXP input counters only, while richer identity layers, any later bounded multiplier policy, delegation semantics, and soulbound reputation policy still remain future work

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

- `Version`: 0.1.0
- `Last Updated`: April 2026
- `Author`: LLB Lab
- `License`: MIT
