# DEOS Governance Specification

- **Component**: `pallet-governance` + runtime governance integration
- **Status**: Normative

> The key words **MUST**, **REQUIRED**, **SHALL**, **SHOULD**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this document are to be interpreted as described in RFC 2119.

> Implementation map: [`architecture.en.md`](./architecture.en.md)
> This document is the single ultimate contract for governance in the TMCTOL standard on DEOS, including L2 TOL patterns, invoice voting, treasury integration, and domain hierarchy.

---

## 1. Purpose

This specification defines the normative product and protocol contract for DEOS Governance in the TMCTOL standard.

`DEOS Governance` is the DEOS-specific bounded dual-track alternative to Polkadot OpenGov. Its job is not to replace the autonomous economic kernel with politics-by-default; it governs the residual social and agent-driven domain that remains after protocol autonomy mechanizes what it can.

The governance layer has four jobs:

1. Govern proposals and referenda through explicit, bounded lifecycle rules
2. Express differentiated voting tracks (ordinary, protection, and payload-specific primary tracks such as invoice-style treasury spending)
3. Provide explicit L1 Strategy / L2 Tactics layering with protection flow from strategic to tactical domains
4. Export bounded resolved governance participation (including authorship inputs) without owning downstream reward settlement

This document is the contract layer for **how governance should behave**.
`template/pallets/governance/docs/architecture.en.md` remains the implementation-first description of the paired runtime architecture.

---

## 2. Core Governance Atoms

### 2.1 Governance Domain

A `GovernanceDomain` is the unit within which proposals, ballots, winning memory, and governance participation coefficients are evaluated.

The runtime MUST define how domain ids map to actual governed subjects.
The contract permits asset-scoped domain bindings, but governance is not required to remain identical to the reference runtime's `DomainId = AssetId` style.

### 2.2 Governance Item

A `GovernanceItem` is the canonical identity of one proposal / referendum subject inside a domain.

The governance system MUST preserve these distinct notions:

- `item identity` — which subject is being voted on
- `proposal lifecycle state` — whether the item is active, rejected, resolved, veto-cancelled, or retained as recent finalized history
- `reward-memory relevance` — whether the resolved item still contributes to the bounded winning-vote window

The system MUST NOT assume that an item's on-chain identity is stored forever inside the pallet itself.
Permanent history MAY live in indexers or future dedicated archival surfaces.

### 2.3 Protected Governance Decomposition

All referenda in the system operate under the **Protected** model by default.

There is no autonomous `ProposalClass` ontology.
Governance behavior is expressed through four smaller axes instead:

- `GovernanceDomain` — who governs and which protection surface guards that domain
- `CadenceMode` — `Ordinary` or `Fast`
- `ProposalPayloadKind` — what kind of action is being authorized
- `ProtectionTrack` — `Veto / Pass` as the constitutional override surface

The payload-kind vocabulary is:

- `L1RootAction` — protocol-level action executed with the governance-controlled super-user / Root-equivalent authority
- `L2TreasurySpend` — domain-local treasury spend
- `L2ParameterChange` — domain-local parameter/configuration change
- `Intent` — domain-local non-executable expression of community will inside the current governance domain
- `L2SignalToL1` — domain-local expression of will from L2 toward L1 that does **not** itself execute Root-equivalent authority

This simplification removes unnecessary abstraction while preserving the core guarantee that strategic matters remain protected.

### 2.4 Governance Layering and Canonical Subjects

TMCTOL governance MUST preserve the manifesto-level layering:

- `L1 Strategy` — direction, capital allocation, and ecosystem-level protection
- `Bridge` — coordination that carries strategic protection into tactical domains
- `L2 Tactics` — domain-local execution, contributor coordination, and local treasury priorities

The contract SHOULD keep these canonical governance subjects explicit:

- `Native` — parachain base token governed at the strategic layer (`$NTVE` in the reference line)
- `$VETO` — constitutional protection token reserved for the strategic / Native domain in the reference line
- `L1 TOL` — protocol-owned liquidity controlled at the strategic layer
- `L2 TOL` — tactical DAO or subdomain paired with L1 Native liquidity
- `Ecosystem L2 TOL` — tactical domain bootstrapped by strategic origin and treasury funding
- `L2TOLT` — tactical-domain token emitted with mandatory TOL support
- `$BLDR` — canonical tactical domain token for payroll and contributor coordination

The governance contract MUST keep strategic protection explicit rather than smuggling it into ordinary vote math, social convention, or proposal-id naming.

---

## 3. Lifecycle Contract

Every governance item MUST move through an explicit bounded lifecycle that distinguishes protection, primary, finalization, and enactment phases.

Minimum lifecycle states:

1. `Submitted / ProtectionWindowOpen`
2. `Lead-in / PrimaryNotYetOpen`
3. `PrimaryVotingWindowOpen`
4. `FastTrackPrimaryVotingWindowOpen` (if urgent mode is authorized)
5. `FinalizedApproved`
6. `PendingEnactment` (if enactment delay is enabled)
7. `Enacted`
8. `ExecutionFailed`
9. `Rejected`
10. `VetoCancelled`
11. `Retained recent finalized outcome`
12. `Expired from bounded on-chain recent history`

The implementation MAY compress some of these into storage/query helpers, but the public contract MUST keep them distinguishable.

The governance system MUST ensure:

- Active proposals are bounded
- Ballot storage is bounded
- Maturity and enactment servicing are bounded
- Execution failure is explicit rather than silently masquerading as enactment
- Recent finalized-outcome retention is bounded
- Reward-memory relevance is bounded by configured lookback

### 3.1 Lead-in and Early Protection Window

The governance system MUST support a runtime-configured `lead-in period` before the ordinary primary voting window opens for public referendum combinations that use ordinary cadence.

For the public cadence:

1. A submitted proposal MUST open its protection track immediately on submission
2. The lead-in duration MUST be runtime-configured and queryable
3. Protection-track `Veto / Pass` ballots MUST be accepted during lead-in
4. Primary-track ballots cast during lead-in MUST be rejected
5. The default public lead-in SHOULD be `3 days`
6. The ordinary public protection window SHOULD remain open for `7 days` from submission
7. The proposal MUST transition automatically from lead-in to `PrimaryVotingWindowOpen` without admin action unless urgent fast-track authorization has already opened the primary track earlier

Rationale: the canonical Declining Power curve now starts at `7x`, not `10x`, but the underlying fairness problem is the same — strategic/protection reaction should begin immediately, while ordinary temporal weighting should only start after the public has had a bounded awareness window.

### 3.2 Enactment Delay and Optional Confirm Period

The governance system MUST support a runtime-configured `enactment delay` after final approval and before the approved action takes effect.

Rules:

1. A finalized approved proposal with a positive enactment delay MUST enter `PendingEnactment`
2. The enactment delay MUST be runtime-configured and queryable
3. Ordinary public referenda SHOULD default to a `3 day` enactment delay
4. Urgent fast-track combinations MAY set enactment delay to `0`
5. The query contract MUST distinguish `finalized approved but not yet enacted` from `already enacted`
6. Enactment servicing MUST remain bounded

An implementation MAY also support a `confirm period` before finalization.
If confirm is enabled for a domain + payload-kind combination, a matured proposal that currently passes ordinary policy MUST sustain that passing state continuously for the full confirm duration before finalization, and the confirm timer MUST reset if later ballots interrupt that state.
That confirm path is optional; the public reaction buffer is the enactment delay above, not a mandatory confirm window.

### 3.3 Urgent Fast-Track Cadence

The governance system MAY mark specific domain + payload-kind combinations as `expeditable`.
If an expeditable proposal receives raw protection-track `Pass` support that meets the runtime-configured urgent-pass threshold against that domain's total eligible protection supply, the proposal MUST enter urgent fast-track cadence.
For the default public contract that threshold SHOULD be `strictly greater than 50%`, while the current TMCTOL strategic runtime-upgrade line intentionally tightens `L1RootAction` to unanimous raw protection-track `Pass` over the full eligible `$VETO` supply.

Urgent fast-track rules:

1. The urgent-pass trigger MUST be computed against raw protection supply, not against the decline-weighted protection tally
2. Fast-track authorization is purely procedural and MUST NOT count as substantive primary-track approval
3. Any remaining lead-in ends immediately once fast-track authorization is reached
4. The primary track opens immediately at that moment
5. The urgent primary window lasts exactly `1 day`
6. The urgent primary window uses flat `1x` vote power for the full day; the canonical Declining Power curve does not apply to that urgent primary track
7. If the urgent primary track passes, enactment happens immediately
8. The immediate-threshold raw `Veto` brake MUST remain live until urgent finalization
9. Once urgent fast-track is authorized, the ordinary final protection gate is treated as procedurally satisfied for that item unless a later immediate-threshold veto cancels it
10. A runtime MAY define an even narrower unanimous-protection special case for specific payload kinds if the constitution demands it; on the current TMCTOL line, only `L1RootAction` may use such a rule
11. Under that special case, if raw protection-track `Pass` reaches the full eligible protection supply for that runtime-upgrade payload, the runtime MAY bypass the ordinary/urgent primary-track vote entirely and execute the already-authorized runtime-upgrade path immediately
12. That unanimous protection path MUST remain payload-kind-scoped and MUST NOT generalize into a blanket positive-governance override for unrelated referendum families

Rationale: urgent runtime/security actions need speed, but they should not reward whoever reached the vote screen first with extra primary-track temporal weight.

---

## 4. Track Model

### 4.1 Ordinary Track

The ordinary track is the baseline referendum track.
Its minimum contract is weighted `Aye / Nay` voting with:

- A bounded voting window preceded by the lead-in contract from Section 3.1 for public ordinary-cadence combinations
- A runtime-defined fixed approval threshold per Section 4.3
- A runtime-defined fixed turnout floor per Section 4.3
- Explicit passing / failing semantics
- A runtime-defined ordinary primary duration, with `7 days` as the default public cadence
- An optional enactment delay per Section 3.2

### 4.2 Protection Track

The governance system MUST support a secondary protection track for domains that need an explicit constitutional cancellation layer above the primary track.
The protection lane is represented through `Veto` plus protection-track `Pass`, and its backing power surface remains domain-specific rather than universally tied to one token.

The protection-track contract MUST keep these properties:

1. `Separate semantics` — the protection track MUST be represented as its own track or equivalent explicit state, not folded invisibly into ordinary `Nay`
2. `Explicit domain binding` — each protected governance domain MUST declare which protection surface backs that track
3. `Explicit threshold` — protection-triggered cancellation threshold MUST be runtime-defined and queryable
4. `Minimum raw protection floor` — the final protection-track gate for ordinary cadence MUST support a runtime-defined minimum raw `Veto` turnout floor against the total eligible protection supply, so accidental dust cannot activate the fail-closed branch by itself
5. `Early-open availability` — the protection track MUST open at submission and remain available throughout lead-in plus any ordinary primary window
6. `Dual-mode cancellation` — the protection track MUST support both of these contracts:
   - `immediate threshold veto` — if frozen raw protection power on recorded `Veto` ballots becomes **strictly greater** than the configured threshold against the total eligible protection supply, the proposal MUST resolve into `VetoCancelled` immediately
   - `final protection-track gate` — if immediate threshold veto did not happen, the ordinary-cadence final proposal outcome MUST still be gated by the separate protection track at resolution time, but only once raw `Veto` turnout reaches the configured minimum floor
7. `Explicit pass side inside the protection track` — the protection track MUST support both `Veto` and explicit protection-track `Pass` ballots or an equivalent two-sided representation, so consumers can distinguish `cancel`, `allow ordinary track to decide`, and `authorize urgent handling when that domain + payload-kind combination allows it`
8. `Track-local vote replacement` — within the protection track, a later `Veto` or `Pass` ballot MAY replace that account's earlier protection-track ballot for the same item; this replacement MUST remain bounded and MUST NOT mint duplicate participation or winner-memory effects
9. `Track independence` — one account MAY participate once in the primary track and once in the protection track for the same item; the protection track MUST NOT be modeled as a hidden alias of ordinary voting
10. `Procedural acceleration` — expeditable domain + payload-kind combinations MAY interpret the runtime-configured raw protection-track `Pass` threshold from Section 3.3 as urgent authorization, but that signal MUST remain procedural rather than substantive approval
11. `No winner-memory credit` — a veto-cancelled proposal MUST NOT mint ordinary winning-vote credit into governance reward memory unless a future domain + payload-kind policy explicitly defines a different rule
12. `Boundedness` — protection-track participation, servicing, retention, and status storage MUST remain bounded

The contract requires:

- Protocol / network governance protection is backed by the well-known `$VETO` protocol asset
- Canonical tactical `$BLDR` governance protection is backed by runtime-resolved native `NativeVotePower` from explicit native economic locks
- Protection-track ballots carry their own vote-time epoch, so late `Pass -> Veto` or `Veto -> Pass` changes are repriced to the later weaker Declining Power multiplier rather than inheriting an earlier stronger one
- Immediate cancellation uses frozen raw `Veto` ballot power / total eligible protection supply and requires `Veto` power to be **strictly greater** than the configured threshold rather than merely equal to it
- For `$VETO`-backed protocol governance, that raw guard compares live `$VETO` balance against total `$VETO` issuance
- For the canonical `$BLDR` domain, that raw guard compares frozen native `NativeVotePower` against total eligible native `NativeVotePower`
- The final protection-track gate requires a raw `Veto` dust floor of at least `1%` of total eligible protection supply before fail-closed blocking can activate
- If immediate threshold veto does not happen and that raw `Veto` floor is met, the ordinary-cadence protection track is fail-closed unless decline-weighted protection-track `Pass` strictly outweighs decline-weighted `Veto`
- All first-class proposal families are protected by default unless a contract change explicitly declares otherwise

### 4.3 Fixed Approval and Turnout Policy

The contract uses runtime-configured fixed approval thresholds and fixed turnout floors rather than adaptive curves.

Contract rules:

1. The required approval ratio and turnout floor MUST be runtime-configured and queryable
2. Those thresholds MUST be deterministic for a given domain + payload-kind combination
3. Different domain + payload-kind combinations MAY use different fixed thresholds, but the thresholds themselves MUST NOT change as the voting window ages
4. Protection-track cancellation thresholds remain independent raw-supply guards per Section 4.2
5. Any temporal asymmetry MUST come from lead-in, Declining Power, and optional urgent fast-track cadence rather than from moving approval/support curves

Rationale: fixed thresholds are materially easier for participants to understand, while the time dimension is already carried by the lead-in window, the canonical Declining Power curve, and the urgent fast-track path.
Adaptive approval/support curves are excluded.

### 4.4 Domain Hierarchy

The governance contract SHOULD model each governance domain as an explicit two-layer topology:

- `primary track` — the default decision surface for that domain
- `protection track` — a secondary cancellation / override surface guarding that domain

Current canonical hierarchy:

- `Protocol / network governance` — primary track = `Native`, protection track = `$VETO`
- `Canonical tactical governance` — primary track = local tactical domain token (`$BLDR` in the reference line), protection track = `Native`

The runtime MAY add more domains later, but this topology MUST stay explicit and queryable rather than being hidden in ad-hoc formulas or proposal-id conventions.

### 4.5 L2 TOL Governance Patterns

L2 TOL systems introduce tactical governance domains that MUST compose with the core track model while adding domain-specific execution patterns.

`Invoice Voting Pattern` (canonical tactical example and a first-class contract for `$BLDR` governance):

- Primary evaluation track = domain token (e.g. `$BLDR`)
- Protection track = upstream strategic surface (e.g. `Native`)
- Primary-track options: `AMPLIFY (×2.0)`, `APPROVE (×1.0)`, `REDUCE (×0.5)`, `NAY`
- Protection-track options: `VETO`, `PASS`
- `NAY` on the primary track rejects the invoice without payout
- `VETO` on the protection track cancels or invalidates the referendum at the constitutional layer
- The winning positive option determines a discrete payout scalar; the contract does **not** blend positive options into one averaged multiplier
- Protection remains binary and separate from pricing
- During ordinary cadence, the canonical Declining Power rule from Section 5.2 applies to both tracks
- Fast-track by the Section 3.3 raw protection-track `Pass` threshold is **not** part of the default invoice contract unless a future domain + payload-kind policy explicitly opts into urgent mode

`Treasury Participation Modes`:

- `One-shot seed` — immediate strategic position
- `DripVault stream` — continuous support over configurable duration with per-block execution
- `Revenue recycling` — percentage of income auto-converts to domain token

`LP Management`:

- LP tokens marked as L2 TOL can be managed in `Locked Mode` or `Treasury Mode`
- Locked Mode requires supermajority approval, timelock, and strategic veto window
- Treasury Mode keeps LP under DAO treasury governance
- All LP movements MUST remain under protocol-controlled accounts and move only through governance-defined mechanisms
- Cross-parachain deployment, liquidity rebalancing, and multi-token liquidity deployment are strategic capabilities subject to governance approval

`Team Share Configurability`:

- Team allocation fully configurable (0-100%)
- Vesting duration fully parameterized (none or any duration)
- Governance rights configurable (full voting, veto-only, LP-based voting)
- Transfer restrictions configurable
- All parameters modifiable via governance unless set immutable at creation

All L2 TOL governance MUST compose with the core track, vote-power, lifecycle, and boundedness rules defined in this specification.

---

## 5. Vote-Power Contract

### 5.1 Base Weight Surface

Governance MUST use runtime-configured vote-weight providers rather than hardcoding one-account-one-vote.

The base balance surfaces are:

```text
ordinary Aye / Nay base = same-domain Staking::stake_value(domain, account)
protection-track Veto / Pass base = runtime-declared protection surface for that domain
```

Canonical protection surfaces are:

```text
protocol / network governance => live Assets::balance(VETO_ASSET_ID, account)
canonical tactical $BLDR governance => Staking::stake_value(NTVE_ASSET_ID, account)
```

These base-balance surfaces are then transformed by the temporal-weighting policy rather than being exposed as the final vote-power contract directly.

Recorded ballots use `frozen vote-power settlement`.
At cast time, the implementation resolves the applicable base backing surface, applies that track's temporal policy, and stores the resulting ballot weight. Protection-track ballots also store the raw protection power needed by raw-threshold guards.
That means:

- if the backing balance, stake, staking exchange rate, or LP reserve state changes after the vote, the already-cast ballot's stored weight does not change
- a transferable backing position used by a ballot MUST NOT move to another account while its governance lock is active; otherwise the same fungible units can be frozen repeatedly through different accounts on one item
- one account may still cast at most one primary-track ballot and one protection-track ballot per item
- implementations MUST surface this honestly in queries/UI so users understand that recorded ballot weight is a vote-time snapshot, not a live balance view

If canonical protection power is wider than the stored ballot integer, account power and total eligible supply MUST use one common proportional normalization with enough headroom for the maximum temporal multiplier. Independently saturating account power and total supply is forbidden because it can make a minority position appear to own the full protection supply.

A conforming governance implementation MUST maintain one aggregate account-level governance lock horizon. Each accepted ballot extends that account's `lock_until` to at least the proposal's enactment horizon:

```text
lock_until = max(current_lock_until, proposal_primary_close + enactment_delay)
```

The lock horizon is account-level, not a per-referendum unlock ledger. Runtime-specific staking, asset, or LP custody layers enforce source locks, while the ballot record remains the frozen vote-power fact for the proposal.

For a transferable backing source, the implementation MUST retain one O(1) custody position per account and source. A ballot may increase that position from newly available free balance through checked arithmetic but MUST NOT sum the same locked quantity once per simultaneous proposal. Vote-power reads include free plus custody, so already locked units remain reusable across concurrent items and `Veto <-> Pass` replacement without becoming transferable. The position amount and horizon only increase while active; explicit release after `current_epoch >= lock_until` returns the complete source position. Custody transfer, position mutation, ballot insertion, and immediate terminal handling share one transaction. A host that enables transferable custody MUST expose its actual per-source custody total to integrity tooling so aggregate positions, live-ballot horizons, and host ledger balances can be reconciled.

### 5.2 Declining Power

The TMCTOL public-ballot model uses a canonical piecewise `Declining Power` rule on ordinary tracks.

For ordinary public tracks, the multiplier starts at `7x`, decays linearly to `1x` by the end of the sixth day of that track's seven-day window, and then remains fixed at `1x` for the entire seventh day.
Urgent fast-track primary windows are the explicit exception: they run flat at `1x` for their full one-day duration.

Conceptual model for ordinary seven-day tracks:

```text
progress = (vote_time - track_open) / (track_close - track_open)
if progress <= 6/7:
  temporal_multiplier = 7 - 7 * progress
else:
  temporal_multiplier = 1
final_weight = base_weight * temporal_multiplier
```

Contract requirements:

1. The multiplier MUST be deterministic and runtime-verifiable
2. The decay window MUST be tied to the actual opening and closing timestamps of the specific track being voted on
3. Protection and primary tracks MUST use their own local clocks; protection ordinary cadence starts at submission, ordinary primary cadence starts at primary-open, and urgent fast-track primary uses flat `1x`
4. Early and late voters on the same side MUST be allowed to contribute different effective weight on ordinary cadence
5. The temporal rule MUST be surfaced honestly in queries/UI, not hidden behind opaque aggregate numbers
6. Changing a live ballot side late MUST reprice that ballot to the later vote-time weight rather than preserving an older stronger multiplier
7. The special immediate-veto emergency threshold remains a separate raw-supply guard: it uses the ballot's frozen raw protection power against total eligible protection supply with the configured strict `>` majority rule, rather than consuming the declining-power multiplier

Active proposal ballots MUST carry enough vote-time and frozen-weight information to verify the temporal-weighting rule, and immediate cancellation still uses raw protection power / total eligible protection supply so the explicit strict-majority emergency contract remains independent of the decline-weighted protection tally.

### 5.3 Track Power Profiles

The governance contract MUST allow different enabled tracks to resolve vote power through explicit runtime-configured `TrackPowerProfile`s rather than through one irreversible global formula.

The contract SHOULD represent these profile families:

- `DecliningDirectStake` — same-domain staking weight with ballot-time decay
- `DecliningVetoAsset` — protocol / network protection weight backed by the well-known `$VETO` asset with ballot-time decay
- `DecliningNativeStake` — tactical protection weight backed by runtime-resolved native `NativeVotePower` with ballot-time decay
- `FlatUrgentDirectStake` — same-domain primary weight with constant `1x` urgent handling

Contract rules:

1. Each enabled track MUST resolve through one explicit power profile
2. Profile identity and its backing surface MUST be queryable and documentable
3. Domain-level protection surfaces MUST stay outside the ordinary same-domain rule rather than being hidden inside one opaque formula
4. The urgent flat-`1x` profile MUST be explicit rather than hidden inside ad-hoc lifecycle code
5. The pallet SHOULD stay runtime-configurable at this boundary instead of hardcoding one irreversible constitutional settlement for all future domains

### 5.4 Flat Urgent-Track Exception and Future Proxy Extensions

The contract has one explicit non-declining exception: urgent fast-track primary windows use flat `1x` weighting for `1 day` per Section 3.3.
Any additional non-declining, delegated, or proxy-driven exception MAY be defined later only if measured product/runtime evidence proves that those extra layers are needed.

If such an exception exists:

- It MUST resolve through an explicit power profile
- It MUST be runtime-configurable
- It MUST be queryable and documentable
- It MUST NOT be smuggled into the ordinary same-domain direct-holder rule

The contract defines no proxy-governed vote-power exception beyond the explicit urgent flat-`1x` primary-track exception above.

### 5.5 GovXP Counters and Future Multipliers

Governance MUST persist bounded on-chain GovXP input counters even though the contract does **not** require a live GovXP vote-power multiplier.

The on-chain GovXP counter surface MUST expose these five inputs per `(domain, account)`:

1. Bounded rolling winning-side participation inside the current reward-memory lookback tail
2. Cumulative total proposal participation
3. Cumulative total winning-side participation
4. Cumulative total authored/opened proposals
5. Cumulative total successful authored proposals

Contract rules:

- These counters MUST be queryable on-chain
- Proposal-authorship counters MUST accumulate canonically so a future explicit GovXP policy can consume them without archive reconstruction
- The authored/opened counter MUST increment when a proposal is opened
- The successful-authored counter MUST increment when that proposal reaches successful approval/finalization
- Any saturation needed for later formula consumption MUST remain bounded and explicit
- A live GovXP multiplier MUST NOT silently appear inside vote power
- Enabling a live GovXP multiplier requires an explicit contract change, and its ceiling MUST NOT exceed `3x` without another explicit contract change

The contract treats the counters as canonical and leaves a live multiplier outside the current capability boundary.

### 5.6 Extended GovXP Policy Families

The contract keeps GovXP narrow.
Only the chain-native counters above are required.
A future contract change MAY define explicit bounded multiplier formulas, authorship-quality policy, invoice-outcome credit, or delegation-linked logic, but only if each ingress family remains bounded, auditable, and domain-scoped.

If such a later profile exists, the contract MUST keep:

1. Explicit ingress semantics per event family
2. Public queryable counters or equivalently bounded derivable projections
3. Deterministic update points at proposal, invoice, or finalization boundaries
4. Reward and penalty values that stay runtime-configurable rather than hardcoded forever
5. Saturation or decay rules preventing unbounded influence growth
6. A maximum live multiplier ceiling of `3x` unless a later specification revision raises it explicitly

---

## 6. Resolution Contract

Ordinary and urgent resolution MUST remain explicit and policy-driven.

For ordinary binary `Aye / Nay` tracks, the fixed-threshold contract is:

```text
weighted_turnout = weighted_aye + weighted_nay
approval_ratio = weighted_aye / weighted_turnout
primary_passes =
  weighted_turnout >= minimum_turnout
  AND weighted_aye > weighted_nay
  AND approval_ratio >= approval_threshold
```

Contract rules for that formula:

- `weighted_turnout` uses the decline-weighted primary-track tally, not voter count and not eligible-supply ratio
- `minimum_turnout` is therefore a fixed weighted-unit floor in the primary track's own vote-power units
- `approval_threshold` is evaluated as `weighted_aye / (weighted_aye + weighted_nay)`
- If `weighted_turnout == 0`, the item resolves as `NoVotes`
- If `weighted_aye == weighted_nay`, the item resolves as `VoteTie`
- If turnout clears but neither side reaches the approval threshold, the item resolves as `ApprovalThresholdNotMet`
- Urgent primary windows use the same binary passing formula unless a domain + payload-kind policy explicitly overrides it, but they apply flat `1x` primary weights instead of the ordinary Declining Power curve

At minimum, proposals MUST resolve through these decision stages:

1. Check whether the protection track has already triggered immediate-threshold cancellation
2. If lead-in is still active and urgent authorization has not happened, keep the proposal in a non-primary-open state
3. For ordinary cadence at or after primary maturity, evaluate `weighted_turnout` against the fixed turnout floor from Section 4.3
4. For ordinary cadence, evaluate the weighted passing / failing side against the fixed approval threshold from Section 4.3 using the binary formula above
5. For ordinary cadence, if raw `Veto` turnout reaches the configured floor, apply the final protection-track gate from Section 4.2
6. For urgent cadence, evaluate the `1 day @ 1x` primary window after fast-track authorization; the ordinary final protection gate is treated as procedurally satisfied by that fast-track authorization itself, but immediate-threshold veto remains live until finalization
7. Emit explicit finalized outcome state
8. If approved and enactment delay is positive, enter `PendingEnactment`; otherwise attempt enactment immediately
9. If enactment dispatch fails, enter explicit `ExecutionFailed` state
10. Retain only bounded recent outcome history

Finalized projections MUST own proposal approval exactly once as one approval fact containing approval epoch and winner count. Pending enactment and later enacted, execution-failed, or advisory-finalized states MUST reuse that fact rather than repeat its fields. Typed execution detail MAY add a success receipt or failure reason, but MUST NOT duplicate payload kind, derived authority, approval epoch, or terminal epoch already owned by metadata, policy, and finalized lifecycle state. Recent bounded history MUST expose the complete domain-plus-item identity.

The system MUST distinguish between:

- `policy currently passes`
- `policy currently fails`
- `finalized approved`
- `pending enactment`
- `enacted`
- `execution failed`
- `finalized rejected`
- `finalized veto-cancelled`

That distinction matters for auto-finalization, recovery, UI, and reward-memory export.

### 6.1 Multi-Option Primary Evaluation Tracks

Some tactical domains MAY use a multi-option primary track instead of bare `Aye / Nay`.
The canonical example is invoice governance in the `$BLDR` domain, and the contract treats that invoice line as a first-class capability rather than an optional extension.

Invoice or evaluation-track contract:

- Positive evaluation family = `Amplify`, `Approve`, `Reduce`
- Explicit primary-track rejection branch = `Nay`
- Separate protection lane = `Veto`, `Pass`

Resolution math for such a payload kind is:

```text
weighted_positive = weighted_amplify + weighted_approve + weighted_reduce
weighted_turnout = weighted_positive + weighted_nay
positive_ratio = weighted_positive / weighted_turnout
```

Resolution order for such a payload kind MUST remain:

1. Evaluate immediate protection cancellation first under Section 4.2
2. For ordinary invoice cadence, apply the ordinary final protection gate before pricing if raw `Veto` turnout reaches its floor
3. If `weighted_turnout == 0`, reject as `NoVotes`
4. If `weighted_turnout < minimum_turnout`, reject for turnout failure
5. If `weighted_positive <= weighted_nay`, reject the invoice with no payout
6. If `positive_ratio < approval_threshold`, reject the invoice with no payout
7. If the positive family passes, select the winning positive option by highest individual positive weight
8. If two or more positive options tie for highest positive weight, choose the tied option with the lowest payout scalar so tie handling stays deterministic and fail-safe
9. Apply that winning discrete scalar (`Amplify = 2.0x`, `Approve = 1.0x`, `Reduce = 0.5x`)
10. `Pass` never prices the invoice; it only governs the constitutional protection path

### 6.2 Invoice Execution Contract

An invoice-style proposal MUST define an explicit executable payout payload.
At minimum that payload MUST declare:

- `beneficiary` — the account that receives the payout
- `payout asset` — the asset in which the invoice is paid
- `base amount` — the pre-scalar amount approved for evaluation
- `funding source` — the treasury or other governance-declared controlled account from which payment is drawn

Execution rules:

1. The discrete winning scalar from Section 6.1 applies to the declared `base amount`
2. `final payout = base amount * winning scalar`
3. The funding source MUST be one of the domain's governance-declared executable treasury sources; payout MUST NOT be implicitly drawn from arbitrary protocol balances. On the current reference line, the tactical `$BLDR` domain declares exactly one executable treasury source: `BldrTreasury`
4. The beneficiary is the explicit payload field, not automatically the proposer unless those identities are intentionally the same
5. Invoice execution MUST be transactional: either the full payout executes, or no payout executes
6. If the payload exceeds declared domain/payload caps or available spendable capacity at enactment, execution MUST fail explicitly rather than partially paying
7. A successful invoice enactment MUST produce a bounded on-chain observability record that includes beneficiary, payout asset, base amount, winning scalar, final payout amount, and funding source

The protection lane MUST remain constitutionally separate from primary-track pricing:

- `Nay` rejects the item
- `Veto` cancels or invalidates the referendum
- `Pass` allows the primary track to decide or, for expeditable domain + payload-kind combinations only, authorizes urgent handling procedurally

### 6.3 Advisory Payload Contract

`Intent` and `L2SignalToL1` are bounded advisory payload kinds.
They express social or political will, but they do not by themselves dispatch privileged state transitions.

Both payload kinds MUST remain structurally bounded.
At minimum, an implementation SHOULD support this shape:

- `referenced_payload_hash: Option<Hash>`
- `summary: Bounded UTF-8 text`
- `doc_cid: Option<Bounded CID>`

Recommended bounds:

- `MaxIntentSummaryBytes = 128`
- `MaxSignalSummaryBytes = 128`
- `MaxCidBytes = 96`

Contract rules:

1. `Intent` is same-domain only: it expresses the will of the current governance domain without escalating authority to another domain
2. `L2SignalToL1` is upward only: it expresses an L2 domain's will toward L1 without executing Root-equivalent authority directly
3. Advisory payloads MUST NOT accept arbitrary long-form on-chain text blobs
4. `summary` MUST be non-empty and SHOULD be valid UTF-8
5. `doc_cid` is optional auxiliary context only and MUST NOT be treated as the canonical governance truth surface
6. If `referenced_payload_hash` is present, it SHOULD identify a concrete payload that the domain is discussing, but the advisory payload itself remains non-executable
7. `Intent` and `L2SignalToL1` MUST NOT mint governance winner-memory or GovXP credit by default; a later revision may opt specific advisory flows into reward relevance explicitly, but the default contract is no credit

---

## 7. Submission, Control, and Public Participation

The governance contract MUST distinguish:

- Who may submit a proposal
- Who may cast votes
- Who may perform narrow recovery / override actions
- Who may execute privileged post-approval actions

The specification MUST state the intended control contract honestly:

1. Signed users SHOULD be able to participate in the public voting tracks enabled for their domain + payload-kind combinations
2. Proposal submission MAY remain payload-kind-specific and runtime-configurable, but combinations intended for public use SHOULD expose signed submission
3. Submission MAY charge a runtime-configured opening fee collected into the runtime Fee Sink as economic friction only; liveness, bounded-state, and anti-DoS claims MUST rely on structural domain, strategic-reserve, per-author, maturity, enactment, ballot, and per-block service bounds rather than the fee's configured amount or market value
4. If GovXP / reputation policy depends on proposal authorship, each live proposal MUST carry one explicit proposer / sponsor identity even when a privileged origin submits it on that account's behalf
5. Proposal-opening and proposal-success authorship counters MUST be recorded on-chain so a future explicit GovXP policy can consume them without archive reconstruction
6. Narrow admin recovery / override tools MAY exist, but they MUST be explicit and limited
7. Public governance UX MUST NOT imply powers that still belong only to admin/root

### 7.1 Strategic Proposal Ingress

A protocol `L1RootAction` MUST be creatable after bootstrap through the existing signed proposal transaction without requiring an externally reachable Root origin.

Contract rules:

1. Signed strategic submission MUST be admitted only for runtime-declared protocol / Native domain combinations; the current strategic surface contains `L1RootAction` and `Intent`
2. Signed protocol-domain `L1RootAction` and `Intent` admission MUST require a runtime provider to confirm that the signer has nonzero primary-track governance power for that domain; protection-track power MUST NOT satisfy this predicate
3. Signed `Intent` may remain public without primary-track eligibility only in tactical or other explicitly public non-protocol domains
4. Every signed proposal MUST reference an available preimage that passes the runtime's bounded payload-kind/domain validator before capacity or fee mutation; missing, oversized, malformed, trailing-byte, and incompatible payloads MUST fail with typed admission errors and exact pre-state
5. Administrative submission MAY retain hash-first bootstrap/recovery behavior, but it MUST pass the same item, domain, author, maturity, and capacity classifier and gains no signed-ingress bypass
6. The eligible signed path MUST retain the ordinary signed proposal opening fee, bounded active-proposal capacity, duplicate rejection, proposer identity, authorship counters, preimage contract, and transactional rollback behavior
7. Each domain MUST reserve `StrategicProposalReserve` slots inside `MaxActiveProposalsPerDomain`; general proposals stop at `MaxActiveProposalsPerDomain - StrategicProposalReserve`, while protocol-domain `L1RootAction` may consume the complete domain cap
8. One admission classifier MUST derive submission authority, required signer eligibility, capacity limit, author bound, opening-fee applicability, and rejection precedence before economic mutation; dispatch and bounded authority/fee views MUST consume that classifier rather than reimplementing policy
9. Rejection precedence MUST be signed authority, primary eligibility when required, signed preimage availability and compatibility, duplicate identity, domain/capacity lane, canonical author-index integrity, per-author capacity, and maturity-bucket capacity, followed only then by fee collection and proposal insertion
10. Admission MUST compute the exact maturity epoch and prove room in that bounded bucket before fee transfer; the later transactional insertion MUST use the admitted epoch, so no intervening policy or time recomputation can move the proposal to a different bucket
11. Strategic capacity classification MUST derive internally from the existing `(domain, payload_kind)` submission-authority binding; callers MUST NOT select a priority, lane, or proposal class
12. The reserve MUST be nonzero and strictly below the domain cap; it creates no second proposal collection or count cache, and every ordinary terminal path releases capacity by removing the proposal from the existing active set
13. One author MUST hold fewer than `MaxActiveProposalsPerAuthor` active proposals per domain before admission; the implementation MUST derive this bounded count from the canonical active-id and author records rather than persisting another count cache
14. A missing author record for any indexed active proposal MUST fail admission as an invariant error rather than undercounting occupancy
15. Every terminal path MUST use one shared cleanup operation that validates nonzero domain occupancy before removing canonical active state, removes the author and active-id entry once, and decrements the domain count exactly once; inconsistent occupancy MUST fail closed and roll back the terminal transaction
16. Enactment success or execution failure occurs only after proposal resolution has already performed this shared active-capacity release; pending enactment MUST NOT retain active admission capacity
17. An ineligible signer MUST fail before fee transfer, events, authorship mutation, active-count mutation, maturity indexing, or preimage requests
18. Submission grants only the right to create the bounded proposal lifecycle; it MUST NOT expose a direct Root dispatcher, arbitrary runtime call, or privileged control action to the signer
19. Successful enactment MUST remain restricted to the existing strategic `L1RootAction` payload executor, which accepts only its dedicated bounded payload contract and derives Root-equivalent execution internally after governance approval
20. `$VETO` remains protection power only: holding or voting `$VETO` MUST NOT satisfy primary submission eligibility or create positive agenda-setting authority
21. Sudo, genesis-only proposal fixtures, development Root accounts, XCM superuser conversion, rehearsal-only authority, and fabricated pending authorization state are non-conforming substitutes for this ingress

### 7.2 Proposal Payload and Execution Authority

Every proposal MUST bind to exactly one `GovernanceDomain`, one `CadenceMode`, one `ProposalPayloadKind`, and one `executable payload identity`.

Contract rules:

1. The payload MUST be typed and interpretable by the runtime; opaque uninterpreted bytes MUST NOT be the canonical governance payload contract
2. A conforming implementation MAY realize that payload as either `(a)` a bounded typed action enum, or `(b)` a preimage-backed runtime call identity, but in both cases the payload-kind semantics MUST be queryable before voting
3. Execution authority and dispatch origin MUST be derived deterministically from `GovernanceDomain + ProposalPayloadKind`
4. `Enacted` means the payload was actually dispatched successfully under that declared authority, not merely that voting approved it
5. Root-equivalent powers MUST only be reachable through `L1RootAction` payloads in the strategic / Native domain
6. `L2TreasurySpend` and `L2ParameterChange` payloads MUST enact only through domain-local authorities; they MUST NOT dispatch as Root
7. `Intent` and `L2SignalToL1` are non-executable advisory payloads; they MUST NOT dispatch privileged state transitions by themselves
8. If an L2 domain needs Root-equivalent action, it MUST use `L2SignalToL1` so the domain records and exports its will without directly executing super-user power
9. `L2ParameterChange` MUST target explicit domain-owned or explicitly delegated control surfaces; it MUST NOT be justified only by the existence of a convenient admin setter somewhere else in the runtime
10. If a mechanism still remains system-owned or L1-owned rather than explicitly delegated to that domain (for example a System Actors policy surface not yet delegated), the L2 domain MUST use `L2SignalToL1` or another explicit handoff surface instead of mutating that mechanism as though it were already domain-local
11. If enactment dispatch fails, the item MUST enter explicit `ExecutionFailed` state or an equivalent explicit failure status rather than pretending the proposal was enacted successfully
12. Execution MUST be transactional: a failed enactment MUST NOT leave partial multi-step side effects behind

### 7.3 Payload Preimage Admission Policy

A proposal always binds one `payload_hash`, while signed admission requires its exact bounded typed preimage to be available before capacity, fee, event, authorship, maturity, or active-state mutation.

Canonical policy:

1. Every signed payload kind, including `Intent` and `L2SignalToL1`, MUST pass the runtime's domain/payload-kind preimage validator before admission
2. Missing, oversized, malformed, trailing-byte, and domain-incompatible signed preimages MUST fail with typed errors and exact pre-state
3. A requested-but-unavailable preimage is not sufficient for signed admission
4. Administrative bootstrap/recovery submission MAY remain hash-first, but it MUST use the same item, author, capacity, and maturity classifier and gains no signed authority or fee bypass
5. Executable payload enactment MUST revalidate runtime-visible payload availability and compatibility because retained bytes or runtime policy may change after admission
6. Advisory payloads remain non-executable, but signed advisory authors MUST still bind voting to canonical typed bytes rather than an opaque hash
7. Query surfaces MUST expose noted/requested status so authoring clients can require the note step and inspection clients can describe administrative hash-first state honestly
8. Product UX and write adapters MUST block signed submission until the exact preimage is available

Rationale: hard signed admission gives one canonical typed proposal identity before economic mutation, while the explicit administrative exception preserves bounded bootstrap and recovery without weakening public authoring.

---

## 8. Runtime Upgrade Authority

Governance-driven runtime upgrades are part of the governance surface and MUST use an explicit payload kind and execution contract.

The governance system MUST NOT treat runtime upgrades as a hidden side effect of ordinary proposal approval.
Instead, a runtime-upgrade path MUST define:

- The `ProposalPayloadKind` that represents a runtime upgrade (`L1RootAction` in this minimal vocabulary)
- The approval / turnout / veto rules for that payload kind in the strategic / Native domain
- Whether that domain + payload-kind combination is expeditable under Section 3.3
- The authority that can execute `set_code` after successful approval
- The enactment rules for ordinary and urgent handling
- The recent finalized-outcome and observability surface for that payload kind

The runtime-upgrade payload SHOULD be a dedicated bounded structure carrying only `code_hash` rather than a generic arbitrary Root-call envelope.
Malformed upgrade payload bytes MUST fail explicitly as payload invalidity rather than pretending to be another supported Root action.
The governance system MUST be able to execute `set_code` itself after successful approval.
The TMCTOL standard MAY also grant `L1RootAction` one constitutional acceleration path: unanimous protection-track `Pass` over the full eligible `$VETO` supply may authorize immediate execution without waiting for a separate primary-track ballot, because that path is a tightly scoped runtime-upgrade exception rather than a general positive-governance lane for `$VETO`.
The contract requires governance-owned execution rather than external superuser dependence.

---

## 9. Governance Participation Export Contract

Governance participation memory is a secondary export, not the primary meaning of governance.
The pallet owns this bounded projection but MUST NOT callback into, schedule, or mutate downstream reward settlement.

Rules:

1. Only resolved governance outcomes may contribute winner memory
2. One governance item may contribute at most one counted winning point per eligible account within the live lookback horizon
3. The same resolved item MUST NOT be ingested twice while still economically relevant
4. If an account's rolling winning-memory sum reaches zero, its reward-memory storage SHOULD be evicted
5. The exported coefficient MUST remain normalized against bounded runtime-configured capacity, not against unbounded historical totals

This keeps governance participation memory sparse and economically relevant instead of archival.

---

## 10. Query Contract

The governance query contract MUST distinguish bounded canonical on-chain projections from indexed/materialized views.

`Canonical on-chain governance projections` SHOULD cover the live bounded UX surface:

- Active proposal / referendum state for known item identities
- Governance domain
- Cadence mode
- Proposal payload kind
- Executable payload identity and its queryable type surface
- Declared execution authority / dispatch origin identity for that domain + payload-kind combination
- Current lifecycle phase (`lead-in`, `primary open`, `urgent primary open`, `pending enactment`, `enacted`, `execution failed`, `rejected`, `veto-cancelled`)
- Protection-window and primary-window timing, including open/close timestamps or epochs
- Whether that domain + payload-kind combination is expeditable, whether urgent fast-track has been authorized, and when that authorization happened
- Current weighted tally for each live enabled track
- Current resolution state
- Runtime-declared vote-power profile identity for each live enabled track
- Recent finalized outcome inside bounded retention
- Veto-cancelled status when applicable
- Governance participation coefficient export
- Active proposer / sponsor identity when authorship is part of on-chain GovXP policy
- Recent execution result and execution-failure reason when enactment has already been attempted
- GovXP input counters (`rolling winning`, `cumulative total participation`, `cumulative total winning participation`, `cumulative authored proposals`, `cumulative successful authored proposals`)
- L2 TOL reserve state, LP custody state, and other bounded treasury/liquidity projections whenever governance authority depends on them
- Team-share or treasury vesting state when governance rights or withdrawal policy depend on that schedule
- DripVault or other bounded streaming state when treasury execution is time-fragmented by governance policy
- A bounded active / recent proposal index or equivalent query surface if proposal discovery itself is treated as canonical live product UX

`Indexed / materialized governance views` SHOULD carry the heavier surfaces:

- Full referendum archive
- Historical ballot timelines
- Search/filter across expired items
- Long-range participation analytics
- Operator dashboards beyond bounded recent state

The runtime MAY expose canonical projections through pallet storage, helpers, runtime APIs, or view functions, and MAY expose the heavier class through indexers/materializers, but the contract MUST be explicit enough that canonical live proposal discovery or status UX does not silently depend on ad-hoc storage reconstruction.

### 10.1 Event and Observability Contract

A conforming implementation MUST emit a bounded event surface sufficient for indexers, operators, and product clients to reconstruct governance lifecycle and execution truth without inferring it from ad-hoc storage archaeology alone.

At minimum, the event contract MUST cover:

1. `Proposal submitted` — domain, item identity, cadence mode, payload kind, proposer/sponsor, payload identity
2. `Vote cast or replaced` — domain, item identity, account, track, choice, vote-time epoch
3. `Urgent fast-track authorized` — domain, item identity, authorization epoch, protection `Pass` condition that triggered it
4. `Proposal finalized approved / rejected / veto-cancelled` — explicit final outcome family and bounded reason data when relevant
5. `Enactment scheduled` — domain, item identity, finalized epoch, enactment epoch
6. `Enacted successfully` — domain, item identity, payload identity, execution authority, success marker
7. `Execution failed` — domain, item identity, payload identity, execution authority, bounded failure reason
8. `Invoice executed` when an `L2TreasurySpend` payload pays out — beneficiary, payout asset, base amount, scalar, final payout amount, funding source
9. `Runtime upgrade executed` when an `L1RootAction` runtime-upgrade payload succeeds — code-update observability sufficient for operators and indexers to identify the upgrade event

The event contract MAY be finer-grained than this minimum, but it MUST NOT be poorer than this minimum.

---

## 11. Treasury and Tactical Domain Governance Contract

### 11.1 LP Custody, Floor Discipline, and Management Modes

If a tactical domain is backed by TOL LP, governance MUST keep that LP under protocol-controlled accounts.
At minimum the contract SHOULD remain able to represent these management modes:

- `Locked Mode`
  - LP withdrawal requires domain supermajority approval
  - LP withdrawal obeys any configured timelock
  - any declared strategic veto window still applies
- `Treasury Mode`
  - LP remains on the domain treasury balance under declared governance rules
  - governance controls relocation, fee activation, and strategic reuse under the domain constitution

Strategic constraints:

- LP MUST NOT leave protocol control except through governance-defined mechanisms
- Cross-parachain deployment, liquidity rebalancing, XYK fee activation, and multi-pair deployment MUST require explicit governance approval
- If a tactical domain depends on XYK validity or dust resistance, initial tactical liquidity SHOULD enforce a configured minimum Native floor (`L2_TOL_native >= configured_minimum_native`)

### 11.2 Treasury Execution Patterns

The governance contract SHOULD remain able to express at least these treasury-participation modes:

- `One-shot seed` — immediate strategic position
- `Per-block DripVault stream` — continuous support over configurable duration with per-block execution
- `Revenue recycling` — percentage of income automatically converts into the domain token or another governance-declared treasury asset

If treasury execution is fragmented through per-block streaming, the contract SHOULD preserve these properties:

- Policy declares total allocation, duration, and cadence in blocks
- Execution occurs as bounded repeated actions rather than one large transfer
- Each step still obeys ordinary safety rules (permissions, slippage/oracle constraints, accounting)
- Remaining allocation, elapsed schedule, and completion state stay queryable on-chain
- Smaller per-step notional MAY reduce extractable value, but MUST NOT be advertised as eliminating MEV entirely

### 11.3 Team Share and Treasury Configuration

Tactical-domain governance SHOULD remain able to configure these team-share and treasury surfaces:

- Team allocation anywhere from `0%` to `100%`
- Vesting duration anywhere from none to an arbitrarily long configured schedule
- Governance rights ranging across full voting, protection-only rights, or treasury/LP-mediated participation
- Transfer restrictions ranging from immediate transferability to explicit lock periods
- Immutable-at-launch versus later-governance-adjustable configuration

The contract MUST keep such choices explicit rather than hiding them inside one hardcoded launch model.

### 11.4 Tactical-Domain Bootstrapping and Rollout

The governance contract SHOULD distinguish at least two tactical-domain rollout lines:

- `Strategically authorized ecosystem tactical domains`
  - strategic authorization required
  - initial Native support sourced from strategic treasury allocation
  - emission model, share split, team-share policy, and governance schema declared at creation
- `Permissionless or threshold-gated tactical domains`
  - creator supplies the configured minimum Native threshold
  - the same governance-schema declaration requirements still apply

Operationally, a tactical domain SHOULD preserve these phases:

- `Genesis` — registration and configuration validation
- `Launch` — initial funding, first mint or distribution, pool activation, and governance enablement
- `Operation` — recurring mint, treasury, reward, and governance flows

Non-binding maturity heuristics MAY include liquidity depth, positive cash flow duration, participation rate, and the absence of repeated protection cancellations; such heuristics are product signals, not automatic constitutional truth by themselves.

## 12. Security and Guarantee Contract

### 12.1 Governance Attack Surfaces and Mitigations

The governance contract SHOULD remain analyzable against at least these attack families:

- `mint-whale or tactical capture` — mitigated by explicit primary/protection hierarchy above tactical execution
- `flash or late-stage governance` — mitigated by early-open protection, lead-in, and ballot-time Declining Power on ordinary tracks
- `protocol capture` — mitigated by the constitutional `$VETO` protection layer above strategic governance
- `proposal spam or agenda flooding` — structurally bounded by domain capacity, strategic reserve, per-author capacity, bounded maturity/enactment buckets, bounded ballots, and bounded servicing; the Fee Sink-collected opening fee adds economic friction but is not a liveness or anti-DoS premise
- `treasury drain or invoice fraud` — mitigated by domain/payload-specific thresholds, binary protection cancellation, and explicit proposal/invoice lifecycle rules
- `GovXP farming or reputation manipulation` — mitigated by storing bounded counters while excluding live vote-power amplification until an explicit contract change, and by keeping advisory payloads non-rewarding by default

For richer GovXP profiles, the contract SHOULD stay honest about specific threat families such as collusion farming, sybil delegation, and elite entrenchment rather than treating them as invisible implementation details.

### 12.2 Attack-Cost Modeling and Guarantee Boundary

A tactical-domain attack-cost model MAY be expressed as a bounded lower-envelope such as:

```text
Attack_cost = min(native_protection_supply * threshold, circulating_domain_token * time_weighted_price, native_market_cap * protection_quorum, bootstrap_native_threshold * native_price)
```

Such a model is useful only as a governance-security heuristic.
It MUST NOT be confused with an unconditional market guarantee.

`Hard guarantees` SHOULD include:

- Deterministic vote math for the declared governance schema
- Enforceable governance gates (thresholds, lead-in, enactment delay, protection paths, urgent handling rules) when encoded on-chain
- On-chain transparency for vote records, LP movements, treasury state transitions, and bounded streaming state
- Configuration-bounded behavior rather than unbounded hidden policy

`Soft guarantees` remain dependent on market and operational conditions, including:

- MEV resistance from micro-streaming
- Realized attack cost under changing liquidity and participation
- Stress-floor realization under adverse markets
- Long-run governance quality and parameter stewardship

## 13. Minimum Realization Surface

A conforming governance subsystem SHOULD realize the contract above through bounded runtime surfaces covering at least:

- Tactical-domain registration, activation, and bootstrap validation
- Proposal or invoice submission, vote casting, deterministic resolution, enactment scheduling, and explicit execution authority
- Typed payload or preimage-backed payload handling for governance actions
- Burned proposal-opening-fee handling for payload kinds that use it
- Governance-driven runtime upgrade execution, including `set_code`
- Invoice payout execution from declared funding sources
- Treasury actions, LP accounting, and streaming-state transitions
- GovXP counter updates at declared lifecycle boundaries
- The event/observability contract from Section 10.1

Realization-side hook semantics SHOULD stay explicit for at least:

- Vote-cast persistence needed for deterministic resolution and GovXP logic
- Post-finalization enactment-delay servicing and urgent-immediate enactment handling
- Protection and timelock outcomes
- Streaming step progression and treasury accounting

Conceptual storage SHOULD remain able to project at minimum:

- GovXP state per account
- Proposal or invoice voter metadata needed for reward and resolution logic
- Lifecycle timing state for protection, primary, and enactment phases
- L2 TOL reserve and LP tracking when governance controls those assets
- Voting-power decay schedules and other runtime-configured policy surfaces
- Treasury, team-vesting, and streaming state when those schedules affect governance authority

## 14. Conformance Boundary

A conforming implementation MUST preserve the lifecycle, track, vote-power, query, and boundedness rules defined above.
---

## 15. Non-Goals

This contract does not specify:

- Full permanent archival governance history inside one pallet
- Full GovXP mathematics, live GovXP vote-power integration, and soulbound identity implementation
- Blended invoice settlement; the canonical invoice path is discrete `Amplify / Approve / Reduce / Nay`
- A promise that all governance payload kinds become public-submittable in the immediate next runtime slice
- Conviction-style lock-for-weight mechanics — TMCTOL's liquid staking architecture (`stXXX` receipts) is structurally incompatible with hard token locks; the temporal-weighting role is already served by Declining Power, and commitment signals belong in later GovXP policy rather than in a separate lock multiplier
- Per-track delegation or proxy voting
- Proposal bonds or refundable decision deposits as the anti-spam contract; bounded structural admission/service limits are authoritative, with a Fee Sink-collected opening fee permitted only as additional economic friction
- Adaptive approval/support curves in any form

Those remain future evolutions or explicit exclusions, but any later extension SHOULD compose with the track, vote-power, lifecycle, and boundedness rules defined here.

---

_End of specification._
