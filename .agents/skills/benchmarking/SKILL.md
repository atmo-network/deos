---
name: benchmarking
description: Selects, designs, interprets, and integrates benchmarks without duplicating shared scripts or overstating evidence; the current DEOS route covers FRAME runtime measurement.
---

# Benchmarking

Use this skill when a DEOS change needs benchmark design, focused measurement, parameter comparison, generated-weight integration, or production evidence. The current executable route covers FRAME runtime benchmarking; frontend benchmarking remains out of scope until the project adopts an explicit measured need and shared command surface.

## Ownership Boundary

This skill owns benchmark methodology and agent judgment:

- identify the bounded unit of work and relevant worst-case state;
- select compile, diagnostic, comparative, or production evidence;
- interpret RefTime, ProofSize, reads, and writes independently;
- route accepted numbers to generated weights and architecture evidence;
- reject claims stronger than the executed evidence.

It does not own runtime semantics, pallet implementation, AAA scheduler stress policy, release publication, or command implementation. Shared execution remains in [`scripts/benchmarks.sh`](../../../scripts/benchmarks.sh), production Wasm construction remains in [`scripts/03-build-runtime.sh`](../../../scripts/03-build-runtime.sh), and each script's `--help` remains the sole flag/usage reference.

## Single-Owner Map

| Concern | Truth owner |
| --- | --- |
| Command syntax and prerequisites | Root script and its `--help` |
| Benchmark selection and interpretation | This skill |
| Worst-case setup and postconditions | Owning pallet `benchmarking.rs` |
| Accepted production numbers | Runtime-generated `weights/*.rs` |
| Architectural meaning and hashes | Owning architecture document |
| Remaining evidence | `BACKLOG.md` |

Do not copy script flag inventories, generated numbers, benchmark setup code, or subsystem policy into this skill. Link to the owner and retain only reusable decision rules.

## Evidence Classes

| Class | Purpose | Permitted conclusion |
| --- | --- | --- |
| Compile | Verify benchmark registration and buildability | The benchmark surface compiles |
| Diagnostic | Falsify setup or compare a local code path cheaply | Relative/local behavior only |
| Comparative | Compare candidates under identical steps, repeats, runtime, and host | Candidate selection for measured cases |
| Production | Accept generated weights from the production benchmark runtime and rebuild production Wasm | Runtime weight/evidence acceptance within measured bounds |

Unavailable optional comparative evidence may yield a narrower result. Missing production evidence fails closed when weights, runtime admission, or release claims depend on it.

## Route

1. Read the changed call path, existing benchmark, `WeightInfo`, runtime binding, owning architecture section, and open backlog gate.
2. Define one operation with explicit bounded components and postconditions. Split cases when storage topology changes, such as preserving versus deleting a page.
3. Run the benchmark compilation route through `scripts/benchmarks.sh --check`.
4. Use one focused extrinsic and a temporary output for diagnostic evidence. A focused output may supply a reviewed method, but it must never replace the complete generated pallet file.
5. Run a same-environment comparative matrix only when selecting a parameter or representation.
6. Accept production weights only after production-quality measurement, method/storage annotation review, runtime binding, and focused compile/check validation.
7. Rebuild production Wasm through `scripts/03-build-runtime.sh` when accepted runtime weights or runtime code change.
8. Record only accepted numbers, tool/runtime conditions, hashes, limits, and claim boundaries in the owning architecture document; narrow `BACKLOG.md` to what remains.
9. Run the changed-scope completion route. Escalate to the owning delivery/release gate when capacity, fairness, liveness, admission, or release acceptance changed.

Use `--skip-build` only while reusing the same freshly built benchmark runtime for a coherent matrix. Rebuild after source, features, runtime configuration, or toolchain changes.

## Benchmark Design

- Measure work proportional to the bounded state actually processed, not total global state by convenience.
- Construct the smallest state that reaches the real worst-case branch; do not inflate unrelated actors, assets, pages, or history.
- Separate operations when they have different reads, writes, proof topology, cleanup, or failure semantics.
- Keep setup outside the measured block and assert the intended branch and resulting state inside or after it.
- Use measured ProofSize mode for storage-sensitive paths and inspect generated storage annotations against implementation reality.
- Model independent RefTime-stop and ProofSize-stop behavior when the production caller admits work in both dimensions.
- Treat page size as I/O granularity unless verified runtime budgeting establishes a throughput bound.

## Interpretation Contract

- RefTime, ProofSize, reads, and writes remain separate evidence dimensions.
- Minimum execution time is not the charged weight and does not override the generated model.
- Host timing supports controlled comparison; it does not establish reference-block throughput.
- A count ceiling, diagnostic benchmark, or `Weight::MAX` test does not prove ordinary-block capacity.
- Fixed benchmark cases prove only their measured topology. State assumptions and avoid extrapolation beyond them.
- Production claims require the runtime-bound method and production-Wasm evidence, not a temporary generated file alone.

## Integration Checks

Before handoff, verify:

- benchmark name, `WeightInfo` declaration, generic fallback, and runtime implementation agree;
- generated read/write counts match the storage annotations and measured branch;
- no placeholder runtime weight remains;
- formatting, benchmark compilation, scoped Clippy/checks, and relevant tests pass through root scripts;
- architecture prose distinguishes measurement, inference, configured ceiling, and throughput claim;
- recorded weights and Wasm hashes match current files.

Stop rather than generalize when the next benchmark lacks a defined production caller, bounded component, acceptance decision, or truthful claim it could change.
