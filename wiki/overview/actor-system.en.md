---
type: overview
title: Actors System
description: DEOS Actors is the bounded runtime actor system — the pallet, scheduler, lifecycle rules, fee model, and deterministic execution environment that host individual actors while keeping domain logic in adapters and pallets.
locale: en
canonical_page_id: actor-system
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/actors/docs/specification.en.md
  - resource: ../../template/pallets/actors/docs/architecture.en.md
  - resource: ../../docs/actors.integration.en.md
  - resource: ../../docs/oracle.integration.en.md
  - resource: ../../docs/actors-control-plane.contract.en.md
  - resource: ../../template/pallets/actors/docs/embedding.md
  - resource: ../../docs/core.architecture.en.md
status: stable
audience: newcomer
tags:
  - overview
  - actor
  - runtime
  - automation
related:
  - AA-Actor
  - Typed Observations
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance
  - Core Terms
---

# Actors System

## Summary

`DEOS Actors` names the whole runtime actor system: `pallet-deos-actors`, scheduler, lifecycle rules, fee model, actor accounts, and typed execution environment for bounded protocol flows.

An [AA-Actor](actor.en.md) is one concrete instance inside that system. This page explains the system-level contract.

## System Contract

Actors gives the runtime one reusable way to run bounded Actor Contracts instead of hardcoding every recurring flow into a dedicated pallet.

The normative system contract requires:

- Deterministic scheduling on one persistent Service ring: each block is one immutable round, every Actor receives at most one turn and commits at most one Step per block, and work admitted or reentered in block `N` is eligible no earlier than `N + 1`. A live head that does not fit the remaining Weight is never bypassed; later residents keep their order. Suspended retries and timed reviews wait in exact Block or Tick deadlines, due deadlines and the bounded parked-balance dependency scan run in the mandatory prepass, and capacity refusal preserves the exact prior residence;
- Balance/event-driven triggering through direct producer-owned adapters without an event scanner or deferred compatibility queue;
- Two-dimensional RefTime and ProofSize admission before each detection, residence, deadline, close, or cycle operation, including a generated fixed hook base before any `on_idle` storage access;
- Typed tasks such as transfer, swap, liquidity, burn, mint, stake, and unstake;
- Lifecycle rules for identity-only dormancy, atomic activation/deactivation, pause, failure, auto-close, manual close, and mandatory internal terminal transitions;
- Adapter boundaries with runtime-derived worst-case weights so Actors orchestrates mechanics without owning DEX, staking, or asset logic.

Actor balances can function like trigger messages: an asset arriving on an actor account can wake the Actor Contract, and that pending signal must retain a bounded path to eventual eligibility. Manual and matched address events coalesce through the single `pending_signal` latch; Opening consumes it atomically, while deferral, pause, and scheduler movement preserve it. Pause moves the process out of service and keeps any already-paid latch; while the global breaker is active, the live Service head keeps its place. Observation publication never wakes an Actor: an Actor reads a typed observation only through a fresh-only Step predicate at its own check or Attempt, so a price-reactive strategy pairs a `Cadenced` or `AddressEvent` Trigger with that predicate. Ordinary `Fixed`/`Percent` inputs resolve afresh at each Attempt; the distribution forms below preserve one bounded Cycle snapshot.

Balance-dependent parking retains tracking only for current registrations. Empty pages and the last unused source are reclaimed atomically with bounded work; re-registration receives a fresh source ID. This tracking cleanup preserves live scan obligations and does not move sovereign custody. Physical qualification of the changed cleanup and scan costs remains open.

Individual `pause_actor` and `resume_actor` calls are available only for Mutable System Actors through their signed owner or `SystemOrigin`. User Actors cannot use these calls; rejection preserves their state and custody before no-op or expiry handling. The global breaker now refuses explicit lifecycle controls, Manual occurrences, replacement, Active/Dormant close, single/batch sweep and active-limit mutation before no-op or expiry effects. Both privileged scalar setters remain available. Frozen Prepass preserves Deadline/dependency/Service authority and opens resource accounting. Freeze records the existing block-local halt; same-block thaw cannot resume Drain.

Common certified preflight/consequence temporarily refuse positive Active ingress; balance-dependency publication also blocks revision/scan mutation. Zero/self and absent/Dormant destinations remain balance-only. Native producer fixtures additionally verify nine signed/dynamic call variants under block-atomic storage rollback, both XCM origin/deposit paths with returned holding, Task mint, Router fee and TMC distribution. Each succeeds after thaw. Complete authored/imported blocks, maximum geometry and physical qualification remain open.

New User identities additionally require the chain flag `UserCreationEnabled`; missing authority denies creation. Reference genesis explicitly disables it without freezing System topology. Automatic/exact-slot and Active/Dormant creation are all denied before Actor payment or slot allocation, while existing User service/control and System creation remain independent. The privileged `set_user_creation_enabled` call changes this flag even while the breaker is set, emits a typed event and preserves existing Actors. The reference runtime shares one exact L2 allowlist for witness validation and direct execution of Router fee and both Actor flags; signed protocol submission requires primary-track power. Native signed User-admission enactment now passes on both named presets over ordinary voting and the unchanged delay.

Native signed freeze/thaw also passes both presets without Root recovery, including thaw submission while the breaker is set; disabled User admission and ledger-only fee collection remain intact. Full machine-freeze semantics and physical/artifact qualification remain open.

Four linked complete blocks per preset now pass native author/import replay around actual Governance thaw: frozen Fee Sink state and its Deadline survive, the approved thaw commits, and cadence service resumes. User admission remains disabled and System identity is preserved. The prefix remains native preparation with a constructed parent; exact Wasm, maximum producer geometry and physical qualification remain open.

On the same frozen pre-state, a candidate with real inherents, a valid signed Native transfer and then certified ingress into Fee Sink is rejected during authoring and Full Executive import. The declared host transaction restores the complete root, including the valid prefix's balance, nonce and fees. This covers signed Native ingress; other producer blocks and node/network import remain open.

The Automation Observe view reads the bounded feed registry and one selected feed at a finalized block. It shows exact directional identity, scale-formatted scalar, producer/provenance, aggregation, lifecycle, update block, revision, authored age, and Fresh/Stale/Unavailable/Uninitialized status. It also states latest-state coalescing and warns that local pre-execution pool reserves provide neither external fair price nor MEV/ordering protection; history remains materialized.

Funding uses ordinary inbound transfers rather than a dedicated value-transfer call. The Contract funding policy or the default-deny `FundingAuthority` decides whether a certified credit is accepted; rejected or source-less deposits remain balance-only custody, while post-expiry ingress closes the actor inline. Each supported producer preflights before value movement and submits one direct fallible notification in the same transaction, so a failure rolls back rather than silently losing state. Actors keep no funding accumulator. Ordinary Fixed/Percent inputs read the current available balance; distribution uses the single Run snapshot described below.

Readiness can latch only while Idle. During Running or Suspended, incoming assets update ordinary custody without a Trigger fee, deferred Cycle, or change to the current cursor and retry authority; a later Attempt reads the current available balance.

The reference runtime limits ParkedBalance to one watched asset per Actor, independently of Contract length and allowlists; custody may contain other assets. ParkedBalance activation or replacement captures current owned balances and parks atomically without executing a Cycle; earlier deposits are not replayed. Cadenced is excluded, while an unconsumed AtTime may coexist with Park. Already paid readiness remains owed. Park now requires its live parked-balance episode and has one negative reason, `ParkedBalanceBelowThreshold`; observation-era reasons and episode-free readiness bypasses are removed. Dependency storage follows current registrations: empty pages and the last unused source are reclaimed with bounded work, and re-registration receives a fresh source ID. Cleanup preserves active scan obligations and sovereign custody. A positive balance check now atomically pays for useful readiness, releases Park and publishes one Pending readiness for the next block; User and System native tests cover this transition and payment refusal. Complete physical qualification and metadata/quote consumer refresh remain open.

Distribution uses individual Transfer Steps. The first captures one asset/volume snapshot after any producing Steps; later Steps reuse it, so new ingress does not change shares. Each Step commits independently; a later failure preserves earlier transfers. Skipping capture also skips reuse. Completion, abort, reset and Close remove the snapshot with Run. Prior four-watch/fanout measurements do not certify this configuration.

## Fees and State Removal

Paid Idle readiness with known window/cooldown eligibility beyond B+1 stays in one primary Block Deadline until due return to next-round Service. Independent window expiry can take precedence. The paid latch remains, redundant signals add no payment, and the known delay creates no repeated Actor-specific Service inspection. Public User/System native cases and a selected zero-Step User window diagnostic cover this publisher; complete physical prices and other refusal backing remain open.

Approved behavior policies remain Gate B work. The authored `on_unfunded` field is present in Contract/header/hash; only Close is admitted. System AwaitFunding is forbidden, and User AwaitFunding is explicitly unavailable until financial waiting is implemented. The field adds one header byte; metadata, holds and physical costs need final qualification. Legacy completion is removed from Rust; `close_when` is authored but present admission is unavailable. Prepared native Service transactions now commit policy candidates and Close atomically; public policy admission, financial wait and generated client/ABI remain open. Current-Step precondition/Action and clean facts distinguish plan end from early `StopCycle`. Failed results and economic skips disqualify; false control predicates and financial suspension without progress are neutral. A pure candidate requires plan end and economic success. Prepared native tests cover persisted Cycle flags, clean completion, economic-skip rejection and rollback after late control or User Action-collector refusal; full public conformance remains open.

Gate B now selects optional Actor-keyed authored-policy and mutable monitor/current-Cycle companions behind a fixed head presence bit. A native layout probe excludes policy width from shared head metadata and monitor width from no-policy Run capacity; its absent-policy selector reads only the head. The cold authored-policy companion is installed in Contract geometry and authenticated on full reconstruction; a lifecycle-owned episode and required-only mutable monitor now preserve values on re-certification and reset them on semantic replacement. Actual hold/key tests exclude optional-width costs from common head/Run geometry. The internal current-Step commit path is installed and exercised with prepared native policy sources. Policy-bearing effectful Step control has separate fail-closed maximum/actual host ports; ordinary Step pricing grants no policy admission. Zero-Step policies now have equivalent control ports shared by Pipeline pricing and Service settlement, verified by native User fee/Close rollback witnesses. Optional policy/monitor Close now has an independent fail-closed host price guarding prepared execution and the loaded finalizer. Prepared complete Close checks stored admission, body and current host authority. Prepared canonical Service now authenticates policy headers and requires Step/zero-Step/Close maxima, with accumulated and clean-completion witnesses. A separate inspection maximum is booked before companion reads and retained on refusal; Pipeline pricing includes bounded inspections. Non-resident terminal simulation runs the same destructor under rollback and exposes hold-release failure. Public policy admission, lifecycle/call coverage, economic backing, production limits and complete Weight/ProofSize qualification remain open.

AwaitFunding is approved as passive waiting for the original unpaid service: funding restores that work, while continued insolvency permits apoptosis after 28 days. One waiting episode retains its deadline through dust, notifications and tentative restoration. Waiting creates no periodic fee or negative polling, and the target requires no separate owner-payment ticket for each entry. Prepared native Trigger/Pipeline pricing includes fixed Opening/Action-wait reserves collected in existing fees at useful readiness or Opening once. A host port now models fixed unpaid-AtTime backing at installation, with passive entry and payment rollback. Complete fee/producer and physical qualification remain open; financial waiting remains unavailable.

An independent committed-history oracle now checks all eight outcome counters and both Cycle counters through a prepared native trace of actual failure, fee waiting, successful retry and a later false predicate. Waiting preserves the prior failure and creates no new observation. Success after failure leaves that Cycle unclean; a skipped economic Step also prevents clean completion. The first later clean Cycle closes on the authored policy. This is native preparation; public reference-adapter conformance remains open.

Native preparation covers entry and recovery for AtTime, Opening and Running/Suspended Action, plus custody-neutral expiry. Funded return uses B+1 Service; the original episode, debt, expiry and paid prefix remain until the owed boundary commits or retires. AtTime recovery pays only its Trigger; a first-Action wait reuses the already-paid Pipeline. Cadenced recovery restores the next future point. Due inspection preserves ordinary WindowExpired before solvency. Joint native policy/wait traces preserve success counters and clean-Cycle state across pristine and later waits, then close on the authored condition. Late refusal restores the original wait and monitor; cadence remains unarmed. Prepared owner controls preserve exact no-ops and custody, roll back late refusal and detach replaced waits. Single/batch Sweep preserve AtTime waiting and WindowExpired precedence for windowed sources. Native repeated hints and reparking retain the original wait. Prepass/Drain/finalization preserve overdue Deadline or tentative Service waiting through freeze and forbid same-block resumption after thaw. Joint native traces retain monitor counters, clean flags and the paid prefix until funded recovery or custody-neutral grace expiry.

Native post-movement models authenticate source policy and current fee-native spendability. An admitted hint can return owed work without a Trigger fee or new Cycle; insufficient and repeated hints never extend expiry or reshuffle Service. Reference XCM deposits now see arriving Trigger funds and return incoming holding after late refusal. These native Close-policy witnesses leave concrete AwaitFunding producers, production and joint-monitor qualification incomplete.

User creation pays for state installation through ordinary transaction payment and backs eventual complete removal through the Creation Fee. Each Pipeline pays for its own machine execution and Cycle completion, without repeating the final Actor-destruction charge. Trigger occurrences and invoked Action effects remain separate fees; refundable state holds are not cleanup payments.

Refundable state backing reserves bounded autonomous capacity at installation: Run, an AwaitFunding episode and a temporal pointer while Cadenced or unconsumed AtTime can need it. Removing detector membership for a latch or financial wait retains that capacity, so legal spending of the owner's unheld funds does not block rearm. Consumed AtTime releases its pointer capacity; deactivation and Close release Active backing. This does not qualify public AwaitFunding or pay for its inspection work.

Prepaid cleanup still consumes current-block Weight and never transfers sovereign custody. Close currently removes process state atomically; prepayment does not imply deferred or incremental deletion. The configured Creation Fee alone is not evidence of a measured worst-case cleanup tariff, and fresh cost quotes are not production resource certification.

## Block-Paced Execution and Resource Zipper

Production execution separates two component-wise `Weight` domains. **Actor Control** has a hard ceiling of one third of schedulable block capacity and pays for detection, materialization, current-Step evaluation, Service ring and deadline movement, lifecycle bookkeeping, and bounded cleanup. **Shared Economic Execution** owns the remaining two thirds and pays both ordinary external calls and Actor Task effects through the same host mechanisms and Weight owners.

A Service turn charges discovery, the loaded-state check, and one transition owner. If admission closes the actor, one complete close owner replaces that turn's provisional prefix rather than being added to it. Capacity refusal can inspect the same head in both Prepass and Drain without another fee or Step. Native boundary tests now recover in order through public Prepass, including mandatory work, and preserve Q1 through Drain. Under complete sound declarations, a conforming fresh allocation and stable eligible/funded authority without freeze or collector/invariant failure, at most two capacity refusals precede next-block admission. This conditional count does not qualify physical prices or their payer; other refusals and shared selector work remain separate. The [fee composition owner](../../docs/actors.integration.en.md#fee-composition) retains evidence and limits.

Under continuous demand, Shared Economic grants equal base turns to Actor effects and signed user dispatch. Either side may use capacity the other leaves idle, but it cannot consume the other's guaranteed turn while both are saturated. `RefTime` and `ProofSize` are checked independently: spare capacity in one component cannot compensate for exhaustion in the other. Borrowing changes available capacity only; it never changes Service order, Actor class neutrality, or next-block causality. This work-conserving composition is the **Economic Zipper**.

Contract admission now checks independent fresh Control and Actor-effect turns after mandatory work, including inspection, placement and reachable cleanup. Fitting the combined idle reserve is insufficient. Dynamic Prepass cannot reduce configured minimum turns; the reference admission certificate commits that budget. Native boundary and composition checks qualify the declared guard; complete physical bounds, refusal backing and production binding remain open. The [integration owner](../../docs/actors.integration.en.md#current-distribution-and-evidence-domain) retains the source and evidence scope.

The prepared Pipeline price now includes common Actor-specific Service discovery, state inspection and the outer ring transition for each admitted attempt, including zero-Step completion. Those costs were separately accounted in Service but omitted from the price. A native independent-runtime test with a linear fee confirms payment against declared Control. A selected Wasm User zero-Step profile covers the complete per-Actor Service encounter with local resource settlement; it establishes no general maximum. Scheduling traces now require passive Idle after completion; closed-round guards retain real Running sources. This adds no destruction surcharge or new fee timing; physical prices, complete reachable Cycle composition and backing of unpaid/refused inspections remain open. The [fee composition owner](../../docs/actors.integration.en.md#pipeline-fee-and-lifetime-cleanup-partition) retains the input and evidence limits.

In prepared AwaitFunding pricing, useful readiness owns the first additional financial check at Opening. Pipeline excludes that check once while backing later Step/retry checks and separate Action-wait recovery. Native collection witnesses confirm the fee separation; public financial waiting remains unavailable.

Measured raw trie proof, compact benchmark proof and generated Weight estimates describe different quantities. FRAME's proof correction updates its block accounting; it does not independently certify the Actor Control maximum. Complete Close bounds remain open. The [runtime integration](../../docs/actors.integration.en.md#benchmark-host) owns these metric definitions and their qualification limits.

Each block follows one consensus sequence: required context inherents, Mandatory Actor Prepass, signed external dispatch, Actor Drain, and finalization. Prepass is the sole pre-external Actor owner. It opens the block's Service round, in which every Actor receives at most one turn and commits at most one Step. A Trigger occurrence observed in block `N` can become executable no earlier than `N + 1`; congestion may delay it further. `on_initialize` performs no Actor work, and neither the base pass nor Drain may bypass the live Service head because a later Step is cheaper.

The system scales with the **active frontier**, not by scanning all identities. Dormant identities carry no active scheduling topology. Active Contracts keep compact hot scheduling and run authority separate from cold authored Contract bodies and bounded Step chunks. The current Step is loaded only when needed; the runtime does not duplicate a whole Contract into the Service ring, run state, or an off-chain cache. A User Actor reserves an exact bounded state hold for its active-lifetime run authority, so autonomous Opening does not depend on the owner's later free balance; that hold persists through progress and suspension and releases on deactivation or close.

In the reference runtime, Actor-state holds can coexist with a signed Preimage deposit. Closing the last User Actor releases only its Actor hold and preserves the unrelated deposit and Preimage data. The runtime integration document owns the current producer bindings and their qualification limits.

These rules are capacity and causality contracts, not latency guarantees. Strict Service order, one Step per Actor per block, bounded deadline pages, and available two-dimensional Weight determine observed completion time.

Service progress also requires successful fee collection when payment capacity exists. Trigger rearm reads no observation, so an unavailable or deactivated feed only makes the dependent predicate false at its Attempt; it never blocks the live head.

## Verifiable Straight-Line Composition

Each Actor Contract is represented publicly by bounded `ContractSteps`; `MaxContractSteps` limits the ordered `steps` field for both User and System actors. An absent Step `precondition` is unconditional; a present `Precondition` is one bounded DNF whose outer clauses compose with OR and timed predicates inside each clause compose with AND. Every admitted predicate is visited, any predicate error fails the whole expression, and a false precondition only advances to the next fixed step. Every predicate reads current state immediately before its step, observes successful earlier effects, and is read again on retry; no result is frozen at cycle start. No precondition introduces jumps, loops, callbacks, arbitrary calls, or authored successors.

Pipeline pricing includes one `StopCycle` visit. Without a precondition, it completes the Cycle and excludes following Steps from execution pricing. A conditional return retains suffix pricing: a false predicate skips it, and a Permanent predicate error with `ContinueNextStep` advances. It has no failing Task or unpaid Action liability, so authored retries add no visits. The complete body, loading geometry, holds and Close costs remain retained.

The native Actor Contract now authors optional `close_when`; the legacy completion enum and productive-Cycle close reason are removed from Rust. Absent policy adds no outcome terminal. Present policy is validated but explicitly unavailable until complete transition/resource authority exists; it never silently becomes absent policy. Header and current-Step views carry only a fixed presence bit. Positive fixtures choose their thresholds explicitly; the removed rule has no compatibility alias or automatic replacement.

Canonical SCALE `ContractInput` remains the source of truth across metadata-bound authoring, structural diff, static analysis, simulation, and governance composition. Visual blocks or neural proposals may project or propose this finite AST, but deterministic validation, human approval, encoding, and runtime execution stay authoritative.

Production and rollback-only runtime simulation share one Step evaluator. Its `StepOutcome` retains a concrete failure cause separately from retry disposition, and one `AttemptDisposition` owns completion, failure, suspension, or close; simulation adds only bounded trace records, requested-mode checks, transaction-depth protection, and rollback.

Runtime simulation authenticates the original unpaid boundary of an AwaitFunding process. Without an independent terminal, it returns `NotReady` while that boundary remains waiting, including a funded hint's tentative Service return; actual Service owns recovery. An expired lifetime Window retains precedence: simulation checks the Close budget and runs the common destructor under rollback, preserving the prior Run result without a new Step. Prepared native witnesses compare Opening, policy Close and late payment refusal with real execution and preserve the full storage root. Public policy and waiting admission remain gated.

The typed public inventory covers Predicates, Tasks, amounts, Triggers, funding and outcome policies, Events, Errors, views and adapter results. Named package, DEOS runtime and independent-runtime tests own their actual execution coverage. Present `close_when` and User `AwaitFunding` remain unavailable in public admission until their resource and lifecycle gates close; prepared reducer tests do not establish public conformance, and production metadata/client binding remains open. Constructor-free runtime-upgrade cancellation and context-free amount-dependency placeholders are not encoded as future options.

`StopCycle` normally completes the Cycle at its current Step. Its work belongs to Control, prepaid by the User Pipeline Machine fee; System remains fee-exempt. It reserves zero economic effect capacity and produces no Action fee or Action observation. It contributes one normal completion; a control-only Cycle or an unvisited suffix is not clean. Action, normal-Cycle and clean-Cycle conditions in `close_when` are separate authored choices.

The control-plane corpus models descending buy and ascending sell buckets as independent bounded one-shot actors over directional local-pool observations. It does not mislabel those price feeds as treasury reserve ratios or absolute liquidity depth: only the manual execution cores exist for those reactions until typed producers and meanings ship. A block-height release demonstrates an available non-price scalar strategy using runtime-owned current block truth.

## Progress-Preserving Continuation

A Mutable actor may mark a step `RetryLater { max_attempts }` with a nonzero `u32` limit. Temporary adapter failure or unavailable funding increments the unsuccessful-attempt count at the unresolved cursor; the initial failure counts as `1`, and advancing to a later cursor resets the local count. Actors keeps one sparse run with that count, the last-attempt block, and cumulative outcomes; inputs are read again at every retry.

Retries reuse the same logical-cycle nonce: a short backoff stays in the Service ring and a longer one waits in an exact Block deadline. They start at the unresolved step instead of replaying the committed prefix. Reaching the local limit closes the actor with `RetryAttemptsExhausted`; a simultaneous actor-wide failure cutoff does not replace that more precise reason.

Permanent and unsupported-adapter failures never create Continuation. Immutable actors cannot use bounded `RetryLater`. Cancellation deletes current progress without compensation, prefix rollback, funding promotion, or balance movement; pause and the global breaker preserve it. Cancellation removes any retry deadline together with the run, so no stale deadline can claim the Actor again.

`CycleStarted` appears once. `CycleContinued` and `CycleSuspended` identify the cycle and unresolved cursor, while one cumulative `CycleSummary` terminates the logical cycle. Current Continuation is canonical chain state. Long attempt timelines require a materialized event index.

The latest funding-unavailable Step result distinguishes missing fee-native Action payment (`ServiceFee`) from missing Task assets or staking shares (`TaskInput`). Both occur before Action invocation. System Actors are exempt from Actors service fees and cannot produce the first reason; the continuation tag does not duplicate this provenance.

## Operational Observability

Actors keeps current starvation observability sparse. `IdleStarvationState` is absent/Healthy during normal operation. With the breaker inactive it becomes `Starving { consecutive_blocks }` when live Service work remains, no attempt commits, and the actor pass stops on Weight, fee collection, or an invariant; it becomes `Alerted { consecutive_blocks }` at the configured threshold. No live work, a committed attempt, pass exhaustion, or breaker activation clears or freezes state as specified, so an empty ring with exhausted budget does not count as starvation.

`IdleStarvationDetected` and `IdleStarvationRecovered` each emit once per alerted interval. The current phase is canonical chain state; long-term alert history and duration trends belong in an indexed view built from those events. The production-Wasm healthy-empty probe confirms five reads and zero writes. Distinct deadline keys live in paged binary min-heaps with exact reverse handles. Insert, pop-min, and exact removal use at most `ceil(log2(MaxActiveActors))` sift steps, covered by maximum-depth generated benchmarks.

Every DEOS System swap also applies a local reference-deviation guard. A nonzero EMA remains eligible through age 100 blocks; zero, missing, or older EMA falls back to the direct-pool reserve ratio, and absence of both references fails Temporary before mutation. The observed pool may already have been manipulated, so this guard proves neither external fair price nor transaction-order protection and does not replace task slippage or output bounds.

The read-only `ActorEligibilityApi::actor_eligibility` projection reports current readiness, the scheduler-owned phase, and the next eligible block at one finalized block, reusing the same pure cadence/cooldown/window/backoff/breaker/latch owners as admission. Generated metadata binds the complete terminal-reason set; unknown eligibility, reason, phase, or typed-failure variants never enter display state. Clients never reimplement that arithmetic; the projection never promises service, because Service position and available Weight decide actual admission.

Off-chain feedback analysis separates observation-caused recurrence from shared account, pool, reserve, or TMC resources. Only evidence-addressable reactive edges can form reactive-cycle findings; resource coupling remains a separate unscored advisory signal with unknown causal and economic significance. This analysis cannot reject plans, alter scheduling, or claim execution.

Prepared browser monitoring reads finalized semantic/process owners and checks their generation. It shows current Service, Deadline, Parked or Disabled residence instead of legacy FIFO tickets; absent canonical metadata means unavailable, not an absent Actor. Prepared eligibility v8 separately reports the original unpaid service and debt, current protected fee-native spendability, exact funding shortfall, entry time and original grace Tick. Large amounts retain exact precision; no shortfall does not promise execution admission, successful fee collection or future solvency. Prepared resource v2 separately reports creation admission, machine freeze and block-local halt. Eligibility also reports the behavior episode start, referenced caps, capped values and current clean-Cycle flags without presenting them as lifetime counts. Native read-only and isolated SCALE fixtures pass; generated bindings, real RPC/rendering and complete financial receipt presentation remain open. [Browser realization](../../web-client/docs/architecture.en.md) owns this projection.

## Embedding Boundary

After completion or abort, an Idle Actor without a new signal waits outside Service. Its identity, custody and holds survive; temporal readiness and window expiry keep their deadlines. New useful readiness returns through the ordinary B+1 path. Native tests cover quiet blocks without Actor-specific inspections, recovery and exact placement. Complete native quiet/Session and signed Governance-cleanup blocks also preserve funds and cadence; prepared clock gaps remain conditional. Complete resource prices and backing for other refusals remain unqualified.

External runtimes can reuse `pallet-deos-actors` without inheriting the DEOS/TMCTOL System actor catalog. The host runtime provides bounded adapters for assets, caller-aware DEX quotes, staking shares, liquidity donation, funding authority, atomic fee collection, fallible ingress, and two-dimensional task weights. Actors owns scheduling, lifecycle, policy-aware amount resolution, fee reservation, and task orchestration. User fees follow four independent boundaries (Creation, useful Trigger occurrence, Pipeline Opening, and invoked Action), and each charge is one atomic `FeeCollector` transfer. The collector transfers the full charge into Fee Sink; downstream allocation remains outside Actors. The DEOS reference Fee Sink currently applies its 50/50 staking/liquidity Actor Contract.

For current Close-only reference contracts, direct Transfer/Burn preserve the source's Action reserve through the inspected ledger and ingress callbacks. A receiver pays its own readiness fee; its installed hold does not grow on latching, including when its owner is the source. Native and nonsufficient Local Service witnesses retain the exact source floor and paid Action under that alias. A future producer payment for funded AwaitFunding return needs a separate joint-reserve/rollback qualification; public waiting, common refusal backing and physical prices remain open. [Fee composition](../../docs/actors.integration.en.md#fee-composition) owns the source argument and evidence limits.

Reference DonateLiquidity transfers all newly minted staking receipts. A positive receipt minimum removes the transient consumer, including at rounded reserve ratios; zero minimum can retain a zero-balance account. The User adapter checks Action-payment capacity inside its effect transaction: insufficient capacity rolls back the effect as one paid permanent failure, while sufficient reserve permits execution. System Actors retain their fee exemption. Native witnesses cover these boundaries; complete physical costs and AwaitFunding backing remain unqualified.

Under coherent current Actor, ledger and hold authority, all nine economic User Tasks preserve the current Action reserve: Transfer/Burn limit the input, and the other seven check spendability before their effect commits. A failed invocation restores its ledger effects and still owes its Action fee. Successful placement adds no debit of that reserve; Close only releases holds. Mint is System-only and StopCycle has no Action fee. This conditional source argument requires the collector's own recipient premises; it does not establish finite backing for repeated refusals or future AwaitFunding recovery.

The configured transaction-extension chain cannot create Root: its twelve elements retain the input origin or substitute `Authorized`, and the tuple/wrapper forwards that result. This excludes Root as the ordinary outer-extrinsic origin; internal privileged calls and direct host origins retain their separate authority. Fee Sink is Mutable and controlled by Root or its pallet-account owner; inability to sign for that account is a cryptographic premise.

The current reference Governance payload executor supplies no Native Fee Sink debit: L1 only authorizes an upgrade, L2 parameters use three scalar surfaces, and invoices use the Assets ledger from fixed BLDR Treasury. Native dispatch fixtures preserve Fee Sink's exact account during both selected invoice payouts and reject an encoded Native debit with the root unchanged. This scope excludes replacement runtime code, other callers and Governance hooks; unrestricted Root remains able to violate the collector's recipient premise.

Staking's configured security-reward funding source is Fee Sink, but production `TrustedSet` rejects this funding before Native transfer, even for Root. Native preset tests preserve empty reward obligations and all accounts through Session callbacks; benchmark LP-mode overrides and successor runtime code fall outside that scope.

Preimage's lazy legacy conversion could hold Native from a stored payer, but its old table is empty at fresh genesis and current interfaces do not populate it; the explicit SDK migration is not configured. Current tickets charge the signed note author, retain that owner and refund their hold on removal. Native tests preserve Fee Sink's exact account on both presets. This excludes the selected Preimage debit under unchanged bindings and a distinct signer, not fabricated origins, arbitrary storage writes or all Native callers.

Public Assets deposits charge the signed payer. Ownership transfer moves existing reserves, so assigning an asset to Fee Sink can credit its Native reserve without reducing its free anchor. The old owner cannot subsequently charge it for metadata. Four complete native signed blocks across both presets preserve that anchor and issuance while collecting actual fees. Arbitrary host authority and complete ledger preservation retain separate obligations.

In the current supplied-payer composition, pool deposits bind the creator/touch signer. No installed consumer calls the metadata or approval mutator; genesis supplies no pools. Reserved Actor identities prevent another Actor using Fee Sink as its source, and its two-Transfer plan remains owner-controlled. Four native signed pool blocks verify a creator refund, another signer's new deposit and rejected plan replacement. Root, arbitrary host calls and successor code stay outside this exclusion.

Partial XCM withdrawal now keeps Native credit destruction inside the storage transaction. Native Parent messages with an unavailable second asset preserve the source balance and total issuance on both presets. These native scenarios qualify that failure boundary; complete Native conservation and production execution remain separate.

After paid Manual readiness, Root removal of Fee Sink's ED also makes automatic Pipeline Opening refuse across fresh native Prepasses. The Actor remains solvent and retains its latch, custody and process; incurred Control stays charged. The failure halts further Actor work in that block, so Drain performs only housekeeping. Restoring one ED permits one original Pipeline payment and completion. This prepared public-call witness supplies no Signed exploit, public AwaitFunding or finite backing for repeated refusals.

The independent `template/pallets/actors/embedding-runtime` external-consumer fixture makes this boundary executable. It starts with zero System Actors, uses local account/asset types and smaller scheduler pages, and proves direct Executive ingress, fresh-genesis integrity, deterministic unsupported adapters, User/System Continuation, User exact-output swaps, System-only minting, try-state, and no-std operation. It is portability evidence, not a second product or prescribed topology.

The unlaunched reference chain keeps fresh-baseline Actors storage version `15` and ships no historical migration. This baseline includes canonical Trigger runtime state, refundable bond state, block-resource phase state, and active-lifetime RunState hold authority. The independent embedding gate executes absent and bounded present Precondition, `StopCycle`, and Continuation behavior without a DEOS/TMCTOL helper or actor-topology dependency.

The DEOS reference runtime also owns `LpPairByTokenId` outside generic Actors, so liquidity removal resolves one exact LP-to-pair entry instead of scanning pools. Internal adapters and the transaction extension maintain that index when pools are created or first funded. Both authored minimum outputs reach Asset Conversion directly; an outer transactional balance-delta check remains as defense in depth.

Each Task has its own transaction. If an adapter fails after partial mutation, the failed Task rolls back its local effects and success event; earlier successful Steps remain committed. Transfer checks source withdrawal and recipient deposit before moving funds; an unavailable recipient deposit produces a Temporary failure. Distribution uses separate Transfer Steps that reuse one captured volume, so a later failure preserves earlier shares. `ContinueNextStep`, `AbortCycle`, or Mutable-only bounded `RetryLater` decides whether execution advances, terminates, or suspends at the same Step.

## Control-Plane Boundary

Off-chain tooling binds an executable plan to genesis hash, runtime versions, metadata hash, actor type, mutability, and exact `ContractInput` SCALE bytes. Human JSON is a lossless projection, not runtime truth. A deterministic `contractId` supports review and correlation while metadata changes require explicit decode, validation, and re-encoding.

Plan diffs, forecasts, simulations, governance composition, and long configuration/cycle history remain local or materialized surfaces. They carry block, metadata, and model provenance and never expand consensus state or authorize signing implicitly.

The browser’s adapter-local simulation now checks explicit `close_when` against all ten outcome metrics. It carries capped episode values and clean-Cycle flags between calls, preserves earlier effects, and distinguishes normal completion from clean completion. These values are local projections, not authoritative chain counters or exact lifetime statistics. Fee insolvency and AwaitFunding are explicitly unsupported; production metadata and resource bindings still need coordinated refresh.

Typed authoring and configuration IR v2 now preserve explicit outcome conditions and the fee-shortfall choice, including User-only AwaitFunding. Encoding checks the supplied metadata vocabulary and clause/atom limits and refuses legacy completion. Analysis preserves the exact condition and labels unqualified policy/wait costs. Live headers expose only condition presence and the fee-shortfall choice. Runtime result decoding distinguishes fee insolvency from Task-input shortage and rejects the retired productive Close reason. Cycle abort is not Actor destruction; Immutable warnings make no permanent-custody promise. Native codec witnesses do not prove runtime admission, actual Wasm execution or production resource binding.

## Portability Boundary

The current staking contract is intentionally generic:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

Actors does not encode a DEOS-specific native-only staking task, collator selection, `stNTVE` naming, or `$NTVE/stNTVE` LP custody. Runtime adapters decide what a generic staking position means, expose its share balance, and optionally map it to a transferable share asset for `Unstake` amount resolution. That share-asset identity remains stable for the admitted position key; a runtime upgrade must introduce a new key rather than reinterpret an active plan. Execution fails closed if the mapping disappears. In DEOS, the adapter routes native staking into generic `pallet-staking::stake` with the configured native asset id, while nomination security remains a separate locked-LP staking/governance surface.

This keeps Actors useful outside one tokenomic configuration.

## Current DEOS Role

On the current reference line, Actors is the execution substrate for runtime-side protocol behavior: burning, liquidity provisioning, treasury splitting, bucket handling, `$BLDR` lane flows, and native staking LP provisioning.

The shipped runtime reserves fifteen deterministic System addresses but enrolls only three active Actor Contracts at genesis: Burn Actor, Fee Sink, and BLDR Splitter. These contracts react to verified inbound value rather than periodic polling. Twelve System identities start dormant with no Actor Contract, funding, fee, residence, deadline, or cycle state. Ten are Mutable activation candidates. First-order Bucket A and BLDR Anchor are sealed Immutable identities: every control path rejects activation or close, while the runtime LP freezer admits incoming LP but exposes zero reducible balance to ordinary, admin-forced, internal transfer, burn, and LP-class destruction paths. Native staking LP provisioning can activate only after the receipt asset, staking pool, dormant identity, and non-empty `$NTVE/stNTVE` AMM are ready.

The DEOS runtime also owns a bounded ranked manifest for certified System-to-System activation edges. Active System Contract installation or replacement is rejected before mutation when it declares an edge outside that acyclic topology. The projection is descriptive host metadata rather than Contract identity or an execution source; Oracle publishers, ordinary users, external markets, and uncertified transfers remain outside its guarantee. User Actors may form cycles without graph analysis, but every attempt remains fee-bearing, bound by Service order, limited to one Step per Actor per block, and subject to minimal apoptosis when Trigger or Pipeline funding stops.

Actors does not replace TMC, DEOS Router, DEOS Staking, or DEOS Governance. Those subsystems own math and domain rules. Actors gives them a deterministic way to be orchestrated together.

## Why It Exists

Without Actors, recurring economic workflows would keep becoming bespoke pallet logic. Actors makes those workflows explicit, bounded, governable, and composable as typed actor graphs.

One actor's balance outflow can become another actor's trigger message. Larger protocol behavior can therefore emerge from small bounded parts while still running inside deterministic scheduling and execution limits.

Within the existing task and adapter language, many workflow/topology changes can move from runtime rewrites into on-chain actor-graph configuration. Runtime upgrades remain necessary for new primitives, adapter surfaces, or safety invariants.

## Related

- [AA-Actor](actor.en.md)
- [Typed Observations](typed-observations.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance](governance.en.md)
- [Core Terms](../glossary/core-terms.en.md)
