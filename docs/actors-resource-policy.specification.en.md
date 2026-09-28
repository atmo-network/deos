# DEOS Actors Resource Policy Specification

- **Scope**: DEOS reference-runtime block resource allocation
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase. This document defines the host resource policy under which the DEOS reference runtime composes portable Actors with ordinary economic dispatch. The Actors package specification owns Actor semantics, Service order, block rounds, the meter semantics, and the pass order (its §3.2 and §9); this document owns the reference runtime's selected allocation, fixed envelope, phase marker, inherent validity rules, runtime/node boundary, and the evidence the reference runtime must provide.

---

## 1. Resource Domain

`Weight` is an ordered pair:

```text
Weight = (RefTime, ProofSize)
```

Every comparison, subtraction, ratio, reservation, release, and limit in this document MUST be applied independently to both components. A call or Actor Step fits an envelope only when both components fit. Neither component MAY be converted into, borrowed from, or compensated by the other, and the runtime MUST NOT reduce Weight to a scalar fairness score.

The runtime MUST use one block-bound budget authority for fixed/context accounting and the Economic Zipper. It MUST settle the required initialization and context prefix, preserve all still-outstanding fixed work, and freeze the resulting allocation exactly once before the first Actor Prepass reservation or effect. There MUST NOT be a separate reserve-reclamation allocator alongside the Economic Zipper.

Budget construction MUST use bounded arithmetic over settled charges and a fixed set of outstanding cost owners, not scan Actors, proposals or queued messages to rediscover demand. It MUST reuse the current-block state and existing reservation/settlement path rather than add a parallel ledger or scheduler. Added construction, per-operation accounting and final reconciliation work MUST have complete Weight owners; conformance evidence MUST report those costs separately from service and Task effects in both dimensions, including their share of the quiet and busy block budgets. Compactness MUST be established by implementation shape and measured overhead, not assumed from the allocation formula.

Let:

```text
MaxBlockWeight
SettledPrefixWeight
RemainingFixedReserve
FixedBlockWeight = SettledPrefixWeight + RemainingFixedReserve
SchedulableBlockWeight = MaxBlockWeight - FixedBlockWeight
```

be component-wise Weight values. `MaxBlockWeight` is the common FRAME and economic maximum; a larger independent economic ceiling or an additive percentage-based idle allowance MUST NOT create capacity outside it. All additions and subtractions MUST be checked, and `FixedBlockWeight <= MaxBlockWeight` MUST hold before economic admission.

The selected reference-runtime `MaxBlockWeight` MUST be `Weight::from_parts(2_000_000_000_000, 10_485_760)`. FRAME, the economic allocator and context/idle accounting MUST use this common ceiling; the ProofSize increase MUST NOT increase RefTime or silently multiply dependent service reservations. The target relay context MUST admit a PoV allowance of 10,485,760 bytes, and complete-block evidence MUST check the PoV boundary independently rather than equate charged ProofSize with encoded PoV size. Neither Actor effects nor system work receive capacity outside this common budget.

`SettledPrefixWeight` is authoritative charged Weight for the completed initialization/context prefix, not elapsed time or an independently measured storage proof. It includes any later work already prepaid by that prefix's Weight owners. `RemainingFixedReserve` covers only fixed work not already covered by that charge, including outstanding context bookkeeping, post-inherent/poll hooks, non-economic idle work and finalization. Every cost MUST have exactly one owner across the settled prefix, outstanding fixed reserve, Actor Control and Shared Economic Execution.

Initialization and context include Timestamp, bounded session rotation, parachain validation and Message Queue service. Once a service phase is closed and its complete charged Weight is authoritative, its unused maximum MUST NOT remain reserved for that block. An empty queue still pays its bounded inspection and bookkeeping costs. A hook return that prepays finalization MUST NOT be interpreted as initialization-only or discounted a second time. Unsettled or uncertain costs MUST retain their sound maximum; uncertainty MUST NOT create credit.

The mandatory Actor Prepass and Actor Drain remain outside fixed work: Actor-specific execution and resource bookkeeping charge Actor Control, while Task effects charge Shared Economic Execution. Remaining fixed work MUST preserve its bound until block completion; the frozen economic allocation MUST NOT expand again when later fixed work consumes less than reserved. Per-operation actual-Weight settlement within the frozen domains remains governed by Section 4.

All non-Actor fixed/context work MUST share one system quarter-block ceiling, including Governance, Message Queue, session rotation, consensus/context establishment, base overhead and outstanding non-economic idle/finalization work:

```text
SystemMaximum = floor(MaxBlockWeight / 4)
OtherMandatoryMaximum + GovernanceMaximum + MessageQueueMaximum <= SystemMaximum
FixedBlockWeight <= SystemMaximum
```

The ceiling applies independently to RefTime and ProofSize and bounds complete admitted costs, including bookkeeping. Mandatory work MUST retain sound bounds first; Governance and Message Queue service maxima MUST fit the remaining system allowance before execution. No mandatory operation may be skipped to satisfy the ratio, and an overcommitted configuration MUST NOT be enforced merely by rejecting after effects. The service split is reference-runtime configuration, not an SDK-mandated percentage. Each cost retains its own accounting owner within this shared ceiling; no additional fixed envelope may sit outside it.

The system quarter is a worst-case allowance, not a permanent deduction from every block. At the freeze, the runtime MUST use complete settled prefix charges plus the sound outstanding fixed reserve, rather than retain unused maxima of closed phases. In particular, a completed non-rotation initialization MUST NOT retain the session-rotation maximum. The source of extra capacity is authoritative completed work and preserved outstanding bounds, never expected quietness, transaction-pool contents or wall-clock timing.

Maximum admitted system work leaves at least three quarters of the block for the three-way allocation. At that boundary each base share is approximately one quarter, and Actor effects plus user dispatch share approximately one half with Section 3's forward borrowing. A quiet system prefix instead approaches three one-third shares, still after actual system charges and outstanding reserves. Neither a permanent worst-case allocation nor a literally cost-free whole-block allocation is valid. Worst-case persistent Step admission and best-case block throughput MUST be evaluated separately, using the same allocation rule.

The runtime MUST admit initialization/context service under finite maxima that leave the mandatory Actor Prepass, Actor finalization and one maximum admissible Service Step feasible under the worst admitted fixed path in both Weight dimensions. This worst-case configuration bound governs persistent Step certificates; a quiet block's larger frozen allocation MUST NOT authorize a permanently larger Step. Conditional settlement improves use of quiet blocks, not the soundness of an overcommitted worst-case configuration. Fixed-service caps and liveness obligations MUST NOT be reduced implicitly to manufacture Actor headroom.

Every variable-size context inherent component contributing to `FixedBlockWeight` MUST have a runtime-declared finite admission bound whose generated Weight is valid at that bound. Benchmark component ranges, relay-side expectations, block-length limits, nominal reserves, and post-dispatch overweight rejection are not admission authority. The Actor Prepass `check_inherents` owner MUST inspect the shared parachain inherent data and reject full or hashed DMP/XCMP geometry beyond those bounds before execution; the node provider MUST construct within the same limits. Direct prepass dispatch still verifies established canonical context and finalization still requires the completed phase protocol, but neither may claim to reconstruct discarded inherent payload geometry from post-dispatch storage.

A maintenance branch that is unreachable in the active fresh-genesis topology is not fixed work merely because upstream code contains it. Its absence MUST be mechanically proved; any runtime upgrade or configuration that can activate it MUST recompute and admit its maximum before activation. The runtime MUST benchmark the complete maximum admitted context path, including validation, metadata traversal, full/hashed handling, and outer bookkeeping. Existing SDK leaves may remain the fixed owner only when their component-wise registered composition dominates that complete measured path. The measured owner is then a falsification floor rather than an additive duplicate; failed dominance requires a replacement fixed owner before the geometry remains admissible.

## 2. Resource Algebra

The DEOS reference policy applies once to the frozen `SchedulableBlockWeight`:

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

`ActorControlLimit` pays only for Actor-specific detection, materialization, canonical hot-state and current-Step loading, precondition and amount evaluation, Service ring and temporal topology, run-state bookkeeping, Actor fee bookkeeping, retry/completion, and repairable fault handling. Materialization fairness does not require all family minima in one block: when their sum does not fit, the persistent cursor admits one fitting family quantum and rotates future first service without borrowing Shared Economic capacity.

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

`fixed_weight` is the block's frozen `FixedBlockWeight`, not the sum of all configured service maxima. The bounded canonical projection MUST distinguish the charged prefix, outstanding fixed reserve, frozen limits and economic usage. A retained `fixed_reserved` total MUST identify this sum rather than claim measured actual work. The latest finalized snapshot is read-only observation, never a second allocator; historical sequences are materialized truth.

The same immutable current-block allocation MUST govern Prepass, external admission, Actor Drain and final reconciliation. Later reads MUST NOT recompute limits from FRAME's shrinking remainder. No task, author input or transaction-pool observation may select a larger allocation. Absent, stale, duplicate or inconsistent freeze authority MUST fail closed before the affected effect. Resource introspection before the freeze MUST distinguish configured admission guarantees from a current-block allocation that does not yet exist.

Any overflow, underflow, inconsistent reservation, impossible release, or disagreement that would make authoritative accounting uncertain MUST fail closed before the affected economic effect. Already committed earlier extrinsics or Actor Steps remain durable. Optional Actor work MUST halt for the block when safe accounting cannot be recovered.

## 3. Economic Zipper

The selected DEOS reference allocation is exact thirds: Actor Control receives floor one third, Shared Economic receives the remainder, and that remainder is split floor/remainder between Actor and user base turns. One third is the maximum permissible Actor Control share; further throughput work MUST optimize inside it. The Shared Economic envelope has symmetric approximately one-third turns under continuous demand:

```text
Actor base turn = ActorBaseTurn
User base turn  = UserBaseTurn
```

The split is by multidimensional Weight, never by call count, Actor count, fee, Actor class, or node-local arrival time. The one-third Control ratio is fixed; its absolute block allowance varies with the settled prefix. The primary Actor design comparison MUST use the fixed idealized zero-system/zero-user profile in the [performance-assurance specification §1.5](./actors-performance-assurance.specification.en.md#15-frozen-actors-only-design-comparison). Production integration comparisons MUST hold prefix workload and frozen allocation constant, or report allocation changes separately from machinery efficiency. Neither dynamic production headroom nor the worst-case system quarter may silently replace the idealized profile's fixed numeric ceilings. Actor effects and user dispatch retain distinct usage attribution inside the one shared economic pool.

Actors MAY consume up to `ActorBaseTurn` during the pre-user Actor base pass. Ordinary external extrinsics then MAY consume every part of `SharedEconomicLimit` not already consumed by Actor effects. This includes the complete User base turn and any Actor base-turn capacity Actors left unused. After external dispatch, Actor Drain MAY consume the remaining Shared Economic capacity, including unused User base-turn capacity.

Work conservation changes available capacity, not ordering authority:

- The runtime MUST NOT construct a consensus queue that merges node-local transaction-pool arrival order with Actor Service order.
- Block authors retain ordinary external-extrinsic ordering, subject to runtime validity and resource checks.
- Actor effects retain the canonical on-chain Service order.
- User and System Actors share the same Service ring and resource rules; Actor class MUST NOT affect ordering, admission, or allocation.
- Borrowing unused capacity MUST NOT reserve future work, bypass the current live Service head, or admit current-block readiness before the next block.

`UserBaseTurn` is the minimum Shared Economic capacity left for valid ordinary external dispatch when Actors fully consume `ActorBaseTurn`, except for unavoidable dispatch granularity: a user call that does not fit the remaining two-dimensional envelope is not partially admitted. Actor Drain begins only after the external-extrinsic phase and therefore cannot take capacity from a valid user call already admitted by the block author.

## 4. Admission and Actual Weight

Every ordinary extrinsic and Actor Step effect MUST reserve its declared maximum Weight before semantic mutation. Actor Step block admission MUST also reserve its maximum current Actor Control envelope before evaluating predicates or invoking an effect. Trigger and Pipeline economic admission follow the canonical fee boundaries in the Actors specification: only a useful `pending_signal: false -> true` transition performs Actor-specific Trigger work and charges its generated family owner, while later Idle readiness consumption separately charges complete bounded Pipeline Machine service before Opening. Creation economically backs eventual complete Actor-state destruction; no Pipeline surcharge repeats that payment. Neither prepayment reserves one-block Weight or future Action effects. Running/Suspended Steps consume paid machine authority without a control fee or economic-close classification.

Each Action-bearing Task attempt reserves only its current maximum effect fee while preserving the ledger minimum, then replaces it with valid actual effect Weight. An unfunded non-invoked Action yields `FundingUnavailable`, consumes prepaid Actor Control only, and follows authored policy. An underfunded Trigger occurrence creates no readiness and never invokes apoptosis. An Idle User that cannot fund Pipeline Machine plus ledger minimum while consuming paid readiness selects a separately generated minimal-apoptosis Actor Control owner. It consumes no Shared Economic Task envelope, performs no Opening/Task/custody mutation, refunds no prior Trigger fee, and may allow Service progress to continue only after process cleanup commits atomically.

Control and effect are separate accounting domains of one synchronous Step attempt, not independent execution queues. The runtime MUST atomically admit both maxima before an effect or Step-progress commit. Refusal of either domain MUST release any incomplete paired reservation and preserve the exact live Service head, progress and attempt authority without successor bypass; legitimate prior inspection remains charged. A later eligible phase or block may retry that head when capacity exists. An executed business failure instead follows the authored retry/terminal contract. Merging the budgets does not replace these atomicity obligations.

An ordinary external extrinsic's Shared Economic reservation MUST include its call and extension Weight, FRAME class base-extrinsic Weight and encoded length charged as ProofSize. Settlement MAY reclaim dispatch work but MUST retain the base and encoded-length overhead, including after failed dispatch. These per-extrinsic costs MUST NOT also be reserved as fixed system work; the system envelope retains block-level and mandatory-context overhead. Later FRAME proof reclaim MUST NOT create a second economic credit.

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

## 5. Maximum Step Fit and Head Fragmentation

Every admitted Actor Step MUST declare a maximum control envelope and maximum Task-effect envelope. The effect envelope MUST fit `ActorBaseTurn`, and the control envelope MUST fit `ActorControlLimit`, both component-wise. A runtime configuration that admits a Step violating either condition is invalid.

Strict Service order forbids bypassing a live head merely because a later Step is smaller. The Actor base pass or Actor Drain MUST stop when the live head cannot fit either the remaining Actor Control capacity or the remaining Shared Economic capacity available to that pass.

Head fragmentation is acceptable only when all of the following hold:

- The stop is caused by the exact live head failing a component-wise fit check.
- No later resident executes around that head.
- The runtime records the remaining Actor Control and Shared Economic Weight and the head's declared envelopes for measurement.
- In at least one blocking component, the stranded remainder is strictly less than the corresponding required head component.
- The required head component is no greater than the configured maximum admitted single-Step component.

This is the maximum semantic fragmentation bound. No stronger bound is valid for the non-blocking Weight component: a ProofSize-heavy head may strand substantial RefTime and vice versa. Production acceptance MUST report both components and MUST NOT disguise such stranding through a scalar percentage. If measured fragmentation prevents the release's declared service or regression objectives, the implementation or admitted Step geometry must change; head bypass is not an allowed correction.

## 6. Canonical Block Phase Protocol

Every block MUST have the following semantic order:

```text
1. Required initialization and context-establishing inherents
2. One budget freeze, then Mandatory Actor Prepass inherent
3. Signed and ordinary external extrinsics
4. Actor Drain during on_idle
5. Finalization
```

Timestamp, parachain validation data, and every other required context-establishing inherent remain mandatory fixed/context work outside the Economic Zipper. They MUST execute before the Actor Prepass. The prepass MUST verify from canonical current-block runtime state that Timestamp and every runtime-declared required parachain context owner have been established; absence, stale-block context, or impossible ordering makes the prepass invalid.

Each block MUST contain exactly one Actor Prepass inherent, including a block with no Actor work. The call carries no author-selected scheduling, budget, or payload data. Its presence establishes the consensus phase boundary. A duplicate prepass, a prepass before required context, or any signed or ordinary external extrinsic before the prepass makes the block invalid. An external extrinsic submitted after the prepass remains subject to ordinary runtime validity and the Shared Economic meter.

The freeze is part of the existing context-to-Prepass transition, not an extra author-supplied inherent or a second scheduler. Its snapshot MUST exclude any provisional Actor Prepass maximum already booked by FRAME; that reservation is not settled fixed work. The concrete execution boundary MUST prove which charges are settled, prepaid or outstanding, including extrinsic base/extension/encoded-length charges, FRAME reclaim and hooks that execute after all inherents. Subtracting an unclassified aggregate counter inside Prepass is insufficient.

The runtime MUST retain one current-block phase marker:

```text
ContextIncomplete → PrepassExecuting → ExternalPhase → FreshDrain → Finalizable
```

Transitions are one-way. External extrinsic validity requires `ExternalPhase`; Actor Drain sets `Finalizable` on completion even when no work exists. Finalization MUST fail closed unless the marker proves exactly one completed prepass and this progression. The marker is transient block protocol state, not historical telemetry.

### 6.1 Control Open and Round Eligibility

The prepass first performs control-open work under `ActorControlLimit`. Control-open MAY service only obligations whose causal lower bound permits service at the start of the current block. Current Timestamp MAY identify a cadence boundary during prepass, but readiness caused by observing that boundary remains next-block.

Same-block causality is enforced by the Actors block round, not by a ticket cutoff: every admission or reentry in block `N` is ineligible until `N + 1`, and a resident already considered in the current round cannot execute again (Actors specification §3.2). No execution cutoff is captured or stored. Readiness observed in block `N` becomes eligible no earlier than `N + 1`; later execution remains best-effort under the Service order and available `on_idle` Weight.

### 6.2 Actor Base Pass and External Phase

After control-open, the prepass executes the Actor base pass over the Service ring. It MUST stop when any of the following first prevents the exact live head from proceeding:

- The Actor base turn lacks the head's Task-effect envelope.
- The remaining Actor Control meter lacks the head's control envelope.
- The live head is not eligible in the current round.
- The strict Service order, breaker, fault, or fail-closed accounting contract requires a stop.

A ready Idle User head lacking complete activation admission is not a fragmentation stop when the remaining Actor Control meter fits minimal apoptosis. Cleanup consumes the live head and process topology atomically, preserves custody, and then permits Service progress. A Running or Suspended User is never economically classified at the head. If required cleanup control does not fit, the pass stops without mutation.

An empty or immediately stopped base pass is valid; the mandatory prepass still commits its phase marker and actual control Weight.

After prepass completion, the block author MAY include signed and ordinary external extrinsics in any order accepted by runtime validity. Their consensus result MUST depend only on block contents and canonical state, never on node-local transaction-pool arrival timing. External dispatch consumes the Shared Economic capacity remaining after Actor base-pass effects and MAY borrow all Actor base-turn capacity left unused. Readiness observed during the current block is ineligible until the next block.


### 6.3 Actor Drain

During `on_idle`, Actor Drain MAY consume Shared Economic capacity left after external dispatch and Actor Control capacity left after all prior Actor work. It continues from the exact Service cursor within the same block round.

Actor Drain MUST NOT:

- Execute readiness produced or observed in the current block, including Actor-produced, cadence, retry, successor, or incompletely materialized readiness.
- Bypass the current live head, reserve effect capacity for newly eligible work, grant a second Step to one Actor, or grant class preference.
- Reopen either control stage, reserve capacity from a future block, or exceed the actual block remainder supplied to `on_idle`.

The prior-generation base pass is the nonzero Running-continuation floor. Newly eligible work uses only remaining ordinary capacity and may borrow no protected future amount. When no eligible head fits, Drain returns its exact actual Weight and stops. Finalization then reconciles complete block meters under Section 2.

### 6.4 Runtime and Node Boundary

The runtime owns the Actor Prepass inherent identifier, payload-free call, required-presence rule, ordering checks, phase state, control-open algorithms, block-round eligibility, certified causal provenance, Actor base pass, Actor Drain, actual Weight, duplicate rejection, and final reconciliation. These are consensus rules and MUST NOT depend on node policy.

The node-side inherent provider owns only deterministic insertion of the runtime-declared empty Actor Prepass inherent data item into every authored block after supplying required context inherent data. It MUST NOT select Actors, budgets, or execution counts. Failure to construct the required data is a block-authoring failure, not permission to omit the prepass.

The runtime `inherent_extrinsics` path MUST derive exactly one canonical prepass extrinsic from that data. The runtime `check_inherents` path is the primary import-validation owner for missing, duplicate, malformed, noncanonical, or over-bound context/prepass inherent data and extrinsics. Runtime extrinsic application MUST independently reject duplicate or misordered prepass execution and external-before-prepass dispatch, while finalization MUST reject absence. These defenses ensure that direct block construction cannot bypass the import-time check.

Node-local signal arrival is not inherent input. Actor signals enter only through their canonical runtime operations and acquire on-chain causal and Service-order authority there.

This protocol does not redefine FRAME dispatch classes or create an Operational reserve; introducing a concrete Operational call requires a separately measured block-weight policy.

## 7. Economic Binding

The Actors specification exclusively owns creation, state-hold, Trigger, Pipeline Machine, Action, lifecycle, settlement, and projection economics. This resource policy neither transfers value nor redefines those boundaries. Fee or hold exemption changes value movement only: it MUST NOT change Actor Control or Shared Economic accounting, Service order, block admission, actual-Weight evidence, or the protected ledger minimum. Economic prepayment does not reserve future block Weight, and reclaimed block Weight does not imply an economic refund.

## 8. Conformance

A conforming runtime MUST provide generated or executable evidence that falsifies at least:

- Independent RefTime and ProofSize exhaustion.
- Full Actor and user base-turn contention.
- Actor-empty and user-empty work conservation.
- Partial demand on either side.
- Maximum single-Step fit.
- Strict-head fragmentation without bypass.
- Fixed T+1 eligibility, at most one turn per Actor per round, no current-block or Actor-produced same-block cause, and Running progress under sustained ingress pressure.
- Pre-dispatch reservation and post-dispatch actual reclaim.
- Failed external dispatch and committed unsuccessful Actor Step accounting.
- Arithmetic or meter corruption fail-closed behavior.
- Agreement between internal resource totals and FRAME-registered block Weight, including the common maximum, extrinsic overhead and reclaim ownership.
- Quiet and busy initialization/context prefixes producing exactly one correctly sized frozen allocation, with identical domain limits throughout Prepass, external dispatch and Drain.
- Rejection of duplicate/stale freeze authority, underflow, an outstanding unowned cost, or attempted second credit for a precharged reservation or refunded cost.
- Preservation of post-inherent/poll, non-economic idle and finalization bounds, mandatory Actor progress and fixed-service liveness under the maximum admitted fixed workload.
- The whole-system quarter-block ceiling, including complete mandatory work, service overhead and outstanding fixed reserves in both Weight dimensions; reject overcommitted configurations before execution, and witness rotation/non-rotation prefixes without duplicate or permanently retained rotation charges.
- Explicit construction, per-operation accounting and final reconciliation overhead under quiet and busy prefixes, with bounded allocator work, no demand scan and no duplicate budget authority.
- Worst-admitted, quiet and intermediate Message Queue/Governance phases, plus mixed Actor/user contention, using the same budget authority and Actor workload. Record settled service costs, other fixed costs, outstanding reserve, frozen thirds and completed Steps in both Weight dimensions; quiet phases MUST release unused service allowance, while the worst admitted fixed path MUST retain mandatory Actor progress and maximum-Step fit. Absence of useful service work MUST NOT erase inspection costs or imply a literal whole-block economic budget.
- User/System Actor resource neutrality for executable Steps; User-only boundary admission and minimal apoptosis consume Actor Control but never scheduler priority.
- Idle nonviable-head minimal cleanup without effect reservation, custody mutation, mid-run classification, or later-resident bypass before cleanup commits.

Telemetry and runtime API projections MAY expose finalized counters, but they are read-only observations and MUST NOT become resource authority.
