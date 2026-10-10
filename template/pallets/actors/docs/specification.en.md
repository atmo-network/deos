# DEOS Actors Specification

- **Scope**: Bounded economic actor runtime contract
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase.

---

## 1. Ownership and conventions

This specification owns Actor semantics, the public ABI shapes listed in it, and the resource guarantees every conforming implementation MUST provide. It does not own:

| Surface | Owner |
| --- | --- |
| Exact SCALE encoding, discriminants, and type paths | Runtime metadata |
| Storage items, keys, internal records, and page geometry | Generated storage descriptors and the package architecture document |
| Measured `Weight(RefTime, ProofSize)` values | Generated Weight descriptors |
| Concrete host adapter trait signatures | The package embedding guide |
| A host's concrete block allocation, fixed envelope, and reference constants | That host's integration documentation |

Each semantic function has exactly one owning section. Other sections use it only by reference, for example `Opening (§6.2)`. If two passages conflict, the owning section wins.

Rust blocks declaring public types, calls, events, or errors are normative shapes. Other code blocks are semantic pseudocode: they fix branch order, dependencies, and failure propagation, not identifiers or Rust syntax.

---

## 2. Core contract

### 2.1 Invariants

1. Equal canonical state and block context MUST produce equal behavior.
2. Every path MUST be `O(1)` or `O(K)` under explicit finite bounds.
3. The complete `Weight(RefTime, ProofSize)` of a transition MUST be admitted before semantic mutation.
4. An Actor Contract is a bounded linear sequence of `0..=MaxContractSteps` Steps (§4.1).
5. Contracts contain no loops, jumps, nested contracts, opaque dispatch, Task-authored memory, or authored whole-pipeline rollback.
6. The service quantum is Q1: one Actor MAY commit at most one Step in one block (§7.1).
7. A Pipeline MAY span multiple blocks and is non-atomic across committed Steps (§7.1).
8. An admitted Pipeline MUST NOT undergo economic apoptosis before its Cycle boundary (§10.4).
9. A cause observed in block `N` cannot authorize Actor execution before block `N + 1` (§3.2).
10. User and System Actors share one Service ring and class-neutral encounter order (§3.2).
11. Trigger occurrence, Pipeline Opening, and Action execution are independent transitions (§5.2, §6.2, §7.1).
12. Trigger, Pipeline Machine, and Action work have disjoint economic owners (§8).
13. Close deletes process semantics and MUST preserve sovereign custody (§10.3, §11.3).
14. Observations never activate Actors; they are read only by Step predicates at the actual check or Attempt (§5.1, §7.2).
15. Unknown capability, stale authority, unbounded work, invalid actual Weight, or unknown downstream failure MUST fail closed.
16. Rejected control transitions restore Actors state, events, scheduler state, and Actors fee movement to pre-state. Host nonce and ordinary transaction-payment effects are outside this definition.

### 2.2 Atomicity terms

| Term | Meaning |
| --- | --- |
| Rejected control transition | Call returns `Err`; Actors-owned state and events equal pre-state. |
| Provisional Task commit | Task layer succeeded inside the current Step transaction but is not durable until that transaction commits. |
| Committed unsuccessful attempt | The current Step transaction durably commits suspension or failure; fees owned by that committed transition remain charged. |
| Rolled-back scheduler attempt | The current Step transaction fails; residence, run, Task effects, Actors fees, and Actors events equal pre-attempt state. |

---

## 3. Current-state service

This section owns residence, service order, block rounds, wake acknowledgment, parking, and generation safety. Removed historical amount forms, Trigger histories, Opening snapshots, deferred Cycles, queue tickets, and per-Step successor tickets are invalid encodings, not compatibility aliases.

### 3.1 Residence states

Each Active Actor owns exactly one generation-bound process. A serving process has exactly one residence:

| Residence | Canonical obligation | Permitted exit |
| --- | --- | --- |
| `Live` | One Service ring membership for an admitted Pipeline or an Idle Actor due for a current start check. | One metered turn may retain Live, move to Deadline or Parked, disable, or close. |
| `Pending` | One Service ring membership for paid readiness or an owed current activation check; no Cycle admitted yet. | False refreshes parking evidence; true opens a Cycle no earlier than the next round; resource refusal retains Pending. |
| `Deadline` | One known future eligibility: paid Idle readiness, a suspended retry or a timed review. | Due extraction returns the process to Service no earlier than the next round. |
| `Parked` | An Idle process with a generation-bound negative certificate for a parked-balance plan (§3.4). | A qualifying invalidation creates one coalesced `Pending` check. |
| `Disabled` | No ordinary residence: owner-paused authority, or an unlatched Idle process passively awaiting readiness. | Owner pause requires authorized control; Protocol Idle accepts a new useful Trigger through ordinary paid publication. |

A temporal Trigger (`AtTime`, `Cadenced`) MAY additionally own one independent Trigger deadline. That deadline is detector authority, not a residence, and cannot execute the Actor directly.

After completion or abort, Idle without a pending signal or Run MUST leave Service. Without a window it keeps Protocol Disabled process authority; an independent window expiry owns its Block Deadline. Temporal detector authority survives this placement, and a parked-balance plan owns its bounded current-state review and dependency episode. Passive waiting performs no recurring Actor-specific inspection or rent. A new useful Trigger restores ordinary paid readiness with B+1 admission; it is not owner pause or deactivation. Capacity and collector/invariant refusals still preserve their original owed work and head rather than silently becoming passive.

A useful Idle occurrence preserves its paid latch in one primary Block Deadline when known window/cooldown eligibility is later than `B + 1`; its key is the earlier of eligibility and independent window expiry. Otherwise it publishes Pending Service for `B + 1`. A due Deadline return still obeys the next-round rule. Known future eligibility MUST NOT cause repeated Actor-specific Service checks during the delay.

Park MUST refer to a live generation-bound parked-balance episode and its exact dependency owner. Its sole negative reason is `ParkedBalanceBelowThreshold`; missing episode state is an invariant refusal. Observation predicates, unavailable observation sources and passed observation boundaries MUST NOT create Park residences or bypass useful-readiness payment when returning to Service.

The read-only eligibility projection MUST report the authenticated current process residence: Service kind, Deadline key, Parked reason and optional review block, or Disabled. It MUST NOT infer residence from missing membership or expose a Trigger deadline as the residence. Financial waiting preserves its original unpaid temporal obligation independently of the current residence; readiness classification alone is not funding or service admission.

An open Cycle is Live when eligible by the next permitted round and in Deadline when eligibility is known later; it is never Parked. Unknown, stale, unavailable, or corrupt state cannot certify indefinite parking: it selects a typed deadline, deterministic timed review, or failure.

Every residence transfer is transactional: the complete transition commits, or the former residence remains unchanged. Exactly one source or destination obligation survives any failure. Every membership, wake, pending record, and deadline binds the exact generation; stale-generation work has no authority over a recreated Actor.

### 3.2 Block rounds and Service order

One block owns one immutable round identity shared by every Actor pass. The outcome MUST equal that of a snapshot of the ordered eligible membership taken at the round boundary, but implementations MUST NOT enumerate the ring to achieve it. A member receives at most one ordinary turn per round, and at most one Step commits for an Actor per block. A member admitted or reentered in block `B` is ineligible until `B + 1`. Removal never donates its turn; remove/reinsert and generation replacement cannot reset the same-block guards.

Admission fixes initial ring order; service then preserves cyclic encounter order among residents. A successful Step whose continuation is due next round retains membership and advances the Service cursor. Every newly admitted, reentered, or replacement membership is placed behind all current residents as encountered from the persistent next-encounter cursor. After `A` advances in `A -> B -> C`, admitting `D` before the next block yields next-round order `B -> C -> A -> D`.

Head, tail, and interior removal, Deadline transfer, parking, close, and additions preserve the encounter order of survivors. The valid Service head is authoritative: it cannot be bypassed, no cheap/heavy/System/User reordering exists, and when its complete transition does not fit, the pass stops and later residents remain untouched. Certified absence of current work is a distinct fully admitted transition and may detach the head. Empty, stale, already-served, and ineligible physical entries have explicit bounded traversal and cannot cause an infinite wrap.

Level-sensitive recurrence checks current state after Cycle completion no earlier than the next round and cannot start twice in one block. While an Actor is Running, in retry Deadline, Pending, or Live, repeated Manual or source hints coalesce and create no second Cycle, cursor reset, retry reset, or fee claim. Cadence misses coalesce into one current check without catch-up. One-shot temporal service has no recurrence after its completed or terminal Cycle.

### 3.3 Wake and acknowledgment

Invalidation records only that a recheck is owed; repeated updates coalesce. Registration, evaluation, and acknowledgment MUST be atomic or revisioned so an update before, during, or after evaluation cannot disappear: acknowledgment clears only the exact covered revision, and a later revision leaves the check owed. Pending saturation preserves one durable obligation. Disabled Actors and closed generations ignore ordinary wake hints.

Each transition pre-admits its complete multidimensional Weight and economic charge before mutation. Notification, current activation checks, residence transfer, Attempt and effect, resource refusal, and close are independently priced owners. No fee reserves future capacity or buys a second Cycle.

### 3.4 Parked-balance activation

`ParkedBalance` is an authored recurring activation mode for an Idle Parked generation. Its plan names at most `MaxParkedBalanceWatches` exact sovereign assets and a nonzero authored minimum delta for each. This host-selected bound is independent of Contract length and funding/source allowlists. It excludes `Cadenced`; periodic recurrence remains a separate authored mode. A running Cycle, retry, Live continuation, Pending check, Disabled Actor, or closed generation owns no parked-balance registration.

The watched quantity is the host ledger's authoritative total owned balance of the exact sovereign account and asset. It includes held or frozen ownership and is distinct from spendable `Available`, which remains the only basis for Task amounts. A hold or freeze change alone does not qualify. A host that cannot expose this quantity and its minimum balance coherently MUST reject the mode.

For each watched asset `a`, with checked or widened arithmetic:

```text
floor[a]     = 100 * minimum_balance[a]
threshold[a] = max(authored_min_delta[a], floor[a])
delta[a]     = abs_diff(total_balance_now[a], anchor[a])
qualified    = any(delta[a] >= threshold[a])
```

Movement in either direction qualifies, equality at the threshold qualifies, and quantities of different assets are never summed. A missing, zero, or unrepresentable minimum rejects admission; a later minimum or plan change requires authorized reconfiguration under a new configuration identity. There is no saturation, unit substitution, or price conversion.

Activation and reconfiguration perform one atomic arm without execution: after all mutations of that transition commit, the current total balances become the anchor and the Park registration is published. Earlier deposits are never replayed as activation. After each completed balance-recurring Cycle, all effects, fees, and reservation releases commit before one atomic anchor capture and Park publication, starting a new parking episode; no intermediate sample, notice, or failed check moves the anchor.

An already paid Idle readiness remains owed across reconfiguration: it retains Pending authority instead of acquiring Park registrations or paying another Trigger fee. Its completed Cycle arms the current plan after committed effects. This exception preserves the single paid latch (§5.3), not historical balance movement.

A qualifying change creates one generation- and episode-bound Pending check; repeated notices coalesce. A negative check returns to Park with the same anchor and acknowledges only its covered revision; a later revision creates one new check, and a revision racing evaluation remains owed. A qualifying change that later reverses remains an owed check, but current conditions and current `Available` are revalidated before Cycle admission and effects. Adapters that publish per-asset causal revisions after committed transfer, mint, and burn provide event-driven invalidation; every other mutation route is covered by mandatory timed review.

A positive current check commits one useful readiness occurrence under §5.2, charging the User fee for its generated balance-check/materialization owner under §8.3 or retaining System fee exemption. Payment, the latch, Park release and B+1 Service publication are atomic. Fee or resource refusal preserves the original episode, anchor and exact check authority; it creates no free readiness or new Cycle. This derived balance occurrence requires no authored Manual call; the configured Trigger retains its latching and re-arm rules (§5.3, §5.5).

Dependency-source storage MUST be bounded by current registrations rather than historical Actor churn. Removing a registration MUST reclaim any newly empty registration page with bounded work; removing the last registration MUST atomically retire the source identity, revision, scan membership and remaining topology without scanning source history. Re-registration of the same asset receives a fresh monotone source ID; retired IDs are never reused. Reclamation during an active scan MUST preserve its fixed frontier and every surviving registration's owed check. Cleanup retains the ordinary synchronous Close and physical admission obligations (§8, §10.3).

### 3.5 Classification

One pure classifier owns Active-state classification. It validates canonical partitions (§4.5), `CycleState` against the run (§6.1), and current Contract, run, and admission bindings (§4.4), computes stored-state terminal predicates (§10.2), and selects the execution phase in this order: circuit breaker, pause, terminal, block/retry/cadence-tick wait, signal wait, ready. The phases are `GlobalCircuitBreaker`, `Paused`, `WaitingRetry`, `WaitingBlock`, `WaitingCadenceTick`, `WaitingSignal`, and `Ready`.

Opening-specific fee and nonce checks belong to Opening (§6.2), not the classifier. Classification errors are typed and MUST NOT become absence, waiting, or `Ready` (§13.4).

---

## 4. Actor Contract and canonical state

### 4.1 Public Contract model

```rust
struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy, ParkedBalance, CloseWhen> {
  trigger: Trigger,
  cooldown_blocks: u32,
  window: Option<ScheduleWindow<BlockNumber>>,
  steps: Steps,
  funding: FundingPolicy,
  close_when: Option<CloseWhen>,
  on_unfunded: OnUnfunded,
  parked_balance_activation: Option<ParkedBalance>,
  auto_close_at_cycle_nonce: Option<u64>,
}

struct Step<Precondition, Task> {
  precondition: Option<Precondition>,
  task: Task,
  on_error: StepErrorPolicy,
}

struct ScheduleWindow<BlockNumber> {
  start: BlockNumber,
  end: BlockNumber,
}

enum OnUnfunded { Close, AwaitFunding }

enum InitialLifecycle { Dormant, Active }
```

`steps.len()` MUST be in `0..=MaxContractSteps`; zero Steps is first-class (§6.5). A present `parked_balance_activation` MUST hold a nonempty list of at most `MaxParkedBalanceWatches` strictly asset-sorted unique watches with nonzero authored minimums, a nonempty Step plan, absent `close_when`, and no cycle-nonce auto-close target; creation, activation, replacement, and decoding fail closed on any incompatible or noncanonical value.

`close_when` (§7.7) is the sole authored outcome-driven terminal policy. The former `completion` field, `CompletionPolicy` variants and productive-Cycle close reason MUST NOT remain as parallel authority or compatibility aliases. Absent `close_when` adds no outcome terminal; it preserves independently owned lifetime, failure and nonce terminals (§10.2). Authors MUST choose their Action or normal/clean Cycle condition explicitly rather than infer equivalence to the retired productive-Cycle rule. `on_unfunded` selects only the fee-native insolvency disposition in §10.7; it never selects Task-error continuation or a future Cycle. `AwaitFunding` is User-only; System admission MUST reject it because System Actors are Actors-fee-exempt.

Contract replacement replaces the complete authored value. Canonical equality is an exact no-op before rate limiting, cancellation, clocks, topology, writes, fees, or events.

### 4.2 Semantic identity

The semantic Contract ID is the protocol-fixed digest of the admitted canonical authored bytes only:

```text
SemanticContractId = Blake2_256(
  SCALE((
    b"DEOS_ACTOR_CONTRACT", // fixed [u8; 19]
    (
      trigger,
      cooldown_blocks,
      window,
      funding,
      completion,
      parked_balance_activation,
      auto_close_at_cycle_nonce,
    ),
    ordered_steps,
  ))
)

BodyCommitment = Blake2_256(
  SCALE((
    b"DEOS_ACTOR_BODY", // fixed [u8; 15]
    [(0u32, Step[0]), (1u32, Step[1]), ...],
  ))
)
```

Indexes MUST be contiguous and equal the exact Step count. Predicate canonicalization (§7.2) precedes identity derivation; whitelists are accepted only when already canonical (§5.1). Formatting, labels, storage wrappers, runtime state, topology, runtime Weight, and physical fragments MUST NOT affect either identity.

### 4.3 Contract storage guarantees

An admitted Contract is stored as a hot head plus a lazy tail and optional behavioral companions. The head holds compact scheduling/funding/lifetime fields, a fixed-size `close_when` presence discriminator, Step count, Step 0 when present, both identities, admission authority (§4.4), and the per-Step resource envelopes. Authored `close_when` bytes live in an optional Actor-keyed Contract companion; mutable monitor and required current-Cycle qualification live in an optional Actor-keyed runtime companion. The tail holds Steps `1..N` in bounded, gap-free, non-overlapping chunks keyed by their first Step index; zero- and one-Step Contracts have no tail.

1. Ordinary execution loads the head and, for Steps beyond 0, the one chunk holding the current Step. Policy-bearing execution additionally loads only its required bounded behavioral companions; absent `close_when` MUST NOT cause a policy/monitor lookup, including an absence read.
2. Unreached Steps MUST NOT add execution ProofSize merely because they exist, and `MaxContractSteps` MUST NOT enlarge the maximum current-Step read.
3. Full reconstruction happens only in lifecycle mutation, runtime projection, integrity checks, and upgrades.
4. Every fragment binds the Actor, semantic Contract ID, body commitment, and exact index range; missing, stale, overlapping, foreign, or orphan fragments fail closed.

The companion is part of the owning Contract, not another activation or scheduler owner. Full canonical projection MUST reconstruct the original authored `close_when` value, and physical placement MUST NOT change its semantic hash. A present discriminator with missing/stale policy or required monitor state is a typed invariant refusal, never implicit absence or reset. Companion installation, re-certification, replacement and destruction share the existing lifecycle transaction; episode preservation/reset follows §§7.7–7.9. Policy width MUST NOT enlarge shared no-policy head metadata or no-policy hot/Run hold capacity; constant selector cost and policy-bearing key/read/cleanup costs still need complete resource owners. Financial waiting retains its independent §10.7 ownership.

### 4.4 Admission authority and Pipeline envelope

Each admitted Contract carries a runtime-owned admission certificate binding at least the semantic Contract ID, body commitment, runtime semantics version, production Weight identity, storage geometry version, and configured bounds. It is derived authority, not authored state. A stale certificate fails closed until bounded re-certification or authorized Contract replacement; re-certification preserves semantic identity when authored bytes are unchanged.

Each Step carries one runtime-owned resource envelope with a maximum Actor Control Weight and a maximum Task-effect Weight, bound to its index, fragment, predicate and read geometry, Weight identity, and configured bounds.

Admission MUST reject a Contract whose largest reachable Service encounter cannot fit a fresh guaranteed base turn in either Weight dimension. Control includes common inspection, the current-Step/zero-Step owner, reachable placement and terminal cleanup, after reserving mandatory Prepass, both Deadline frontiers, bounded parked review and finalization. Task effects must independently fit the Actor effect base turn; idle borrowing or a combined `on_idle` reserve cannot establish this guarantee. Configured minimum domains belong to admission authority; a dynamic block allocation MUST preserve those Control and both economic base minima.

The Pipeline fee envelope is the generated maximum **reachable** chargeable Actor Control work of one complete admitted Pipeline. It includes Opening exactly once (§6.2), each reachable current-Step control path (§7.1), retry control up to authored bounds (§7.5), and the reachable Cycle-completion work (§6.6). It excludes Trigger work (§5.2), Task effects (§12.1), and final Actor-state destruction through Close or minimal apoptosis (§10.3–§10.4), whose economic owner is creation (§8.2). It MUST NOT be the sum of mutually exclusive Step maxima; Opening-only work is not multiplied by Step 0 retries and Cycle-finalization work is not multiplied across Steps.

`StopCycle` has no Temporary Task failure or unpaid Action liability, so its authored retry allowance adds no visits. A present precondition may skip it or produce a Permanent error handled by §7.5; an absent precondition always completes the Cycle and excludes later Step execution from pricing. The complete authored body, current-Step loading geometry, holds and destruction remain independently bounded and funded; reachability exclusion MUST NOT truncate those owners.

An admitted turn's Actor-specific Service discovery, loaded-state inspection and outer ring transition belong to that Pipeline's control price, independently of the inner current-Step host port. This does not back inspections before an Opening that never commits, or arbitrary repeated collector/invariant refusals.

Fee ownership does not reduce physical resource admission. Step and lifecycle Weight envelopes MUST still reserve and account for every reachable close branch in both RefTime and ProofSize. Chargeable Cycle control and creation-backed destruction require explicit non-overlapping cost owners; subtracting an unrelated public-close coefficient from a complete-Step measurement does not establish that partition.

Create and semantic update are the only envelope producers. Opening reads the fixed-size envelope in `O(1)` and MUST NOT load tail chunks to quote Pipeline service (§6.2, §8.4).

### 4.5 Canonical facts and partitions

```rust
type ActorId = u64;
type OwnerSlot = u8;
type SystemSovereignId = u64;

enum ActorType { User, System }

enum ActorClass {
  User { owner_slot: OwnerSlot },
  System { sovereign_id: SystemSovereignId },
}

enum Mutability { Mutable, Immutable }
enum ActiveLifecycle { Active, Paused }
enum CycleState { Idle, Running, Suspended }

struct OutcomeTotals {
  executed_steps: u32,
  committed_effectful_tasks: u32,
  precondition_skips: u32,
  skipped_resolution: u32,
  skipped_funding_unavailable: u32,
  failed_steps: u32,
}

enum SuspensionReason { FundingUnavailable, Temporary }
enum CycleResult { Completed, Failed, Cancelled }
```

Each fact has one canonical owner; internal record layouts are implementation-owned:

| Fact | Canonical owner |
| --- | --- |
| Owner, class, mutability, sovereign account, latest terminated Cycle nonce, last control-mutation block | Actor identity |
| Authored Contract, identities, admission authority, envelopes | Contract head and tail (§4.3, §4.4) |
| Lifecycle, cycle phase, Trigger runtime state, latch, failure streak, schedule anchor, last Cycle block, terminal marker | Actor hot state |
| Open-Cycle cursor, outcomes, retry state, and Contract binding | Run state (§6.1) |
| Generation and residence | Actor process (§3.1) |
| Service ring, deadlines, park registrations, and reverse handles | Bounded scheduler topology |

`ActorType` is derived from `ActorClass` and MUST NOT be stored. Composite views and runtime API projections are read-only. No persistent cache may duplicate authored fields, Steps, cursor, outcomes, lifecycle, or latch.

Dormant means only the identity and its slot or locator exist. Dormant Actors own no Contract, hot state, process, run, detector membership, residence, Trigger deadline, or Active-state hold. Public creation admits Dormant only as Mutable; a host genesis configuration MAY declare a sealed Immutable System identity that can never activate or close through Actor control.

---

## 5. Trigger machine

### 5.1 Trigger families and runtime state

```rust
enum Trigger<AccountId, AssetId> {
  Manual,
  AddressEvent {
    source_filter: SourceFilter<AccountId>,
    asset_filter: AssetFilter<AssetId>,
  },
  AtTime { after_ticks: u64 },
  Cadenced { every_ticks: u64 },
}

enum SourceFilter<AccountId> {
  Any,
  OwnerOnly,
  Whitelist(BoundedVec<AccountId, MaxWhitelistSize>),
}

enum AssetFilter<AssetId> {
  Any,
  Whitelist(BoundedVec<AssetId, MaxWhitelistSize>),
}

enum FundingProvenance { Signed, InternalProtocol, Xcm }

struct AddressEvent<AccountId, AssetId, Balance> {
  destination: AccountId,
  source: Option<AccountId>,
  asset: AssetId,
  amount: Balance,
  provenance: Option<FundingProvenance>,
}

enum TriggerRuntimeState {
  Stateless,
  AtTime { anchor_tick: Option<u64>, consumed: bool },
  Cadenced { anchor_tick: Option<u64> },
}
```

`Manual` and `AddressEvent` require `Stateless`, `AtTime` requires AtTime state, and `Cadenced` requires cadence state; a mismatch is an Actor invariant failure (§13.4). Whitelists MUST be nonempty, duplicate-free, and strictly ordered by canonical typed SCALE bytes; admission rejects rather than normalizes noncanonical input.

Observation publication is never a Trigger source. An observation-reactive strategy composes a `Cadenced` or `AddressEvent` Trigger with a fresh-only observation precondition (§7.2).

### 5.2 Useful Trigger occurrence

A Trigger occurrence is exactly one family-specific cause that:

1. Passes source and detector authority;
2. Is semantically relevant;
3. Observes `pending_signal == false`;
4. Successfully charges the User Trigger fee, or is System fee-exempt (§8.3);
5. Commits `pending_signal: false -> true`;
6. Disables further Actor-specific occurrence work until re-arm (§5.3);
7. Emits `TriggerOccurrenceProcessed` (§13.2).

An occurrence is not a Cycle and does not charge Pipeline service (§6.2, §8.4). Source publication, movement, or explicit-call work has its own owner (§8.6, §12.4); the occurrence owns only Actor-specific detection, matching, and materialization attributable to `false -> true`.

### 5.3 Latch and redundant activity

`pending_signal` is the sole readiness latch:

```text
false -- useful occurrence --> true -- Opening --> false
```

While `pending_signal == true`, source activity MUST NOT create another occurrence, charge another Trigger fee, queue another Pipeline, accumulate Trigger history, or change any run cursor, current-state basis, or sovereign balance; where practical the Actor is absent from or disabled in the relevant detector topology. Source-owned state MAY keep changing, and economic ingress MAY still update custody (§7.4, §12.4).

The latch bounds one Actor to at most one open Pipeline and one paid Idle activation; no deferred Pipeline request or source history exists. No call clears the latch directly: Opening consumes it and deactivation or close deletes it (§6.2, §10.3). Running or Suspended source activity creates no latch and never alters the cursor, retry, or eligibility.

### 5.4 Trigger underfunding and source advancement

User Trigger underfunding means the sovereign account cannot pay the current Trigger fee (§8.3). It MUST NOT create free polling or repeated charging of the same cause.

| Family | Underfunded result |
| --- | --- |
| `Manual` | Reject the call; no Actors state, event, or Trigger fee change. Ordinary signed transaction payment remains. |
| `AddressEvent` | The certified movement and independent funding semantics MAY commit; no latch or Trigger fee. The movement is not retried as a Trigger. |
| `AtTime` | A User Actor follows `on_unfunded`: `Close` consumes the source and selects `TriggerAdmissionInsufficient`; `AwaitFunding` preserves one owed occurrence (§10.7). System is fee-exempt. |
| `Cadenced` | The due point is skipped and the next future cadence point is installed; no latch or Trigger fee. |

Fee-collector failure is not underfunding. The Trigger charge reports `TriggerFeeCollectionFailed`, never `InsufficientFee`, when the collector rejects collection after capacity admission; the owning transaction preserves the exact source obligation and performs no occurrence mutation.

### 5.5 Trigger re-arm

Opening consumes the latch and re-arms the Trigger (§6.2):

| Family | Re-arm rule |
| --- | --- |
| `Manual` | Stateless and enabled. |
| `AddressEvent` | Stateless and enabled. |
| `AtTime` | Never re-arms; `consumed == true`. |
| `Cadenced` | Installs the first canonical cadence deadline strictly after the current authoritative tick; no catch-up. |

These rules also apply when Contract replacement preserves a latch acquired by the previous Trigger. A Running or Suspended Actor cannot acquire a latch for a second Cycle; after completion or abort, detector authority is derived again from current canonical state.

### 5.6 Causal cohorts

A causal cohort is a bounded ordered set of Actors affected by one shared Trigger cause, such as one Cadenced due tick. Cohort membership depends only on shared causal authority, never on Contract length, Actor class, Task kind, fee, or Weight.

An implementation MAY amortize source traversal, candidate classification, and occurrence materialization across a cohort when every member follows the same generated control branch. Cohorting MUST NOT reorder candidates, create Task-shape affinity, or merge per-Actor obligations: each Actor keeps its own Trigger fee, latch, residence, events, Pipeline fee, and Step outcome, and after Step 0 each nonterminal member follows independent Service scheduling. Unreached Pipeline length MUST NOT add first-reaction work (§4.3).

### 5.7 Family semantics and time bounds

- `Manual`: only the authorized owner may call `manual_trigger` (§13.1). A useful call is an occurrence (§5.2); a latched call is an exact Actor no-op that retains ordinary transaction payment (§8.3).
- `AddressEvent`: only certified positive non-self movement forms a cause (§12.4). Matching is independent of funding acceptance (§7.4). `SourceFilter::Any` accepts any source including absent; `OwnerOnly` requires a concrete source equal to the owner; `Whitelist` requires a concrete listed source. `AssetFilter::Whitelist` requires the exact listed asset.
- `AtTime`: `after_ticks > 0`. One relative consensus-time deadline that fires at most once, never catches up, and never re-arms.
- `Cadenced`: `every_ticks > 0`. Anchored at Active installation with ceiling quantization. A useful occurrence disables cadence detection until Opening re-arms it; missed latched-period points are forgotten.

`AtTime` and `Cadenced` use the host-configured consensus tick, disallow `cooldown_blocks` and `ScheduleWindow`, and MUST fit `MaxTemporalDelayTicks` (§14.3). Temporal arithmetic is constant-time and never iterates or replays missed points:

```text
first_due = ceil(now_millis / tick_millis) + period
next_due  = first canonical cadence point strictly greater than now_tick
```

At genesis, temporal anchors are uninitialized; the first temporal service after the timestamp inherent installs the full future deadline without creating readiness.

For signal-driven Triggers, block eligibility is:

```text
cooldown_anchor = last_cycle_block.or(schedule_anchor)

cooldown_eligible_at =
  schedule_anchor                                      for the first Cycle
  checked_add(cooldown_anchor, cooldown_blocks)        otherwise

signal_eligible_at(cause_floor) =
  max(cause_floor, cooldown_eligible_at, window.start when present)
```

A `ScheduleWindow` is inclusive. Validation requires `end > start`, representable `end + 1`, inclusive length `>= MinWindowLength`, current block `<= end`, bounded future start delay, and first possible eligibility `<= end`. `terminal_at = end + 1` (§10.2). Active installation sets `schedule_anchor = max(now, window.start)` for a future window, otherwise `now`.

### 5.8 Detection geometry

Detection is source-specific: `Manual` through the direct call, `AddressEvent` through the certified destination and filter path, and `AtTime`/`Cadenced` through the temporal deadline index. A universal Trigger index is forbidden.

Detector workers MUST preserve source revision and time order, inspect only exact affected candidates, admit complete multidimensional Weight before mutation, use bounded candidate and page counts, preserve one exact source obligation on refusal, and never skip corruption or let later source work overtake it.

---

## 6. Pipeline and Cycle

### 6.1 Cycle and run state

A run exists iff `CycleState` is `Running` or `Suspended`. `Running` has no suspension, `Suspended` has exactly one `SuspensionReason`, and `Idle` has no run. The run binds the exact Contract, admission identity, and paid Pipeline envelope, and holds the open Cycle nonce, the cursor (the sole current Step index), unsuccessful attempts at the cursor, last attempt and committed-Step blocks, the eligibility block, cumulative outcomes, and the last Step outcome. Completed-run history is not retained in consensus state.

A paid Opening whose first Action liability enters AwaitFunding retains a pristine Running cursor 0: no committed-Step block, last Step outcome, cumulative outcome or unsuccessful attempt exists. Its paid machine authority is the ordinary Run authority; restoration executes that Step without Opening or another Pipeline charge.

### 6.2 Opening

Opening is the only transition that consumes paid readiness into one Cycle. It MUST execute in this order:

1. Validate the Actor, latch, residence, Contract, admission, and Pipeline envelope (§3.1, §3.5, §4.4).
2. Apply Opening terminal checks (§10.2).
3. Validate that the active-installed maximum run-state hold is present (§8.2).
4. Check and charge the complete Pipeline Machine fee (§8.4).
5. Derive `run.cycle_nonce = identity.cycle_nonce + 1` without changing the identity nonce (§6.4).
6. Consume `pending_signal` (§5.3) and re-arm the Trigger (§5.5).
7. Emit `CycleStarted` (§13.2).
8. For a nonempty Contract, process Step 0 in the same current-Step transaction (§7.1), including its Action-liability AwaitFunding transition (§10.7); for a zero-Step Contract, finalize atomically (§6.5).

Opening MUST NOT partially occur. Insufficient Pipeline payment follows `on_unfunded`: `Close` invokes minimal apoptosis with `CycleAdmissionInsufficient` (§10.4), while `AwaitFunding` preserves paid readiness and the unpaid Opening (§10.7). Inconsistent hold authority fails closed as an Actor invariant; the prior Trigger fee remains final (§8.3). A Pipeline fee-collection failure despite valid capacity rolls back the entire Opening, keeps the latch consumable, and preserves the Service head.

### 6.3 Current Attempt inputs

Cycle admission captures no predicate result, amount basis, or funding history. Each Attempt loads only its current Step and reads its predicates and live inputs from current authoritative state after fee reservation; retries repeat those reads. `Fixed(value)` is checked against current capacity without clipping, and `Percent(perbill)` uses widened floor arithmetic over current `Available`. Transfer additionally supports the bounded cycle-balance distribution forms (§11): the first capture Attempt freezes one asset/volume pair, and later Attempts retain that basis while checking current capacity. Every debit preserves the current Action-fee reservation and protected minimum (§8.5, §11.4).

### 6.4 Cycle nonce

The identity nonce is the latest terminated Cycle nonce. Opening derives `run.cycle_nonce = checked_add(identity.cycle_nonce, 1)` and does not mutate the identity nonce. Completion, failure, cancellation, or close assigns `identity.cycle_nonce = run.cycle_nonce` exactly once. Retry, suspension, Step progress, breaker deferral, and congestion change neither nonce.

### 6.5 Zero-Step Cycle

A zero-Step Contract has no Task, Step event, tail, or run. A ready zero-Step Cycle consumes one Service encounter and atomically performs Opening (§6.2), charges the zero-Step Pipeline Machine fee (§8.4), emits `CycleStarted`, commits the new nonce (§6.4), emits `CycleSummary(Completed)`, and applies terminal policy (§10.2). `auto_close_at_cycle_nonce = Some(1)` makes a fresh zero-Step Contract one-shot.

### 6.6 Completion and cancellation

A Cycle completes at plan end or on successful `StopCycle` (§7.6). Finalization MUST record the final Step outcome when present (§7.5), emit the boundary events (§13.2), commit the run nonce (§6.4), delete the run while preserving the active-installed run-state hold (§8.2), update `last_cycle_block`, apply terminal precedence (§10.2), and otherwise return to Idle and recompute the Idle residence. Source activity that occurred while Running or Suspended creates no second Cycle or deferred readiness.

Cancellation deletes the run without compensating earlier committed Steps: it emits `CycleCancelled`, then `CycleSummary(Cancelled)`, commits the run nonce, and deletes the run. Contract replacement, deactivation, explicit cancellation, close, and incompatible upgrade own their cancellation reasons (§10.3, §10.5, §14.2).

---

## 7. Step execution

### 7.1 One-Step attempt transaction

An Attempt is one admitted execution of exactly one current Step; a retry is a later Attempt at the same cursor. Before reading any predicate or dynamic amount, the scheduler MUST reserve the current Step's maximum Actor Control Weight and maximum Task-effect Weight (§9.1) and, for User Actors, the maximum current Action fee (§8.5). If any reservation does not fit, the Attempt defers without evaluation or mutation.

One current-Step transaction atomically owns:

```text
generation/process/Service/Contract validation
+ optional Opening for cursor 0
+ current-Step precondition
+ amount resolution
+ at most one Task effect
+ Action fee settlement when a Task is invoked
+ Step outcome and counters
+ cursor, suspension, completion, or close transition
+ retained Service, Deadline, Parked, Disabled, or close placement
+ Actors events
```

A Task MUST NOT be split across blocks, and earlier committed Steps are never rolled back by later failure. After a committed Step, `last_committed_step_block` equals the current block; a second Step commit for the same Actor in that block MUST fail closed independently of residence correctness.

An advancing Step with a successor sets eligibility to `current_block + 1` and retains one Live Service obligation (§3.1); it does not load or execute the successor. A retry suspension atomically moves the same process from Service to its exact Deadline.

### 7.2 Precondition

```rust
struct Precondition<P, MaxClauses, MaxPerClause> {
  clauses: BoundedVec<BoundedVec<P, MaxPerClause>, MaxClauses>,
}
```

```rust
enum PredicateError { InvalidObservation }

enum Predicate<AssetId, Balance, BlockNumber, FeedId> {
  BalanceAbove { asset: AssetId, threshold: Balance },
  BalanceBelow { asset: AssetId, threshold: Balance },
  BalanceEquals { asset: AssetId, threshold: Balance },
  BalanceNotEquals { asset: AssetId, threshold: Balance },
  BlockNumberAbove { threshold: BlockNumber },
  BlockNumberBelow { threshold: BlockNumber },
  ObservationAbove { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationBelow { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationNotEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
}
```

A present Precondition is nonempty DNF: outer clauses are OR, inner predicates are AND. Admission canonicalizes predicates and clauses by canonical typed SCALE order, rejects duplicate clauses after predicate deduplication, and absorbs exact supersets under `A OR (A AND B) = A`.

Every admitted predicate is evaluated with no short-circuit, because Weight MUST be data-independent. Evaluation happens immediately before the owning Step and observes prior committed Steps plus intervening external state.

`Above` is strict `>` and `Below` is strict `<`. An `Unavailable`, `Uninitialized`, or `Stale` observation evaluates false; a structurally invalid `Fresh` observation is `PredicateError::InvalidObservation`, a Permanent Step failure. `max_age_blocks` MUST be positive. Balance predicates read only the Actor sovereign account, subtract the current Action-fee reservation for the fee-native asset, and grant no spending authority (§8.5, §11.4). A false Precondition emits `StepSkipped(PreconditionFalse)` and advances (§7.5).

### 7.3 Amount resolution

```rust
enum AmountResolution<Balance> {
  Fixed(Balance),
  Percent(Perbill),
  CaptureCycleBalance { percent: Perbill, share: Perbill },
  CycleBalancePercent(Perbill),
}
```

Each Task amount resolves to `Resolved(positive value)`, `Skipped`, or `FundingUnavailable`:

- Percentages use widened floor division, and a dynamic zero is `Skipped`;
- A positive exact debit above current capacity is `FundingUnavailable` and MUST NOT be silently reduced;
- `Percent(100%)` resolves the whole current available amount independently at each Attempt and has no lifecycle privilege. The cycle-balance forms below retain a captured distribution basis instead.

For multiple amount fields: any `FundingUnavailable` gives `FundingUnavailable`, else any `Skipped` gives `Skipped`, else the Task is executable with all values.

| Resolution surface | Tasks | Live capacity |
| --- | --- | --- |
| Preserve-source | Transfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | current preservable balance |
| Output-target | Mint, SwapOut | authored output; percentage amounts forbidden |
| Share-spend | Unstake | current staking shares |

### 7.4 Funding authorization

```rust
enum FundingSourcePolicy<AccountId> {
  OwnerOnly,
  SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>),
  RuntimePolicy,
  AnyVerifiedIngress,
}
```

A positive certified credit is authorized only when the funding policy accepts its source and provenance:

| Policy | Accepted source |
| --- | --- |
| `OwnerOnly` | signed provenance and concrete source equal to owner |
| `SignedAllowlist` | signed provenance and concrete allowlisted source |
| `RuntimePolicy` | the configured authority accepts the exact source/provenance pair; all-`None` is denied |
| `AnyVerifiedIngress` | a concrete source or typed provenance exists; all-`None` is denied |

Rejected credit remains custody only; Trigger matching is independent (§5.7). Actors retain no amount accumulator or funding history: every later amount resolution reads current `Available`. Close and deactivation move no custody (§10.3).

### 7.5 Step outcome and error policy

```rust
enum StepErrorPolicy {
  AbortCycle,
  ContinueNextStep,
  RetryLater { max_attempts: u32 },
}

enum FundingUnavailableReason { ServiceFee, TaskInput }

enum StepSkippedReason {
  PreconditionFalse,
  ResolutionSkipped,
  FundingUnavailable(FundingUnavailableReason),
}

enum StepOutcome {
  Executed,
  Stopped,
  Skipped(StepSkippedReason),
  FundingUnavailable(FundingUnavailableReason),
  Failed(TaskFailure),
}
```

`RetryLater.max_attempts` counts the first unsuccessful execution and all retries and MUST be in `2..=MaxRetryAttempts`.

`FundingUnavailableReason::ServiceFee` means the User cannot reserve the exact current Action liability above its protected fee-native minimum. `TaskInput` means the current Task's required assets or staking shares are unavailable after preserving that liability. A nonzero User Action liability MUST be checked before Task input resolution; a false precondition creates no Action payment obligation. System Actors cannot produce `ServiceFee`. Funding source belongs to the latest Step outcome or skipped receipt; `SuspensionReason::FundingUnavailable` remains the undifferentiated continuation tag rather than duplicating the source in Run storage. Neither source is an invoked Action failure (§7.7); financial-wait selection uses only `ServiceFee` (§10.7).

Each step whose enclosing scheduler attempt commits selects exactly one row from the closed transition table:

| ID | Step result | Policy | Durable transition |
| --- | --- | --- | --- |
| `ST-01` | Precondition false | any | Record skip; advance or complete. |
| `ST-02` | Resolution `Skipped` | any | Record skip; advance or complete. |
| `ST-03` | `FundingUnavailable` | `AbortCycle` or `ContinueNextStep` | Record funding skip; advance or complete. |
| `ST-04` | `FundingUnavailable` | `RetryLater`, below bounds | Suspend same cursor. |
| `ST-05` | `FundingUnavailable` | `RetryLater`, local or global bound reached | Record terminal failure. |
| `ST-06` | Effect success | any | Record effectful success; advance or complete. |
| `ST-07` | `StopCycle` success | any | Record stopped; complete immediately. |
| `ST-08` | Temporary failure | `ContinueNextStep` | Record failure; advance or complete. |
| `ST-09` | Temporary failure | `AbortCycle` | Record failure; terminate Failed. |
| `ST-10` | Temporary failure | `RetryLater`, below bounds | Record failure; suspend same cursor. |
| `ST-11` | Temporary failure | `RetryLater`, local or global bound reached | Record terminal failure. |
| `ST-12` | Permanent failure or predicate error | `ContinueNextStep` | Record failure; advance or complete. |
| `ST-13` | Permanent failure or predicate error | `AbortCycle` or `RetryLater` | Record failure; terminate Failed. |

A step increments at most one primary outcome counter. The deltas are exact: precondition skip increments `precondition_skips`; resolution skip increments `skipped_resolution`; advancing funding skip increments `skipped_funding_unavailable`; successful effect increments `executed_steps` and `committed_effectful_tasks`; successful `StopCycle` increments `executed_steps`; invoked Task failure increments `failed_steps`.

`FundingUnavailable` is not a Task failure, and `AbortCycle` aborts on Task failure only. A retry suspension increments the cursor-local and global unsuccessful counters exactly once; the local bound wins when both are reached by the same Attempt. A terminal Failed Attempt increments the global streak once; a `ContinueNextStep` failure, deferral, pause, cancellation, exact no-op, and advancing funding skip do not. A completed Cycle resets the global streak.

```text
next_local  = previous suspension at same cursor ? previous + 1 : 1
next_global = unsuccessful_attempt_streak + 1
backoff(i)  = min(2^i, 8) blocks

retry_eligible_at =
  last_attempt_block + max(cooldown_blocks, backoff(next_local - 1))
  bounded by [window.start, terminal_at] when a ScheduleWindow is present
```

A retry suspension inside an active window therefore wakes at the window terminal rather than sleeping past it, and the persisted run eligibility equals the exact Deadline destination.

### 7.6 Task semantics

```rust

enum InputLimit<Balance> { LiveQuote, Absolute(Balance) }

enum Task<AccountId, AssetId, Balance> {
  Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
  SwapIn { asset_in: AssetId, amount_in: AmountResolution<Balance>, asset_out: AssetId, slippage_tolerance: Perbill },
  SwapOut { asset_out: AssetId, amount_out: AmountResolution<Balance>, asset_in: AssetId, input_limit: InputLimit<Balance>, slippage_tolerance: Perbill },
  AddLiquidity { asset_a: AssetId, asset_b: AssetId, amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance>, min_lp_out: Balance },
  RemoveLiquidity { lp_asset: AssetId, asset_a: AssetId, asset_b: AssetId, lp_amount: AmountResolution<Balance>, min_amount_a: Balance, min_amount_b: Balance },
  Burn { asset: AssetId, amount: AmountResolution<Balance> },
  Mint { asset: AssetId, amount: AmountResolution<Balance> },
  Stake { asset: AssetId, amount: AmountResolution<Balance> },
  DonateLiquidity { asset_a: AssetId, asset_b: AssetId, max_amount_a: AmountResolution<Balance>, max_ratio_error: Perbill },
  Unstake { asset: AssetId, shares: AmountResolution<Balance> },
  StopCycle,
}
```

- At most one Task effect is invoked per Attempt (§7.1), through the canonical host operation (§12.1).
- `Mint` is System-only. Self-transfer is invalid.
- Each Task contains at most two `AmountResolution` fields, and every debit preserves the protected minimum (§11.4).
- `Transfer(Percent(100%))` has no close-specific privilege.
- Outcome-policy observations belong to the committed Step/Cycle boundary (§7.7), not individual adapter legs.

Distribution uses separate `Transfer` Steps. `CaptureCycleBalance { percent, share }` captures `floor(percent × current preservable balance)` on the first attempted distribution Step, then transfers `floor(share × captured volume)`. `CycleBalancePercent(share)` uses the same volume; absence after a skipped capture gives `Skipped` and never recaptures. Admission permits one capture per Contract, positive percentages/shares, same-asset reuse strictly after capture, and cumulative shares at most one. These forms are Transfer-only. One optional asset/volume pair lives inside the current Run, survives retry unchanged, and disappears with Run on every completion, abort, reset or Close. Current capacity and fee reservation are checked independently at every Attempt. Inter-Step ingress does not change captured amounts; rounding remainder stays in custody. Every Transfer invokes the host's exact `preflight_transfer` before ledger mutation; a temporary deposit refusal follows the authored retry policy. Each Transfer commits independently and invokes at most one recipient's certified ingress, including synchronous terminal finalization (§12.4).

`SwapIn` and `SwapOut` use current executable quotes inside the adapter boundary (§12.4); `InputLimit::Absolute` is a cap, not an admission gate. `AddLiquidity` amounts are debit caps and actual used amounts and LP output are returned; `RemoveLiquidity` debits the exact resolved LP amount. `DonateLiquidity` uses one authored asset-A cap plus one current derived cap on pre-existing asset-B debit; the adapter may acquire asset B within the atomic operation, so returned donated B may exceed that debit cap.

---

### 7.7 Outcome-policy language

An outcome close expression is nonempty bounded DNF. Outer clauses are OR; inner atoms are AND. The only atom is `metric >= positive_threshold`, with a fixed-width `u32` threshold in `1..=u32::MAX`. Absence of an expression means no outcome-policy terminal condition, not an empty expression that evaluates true.

```rust
enum BehaviorMetric {
  PreconditionPassTotal, PreconditionPassStreak,
  PreconditionRejectTotal, PreconditionRejectStreak,
  ActionSuccessTotal, ActionSuccessStreak,
  ActionFailureTotal, ActionFailureStreak,
  CycleCompletedTotal, CycleCleanTotal,
}

struct BehaviorAtom { metric: BehaviorMetric, threshold: u32 }

struct CloseWhen<MaxClauses, MaxAtomsPerClause> {
  clauses: BoundedVec<BoundedVec<BehaviorAtom, MaxAtomsPerClause>, MaxClauses>,
}
```

Both collection limits MUST be positive and their product MUST fit `u32`; the host MUST also qualify encoded size and complete validation/evaluation Weight before admission. The ten metric discriminants follow the declaration order. Unknown metrics, zero thresholds, empty expressions or clauses, and overlimit encodings MUST be rejected. Arbitrary arithmetic, NOT, equality, counter-to-counter comparison, time windows and authored destinations are outside this language.

A clause MUST contain unique metrics in declaration order. Clauses MUST be unique and strictly lexicographically ordered by their `(metric discriminant, numeric threshold)` sequences. A clause that implies another clause MUST be rejected as redundant when every atom in the weaker clause has a same-metric atom with an equal or greater threshold in the stronger clause. This is bounded threshold absorption only; no cross-metric theorem prover or satisfiability minimizer is required. Authoring MAY produce this normal form, but stored/admitted policy validation MUST reject noncanonical values rather than silently reinterpret their identity.

Evaluation MUST visit every atom in canonical order, without value-dependent short-circuiting. It reads stable policy metric values and causes no external observation reads, Task invocation, scheduler transition or side effect by itself. Validation, compilation, evaluation and terminal application have separate bounded resource owners.

One precondition observation is the boolean result of the complete present Step precondition, never an individual predicate leaf. Absence and evaluation error produce neither pass nor reject. One Action observation belongs to one invoked logical economic Task, irrespective of its internal recipients or route hops; `StopCycle` produces none. A successful Task counts only when its enclosing Actor transition commits. An invoked typed Task failure, including rejected effect postconditions, counts when failure handling commits even if the Task-local effects roll back. Skips, pre-invocation funding outcomes, unpaid service and capacity refusal produce no Action observation; enclosing rollback produces no durable observation in either stream.

Totals accumulate within one installed behavior episode. Each stream's streak counts consecutive results in that stream: one outcome resets the opposite streak, while absence changes neither. Cycle boundaries, scheduler deferral, financial waiting, top-ups and preserved control transitions do not reset a streak. Semantic replacement cancels the open run before starting a new episode; exact no-op and technical re-certification preserve the episode. Deactivation ends the episode, and reactivation starts a new one without importing historical observations. Generation changes alone do not define episode semantics.

`CycleCompletedTotal` counts one committed normal Cycle completion: actual plan end, successful `StopCycle` at any position, or zero-Step completion. Abort, cancellation and policy closure before normal completion do not count. `CycleCleanTotal` additionally requires actual plan-end traversal, at least one successful economic Action, successful execution of every economic Step, and no failed Task attempt, predicate error or skipped economic Step in that Cycle. Failed-then-successful Task retry is not clean; financial waiting with no Task result is not a failure. Early return with an unvisited tail and empty/control-only Cycles are not clean. Derive both at their actual boundary; neither is a quotient of Action totals or an inference from a cross-Cycle streak.

Only referenced policy metrics need retained accumulators. Each retained value MAY cap at its metric's largest referenced threshold, preserving `>=` truth and opposite-streak resets; capped state MUST NOT be exposed as exact lifetime telemetry. The policy is evaluated against the committed candidate Step result and any Cycle completion it causes, never between a passed precondition and its permitted Action. A selected terminal transition MUST commit atomically with that result and MUST NOT roll back earlier committed Steps or recursively count its own cleanup events.

### 7.8 Derived policy requirements and binding

Compilation MUST consume a validated canonical expression. Its metric dictionary MUST contain exactly the referenced metrics, each once in declaration order, with a `u32` cap equal to that metric's largest referenced threshold. The dictionary is bounded by the ten-metric catalog, not by Contract length. Unreferenced metrics need no episode accumulator; absent `close_when` needs no outcome-policy plan or accumulator.

`CurrentOutcome` tracking is permitted only for a pure disjunction of singleton threshold-1 clauses. Its first satisfying observation closes the episode, so episode accumulators are unnecessary; this does not eliminate necessary current-Cycle clean-qualification flags. Every other expression uses `Accumulated` tracking. No general boolean minimization or assumption that conjunctions mature in one Step may remove their history.

A derived plan MUST bind the canonical policy bytes and the validated clause/atom limit snapshot to the owning semantic Contract ID and applicable certified admission identity (§4.4). The binding inputs MUST come from the canonical Contract/certificate, not from author-supplied assertions. The policy requirement digest is `blake2_256(SCALE((b"DEOS_ACTOR_BEHAVIOR_PLAN", binding, (max_clauses, max_atoms), expression)))`, where `binding` encodes the 32-byte semantic Contract ID followed by the 32-byte admission identity. Limits are fixed-width `u32` values captured during validation. Successful codec decoding alone MUST NOT authorize derived fields; verification MUST establish both this identity and the complete derived dictionary/tracking mode against the validated source.

Binding validity and behavior episode identity are distinct. Technical re-certification MUST rebind derived authority as necessary while preserving the episode's values (§7.7); it MUST NOT create a semantic replacement or replay past observations. Actual storage placement, holds, Step/evaluation geometry and complete resource admission remain independently bounded owners, not consequences of this digest or an encoding maximum.

Read-only eligibility MUST distinguish absent `close_when` from an authenticated policy episode. A present episode reports its lifecycle-owned start generation, tracking mode and canonical referenced metric/cap dictionary. Its optional monitor MUST distinguish stateless CurrentOutcome from required accumulated or clean-Cycle state. Accumulated values report only authenticated capped slots, never exact lifetime totals; CurrentOutcome reports no historical counters. Current clean flags, when present, retain their authenticated Run nonce and qualification state. Missing, foreign or malformed required state MUST refuse; projection MUST NOT allocate a monitor, replay observations, evaluate closure or mutate the episode.

### 7.9 Episode state and candidate reduction

The monitor MUST bind to the validated policy requirements and the lifecycle-owned behavior episode. For accumulated tracking, retained slots MUST correspond exactly to the plan's referenced metrics in canonical order, each value in `0..=cap`. Current-outcome tracking MUST retain no episode counter slots. A stale binding, wrong episode, missing/extra slot or above-cap value is a typed invariant refusal, never absence or an implicit reset. Physical episode tags and storage placement remain implementation-owned; the host MUST authenticate the expected episode from canonical lifecycle state rather than accept an author's assertion.

Reduction MUST propose state without separately committing it. The current complete Step's precondition and Action results and any already qualified normal Cycle completion are one input, not independently applied subtransitions. A rejected precondition cannot accompany an invoked Action, and an invoked Action failure cannot accompany a clean-Cycle completion. Cycle qualification remains owned by the run/Step boundary (§7.7), not by a counter reducer that infers it from totals. The enclosing Actor transaction alone owns commit/rollback and consuming a satisfied condition through the single terminal reducer; discarding a proposed result preserves the committed prefix and all old monitor values.

Technical re-certification MAY replace the derived plan binding without replacing the episode only when semantic Contract identity, canonical expression, metric/cap dictionary and tracking mode remain unchanged. Otherwise rebind MUST refuse and the lifecycle owner MUST perform the authored semantic-replacement/reset transition. Re-certification, pause, funding and scheduler transitions MUST NOT be used as synthetic Step observations or periodic policy evaluation.

---

## 8. Economics

### 8.1 Economic surfaces

Actors has four service-fee boundaries and one refundable state resource:

```text
committed User creation        -> ActorCreationFee
useful Trigger occurrence      -> TriggerFee
Pipeline Opening               -> PipelineMachineFee
invoked Action attempt         -> ActionExecutionFee
retained User Actor state      -> ActorStateHold
```

Ordinary transaction payment remains owned by the host. Task-native protocol fees remain owned by their mechanism (§12.1). System Actors are exempt from Actors Creation, Trigger, Pipeline, and Action fees but consume identical block resources; Task-native fees MAY still apply. Every Actors fee transfer is ledger-only and MUST NOT create AddressEvent ingress, funding, readiness, or placement.

### 8.2 Creation fee and state hold

User creation pays for physical Actor-state installation and its eventual complete removal. The host transaction fee owns the create-call execution; `ActorCreationFee` is the fixed nonrefundable process-admission, anti-spam, and lifetime-cleanup charge, paid by the signed creator only when User creation commits. It is not a second payment for create-call Weight and prepays no Trigger, Pipeline execution, or Action service.

The committed Creation Fee economically backs complete Close (§10.3), including minimal apoptosis (§10.4), across the admitted User lifecycle, not only pre-Opening failure. Final state destruction MUST NOT be charged again at Pipeline Opening, retry, or automatic closure, and MUST NOT depend on the Actor's later solvency. The obligation survives a later cleanup cost exceeding the historical fee. Creation and later activation or Contract replacement MUST stay within the host's bounded lifetime-cleanup domain; a larger refundable state hold alone does not pay for additional cleanup work.

Prepayment is an economic obligation, not stored future block capacity, a per-Actor fee escrow, or permission for unmetered work. Cleanup consumes current-block Actor Control Weight when executed (§9). Ordinary explicit-control transaction payment remains host-owned (§10.3); refundable state backing remains separate from this nonrefundable fee.

`ActorStateHold` is a refundable hold on the User owner account backed by retained geometry and bounded capacity for its reachable autonomous states; it is not a fee and not rent:

- Elapsed time does not change it and no recurring collection exists;
- Zero- and one-Step Contracts MUST NOT reserve a maximum-size body footprint;
- Active installation reserves one type-derived maximum run-state hold before autonomous service becomes possible, never a hand-maintained byte constant;
- Opening, Step progress, retry, suspension, Cycle boundary, and cancellation MUST NOT mutate or release that run-state hold;
- A Cadenced Trigger reserves one type-derived maximum temporal pointer while Active; an AtTime Trigger reserves it until its occurrence is consumed. Removing detector membership for a latch or financial wait MUST NOT release capacity still needed by that Trigger. Autonomous rearm MUST NOT require additional unheld owner funds;
- An AwaitFunding Contract reserves one type-derived maximum financial episode record before autonomous service; publishing or retiring an episode MUST NOT charge a new owner hold;
- Deactivation releases Active Contract, topology, funding, and run holds atomically and retains only the Dormant identity hold; close releases every Actor-state hold (§10.3);
- Inability to reserve rejects creation, activation, or replacement atomically;
- Sovereign custody is not Actor state and is never held by this mechanism (§11.3).

System state is host-owned bounded capacity and MAY be hold-exempt; exemption grants no resource or scheduler preference.

### 8.3 Trigger fee

```text
TriggerFee = WeightToFee(maximum generated control Weight of one useful family-specific occurrence)
             + independently qualified dedicated wait-work backing, when configured (§8.7)
```

A User Trigger fee is charged exactly once, from fee-native balance above `MinUserBalance`, before `pending_signal: false -> true` (§5.2, §11.4). Redundant latched-period activity charges nothing (§5.3). A useful User `manual_trigger` charges the complete generated Manual occurrence owner to the sovereign account and returns `Pays::No`; a redundant, rejected, underfunded, or System Manual call retains ordinary transaction payment and charges no Trigger fee. Automatic source producers pay their own source work; the Actor pays only its occurrence. Committed Trigger fees are nonrefundable and independent of later Pipeline admission.

One paid readiness may additionally back fixed work for its single owed Opening wait under §8.7, including zero-Step Opening. This dedicated component MUST NOT reuse ordinary occurrence, Pipeline control or destruction backing. An unpaid AtTime occurrence still requires previously secured installation/producer backing; its future Trigger fee cannot pay its own wait entry. Repeated tentative returns and refusal work remain separately bounded and backed.

The first additional financial Service inspection at Opening belongs to this dedicated readiness backing, for both zero-Step and nonempty Pipelines. Pipeline pricing MUST exclude that inspection once and retain separate backing for later Step and retry inspections. Fixed Opening-wait and Action-wait recovery work remain distinct from the initial inspection.

### 8.4 Pipeline Machine fee

```text
PipelineMachineFee = WeightToFee(Pipeline envelope, §4.4)
```

The User fee is charged once at Opening from fee-native balance above `MinUserBalance` (§6.2, §11.4). It prepays the reachable chargeable machine work through that Pipeline's Cycle boundary (§4.4), including ordinary Cycle completion but excluding final Actor-state destruction already backed by creation (§8.2). Pipeline quotes MUST NOT add a lifetime-cleanup surcharge. There is no per-Step machine charge, remaining machine budget, hold or refund settlement, Trigger work, or Task-effect work in it. Running and Suspended Attempts consume paid machine authority without a balance predicate.

Insufficient Pipeline payment follows §10.7 before Opening; block capacity refusal never selects insolvency or apoptosis. Collector failure rolls back Opening and preserves the latch (§6.2). Committed Pipeline fees are nonrefundable.

### 8.5 Action fee

Before an Action-bearing Task is invoked, the Attempt reserves the maximum Task-effect fee from the User Actor's fee-native balance above `MinUserBalance`:

```text
cannot reserve -> on_unfunded disposition (§10.7); no Action invocation; no Action fee
can reserve    -> invoke canonical effect
                  -> settle valid actual effect Weight
                  -> charge actual Action fee on success or typed failure
```

A retry is a new independently charged Action attempt. Before a Suspended User Actor receives retry execution, the scheduler proves that its fee-native balance above `MinUserBalance` covers the current Action maximum; otherwise it follows `on_unfunded` before invocation (§10.7). Effect and actual-fee settlement form one transaction, so unused liability is released and no debt survives rollback. False precondition, skipped resolution, `FundingUnavailable`, financial waiting and `StopCycle` invoke no Action and charge no Action fee. Invalid or greater-than-reserved actual Weight rolls back the current-Step transaction and fails closed.

Before collecting a nonzero User Action fee, settlement MUST revalidate that current fee-native spendability covers `MinUserBalance` plus the exact actual fee. Arithmetic overflow or a post-effect shortfall is an invariant refusal: the owning Step transaction rolls back effects, fees, progress and events; it creates no new financial-wait episode or Action observation.

`StopCycle` work belongs to Control in both admission and actual settlement, with zero economic effect capacity and zero Action fee. Its complete control cost remains inside the prepaid Pipeline Machine owner for User Actors; System fee exemption preserves the same physical resource assignment.

### 8.6 No double charging

No two economic surfaces may charge the same Weight or retained byte:

| Work | Sole economic owner |
| --- | --- |
| External call dispatch and origin work | ordinary transaction fee |
| User process admission and eventual complete state destruction | Actor Creation Fee (§8.2) |
| Retained Actor state | Actor State Hold (§8.2) |
| Useful readiness `false -> true` | Trigger Fee (§8.3) |
| Complete admitted Pipeline control | Pipeline Machine Fee (§8.4) |
| Invoked Task effect | Action Fee (§8.5) |
| Router trading, staking, and other native fees | underlying host mechanism (§12.1) |

### 8.7 Financial-wait economic obligations

`AwaitFunding` (§10.7) creates no recurring rent, negative polling or free Action attempt. Admission MUST establish finite economic backing and physical admission for each reachable wait transition before it can be published:

| Work | Required backing |
| --- | --- |
| Wait entry and expiry registration | An independently prepaid or transaction-paid bounded owner; an unpaid Trigger or Pipeline fee cannot pay for its own wait entry |
| Certified-ingress solvency hint, including a negative result | The producer's ordinary transaction/effect owner; no Actor readiness fee and no free notification loop |
| Funded recovery and wait detachment | A prepaid or transaction-paid bounded owner, plus collection of only the originally unpaid Actor service liability |
| One due solvency inspection and its funded-resumption branch | Finite backing secured before that waiting episode starts; no recurring negative review |
| Final synchronous Actor destruction | Creation-backed cleanup (§8.2), still physically admitted and metered in the current block |

A waiting episode means one continuous wait for the same unpaid service boundary, not a Cycle or periodic payment. Finite backing is internal work ownership; the host MAY secure dedicated bounded wait-work components through existing Creation, Trigger or Pipeline fee envelopes or a transaction-paid producer. This requires explicit additional bounds and qualification, not reuse of an ordinary control or destruction allowance. It does not require a separate authored payment for each entry or notification.

An installation that can publish an unconsumed AtTime occurrence MUST secure its fixed unpaid-occurrence wait work atomically with installation, before autonomous service becomes possible. Backing applies to that generation and semantic Contract only. Activation from Dormant or semantic replacement publishing an unconsumed occurrence cannot reuse an earlier installation's allowance; an exact no-op or technical re-certification creates no new owed occurrence. The host owns this installation-payment composition independently of the original Trigger liability and refundable state hold.

These work owners MUST remain disjoint from Action effects, ordinary Pipeline control and final destruction. Repeated wait/resume episodes and funded-at-expiry recovery MUST NOT reuse an exhausted allowance. Creation's destruction backing alone does not fund unlimited solvent expiry checks. Refundable state holds do not pay for work. Exact tariff, reachable episode bounds, reservation and producer composition require physical qualification before production admission; this contract supplies no numeric coefficient or implicit funding source.

---

## 9. Block resources and service passes

### 9.1 Meters

Block Weight after the host's fixed and context envelope is schedulable and splits component-wise into two non-convertible meters:

```text
ActorControlLimit   = floor(SchedulableBlockWeight * host_control_share)
SharedEconomicLimit = SchedulableBlockWeight - ActorControlLimit
ActorBaseTurn       = floor(SharedEconomicLimit / 2)
UserBaseTurn        = SharedEconomicLimit - ActorBaseTurn
```

The host configures `host_control_share` in `(0, 1]`; its integration documentation owns the selected value and the maximum fixed envelope. Allocation MUST reject a settled prefix plus outstanding fixed reserve that exceeds that host ceiling in either Weight component, even when the sum fits the complete block. Unused fixed allowance MUST NOT reduce the schedulable remainder. Both components are compared, reserved, and released independently; neither converts into the other. The context-to-Prepass transition MUST freeze exactly one immutable budget in the current-block resource state. Prepass, external admission, Drain and final reconciliation MUST use that budget; later configuration reads or caller-supplied limits MUST NOT replace it. Before the transition no current allocation exists, and the bounded finalized snapshot MUST retain the budget that governed its block.

Actor Control pays only Actor-specific work: detection and occurrence materialization, latch and placement, Opening, precondition and amount evaluation, run persistence, retry, completion, close control, scheduler topology, and bounded cleanup. Shared Economic pays ordinary external economic dispatch and Actor Task effects; a Task effect and its external equivalent use the same host mechanism and effect Weight (§12.1). Actor Control MUST NOT borrow Shared Economic capacity and MUST NOT be lent to it; actual-Weight reclaim never moves Weight between the meters.

### 9.2 Prepass, base pass, external phase, and Drain

Every block contains exactly one mandatory Actor Prepass inherent after required context inherents and before ordinary external extrinsics, including a block with no Actor work. It carries no author-selected scheduling, budget, or payload data. Its declared Weight MUST cover every legal frozen allocation, including a quiet prefix, independently of the configured worst-case fixed envelope. That declaration is provisional FRAME accounting, not authority to exceed frozen domains; the Prepass MUST return its charged actual Weight for settlement. The Prepass:

1. Services bounded due Deadline and detector obligations caused before the current block boundary (§5.8);
2. Materializes eligible readiness (§5.2);
3. Opens or resumes the block's Service round at the persistent ring cursor (§3.2);
4. Runs the Actor base pass: encounters residents in ring order and admits each complete transition before mutation, stopping at the first head whose control or effect envelope does not fit the remaining Actor Control meter or `ActorBaseTurn`.

Ordinary external extrinsics then consume Shared Economic capacity, including any `ActorBaseTurn` the base pass left unused. After external dispatch, Actor Drain MAY continue from the same cursor and round using the remaining Actor Control meter and remaining Shared Economic capacity. Drain MUST NOT execute current-block readiness, grant a second Step to one Actor, reorder the head, or exceed the actual remainder.

Current-block publications use next-block eligibility, and a resident already considered in the current round cannot execute again; together these prevent same-block Actor recursion without any per-Step ticket. A maximum valid Step may be the only Actor Step in a block: this is paid bounded service, not starvation. Pipeline- or Action-fee collector failure rolls back the current transition, preserves the Service head, and stops the pass; it is not a Task failure.

### 9.3 Liveness

Given a finite bounded Service population, recurring conforming Actor Control and Shared Economic capacity, finite stale churn, eventual placement capacity, successful fee collection when valid payment capacity exists, and no structural invariant fault, all eligible residents receive service in persistent ring order. No fixed block latency is promised; a larger runnable population MAY lengthen inter-Step gaps while preserving order and eventual service.

Without successful fee collection, the Service head MAY block later service despite spare Weight; only restored prerequisites or already-authorized lifecycle transitions resolve that obstruction. Starvation telemetry MUST NOT change priority, order, or execution authority.

---

## 10. Lifecycle

### 10.1 Class and mutability

User creation is signed; the caller becomes owner and consumes one owner slot (§11.2). System creation requires `SystemOrigin` and consumes a System locator (§11.2).

| Actor | Authorized control |
| --- | --- |
| User | signed owner |
| System | signed owner or `SystemOrigin` |

A User Mutable Actor MAY update its Contract, cancel a run, deactivate, reactivate, and explicitly close. Individual pause and resume MUST reject every User Actor without mutation; outside machine freeze, the signed owner of a valid Active User Actor receives `BadOrigin`, including when resume would otherwise be an exact no-op or a window has expired. Public Dormant creation requires `Mutable`. Public Immutable creation MUST install an Active Contract because no later activation authority exists. A host genesis configuration MAY declare an Immutable Dormant System identity as a permanently sealed role; activation and owner close MUST reject it.

A User Immutable Actor rejects Contract replacement, pause and resume, activation and deactivation, cancellation, and owner close; MAY author a Manual Trigger; cannot author `RetryLater`; MUST set `auto_close_at_cycle_nonce = Some(1)` when authoring `AtTime`; and terminates only through authored or lifecycle terminal rules, one-shot Trigger underfunding (§5.4), Pipeline-admission apoptosis (§10.4), or deployed-lineage upgrade (§14.2).

A System Immutable Actor rejects actor-scoped control, cannot author Manual or `RetryLater`, is Actors-fee-exempt, and remains Active until an independently owned terminal rule or upgrade removes it.

Immutability fixes policy and owner control, not runtime Weight, fee conversion, adapter behavior, or host economics. `auto_close_at_cycle_nonce` changes only through complete Mutable Contract replacement and MUST be strictly above the current terminated nonce within `MaxAutoCloseNonceHorizon`.

### 10.2 Terminal precedence

Terminal predicates are evaluated only by their owning transition:

| Transition | Precedence |
| --- | --- |
| Stored-state classifier (§3.5) | `WindowExpired`; `RetryAttemptsExhausted`; `ConsecutiveFailures`; `AutoCloseNonceReached` while Idle; already-materialized permanent `SchedulerIndexExhausted` |
| Opening (§6.2) | stored-state terminal reason; `CycleNonceExhausted`; `on_unfunded` disposition when the Pipeline Machine fee cannot be provided (§10.7) |
| Step finalization (§7.5) | retry-local bound; global failure bound; `BehaviorPolicySatisfied`; `AutoCloseNonceReached` after a non-suspended Cycle boundary; permanent deadline-placement `SchedulerIndexExhausted` |
| Temporal Trigger service (§5.4) | `on_unfunded` disposition for a due User `AtTime` source that cannot pay its Trigger fee (§10.7) |

One terminal reducer applies this precedence to the complete candidate transition. A satisfied `close_when` yields one `BehaviorPolicySatisfied` reason irrespective of the number of satisfying clauses. It MUST NOT suppress an Action permitted by the current precondition. Mid-Cycle policy Close cancels with `Closing(BehaviorPolicySatisfied)` and creates no normal Cycle completion observation; a Step that normally completes the Cycle first supplies its qualified completion to the same candidate (§7.7). Cleanup/cancellation events never feed the policy. Active-state hold inconsistency at Opening fails as an Actor invariant rather than a close reason.

### 10.3 Close

Close is lifecycle cleanup only. It MUST atomically cancel any open run (§6.6), revoke execution authority, remove the Contract, admission authority, hot state, run, detector memberships, residence, deadlines, reverse handles, and holds, release the User slot or mark the System locator vacant (§11.2), and emit `ActorClosed` (§13.2).

Close MUST NOT enumerate assets, transfer balances, unwind positions, invoke a Task, select a refund recipient, or alter sovereign custody. Explicit User close is available only to the signed owner of a Mutable User Actor; its ordinary transaction fee is paid by the caller and requires no Actor solvency.

### 10.4 Minimal User apoptosis

Minimal User apoptosis is automatic process-state collection with no custody effect. Under `on_unfunded = Close`, it occurs only for `TriggerAdmissionInsufficient` from one-shot User `AtTime` (§5.4), `CycleAdmissionInsufficient` before Opening (§6.2), or `CycleAdmissionInsufficient` before a Suspended User retry whose current maximum Action liability is unavailable (§8.5). `AwaitFunding` substitutes the exact waiting transition in §10.7; it does not bypass independently owned terminals.

It MUST NOT occur after an Action effect or leave fee debt. Trigger and Pipeline funding never close a Running or Suspended Pipeline; only the bounded pre-invocation retry-liability rule may close a Suspended User Actor. Minimal apoptosis performs Close (§10.3) without a Task, custody scan, fee reserve, or economic policy; its cleanup obligation is backed by the committed Creation Fee (§8.2).

### 10.5 Control transitions

Contract replacement, deactivation, explicit run cancellation, close, and incompatible upgrade MUST cancel the open run before changed meaning becomes executable (§6.6).

Individual pause and resume are available only to Mutable System Actors through the signed owner or `SystemOrigin`. Origin, class and immutability checks MUST precede expiry substitution and exact no-op handling. Pause preserves the Contract, any open run, latch, clocks, failure state, and Trigger detector evolution, and moves the process to `Disabled`. Resume reconstructs the exact required residence (§3.1). An Idle paused Actor may retain one already-paid latch, but source activity creates no deferred readiness and no Step executes.

Active creation or activation installs a new Active epoch with `Active/Idle`, no run, `pending_signal = false`, zero failure streak, current Trigger runtime state, schedule clocks, terminal marker, generation-bound residence, detector topology, and exact state hold. Activation preserves the Dormant identity nonce. Deactivation cancels any run and removes the Active epoch while preserving identity, slot or locator, nonce, custody, and the persistent control clock. Dormant activation with an exhausted Cycle nonce closes the identity with `CycleNonceExhausted` instead of installing an unusable epoch.

Authorized Mutable control MAY repair a non-window stored terminal condition before the scheduler or sweep commits Close; `WindowExpired` substitutes Close before the requested mutation. Mutable control is limited to one committed semantic mutation per Actor per block; an exact no-op returns before rate limiting.

Outside machine freeze (§10.6), permissionless sweep is bounded and closes only stored-state terminal reasons (§10.2); it never predicts Trigger or Pipeline affordability. Admissible Actor-targeting control and certified ingress may substitute Close only for `WindowExpired`.

### 10.6 Creation admission and machine freeze

Two independent bounded authoritative chain flags own global control: `UserCreationEnabled` admits new User identities; the existing `GlobalCircuitBreaker` freezes the Actor machine. Neither flag is an Actor residence, class, lifetime policy, fee exemption, or second scheduler. The host MUST explicitly own their genesis values and privileged control origin.

Disabling User creation MUST reject both automatic-slot and exact-slot creation, Active and Dormant, before identity/slot allocation, Creation Fee collection, hold installation or any Actor event. It MUST NOT disable existing User service, funding, activation, replacement, cancellation or close, and MUST NOT affect System creation. Recreating an emptied User slot creates a new identity and is therefore denied. Ordinary caller transaction payment remains independent of Actor admission.

Machine freeze MUST stop ordinary Actor execution, topology publication and destruction for both classes and mutabilities. It MUST preserve Contract/admission identity, paid latch, Run/cursor/nonce, monitoring episode, Service order, detector authority, Deadline handles, Park/Pending obligations, holds and sovereign custody identity; host-ledger balances remain subject to the declared balance-only movements below. Freeze rejection precedes expiry substitution and exact Actor no-op handling; an ordinary Root-equivalent Actor call does not bypass it.

| Path | User creation disabled, machine unfrozen | Machine frozen, either creation flag |
| --- | --- | --- |
| New User identity: automatic/exact slot, Active/Dormant | Reject `UserCreationDisabled` before Actor mutation/payment | Reject `GlobalCircuitBreakerActive` |
| New System identity or vacant locator attachment, Active/Dormant | Existing System authority and bounds | Reject `GlobalCircuitBreakerActive` |
| Existing activation, replacement, deactivation, cancellation, close, Manual occurrence, System pause/resume | Existing class/origin/mutability rules | Reject before no-op, expiry, fee or publication |
| Active-limit mutation and ordinary custody-locator allocation | Existing privileged rules | Reject; not thaw authority |
| Single/batch sweep, stale cleanup, apoptosis and other automatic terminals | Existing bounded owners | Defer automatic work; reject explicit sweep without Actor events |
| Mandatory Prepass, external resource accounting, Drain finalization and consensus clock | Existing block-resource protocol | Complete bounded resource/clock/telemetry protocol only |
| Service round opening/advance, Opening, Step and Actor service fees | Existing Q1/B+1 and fee owners | No Actor service mutation or Actor fee collection |
| Block/Tick Deadline extraction, temporal anchoring/rearm, retry and financial expiry | Existing bounded Deadline owners | Retain exact obligations; no extraction, rearm or expiry destruction |
| Parked dependency publication/scan, Pending and timed review | Existing bounded source and residence owners | No revision/publication/scan or Actor rescheduling |
| Positive certified ingress to an Active Actor, any Trigger | Existing transactional ingress semantics | Temporary freeze refusal; no accepted Actor consequence |
| Zero/self/no-op, absent/Dormant or genuinely uncertified custody-only movement | Existing balance-only classification | Balance-only only if no Actor publication is claimed |
| Pure queries, classification, cost quotes and rollback-only simulation | Existing bounded read contracts | Reads remain; simulation reports frozen without an Attempt |
| Privileged creation-admission and freeze setters | Declared control origin | Remain available; no per-Actor mutation or scan |
| Explicit authorized runtime upgrade/genesis installation | Declared upgrade/genesis authority | Outside ordinary service; preserve or explicitly re-establish owned invariants |

Certified ingress MUST NOT silently turn an accepted Actor consequence into balance-only success while frozen. Its read-only preflight and consequence point MUST both enforce temporary refusal; the host's declared atomicity owner rolls back the claimed movement, whether notification is post-movement, post-dispatch or transactional precommit (§12.4). Ordinary asset subsystems that require this Actor consequence MAY therefore refuse during freeze. No frozen-event archive, catch-up queue, synthetic latch, or deferred fee debt is permitted.

Balance-only custody remains possible through the already declared non-Actors ledger boundary, including ledger-only fee collection. It creates no readiness promise. A producer claiming balance-dependency publication MUST also refuse/roll back while frozen rather than mutate a revision or lose a causal obligation. A host MUST NOT evade the certified-producer inventory by rerouting a normally certified movement as uncertified. Lost notifications outside that inventory retain their existing bounded-review semantics; they do not become a guaranteed thaw replay.

Consensus time and checked block-resource phase accounting MUST continue during freeze. Actor lifetime and financial grace use their original absolute deadlines; freezing does not extend grace or reset clocks. On thaw, retained due obligations become serviceable through the same bounded owners, never before their due time, without replaying completed Steps, charging a second Pipeline fee or creating a second Cycle. Cadenced service retains its no-catch-up rule; an owed one-shot remains owed. Thaw performs no population scan or synchronous backlog drain.

Freeze takes effect at its committed call boundary and does not undo earlier committed block prefixes. If Prepass observed freeze or freeze was committed after Prepass, ordinary automatic Actor work MUST remain halted for the rest of that block even if a later call thaws it; the next block opens one normal Service round. The existing block-resource authority owns this intra-block halt, not a second persistent freeze switch. Post-thaw owner controls still obey their normal one-mutation-per-block and B+1 publication rules.

The host MUST bind both setters through a bounded authenticated control path that remains reachable while frozen. Failed authorization changes neither flag nor Actor state; admitted control writes and events have complete Weight owners. Public projections MUST distinguish both chain flags and block-local service halt from individual System pause. No flag or quoted prepayment waives current-block capacity, destruction admission or transaction rollback.

---

### 10.7 Fee-insolvency waiting

`on_unfunded` applies only to a User Actor's inability to provide the exact fee-native service liability above its protected minimum. The check MUST distinguish that condition from Task input shortfall, block capacity refusal, business/adapter failure, collector rejection and stale authority. None of those other conditions enters financial waiting. An invoked failed Task remains an Action failure; unpaid service invokes no Action and contributes no Action observation (§7.7).

The source and owed-service boundaries are distinct:

| Unfunded boundary | `Close` disposition | `AwaitFunding` disposition |
| --- | --- | --- |
| Manual Trigger source | Reject without an occurrence | Same rejection; no owed automatic occurrence and no wait |
| AddressEvent source | Custody-only movement without readiness | Same custody-only movement; no deferred Cycle or wait |
| Cadenced Trigger point | Skip and re-arm to a future aligned point | Same point skipping; no wait or eventual-deletion promise |
| Due AtTime Trigger | Minimal apoptosis (§10.4) | Retain one owed occurrence with its Trigger fee unpaid |
| Idle Pipeline Opening | Minimal apoptosis (§10.4) | Retain the already-paid readiness latch with the Pipeline fee unpaid |
| Suspended retry Action liability | Minimal apoptosis (§10.4) | Retain the same Run, cursor, paid Pipeline and retry authority; no retry increment or result |
| Running Action liability before invocation | Existing non-invoked `FundingUnavailable` continuation (§7.5) | Retain the same Run/cursor with the current Action fee unpaid; no outcome or retry increment |

The Running `Close` row preserves its selected comparison behavior; the policy label does not promise destruction at every unfunded boundary. `FundingUnavailable` in that row MUST retain typed fee-shortfall provenance so it cannot be mistaken for Task input shortfall. Ordinary non-fee funding outcomes keep their existing Step-error owner. Source skipping and custody movement MUST NOT fabricate an owed service boundary merely to enter waiting.

A waiting record MUST bind the Actor generation, behavior episode and exact unpaid boundary. It is a typed financial-wait reason on the canonical process/Deadline substrate, not idle `ParkedBalance`, a new queue or an off-chain obligation. At most one absolute expiry exists per waiting episode. Wait entry stops busy Trigger evolution, preserves all previously paid authority and committed effects, and publishes no future Cycle. Dust, negative hints, repeated notifications and balance oscillation never restart or extend expiry. A hint or tentative return to service alone does not start another episode: the original financial authority remains until the owed service boundary commits or is retired.

Grace is exactly 2,419,200 consensus seconds (28 days). At a 500 ms consensus tick it is 4,838,400 ticks. Host conversion MUST be checked, within `MaxTemporalDelayTicks` and rounded so expiry is never early. Absolute consensus time continues through machine freeze (§10.6); freeze or bounded service congestion MAY delay inspection, never extend the recorded grace or authorize early cleanup.

Recovery requires current fee-native spendability sufficient for the exact unpaid liability, not a deposit delta or a 100-ED threshold. A producer-paid certified-ingress hint MAY make funded recovery serviceable; dust or an uncertified notification does not restore service authority. Recovery MUST authenticate generation/episode/boundary, collect only unpaid work, detach wait authority atomically and respect B+1. It MUST NOT charge a second Pipeline fee, replay a committed prefix, reset outcome streaks or create another Cycle. Temporal sources re-arm from current state without catch-up; the one owed AtTime occurrence remains owed until its fee/occurrence transition commits.

The single due inspection revalidates current authority and solvency. An insolvent Actor whose full grace has elapsed closes through the existing synchronous destructor with `FundingGraceExpired`; a funded Actor follows recovery instead. Independent lifetime and safety terminals retain their own precedence and may apply earlier. A lost or uncertified funding notification has no prompt-recovery guarantee: absent another certified hint, the absolute expiry inspection is the recovery opportunity. This latency bound does not promise same-block inspection under freeze or resource congestion. Retiring or replacing a waiting generation/episode MUST detach its old expiry authority; a stale due item cannot close its successor.

Read-only eligibility MUST project the authenticated original unpaid service and liability, current fee-native service spendability above the protected minimum, their non-negative funding shortfall, entry time and absolute grace expiry separately from current residence. These current-state amounts MUST NOT be presented as proof of execution admission, fee-collector success or future solvency. Absence of a financial wait MUST NOT be presented as proof of future solvency.

Wait entry, funded resumption and grace expiry MUST have distinguishable typed receipts tied to the unpaid boundary/episode. Entry/resumption creates no Step, Action or Cycle-completion observation; expiry reuses ordinary cancellation/Close event ordering. Error projections MUST distinguish fee-native insolvency, input shortfall, collector failure, capacity refusal and invariant refusal. Economic backing for every published episode remains mandatory (§8.7).

`FinancialWaitReceipt { actor: ActorRef, episode_identity: [u8; 32], unpaid_service: UnpaidActorService }` retains the original episode and boundary identity. `FinancialWaitRecovered` commits only when that boundary commits or retires in its owning service transaction, including a current predicate skip or input shortfall that retires the Step. It adds no observation by itself. Collector, capacity or invariant rollback MUST restore the financial authority and emit no recovery receipt.

`FinancialWaitEntered` commits only with publication of that unpaid boundary and its expiry authority. Recovery of an unpaid Opening may atomically pay its machine fee and enter a distinct first-Action wait; the recovered Opening receipt precedes the new entry receipt. This is a new unpaid boundary after the old boundary committed, not renewal by a hint or partial payment. No Step result or monitor observation commits for the uninvoked Action.

## 11. Sovereign custody and identifiers

### 11.1 Sovereign-account derivation

```text
User seed   = Blake2_256(SCALE(ActorsPalletId, b"user", owner, owner_slot))
System seed = Blake2_256(SCALE(ActorsPalletId, b"system", system_sovereign_id))
account     = HostSovereignAccountDeriver(seed)
```

Derivation MUST be total, deterministic, class-separated, and stable for every previously admitted custody identity.

### 11.2 User slots and System locators

User slots are owner-local with `0 < MaxOwnerSlots <= 255`; `u8::MAX` is never a valid slot. Default User creation selects the lowest free slot, exact-slot creation requires that exact free slot, and User close releases it. System creation allocates a unique locator and System close marks it vacant; locator reuse creates a new Actor id with the same sovereign account. Actor ids never repeat, and every Active epoch carries a checked generation.

Sovereign custody at an unindexed derived account does not block exact reattachment. Reserved-account and live-collision errors remain distinct (§13.4). A previously registered vacant System locator remains reattachable even if later host policy classifies its account as reserved; the exception applies only to that locator. Custody identity survives host account-provider removal, although host dust or reaping policy may remove value independently.

### 11.3 Custody reattachment and recovery

Process lifetime and custody lifetime are independent. After Close the sovereign account, its balances, and adapter-exposed positions remain host-ledger state, and no Actors authority exists until reattachment. A fresh User Actor reattaches the same custody iff owner and exact slot equal those of the closed Actor; a fresh System Actor reattaches iff it reuses the same vacant locator. Reattachment inherits custody only, never Contract, mutability, nonce, Trigger state, lifecycle, run, or guarantees.

Recovery uses ordinary authored Contracts and Tasks (§7.6). Clients MAY automate exact-slot recreation, funding, recovery Contract generation, withdrawal, and re-close; Actors provides no direct recovery-transfer call.

### 11.4 Protected minimum

```text
protected_minimum   = MinUserBalance            for User Actors and the fee-native asset
protected_minimum   = host asset minimum balance otherwise
spendable_balance   = balance - current_action_fee_reservation
preservable_balance = spendable_balance - protected_minimum
```

Subtraction saturates only where shown. Every authored debit, including `Percent(100%)`, uses preservable capacity, and no lifecycle branch grants a source-exhaustion privilege.

---

## 12. Host effects and adapters

### 12.1 Canonical Task-effect ownership

Each effectful Task invokes the same canonical host economic mechanism as its external equivalent, with identical state-transition owner, atomicity boundary, synchronous consequences, typed failure classification, and production effect Weight. Actors MAY use narrow typed ports and need not construct a runtime call, but a cheaper shadow mechanism is forbidden. Actors owns orchestration control only; the host mechanism owns Task-effect Weight and Task-native fees.

### 12.2 Adapter obligations

The host provides asset, DEX, liquidity, staking, fee-collection, observation, ingress, and account-policy adapters; the package embedding guide owns their exact trait surface. Every adapter MUST be deterministic and bounded, return typed `TaskFailure { error, retry: Temporary | Permanent }` rather than panicking, report actual Weight no greater than its reserved maximum, and fail closed when a mutation capability is missing. Fee collection is one atomic ledger-only movement into the host's collection account.

### 12.3 Failure classification

Retryability MUST NOT derive from strings, module indexes, or broad token errors. Temporary covers dynamic slippage, current quote or cap insufficiency, stale reference, liquidity movement, recipient deposit unavailability, and recoverable placement capacity. Permanent covers malformed configuration, invalid provenance, missing static capability, monotonic namespace exhaustion, topology corruption, and invariant failure. Unknown downstream error is Permanent. `FundingUnavailable` is an amount or Action admission outcome, not a `TaskFailure` (§7.3, §8.5).

### 12.4 Special rules

**Swap.** The DEX adapter obtains a current executable quote internally, applies the authored cap and tolerance, invokes the canonical router, validates actual amounts, and returns typed failure; Actors exposes no independent quote surface. For `SwapOut`:

```text
capacity_cap     = preservable input balance
authored_cap     = capacity_cap                       for LiveQuote
                   min(capacity_cap, absolute_cap)    for Absolute
effective_max_in = min(authored_cap, quote + ceil(quote * tolerance))
```

A zero or insufficient cap is `FundingUnavailable` or Temporary according to whether dispatch was invoked. `SwapIn` requires positive actual output no worse than the authored tolerance against its current executable quote; 100% tolerance never permits zero output.

**System swap guard.** User swaps have no Actors-specific reference guard. System swaps require fresh nonzero directed reference values and MUST satisfy, before mutation and against returned actual amounts when they differ from the quote:

```text
exec_out * ref_in * Perbill::ACCURACY
  >=
(Perbill::ACCURACY - max_deviation) * ref_out * exec_in
```

All products use widened checked arithmetic. A missing, stale, or zero reference or excessive negative deviation is Temporary. The router neither owns nor reinterprets this guard.

**Liquidity.** Ordered LP identity is validated at admission and execution. Add and Donate actual debits MUST stay within supplied caps, and RemoveLiquidity MUST debit the exact resolved LP amount. For `DonateLiquidity`, Actors derives `max_b = preservable_balance(asset_b)` as the cap on pre-existing asset-B debit; the adapter MUST enforce it, and Actors independently verifies the returned asset-A spend.

**Staking.** Staking-share identity is admitted and stable. `Unstake(Percent(100%))` resolves to the full current share balance. Transferable staking receipts remain ordinary custody (§11.3).

**Certified AddressEvent ingress.** Only certified producers create AddressEvent semantics (§5.7):

| Protocol | Ordering and atomicity owner |
| --- | --- |
| `PostMovementNotify` | read-only preflight, movement, one notify; producer storage transaction |
| `BlockAtomicPostDispatch` | read-only preflight, successful dispatch, one notify; block/import state transaction |
| `XcmTransactionalPrecommit` | read-only preflight, ledger credit retaining the incoming holding, one Actors consequence, consume holding only on success; asset-transactor storage transaction returns holding on refusal |

Every certified protocol defines its source and provenance, read-only preflight, atomicity owner, Actors consequence owner, failure mapping, and complete Weight. Uncertified movement, an absent or Dormant destination, and zero, self, or no-op movement are balance-only. Outside machine freeze (§10.6), terminal substitution happens before any funding or readiness consequence (§10.2). Freeze refusal precedes terminal substitution. Fee collection is never certified ingress.

**Observation publication.** Observation publication creates no Actors ingress, subscription, Pending obligation, or Trigger cause. Its cost and revision semantics belong to the observation owner; Actors read the committed current value only when an authored predicate is evaluated (§7.2).

---

## 13. Calls, events, APIs, and errors

### 13.1 Calls and authorization

Canonical calls:

```text
create_user_actor
create_user_actor_at_slot
create_system_actor
create_system_actor_at_sovereign_id
activate_actor
deactivate_actor
pause_actor
resume_actor
manual_trigger
update_contract
cancel_run
close_actor
set_user_creation_enabled
set_global_circuit_breaker
set_active_actor_limit
permissionless_sweep
permissionless_sweep_many
actor_prepass
```

| Call | Origin |
| --- | --- |
| User creation | signed creator |
| System creation, locator reuse, active limit | `SystemOrigin` |
| User-creation admission and machine freeze | configured privileged control origin; available during freeze (§10.6) |
| User control | signed owner, subject to mutability (§10.1) |
| System control | signed owner or `SystemOrigin`, subject to mutability (§10.1) |
| Sweep | any signed origin |
| Prepass | mandatory unsigned inherent origin |

Creation charges the ordinary transaction fee, Actor Creation Fee, and state-hold delta (§8.2). `manual_trigger` follows Manual semantics (§5.7, §8.3). `close_actor` is signed Mutable User control and follows Close (§10.3).

### 13.2 Events and ordering

The ordered event ABI is normative:

```rust
enum Event<AccountId, AssetId, Balance> {
  ActorCreated { actor_id: ActorId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle },
  ActorActivated { actor_id: ActorId },
  ActorDeactivated { actor_id: ActorId },
  ActorPaused { actor_id: ActorId },
  ActorResumed { actor_id: ActorId },
  FinancialWaitEntered { receipt: FinancialWaitReceipt },
  FinancialWaitRecovered { receipt: FinancialWaitReceipt },
  ActorClosed { actor_id: ActorId, reason: CloseReason },
  CycleStarted { actor_id: ActorId, cycle_nonce: u64 },
  CycleSummary { actor_id: ActorId, cycle_nonce: u64, result: CycleResult, outcomes: OutcomeTotals },
  CycleSuspended { actor_id: ActorId, cycle_nonce: u64, cursor: u32, reason: SuspensionReason, cumulative_outcomes: OutcomeTotals },
  CycleContinued { actor_id: ActorId, cycle_nonce: u64, cursor: u32 },
  CycleCancelled { actor_id: ActorId, cycle_nonce: u64, reason: CancellationReason },
  CycleStopped { actor_id: ActorId, cycle_nonce: u64, step_index: u32 },
  StepSkipped { actor_id: ActorId, cycle_nonce: u64, step_index: u32, reason: StepSkippedReason },
  StepFailed { actor_id: ActorId, cycle_nonce: u64, step_index: u32, retry_class: RetryClass, error: DispatchError },
  TransferExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance, to: AccountId },
  SwapExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance },
  BurnExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  MintExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  StakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  UnstakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, shares: Balance },
  LiquidityDonated { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, max_amount_a: Balance, max_amount_b: Balance, amount_a: Balance, amount_b: Balance },
  LiquidityAdded { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance, lp_minted: Balance },
  LiquidityRemoved { actor_id: ActorId, cycle_nonce: u64, step_index: u32, lp_asset: AssetId, lp_amount: Balance, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance },
  ContractUpdated { actor_id: ActorId },
  ActiveActorLimitSet { old_limit: u32, new_limit: u32 },
  UserCreationEnabledSet { enabled: bool },
  GlobalCircuitBreakerSet { paused: bool },
  ManualTriggerSet { actor_id: ActorId },
  TriggerOccurrenceProcessed { actor_id: ActorId, trigger_family: TriggerFamily, fee: Balance },
  PipelineFeeCharged { actor_id: ActorId, fee: Balance },
  ActionFeeCharged { actor_id: ActorId, cycle_nonce: u64, step_index: u32, actual_effect_weight: Weight, fee: Balance },
  SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 },
  IdleStarvationDetected { consecutive_blocks: u32 },
  IdleStarvationRecovered { consecutive_blocks: u32 },
}
```

Ordering:

1. Useful Trigger fee and latch commit before `TriggerOccurrenceProcessed` (§5.2).
2. Opening emits `PipelineFeeCharged`, then `CycleStarted` (§6.2).
3. A Step event precedes any Cycle boundary event caused by that Step (§7.5).
4. Cancellation emits `CycleCancelled`, then `CycleSummary(Cancelled)` (§6.6).
5. A closing Cycle boundary emits `CycleSummary`, then `ActorClosed` (§10.3); pure Idle close emits only `ActorClosed`.
6. `ActionFeeCharged` is the final receipt of a committed Action-bearing Attempt (§8.5).
7. Fee-collection or enclosing transaction failure emits no rolled-back event.
8. Zero-Step Opening emits `CycleStarted`, then `CycleSummary(Completed)`, then optional `ActorClosed` (§6.5).
9. Redundant latched activity emits no Trigger event (§5.3).
10. Active creation emits `ActorCreated` only; Dormant-to-Active emits `ActorActivated`. Contract replacement or deactivation emits cancellation events first when a run exists, then `ContractUpdated` or `ActorDeactivated` (§6.6, §10.5).

Exact fields and discriminants come from metadata.

### 13.3 Runtime APIs

The runtime MUST expose read-only APIs for:

| Capability | Content |
| --- | --- |
| Simulation | At most one current Step of a fresh plan or the current run, rollback-only, under an explicit synthetic Actor Control and Shared Economic budget |
| Cost quote | Creation fee, prospective Trigger fee, prospective Pipeline Machine fee with its components, maximum next Action fee, and Actor state hold |
| Block resources | Configured budget, current-block meter state, and the last finalized block snapshot |
| Eligibility | Absence, dormancy, or the Active classification (§3.5) with latch and residence placement |
| Global controls | Separate creation-admission and machine-freeze chain flags plus current block-local automatic-service halt (§10.6) |

Simulation MUST use the same classifier, precondition, amount, Task, fee, outcome, and policy owners as production (§3.5, §7, §8). A waiting Actor returns not-ready, insufficient synthetic resource returns resource deferral, and fee-collector failure returns its typed error; simulation persists no state or event. No API exposes a remaining Pipeline Machine budget, because none exists (§8.4). Exact request and response types come from metadata.

Clients MUST NOT assemble semantic Contracts from raw storage heads, chunks, or pages; authored Contracts are verified against their semantic Contract ID (§4.2).

### 13.4 Errors and projections

Core classification errors:

```rust
enum ActorClassificationError {
  ActorInvariant,
  RunInvariant,
  ComputationOverflow,
}
```

They project exactly to dispatch, runtime-API, and simulation errors and MUST NOT become absence, dormancy, waiting, or scheduler exhaustion.

```rust
enum Error {
  ActorIdOverflow, ActorNotFound, ActiveActorCapacityExceeded, ActiveActorCountInvariant,
  ActorIdentityCapacityExceeded, ActorIdentityCountInvariant, ActorInvariant, ActorAlreadyActive,
  ActorDormant, ActiveActorLimitExceedsQueueCapacity, ActiveActorLimitTooHigh,
  ActiveActorLimitTooLow, ActiveActorLimitBelowCurrent, ActorPaused,
  ContractStepsExceedOnIdleBudget, ExecutionDelayTooLong, GlobalCircuitBreakerActive,
  UserCreationDisabled, AwaitFundingNotAllowedForSystemActor, AwaitFundingUnavailable,
  InvalidCloseWhen, BehaviorPolicyUnavailable,
  ImmutableActor, InsufficientBalance, InsufficientFee, TriggerFeeCollectionFailed,
  InvalidAmountResolution,
  InvalidParkedBalanceActivation, InvalidPredicate, InvalidAutoCloseNonce, InvalidScheduleWindow, InvalidTriggerConfiguration,
  InvalidTradeBound, InvalidRetryAttemptLimit, InvalidObservationMaxAge, SelfTransferNotAllowed,
  MintNotAllowedForUserActor, NotGovernance, NotOwner, OwnerSlotCapacityExceeded,
  OwnerSlotOccupied, InvalidOwnerSlot, ActorIdOccupied, SystemSovereignCapacityExceeded,
  SystemSovereignUnknown, SystemSovereignOccupied, SystemSovereignInvariant,
  SovereignAccountCollision, ReservedSovereignAccount, TooManyContractSteps,
  QueueTicketExhausted, SchedulerIndexExhausted,
  AutoCloseNonceHorizonExceeded, ControlMutationRateLimited, QueueCapacityUnavailable,
  RetryLaterNotAllowedForImmutableActor, ActorRunNotFound, ActorRunInvariant, ComputationOverflow,
  EmptyPrecondition, ManualSourceDisabled, RecipientDepositUnavailable,
  SystemActorTopologyInvalid,
  AdmissionBoundOverflow, StateHoldUnavailable, StateHoldInvariant, StateHoldOverflow,
  PrepassDuplicateOrStale, ResourceProtocolFailed, PrepassContextIncomplete,
}
```

`UserCreationDisabled` projects the new-User admission gate (§10.6). `AwaitFundingNotAllowedForSystemActor` rejects the forbidden System funding policy (§4.1); `AwaitFundingUnavailable` refuses financial-wait admission or maintenance without its required current authority and backing (§8.7, §10.7). `InvalidCloseWhen` rejects an invalid authored expression (§7.7), while `BehaviorPolicyUnavailable` refuses a valid expression without its required host resource qualification. None grants fallback admission or changes Actor custody.

The error surface MUST distinguish absence, dormancy, and active-state mismatch; authorization and mutability; invalid Contract, Trigger, predicate, amount, trade, retry, and window; capacity and bound overflow; slot, locator, collision, and reserved account; state hold and fee collection; Service, Deadline, and detector invariant failure and monotonic exhaustion; control rate limit and circuit breaker; and Prepass, round, and resource protocol failure. Stale admission or body authority projects to `ActorInvariant`, and stale run authority to `ActorRunInvariant`.

```rust
enum CloseReason {
  OwnerInitiated,
  CycleAdmissionInsufficient,
  TriggerAdmissionInsufficient,
  ConsecutiveFailures,
  WindowExpired,
  CycleNonceExhausted,
  AutoCloseNonceReached,
  RetryAttemptsExhausted,
  BehaviorPolicySatisfied,
  FundingGraceExpired,
  SchedulerIndexExhausted,
}

enum CancellationReason {
  Explicit,
  ContractReplaced,
  Deactivated,
  Closing(CloseReason),
}
```

Resolution outcomes are not pallet errors. `TaskFailure.error` MAY carry a stable dispatch diagnostic without converting the Attempt into a rejected extrinsic.

---

## 14. Storage, upgrades, configuration, and conformance

### 14.1 Storage and integrity

Generated descriptors own exact keys, hashers, prefixes, values, and page geometry. Normatively, storage MUST hold only the canonical facts of §4.5 and the run of §6.1, keep every collection bounded with exact reverse ownership, hold one residence per serving process plus bounded Trigger deadline authority (§3.1), retain no unbounded execution history, fee cache, remaining Pipeline budget, or generic cache-revalidation workset, and reconcile holds exactly (§8.2).

`try_state` MUST verify Contract, body, and admission bindings, run, latch, residence, detector topology, counters, slots, locators, and orphans. Orphan physical records are never semantic authority and are integrity failures unless they are transaction-local writes that roll back.

### 14.2 Runtime upgrades

Before first production genesis, a fresh canonical baseline MAY replace storage or ABI without migration compatibility. After deployed production lineage, every storage or semantic rewrite MUST define source and target schemas, a bounded migration unit and progress owner, an execution gate, Weight, interruption, resume, and idempotence, Actor and custody disposition, an open-run `Cancel | PreserveWithProof` policy, admission re-certification, and the storage-version transition.

A Weight or fee-policy change may alter admission identity but MUST NOT alter semantic Contract identity (§4.2, §4.4). Open paid Pipelines retain their paid service identity through the Cycle boundary unless the migration proves another disposition. A deployed change MUST NOT make previously reattachable custody unreachable without explicit custody disposition (§11.3).

### 14.3 Runtime configuration

Required relations:

1. `0 < MaxContractSteps <= 255`; each host selects its bounded value.
2. `0 < MaxOwnerSlots <= 255`.
3. `MaxRetryAttempts >= 2`, and `MaxContractSteps * MaxRetryAttempts` fits the outcome counters.
4. Current-Step predicate and amount-read bounds cover every admitted Contract.
5. Every Service, Deadline, detector, sweep, and worker bound is nonzero and owns one complete worst-case unit.
6. One maximum current-Step control and effect transition fits the guaranteed base pass (§9.2).
7. Maximum admitted create, activate, update, deactivate, cancel, and close paths remain dispatchable under their call limits.
8. `MinUserBalance >= host minimum balance` and `ActorCreationFee > 0`.
9. `WeightToFee` maps every nonzero User Trigger, Pipeline, and Action Weight upper bound to a positive fee.
10. `host_control_share` is in `(0, 1]` and the meters split as in §9.1.
11. `MaxTemporalDelayTicks` and `MaxExecutionDelayBlocks` are clock-specific and representable, and the consensus tick is positive.

The host's integration documentation owns concrete values, page sizes, worker counts, and generated fee-envelope identities.

### 14.4 Conformance

A runtime conforms iff:

1. Metadata exposes only the canonical public shapes of this specification;
2. Every semantic function follows its owning section, and every cross-reference uses rather than redefines it (§1);
3. Every reachable transition is bounded, pre-admitted, and transactionally atomic at its defined boundary;
4. Residence, rounds, wake, parking, and classification follow §3;
5. Contract storage, identity, and admission follow §4;
6. Trigger occurrence, latch, re-arm, underfunding, and detection follow §5;
7. Opening, Attempt inputs, nonce, zero-Step, and Cycle boundaries follow §6;
8. Q1 Step execution, error policies, and Tasks follow §7;
9. Creation, Trigger, Pipeline, Action, and state-hold ownership follow §8 without overlap;
10. Meters, block passes, and liveness follow §9;
11. Mutability, terminal precedence, close, apoptosis, and the circuit breaker follow §10;
12. Sovereign custody survives process deletion and exact reattachment follows §11;
13. Each Task effect maps to one canonical host owner and typed failure surface (§12);
14. Calls, events, APIs, and errors follow §13;
15. Storage integrity and deployed upgrades follow §14.1 and §14.2;
16. Generated Weight upper-bounds every admitted control and effect branch in both dimensions;
17. Simulation is rollback-only and observationally equivalent to zero or one production current-Step transition.

---

_End of specification._
