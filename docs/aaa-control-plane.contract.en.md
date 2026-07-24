# AAA Control-Plane Contract

> Off-chain contract for representing, reviewing, forecasting, composing, and indexing bounded AAA programs without expanding consensus state.

## Scope

The control plane operates above `pallet-aaa`. It does not change task semantics, scheduler behavior, storage, dispatch indices, or runtime admission. Its first obligation is to preserve an exact relationship between a human-readable plan and the runtime bytes that governance or an owner may submit.

This contract owns executable plan-artifact identity, structural diff rules, forecast provenance, simulation boundaries, governance composition inputs, and materialized history classification. The AAA specification remains authoritative for semantics; runtime metadata and SCALE remain authoritative for concrete encoding.

## Canonical Executable Plan

A canonical executable plan is a chain-bound artifact with these required fields:

| Field | Encoding | Meaning |
| --- | --- | --- |
| `format` | UTF-8 literal | `deos.aaa.plan` |
| `formatVersion` | unsigned integer | Control-plane envelope version; initially `1` |
| `genesisHash` | `0x`-prefixed 32-byte hex | Target chain identity |
| `specVersion` | unsigned integer | Runtime semantic compatibility marker |
| `transactionVersion` | unsigned integer | Signed-call compatibility marker |
| `metadataHash` | `0x`-prefixed 32-byte hex | `blake2_256` of the exact runtime metadata bytes used for encoding |
| `aaaType` | `User` or `System` | Runtime `AaaType` admission context |
| `mutability` | `Mutable` or `Immutable` | Runtime admission and `RetryLater` context |
| `programScale` | `0x`-prefixed SCALE bytes | Concrete runtime `ProgramInput` value |
| `planId` | `0x`-prefixed 32-byte hex | Deterministic artifact identity |

`programScale` is the canonical program representation. JSON objects, form state, labels, comments, token symbols, decimal display amounts, and generated previews are projections only. They must never substitute for exact runtime types or enter `planId` implicitly.

`planId` is `blake2_256("deos:aaa-plan:v1" || LE32(specVersion) || LE32(transactionVersion) || genesisHash || metadataHash || SCALE(aaaType) || SCALE(mutability) || programScale)`. The enum bytes come from the identified metadata; integer components use fixed little-endian encoding, never host-language stringification.

A canonical artifact is executable only when live genesis hash, runtime versions, and metadata hash match the envelope. A mismatch requires explicit re-decode, revalidation, re-encoding, and a new `planId`; tooling must not silently rebind stale bytes.

## Human Projection

A human projection must decode `programScale` through the exact metadata identified by `metadataHash`. It must show every `ProgramInput`, trigger, schedule window, funding policy, step, condition, task, amount-resolution variant, asset id, account id, ratio, and error policy without lossy defaults.

Balances and identifiers use full base-unit decimal strings in transport JSON. Accounts and opaque bytes use canonical hex. Ratios expose their integer `Perbill` parts as well as optional display percentages. Token symbols, labels, localized prose, and decimal formatting remain annotations resolved from an explicitly identified registry snapshot.

Projection acceptance requires `decode(programScale) -> projection -> encode == programScale`. Unknown variants, missing fields, overflow, noncanonical numbers, or metadata lookup failure reject the artifact instead of producing a partial editable plan.

## Structural Diff and Version History

A structural diff compares decoded typed trees only when both artifacts share `genesisHash` and `metadataHash`. It reports additions, removals, moves, and field changes by stable structural path. Array position is semantic for execution-plan steps and conditions; tooling must not sort them for presentation.

Artifacts with different metadata hashes are `IncompatibleUntilRebound`. A migration-aware tool may decode each side with its own metadata and present a named comparison, but it must not claim byte-level or dispatch compatibility.

Version history is materialized truth. An indexer may correlate artifacts with actor calls and lifecycle events, but consensus stores no plan archive. Every history item records source transaction/block identity, observed finality, `planId`, target actor or creation intent, and whether artifact bytes were available or reconstructed.

## Forecast and Dry-Run Provenance

Every forecast records the canonical `planId`, block hash or state snapshot, metadata hash, and runtime API or local model version used. Results are advisory and become stale when any dependency changes.

Weight forecasts must preserve RefTime and ProofSize separately. Fee forecasts identify evaluation, execution upper bound, fee conversion, and lifecycle overhead rather than returning one unexplained number. Amount resolution identifies live balances, trigger snapshots, minimum-balance constraints, fee reservation, and adapter quotes used by each step.

Local simulation cannot claim runtime truth unless it executes the matching runtime Wasm against the identified state snapshot. Heuristic or adapter-local projections must carry a narrower provenance label and may not authorize submission automatically.

A matching-runtime request binds `planId`, genesis, block hash and number, state root/source, runtime-code hash, metadata hash, runtime versions, and runtime API identity. The provider must echo the complete pin and return canonical SCALE result bytes. Client-side hash and echo validation prevents accidental identity drift; it does not prove that an untrusted provider executed the runtime, so the provider or verified executor remains an explicit evidence boundary.

## Matching-Runtime Simulation Provider

The first runtime provider simulates one attempt of an existing active actor whose stored `ProgramInput::Active`, `AaaType`, and `Mutability` exactly match the validated artifact. It supports an idle actor's next fresh cycle and a suspended actor's next Continuation attempt. It does not simulate creation, dormant activation, a proposed replacement program, scheduler throughput, queue position, or future block timing.

The request carries `aaa_id`, exact decoded program, actor type, mutability, and mode `FreshCurrentPlan | CurrentContinuation`. `FreshCurrentPlan` requires idle run state and starts at cursor `0` with the next cycle nonce. `CurrentContinuation` requires suspended run state and reuses the stored nonce, unresolved cursor, trigger snapshot, and cumulative outcomes while incrementing the attempt exactly once.

The runtime API runs only after normal liveness, lifecycle, window, nonce, fee-budget, and Continuation invariants pass. A mismatch or unavailable prerequisite returns a bounded typed error and performs no task. The API remains bounded by the actor's admitted plan, configured maximum steps, existing task weights, and the same adapter calls as production execution; it must not inspect an unbounded event or storage history.

The minimum result carries status `Completed | Aborted | Suspended`, cycle nonce, attempt, start cursor, optional unresolved cursor, finalized-through index, cumulative outcome totals, ordered bounded step outcomes, and canonical SCALE result bytes. A suspended result keeps its cursor unresolved; completed or aborted results expose no live Continuation cursor.

The entire API call executes inside an outer rollback transaction. Successful task effects remain visible to later simulated suffix tasks, failed task-local effects roll back under the existing pallet transaction boundary, and all simulated writes, events, fees, scheduler changes, funding promotion, closure, and adapter effects roll back before the API returns. Explicit rollback remains mandatory even when the host normally discards runtime-API overlays.

A provider calls this API against the exact finalized block named by the request, obtains runtime code and metadata from that same state, and returns their hashes with the block header state root. Remote RPC execution remains trusted-provider evidence unless a verified local executor or state proof independently establishes the same code and state.

## Partial Execution and Donation Sensitivity

Simulation follows task-scoped atomicity and non-atomic plans. It must preserve successful prefixes, roll back failed task-local effects, apply `AbortCycle`, `ContinueNextStep`, or Mutable-only `RetryLater`, and expose the unresolved cursor without inventing compensation.

Donation sensitivity classifies which resolved amounts can change when third parties transfer assets into actor-controlled or adapter-observed accounts before execution. The result identifies the affected step and amount surface; it does not predict external behavior or treat a donation as an attack by default.

## Governance Composition

Governance composition consumes a validated canonical artifact and a separately selected target/action. It must show the exact runtime call, origin/domain requirement, encoded call bytes, preimage or payload hash when applicable, and the artifact `planId`.

The plan artifact does not contain a signature, signer, nonce, tip, proposal advocacy, or governance decision. Signing and submission remain explicit approval boundaries. A composed payload becomes stale under the same runtime identity rules as its source plan.

## Read-Model Boundary

Canonical-chain truth includes current bounded actor program/state, dispatch outcomes, events, and runtime metadata at an identified block. Plan files, diffs, forecasts, simulations, annotations, and long version/cycle/funding histories are local or materialized truth.

Provider failure must degrade to the narrower live-chain surface with an explicit unavailable or stale state. The client must not synthesize archive continuity from session cache or present reconstructed plan bytes as directly stored chain artifacts.

## Validation Contract

Control-plane implementations must cover:

- Deterministic `planId` fixtures and rejection of malformed hex, overflow, stale metadata, and wrong-chain artifacts.
- Exact SCALE decode/project/re-encode round trips for every current plan variant.
- Ordered structural diffs, including incompatible-metadata classification.
- Separate RefTime, ProofSize, fee, state-snapshot, and quote provenance.
- Task rollback, committed-prefix, Continuation cursor, and donation-sensitivity scenarios.
- Runtime-provider rejection for artifact/program mismatch, wrong mode/run state, unavailable fee budget, stale code/state identity, and any write escaping the outer rollback.
- Governance payload byte visibility without implicit signing or submission.
- Finality, reorg, duplicate, replay, and missing-artifact behavior for indexed histories.

No control-plane test substitutes for pallet tests, runtime integration, production weights, or live operator authorization.
