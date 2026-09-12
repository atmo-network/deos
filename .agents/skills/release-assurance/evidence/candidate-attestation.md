# 0.7.26 Candidate Attestation Preparation

## Current Applicability

The recorded checkpoint evidence below predates the PR #32 non-tail Crossing admission repair and does not establish readiness of the corrected candidate. The local fix changes `select_crossing_admission` to exclude tail-only pair fallback when classification carries non-tail refill authority; it preserves the scalar path and invariant guards. Its regression reproduces a worker fault before the fix and proves exact scalar state-root equivalence and continued service afterward. Independent source-geometry and RefTime/ProofSize boundary checks cover the selector.

The corrected-source Weight reassessment is complete in [EXP-0114](../../architecture-experiments/tracks/actors/EXP-0114.md#corrected-source-reassessment--2026-09-12). Five focused selections and four controls on fresh benchmark Wasm `00b469b1…` confirm the installed `1,467,000` ps selector upper fit with zero ProofSize and database work. The production Weight source remains byte-identical at `46a89aa4…`; the other execution owners retain Case A applicability over unchanged paths and domains. The record owns the predeclared method, all observations, output hashes, invalid-launch classification and current evidence disposition.

The corrected canonical build produces production Wasm `91c23b2f77b57265e8e32ec8645610a32b7d4763b0ced31ab56ef64e8c0f5a2d`. Metadata, descriptors, production Weight, ABI, bounds, cost, fee, semantic and ingress evidence remain byte-identical. Observation evidence changes only its runtime code hash to `0x05ce940c1f07464ccece73badd673dacf68d055e110047067d7037d9c44d0efa`; its file SHA-256 is `2823378de12a5b1cfaded2b3c6f335717dc8294932a03a4a526b3c073db116e7`. The exact-Wasm test pin, integration binding and Weight comparison now identify the corrected artifact.

The build, metadata/descriptor export and observation-evidence freshness check passed over unchanged build inputs. Focused validation then passed on source identity `9f8c7d2c4cbe0de6ce9811d86b5c388727e2e1d2e7fde19a9916c8ef3bb4fc32`: all 151 client automation tests, one executed `reference_full_block_replays_in_production_wasm_with_verified_storage_proof` test, workspace/all-target/all-feature Clippy with `-D warnings`, and the changed-scope completion gate including native runtime tests. The replay retained exact source, Weight, Wasm and metadata identities.

The focused replay stdout has SHA-256 `ff1ad7640cfea496fac4a51f314d4708a744774bb74f1e1a2dff124fcbb523d7`; `DEOS_VERBOSE=1 ./scripts/actors-assurance.sh --production-reference-replay` reproduces this proof boundary. Corrected-candidate full validation and its attestation reconciliation remain required under `BACKLOG.md`. The checkpoint identities and full-profile validation below remain pre-repair evidence only, not corrected-candidate release readiness.

## Evidence Boundary

This retained local preparation binds the pre-repair validated artifact-bearing release checkpoint. It proves local identity agreement for that checkpoint only; it is not a signed release attestation and does not claim corrected-candidate readiness, production-network safety or the unmet User reactive target.

- Candidate checkpoint: `7e0e1eb8` (release identity prepared and rebound; exactly the tree validated below).
- Candidate tree: `c048b49f381dfbe4c2f84179a3e1e15997e12641`.
- Sorted tracked-content manifest digest: `7c5847b3d3976df1f256c60342763f6b88dacc2371028c9385abb47103f31d36`.
- Exact-tree `./scripts/validate-local.sh full` log: SHA-256 `ce1d09d45293d0308a43c45c505507e7834ebe02185493d295012da1734cb03d`.
- Dependency-provenance log: SHA-256 `fc99520e835defdce4a51cca6720f64453b3ad5fe060b3caecdbb3970b31e70e`.

The validation and dependency-provenance logs are transient local outputs; the recorded digests bind the observed runs and the commands below reproduce them from this checkpoint.

## Release-Version Reissue

The workspace package-version bump `0.7.25 → 0.7.26` reissues the production Wasm byte identity while leaving every semantic artifact unchanged. The sealed candidate checkpoint built `77eaf529…`; the prepared release tree builds `3388be55…`, and the canonical build inside the full validation reproduced `3388be55…` bit-exactly. Production Weight, runtime metadata, PAPI descriptors, ABI, bounds, cost, fee, semantic and ingress evidence are byte-identical across the reissue; only the Wasm, its code hash in the observation evidence and identity prose moved. The exact-Wasm regression suite is rebound to the release artifact, including `ACCEPTED_PRODUCTION_WASM_SHA256` in `production_block_replay.rs`, and revalidated end to end.

## Artifact Identities

| Artifact | SHA-256 |
| --- | --- |
| Production compact Wasm | `3388be55da933c90c69465a35c08c09aae420165d7a6608816f65fb13b37cbf7` |
| Runtime metadata | `75868ea75fd85e7b79db12650a7407e89e249f5564b2157d55ac46cd26cb5d12` |
| PAPI descriptor manifest | `2f9c6de877f2f4d575c49992a262109d1635f68e6b586cb9b34844cbf3849da2` |
| Actors production Weight | `46a89aa48b6814b0d02b3b423b99a134cc6b84f08cbb1c3dfaf4bf4074c18346` |
| Actors ABI manifest | `7c645cde7c5405ee805281dace5cddd0e80fa60be6fdf45ac06fbeabe3403e71` |
| Actors protocol bounds | `0cd7716d1e694b516a33a47c40f16cf97ba90510e8b4792feddac89826e29d15` |
| Actors cost vectors | `06bd4d21b7dcb5cb0d35336035689195714d4fcfa1e613ba1dd1e61f6cf22c69` |
| Actors fee vectors | `4b902a9d8aca969e09b7cbe6a1227628a65f4c242f7b80f84e032c6245be5b56` |
| Actors semantic manifest | `15ab53a31193e8a03686db673b0c57929f8425d92ea14868e7be7da831819342` |
| Observation evidence | `8c6013905a0bbbf2be3dc81ef1d473e5760ae90a2da333a74b3b7ff8990e5a86` |
| Ingress evidence | `ea9697460ec0ad05c89b68f263b60ead719e26cc9164d240c1da3e2bd347c67a` |

The descriptor-manifest digest above is corrected to match `web-client/.papi/descriptors/package.json` at `7e0e1eb8`; this corrects the recorded digest, not the artifact.

The full profile passed on the checkpoint — simulator tests, complete Rust workspace CI, clean web-client validation, the full Actors assurance gate, benchmark compilation with the generated storage-name audit, the deterministic production runtime, and metadata, descriptor and generated-evidence regeneration — with zero worktree drift. The checkpoint's generated consumer identities carry the retained binding: the cost and fee vectors and the observation evidence name the reissued Actors Weight `46a89aa4…`, and the observation evidence names the release runtime code hash `0xe88130ce…`.

## Weight Owner Soundness

The v0.7.25-to-0.7.26 Weight delta is exactly one new production owner: `crossing_selection_probe` (`1,467,000` ps RefTime, `0` ProofSize, `0` reads, `0` writes). All 160 existing bodies are byte-identical, the delta ledger freshness check passes, and no other production pallet has a Weight delta. The release-version reissue does not alter the comparison: the production Weight source is byte-identical across the candidate checkpoint and the release tree, and the regenerated delta ledger carries the release Wasm identity.

The owner benchmarks the extracted `select_crossing_admission` selection arithmetic. The service loop reads the owner once, checks `can_consume` before the arithmetic, consumes it once and then calls the selection; the selection performs no storage access and returns `Refused` whenever its generated owner does not fit, so the segment cannot execute on unreserved Weight and is charged exactly once per unit. The Benchmark Reassessment Protocol qualification is explicit: two fresh full-pallet generations were environment-dominated and inadmissible, so the pinned binding is retained and the previously unowned admission-selection segment is explicitly covered after `actors/EXP-0114`.

Every other production Weight owner is unchanged from `v0.7.25`. The only non-generated `WeightInfo` binding in the runtime is the upstream `cumulus_pallet_weight_reclaim` config, which SDK 2606 generates privately and exposes as the same measured constant through its public `()` implementation.

## Threat Boundary Review

The candidate diff adds no authority, storage partition, scheduler path, certified producer, adapter, custody role, XCM route or read projection, so no threat family or trust assumption changes. The release-version reissue likewise changes only the package-version string embedded in the Wasm bytes and moves no boundary. The sole generated-artifact addition is the Weight owner above, which closes rather than widens the undercharged-branch surface. The stale four-candidates-per-block assumption was replaced by the retained `2/6/8` Crossing cycle with its eight-per-block ceiling. The dependency review on 2026-09-12 re-validated the locked graphs, licenses, binary checksums and current advisories against the dated exception ledger (RustSec database `b50980aad8b8f14f77e25a97b32dd94bf008b0af`, 17 material findings, no critical npm finding).

## Reproduction

- Manifest digest:

```text
git ls-tree -r --name-only 7e0e1eb8 | LC_ALL=C sort | while IFS= read -r f; do
  printf '%s  %s\n' "$(git show "7e0e1eb8:$f" | sha256sum | cut -d' ' -f1)" "$f"
done | sha256sum
```

- Exact-tree validation: `./scripts/validate-local.sh full`
- Dependency provenance: `./.agents/skills/release-assurance/scripts/dependency-provenance.sh`
- Weight delta: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh [--check]`

## Remaining External Gate

After the local gates in `BACKLOG.md` pass, remote CI and review resolution must cover the corrected PR candidate before guarded merge, tag and release publication. PR #32 is already under review: the pre-PR squash is complete and must not be repeated. Corrected-candidate preparation has resumed; a remote correction must preserve reviewed history under `AGENTS.md` and pass the authorized release workflow rather than treating old-head CI as fresh evidence. Separate artifact packaging, published `SHA256SUMS`, and platform or signing attestation remain unselected capabilities, not established repository release requirements.
