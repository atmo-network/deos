# DEOS Actors Performance Assurance Specification

- **Scope**: Project-owned production performance and stress acceptance
- **Target**: `pre-1.0.0`
- **Status**: Normative

This document defines the workloads and evidence required to accept DEOS Actors scaling claims. It does not select physical architecture, generate production Weight, replace pallet correctness tests, or make release attestation decisions. Runtime code, generated Weight, project validation, Experiment Records, and release assurance retain their respective ownership.

---

## 1. Evidence Contract

Every target run MUST record:

```text
exact source tree
runtime and production-Wasm identity
production Weight identity
runtime constants and Actor ceiling
state population and deterministic seed
start and final block
per-block Actor Control, Actor effect, and user-dispatch Weight
RefTime and ProofSize independently
completed, skipped, failed, suspended, closed, and pending counts
FIFO, detector, wakeup, body, and resource faults
```

Setup, genesis construction, prefunding, and deterministic population generation are outside the measured block interval. The measured interval begins with the named trigger operation or first baseline operation. A profile passes only against runtime-bound production Weight and production-Wasm or stronger full-runtime block evidence. Native wall-clock or synthetic microbenchmarks may diagnose a candidate but cannot establish these targets.

All populations, market state, authored bounds, transaction demand, and seeds MUST be deterministic and retained by the project harness. Weight fairness and deltas are component-wise; no scalar score may combine RefTime and ProofSize. Database reads/writes, persistent bytes, block count, and faults remain separately reported.

Independent RefTime-heavy and ProofSize-heavy user witnesses MUST be attempted. When the production call surface cannot saturate one component before another production bound rejects the next valid call, retain the highest justified reachable witness, identify the binding component and next-call deficit, report the unused component without calling it saturated, and keep the impossibility claim subordinate to an explicit callable-surface audit.

Architecture-affecting candidate comparisons use Experiment Records. The accepted project harness and generated Weight remain project truth; release assurance may review their exact-tree identity but is not a project validation dependency.

Maximum-context evidence MUST use the runtime-admitted DMP message, HRMP message, and HRMP channel bounds together, preserve the full/hashed split produced by the configured PoV allocator, execute the actual context inherent in benchmark Wasm, and record the fixture and benchmark-Wasm identities. A registered SDK composition may own the fixed envelope only after component-wise dominance over the complete measured path is proved without additive double accounting.

### 1.1 Independent Throughput Axes

Every throughput profile MUST report two independent primary axes:

```text
S = committed Actor Steps / measured block
A = distinct Actors that committed at least one Step / measured block
```

Report per-block values and the measured-interval distribution for both axes. Never omit one by inferring it from the other. Under authoritative Q1, one Actor commits at most one Step per block, so `S == A` is a conformance expectation for committed service; any divergence requires an explicit counted cause or is a defect. Both axes remain named independently so future derived fast paths or a separately reopened service contract cannot silently change metric meaning.

The historical target remains:

```text
10,000 completed one-Step User Actor Cycles
/ 100 eligible production blocks
= 100 cheap committed Actor Steps/block
```

For that exact Q1 one-Step witness, the target also entails 100 distinct Actors progressed/block. It is not a universal promise of 100 distinct Actors/block for multi-Step, mixed, market-heavy, divergent, or differently paced workloads.

Candidate comparisons MUST expose the two-axis Pareto frontier. A candidate is Pareto-dominated when another conforming candidate provides at least as much `S` and `A` under the same workload and is strictly better on one. No arbitrary scalar score may collapse `S`, `A`, RefTime, ProofSize, latency, or fairness into one rank.

### 1.2 Required Cost, Latency, and Fairness Metrics

For each candidate and workload report at minimum:

```text
ActorMachineRefTimePerCommittedStep
ActorMachineProofSizePerCommittedStep
DBReadsPerCommittedStep
DBWritesPerCommittedStep
TriggerToFirstStepLatencyBlocks
TriggerToCompletionLatencyBlocks
ServiceGapBlocks
HeadOfLineUnusedRefTime
HeadOfLineUnusedProofSize
UsefulActionWeight
ActorMachineWeight
```

`ServiceGapBlocks` is the number of blocks between committed progress opportunities for a continuously runnable Actor. Report the full deterministic sequence for bounded profiles or at least mean, median, p95, p99, maximum, and the affected Actor count for larger profiles. Under homogeneous Q1 pressure compare observed gaps with `ceil(runnable_actor_population / sustainable_committed_step_capacity_per_block)` and explain fragmentation, control, effect, or eligibility deviations. `HeadOfLineUnusedRefTime` and `HeadOfLineUnusedProofSize` are the otherwise available component-wise capacities stranded when the strict live FIFO head cannot fit; do not convert them into one scalar.

Actor-machine and action ownership remain separate:

```text
ActorMachineWeight =
  scheduler and causal materialization
  + canonical state and current-fragment loading
  + predicates, resolution, admission, and fee bookkeeping
  + run/continuation persistence
  + lifecycle, placement, and Actor events

UsefulActionWeight =
  canonical committed or typed-failed economic Task-effect Weight
```

Report each Weight component independently and MAY additionally report `UsefulActionWeight / ActorMachineWeight` only as two labeled component ratios when the denominator is nonzero. A heavy economic action is neither a scheduler regression nor a priority claim. A lower machine ratio cannot hide higher absolute ProofSize, reads/writes, latency, failure, or head-of-line loss.

For shared-cause profiles additionally report causal cohort materialization cost, ordered Fresh candidates admitted, Step-0 commits/completions, multi-Step Actors entering independent Running state, dissolution cost, and effect on continuation service gaps. The cohort normally ends after Step 0; one-Step Actors may complete within it. Do not spend consensus work tracking later behavioral alignment or rebuilding cohorts after divergence. Cohort membership derives only from one shared Trigger instance; shared Task shape and Contract length create no scheduling affinity or eligibility filter. Any candidate requiring `steps.len() == 1` or equivalent total-length classification is nonconforming.

### 1.3 Required Workload Classes

A service-throughput claim MUST cover enough distinct action classes to distinguish Actor-machine amortization from useful economic work. The minimum representative set is:

- Control-only or `StopCycle`-like cheap Step.
- Burn-like cheap effect.
- Transfer.
- Swap or another bounded market action.
- Liquidity-heavy action.

Each class uses its canonical effect owner and reports typed outcomes separately from runtime service. The scheduler MUST NOT rank by Task family, declared Weight, fee, or Actor class to improve packing. Workload-class results are separate evidence points and MUST NOT be averaged into one synthetic Actor.

## 2. Ten-Thousand Actor Targets

### 2.1 Reactive Transfer

Construct 10,000 prefunded one-Step User Actors on one feed:

```text
ObservationCrossing
→ Transfer
→ completed Cycle
```

One accepted changed publication in source block `N` causes the qualifying crossing. Every Actor MUST reach a completed one-Step Cycle within the 100 eligible production blocks `N + 1 ..= N + 100`. The profile fails on a lost signal, duplicate Step, FIFO violation, block Weight overrun, unresolved worker fault, or incomplete Actor. Report `S` and `A` per block even though both count the same committed population in this exact Q1 one-Step profile.

The Transfer destination and amount geometry MUST be identical across the population and executable throughout the run. Setup MUST isolate scheduler/control throughput from avoidable recipient-creation or insufficient-funding failures while retaining the real certified ingress and ledger consequences of the canonical Transfer operation.

### 2.2 Contended Transfer

Repeat the Reactive Transfer profile while valid ordinary external extrinsics continuously consume the complete User base turn in every measured block. Actors MUST still complete all 10,000 Cycles within blocks `N + 1 ..= N + 100` using their guaranteed Actor base turn.

The external workload MUST be deterministic, independently successful, and sufficient in both Weight dimensions to demonstrate base-turn contention without exceeding runtime validity. The report MUST show `S`, `A`, Actor/user consumption, borrowing, service gaps, and component-wise head fragmentation independently.

### 2.3 Reactive SwapOut

Construct 10,000 prefunded one-Step User Actors:

```text
ObservationCrossing
→ bounded SwapOut
→ committed typed Step disposition
```

One qualifying publication in block `N` causes readiness. Every Actor MUST commit one typed Step disposition within blocks `N + 1 ..= N + 100`. The market fixture MUST use deterministic bounded liquidity and authored input protection; it MUST NOT fabricate 10,000 stale shared quotes or promise that every market operation succeeds economically.

Report runtime service separately from economic outcome:

- Runtime service: The Step committed one canonical `StepOutcome` without duplicate, loss, FIFO violation, or block overrun.
- Economic success: The Task returned the successful SwapOut outcome.
- Economic non-success: Precondition skip, funding outcome, typed Temporary/Permanent failure, suspension, or terminal policy disposition.

The throughput target concerns committed typed dispositions. Economic success rate is a separate measured result and MUST NOT be relabeled as scheduler failure or guaranteed market execution. Report `S`, `A`, action Weight, and Actor-machine Weight independently so market cost cannot be mistaken for scheduler policy or overhead.

## 3. Baselines and Differential Overhead

### 3.1 Manual Baseline

Measure the same one-Step Transfer and SwapOut Contracts, populations, balances, Tasks, and block policy through Manual readiness without observation detector work. Under the production `NextBlock` timing baseline, Manual signals remain causal next-block obligations; setup MAY batch deterministic signal extrinsics only when their transaction Weight is reported separately from Actor Control and effect execution.

The baseline isolates detector/materialization work from current-Step control and Task effect work. It does not replace the reactive acceptance targets.

### 3.2 External Baseline

Measure the equivalent external Transfer and SwapOut operations through their canonical public mechanisms under the same runtime, market state, recipients, authored bounds, Shared Economic budget, and production Weight owners. External transaction validation/payment overhead MUST be reported separately rather than subtracted as if it were Actor control.

### 3.3 Actor Control Differential

For each equivalent effect branch, report:

```text
Actor control overhead =
  Actor Step total Weight
  - canonical equivalent Task-effect Weight
```

The subtraction is checked independently for RefTime and ProofSize. The canonical adapter effect Weight is the subtraction owner; external extrinsic envelope overhead is not. A negative component indicates mismatched branches or accounting error and invalidates the comparison.

Also report database reads/writes and proof contributors so a lower RefTime cannot hide increased ProofSize or persistent topology cost.

### 3.4 Step-0 Timing

Assure the protocol-fixed next-block eligibility floor using identical authored block extrinsics, Trigger causes, FIFO backlog, runtime constants, Contracts, balances, and action fixtures.

Report separately:

- Trigger-to-first-Step latency and Trigger-to-completion latency.
- Readiness materialization/control Weight and Step-0 machine/effect Weight.
- Prepass and Drain `S`/`A`, ProofSize, reads/writes, cutoff, and stranded capacity.
- Exact state visible when Step 0 becomes eligible no earlier than `N + 1`.
- Actor-produced causes and proof that their first target Step is no earlier than block `N + 1`.
- Newly eligible populations `10/100/1,000/10,000`, low and sustained ingress, and a large shared-cause herd.
- Continuously runnable Running populations `10/100/1,000/10,000` under sustained ingress, with mean/p50/p95/p99/max inter-Step service gap and base-pass progress.
- One-Step completion versus multi-Step transition into independent Running service.

The implementation fails assurance on lost or duplicate readiness, bypass of an older FIFO head, current-block Actor recursion, repeatable privilege, Running base-pass loss, component-wise block overrun, an unbounded materialization/cutoff path, or any execution before the source block's `N + 1` eligibility floor.

### 3.5 Minimal User Apoptosis

Compare the retained historical current-Step economic-close baseline with Pipeline-admission-only minimal apoptosis and the four fee boundaries under identical Q1 Contract geometry, Task effects, block resources, and toolchain. Rerun only workloads whose Trigger pricing, Pipeline Machine strategy, Action-fee path, zero-Step geometry, cleanup, custody, projection, or benchmark setup crosses the changed dependency.

Pipeline Machine strategy MUST compare:

- `P0 UpfrontBounded`: One complete worst-case machine charge at Opening, no hold/refund/run ledger.
- `P1 HoldAndSettle`: Maximum machine hold with valid-actual settlement/refund.
- `P2 PerStepMachine`: Current-Step machine charging during Q1 service.

Measure `0/1/4/8/32` Steps across no/max Precondition geometry and reads, no/max RetryLater control paths, long inter-Step gaps, and identical Action effects. Report RefTime, ProofSize, reads/writes, accounting touches, persistent bytes, Wasm bytes, charged-versus-real machine work, failure surface, and runtime/API complexity. P0 is the production-leading hypothesis; P1/P2 cannot be accepted from lower charged cost alone when accounting/state complexity is not materially justified.

Additionally report:

- Trigger occurrence service by `Manual`, `AddressEvent`, `ObservationChange`, `ObservationCrossing`, `AtTime`, and `Cadenced`, separating source publication/ordinary transaction work from the disjoint User Trigger Fee.
- Independent Trigger and Pipeline deltas for: Idle useful occurrence with immediate Opening; one busy `false -> true` occurrence during an active Pipeline; redundant latched-period sources that perform no Actor-specific evaluation or charge; delayed Opening after retained `pending_signal`; paid useful Trigger followed by unavailable Pipeline Machine capacity; and a zero-Step immutable AtTime one-shot.
- For every separation fixture, component-wise incremental RefTime, ProofSize, database reads/writes, accounting touches, fee movement, latch transition, detector disable/re-arm, and Cycle transition attributable separately to useful occurrence materialization and Pipeline Opening.
- Running/Suspended current-Step cost at every cursor, proving no balance viability read, machine charge, machine hold, or suffix reconstruction after Opening.
- Success, typed failure, retry, and `FundingUnavailable` Action paths: every invoked Action pays valid actual effect Weight; an unfunded non-invoked Action pays zero and follows prepaid control policy without apoptosis.
- Pipeline-admission-insufficient minimal apoptosis Weight, ProofSize, reads/writes, touched keys, event count, custody-root invariance, non-refund of committed Trigger fees, and absence of any Task/economic adapter.
- Exact-slot custody recovery, User Immutable early-close rejection, ordinary `Transfer(AllAvailable)` source preservation, and no close-time adapter invocation.
- A zero-Step immutable AtTime one-shot fixture, reporting Creation, Trigger occurrence, delayed Pipeline Machine Opening, Action, completion, and cleanup dimensions independently; Action cost MUST be zero.
- Wasm size, run-state bytes, canonical lifecycle-state count, public event/error/API variants, Weight-owner count, and mandatory test-matrix count. `ActorFundingWait`, reactivation, and population-scale sleeping-state counts MUST remain zero.

The target fails on any combined Trigger/Pipeline charge, Actor-specific evaluation or fee for redundant latched-period source activity, duplicate deferred Pipeline, failure to re-arm from current authoritative state after latch consumption, refunded prior useful Trigger fee on Pipeline insufficiency, economic close between admitted Steps, future Action-fee prepayment, unpaid invoked Action effect, machine accounting inside Running/Suspended under P0, persistent funding wait, custody mutation during apoptosis, owner destruction of User Immutable, inability to recover by exact-slot recreation, invalid zero-Step authority, unbounded Opening work, or changed System service order.

## 4. Contract Complexity Matrix

### 4.1 Short Contracts Under Global Ceilings

Benchmark semantically identical one-, two-, and three-Step Contracts under `MaxContractSteps = 4`, `8`, and `12`. For each current Step report control/effect Weight, ProofSize, loaded fragments, state hold, create/update/close Weight, and persisted run footprint.

Raising the global ceiling MUST NOT add cold body reads, unused Predicate work, maximum-pipeline ProofSize, or a fee for unauthored machine geometry to the identical short Contract. Any fixed difference requires an explicit runtime-owned cause and production evidence.

### 4.2 Maximum Contract Pacing

A maximum 12-Step DEOS Actor with every Step executable MUST use exactly 12 distinct committed-Step blocks, with at most one committed Step in each block, and MUST NOT complete before its twelfth eligible execution block. Consecutive execution blocks are required only in a fixture that guarantees no FIFO or resource congestion. The profile records every cursor, ticket, source block, successor eligibility, execution block, outcome, and final nonce. Congestion may make completion later but never earlier.

### 4.3 Mixed Complexity

Construct:

```text
9,500 one-Step Actors
400 three-Step Actors
100 twelve-Step Actors
```

Activate all through one deterministic profile while preserving strict FIFO and block pacing. Report `S`, `A`, service gaps, progress, and completion separately by complexity class. Every admitted Actor must make eventual progress under recurring conforming capacity; short Actors receive no class priority, and maximum Contracts cannot create cold-body or whole-cycle admission cost for unrelated short Actors.

No ten-minute completion promise applies to all Steps in this mixed workload unless measured evidence independently establishes one.

### 4.4 Mixed-Length Shared-Trigger Step 0

Construct one shared Trigger instance whose affected causal cohort contains an equal deterministic mix of Contracts with lengths `0`, `1`, `4`, `8`, and `12`. Nonempty members use the same semantic Step-0 Predicate/Task/resource shape; only zero-Step absence or unreachable suffix length/content differs. Run populations `10/100/1,000/10,000` under both T+1 and any specification-admitted T0 candidate.

Report cohort discovery/materialization, first Opening/Step-0 control/effect Weight, ProofSize, reads/writes, loaded fragments, first-reaction latency, zero-Step completions, Step-0 commits, one-Step completions, and multi-Step transitions into independent Q1 Running service. After Step 0, report continuations separately by Contract length; they are not cohort work.

Initial causal candidate discovery MUST be length-blind; fixtures fund every member for its own activation quote. First reaction MUST load no tail fragment and incur no RefTime/ProofSize/read/write term proportional to unreachable Steps or tail chunks. Pipeline Machine charging consumes one fixed-size generation-bound hot-header projection; the amount intentionally follows authored bounded control geometry while projection read/encoding topology stays constant. Any cold-tail read, suffix reconstruction, or one-Step-only cohort filter fails No Ceiling Tax.

## 5. Active-Frontier Stress Profiles

### 5.1 Cadence Herd

Construct 100,000 Actors with identical or tightly clustered due ticks. Measure empty-time probes, due-page materialization, aggregate FIFO placement, partial resumption, block/tick ordering, and total convergence. Service MUST remain bounded per block, preserve temporal order, coalesce missed cadence without catch-up Cycles, and expose no unbounded scan or duplicate ticket.

### 5.2 Observation Herd

Construct 100,000 `ObservationChange` subscriptions or `ObservationCrossing` memberships on one hot feed, subject to the tested profile's explicit configured capacity. Measure publication ingress separately from deferred page/threshold discovery, candidate materialization, FIFO placement, faults, and convergence.

Publication work MUST remain cardinality-independent. Deferred work MAY span blocks but MUST preserve revision order, exact membership authority, causal delay, and bounded per-block control.

### 5.3 Million Dormant

Construct 1,000,000 Dormant identities in the stress state generator without claiming that the production runtime admits one million Active Actors. A tiny fixed active frontier is then exercised while all other identities remain Dormant.

Ordinary idle consensus work MUST use the same bounded probe/read topology as the zero- or small-dormant baseline. RefTime, ProofSize, and database reads/writes MAY differ only through a declared fixed-width encoding or database effect independent of dormant population; no loop, page walk, or identity-proportional probe is allowed. Activation of one selected identity may scale only with that identity's bounded lifecycle work.

TryRuntime or offline integrity cost for constructing/checking the full stress state MUST be reported separately from consensus idle Weight.

## 6. Claim Boundary

Passing these profiles supports only the exact populations, Tasks, runtime constants, state geometry, production Weight, and source tree measured. It does not establish:

- 10,000 maximum-cost 12-Step Cycles in ten minutes.
- One million Active Actors.
- Exact execution-block SLAs for individual Actors.
- Market success, fair price, MEV protection, or permanent liquidity.
- Equivalent throughput after runtime bounds, Weight, storage geometry, Tasks, or block policy change.

Changed assumptions require rerunning the affected project profile and invalidating or superseding architecture evidence whose baseline no longer applies.
