---
name: runtime-upgrade-delivery
description: Coordinates guarded DEOS runtime-upgrade preparation, authorization verification, ministerial relay planning, and post-upgrade evidence without owning governance decisions or shared commands.
---

# Runtime Upgrade Delivery

Use this skill when preparing, checking, relaying, or verifying an authorized DEOS runtime upgrade.

## Ownership Boundary

This skill owns operator sequencing, evidence classification, safety gates, and handoff for the two-step upgrade path. It does not decide which code governance authorizes, define runtime version or migration semantics, implement shared commands, hold signing credentials, or publish a release.

The authority split remains exact:

```text
governance authorizes code_hash
  → operator verifies identical local code bytes
    → any operator may relay those already-authorized bytes
      → chain execution and events determine the result
```

Relay is ministerial transport, not a second governance vote or policy interpretation.

## Single-Owner Map

| Concern | Truth owner |
| --- | --- |
| Upgrade authority and on-chain semantics | Governance/System code, tests, and governance architecture |
| Version, storage, and migration contract | Runtime/pallet code, tests, and owning subsystem docs |
| Production Wasm construction | `scripts/03-build-runtime.sh` and its `--help` |
| Live authorization check and relay command | `scripts/authorized-upgrade-local.sh` and its `--help` |
| Live migration/block checks | `scripts/try-runtime-local.sh` and its `--help` |
| Operator sequencing and evidence claims | This skill |
| Remaining acceptance gates | `BACKLOG.md` |

Do not copy command flag inventories, runtime versions, authorized hashes, generated call data, or migration rules into this skill.

## Evidence Ladder

| Rung | Evidence | Permitted conclusion |
| --- | --- | --- |
| Offline artifact | Reproducible production Wasm path, size, and hash | One local candidate artifact exists |
| Local compatibility | Focused tests, metadata/version assertions, and required try-runtime checks | Candidate satisfies executed local checks |
| Live authorization | Chain view and verifier classify the candidate hash | Awaiting authorization, mismatch, or ready to relay |
| Relay plan | Matching offline call data and explicit target | A ministerial relay can be prepared |
| Submitted relay | Explicit approval plus observed submission result | Transaction was submitted; not yet necessarily enacted |
| Post-upgrade observation | On-chain events, code/version state, and required health checks | Observed chain state reflects the upgrade outcome |

Never promote an offline hash, prepared call, submitted transaction, or expected event into a stronger rung.

## Route

1. Read the owning runtime/specification changes, storage-version consequences, runtime version contract, integration tests, governance architecture path, and open release gate.
2. Build the production Wasm through the shared numbered script when the candidate artifact is absent or stale. Record its exact hash without treating it as authorization.
3. Run the smallest required local runtime checks and migration/try-runtime route. A fresh-genesis reference change does not invent migration ceremony; a live downstream upgrade requires its owning bounded migration evidence.
4. Use the shared authorized-upgrade verifier in plan-only mode against the explicit target chain.
5. Stop on `awaiting-governance-authorization` or `authorized-hash-mismatch`. Report the target, local hash, observed authorization state, and exact unblocker.
6. When the verifier reports `ready-to-relay-code`, prepare call data or a relay plan without submission.
7. Treat any submission flag, signer use, account mutation, or live relay as an approval gate. Proceed only after explicit user authorization for that exact target and artifact.
8. After an authorized relay, observe system events and runtime code/version state, then run the declared post-upgrade health checks. Report failures as observed chain state, not inferred rollback or success.
9. Synchronize architecture/backlog/release evidence only to the highest rung actually completed, then run the changed-scope completion route.

## Safety Contract

- Default to plan-only behavior.
- Bind every live check and relay plan to an explicit endpoint, Wasm path, and hash.
- Never substitute a newly built artifact after authorization without rechecking the hash.
- Never bypass a mismatch, disabled/missing version check, failed try-runtime requirement, or unresolved storage-version consequence.
- Invalid authorized code may clear pending authorization with an explicit rejection event; prevention through exact preflight matters more than retry convenience.
- Keep credentials and signer material outside repository files and skill text.
- Do not use browser UI as the relay surface; the current browser contract remains read-only for this second step.
- Do not claim rollback unless an implemented and tested rollback mechanism produced evidence for it.

## Graceful Degradation

An unavailable RPC permits offline artifact and local compatibility evidence only. Missing optional call-data tooling may yield a narrower ready-state report if live hash verification still succeeded. Missing required authorization, hash equality, migration evidence, credentials, approval, or post-upgrade observation fails closed at that rung.

## Handoff

Report only:

- target endpoint/network identity;
- candidate Wasm path and hash;
- highest completed evidence rung;
- authorization classification and version-check state;
- local/try-runtime checks actually executed;
- whether relay remained plan-only or received explicit approval;
- observed post-upgrade events/state when available;
- blocker and exact unblocker.

Stop when the next step needs governance action, explicit relay approval, unavailable live evidence, or an unresolved compatibility decision.
