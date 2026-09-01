# Governance: Bounded Participation and Proposal Lifecycle Architecture

> Contract layer: [`specification.en.md`](./specification.en.md)
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

`pallet-governance` is the live bounded governance kernel in the current DEOS reference runtime. It exports governance-owned participation memory without calling into or scheduling downstream reward settlement.
It serves two coupled but distinct roles:

1. `participation-memory kernel`
   It tracks bounded rolling winning-vote memory per `(domain, account)`, cumulative participation plus proposal-authorship totals for GovXP inputs, and exports a normalized `governance_participation_coefficient(domain, account)` plus raw GovXP counter inputs

2. `proposal lifecycle kernel`
   It provides a bounded active proposal, ballot, resolution, rejection, auto-finalization, and recent finalized-outcome surface

The pallet is intentionally not a maximal governance platform.
It is a bounded runtime component whose main architectural job is to turn governance outcomes into sparse, queryable participation memory without unbounded storage growth.

## Architecture Overview

### Design Principles

1. `Participation coefficient stays winning-memory-first`
   The exported governance participation coefficient is based on counted winning outcomes rather than raw vote count or mere turnout, while monotonic participation/authorship totals remain separate GovXP inputs

2. `Bounded everywhere`
   live memory, per-epoch item sets, per-call account sets, active proposals, maturity buckets, finalized-outcome retention, and expiry servicing are all bounded

3. `Runtime-as-Config`
   lookback width, per-epoch caps, vote weight, voting period, threshold, turnout floor, and retention are runtime policy, not pallet constants

4. `Proposal lifecycle as participation ingress`
   proposal handling feeds bounded participation memory through a real lifecycle rather than leaving outcome accounting at manual admin injection forever

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

    Memory --> Coeff[governance_participation_coefficient(domain, account)]
    Coeff --> Runtime[Runtime adapter reads at snapshot boundary]

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
The implementation constructs the exact configured slot width without fallible pushes, rotates the ring forward on access, and clears expired slots as epochs move. Stored windows must retain exactly `WinningVoteLookbackEpochs` slots: mutation returns `RewardWindowInvariant` on disagreement, read projections return zero reward power, and expiry service leaves malformed state untouched rather than panicking. Rolling, participation, winning, authored, and successful-authorship increments are checked and return `RewardCounterOverflow`; transactional proposal and resolution ingress restores prior windows, counters, custody, and proposal state on a late counter failure. Runtime integrity requires the configured maximum rolling count and retained finalized-outcome capacity to fit their stored widths.

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
   - Newest-first records carrying explicit `ProposalIdentity { domain, item_id }` plus one canonical `FinalizedProposalRecord`

3. `proposal_vote_tally(domain, item_id)`
   - Voter counts
   - Weighted primary-track `Aye / Nay / Amplify / Approve / Reduce`
   - Weighted protection-track `Veto / Pass`
   - Total primary turnout plus total protection turnout

4. `proposal_vote_power_profile(domain, item_id, vote_kind)`
   - Runtime-declared power-profile identity for the live track family behind that vote kind
   - Current runtime returns `DecliningDirectStake` for ordinary `Aye / Nay`
   - Current runtime returns `DecliningVetoAsset` for protocol / network protection-track `Veto / Pass`
   - Current runtime returns `DecliningNativeStake` for `$BLDR`-domain protection-track `Veto / Pass`, backed by locked `$NTVE`, locked `stNTVE`, and locked `$NTVE/stNTVE` LP-derived `NativeVotePower`

5. `proposal_resolution_state(domain, item_id)`
   - `VotingWindowOpen { current_epoch, maturity_epoch }`
   - `PassingAye`
   - `PassingAmplify / PassingApprove / PassingReduce` when family policy enables invoice-style primary resolution
   - `PassingNay`
   - `Rejected { reason }`

6. `proposal_status(domain, item_id)`
   - `Active(ProposalResolutionState)` while proposal storage still exists
   - `PendingEnactment { approval: ProposalApproval { approved_epoch, winner_count }, enactment_epoch }` after successful finalization when a positive enactment delay is scheduled and not yet elapsed; the shape cannot nest a rejected or already executed outcome
   - `Finalized(FinalizedProposalOutcome)` once proposal state is gone and either no enactment delay exists or the pending window has elapsed; approved outcomes reuse one `ProposalApproval` and one typed enactment result

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

10. `proposal_admission_policy_view(domain, payload_kind)`

- Canonical bounded classifier projection over one `(domain, payload_kind)` pair
- Reports authority, `General | Strategic` capacity lane, domain lane limit, per-author limit, signed-preimage requirement, and opening fee from the same policy consumed by dispatch
- Protocol / Native `L1RootAction` and `Intent` are `PrimaryEligibleSigned`; non-protocol `Intent` plus tactical `$BLDR` `L2SignalToL1` and `L2TreasurySpend` are `Signed`; remaining combinations are `AdminOnly`
- Opening fees apply only to signed combinations and provide economic friction without replacing structural domain, reserve, author, bucket, ballot, or service bounds

11. `proposal_payload_availability(domain, item_id)`

- Additive payload-readiness scaffold over one item
- Reports whether the stored `payload_hash` currently has a registered preimage and whether that preimage is requested in the canonical runtime preimage subsystem

12. `payload_hash_preimage_status(payload_hash)`

- Additive preimage-status scaffold over one payload hash before proposal submission exists
- Reports whether that exact hash already has a noted preimage, whether it is requested but not yet noted, and the noted payload length when available, so browser-side advisory composition can stay on canonical governance query surfaces instead of reading raw preimage storage layout directly

13. `payload_preimage_note_cost(payload_len)`

- Additive bounded preimage-note cost scaffold over one byte length
- Reports the current runtime's generic preimage note deposit for that payload length so browser-side signed advisory composition can quote the optional `Preimage.note_preimage` path honestly without hardcoding runtime pricing constants

14. `payload_admission_witness(payload_hash, (domain, payload_kind))`

- Reports the compact typed evidence produced from the exact noted bytes by the bounded host validator
- Binds encoded length, semantic domain/kind, derived execution authority, schema version, and runtime-spec compatibility without exposing or rereading the payload bytes

15. `proposal_primary_track_family(domain, item_id)`

- Additive primary-track contract scaffold over one proposal item
- Reports whether the current runtime treats that proposal's primary lane as `Binary` or `Invoice`
- The current reference runtime now returns `Invoice` for tactical `L2TreasurySpend` in the canonical `$BLDR` domain and `Binary` elsewhere on the current launch line

`proposal_primary_track_tally(domain, item_id)` is the companion family-aware primary-lane summary.
For binary families it reports `Aye / Nay` weights plus the current leading side.
For invoice families it reports `Amplify / Approve / Reduce / Nay` weights, aggregate positive weight, and deterministic lowest-scalar tie-breaking for the current leading positive option.

`retained_proposal_winning_primary_option(domain, item_id)` is the retained finalized-outcome companion.
It reports the already-selected winning primary option (`Aye / Nay / Amplify / Approve / Reduce`) while bounded finalized retention still exists, so delayed enactment, clients, and audits do not need to reconstruct winner identity from internal executor paths or raw tallies alone.

15. `proposal_timing(domain, item_id)`

- Additive timing scaffold over one active proposal
- Submitted epoch, protection open/close, ordinary primary open/close
- Optional urgent-open override and optional pending-enactment epoch

16. `proposal_urgent_eligibility(domain, item_id)`

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
- `PayloadAdmissionWitnesses[(payload_hash, (domain, payload_kind))]`: compact typed evidence used by signed admission; physical identity permits one hash to be validated independently for distinct semantic domain/kind pairs without overwriting another pair
- `ProposalVotesByItem[(domain, item_id)]`: bounded primary/protection ballot sets with frozen weight/raw power
- `GovernanceLocks[account]`: aggregate `lock_until` extended to the maximum touched enactment horizon
- `VotePowerCustodyByAccount[(account, lock_id)]`: one aggregate transferable source position containing its locked amount and maximum ballot horizon
- `ProposalUrgentAuthorizedAt[(domain, item_id)]`: written once when expeditable `Pass` crosses raw threshold
- `ProposalPendingEnactmentAt[(domain, item_id)]`: approval scheduling state when enactment delay is positive
- `PendingEnactmentBuckets[epoch]`: epoch-keyed bounded servicing for pending enactment attempts
- `ActiveProposalIdsByDomain[domain]`: bounded canonical active enumeration, domain-local cap/cardinality owner, and terminal-membership corruption boundary
- `ActiveProposalIdsByDomain[domain]`: canonical bounded live list for active item ids in one domain; the same cleanup removes the id and author once before resolution, rejection, or veto cancellation commits
- `ProposalMaturityBuckets[epoch]`: epoch-keyed auto-finalization schedule, no global active scan
- `FinalizedProposals[(domain, item_id)]`: queryable temporary `FinalizedProposalRecord` owning the lifecycle outcome and optional typed execution success/failure detail together
- `ProposalWinningPrimaryOptionByItem[(domain, item_id)]`: retained resolved primary-side winner for enactment
- `FinalizedProposalOutcomeExpiryBuckets[epoch]`: finalized-outcome retention control
- `ExpiryBuckets[epoch]`: winning-vote expiry schedule for accounts whose memory may decay
- `LastProcessedEpoch`: `on_initialize` service cursor preventing repeated work

Epoch values are admitted through exact `Epoch <-> u32` round trips; voting-window rotation rejects unrepresentable epochs and uses saturation only for the explicit pre-genesis expiry floor. Proposal, confirmation, enactment, retention, and reward-expiry deadlines use checked addition. Unrepresentable horizons return `EpochArithmeticOverflow`; terminal rescheduling does not clamp to `u32::MAX` or fabricate a later epoch.

`MaxEpochCatchUpPerBlock = 1` advances through empty epochs chronologically, while `CurrentEpochServicePhase` serializes maturity, pending enactment, finalized-outcome expiry, and reward expiry for the first owned epoch. Each nonempty phase processes only its configured per-block item cap, retains the ordered suffix under the same epoch key, and prevents `LastProcessedEpoch` from advancing until all four families drain. Generated production Weight separately measures the phase/base path and each bounded item branch; runtime evidence proves every admitted branch fits the block in RefTime and ProofSize.

`Migration state`:
The current fresh-genesis storage baseline is `4`. In addition to the active ballot and account-lock topology, `PayloadAdmissionWitnesses` stores compact typed admission evidence under payload hash plus domain/kind identity; this repository ships no bridge from baseline `3`, and a downstream live lineage must own an explicit bounded migration before adopting this layout.

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

### 2. Governance participation coefficient calculation

`governance_participation_coefficient(domain, account)`:

- Reads the account's winning-vote window
- Rotates it to the current epoch
- Returns `rolling_sum / (lookback * max_votes_per_epoch)` as `FixedU128`
- Returns zero if the window is absent or empty

So the coefficient is normalized against the runtime's own configured capacity, not against an unbounded historical total.

### 3. Proposal submission and voting

`submit_proposal(domain, item_id, proposer, cadence_mode, payload_kind, payload_hash)`:

- Admin-only explicit submit path
- Reuses the single admission classifier in admin mode, including duplicate, capacity-lane, author-index integrity, and per-author bounds
- Checks duplicate active proposal for the same item
- Derives the capacity lane from the existing `(domain, payload_kind)` submission-authority binding
- General proposals stop at `MaxActiveProposalsPerDomain - StrategicProposalReserve`; protocol `L1RootAction` may consume the complete domain cap
- Uses the existing domain count and active-id list; no strategic count, second collection, or caller-selected priority exists
- Scans the bounded active-id list and canonical author records to enforce `MaxActiveProposalsPerAuthor` per domain without another count cache
- Fails closed when an indexed active proposal lacks its canonical author record
- Records `submitted_epoch`
- Records one explicit logical `proposer` / sponsor for that active item
- Records additive proposal metadata (`CadenceMode + ProposalPayloadKind + payload_hash`)
- Increments the proposer's cumulative authored-proposal GovXP counter
- Computes `maturity_epoch = submitted_epoch + ProposalLeadInPeriod + ProposalVotingPeriod`
- Inserts a maturity touch into `ProposalMaturityBuckets[maturity_epoch]`
- Emits `ProposalSubmitted`

`submit_signed_proposal(domain, item_id, cadence_mode, payload_kind, payload_hash)`:

- Signed submit path for runtime-approved public combinations only
- Reuses the single admission classifier also consumed by bounded authority/fee views
- Applies rejection precedence as authority, required eligibility, compact preimage status/witness compatibility, duplicate, domain capacity, author-index integrity, author capacity, and exact maturity-bucket capacity before fee transfer
- Admission computes and retains the maturity epoch; insertion uses that admitted value and transactionally writes the same prechecked bucket after fee collection
- Reads only compact preimage availability/length status plus `PayloadAdmissionWitnesses`; the generic `PreimageFor` value is absent from this call's generated storage proof
- Requires exact witness agreement on hash-keyed semantic identity, byte length, execution authority, schema, and runtime-spec compatibility
- Direct host-executor invocation with an advisory kind returns typed `UnsupportedCall`; missing tactical treasury configuration returns `DispatchFailed`; bounded winner projection returns `ProposalVoteSetFull` on disagreement rather than relying on panic-only assumptions
- `PrimaryEligibleSigned` additionally calls the host eligibility provider before any fee or proposal mutation
- The DEOS provider admits protocol / Native `L1RootAction` and `Intent` only when the signer has nonzero direct primary-track staking power; `$VETO` protection balance never enters that predicate
- Derives proposer identity from the signer rather than an admin-supplied sponsor field
- Transfers the runtime-configured native opening fee into Fee Sink before proposal creation
- Uses transactional semantics so an unexpected late insertion failure rolls back the fee, while every declared admission failure including full maturity capacity happens before fee transfer
- Reuses the same bounded active-proposal insertion path and GovXP authorship accounting once admitted
- Emits `ProposalOpeningFeeCollected` when the opening fee is non-zero, then `ProposalSubmitted`

`prepare_payload_admission_witness(domain, payload_kind, payload_hash, payload)`:

- Signed preparation boundary accepting at most 262 payload bytes, requiring their runtime hash and length to match compact current preimage status, and asking the host runtime for one complete typed result; callers never supply witness fields
- Enforces kind-specific encoded ceilings: exact fixed typed bounds for Root upgrade and tactical invoice payloads, the six-byte Router-fee call encoding, and 262 bytes for the maximum advisory tuple
- Rejects missing, oversized, malformed, trailing-byte, domain/kind-incompatible, authority-incompatible, or schema-incompatible bytes before writing
- Reserves one runtime-configured nonzero native storage deposit from the preparer, stores the depositor and deposit with the derived compact witness under `(payload_hash, (domain, payload_kind))`, and emits `ProposalPayloadAdmissionWitnessPrepared`; a failed refresh preserves the prior witness, reserve, and events exactly
- Successful signed submission consumes the exact witness and releases its deposit in the proposal transaction; abandoned witnesses retain their reserve even if the generic preimage is unnoted, bounding persistent state by funded liability
- Reads no generic preimage value, so both witness preparation and later signed submission remain Normal-class dispatchable without increasing block limits; enactment separately reads the exact stored bytes selected by proposal hash

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
- Ballot insertion, governance-lock extension, GovXP participation, urgent authorization, and immediate terminal resolution share one transaction; terminal failure restores exact pre-vote state
- Ballot, primary-turnout, and protection-turnout sums use checked `u64` accumulation; `ProposalVoteTallyOverflow` rolls back vote ingress and makes tally/status projections unavailable rather than saturating a malformed result
- Invoice positive-weight policy widens the three-option sum into `u128`, while its bounded read projection uses checked `u64` addition and becomes unavailable on malformed overflow; boundary tests distinguish the exact `u64::MAX + 1` majority from a saturated tie
- Veto and fast-track threshold cross-products widen every `u64` weight by the exact Perbill denominator in `u128`, including `u64::MAX`, so boundary comparison cannot overflow or round through a narrowed ratio
- The later maturity-time protection gate only becomes active once raw `Veto` turnout reaches the runtime dust floor against that same protection supply
- Emits `ProposalVoteCast`

Resolution, rejection, and veto cancellation all use one transactional active-state cleanup. Approval releases domain and per-author admission capacity before optional pending enactment, so later enactment success or execution failure does not own or release active capacity again.

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

- Delete `FinalizedProposals[(domain, item_id)]` together with its optional execution detail

This enforces bounded recent-history retention.

## Finalized Outcome Retention

Finalized outcomes are recorded from both final paths:

- `resolve_active_proposal(...)` records `Approved { approval: ProposalApproval, enactment: NotAttempted }`
- `reject_active_proposal(...)` records `Rejected { finalized_epoch, reason }`
- `veto_cancel_active_proposal(...)` records `VetoCancelled { finalized_epoch, veto_weight }`

Approved enactment updates only the nested result while preserving the same approval fact:

- `Enacted { epoch }`
- `ExecutionFailed { epoch }`
- `AdvisoryFinalized { epoch }`

`FinalizedProposalRecord.execution_detail` carries only typed success or failure detail. Payload kind, authority, approval epoch, and terminal epoch remain owned by metadata, derived policy, and the finalized outcome rather than being repeated in the detail.

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

3. `FinalizedProposals[(domain, item_id)]`
   retains recent finalized status and typed execution detail under one bounded owner

So the real implementation contract is:

> `(domain, item_id)` is unique while it is live or still economically relevant, not forever as chain-archival identity

That means consumers should not assume this pallet alone provides eternal governance history.
If long-lived archival identity is needed, indexers, events, or a future dedicated history surface must carry that burden.

The related state distinction is also important:

- `proposal_resolution_state(...)` describes the current policy result of an **active** proposal
- `proposal_status(...)` returns active state first and only falls back to retained finalized outcome once active proposal storage is gone
- `FinalizedProposals` describes a concluded proposal, but only while retention has not yet expired

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
| `10` | `unlock_vote_power` | signed release of one matured transferable source position |
| `11` | `prepare_payload_admission_witness` | bounded typed preimage validation and compact witness preparation |

## Events and Errors

### Events that form the live operational surface

- `ProposalOpeningFeeCollected`: signed public submission paid the opening fee into Fee Sink
- `ProposalPayloadAdmissionWitnessPrepared`: exact hash/domain/kind witness was derived from currently available typed bytes
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
- `ProposalPreimageWitnessMissing` / `ProposalPreimageWitnessStale`: signed admission lacks current compact evidence or its length/authority/compatibility no longer matches
- `DuplicateWinningVoteItem` / `DuplicateWinningVoteResolutionItem`: live memory uniqueness violation
- `RewardWindowInvariant`: persisted reward-memory width disagrees with the configured lookback
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
- `StrategicProposalReserve = 1`
- `MaxActiveProposalsPerAuthor = 16`
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
- `MaxEpochCatchUpPerBlock = 1`
- `MaxMaturingProposalsPerBlock = 2`
- `MaxPendingEnactmentsPerBlock = 4`
- `MaxFinalizedProposalOutcomesPerBlock = 1024`
- `MaxExpiringAccountsPerBlock = 512`
- `FinalizedProposalOutcomeRetentionEpochs = 16`
- `MaxFinalizedProposalOutcomesPerEpoch = 1024`
- `MaxExpiringAccountsPerEpoch = 1024`

The two-proposal maturity bound keeps the generated worst-case hook ProofSize compatible with the runtime fixed envelope while preserving bounded continuation across later blocks; increasing it requires recomputing `FixedBlockWeight` and maximum signed-call fit.

Ordinary and invoice-family resolution consume exactly the fixed `ProposalApprovalThreshold` and `ProposalMinimumTurnout`. No adaptive ceiling, progress, or decay policy is reconstructed in views or resolution.

### Vote weight providers

For ordinary `Aye / Nay` voting in normal runtime builds, base balance is still:

```text
Staking::stake_value(domain, account)
```

and the runtime provider transforms that base through the shipped piecewise ballot-time Declining Power curve using bounded proposal-time context (`item_id`, `current_epoch`, `submitted_epoch`, `maturity_epoch`, `vote_epoch`), clamping the final result to `u32`.

For the live protection track, the base surface is domain-specific:

```text
protocol / network governance => Assets::balance(VETO_ASSET_ID, account)
$BLDR tactical governance => Staking::stake_value(NTVE_ASSET_ID, account)
```

and the runtime applies the same shipped piecewise ballot-time Declining Power rule to the stored protection-track ballot epoch in both cases.

Immediate-threshold cancellation compares frozen raw protection power from recorded protection ballots against total eligible protection supply:

```text
protocol / network governance => Assets::total_issuance(VETO_ASSET_ID)
$BLDR tactical governance => Staking::pool(native_asset_id).accounted_balance
```

using a strict `>` comparison against the runtime threshold, while the end-of-window protection gate first requires raw `Veto` turnout to reach the runtime `1%` dust floor and then compares decline-weighted `Veto` vs decline-weighted `Pass` tallies.

The runtime stores protection ballots as `u64` while canonical balances are `u128`. Values above the exact `u64` envelope use one `U256` proportional normalization for account power and total supply, capped at `u64::MAX / 7` so every ordinary Declining Power result remains representable. Independent account/supply saturation is forbidden because it can turn a minority holder into an apparent 100% holder.

In `runtime-benchmarks` builds, the ordinary provider retains equal weight `1`. The protection provider executes its production balance, issuance, normalization, and Declining Power path against a benchmark-prepared `$VETO` asset; `cast_vote` fills the veto set and executes immediate cancellation so generated weight covers the terminal branch and all winner-participation writes.

This demonstrates the project's `Runtime-as-Config` discipline. The pallet does not hardcode one-account-one-vote or an ordinary-ballot staking formula. It keeps ordinary and protection vote-power surfaces runtime-wired rather than baking asset lookup or temporal policy into pallet logic.

The runtime centralizes domain hierarchy, profile identity, and weight behavior through one typed `GovernanceDomainPolicy` declaration and shared consumers in `runtime/src/configs/governance_config.rs`. Protocol `$NTVE + $VETO` governance and tactical `$BLDR + $NTVE` governance therefore stay aligned across tally logic, query identity, and exported domain policy.

The current public surface is intentionally narrow: `governance_domain_policy(domain)` exposes the launch-line ordinary/protection power profiles, but it does not yet attempt to encode richer future class families, execution authorities, or broader constitutional topology beyond the current bounded query contract.

## Current Launch Policy

For the current launch line, the runtime policy is now intentionally frozen to the simplest bounded rule set that already exists in code:

**1. Ballot-time Declining Power.**

Normal runtime builds apply the shipped piecewise `7x -> 1x` curve to ordinary and protection-track ballots. Ordinary `Aye / Nay` derive their base from same-domain `Staking::stake_value(domain, account)`; protection-track `Veto / Pass` use the runtime-declared protection surface for that domain.

For `$BLDR`, the native protection surface adds locked `$NTVE`, locked `stNTVE` converted through the staking exchange rate, and account-level locked `$NTVE/stNTVE` LP converted into conservative native-equivalent `NativeVotePower`.

**2. Domain-scoped hierarchy.**

Protocol / network governance runs as `$NTVE` primary + `$VETO` protection, while `$BLDR` tactical governance runs as `$BLDR` primary + `$NTVE` protection. Both use the same bounded `Veto / Pass` cancellation lane with different base-weight surfaces.

**3. Protection-track cancellation.**

Domain-specific backing enables the first live protection slice. Protocol governance uses the well-known `$VETO` asset class, created at genesis with deterministic metadata and an Asset Registry-owned admin surface. `$BLDR` governance uses locked `$NTVE` / `stNTVE` / `$NTVE/stNTVE` LP-derived native `NativeVotePower`.

One account may vote once in each track on the same item. Later protection replacements use the later ballot epoch, and protection ballots remain admissible until the configured close.

Immediate cancellation requires frozen raw protection power to be **strictly greater** than the threshold against eligible protection supply. Raw `Veto` turnout below `1%` of supply counts as dust; otherwise the gate stays fail-closed unless decline-weighted `Pass` strictly outweighs decline-weighted `Veto`. Veto-cancelled items receive no governance reward-memory credit.

**4. Override and recovery.**

Admin control stays intentionally narrow. `reject_proposal(...)`, policy-aware `force_resolve_proposal_from_votes(...)`, and `requeue_proposal_for_auto_finalization(...)` form the recovery/override surface, with no broader arbitrary override contract.

**5. Finalized history.**

Bounded finalized-outcome retention is sufficient for the kernel pallet. Durable archival history belongs to events, indexers, or a future dedicated history surface rather than unbounded in-kernel storage growth.

Transferable ballot source non-reuse is enforced through `VotePowerCustodyByAccount` and the runtime `VotePowerCustody` adapter. The reference runtime transfers all free `$VETO` or same-domain staking receipt balance into one framework-owned custody account after a ballot is accepted. Later ballots read free plus already-custodied balance with checked addition, reuse that amount across concurrent proposals and `Veto <-> Pass` replacement, and increase the position only from newly free units. Distinct receipt assets retain distinct lock IDs, while domains backed by `$VETO` deliberately share its one aggregate source. The signed `unlock_vote_power` call releases the source after its monotonic maximum horizon.

Runtime regressions independently prove transfer rejection, concurrent reuse, repeated-vote rejection, `Veto <-> Pass` replacement, later-position growth, multiple domain receipt lock IDs, maximum-horizon extension, proportional `U256` normalization through `u128::MAX` supply, and matured release for both `$VETO` and same-domain staking receipts. Deterministic faults immediately after custody lock and unlock transfers prove exact storage-root restoration across voter/custody ledgers, ballots, aggregate positions, governance locks, participation, proposal state, and events. Try-state reconciles every aggregate position with its governance horizon, every live transferable ballot with a covering position and maximum proposal horizon, and each represented lock ID's summed positions with the host custody ledger.

## Query and Computation Semantics

### Tally and resolution are derived, not cached

The current pallet does not keep a precomputed weighted tally per proposal.
Instead:

- `ProposalVotesByItem[(domain, item_id)]` now stores bounded ballot sets for primary-track `Aye`, `Nay`, `Amplify`, `Approve`, `Reduce` plus protection-track `Veto`, `Pass`, with each ballot carrying the account, vote-time epoch, frozen computed weight, and frozen raw protection power
- `cast_vote(...)` computes ordinary ballot weight through `ProposalVoteWeightProvider` and protection-track ballot weight/raw power through `VetoVotePowerProvider` exactly once at vote time; `proposal_vote_tally(...)` and resolution then sum the stored ballot weights rather than re-reading live balances
- `cast_vote(...)` extends `GovernanceLocks[account].lock_until` to the maximum of its current value and the proposal's effective primary close plus enactment delay; runtime staking integration uses that horizon to refuse collator-LP, standalone-governance-LP, `$NTVE`, and `stNTVE` unlock requests while the locked position is still custody-backing frozen `NativeVotePower`
- After ballot admission, `cast_vote(...)` transactionally moves newly exposed transferable `$VETO` or staking receipts into aggregate source custody and extends that source horizon; a transfer failure rolls back the ballot and participation effects
- `unlock_vote_power(lock_id)` transactionally returns the full aggregate source position after its horizon and removes the position; early and unknown releases fail explicitly
- `cast_vote(...)` now already enforces the generic rule that ordinary ballots cannot enter before `primary_open`, while protection-track ballots remain admissible during any configured lead-in
- `proposal_resolution_state(...)` first checks whether frozen raw protection-majority triggers the immediate threshold. After maturity, raw `Veto` turnout must clear the `1%` dust floor before the stored-weight `Veto` versus `Pass` gate applies; only then does the evaluator derive primary state from the family-aware tally.
- Binary families use weighted `Aye / Nay`. Invoice families use `weighted_positive` versus `weighted_nay` with deterministic lowest-scalar tie-breaking across `Amplify / Approve / Reduce`. The current runtime reports `Invoice` for tactical-domain `L2TreasurySpend` in the canonical `$BLDR` domain and `Binary` for every other enabled combination.
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

This subsystem follows the project-wide [`read-model.contract.en.md`](../../../../docs/read-model.contract.en.md) split.

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
- `proposal_admission_policy_view(domain, payload_kind)`
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
- `governance_participation_coefficient(domain, account)`
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
- `recent_finalized_proposals(domain)` returns newest-first bounded recent-finalized summaries for retained outcomes through one canonical runtime view instead of asking clients to sort raw retained-outcome storage themselves; defensive overbound storage is truncated to the newest configured projection rather than panicking
- `ActiveProposals` and `FinalizedProposals` remain keyed by `(domain, item_id)` underneath those surfaces; recent records also expose that full identity explicitly

That means live proposal discovery and bounded recent-finalized discovery are chain-native today, while full archive/search/filter UX across expired history still belongs to explicit indexed/materialized views. Consumers SHOULD NOT treat ad-hoc iteration over current raw storage topology as the stable product contract.

### Current runtime-upgrade operator path

The current line has one explicit bounded off-browser operator flow:

1. `/scripts/authorized-upgrade-local.sh check` pins one finalized block and compares its runtime code with the selected local candidate
2. the same read-only check requires `PrimaryEligibleSigned` for protocol `L1RootAction` and reports current `$VETO` issuance; zero issuance fails strategic lifecycle readiness closed rather than implying authorization can complete
3. `prepare-authorization` emits candidate-bound stake, exact preimage, compact witness preparation, and signed proposal call data with finalized item/balance/fee checks; it never signs and withholds the protection `Pass` call while lifecycle readiness fails
4. separately approved governance actions may create the proposal and, after legitimate protection power exists, authorize one `code_hash` through `L1RootAction -> System.authorize_upgrade { code_hash }`
5. operators read `authorized_runtime_upgrade()` and the helper classifies the selected code as `awaiting-governance-authorization`, `authorized-hash-mismatch`, or `ready-to-relay-code`
6. at `ready-to-relay-code`, `/scripts/authorized-upgrade-local.sh apply` provides the dedicated relay submit surface and stays plan-only unless `--submit` is passed explicitly

This is intentionally still an off-browser operator flow rather than a browser action. The browser governance surface remains read-only for that second step, while the verifier and relay helper both default to non-submitting behavior.

### Current post-bootstrap relay contract

There is no longer a separate bootstrap superuser owner for the external relay step.
The current contract is narrower and cleaner:

- governance decides _which_ `code_hash` is authorized
- any operator MAY relay the matching code bytes after the verifier reaches `ready-to-relay-code`
- that relay step MUST remain ministerial rather than becoming a second governance veto or reinterpretation surface

This keeps the current line honest: governance owns authorization of upgrade intent, while the later `apply_authorized_upgrade` call is only a transport step for already-authorized bytes.

## Integration Boundary with DEOS Staking

The key exported staking surface is:

- `governance_participation_coefficient(domain, account)`

The pallet also now exports GovXP input observability through:

- `govxp_counters(domain, account)`

In the current runtime, staking maps:

- `reward_governance_domain(asset_id) = asset_id`
- `governance_participation_coefficient(asset_id, account) = Governance::governance_participation_coefficient(asset_id, account)`

The runtime may read this projection at a native-security snapshot boundary. The view rotates a local copy to current epoch and never persists query-time cleanup. Governance itself imports no Staking surface, has no reward-touch callback, and schedules no downstream settlement mutation.

## Validation Surface

The implementation is covered by:

- Pallet tests in `template/pallets/governance/src/tests.rs`
- Runtime integration tests in `template/runtime/src/tests/governance_integration_tests.rs`
- FRAME v2 benchmarks in `template/pallets/governance/src/benchmarking.rs`
- Runtime weight bridge in `template/runtime/src/weights/pallet_governance.rs`
- Try-state reconciliation of reward-window width, rolling sums, epoch vote caps, resolution-window width, per-domain finalized-projection cardinality, and payload-witness key/contract agreement

The production bridge was regenerated with `frame-omni-bencher 0.22.0`, `50` steps, and `20` repeats.

`submit_signed_proposal` measures primary eligibility, compact status/witness admission, opening-fee transfer, and strategic creation with the domain index filled to `MaxActiveProposalsPerDomain - 1`; it charges `675,865,000 / 324,459` plus 138 reads and seven writes. `prepare_payload_admission_witness` measures the maximum valid 262-byte payload and charges `22,419,000 / 3,556` plus two compact-status reads and one witness write. `cast_vote` charges `1,266,312,000 / 656,094` plus 269 reads and 271 writes. `unlock_vote_power` charges `68,096,000 / 6,208` plus five reads and five writes.

The runtime benchmark helper ensures the protocol governance asset and staking pool exist, funds the caller, and stakes it before measurement. Lifecycle benchmarks derive voting and maturity epochs from runtime lead-in and voting-period constants rather than mock-only block numbers.

Error narrowness is compile-enforced at the preimage adapter boundary: `ProposalPreimageAdmissionError` has one exhaustive conversion into the four validation dispatch errors, and `preimage_admission_error_core_maps_exhaustively_to_dispatch` executes every mapping. Compact witness absence and staleness remain distinct pallet-owned admission errors.

Coverage includes:

- Duplicate item protection
- Pre-fee rejection without mutation for witness/status, author/domain-capacity, and maturity-bucket failures
- Transactional opening-fee, event, proposal, index, maturity, and authorship rollback on a late post-fee counter failure
- State-preserving witness refresh failure and exact signed enactment from the preimage selected by committed proposal hash despite a competing valid payload
- Transactional rollback of late reward-memory batch failures
- Weighted vote-derived outcomes
- Turnout and approval-threshold rejection
- Auto-finalization and retry deferral
- Early force resolution
- Finalized outcome retention and expiry
- Governance participation projection and absence of downstream reward-touch callbacks

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

Ordinary `Aye / Nay` applies ballot-time Declining Power to same-domain `Staking::stake_value(...)`. Protocol / network protection-track `Veto / Pass` applies the same curve to the `$VETO` asset. `$BLDR` protection applies it to locked `$NTVE` / `stNTVE` / `$NTVE/stNTVE` LP-derived native `NativeVotePower`.

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

**10. Builder invoice settlement has not converged to the current contract.**

The runtime still selects the fixed `BldrTreasury` enum, carries no bounded validated CIDv1 invoice identity or explicit validated treasury account, transfers only the full scalar target, and collapses insufficient capacity into generic dispatch failure. Runtime types, tests, benchmarks, Weight, metadata, and clients must implement `BaseFloorCapped` settlement, the new payload fields, and typed below-floor `InsufficientTreasuryCapacity` evidence before this architecture can claim the complete Builder invoice contract as shipped.

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
