# Runtime Threat and Trust-Boundary Review Map

## Purpose

This private Skill reference provides a compact cross-system threat and abuse-case checklist for release assurance. Package specifications own subsystem invariants, package architecture documents own shipped implementation, integration documents own concrete DEOS composition, and tests own executable evidence. This map does not replace those project owners or claim immunity from market, governance, operator, or supply-chain failure.

The protected outcome is deterministic bounded protocol behavior under malformed input, adversarial ordering, delayed service, arithmetic boundaries, partial external failure, and inconsistent state. Required boundaries fail closed rather than fabricating state, authority, price truth, execution success, or rollback.

## Trust Boundaries

| Boundary | Trusted input or authority | Untrusted or fallible side | Required treatment |
| --- | --- | --- | --- |
| Runtime origin | Root and explicitly typed governance adapters | Ordinary signed accounts and arbitrary runtime calls | Reject implicit privilege; test every privileged origin against a signed caller. Signed Oracle publication remains feed-local and grants no general administration. |
| Consensus storage | Canonical bounded partitions and indexes | Partial, corrupt, stale, or contradictory partition combinations | Load through one canonical classifier, return typed failure or conservative absence, and reconcile complete bounded topology in try-state. |
| Scheduler service | One FIFO, wakeup owner, ticket allocator, and typed block/tick clocks | Queue pressure, delayed timestamps, heavy heads, stale entries, and index exhaustion | Preserve order and committed progress, bound every scan and retry, reselect mixed clocks per unit, and terminate unrecoverable index exhaustion explicitly. |
| Actor adapter call | Typed task input, task-owned spend/output bound, and measured host adapter | DEX, staking, asset, Oracle, or hook rejection after partial work | Keep task-local effects transactional; preserve committed plan prefixes; classify temporary and permanent failures without compensation or whole-plan rollback. |
| Certified movement | Producer-declared movement protocol and preflight owner | Reordered notify, post-dispatch failure, XCM holding failure, or duplicate delivery | Admit only the closed certified protocol inventory; bind consequence, rollback, and Weight ownership; never infer movement by scanning events. |
| Arithmetic | Checked or widened authoritative calculation | Overflow, narrowing loss, denominator failure, terminal horizons, and adversarial reserve magnitudes | Use checked operations, `U256` where products can exceed native widths, and exact narrowing. Saturation is limited to explicit semantic floors, conservative Weight caps, or bounded telemetry. |
| Price reference | Fresh typed Oracle observation, then a direct-pool reserve fallback | Stale or missing feeds, manipulated pools, self-certifying execution quotes, and MEV | Keep the execution quote out of its own reference path, reject excessive deviation before mutation, and make no external fair-price or ordering guarantee. |
| Governance service | Chronological bounded epoch buckets and typed payload authority | Delayed clocks, maximum-density buckets, malformed tallies, and unsupported payload execution | Drain one persisted phase at a time under measured caps, advance epochs only after every family drains, fail arithmetic transactionally, and deny unsupported authority. |
| Governance custody | Checked aggregate source custody and maximum retained horizon | Concurrent proposals, replacement votes, double counting, premature unlock, and source-ledger drift | Reuse only actually custodied power, admit only free increments, extend horizons monotonically, and reconcile aggregate custody against host ledgers. |
| Runtime custody identity | Host-owned infallible account mappings | Narrow-account truncation, tagged subaccount collision, actor-id aliasing, and signed ownership | Preserve stable host addresses, prove non-aliasing across custody roles, and keep System Actor control under a non-signable pallet account. |
| Asset and XCM identity | Exact bounded `Location <-> AssetId` bijection and configured reserve path | Native `Here` registered as foreign, stale reverse mappings, multi-asset holding, teleport, and arbitrary `Transact` | Reserve host locations, reconcile both mapping directions, cap holding at one asset, and keep teleport and arbitrary execution filters closed. |
| Read and client projection | One finalized runtime code, metadata, state, descriptor, and evidence identity | Malformed state, unknown variants, stale generated assets, session reconstruction, and provider substitution | Return typed failure or explicit unavailability; reject identity drift and unknown variants; never present cached, derived, or materialized data as canonical chain truth. |
| Weight and block budget | Production-Wasm benchmarks and explicit two-dimensional maxima | Undercharged branches, hidden database work, ProofSize growth, and hook pressure | Account separately for RefTime, ProofSize, reads, and writes; bind generated host weights; prove every admitted maximum composition fits its runtime budget. |
| Runtime panic surface | Mechanically precluded invariant sites with exact ownership | User-reachable input, corrupt storage, adapter failure, arithmetic boundary, and new unchecked assumptions | Return typed failure for reachable cases. Audit every retained panic site and reject new sites without exact mechanically enforced preconditions. |
| Dependency and toolchain provenance | Exact tool versions, immutable revisions, registry integrity, and reviewed exceptions | Compromised packages, stale advisories, substituted bootstrap tools, and unreviewed transitive reachability | Fail on unreviewed or expired material findings, forbid npm critical exceptions, verify installed tool identity, and retain reachability/rationale/expiry for every exception. |

## Abuse-Case Falsifiers

| Abuse case | Falsifying evidence owner |
| --- | --- |
| A signer acquires Root-equivalent Actors, Governance, Router, Staking, TMC, Asset Registry, asset-force, preimage, XCM, XCMP, collator, or upgrade authority. | Runtime authority inventory in `template/runtime/src/tests/integrity_tests.rs` and typed payload coverage in `governance_integration_tests.rs`. |
| A malformed Actor partition executes, consumes placement, reports ready, or accepts Continuation mutation as though active. | Canonical-loader, partition-mask, scheduler, projection, and try-state tests in `template/pallets/actors/src/tests.rs`. |
| Continuation cancellation re-primes a retained signal behind its old physical wakeup, creating two conflicting pointer claims and a permanent due-head stall. | Exact wakeup invalidation tests in the Actors package and independent DEX embedding fixture, plus the measured `continuation_cancel` branch. |
| A failed task, certified movement, XCM deposit, Router call, pool index, TMC distribution, staking operation, or Governance terminal action retains a partial root. | Package and runtime exact-root rollback tests owned by the affected subsystem. |
| Delayed Governance epoch work skips chronology, drops a same-epoch suffix, advances early, or exceeds its measured per-block family cap. | Governance epoch-service unit tests, runtime maximum-composition assertions, and generated Governance weights. |
| Concurrent Governance proposals count more voting power than source custody or release it before the maximum horizon. | Governance aggregate-custody tests and runtime host-ledger reconciliation. |
| Native `Here` enters the foreign registry, one XCM program exceeds the holding bound, or teleport/arbitrary call execution becomes reachable. | Asset Registry reserved-location tests and runtime XCM identity, holding, filter, ingress, and rollback tests. |
| A Router System swap certifies itself, accepts an out-of-bound reference deviation, or mutates before reference rejection. | Router package tests and runtime Actors/Router integration tests. |
| An unknown runtime eligibility, close reason, execution phase, or typed failure reaches browser display as a valid state. | `web-client/scripts/test-actors-eligibility.mjs` and generated Actors ABI freshness checks. |
| A production path reaches a package fallback Weight, exceeds RefTime or ProofSize admission, or changes database cardinality without ledger evidence. | Runtime Weight tests, production generated files, `scripts/benchmarks.sh`, and this Skill's Weight delta evidence. |
| A reachable panic or unreviewed dependency/tool substitution enters the candidate tree. | Alignment panic audit, this Skill's dependency-provenance review, project bootstrap identity checks, and the completion gate. |

## Assumptions and Non-Guarantees

- Root remains a constitutional trust boundary; typed governance narrows ingress but does not make an authorized Root-equivalent payload harmless.
- The active collator set remains permissioned until a parachain-consumable relay beacon satisfies the randomness contract.
- Registered Oracle publishers are trusted only for their admitted feed identities; publication does not grant broader authority and local reserve fallback is not an independent market oracle.
- Slippage, output bounds, and reference deviation limit protocol fills but do not guarantee fair price, MEV resistance, deep liquidity, execution liveness, or profitable economic outcomes.
- Bounded retry and catch-up guarantee a finite service path only while blocks continue and required markets, adapters, budgets, and dependencies eventually become available.
- Canonical browser views cover bounded current chain truth. Archive, search, and unbounded history require an explicitly materialized provider.
- Dependency review proves the recorded graph and reachability classification, not the absence of unknown upstream vulnerabilities.
- DEOS `0.x` is fresh-genesis source. No production storage lineage, `Live` preset, or downstream network-launch assurance exists before `1.0`.

## Change Discipline

During release assurance, a change that adds an authority, storage partition, scheduler path, certified producer, external adapter, custody role, XCM route, read projection, generated artifact, or dependency exception must update its owning project specification or architecture evidence and this checklist when the threat family or trust assumption changes. Pure implementation detail that leaves these boundaries unchanged stays with its package owner and must not be duplicated into this map.
