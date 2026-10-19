# DEOS Actors Performance Assurance Specification

- **Scope**: Project-owned production performance and stress acceptance
- **Target**: `pre-1.0.0`
- **Status**: Normative

This document defines the workloads and evidence required to accept DEOS Actors scaling claims. It does not select physical architecture, generate production Weight, replace pallet correctness tests, or make release attestation decisions. Runtime code, generated Weight, project validation, Experiment Records, and release assurance retain their respective ownership.

---

## 1. Evidence Contract

Every acceptance profile run MUST record:

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

Setup, genesis construction, prefunding, and deterministic population generation are outside the measured block interval. The measured interval begins with the named trigger operation or first baseline operation. A production acceptance profile passes only against runtime-bound production Weight and production-Wasm or stronger full-runtime block evidence. The isolated design-comparison profile in §1.5 has its own explicit evidence boundary and does not establish production acceptance. Native wall-clock or synthetic microbenchmarks may diagnose a candidate but cannot establish acceptance evidence.

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

Measurement exports MUST retain `CommittedSteps` and `DistinctActorsCommitted` as the named S/A numerators, alongside `SuccessfulEconomicSteps`, `DistinctActorsWithSuccessfulEffects`, `ActorsConsidered` and typed failure/skip/suspension outcomes. Considered Actors include inspected no-progress encounters and are not inferred from successful receipts; declare whether repeat encounters are counted separately. Control-only/no-op/StopCycle and unsuccessful typed commits follow the canonical Step disposition, not an economic-success filter. Historical all-successful records may have equal counts without changing these definitions. Missing instrumentation is reported as unavailable, never as zero.

A consumed refused Service turn under [Actors specification §3.2](../../template/pallets/actors/docs/specification.en.md#32-block-rounds-and-service-order) is a considered/admitted opportunity, not a committed Actor Step. Reports MUST separate it from S/A, Task failures, authored retry counters and Cycle outcomes, retain its incurred Control/effect cost, and expose later-resident progress or budget exhaustion. An all-refused round MUST NOT look like useful recovery merely because Service control committed or the cursor completed a scan. Reuse bounded off-chain/test collectors; this reporting requirement authorizes no measurement-only consensus flag, counter or event.

The `0.7.25` release line carried a mandatory target:

```text
10,000 completed one-Step User Actor Cycles
/ 100 eligible production blocks
= 100 cheap committed Actor Steps/block
```

That horizon is retained as a historical reference coordinate for like-for-like comparison; it is not a current release gate. For that exact Q1 one-Step witness, the horizon also implies 100 distinct Actors progressed/block. It is not a universal promise of 100 distinct Actors/block for multi-Step, mixed, market-heavy, divergent, or differently paced workloads.

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

### 1.2a Cost of a Useful Workflow

A lifecycle-economics comparison MUST define a useful workflow and its success predicate before collection: the intended economic end state, required authored Steps, permitted recovery and bounded observation horizon. A successful Action, a completed Cycle or Actor destruction alone MUST NOT substitute for that predicate. Report H1 long-running/retry, H2 passive/wake and H3 mixed-economic workloads separately; do not optimize or rank the full lifecycle from a first-block all-successful cohort alone.

For each additive cost dimension `d`, report `C_useful_workflow[d] = TotalCohortServiceCost[d] / SuccessfulUsefulWorkflows`. The numerator includes the entire declared cohort's service up to the fixed cutoff, including failed, skipped, suspended and unfinished work, not only costs of winners. Retain raw totals, original cohort size, completed-successful/failed/pending/censored counts and observation duration. With zero successes the ratio is undefined and MUST be reported as such, not zero. This finite-window ratio is not the eventual cost of censored workflows.

| Dimension | Required observation / boundary |
| --- | --- |
| User payments | Actual Creation, useful Trigger, Pipeline and Action payments by payer and asset; Task/Router fees and ordinary transaction fees separately. Retain setup payments even when outside the execution window. |
| System service | Reserved maxima and settled Control/effect Weight by phase, plus attributable mandatory/maintenance work; zero Actor fees do not imply zero block cost or an issuance-funded subsidy. |
| Execution resources | Separate RefTime and ProofSize totals and per-success ratios; booked Weight, physical execution time and measured proof remain different channels. |
| Completion time | Success-latency distribution and unfinished-work age/censoring from a fixed causal origin; do not divide total elapsed time by completions and label it per-workflow latency. |
| Held capital | Per-asset peak, release time or retained hold at cutoff; optionally exact hold-balance × block duration over the observation interval, with aggregate capital-blocks per success. Refundable holds are not fees or consumed principal. |
| Protocol Service refusal overhead | Report pre-admission refusals and admitted consumed refused turns separately, with incurred Control/effect resources and qualified post-rollback Service completion cost; neither is a committed typed Task failure. Show aggregate block usage and useful-service/fairness impact; no new Actor fee, lifetime rent or assumed monetary payment ([Actors specification §8.8](../../template/pallets/actors/docs/specification.en.md#88-bounded-protocol-service-overhead)). |
| Background/shared work | Separate direct attribution from block-wide residuals; disclose any allocation rule and retain the unallocated total. Never silently charge the same shared owner to every Actor. |

Creation is a one-time owner; Trigger, Pipeline and Action retain their independent charging boundaries. Actor lifetime itself incurs no recurring rent. For recurrent workflows, report creation separately and declare any finite amortization basis rather than assuming infinite reuse. Economic principal and swap proceeds are not service fees. Do not add currency payments, resource reservations, settled Weight, latency or holds into a scalar, or count reservations plus their settlement as two expenditures. Every cost retains its payer/resource owner; unavailable observations remain explicit.

An implementation comparison MUST hold workflow goals, initial cohort/funding, horizon and causal semantics fixed, and report protected failures, completion rate, latency, both resource dimensions and held capital alongside any gain. An external-contract/keeper comparison additionally needs equivalent goals, reliability/timing and trust assumptions, on-chain payments and disclosed off-chain operating costs; no competitiveness claim follows from Actor-only measurements. Add these outputs incrementally through existing collectors and economics/cumulative comparisons; missing optional metrics limit their claims, not unrelated qualified hot-path work.

### 1.3 Required Workload Classes

A service-throughput claim MUST cover enough distinct action classes to distinguish Actor-machine amortization from useful economic work. The minimum representative set is:

- Control-only or `StopCycle`-like cheap Step.
- Burn-like cheap effect.
- Transfer.
- Swap or another bounded market action.
- Liquidity-heavy action.

Each class uses its canonical effect owner and reports typed outcomes separately from runtime service. The scheduler MUST NOT rank by Task family, declared Weight, fee, or Actor class to improve packing. Workload-class results are separate evidence points and MUST NOT be averaged into one synthetic Actor.

### 1.4 Acceptance Classification

Evidence and requirements in this document belong to one of five classes:

- **Historical target**: a mandatory threshold of a past release line, such as the `0.7.25` horizon of 10,000 completed one-Step User Actor Cycles in 100 eligible production blocks, together with its recorded outcome. Historical targets remain discoverable and comparable but MUST NOT gate a later release.
- **Workload size and horizon**: the constructed population, deterministic fixture, and measured interval of a profile, such as the `10/100/1,000/10,000` populations or a 100-block interval. A size or horizon defines what was measured; it does not by itself define a pass condition.
- **Correctness assertion**: a required behavior such as no lost signal, no duplicate Step, strict FIFO, Q1, component-wise fit, no block Weight overrun, no unresolved worker fault, or resource-policy conformance. A correctness failure invalidates the run regardless of any throughput result.
- **Quantitative regression baseline**: a sealed like-for-like comparison result under a declared workload, semantic contract, resource policy, host/configuration, artifact identities, and measurement method. A protected metric MUST NOT regress beyond the materiality declared before measurement; a baseline value is a comparison coordinate, not a universal threshold.
- **Current release gate**: the rule a release decision actually applies. Correctness, resource-policy conformance, and declared comparative regression are gates; closure of a historical horizon is not.

Each claim MUST identify its supporting class. Reporting completion against a historical horizon remains REQUIRED for continuity, but the horizon MUST NOT be presented as a current release requirement.

### 1.5 Frozen Actors-Only Design Comparison

Actor machinery design comparisons MUST use one fixed idealized resource profile until the current design campaign converges or an explicit benchmark-contract decision replaces it. Production conditional allocation remains governed by the resource-policy specification; it MUST NOT silently change this comparative profile. The reference block envelope is fixed numerically, not read from a changing runtime `BlockWeights` getter:

| Quantity | RefTime (ps) | ProofSize (bytes) |
| --- | ---: | ---: |
| Reference block envelope | 2,000,000,000,000 | 10,485,760 |
| Non-Actor system prefix and outstanding reserve | 0 | 0 |
| Ordinary user dispatch | 0 | 0 |
| Actor Control ceiling | 666,666,666,666 | 3,495,253 |
| Shared Economic ceiling, all available to Actor effects over both passes | 1,333,333,333,334 | 6,990,507 |
| Initial Actor base turn | 666,666,666,667 | 3,495,253 |

The first baseline measurement MUST follow implementation and functional validation of the selected 10,485,760-byte common runtime/FRAME ceiling, the shared allocation/freeze mechanism, and this isolated harness. Complete production system-quarter certification and unrelated Governance/XCM lifecycle repairs are separate acceptance gates, not prerequisites for the explicitly zero-system comparison; no uncertified production cost may be discounted by invoking this isolation. Increasing ProofSize MUST NOT implicitly increase RefTime. Measurements under a different numeric ceiling are not the baseline or evidence of machinery improvement for this profile.

The prepared measurement source MUST be checkpointed locally and remain clean during the baseline run. Record the commit/tree, toolchain, workload, command, applicable Weight identity and the identity and role of the executed Wasm. A benchmark Wasm is not an accepted production Wasm. Necessary scoped calibration and functional checks precede this checkpoint and are preparation, not the throughput baseline; known underbounds or inapplicable coefficients in any executed or reserved owner invalidate charged-capacity claims. Never satisfy identity checks by selecting a previously released artifact or silently omitting a required check.

Before baseline, functional evidence MUST show that independent Control/effect refusal preserves the live head and Step progress, releases incomplete reservations, charges legitimate inspection and prevents successor bypass. Cover each Weight dimension, base/Drain phase boundaries and next-block recovery of the same head with no duplicate Step. Distinguish budget refusal, an executed business failure and a qualified consumed refused turn. The latter requires Actor rollback plus only the allowed Service-control delta, same-round suppression and later-resident progress; unqualified errors must still stop. Compare reserved maxima, charged actual work and physical costs separately; reservation-efficiency and allocation-policy changes require the measured machine baseline rather than assuming two accounting domains need two execution queues.

The profile MUST use the canonical Prepass → empty external phase → Drain progression. Actor effects may consume the unused User turn only through ordinary Drain borrowing; the harness MUST NOT grant the whole economic pool to Prepass, lend Control to effects, bypass the live head, or relax next-block eligibility and one-Step-per-Actor pacing. These are capacity ceilings, not guaranteed consumption: Control exhaustion, effect geometry and fragmentation remain measured outcomes.

Zero system work is an explicit isolated benchmark premise, not a claim that a real empty production block has zero cost. The harness MUST exclude unrelated system hooks and latent background Actors from the measured workload, identify its synthetic context setup, and retain all Actor-owned detection, admission, scheduling, fee, retry, cleanup, budget accounting and finalization costs. Canonical Task effects and their synchronous runtime adapter consequences remain charged; no Actor work may be reclassified as free setup or external system work. Genesis, prefunding and population construction stay outside measurement under Section 1.

Each baseline/candidate pair MUST retain identical numeric envelopes, workload and initial semantic state, populations, Contract/Task geometry, admitted single-Step bounds, seeds, measurement method, host/toolchain and database settings. Source, Wasm and Weight identities remain explicit for each candidate; changed coefficients and charged throughput MUST be distinguished from independently measured physical work. A change in allocation or measurement method starts a separately labelled comparison and requires a matching baseline; it MUST NOT be reported as an Actor machinery improvement.

This profile is the primary controlled coordinate for selecting Actor designs, not production acceptance or a replacement for the workload classes and adversarial scenarios in this specification. Full-runtime quiet/system-saturated, session-rotation, Governance/MQ and mixed-user profiles remain separate correctness and release checks. They MUST NOT redefine the frozen idealized comparison envelope. Historical measurements under other budgets remain labelled historical coordinates and MUST NOT be compared directly as speedup factors.

### 1.6 Incremental H1 Diagnostic and Qualification

H1 MUST first exercise one ordinary Mutable User Actor independently at lengths `1/2/4/8/12`, with Manual readiness, no predicates, `on_unfunded = Close`, absent `close_when`, no Parked balance activation, no window or nonce terminal, and one fixed Native Transfer of one existential deposit per Step to an already live, non-Actor recipient. Funding is OwnerOnly. Each length starts from the same synthetic ledger genesis without background Actors, publicly enables User creation, prefunds the prospective sovereign outside measurement, and uses public Actor creation and readiness calls. Test-only aging of stored control clocks MUST NOT substitute for advancing to a later block.

The initial declared-cost diagnostic MUST retain the §1.5 numeric envelope and canonical Prepass → empty external phase → Drain → finalization. Creation occurs at block 1, Manual readiness at block 2, and no Step commits before block 3. Under this singleton uncongested fixture, Step indices `0..length` commit exactly once at blocks `3..=length+2`; completion leaves passive Idle without Run or Service residence. Report sovereign debit, recipient credit, Fee Sink credit, hold preservation, Creation and Trigger fees, one Pipeline payment and pay-as-attempted Action payments independently. Ordinary signed-transaction/extension cost is excluded from these direct public-call diagnostics and MUST NOT be reported as measured.

Reports MUST identify native executable bytes, content-bound source, applicable Weight, Contract encoding, compiler/profile, effective Active limit, workload revision and population. Per-block declared Control/effect vectors use `[RefTime ps, ProofSize bytes]`. Post-trigger Control totals include the signal block's otherwise empty Actor phases but exclude Manual occurrence and creation-call execution; report the selected Trigger-control vector separately. Fee amounts use exact integer strings. Residence endpoints may prove retained links and passive completion but MUST NOT be advertised as logical insert/unlink/write counts.

A singleton declared-owner ledger MUST name each selected generated owner and assert that their complete two-component sum equals the block's settled Control vector. Include fixed/no-work polling, eligible inspection, retained Service suffix, fee bookkeeping and any retained terminal envelope rather than reporting only the inner Step. Distinguish installed arithmetic from applicability or physical soundness; rows consuming an unqualified class/branch owner MUST carry an explicit qualification limit. Retained maxima MUST NOT be reported as exact performed work, and a profile containing Task effects MUST NOT be treated as an independent pure-Control measurement.

The retry companion MUST use a three-Step no-policy User workflow: a Native Transfer prefix, a Local Transfer of one unit to an initially empty recipient, and a Native Transfer suffix. The sufficient Local asset has minimum balance two; the sovereign starts with three units. Its host deposit preflight MUST produce the real Temporary `RecipientDepositUnavailable` outcome, not a substituted provider or fabricated suspension. The middle Step authors `RetryLater { max_attempts: 3 }`; native fee capacity remains sufficient throughout. A terminal companion changes only the authored nonce target to one and MUST close synchronously after the successful suffix without economic unwind.

Both companions MUST retain the successful prefix at block 3, paid failures at blocks 4 and 5, the second failure's Block-7 Deadline, no attempt at block 6, a Control-only Block-7 return eligible at block 8, recovery success at block 8 and the suffix at block 9. Recipient recovery is a public mint of two units at block 6 outside isolated Actor phases; its external execution/transaction cost is explicitly unmeasured. Report three successful Transfers separately from five typed Attempt outcomes and five Action payments; failures are not economic success. Assert one Trigger and Pipeline payment, exact local/native custody, same-cursor failure counts/reset, prefix durability, and complete Actor-state/hold release only for the terminal companion.

Resource controls MUST independently refuse a funded ready-Idle and Running User head on each RefTime/ProofSize component of both Actor Control and shared Economic admission. Use two public three-Step User Actors so refusal cannot silently bypass the head. A bounded real economic prefix MAY place them at Drain; any setup removal uses the ordinary public Mutable lifecycle and reports its selected dispatch accounting separately. Creation/readiness clocks MUST advance normally. Recovery MUST serve both heads in order exactly once, retain their holds and committed prefixes, and restore Q1/B+1 without recharging a Running Pipeline.

Counterfactual prior usage MAY isolate each admission frontier only inside the existing frozen limits. Identify its domain, selected/settled Weight and limiting component; the chosen component MUST leave one unit less than required while the other still fits. Report performed inspection/housekeeping separately from seeded consumption. A resource refusal MUST preserve both User semantic/process/Run/Contract/residence states, custody and fee/event ledgers; it MUST NOT charge Pipeline or Action fees. These controls are separate from the empty-external singleton comparison and do not certify the prefix adapters, complete signed-block costs or capacity.

An isolated empty control MUST execute real Actor initialize/Prepass/Drain/finalization with zero Actor identities, not merely an empty observer closure. Report its housekeeping without economic effects or fees. It MUST NOT be represented as the whole-runtime quiet control, whose production background Actors and other hooks remain independently required.

A diagnostic MAY precede complete collectors and owner qualification, provided it lists every unsupported metric and makes no physical-cost, capacity or improvement claim. Qualification additionally requires independent resource refusals and complete collector/owner coverage for the success, retry and terminal paths. Record collector boundaries for provisional storage operations, selected/reserved/settled Control/effect and all reachable failure suffixes. The qualified baseline MUST freeze those inputs before candidate changes; deterministic traces must reproduce exactly, while any physical timing comparison retains the matched inclusion/noise rules in §6. Neither successful singleton execution nor unchanged declared Weight establishes sound maxima.

A core geometry certificate MUST name the frozen input families, Actor/generation/ID horizon, coexisting versus mutually exclusive residences, head/tail/Run geometry and exact Deadline/reverse-index cleanup. Validate committed boundary coherence outside observed execution, including absence of unsupported companions and orphan state after Close. Native validation may enumerate its explicitly bounded fixture; this MUST NOT imply that the consensus executor enumerates the ring. Report encoded value sizes as fixture observations unless a separate source/codec argument establishes a maximum; they are not trie-proof bounds. Qualification of resource/fee owners remains independent.

Logical-access evidence MUST name its observable API methods, execution phases, omitted channels and transaction semantics. Capture provisional operations even when their values later roll back; distinguish submitted payload bytes from independent pre/post committed-state differences. Same-value Puts and absent Deletes are operations, not net changes. Per-phase reports MUST distinguish measured zero from unavailable; a missing channel MUST NOT be represented as zero. Aggregate owner names against current storage metadata without treating a shared prefix as one key.

Observer setup, state snapshots and verification reads MUST remain outside the collector. Validate scope exclusion, missing keys, repeated reads, same-value writes, rollback and payload encoding; then compare observed and unobserved workflows against identical final state roots and declared/economic traces. An implementation A/B MUST retain the actual final-root vectors for both versions, not infer cross-version equality from separate intra-version observer-equality booleans. Reporting-only exports stay after measured execution and use the identical harness in both versions. Snapshot traversal may warm native backend state, so logical event counts alone MUST NOT support CPU, backend-I/O, trie-proof or capacity claims. A repeated read or unchanged final value is a candidate observation, not proof that its removal preserves intermediate authority, callback freshness or rollback.

### 1.7 Practical Transfer and Four-Predicate Router Profiles

The two primary practical action profiles are a Native Transfer to one shared existing recipient and a foreign-to-Native DEOS Router purchase preceded by exactly four true balance predicates. Run System and User populations separately with ordinary class-specific fees. Report committed successful Actions and distinct served Actors per complete block, full distributions across a declared interval, both Weight dimensions, Control/effect ownership, fees, route/outcome and execution/verification proof. Do not infer an execution ceiling from a readiness-source-limited window or silently replace charged-capacity observations with physical CPU claims.

The Router profile uses one AND clause with four distinct balance keys on each Actor's own sovereign: Native, traded Foreign and two separately funded witness assets. All four predicates must be evaluated by the real Step machine and remain true throughout the declared horizon; no duplicate-key predicates, stubbed evaluator or shared-account substitution. The purchase is a bounded exact-output `SwapOut` for a small fixed Native amount paid with Foreign, with a finite input cap and slippage bound. Record actual input/output, Router fee, selected family/legs and custody deltas; exact-output is not reported as exact-input.

Prepare deterministic asset/pool depth and complete horizon prefunding outside measurement, preserve reference Actors/hooks and ordinary Q1/B+1/FIFO/budgets. No mid-window top-ups or resets may hide price drift, false predicates or failed Tasks. A finite first-block profile pairs the existing128-Actor Manual Transfer setup; sustained Running/recurrence profiles are separate scopes and must identify their readiness supply, Step/Cycle boundaries and warmup coverage. Inability to maintain successful purchases or admission is an explicit result, not permission to count skipped/failed work as swaps or weaken guards. Current owner/artifact applicability and release qualification remain separate gates.

## 2. Ten-Thousand-Actor Reference Profiles

These profiles construct the population and horizon of the historical `0.7.25` target:

```text
10,000 completed one-Step User Actor Cycles
/ 100 eligible production blocks
```

They are retained as reference workloads for like-for-like comparison and so that the recorded `0.7.25` shortfall stays visible. Completion against the historical horizon MUST be reported, and shortfall inside it is a service result rather than a correctness failure. A reference profile run MUST remain admissible and bounded and MUST fail on a lost signal, duplicate Step, FIFO violation, block Weight overrun, unresolved worker fault, resource-policy breach, or accounting inconsistency. It MUST report `S`/`A` per block and each admitted Actor's completion block or explicit unserved/censored disposition across the run and any declared bounded extension horizon. Eventual progress under recurring conforming capacity remains required as in Section 4.3.

### 2.1 Reactive Transfer

Construct 10,000 prefunded one-Step User Actors on one feed:

```text
Cadenced (one shared due tick)
→ Fresh ObservationAbove precondition
→ Transfer
→ completed Cycle
```

The shared due tick is reached in block `N` while the authored observation predicate is true; publication itself activates no Actor. Report each Actor's first completed one-Step Cycle and aggregate coverage against the historical horizon `N + 1 ..= N + 100` and against any declared bounded extension horizon. Report `S` and `A` per block even though both count the same committed population in this exact Q1 one-Step profile.

The Transfer destination and amount geometry MUST be identical across the population and executable throughout the run. Setup MUST isolate scheduler/control throughput from avoidable recipient-creation or insufficient-funding failures while retaining the real certified ingress and ledger consequences of the canonical Transfer operation.

### 2.2 Contended Transfer

Repeat the Reactive Transfer profile while valid ordinary external extrinsics continuously consume the complete User base turn in every measured block. Actors complete under their guaranteed Actor base turn; report the same coverage, horizon, and contention evidence as Section 2.1.

The external workload MUST be deterministic, independently successful, and sufficient in both Weight dimensions to demonstrate base-turn contention without exceeding runtime validity. The report MUST show `S`, `A`, Actor/user consumption, borrowing, service gaps, and component-wise head fragmentation independently.

### 2.3 Reactive SwapOut

Construct 10,000 prefunded one-Step User Actors:

```text
Cadenced (one shared due tick)
→ Fresh observation precondition
→ bounded SwapOut
→ committed typed Step disposition
```

The shared due tick in block `N` causes readiness; the observation predicate is evaluated at each Attempt. Report each Actor's typed Step disposition and coverage against the historical horizon `N + 1 ..= N + 100` and against any declared bounded extension horizon. The market fixture MUST use deterministic bounded liquidity and authored input protection; it MUST NOT fabricate 10,000 stale shared quotes or promise that every market operation succeeds economically.

Report runtime service separately from economic outcome:

- Runtime service: The Step committed one canonical `StepOutcome` without duplicate, loss, FIFO violation, or block overrun.
- Economic success: The Task returned the successful SwapOut outcome.
- Economic non-success: Precondition skip, funding outcome, typed Temporary/Permanent failure, suspension, or terminal policy disposition.

The throughput measurement concerns committed typed dispositions. Economic success rate is a separate measured result and MUST NOT be relabeled as scheduler failure or guaranteed market execution. Report `S`, `A`, action Weight, and Actor-machine Weight independently so market cost cannot be mistaken for scheduler policy or overhead.

## 3. Baselines and Differential Overhead

### 3.1 Manual Baseline

Measure the same one-Step Transfer and SwapOut Contracts, populations, balances, Tasks, and block policy through Manual readiness without observation detector work. Under the production `NextBlock` timing baseline, Manual signals remain causal next-block obligations; setup MAY batch deterministic signal extrinsics only when their transaction Weight is reported separately from Actor Control and effect execution.

The baseline isolates detector/materialization work from current-Step control and Task effect work. It does not replace the reactive reference profiles as comparative evidence.

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

- Trigger occurrence service by `Manual`, `AddressEvent`, `AtTime`, and `Cadenced`, separating source publication/ordinary transaction work from the disjoint User Trigger Fee.
- Independent Trigger and Pipeline deltas for: Idle useful occurrence with B+1 Opening; busy Running/Suspended source activity that preserves custody and current service authority without a latch or fee; redundant latched-period sources that perform no Actor-specific evaluation or charge; delayed Opening after retained Idle `pending_signal`; paid useful Trigger followed by unavailable Pipeline Machine capacity; and a zero-Step immutable AtTime one-shot.
- For every separation fixture, component-wise incremental RefTime, ProofSize, database reads/writes, accounting touches, fee movement, latch transition, re-arm, and Cycle transition attributable separately to useful occurrence materialization and Pipeline Opening.
- Running/Suspended current-Step cost at every cursor, proving no balance viability read, machine charge, machine hold, or suffix reconstruction after Opening.
- Success, typed failure, retry, and `FundingUnavailable` Action paths: every invoked Action pays valid actual effect Weight; an unfunded non-invoked Action pays zero and follows prepaid control policy without apoptosis.
- Pipeline-admission-insufficient minimal apoptosis Weight, ProofSize, reads/writes, touched keys, event count, custody-root invariance, non-refund of committed Trigger fees, and absence of any Task/economic adapter.
- Exact-slot custody recovery, User Immutable early-close rejection, ordinary `Transfer(Percent(100%))` source preservation, and no close-time adapter invocation.
- A zero-Step immutable AtTime one-shot fixture, reporting Creation, current applicability check, delayed control admission, Action, completion, and cleanup dimensions independently; Action cost MUST be zero.
- Wasm size, run-state bytes, canonical lifecycle-state count, public event/error/API variants, Weight-owner count, and mandatory test-matrix count. `ActorFundingWait`, reactivation, and population-scale sleeping-state counts MUST remain zero.

The fee-boundary profile fails on any combined Trigger/Pipeline charge, Actor-specific evaluation or fee for redundant latched-period source activity, busy readiness latch or deferred Pipeline, failure to re-arm from current authoritative state after latch consumption, refunded prior useful Trigger fee on Pipeline insufficiency, economic close between admitted Steps, future Action-fee prepayment, unpaid invoked Action effect, machine accounting inside Running/Suspended under P0, persistent funding wait, custody mutation during apoptosis, owner destruction of User Immutable, inability to recover by exact-slot recreation, invalid zero-Step authority, unbounded Opening work, or changed System service order.

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

### 5.2 Million Dormant

Construct 1,000,000 Dormant identities in the stress state generator without claiming that the production runtime admits one million Active Actors. A tiny fixed active frontier is then exercised while all other identities remain Dormant.

Ordinary idle consensus work MUST use the same bounded probe/read topology as the zero- or small-dormant baseline. RefTime, ProofSize, and database reads/writes MAY differ only through a declared fixed-width encoding or database effect independent of dormant population; no loop, page walk, or identity-proportional probe is allowed. Activation of one selected identity may scale only with that identity's bounded lifecycle work.

TryRuntime or offline integrity cost for constructing/checking the full stress state MUST be reported separately from consensus idle Weight.

## 6. Current-State Workload Matrix

Every current-state Actors comparison and the final production binding MUST use this fixed matrix, horizons, censoring and materiality rule, declared before any result. A workload whose mechanism the runtime does not ship is omitted and reported as such; it MUST NOT be replaced after results are known.

| Workload | Geometry | Purpose |
| --- | --- | --- |
| W1 — long Live economic flow | 32 Actors (8 Fee Sink splitters, 8 Burn, 8 liquidity, 8 Users), 8 Steps each over 64 blocks, every Step `Percent` of current Available | Useful work, current amount, multi-Step residency, heterogeneous goals |
| W2 — funded adjacent retry | 32 three-Step Actors; Step 2 fails temporarily twice, then succeeds after funding or market recovery; 16 blocks | Same-cursor retry, committed prefix, fresh inputs, no second Cycle |
| W3 — recurrence matrix | 8 each level-sensitive `Cadenced`-plus-predicate, coalesced `AddressEvent`, one-shot `AtTime` and `Manual` Actors; 32 blocks | Recurrence counts, no same-block restart, no busy future-start latch |
| W4 — true/false churn | 64 idle Actors alternate start truth each block for 64 blocks; half parked-balance episodes, half timed review | Park/Pending/Live churn and negative-check ownership |
| W5 — sparse Park | 10,000 identities: 32 Live, 32 Pending, 64 timed review, 9,872 parked-balance episodes; 64 blocks | Population independence |
| W6 — revision storm | 1,024 parked-balance Actors across 32 pages; one watched asset receives 4 qualifying updates per block for 32 blocks under capacity-limited Pending | Coalescing, revision acknowledgment, saturation and wake delay |
| W7 — Sleep and reentry | 1,024 Sleepers across 32 C32 slots and 32 deadline keys; 32 due per block for 32 blocks | Deadline order, B+1 eligibility, no range scan |
| W8 — owner mutation | 64 Actors; 8 head/interior/tail/singleton mutations per block across Live, Pending, Sleep and Park; 16 blocks | Arbitrary unlink, round integrity, replacement isolation |
| W9 — heavy effects under demand | 96 Actors (32 Transfer, 16 Burn, 16 Router swap, 16 liquidity, 16 staking); 32 blocks plus valid User/Router demand | Contention, effect settlement, capacity-first stop |
| W10 — close under demand | Close 256 maximum-state Actors at block 1 while W1 runs 64 blocks; recreate 32 identities | Close cost, retained bytes, generation isolation |
| W11 — resource-blocked head | 32 Live Actors whose first transition is just above remaining per-pass resources, followers below; RefTime-only and ProofSize-only boundaries | Head priority, refusal retention, component separation |
| W12 — bridge goals | One Fee Sink cycle, one foreign-fee Burn, one liquidity provision, one User three-Step transfer/retry; 32-Actor replicated cohort | Economic-goal bridge without semantic or order equivalence |

| Rule | Contract |
| --- | --- |
| Demand modes | Actor-only; mixed valid demand alternating ProofSize-heavy remarks and RefTime-heavy Router/User calls up to the first component refusal; maintenance burst at a declared block |
| Horizon | 64 blocks for W1, W4, W5 and W10; other workloads use their table horizon plus 32 drain blocks when work remains |
| Censoring | Classify every intended workflow at horizon end as complete, Live, Pending, Sleeping, Parked, Disabled, closed or faulted with cursor/deadline and reason; unfinished work MUST NOT leave the denominator |
| Order | Equal-semantics comparisons MUST produce identical encounter and Q1 digests; bridges report order differences beside outcomes |
| Capacity | Exact final component budgets and generated owners; a supported transition that cannot fit a full block is an admission defect, not censoring |
| Timing | Deterministic counts reproduce bit-for-bit; native timing uses A/B/A/B with one warmup pair and at least five measured pairs; a RefTime delta inside the matched baseline envelope is inconclusive |
| Materiality | A useful-benefit claim requires one additional complete W1 workflow under the same budget, or removal of at least one persistent scheduler key mutation per successful interior Step with lower total scheduler write bytes and no regression in completed workflows, p95 delay, ProofSize, retained state or debt |

W4–W10 pass their maintenance criterion when unserved work does not grow over the declared drain horizon; complete recovery inside the horizon is reported separately and is not implied by that pass.

## 7. Claim Boundary

Passing these profiles supports only the exact populations, Tasks, runtime constants, state geometry, production Weight, and source tree measured. It does not establish:

- 10,000 maximum-cost 12-Step Cycles in ten minutes.
- One million Active Actors.
- Exact execution-block SLAs for individual Actors.
- Market success, fair price, MEV protection, or permanent liquidity.
- Equivalent throughput after runtime bounds, Weight, storage geometry, Tasks, or block policy change.

Changed assumptions require rerunning the affected project profile and invalidating or superseding architecture evidence whose baseline no longer applies.
