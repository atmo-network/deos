---
name: aaa-delivery
description: Guides AAA scheduler and package-boundary validation, stress and portability evidence, release gating, and delivery handoff.
fmos: true
---

# AAA Delivery

Use this skill when an AAA change needs validation scope selection, scheduler stress evidence, package-boundary portability evidence, or release-candidate handoff. The skill owns AAA delivery judgment; shared root scripts own deterministic execution.

## Workflow Boundary

- Inspect the AAA specification, architecture, backlog slice, changed code, generated weights, and runtime configuration before selecting a gate.
- Route benchmark design, evidence classification, production-weight integration, and claim limits through [`benchmarking`](../benchmarking/SKILL.md); this skill owns the AAA-specific stress profile and release consequence.
- Use `--quick` for bounded implementation slices that still need AAA-specific Clippy and pallet tests.
- Use the full gate for scheduler stress acceptance, release preparation, or changes to queue/wakeup capacity, fairness, liveness, or guaranteed `on_idle` admission.
- Keep the 10,000-entry occupancy profile enabled unless the touched contract cannot affect scheduler storage topology and the handoff states that reason explicitly.
- Treat benchmark-host timing as comparative evidence, never as a reference-block throughput promise.
- Keep the embedding runtime as a separate external-consumer Cargo package under the `pallet-aaa` ownership boundary; pallet unit mocks do not replace that public-contract proof.
- For package-readiness changes, validate fixture feature profiles and a local `cargo package` archive while keeping registry publication approval-gated.
- After a meaningful slice, synchronize `BACKLOG.md` and shipped architecture truth before the repository completion gate.

## Checkpoint Batching

- Treat the active release checkpoint as the commit boundary when its backlog groups several compatible work items.
- Give each item one focused test family or filter. Shared invariant tests must identify the smallest owning item set so failures remain attributable.
- During edits, run focused owners and the cheapest package check; do not replay stress or repository-wide gates after every internal correction.
- Before the checkpoint commit, run its aggregate package/runtime set and one full AAA gate when scheduler, ingress, terminal lifecycle, economics, or production weights changed.
- Repeat a full gate only after it fails and causes implementation/generated-artifact changes, or after a later edit crosses another runtime boundary.
- Documentation-only reconciliation after a green code candidate uses context gates. Final release assembly still runs the complete release matrix once.

## Deterministic Entrypoint

The shared human/CI/skill implementation lives at the root public automation boundary:

```bash
./scripts/aaa-release-gate.sh --help
```

This skill selects and interprets that command; it does not own a second executable copy. Successful orchestration stays compact, failures retain a complete temporary log and print a bounded tail, and `DEOS_VERBOSE=1` restores diagnostic detail.

## Completion Evidence

Report:

- Selected quick/full profile and occupancy decision;
- Passed stress cases and any explicitly skipped external gate;
- Generated weight and production-Wasm hashes when they changed;
- Remaining backlog gate and exact unblocker;
- Confirmation that no throughput claim exceeds measured production-Wasm and runtime-budget evidence.
