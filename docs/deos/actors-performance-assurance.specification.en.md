# DEOS Actors Performance Assurance Specification

- **Scope**: Project-owned production performance and stress acceptance
- **Target**: `pre-1.0.0`
- **Status**: Normative

This document defines the workloads and evidence required to accept DEOS Actors scaling claims. It does not select physical architecture, generate production Weight, replace pallet correctness tests, or make release attestation decisions. Runtime code, generated Weight, project validation, Experiment Records, and release assurance retain their respective ownership.

---

## 1. Evidence Contract

The project-wide [State Witness and Resource Accounting Contract](state-witness-accounting.contract.en.md) owns witness sharing, resource coordinates and matching-unit transfer rules. This specification applies them to concrete Actors/Governance workload and collector gates; it does not maintain a second general proof-accounting contract. Shared payloads, repeated work, database counters and pricing MUST remain independent evidence channels.

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

## Bounded Governance Lifecycle Admission Bridge

This optional profile answers one question: can a fixed signed Intent/NoVotes lifecycle supply a locally Executive-admitted parent and one exact-default-Wasm cleanup child without state fabrication? It is an admissibility bridge, not a dominance argument, physical pricing profile or additional universal release prerequisite. Passing it MUST NOT replace complete fixed-owner qualification or restore the system quarter.

The sole candidate is the development-preset lifecycle owned by `template/runtime/src/tests/governance_integration_tests.rs::same_expiry_domain_indices_follow_signed_intent_on_development_preset`. Its native Governance-only history is a causal blueprint, not an admitted parent snapshot. The packet is one Alice preimage note for the existing advisory bytes, then witness/submission for domain101 and witness/submission for domain102, all signed at block1, Ordinary cadence, Intent, item110. Real signed extensions, nonces, transaction fees, opening fees and witness deposits apply; direct pallet dispatch is not equivalent admission. The external packet adds no vote, Root resolution, token issuance or asset registration.

With the selected reference constants, both proposals mature at144001, expire at144017 and are cleaned at child E=144018; parent P=144017. These dates are the frozen candidate expectation, not permission to set the clock or cursor. A different outcome/date requires stopping this candidate, not extending the horizon or silently rewriting the expectation. The selected population is n=d=2 with singleton discovery indices and empty pending execution, not a smaller production cap.

| Gate | Required evidence before promotion | Smallest falsifier |
| --- | --- | --- |
| Source-only preflight | Exact packet/order/encoding, genesis and declared two-axis admission; hook/callback closure preserving the selected Governance projection; reusable complete block assembly, inherent, seal, import and export owners; explicit artifact issuance plan | A missing context owner, packet that cannot fit block1, unexplained writer, copied consensus runner or required state shortcut |
| Causal native construction | Runtime-owned preset with real bound CODE; linked blocks1..E with full Executive phases, checked inherents, actual configured Actors and no post-genesis policy injection | Skipped block/hook/Prepass/tail, manual System/Governance cursor or phase write, seeded outcome, disabled Actor, unadmitted extrinsic or budget change |
| Parent/child boundary | One committed backend base for P, its actual header/root and complete TOP; native import of P and child against their complete authoring oracles through the existing import/seal owners | Overlay cursor combined with older values, manufactured high-number parent, header/root mismatch, omitted delta or import failure |
| Source projection | Both actual NoVotes records and singleton indices retained until their real cleanup, exact metadata/pending/winning/urgency presence and removal, custody and fees independently reconciled | Unexpected publication, pending effect, changed cleanup date, orphan state or ledger mismatch |
| Exact child handoff | Current-tree default code/source/Weight/configuration identity, one sealed child, compact proof and complete expected ValidationResult from the same oracle; one positive full-output PVF result | Stale/benchmark code, feature/binding mismatch, proof/root/output mismatch, missing mandatory validation work or rejected child |

Preflight MUST be a read-only call-site/admission certificate before builds or history execution. Reuse the fixed-owner and carrier maps, existing lifecycle/presence proofs, `actor_retention_fixture/{block,inherents,native_api,pvf}.rs`, and the selected validation-host tool. Do not invent a second runner or repeat the native-only history to reconfirm it. Preflight MUST identify how the selected production-compatible native binding will be established, what the existing assembler can actually provide, and every missing seam; availability of signing/export helpers alone does not establish a complete assembler. Missing required evidence leaves the profile not execution-ready.

All intervening blocks require actual linked Executive context; sealing a native hook-only snapshot is insufficient. Reference System Actors, consensus hooks, user-dispatch admission and finalization remain enabled. Transaction fees and autonomous allocation may change signer/Fee Sink balances relative to the old native-only ledger: reconcile their actual owners instead of copying that ledger assertion, suppressing those flows or silently accepting unexplained deltas. Zero genesis protection issuance is a checked premise of this selected no-vote trace, not a lifetime supply or authority cap.

Local admissibility MUST state its trust boundary. The existing relay-state proof builder provides synthetic internally consistent relay input; it does not authenticate a live relay-chain ancestry. Native ancestor checks plus one exact-Wasm child MUST NOT be labelled an exact-Wasm ancestor history, network consensus acceptance or deployed-chain provenance. If a consumer requires those stronger claims, this profile is insufficient and MUST stop at that boundary.

| Coverage dimension | This bridge can establish | Explicitly outside scope |
| --- | --- | --- |
| Publication | Two ordinary signed Intent submissions and actual NoVotes maturity | Approval cohorts, terminal rate maxima, new Root/carrier authority |
| Cleanup | Two domains, singleton indices, exact selected entry phase and presence/absence removals | Full indices, repeated large filtering, maximum bucket/domain population or service debt |
| Pending | Empty-pending probe in the real caller | Full ring, executable payloads, Invoice/dispatch failures, retry or rollback envelopes |
| Runtime context | Complete selected native blocks and one sealed/proof-bound child, with configured endogenous work | Maximum Session/XCM/message context or other fixed-owner envelopes |
| Final root | Entire actual child delta, original paths/values and repair required by its real backend | Universal mixed-delta bound, benchmark commit cost or native persistence charged to PVF |
| Cost/evidence | Quoted Weight and observed proof/output with explicit identities | Conservative RefTime/PoV price, total ValidationHost CPU, additive helper/root pricing or quarter credit |

The execution allowance, only after preflight closure and a separately selected implementation/validation slice, is one deterministic construction through E and one positive one-child PVF case. Setup stays outside the child interval. Retain one parent/child pair and complete expected oracle rather than exporting an ancestor corpus. No parameter grid, timing repetitions, corpus reopening or automatic second candidate is admitted. Fresh default code issuance uses `scripts/03-build-runtime.sh` with SKIP_WASM_BUILD absent; an old successful artifact or exit status cannot supply current-tree applicability.

Stop on the first falsifier, unsupported mandatory layer, need for a policy/cap/budget change, unexplained source/artifact difference or requested stronger claim. Record failure as inapplicable or inconclusive with its missing owner; do not repair admission by padding, count/date edits, fabricated funding or optimistic domination. On success, stop with only the narrow local-admissibility/child-handoff claim. Further pricing or dominance requires its own declared coverage and conservative both-axis owner proof; neither a favourable sample nor larger synthetic population supplies it.

## Governance Whole-Hook Cleanup Control

This finite control asks whether one marked complete Governance initialization call is observable with its enclosing clock/pending/phase work, rather than treating helper timing as an inclusive owner. It MUST use the existing `on_initialize_finalized_cleanup` extra and canonical benchmark script, never a copied runner. It is not a replacement production price, complete Governance domain or universal release prerequisite.

| Dimension | Frozen control contract |
| --- | --- |
| Cells | Four exact cells: n1/1024 crossed with initial Maturing/FinalizedOutcome; p0/1 are categorical, never regressed as a continuum |
| Source | One full single-domain discovery index, maximum completed records, empty pending and maturity input, cursor E−1/current epoch E; existing peer/future-state verification remains enabled |
| Issuance | Fresh applicable benchmark-feature artifact, exact source/feature/configuration/Weight identity; old default/benchmark artifacts cannot supply applicability |
| Sampling | One separately retained excluded warmup invocation per cell (repeat1), then one included invocation per cell (repeat3); exact equal low/high components, two steps (the pinned SDK minimum), zero minimum duration; retain actual SDK body/tracking multiplicity, including its four identical coordinate slots for two components |
| Output | Exact extra/raw-only JSON; retain every observation, component/verification result, unit and artifact hash; no fitting, generated coefficient, merge, margin or post-hoc point exclusion |
| Stop | A verification/input/artifact mismatch, unavailable required layer or unexplained model undercoverage stops this control; do not expand cells or change caps/phase/state to make it pass |

Recorded proof and a full original witness are distinct. Any comparison to a source-input upper MUST identify its same-backend ordinary-key, presence/value, foreign-node/repair and recording/whitelist premises. Combined original encoded value-entry mass is not an unconditional floor for a whitelisted, cached or incremental recorder delta. Body timing, modeled DB terms, physical process/worker CPU, post-stop commit/root and network PoV MUST remain separate owners and units.

This control MUST NOT be added beside contained pending/catch-up/cleanup declarations as a surcharge. An inclusive production replacement needs explicit retirement/non-overlap, conservative both-axis coverage over all supported source/failure domains and applicable host/root transfer. Passing these four cells supplies only the declared complete-hook control observations; it does not certify distinct full indices, nonempty pending/effects, worst-case history, a safe cap or system-quarter restoration.

## Governance Domain-Diverse Whole-Hook Source

The domain-diverse complete-hook source MUST reuse the existing n-domain/shared-population preparation and preservation verifier, wrapping the actual `Hooks::on_initialize` rather than transferring helper timing to its caller. `on_initialize_finalized_cleanup_diverse(n,p)` selects n2..1024 distinct due-domain indices and categorical initial phase p0/1 (Maturing/FinalizedOutcome). One common fixture population is distributed across those domains; it is not a product of per-domain maxima, a global retained-state bound or maximum approved-receipt geometry.

The source MUST require matching host epoch, empty pending/maturity input, cursor E−1 and one due identity per selected domain. Verification MUST preserve all non-due records/index tails, future buckets and pending authority, finish in RewardExpiry and retain cursor E−1. Native source checks collect no physical samples. Exact extra/raw-only selection is required; default/bulk inclusion, Weight generation and merge are forbidden. Physical collection needs its own declared input/model/host question and applicable artifact, not a replay of the closed single-domain grid. This source alone supplies no RefTime/ProofSize upper, original-path/root transfer, joint-history attainment or system-quarter credit.

## Independent Full-Index Cleanup Source Gate

Before selecting another physical point for the independent-index gap, the finite source gate MUST prepare exactly two domains at their configured discovery capacities, with equal ordered ID lists and one due identity per domain. Every indexed ID MUST have its finalized record and exact terminal-expiry touch created through the existing lifecycle owner; a two-key read projection with missing peer records is not sufficient for this gate. It MUST preserve all capacities, use no production observer/algorithm change and add no sampling or Weight binding.

Terminal epochs start at1 for the two due identities and at2 onward for peers, respecting the shared expiry-bucket capacity. The service clock MUST be at least the latest terminal epoch and selected old expiry; the completed cursor is selected expiry−1. Preparation MUST expose current epoch and old cleanup epoch separately, keep pending/maturity empty, and verify no future terminal date or already-completed cleanup touch. The selected epoch MUST remain within the actual hook's bounded scan. Source/native verification MUST reconcile Governance try-state before/after the actual hook, preserve all peers/future buckets/pending authority and end RewardExpiry with the old cursor unchanged.

This is a prepared internally reconciled state, not public construction history, Executive admission, proved service-debt attainment, complete PoV or a physical maximum. A bounded direct cursor seed is explicit fixture setup and MUST NOT be relabelled historical progress. Source-gate success permits only consideration of a separately declared physical method with an applicable artifact; it does not authorize collection/generation, infer a global population bound or price the independent-domain maximum from n2.

## Independent Full-Index and Pending Composition Source

The finite joint source MUST compose the two-full-index cleanup component with the existing complete pending topology, without changing either consensus algorithm or capacity. Prepare cleanup before pending, retain the old completed cursor/FinalizedOutcome phase and raise the current clock sufficiently for every due/future approval to predate preparation and satisfy its ordinary enactment delay. The two cleanup domains MUST be disjoint from pending and pending-helper terminal peers; terminal destination buckets MUST not collide with cleanup-owned retained buckets. This is prepared chronology/debt, not public ancestry or debt attainment.

One actual hook MUST execute both owners. Separate component verifiers MUST preserve every cleanup peer/later bucket, old cursor/phase result and pending future membership/receipts/destination capacity. The empty-pending whole-hook wrapper MUST retain its original pending-membership/cursor neutrality checks; a regression MUST reject a changed cursor rather than relaxing the control for composition. The finite reference case uses all128 pending slots/four due effects plus n=d2/S32768 old cleanup, not maximum joint n/d/S, payload/error/rollback coverage, physical timing/PoV, default-host execution or a production price. No new sampler or Wasm follows solely from source conformance.

For a joint full-destination refusal, the exact combined-state expectation MUST be constructed before execution from the committed parent, permitting only old cleanup's independent finite delta and bounded pending-cursor progress. It MUST compare complete committed TOP and V1 root, preserving every other field including pending authority, receipts, destination, effects and events; a pending-only root comparison cannot substitute. Keep the standalone pending-refusal guard unchanged. Terminal cleanup-capacity admission occurs before preimage lookup/executor invocation, so this case MUST be labeled pre-effect refusal, not invoked-effect or late rollback qualification. This source adds no price, timing, PoV or host maximum.

## Independent Full-Index Original-Input and Delta Transfer

The finite native/SDK transfer gate MUST reuse the n=d2 full-index prepared source and unchanged actual hook. It MUST preserve its live clock/cursor in committed snapshots, compare plain/observed complete TOP and returned Weight, and capture every logical point key through the existing SDK event observer. Prefix/root operations outside that observer's point domain MUST fail closed; setup, snapshots, verification and root diagnostics remain outside capture. The benchmark-genesis helper's deliberate Number normalization MUST NOT be used as a live-state snapshot.

Build the exact original V1 trie and net delta from those committed states. Every changed key MUST have an observed mutation owner. The diagnostic may read full original values for every point key as an explicit input superset, not a claim of minimal actual read proof. Whole-delta recording MUST classify extra nodes against the pristine original database; computed/new nodes MUST NOT be supplied as old input. Replaying all original point reads and the delta from original witness plus required original repair inputs MUST match both the runtime post-state root and independent final-state reconstruction. Removing the required shared old-index blob MUST fail its explicit reads even if an overwrite-only root computation would not need it.

This gate qualifies only the selected prepared native/SDK input-to-delta case. It supplies no general repair/proof upper, minimal proof, physical CPU, complete PVF/network PoV, admitted ancestry, maximum-domain transfer or production price. Zero observed extras is not a general zero-repair rule. No closed timing/history corpus, new Wasm, sampler or coefficient is admitted by this gate.

## Complete Governance Owner Method Selection

An emitted physical control MUST close a named coverage gap in a declared owner, not merely confirm another prepared point. Before build/collection, bind the following finite method contract. Existing source/native controls remain reusable evidence, not an implicit executable pricing campaign. No method is execution-ready while one of its required input, host or accounting cells is unsupported.

| Required cell | Admission obligation | Refusal/stop condition |
| --- | --- | --- |
| Source | Classify read-input projection, reconciled prepared state or causally admitted parent; bind clock/cursor/phase, n/d/S, pending visits/topology and record/metadata presence | Try-state or typed maxima promoted to public history; seeded dates/counts used as admission repair |
| Logical versus original input | Retain repeated decode/retain/encode operands separately from original keys, required distinct external value hashes/payload mass and recorder/cache/whitelist scope | S or per-domain payload mass treated as a full-witness/recorded-proof floor despite aliasing; guessed global population cap |
| Effects/errors | Declare reachable payload/result branches, terminal admission, missing preimage, invoked pre-transfer error, nested Err and publication; reuse source-excluded late paths only under their exact fixed-binding premises | Pre-effect refusal presented as invoked mutation/late rollback; unreachable fault injected to manufacture reference pricing |
| Host/axes | Name exact artifact, feature/configuration, caller, collector, supported physical RefTime/ProofSize model and variability/undercoverage falsifier | Raw maxima, logical bytes, modeled DB terms or native body time substituted for a conservative complete owner |
| Delta/root | Bind original-input lineage and whole mixed delta/repair; distinguish body, SDK post-stop root/commit, native persistence and validation-host execution | Helper/root subtraction, execution-order repair elision, proof replay promoted to CPU or network PoV |
| Inclusive replacement | Identify which pending/catch-up/phase declarations retire and how common fixed/root work is charged once on both axes | Whole-hook surcharge beside contained owners, overlap credit or restored quarter without complete qualification |

Before scoped calibration, require a supported candidate model family, applicability/transfer rationale, collector boundary and rejection rule, not already-accepted numerical coefficients. Public ancestry is not mandatory for every prepared-body experiment, but its classification alone does not prove physical domination. Complete production coverage and inclusive binding remain later obligations; staging cannot waive a required cell of the selected experiment. Before gathering more timings to fit a count-based candidate, determine whether the retained same-source input design can identify its proposed terms; duplicate coordinates/repeats add observations, not independent design dimensions. A rank deficiency may reject a calibration design, not every possible conservative envelope or the production protocol.

Closed native Invoice/typed-failure evidence MAY be reused for semantic branch obligations under an explicit noninterference argument: preserve every pending/provider input and current clock, use disjoint cleanup identities/destination buckets, and verify that selected callbacks neither read nor mutate cleanup process state. Cleanup must not mutate the pending economic/provider state. This is conditional branch/source reuse, not a new joint execution, commutative root/CPU theorem or transfer of exact counts/proof/timings. Shared System.Events, Preimage legacy conversion and actual trie paths retain their own owners.

Apply branch exclusion narrowly. `FinalizedProposalMissing` after a successful receipt is source-excluded only for the canonical initial record and existing non-reentrant reference bindings; the generic-host late-fault rollback regression remains necessary portability evidence. Source/binding changes reopen that exclusion. An invoked Invoice capacity Err occurring before transfer does not witness provisional economic mutation before error. No unchanged Invoice/native/history/PVF corpus or broader matrix may be repeated just to change cleanup domain count; require a changed observed obligation first.

## Required External-Value Mass Control

Apply the [distinct-payload/matching-unit rule](state-witness-accounting.contract.en.md#distinct-payload-mass-and-matching-units) to this selected original V1 read projection. The control MUST establish external full-value requirements, exact hash classes, codec behavior and complete original-input scope before using its payload mass. Domain copies, hash-only mutation, recorder increments and compressed PoV MUST NOT substitute for that coordinate.

The finite input-projection control MUST use actual domain-index keys/typed full values, 40 distinct hashes and one alias-domain, exact SCALE lengths and selected V1 raw/compact proof decoding. SDK trie construction MUST receive full storage-key order, not numeric domain order, and its root MUST match independent reconstruction. Check every read and required-blob absence for both singleton/shared value classes, preserving unaffected reads. Report distinct payload mass separately from domain population and actual raw/compact sizes. A numeric comparison with a ProofSize budget is conditional on that consumer bounding the same complete uncompressed witness unit; it proves neither admission, a production impossibility nor a new domain/population cap. No full Governance records/history, physical timing, sampler or Wasm is required or admitted by this control.

## Matched Full-Index Value-Class Source Gate

One native SDK-host source gate MAY compare q1/q2 required value classes at fixed n=d2, two full indices and identical NoVotes record shape/clock/phase. Change only the last peer ID in the second domain; create its corresponding record and terminal touch through the lifecycle owner, preserving lengths, unique IDs, chronology, old due cohort, visits and strict pending-neutral verification. Do not patch a prepared index without its peers or repurpose the maximum-record single-domain artifact as this source.

Commit the prepared parent without clock normalization, require empty initial recorder/whitelist and zero actual compact-proof baseline, then run the unchanged actual hook once per class case in the existing native BenchmarkingState host. Capture proof and counters before preservation/try-state verification or any explicit root query. Check both required index payload reads by raw/compact reconstruction and required-blob absence, and keep every source verification. No recording timer, new sampler/Wasm, rate, n-only envelope, measured CPU improvement, public ancestry, maximal host shape or whole-delta/PVF price follows. Equal source counts can have different recorder branches; q≤d and bounded payload length permit consideration of a per-domain worst-case envelope, not an already qualified coefficient.

## Value-Class Emitted Source/Schema Canary

The next admitted stage is a finite source/collector canary, NOT coefficient calibration: exact extra `pallet_governance::on_initialize_full_index_value_classes`, q0/shared and q1/distinct, same prepared NoVotes source as the matched native gate. The selector clears the whitelist during setup, before SDK admission/recording; setup, prefill/commit, useful body, stop/root, post-body commit and strict verification stay separately owned. The generated v2 boundary must first pass caller-owned no-clock native recording with deliberately supplied discounts, explicit zero start proof, actual required payload membership, one start/stop and pre-verification capture. Do not silently convert a whitelisted increment into an absolute witness. Heavy post-body validation MUST remain inside the generated optional verification branch; a Result-valued terminal expression MUST NOT invoke that verifier unconditionally. Verify=false must skip its reads and mutations while verify=true retains strict refusal; keep the useful marked hook unchanged.

After that gate, one fresh benchmark-Wasm issuance and exactly two excluded raw canary calls MAY run: q low=high0/1, steps2, repeat1, min-duration0, no included/warmup continuation series. Preserve source/build/host/CLI/artifact identity, metadata extra-only presence and ordinary exclusion, exact commands/all rows/errors and immutable per-file identities. Parse SDK decimal timings as strings/BigInt and proof as u32; timings/root fields remain incidental excluded observations, with no fitting/subtraction/slope/price claim. A stale binary, inherited SKIP_WASM_BUILD (including empty/0), missing selector, unexpected component/schema, failed verifier or missing/zero proof result fails closed.

The physical source question is whether that emitted standard SDK path retains the native required-Get classes in its declared absolute body-proof coordinate. Under the pinned original-root/recorder/compact codec premises, every body proof row requires at least(q+1)*65540 bytes from the distinct original payloads. The source excludes ignored nodes and clears whitelist before SDK's current-whitelist prefill; its actual zero-baseline native boundary is an applicability prerequisite, not proof of every default/PVF host. Any emitted row below its matching-unit floor rejects this source/collector transfer, not the protocol or every empirical family. Passing qualifies only this finite emitted canary; no CPU effect, production rate, maximum geometry, whole-delta/root/PoV bound or global domain cap follows. Stop after both cells; only explicit later method selection can admit a timing series.

## Cleanup Required-Read Lineage and Recorder Scope

Apply the mass rule to actual cleanup only when each selected present/canonical index has an original-backend value at the declared base and no earlier write supplies a replacement overlay value. FRAME49 `mutate_exists` uses `try_mutate_exists` → `unhashed::get` → full `sp_io::storage::get`/Decode. Ext serves overlay first, backend second. The first read per unchanged domain may require its original external blob; repeated same-domain cleanup reads shrink from overlay and MUST NOT be counted as new original payload classes. Missing/malformed indices are outside this premise. Source noninterference of the preceding pending pass preserves discovery values but does not establish arbitrary earlier block-prefix/migration behaviour.

The selected SDK recorder has no intrinsic whitelist. BenchmarkingState discounts whitelisted DB trackers; benchmark macro prefill reads whitelist keys before start. Resetting read/write counts does not clear recorder contents; commit/reopen resets recorder and its root. `BenchmarkRecording` reports start/end `proof_size()` difference with saturating subtraction. The backend normally uses encoded compact size at its recorded root, with empty/fallback branches. These boundaries MUST be checked before transferring a full-witness mass floor to the numeric report.

The [baseline/increment rule](state-witness-accounting.contract.en.md#baseline-and-recorder-increment) owns the conditional E/B/P inequality and its representation premises. This cleanup transfer MUST validate actual start/end/root/codec and distinguish source-derived from observed zero baseline; whitelist cardinality or counter reset alone is insufficient. Prefer the retained source/API guards over replaying closed hook/root controls. No arithmetic transfer qualifies RefTime, post-stop root/commit, compressed PoV, admitted parents or inclusive prices.

## Governance Retained-Index Logical-Work Control

This source control asks how much repeated logical value traffic the existing full cleanup hook performs on one retained discovery index, not how much physical CPU, database I/O or trie proof it costs. It MUST reuse the maximum-record/full-index fixture, unchanged hook and existing SDK state-event observer. No production observer or alternate cleanup algorithm is admitted.

The finite control is one plain and one observed native call from identical prepared parents: n1024 due entries, one16384-ID index, empty pending/maturity input, cursor E−1 and initial FinalizedOutcome phase. Setup, input inspection, independent full-state-root checks and postconditions MUST stay outside event capture. The selected key's complete Get/Put sequence, returned/submitted encoded byte lengths and missing-value flags MUST agree with the shrinking canonical index; the caller's returned Weight and full post-state root MUST match plain execution. The existing observer's snapshot bound MUST NOT be raised or silently bypassed as a full-state comparison.

For a canonical unique index of initial length m with n distinct present due identities, source `retain` visits sum(m−j), j=0..n−1, positions. Exact SCALE lengths MUST include compact-vector-prefix changes. Logical repeated value traffic, predicate visits, original backend value, net mutation and unique SDK database counters are different operands. The output MUST label these as source/logical facts, never measured CPU, physical transfer, a production price, admitted history or quarter credit. Stop on input/schema/sequence/state/Weight mismatch; retain no timing sample and do not replay the closed whole-hook grid. A changed caller/input/collector question is required for another control.

## Cleanup Index Logical-Work Bound

A nonempty canonical cleanup-entry state has d queried domain indices of lengths m_i, n_i distinct present due identities per domain, n=sum(n_i), S=sum(m_i), and per-domain capacity M. Require 1≤d≤n≤S≤dM and 1≤n_i≤m_i≤M, with no intervening index growth during cleanup. Logical `retain` positions are V=sum(n_i*m_i−n_i*(n_i−1)/2). Missing/malformed index state is outside this lemma, not silently repaired or made admissible.

For q=floor(n/d), r=n mod d, t=d*q*(q−1)/2+r*q and k=n−d+1, a uniform upper is B=min(n*M−t, k*(k+1)/2+d−1+k*(S−n)). The first arm minimizes the convex shrinking deduction across domains; the second bounds due-only triangular work and weights every non-due peer by the largest possible due cohort k. B is not generally tight and MUST NOT be treated as attained history, CPU instructions or RefTime.

For reference fixed-width four-byte IDs, let c(x) be SCALE Compact<u32> prefix size. Returned logical value bytes are at most4B+d*c(M)+(n−d)*c(M−1); submitted bytes are at most4(B−n)+n*c(M−1). Empty final indices delete the key and submit zero value bytes, so the output expression remains conservative. These are repeated logical values, not original proof-entry blobs or memory-copy/I/O measurements. Intermediate encoded lengths MUST retain compact-prefix discontinuities.

S may be an actual cleanup-entry population or a qualified upper at that entry. Reusing the same-backend debt bound requires its existing chronology/bijection premises and a preserved non-growing index lineage to the actual entry; an overlay cursor MUST NOT rebind old proof inputs. No uniform small debt, global fixture-population cap, code/retention change or production coefficient follows from this bound. Native validation MUST exhaust small canonical partitions against actual Vec retain/SCALE lengths, check invalid inputs and full-width arithmetic, and consume closed large-profile facts without replaying their hooks or timers.

S counts index members separately in each domain even when encoded index values coincide. Apply the [witness-sharing contract](state-witness-accounting.contract.en.md#content-addressed-witness-sharing) without changing that logical operand. A selected SDK/V1 falsifier MUST use actual domain-index keys and bounded encoded values, compare equal-value and equal-length/different-value cases, verify raw/compact proof decoding and required-blob absence, and keep per-domain source work independent of payload sharing. This input-projection control is not a canonical full Governance state, admitted history, complete PoV or physical price; it must not turn byte sharing into CPU credit or mutate runtime capacity.

## 7. Claim Boundary

Passing these profiles supports only the exact populations, Tasks, runtime constants, state geometry, production Weight, and source tree measured. It does not establish:

- 10,000 maximum-cost 12-Step Cycles in ten minutes.
- One million Active Actors.
- Exact execution-block SLAs for individual Actors.
- Market success, fair price, MEV protection, or permanent liquidity.
- Equivalent throughput after runtime bounds, Weight, storage geometry, Tasks, or block policy change.

Changed assumptions require rerunning the affected project profile and invalidating or superseding architecture evidence whose baseline no longer applies.
