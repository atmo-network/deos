# DEOS Actors Local Causal Introspection Specification

> Intended cross-system contract for bounded local explanation and finalized causal evidence. This specification does not claim that its APIs, cause propagation, descriptors, or materializer are already implemented.

## 1. Scope and Authority

Local introspection answers what one Actor Contract can depend on and affect, which identified occurrence opened its current Pipeline, and which effects have committed. It MUST preserve the existing Actor language, admission, fees, custody, Trigger latching, FIFO, Q1, Economic Zipper, and task-scoped transaction boundaries.

The [Actors specification](../template/pallets/actors/docs/specification.en.md) owns execution semantics. The [resource policy](./actors-resource-policy.specification.en.md) owns resource admission and service. The [control-plane contract](./actors-control-plane.contract.en.md) owns Contract artifacts, forecasts, simulation, and existing off-chain feedback analysis. The [read-model contract](./read-model.contract.en.md) owns chain/materialized classification. This document owns only the cross-system introspection boundary and its acceptance criteria.

No global graph, population traversal, automatic multi-hop exploration, Network Physiology, economic value aggregation, execution-block prediction, priority, feedback controller, or new authority is introduced. An application MAY compose separately obtained local results off-chain, but that composition is materialized analysis and cannot inherit canonical-chain completeness.

Implementation follows accepted Actor control geometry. A representation choice for causes or receipts MUST NOT become a second mutable owner of lifecycle, latch, cursor, ticket, funding, or Contract state. Pure query descriptors are optional for an embedding runtime; the reference runtime must cover its admitted host capabilities before claiming complete local introspection.

## 2. Ownership and Dependency Direction

| Owner | Owns | Consumes; excludes |
| --- | --- | --- |
| Actors package | Pure Contract topology extraction; bounded cause association at existing transitions; public local projection types | Its typed language and host ports; no concrete DEOS role or market policy |
| Host domain | Descriptor of its public operation, atomicity, implicit effects, resources, and evidence limits | Its executable semantics; no copied Actor scheduler or alternate execution model |
| Reference runtime | Descriptor composition, producer certification, System roles, identity binding, runtime APIs | Package and host public contracts; no descriptor-driven execution |
| Materializer | Finalized lineage, historical joins, gaps, deterministic artifact format | Canonical state/events and identified schemas; no consensus dependency |
| Client | Local explanation, truth labels, unavailable/stale presentation | Identified projections; no handwritten host semantics or inferred causation |

The extractor consumes canonical typed Contract values. It MUST reuse semantic helpers where those helpers decide dependency timing, task surfaces, admission, or bounds. Host descriptors live with the relevant operation or its reference adapter, and conformance tests bind each descriptor to that operation. Runtime composition supplies identities and typed host mappings; the package never imports the reference runtime.

## 3. Truth Classes and Evidence

Each claim has its own truth class; a response-wide label cannot promote unresolved fields. These classes are orthogonal to the chain/materialized and trusted-provider/verified-execution classifications in the read-model and control-plane contracts.

| Class | Required evidence | Meaning and limit |
| --- | --- | --- |
| `StructuralCapability` | Exact Contract bytes or typed proposed Contract, semantic binding, descriptor identities | A possible dependency or effect; neither readiness nor execution success |
| `CurrentResolved` | One named state hash, relevant code/metadata/binding, exact bounded inputs | A value resolved at that state; not a promise about later execution |
| `ObservedCommitted` | Identified committed transition and block, runtime/schema, exact event or state witness | The evidenced effect committed on that branch; finality is a separate required status |
| `Unknown` | Typed reason and known partial identities | Required evidence is absent, unsupported, stale, pruned, ambiguous, or outside the bounded scope |

Every query envelope identifies genesis, block hash, state root, runtime code hash, metadata hash, and introspection identity. The runtime returns only identities it can establish; the caller binds its result to the selected block/header and independently verifies code and metadata when claiming verified execution. Remote RPC alone remains provider evidence. Provisional results explicitly say unfinalized. Finalized publication requires a finality-verified block and never relies on a caller-supplied `finalized` boolean.

Static proposed-Contract analysis binds exact canonical SCALE and actor type/mutability. It need not invent an on-chain Actor or state evidence. Current resolution for a proposal additionally requires an explicit account/admission context; without that context, account-dependent fields stay structural or unknown. Contract artifact identity remains the existing `contractId`, not a new competing hash.

## 4. Cause Identity

`CauseRef` is a bounded typed source reference, not a free-form string, timestamp heuristic, or list of ancestors. Its canonical envelope consists of a schema version, producer domain identity, one typed source variant, and a bounded occurrence coordinate. The source discriminant and payload are encoded by the identified canonical SCALE schema. Reference hashes use `blake2_256("deos:actor-cause:v1" || SCALE(envelope))`; string concatenation of display values is forbidden.

The runtime occurrence coordinate uses block number, execution phase, and a deterministic producer-local consequence ordinal under an identified parent operation. Ordinals reset per parent, follow actual canonical consequence order, and never wrap. They require no global historical counter. Finalized external identity adds genesis hash and block hash around the runtime reference: the current block hash is unavailable while executing that block and MUST NOT be fabricated inside consensus execution.

| Cause family | Source evidence |
| --- | --- |
| `Manual` | Accepted manual activation call coordinate and runtime-verified caller |
| `AddressEvent` | Certified ingress producer, verified source, destination, asset, and movement coordinate |
| `Observation` | Complete feed identity and the exact revision/transition selected by the existing materialization owner |
| `Temporal` | `AtTime` or `Cadenced`, Actor installation context, and the consumed logical deadline |
| `ActorEffect` | Producing Actor incarnation, Cycle nonce, Step index, attempt coordinate, and committed consequence ordinal |
| `ExternalProtocol` | Certified protocol adapter and its bounded authenticated operation reference |
| `Host` | Identified host operation and its committed consequence coordinate |
| `Gap` | Typed unavailable-origin reason and the bounded known occurrence context |

`ExternalProtocol` and `Host` do not add Trigger families or permit arbitrary callers to certify provenance. They describe the origin carried through an existing admitted ingress path. Actor-produced certified ingress carries its `ActorEffect` origin without duplicating the movement as two independent causes. One effect may cause multiple useful Trigger occurrences; each occurrence references the same source and its own recipient context.

If an upstream producer cannot certify a unique parent operation, the source remains `Gap` with `Unknown` origin. Balance changes, adjacent events, shared assets, equal amounts, or nearby timestamps never establish an ActorEffect parent. Producer labels and application declarations alone cannot upgrade a gap.

## 5. Cause Association and Lifecycle

Cause association is an annotation of a useful `pending_signal: false -> true` transition. It MUST commit atomically with that transition and its existing Trigger fee. Failed fee collection, rejected ingress, or rolled-back movement cannot leave an occurrence or cause behind. Redundant activity while latched neither replaces the cause nor creates an occurrence, evaluation, fee, ordinal allocation, or Actor-specific storage operation.

At most one active-Cycle cause and one pending cause exist for an Actor. While Idle, only the pending cause exists. Opening consumes that pending cause into the admitted Cycle; while Running/Suspended, a later useful latch may retain one pending cause for the next Cycle. Distinct occurrences can reference the same source cause; deduplication MUST NOT merge two legitimate cycles merely because their source references compare equal.

Continuation and Retry retain the active cause and existing Cycle nonce. Failed Task attempts have distinct attempt coordinates but create no committed Task consequences. Completion/cancellation removes active association while preserving or removing pending association exactly as the owning lifecycle transition preserves or removes readiness. Insufficient Pipeline admission records the existing terminal outcome and clears process annotations without reversing the useful Trigger fee or moving custody.

Deactivation, Contract replacement, close, and exact-slot recreation follow existing cancellation/invalidation semantics. No annotation survives as live authority after its owning process is deleted. Historical evidence remains materialized. Actor incarnation identity uses the creation event coordinate plus genesis and Actor ID, so reused locators, sovereign accounts, IDs, or reset nonces cannot alias history. A snapshot without creation evidence may identify a current Actor but MUST leave its historical incarnation linkage unknown.

Observation causes report the revision/transition actually selected at useful materialization, including installation/processing revision context when needed. Coalesced intermediate revisions and latched-period history are explicitly absent; no event queue, old-value archive, or guarantee of delivery per revision is added. Temporal causes identify the consumed deadline, not every missed cadence point. Cause metadata never participates in matching, rearming, cohort grouping, ticket allocation, eligibility, or service order.

## 6. Local Topology Projection

One mutation-free extractor serves stored and proposed Contracts. For equal canonical bytes, actor type/mutability, and binding, their structural projections MUST be identical. The stored wrapper additionally attaches the current Actor/Contract context. It never changes extraction semantics based on physical storage geometry.

The result contains one activation input, Contract-level funding/completion/window constraints, and ordered Step projections. Each Step carries exact index, bounded DNF clause/Predicate positions and timing, amount surfaces, typed host dependencies, error policy, and at most one structural atomic effect bundle. A zero-Step Contract produces an empty Step list and retains its control-only Opening/completion explanation.

Dependency targets are typed: `AccountAsset`, `ObservationFeed`, `Pool`, `Reserve`, `Tmc`, `StakingPosition`, `Actor`, or `Unknown`. Each target includes its owning domain and exact identity when proven. Asset identity alone cannot stand in for an account balance or a pool; unresolved account, receipt namespace, route, or market selection is explicitly unknown. Account derivation uses the runtime sovereign-account owner.

Opening Predicates and frozen amount snapshots stay distinct from Current reads. DNF is preserved as clauses and atoms; the extractor cannot flatten it into an apparent conjunction or skip evaluated atoms based on optimistic short-circuiting. `Fixed`, `PercentageOfCurrent`, `PercentageAtOpening`, `PercentageOfLastFunding`, `AllAvailable`, and input limits retain their canonical semantics. Funding coverage/authorization is a dependency, not proof of available funds.

Forward resource dependency means an earlier Step may write a resource read by a later Step. It is not a scheduler dependency or evidence of activation. False Precondition, resolution skip, funding unavailability, Temporary/Permanent failure, retry, and successful `StopCycle` remain separate outcomes. Suffix resource envelopes reuse canonical cost owners and remain maxima unless separately evidenced path-sensitive results apply.

## 7. Host Descriptors and Atomic Effects

A pure descriptor identifies operation capability, implicit resource reads, possible effect legs, possible induced cause classes, rollback owner, generated Weight owner, fee owners, and unsupported or dynamically unresolved surfaces. Descriptor construction performs no dispatch, quote execution, mutation, network request, or population scan. A current resolver may read only declared bounded keys at one snapshot and cannot turn a possible route into an observed execution.

| Descriptor family | Required boundary |
| --- | --- |
| `AssetOps` | Exact account/asset debits and credits; mint/burn issuance deltas; native anchor/freezer restrictions |
| `DexOps`, Router, TMC | Candidate/route uncertainty, recipient versus gross amounts, trading fees, issuance and reserve effects, Oracle/certified-ingress consequences |
| `LiquidityOps` | Reserve legs, LP mint/burn, retained amounts, pool identity, atomicity and namespace restrictions |
| `StakingOps` | Input/receipt identities, custody and maturity dependencies, adapter-defined native/non-native effects |
| `FeeCollector` | Collection destination and fee kind; allocation is a separate later Actor effect |
| Certified ingress | Producer identity, authenticated source, movement/notify atomicity, possible useful-latch consequence |
| Oracle | Typed producer/feed/revision, freshness and coalescing; no external fair-price claim |

The table names coverage obligations, not a second implementation of these domains. Unknown capability fails closed for that explanation: it emits typed unsupported information and never guesses economic legs. It does not disable an otherwise valid Task or change its execution semantics.

An `EffectBundle` identifies one Task attempt and its atomic effect domain. Structural bundles describe possible successful legs; committed bundles exist only for successful committed Tasks. `StopCycle`, skipped Steps, and rolled-back Tasks have no committed bundle. Prefix bundles survive a later failure. A failed Task may still have a separately evidenced Action fee/control outcome under the existing fee boundary; that charge is not a successful economic Task bundle.

Each leg reports domain, exact or unknown resource, debit/credit/mint/burn kind, amount truth, and evidence. Mint and burn explicitly report issuance delta in the affected asset. No cross-asset sum, USD valuation, or netting away retained balances is canonical. Split legs preserve canonical authored order; implicit legs use descriptor-defined order. A concrete route or receipt conversion is committed truth only when its actual identity and amount are evidenced.

## 8. Resource Attribution and Receipt Decision

Resource reporting separates source operation, shared detection/materialization, Actor-specific Trigger control, Pipeline Machine control, Action attempt, and host Task effect. It names transaction, Trigger, Pipeline, Action, and Task-native/trading fees independently, alongside state holds and runtime subsidy. Both RefTime and ProofSize are mandatory. Maxima, registered/charged Weight, measured execution, and actual fee transfers have distinct labels.

Shared work is attributed to its shared owner and evidence coordinate. Canonical reports MUST NOT divide it equally among recipients, assign it all to the first Actor, or silently count it again within every Step. Analytical allocations are optional materialized policy with a declared formula and cannot claim measured per-Actor cost.

Implementation must first test reconstruction from existing Cycle/Step/fee events and state. Compare that result with a compact receipt event and bounded receipt state for exactness, event bytes, RefTime, ProofSize, reads/writes, and lifecycle cleanup. Retain state only if a required current canonical datum cannot be obtained through an accepted cheaper owner. Unknown actual cost remains unknown; maximum Weight is not an actual measurement.

No newly introduced hot-path read is accepted. Any required added event bytes, writes, larger proof, or computation needs explicit production-Weight and full-runtime differential evidence against frozen geometry, preserving its acceptance gates. If a cause or receipt design fails that gate, revise its encoding/owner or the specification explicitly; never silently weaken causality or carry unaccepted overhead into release.

## 9. System Role Catalog

The reference runtime owns a finite typed `SystemRole` catalog, with stable role identity, sovereign account, optional current Actor identity, lifecycle class, and catalog identity. Classes distinguish active, dormant, custody-only, and vacant bindings. Generic Actors owns none of the concrete role names or topology.

Role identity is independent of display name, Actor ID, storage locator, Contract hash, and incarnation. Historical views resolve role-at-event under the event's runtime/catalog identity. Rebinding a role changes current binding without rewriting history. A custody-only or vacant role must not fabricate a runnable Actor. Genesis and TryRuntime validate unique role keys, admitted sovereign bindings, cardinality, and correspondence with concrete host adapters. Existing Activation DAG and sealed Anchor behavior remain unchanged.

## 10. Binding and Bounded APIs

`introspectionIdentity` is `blake2_256("deos:actor-introspection:v1" || SCALE(binding))`. The binding contains schema version, Actor semantic-model identity, ordered host descriptor identities, System-role catalog identity, and relevant runtime metadata identity. Its canonical order is schema-defined; duplicate keys and unknown required versions fail closed. Runtime code and production Weight identities travel in the snapshot envelope even when unchanged schema permits reuse of structural extraction.

The reference API exposes these logical methods; concrete SCALE names/version numbers belong to implementation metadata. Every method is read-only and returns bounded typed errors (`NotRegistered`, `UnsupportedBinding`, `UnsupportedCapability`, `InvalidContract`, `LimitExceeded`, or `StateUnavailable`) rather than partial unlabeled success.

| Method | Input and bound | Result |
| --- | --- | --- |
| Binding identity | No population input | Binding and published bound constants |
| Actor topology | One Actor ID | One Contract's bounded topology and current context, or absent/dormant status |
| Proposed topology | One bounded canonical Contract plus type/mutability | Structural topology; no installation or execution |
| Current cause | One Actor ID | At most active and pending cause references, with current Cycle context |
| System-role bindings | One role or bounded page cursor/limit | Runtime-owned finite catalog page and catalog identity |

Let `S` be `MaxContractSteps`, `P` the admitted Predicate total, `A` the maximum amount/dependency surfaces per Task, `H` the maximum descriptor legs per Task, and `R` the runtime catalog bound. The implementation publishes concrete encoded-byte and work bounds for all inputs and outputs; topology size/work is bounded by `O(1 + P + S * (A + H))`, cause count by two, and role traversal by `R` with a per-call page cap. Host expansion MUST have explicit finite bounds before a descriptor is admitted. Requests exceeding a bound reject before unbounded allocation.

No API returns raw frame/page/heap coordinates, follows effects recursively, resolves all Actors sharing an asset, walks history, or predicts FIFO service time. Runtime execution cannot call the query path as a prerequisite. Current cost/eligibility projections reuse the existing `ActorCostApi`, `ActorResourceApi`, and `ActorEligibilityApi` owners instead of reconstructing their answers in the client.

## 11. Materializer, Finality, and Invalidation

The reference artifact records `Cause -> TriggerOccurrence -> Cycle -> Step -> ProducedCause` only for evidenced links. It binds genesis, covered block range, finalized block hashes, code/metadata/introspection identities, Contract identity, Actor incarnation, and role-at-event. TriggerOccurrence identity includes its recipient and useful-latch coordinate; Step attempt identity includes block/phase/event coordinates, so retries cannot collide.

Input records are normalized by chain position and canonical consequence order. Identical duplicates are idempotent; conflicting duplicates reject. Artifact bytes use one versioned canonical encoding with stable ordering and exact integer/byte representation; their domain-separated hash excludes presentation text and processing timestamps. Full rebuild and incremental ingestion over identical finalized input MUST produce identical bytes and hashes. CLI and library use the same transform.

Provisional ingestion is held separately and may be rolled back to the common ancestor. A claimed reorganization of already accepted finalized history halts ingestion with a finality-conflict error; it is not silently repaired. Missing ranges, unavailable creation/Contract history, pruned events, unsupported runtime epochs, and uncertified parents create typed gaps. A gap cannot be joined by temporal proximity or erased by a later matching balance.

Caches key by genesis, block hash, runtime/binding, and queried Contract or Actor context. A new head invalidates current resolutions; a code/metadata/descriptor/catalog change invalidates affected interpretations. Stored historical claims retain their original identities and are reinterpreted only with an explicitly matching decoder. Replaced Contracts and recreated Actors never inherit old current causes. Unavailable providers leave direct bounded state available and mark historical continuity unavailable or stale.

## 12. Acceptance and Threat Witnesses

Specification acceptance establishes ownership and implementable boundaries only. Implementation acceptance requires these witnesses against executable package/runtime behavior and the accepted production tree.

| Surface | Required falsification witnesses |
| --- | --- |
| Language | Every Trigger family, DNF/timing, amount/funding mode, Task, completion/error policy; zero and maximum Steps; stored/proposed equality |
| Causes | Fee/ingress rollback, busy deferred latch, repeated source, redundant latch no-op, coalesced observations, missed cadence, retry, close/recreate, Contract replacement |
| Effects | Successful prefix plus failed suffix, atomic multi-leg rollback, separate fee outcome, explicit issuance, unknown route, exact adapter identity |
| Resources | No double charge, shared-owner attribution, maximum/actual distinction, receipt alternatives, bounded bytes and both Weight dimensions |
| Identities | Wrong genesis/code/metadata/binding, source spoofing, ordinal collision/overflow, slot/ID reuse, role rebinding, stale proposal |
| APIs | Oversized and malformed input, bounded expansion, unsupported host, missing Actor, no writes/events/fees, no physical geometry or population scan |
| Materializer | Full/incremental byte equality, reordered/duplicate/conflicting input, reorg/finality conflict, gaps, long Pipelines, high fanout |
| Noninterference | Identical admission, FIFO, Q1, Economic Zipper, class neutrality, committed state/effects/fees with introspection instrumentation; no new hot-path read |

Materializer scale evidence uses 10,000, 100,000 synthetic, and 1,000,000 mostly dormant historical identities with declared ranges, distributions, peak memory, and rebuild time. This off-chain evidence does not establish runtime capacity. Production acceptance includes package/runtime/client/materializer tests, embedding, no-std, TryRuntime, Clippy, production Weight/Wasm, metadata and client ABI convergence under one binding.

## 13. Implementation Anchors

The starting executable surfaces are [Actor typed Contracts](../template/pallets/actors/src/types/contract.rs), [host ports and certified ingress](../template/pallets/actors/src/adapters.rs), [runtime APIs and Cycle/Task events](../template/pallets/actors/src/lib.rs), and [reference Actor configuration](../template/runtime/src/configs/actor_config.rs). Existing events already expose Actor/Cycle/Step coordinates and several economic amounts; they do not by themselves prove complete upstream cause identity, implicit host legs, or actual per-Actor resource attribution.

These anchors constrain implementation, not the reverse: changes to physical control geometry must not change the public local semantics in this document. Shipped storage, modules, descriptors, event coverage, and integration limitations belong in package architecture and the reference integration documentation after implementation and validation.
