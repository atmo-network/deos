---
name: aaa-delivery
description: Guides AAA scheduler validation, stress evidence, benchmark interpretation, release gating, and delivery handoff.
---

# AAA Delivery

Use this skill when an AAA change needs validation scope selection, scheduler stress evidence, benchmark interpretation, or release-candidate handoff. The skill owns agent judgment; its script owns deterministic execution.

## Workflow Boundary

- Inspect the AAA specification, architecture, backlog slice, changed code, generated weights, and runtime configuration before selecting a gate.
- Use `--quick` for bounded implementation slices that still need AAA-specific Clippy and pallet tests.
- Use the full gate for scheduler stress acceptance, release preparation, or changes to queue/wakeup capacity, fairness, liveness, or guaranteed `on_idle` admission.
- Keep the 10,000-entry occupancy profile enabled unless the touched contract cannot affect scheduler storage topology and the handoff states that reason explicitly.
- Treat benchmark-host timing as comparative evidence, never as a reference-block throughput promise.
- After a meaningful slice, synchronize `BACKLOG.md` and shipped architecture truth before the repository completion gate.

## Deterministic Entrypoint

Run the co-located gate directly:

```bash
./.agents/skills/aaa-delivery/scripts/release-gate.sh --help
```

The stable operator/CI compatibility bridge remains:

```bash
./scripts/aaa-release-gate.sh --help
```

Both paths execute the same implementation. Successful orchestration stays compact; failures retain a complete temporary log and print a bounded tail. Set `DEOS_VERBOSE=1` only for diagnosis.

## Completion Evidence

Report:

- selected quick/full profile and occupancy decision;
- passed stress cases and any explicitly skipped external gate;
- generated weight and production-Wasm hashes when they changed;
- remaining backlog gate and exact unblocker;
- confirmation that no throughput claim exceeds measured production-Wasm and runtime-budget evidence.
