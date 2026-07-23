---
name: aaa-delivery
description: Guides AAA scheduler validation, stress evidence, benchmark interpretation, release gating, and delivery handoff.
---

# AAA Delivery

Use this skill when an AAA change needs validation scope selection, scheduler stress evidence, benchmark interpretation, or release-candidate handoff. The skill owns agent judgment; shared root scripts own deterministic execution.

## Workflow Boundary

- Inspect the AAA specification, architecture, backlog slice, changed code, generated weights, and runtime configuration before selecting a gate.
- Use `--quick` for bounded implementation slices that still need AAA-specific Clippy and pallet tests.
- Use the full gate for scheduler stress acceptance, release preparation, or changes to queue/wakeup capacity, fairness, liveness, or guaranteed `on_idle` admission.
- Keep the 10,000-entry occupancy profile enabled unless the touched contract cannot affect scheduler storage topology and the handoff states that reason explicitly.
- Treat benchmark-host timing as comparative evidence, never as a reference-block throughput promise.
- After a meaningful slice, synchronize `BACKLOG.md` and shipped architecture truth before the repository completion gate.

## Deterministic Entrypoint

The shared human/CI/skill implementation lives at the root public automation boundary:

```bash
./scripts/aaa-release-gate.sh --help
```

This skill selects and interprets that command; it does not own a second executable copy. Successful orchestration stays compact, failures retain a complete temporary log and print a bounded tail, and `DEOS_VERBOSE=1` restores diagnostic detail.

## Completion Evidence

Report:

- selected quick/full profile and occupancy decision;
- passed stress cases and any explicitly skipped external gate;
- generated weight and production-Wasm hashes when they changed;
- remaining backlog gate and exact unblocker;
- confirmation that no throughput claim exceeds measured production-Wasm and runtime-budget evidence.
