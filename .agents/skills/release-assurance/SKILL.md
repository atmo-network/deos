---
name: release-assurance
description: Reviews a DEOS release candidate across dependency provenance, runtime trust boundaries, multidimensional Weight deltas, generated evidence, and exact-tree attestation without becoming a project build or CI dependency.
---

# Release Assurance

Use this Skill when a DEOS release candidate needs agent-led assurance beyond ordinary project validation: dependency reachability review, generated production-Weight comparison, cross-system threat-boundary review, artifact identity reconciliation, or exact-tree release attestation preparation.

## Ownership Boundary

This Skill owns release-assurance judgment and its private evidence workflow:

- classify dependency findings by actual release/runtime reachability;
- maintain short-lived reviewed exceptions with explicit rationale and expiry;
- compare production Weight in RefTime, ProofSize, reads, writes, and parameter slopes;
- review cross-system trust boundaries, abuse cases, falsifiers, assumptions, and non-guarantees;
- reconcile runtime, metadata, descriptors, generated evidence, checksums, and the candidate tree before attestation;
- state exactly what local evidence proves and what remains external or unverified.

It does not own runtime semantics, dependency versions, generated production weights, project validation, CI, operator bootstrap, release publication, signing authority, or Git history mutation. Those remain with project code, lockfiles, runtime-generated files, root scripts, workflows, and the authorized release process.

## Dependency Direction

The project does not invoke this Skill or its private scripts. Removing `/.agents/skills` must leave project build, tests, CI, release profiles, and runtime behavior unchanged.

This Skill may read project truth and invoke public project operations. Its private scripts may source `scripts/_common.sh`, inspect lockfiles and generated Weight, call `scripts/01-download-binaries.sh --check`, and consume current advisory services. No root script, workflow, package, or project document may depend on a private path in this Skill.

## Evidence Owners

| Concern | Truth owner |
| --- | --- |
| Rust and npm dependency identity | `template/Cargo.lock`, `web-client/package-lock.json` |
| Rust, Node, and npm toolchain identity | `template/rust-toolchain.toml`, `web-client/package.json` |
| Bootstrap binary identity | `scripts/01-download-binaries.sh` and project pins |
| Runtime semantics and invariants | Owning specifications, architecture documents, code, and tests |
| Accepted production Weight | `template/runtime/src/weights/*.rs` |
| Runtime and generated artifact identity | Built Wasm, metadata, descriptors, generated evidence, and release checksums |
| Open release work | `BACKLOG.md` |
| Assurance method, temporary review decisions, and comparative evidence | This Skill |

## Assurance Route

1. Read the release section in `BACKLOG.md` and `CHANGELOG.md`, inspect the candidate diff, and identify the exact intended version and baseline tag.
2. Run ordinary project validation through the narrowest applicable root route. Skill evidence never substitutes for failed project validation.
3. Prepare exact private review tools:

```bash
./.agents/skills/release-assurance/scripts/prepare-tools.sh
```

4. Review dependency and bootstrap provenance:

```bash
./.agents/skills/release-assurance/scripts/dependency-provenance.sh
```

5. Regenerate the candidate-specific Weight comparison after accepted production Weight changes, then require freshness:

```bash
./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh
./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check
```

6. Review `references/runtime-threat-boundaries.md` against changed authorities, storage partitions, scheduler paths, adapters, custody roles, XCM routes, read projections, panic surfaces, and dependency boundaries. Update the map only when the release changes a threat family or trust assumption.
7. Reconcile the hashes of production Wasm, metadata, descriptors, generated evidence, lockfiles, toolchain pins, and `SHA256SUMS` against one exact candidate tree.
8. Stop before commit signing, history rewrite, push, tag, GitHub Release, or publication unless the authorized release workflow explicitly permits those actions.

## Dependency Review Contract

- Every material Cargo or npm finding must be fixed or appear exactly once in `config/dependency-provenance-exceptions.json`.
- Each retained finding names reachability, a material rationale, and an expiry no more than 90 days after review.
- New, duplicate, stale, expired, fixed-but-retained, or graph-absent exceptions fail.
- npm critical findings cannot be excepted.
- A lockfile-only alternative is not runtime reachability; a native tool finding is not consensus-Wasm reachability. Preserve that distinction without claiming the package is safe in general.
- Review evidence expires with its advisory database, dependency graph, toolchain identity, or recorded horizon.

The current rationale and evidence boundary are in `references/dependency-provenance.md`.

## Weight Delta Contract

- Compare RefTime, ProofSize, reads, writes, and every parameter slope independently.
- Never summarize a multidimensional change as one timing percentage.
- Classify positive deltas as required correctness cost, bounded service topology, merged canonical work, measured optimization interaction, unexplained noise, or regression.
- Reject unexplained positive deltas. Re-run the benchmark or inspect changed storage annotations rather than normalizing noise by prose.
- A generated delta ledger explains accepted numbers; it does not create production Weight. Runtime-generated `weights/*.rs` remains authoritative.
- Preserve benchmark-runtime and final production-Wasm identities separately when they differ.

Candidate-specific generated evidence lives at `evidence/runtime-weight-delta-ledger.md` and is replaced rather than accumulated as global project documentation.

## Threat Review Contract

Use `references/runtime-threat-boundaries.md` as an assurance checklist, not as a second subsystem specification. For every affected boundary, identify:

- trusted authority or input;
- untrusted or fallible side;
- required fail-closed treatment;
- executable falsifier;
- assumption or non-guarantee that limits the claim.

If a claim is already owned by a package specification, architecture document, code, or test, link to that owner rather than duplicating its implementation detail here.

## Completion Evidence

A local assurance pass may claim only:

- project validation passed for its recorded inputs;
- reviewed dependency findings match the current locked graphs and dated exception ledger;
- generated Weight comparison matches current production Weight sources and the stated baseline;
- reviewed threat boundaries have named executable falsifiers;
- declared artifacts and checksums agree with the inspected candidate tree.

It does not prove absence of undisclosed upstream vulnerabilities, production network safety, market fairness, governance benevolence, operator correctness, or signed release identity. Exact-tree signing or equivalent platform attestation remains a separate external gate.
