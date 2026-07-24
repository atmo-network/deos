---
name: staking-delivery
description: Coordinates guarded DEOS native-staking bootstrap readiness, call preparation, authority boundaries, and activation handoff without owning staking semantics or shared commands.
fmos: true
---

# Staking Delivery

Use this skill when checking or preparing the canonical `$NTVE/stNTVE` staking-pool bootstrap and its dependent Liquidity Actor activation.

## Ownership Boundary

This skill owns operator sequencing, readiness interpretation, authority classification, and plan-only handoff. It does not define staking economics, asset identifiers, pool mechanics, System AAA policy, governance authority, transaction signing, or shared command implementation.

The current dependency chain remains ordered:

```text
native staking registration and stNTVE receipt
  → collision-safe LP namespace
    → canonical NTVE/stNTVE pool
      → non-zero balanced liquidity and LP issuance
        → runtime LP validation
          → guarded Liquidity Actor activation
```

A later state never substitutes for missing evidence at an earlier step.

## Single-Owner Map

| Concern | Truth owner |
| --- | --- |
| Staking, receipt, custody, and reward semantics | Staking/runtime code, tests, specification, and architecture |
| Pool and LP namespace behavior | Asset Conversion/runtime code and tests |
| Liquidity Actor behavior and activation guard | AAA/runtime code, tests, and AAA architecture |
| Readiness probe and call-data generation | `scripts/bootstrap-native-staking-local.sh` and its `--help` |
| Operator sequence, evidence level, and authority handoff | This skill |
| Remaining launch or release gates | `BACKLOG.md` |

Do not copy command flags, concrete identifiers, call bytes, percentages, balances, or subsystem rules into this skill. Read them from the target runtime, owning docs, and shared script output.

## Readiness States

| State | Evidence | Permitted action |
| --- | --- | --- |
| Unregistered | Native staking asset/receipt readiness is absent | Prepare the owning Root/governance registration call |
| Namespace blocked | LP namespace cannot safely allocate the canonical pool token | Stop and repair the owning runtime/configuration boundary |
| Pool absent | Registration and namespace are valid, canonical pool is missing | Prepare the approved pool-creation call |
| Pool empty | Pool exists without valid bilateral reserves or LP issuance | Prepare explicit operator staking/liquidity calls |
| Pool ready | Pool, reserves, issuance, and runtime LP validation pass | Check dependent actor state |
| Activation ready | Pool ready and actor exists inactive with the expected role | Prepare guarded activation |
| Active | All readiness and actor activation checks pass | Perform no bootstrap mutation |

Prepared call data proves only that a next action was encoded. It does not prove authorization, signing, submission, inclusion, or resulting readiness.

## Route

1. Read the staking architecture bootstrap sequence, runtime configuration, relevant staking/AAA tests, shared script `--help`, and open delivery gate.
2. Bind the workflow to an explicit endpoint/network and obtain current state through the shared read-only `check` route.
3. Classify the first unmet readiness state. Stop on inconsistent state, identifier collision, unexpected actor role, or unsupported authority rather than skipping ahead.
4. Use the shared `prepare-calls` route only for the next justified transition. Keep generated Root/governance and signed-operator calls distinct.
5. Treat governance execution, signing, submission, liquidity movement, and actor activation as account-affecting approval gates. Do not cross them without explicit authorization for the exact target, calls, assets, and amounts.
6. After an authorized external action, rerun the read-only check and advance only from observed state.
7. Activate the dependent Liquidity Actor only after the pool exists, both reserves and LP issuance are non-zero, and runtime LP validation succeeds.
8. Synchronize architecture/backlog evidence only to the highest observed state, then run the changed-scope completion route.

## Safety Contract

- Default to read-only checks and plan-only call generation.
- Preserve explicit Root/governance versus signed-operator ownership; never collapse them into one privileged bootstrap identity.
- Validate concrete asset and pool identities against the target chain rather than relying on remembered defaults.
- Keep actor activation last and fail closed when pool health or actor identity differs from expectation.
- If a post-pool step fails, leave the actor inactive and return an exact remediation owner.
- Never fall back to liquid `stNTVE`, inferred transfer-event backing, a different pool, or web-client seed state.
- Keep signer material and generated target-specific call data out of durable skill/context prose.
- Recheck after every external mutation; do not compose speculative later calls from unobserved intermediate state.

## Graceful Degradation

An unavailable RPC permits only a generic sequence review, not a target readiness claim. Missing optional machine-readable output may fall back to the same read-only human result. Missing target identity, registration evidence, namespace safety, bilateral liquidity, LP validation, expected actor identity, required authority, or explicit mutation approval fails closed.

## Handoff

Report only:

- Target endpoint/network identity;
- Observed readiness state and first unmet dependency;
- Checks actually executed;
- Next prepared call family and its authority class, without duplicating opaque bytes in durable prose;
- Whether all work remained read-only/plan-only;
- Blocker, remediation owner, and exact unblocker;
- Post-action state only when re-observed.

Stop when the next transition requires governance, signing, funds, live submission, or correction of an inconsistent runtime state.
