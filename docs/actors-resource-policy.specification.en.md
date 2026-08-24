# DEOS Actors Resource Policy Specification

- **Scope**: DEOS reference-runtime block resource allocation
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase. This document defines the host resource policy under which the DEOS reference runtime composes portable Actors with ordinary economic dispatch. The Actors package specification owns Actor semantics, FIFO authority, and Task behavior; this document owns only reference-runtime block phases and multidimensional resource allocation.

---

## 1. Resource Domain

`Weight` is an ordered pair:

```text
Weight = (RefTime, ProofSize)
```

Every comparison, subtraction, ratio, reservation, release, and limit in this document MUST be applied independently to both components. A call or Actor Step fits an envelope only when both components fit. Neither component MAY be converted into, borrowed from, or compensated by the other, and the runtime MUST NOT reduce Weight to a scalar fairness score.

Let:

```text
MaxBlockWeight
FixedBlockWeight
SchedulableBlockWeight
```

be component-wise Weight values. `MaxBlockWeight` is the FRAME maximum block Weight. `FixedBlockWeight` is the conservative runtime-owned envelope for block/context work that belongs to neither Actor Control nor Shared Economic Execution. It includes block initialization/finalization, required context-establishing inherents, and runtime-declared non-economic maintenance reserves, including Timestamp, bounded session rotation, parachain validation context, Message Queue `on_initialize` service, and XCMP Queue lazy-migration `on_idle`. It excludes the mandatory Actor Prepass because the prepass charges its Actor-specific work to Actor Control and its Task effects to Shared Economic Execution.

The runtime MUST configure:

```text
FixedBlockWeight <= MaxBlockWeight
SchedulableBlockWeight = MaxBlockWeight - FixedBlockWeight
```

with checked component-wise subtraction. The configured fixed envelope MUST cover the maximum admitted fixed/context path; unused fixed capacity is not silently reclassified as schedulable capacity within the same block. Increasing fixed work therefore requires explicit recomputation of the schedulable envelope.

Every variable-size context inherent component contributing to `FixedBlockWeight` MUST have a runtime-declared finite admission bound whose generated Weight is valid at that bound. Benchmark component ranges, relay-side expectations, block-length limits, nominal reserves, and post-dispatch overweight rejection are not admission authority. The Actor Prepass `check_inherents` owner MUST inspect the shared parachain inherent data and reject full or hashed DMP/XCMP geometry beyond those bounds before execution; the node provider MUST construct within the same limits. Direct prepass dispatch still verifies established canonical context and finalization still requires the completed phase protocol, but neither may claim to reconstruct discarded inherent payload geometry from post-dispatch storage.

A maintenance branch that is unreachable in the active fresh-genesis topology is not fixed work merely because upstream code contains it. Its absence MUST be mechanically proved; any runtime upgrade or configuration that can activate it MUST recompute and admit its maximum before activation. The runtime MUST benchmark the complete maximum admitted context path, including validation, metadata traversal, full/hashed handling, and outer bookkeeping. Existing SDK leaves may remain the fixed owner only when their component-wise registered composition dominates that complete measured path. The measured owner is then a falsification floor rather than an additive duplicate; failed dominance requires a replacement fixed owner before the geometry remains admissible.

## 2. Resource Algebra

The DEOS reference policy is:

```text
ActorControlLimit   = floor(SchedulableBlockWeight / 3)
SharedEconomicLimit = SchedulableBlockWeight - ActorControlLimit
ActorBaseTurn       = floor(50% * SharedEconomicLimit)
UserBaseTurn        = SharedEconomicLimit - ActorBaseTurn
```

The subtraction definitions assign every indivisible remainder exactly once and ensure:

```text
ActorControlLimit + SharedEconomicLimit = SchedulableBlockWeight
ActorBaseTurn + UserBaseTurn = SharedEconomicLimit
```

`ActorControlLimit` pays only for Actor-specific detection, materialization, canonical hot-state and current-Step loading, precondition and amount evaluation, FIFO and temporal topology, run-state bookkeeping, Actor fee bookkeeping, retry/completion, and repairable fault handling. Materialization fairness does not require all family minima in one block: when their sum does not fit, the persistent cursor admits one fitting family quantum and rotates future first service without borrowing Shared Economic capacity.

`SharedEconomicLimit` pays for ordinary external economic dispatch and Actor Task effects. An Actor Task effect MUST use the same production Weight owner as its equivalent external mechanism. Actor class and fee exemption do not change the meter charged.

The runtime MUST preserve all of the following component-wise:

```text
actor_control_used <= ActorControlLimit

actor_effect_used
+ user_dispatch_used
<= SharedEconomicLimit

fixed_weight
+ actor_control_used
+ actor_effect_used
+ user_dispatch_used
<= MaxBlockWeight
```

`fixed_weight` is the configured fixed/context envelope reserved at admission and reported as `fixed_reserved`; it is not presented as measured actual work. The runtime MUST preserve this complete envelope for every block, and observing less conditional fixed work does not enlarge the current block's schedulable limits. A future `fixed_actual` surface requires one authoritative runtime owner that measures every included hook, inherent, and context component.

Any overflow, underflow, inconsistent reservation, impossible release, or disagreement that would make authoritative accounting uncertain MUST fail closed before the affected economic effect. Already committed earlier extrinsics or Actor Steps remain durable. Optional Actor work MUST halt for the block when safe accounting cannot be recovered.

## 3. Economic Zipper

The selected DEOS reference allocation is exact thirds: Actor Control receives floor one third, Shared Economic receives the remainder, and that remainder is split floor/remainder between Actor and user base turns. One third is the maximum permissible Actor Control share; further throughput work MUST optimize inside it. The Shared Economic envelope has symmetric approximately one-third turns under continuous demand:

```text
Actor base turn = ActorBaseTurn
User base turn  = UserBaseTurn
```

The split is by multidimensional Weight, never by call count, Actor count, ticket count, fee, Actor class, or node-local arrival time.

Actors MAY consume up to `ActorBaseTurn` during the pre-user Actor base pass. Ordinary external extrinsics then MAY consume every part of `SharedEconomicLimit` not already consumed by Actor effects. This includes the complete User base turn and any Actor base-turn capacity Actors left unused. After external dispatch, Actor Drain MAY consume the remaining Shared Economic capacity, including unused User base-turn capacity.

Work conservation changes available capacity, not ordering authority:

- The runtime MUST NOT construct a consensus queue that merges node-local transaction-pool arrival order with Actor tickets.
- Block authors retain ordinary external-extrinsic ordering, subject to runtime validity and resource checks.
- Actor effects retain canonical on-chain strict FIFO ordering.
- User and System Actors share the same Actor FIFO and resource rules; Actor class MUST NOT affect ordering, admission, or allocation.
- Borrowing unused capacity MUST NOT reserve future work, bypass the current live Actor FIFO head, or admit a current-block readiness obligation across the execution cutoff.

`UserBaseTurn` is the minimum Shared Economic capacity left for valid ordinary external dispatch when Actors fully consume `ActorBaseTurn`, except for unavoidable dispatch granularity: a user call that does not fit the remaining two-dimensional envelope is not partially admitted. Actor Drain begins only after the external-extrinsic phase and therefore cannot take capacity from a valid user call already admitted by the block author.

## 4. Admission and Actual Weight

Every ordinary extrinsic and Actor Step effect MUST reserve its declared maximum Weight before semantic mutation. Actor Step block admission MUST also reserve its maximum current Actor Control envelope before evaluating Current predicates or invoking an effect. Trigger and Pipeline economic admission follow the canonical fee boundaries in the Actors specification: only a useful `pending_signal: false -> true` transition performs Actor-specific Trigger work and charges its generated family owner, while later Idle readiness consumption separately charges complete bounded Pipeline Machine/cleanup service before Opening. This prepays economic machine service but does not reserve one-block Weight or future Action effects. Running/Suspended Steps consume paid machine authority without a control fee or economic-close classification.

Each Action-bearing Task attempt reserves only its current maximum effect fee while preserving the ledger minimum, then replaces it with valid actual effect Weight. An unfunded non-invoked Action yields `FundingUnavailable`, consumes prepaid Actor Control only, and follows authored policy. An underfunded Trigger occurrence creates no readiness and never invokes apoptosis. An Idle User that cannot fund Pipeline Machine/cleanup plus ledger minimum while consuming paid readiness selects a separately generated minimal-apoptosis Actor Control owner. It consumes no Shared Economic Task envelope, performs no Opening/Task/custody mutation, refunds no prior Trigger fee, and may allow strict FIFO to continue only after process cleanup commits atomically.

After dispatch, the runtime MUST replace each maximum reservation with valid actual post-dispatch Weight:

```text
used_after = used_before + actual
```

where `actual <= reserved` component-wise. The difference becomes available only within the same owning envelope:

- Released Actor Control capacity remains Actor Control capacity.
- Released Actor effect capacity remains Shared Economic capacity.
- Released user-dispatch capacity remains Shared Economic capacity.

An absent, malformed, greater-than-reserved, or otherwise untrustworthy actual Weight MUST fail closed according to the owning dispatch transaction. Reclaim MUST NOT erase Weight already registered with FRAME, move Weight between Actor Control and Shared Economic domains, or violate the block-total invariant.

A failed external extrinsic is charged its valid actual post-dispatch Weight. A committed unsuccessful Actor Step is charged its actual Actor Control work and any actual Task effect work. A rejected or rolled-back Actor Step MUST preserve the transactional semantics defined by the Actors specification while retaining whatever outer FRAME accounting is required for work already performed.

## 5. Maximum Step Fit and FIFO Fragmentation

Every admitted Actor Step MUST declare a maximum control envelope and maximum Task-effect envelope. The effect envelope MUST fit `ActorBaseTurn`, and the control envelope MUST fit `ActorControlLimit`, both component-wise. A runtime configuration that admits a Step violating either condition is invalid.

Strict FIFO forbids bypassing a live head merely because a later Step is smaller. The Actor base pass or Actor Drain MUST stop when the live head cannot fit either the remaining Actor Control capacity or the remaining Shared Economic capacity available to that pass.

Head fragmentation is acceptable only when all of the following hold:

- The stop is caused by the exact live head failing a component-wise fit check.
- No later ticket executes around that head.
- The runtime records the remaining Actor Control and Shared Economic Weight and the head's declared envelopes for measurement.
- In at least one blocking component, the stranded remainder is strictly less than the corresponding required head component.
- The required head component is no greater than the configured maximum admitted single-Step component.

This is the maximum semantic fragmentation bound. No stronger bound is valid for the non-blocking Weight component: a ProofSize-heavy head may strand substantial RefTime and vice versa. Production acceptance MUST report both components and MUST NOT disguise such stranding through a scalar percentage. If measured fragmentation prevents the release throughput contract, the implementation or admitted Step geometry must change; FIFO bypass is not an allowed correction.

## 6. Canonical Block Phase Protocol

Every block MUST have the following semantic order:

```text
1. Context-establishing inherents
2. Mandatory Actor Prepass inherent
3. Signed and ordinary external extrinsics
4. Actor Drain during on_idle
5. Finalization
```

Timestamp, parachain validation data, and every other required context-establishing inherent remain mandatory fixed/context work outside the Economic Zipper. They MUST execute before the Actor Prepass. The prepass MUST verify from canonical current-block runtime state that Timestamp and every runtime-declared required parachain context owner have been established; absence, stale-block context, or impossible ordering makes the prepass invalid.

Each block MUST contain exactly one Actor Prepass inherent, including a block with no Actor work. The call carries no author-selected scheduling, ticket, budget, or payload data. Its presence establishes the consensus phase boundary. A duplicate prepass, a prepass before required context, or any signed or ordinary external extrinsic before the prepass makes the block invalid. An external extrinsic submitted after the prepass remains subject to ordinary runtime validity and the Shared Economic meter.

The runtime MUST retain one current-block phase marker:

```text
ContextIncomplete → PrepassExecuting → ExternalPhase → FreshDrain → Finalizable
```

Transitions are one-way. External extrinsic validity requires `ExternalPhase`; Actor Drain sets `Finalizable` on completion even when no work exists. Finalization MUST fail closed unless the marker proves exactly one completed prepass and this progression. The marker is transient block protocol state, not historical telemetry.

### 6.1 Control Open and Execution Cutoff

The prepass first performs control-open work under `ActorControlLimit`. Control-open MAY service only obligations whose causal lower bound permits service at the start of the current block. Current Timestamp MAY identify a cadence boundary during prepass, but readiness caused by observing that boundary remains next-block.

After control-open, the runtime freezes `prepass_execution_cutoff` at the greatest canonical Actor FIFO ticket then admitted for base-pass execution, or the explicit empty value. Cutoff capture is mandatory Actor-specific control work and consumes its generated owner from `ActorControlLimit`; it is excluded from `FixedBlockWeight` and Shared Economic capacity. The base pass uses only tickets at or below that cutoff.

This cutoff remains the Actor Drain cutoff and excludes every current-block cause. It is an upper bound, not permission to ignore ticket eligibility, actor/run authority, strict FIFO, the circuit breaker, or either resource meter. Readiness observed in block `N` becomes eligible no earlier than `N + 1`; later execution remains best-effort under FIFO and available `on_idle` Weight.

### 6.2 Actor Base Pass and External Phase

After freezing the cutoff, the prepass executes the Actor base pass against the canonical FIFO. It MUST stop when any of the following first prevents the exact live head from proceeding:

- The Actor base turn lacks the head's Task-effect envelope.
- The remaining Actor Control meter lacks the head's control envelope.
- The live head is above the execution cutoff or is otherwise not temporally eligible.
- The strict-FIFO, breaker, fault, or fail-closed accounting contract requires a stop.

A ready Idle User head lacking complete activation admission is not a fragmentation stop when the remaining Actor Control meter fits minimal apoptosis. Cleanup consumes the live head and process topology atomically, preserves custody, and then permits FIFO progress. A Running or Suspended User is never economically classified at the head. If required cleanup control does not fit, the pass stops without mutation.

An empty or immediately stopped base pass is valid; the mandatory prepass still commits its phase marker and actual control Weight.

After prepass completion, the block author MAY include signed and ordinary external extrinsics in any order accepted by runtime validity. Their consensus result MUST depend only on block contents and canonical state, never on node-local transaction-pool arrival timing. External dispatch consumes the Shared Economic capacity remaining after Actor base-pass effects and MAY borrow all Actor base-turn capacity left unused. Readiness observed during the current block remains beyond its fixed execution cutoff and cannot execute before the next block.


### 6.3 Actor Drain

During `on_idle`, Actor Drain MAY consume Shared Economic capacity left after external dispatch and Actor Control capacity left after all prior Actor work. It continues from the exact canonical FIFO head and uses the current block's fixed `prepass_execution_cutoff`.

Actor Drain MUST NOT:

- Execute a ticket greater than the active cutoff.
- Execute current-block Actor-produced, cadence, retry, successor, retained-signal, or incompletely materialized readiness.
- Give a Fresh ticket priority over an older live head, reserve effect capacity for Fresh work, grant a second Step to one Actor, or grant class preference.
- Reopen either control stage, recompute a frozen cutoff, reserve capacity from a future block, or exceed the actual block remainder supplied to `on_idle`.

The prior-generation base pass is the nonzero Running-continuation floor. Newly eligible work uses only remaining ordinary capacity and may borrow no protected future amount. When no eligible cutoff-bounded head fits, Drain returns its exact actual Weight and stops. Finalization then reconciles complete block meters under Section 2.

### 6.4 Runtime and Node Boundary

The runtime owns the Actor Prepass inherent identifier, payload-free call, required-presence rule, ordering checks, phase state, control-open algorithms, fixed cutoff, certified causal provenance, Actor base pass, Actor Drain, actual Weight, duplicate rejection, and final reconciliation. These are consensus rules and MUST NOT depend on node policy.

The node-side inherent provider owns only deterministic insertion of the runtime-declared empty Actor Prepass inherent data item into every authored block after supplying required context inherent data. It MUST NOT select Actors, tickets, budgets, or execution counts. Failure to construct the required data is a block-authoring failure, not permission to omit the prepass.

The runtime `inherent_extrinsics` path MUST derive exactly one canonical prepass extrinsic from that data. The runtime `check_inherents` path is the primary import-validation owner for missing, duplicate, malformed, noncanonical, or over-bound context/prepass inherent data and extrinsics. Runtime extrinsic application MUST independently reject duplicate or misordered prepass execution and external-before-prepass dispatch, while finalization MUST reject absence. These defenses ensure that direct block construction cannot bypass the import-time check.

Node-local signal arrival is not inherent input. Actor signals enter only through their canonical runtime operations and acquire on-chain causal and FIFO authority there.

This protocol does not redefine FRAME dispatch classes or create an Operational reserve; introducing a concrete Operational call requires a separately measured block-weight policy.

## 7. Economic Binding

The Actors specification exclusively owns creation, state-hold, Trigger, Pipeline Machine, Action, lifecycle, settlement, and projection economics. This resource policy neither transfers value nor redefines those boundaries. Fee or hold exemption changes value movement only: it MUST NOT change Actor Control or Shared Economic accounting, FIFO order, block admission, actual-Weight evidence, or the protected ledger minimum. Economic prepayment does not reserve future block Weight, and reclaimed block Weight does not imply an economic refund.

## 8. Conformance

A conforming runtime MUST provide generated or executable evidence that falsifies at least:

- Independent RefTime and ProofSize exhaustion.
- Full Actor and user base-turn contention.
- Actor-empty and user-empty work conservation.
- Partial demand on either side.
- Maximum single-Step fit.
- Strict-head fragmentation without bypass.
- Fixed T+1 eligibility, cutoff immutability, no current-block or Actor-produced same-block cause, and Running progress under sustained ingress pressure.
- Pre-dispatch reservation and post-dispatch actual reclaim.
- Failed external dispatch and committed unsuccessful Actor Step accounting.
- Arithmetic or meter corruption fail-closed behavior.
- Agreement between internal resource totals and FRAME-registered block Weight.
- User/System Actor resource neutrality for executable Steps; User-only boundary admission and minimal apoptosis consume Actor Control but never scheduler priority.
- Idle nonviable-head minimal cleanup without effect reservation, custody mutation, mid-run classification, or later-head bypass before cleanup commits.

Telemetry and runtime API projections MAY expose finalized counters, but they are read-only observations and MUST NOT become resource authority.
