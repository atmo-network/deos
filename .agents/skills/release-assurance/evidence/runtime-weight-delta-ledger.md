# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.25` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

Candidate release: `0.7.26`. The candidate production runtime was built with `./scripts/03-build-runtime.sh`; compact Wasm SHA-256 is `91c23b2f77b57265e8e32ec8645610a32b7d4763b0ced31ab56ef64e8c0f5a2d`. The accepted benchmark owners use `frame-omni-bencher 0.22.0` / CLI `58.0.0`, `50` steps, `20` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host `fedora`, and CPU `AMD Ryzen 7 4800H with Radeon Graphics`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.

Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: v0.7.25 → 0.7.26 candidate | Base delta | ProofSize: v0.7.25 → 0.7.26 candidate | Reads: v0.7.25 → 0.7.26 candidate | Writes: v0.7.25 → 0.7.26 candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Actors | `crossing_selection_probe` | `— → 1,467,000` | new | `— → 0` | `— → 0` | `— → 0` | C |

## Interpretation

Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.

## Retired Weight Owners

- None.

Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Architecture Experiments Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `135ac7884638775986cc19b2dc6a124800cd6b83072b5c74a5d3edcee48a5b71`.

