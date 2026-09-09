# 0.7.25 Candidate Attestation Preparation

## Evidence Boundary

This local preparation binds the validated artifact-bearing checkpoint before release-history rewrite, signing, push, tag, or publication. It proves local identity agreement only; it is not a signed release attestation and does not claim production-network safety or the unmet User reactive target.

- Candidate checkpoint: `4a4bb07a6e807deb3693e7544cc0389eb00a6046`.
- Candidate tree: `f98374eeee93011379d546bc30eb269573ee0a4b`.
- Sorted tracked-content manifest digest: `8f5a44632a28fabc4f1949f76601a2ba4dc229fd8c61e69d076a2d9feeb8cd44`.
- Exact-tree `./scripts/validate-local.sh full` log: SHA-256 `1b594c30529cb0a02dedc7992bb0994199c9d8729886635502a92a4e12bb8615`.
- Dependency-provenance log: SHA-256 `fc99520e835defdce4a51cca6720f64453b3ad5fe060b3caecdbb3970b31e70e`.

## Artifact Identities

| Artifact | SHA-256 |
| --- | --- |
| Production compact Wasm | `25b9695fd9e900f17ae1f3fb0b815ac1403830264d9c31f7cee54e29f434b700` |
| Runtime metadata | `75868ea75fd85e7b79db12650a7407e89e249f5564b2157d55ac46cd26cb5d12` |
| PAPI descriptor manifest | `84aafcbb7b65f3dc01d307efd94f585e5b5b645ed146e644b7674590320b4505` |
| Actors production Weight | `9ce37ade391a29d4f60779f1887ad242dfeb4919d25cf0047ffba85d18f33901` |
| Actors ABI manifest | `7c645cde7c5405ee805281dace5cddd0e80fa60be6fdf45ac06fbeabe3403e71` |
| Actors protocol bounds | `0cd7716d1e694b516a33a47c40f16cf97ba90510e8b4792feddac89826e29d15` |
| Actors cost vectors | `ffc5f801a6ac469cace362e65125ec3eda2718eccab03e1215f7cbe58f6e3c0b` |
| Actors fee vectors | `9718850dbd55cb1732d725dd16724d34747c2a88621a51d0fa8b957801527abf` |
| Actors semantic manifest | `15ab53a31193e8a03686db673b0c57929f8425d92ea14868e7be7da831819342` |
| Observation evidence | `9e97a864833114cd2c5c5ca587f62ffa2164493d4d57d23bb3a84397eb830dec` |
| Ingress evidence | `ea9697460ec0ad05c89b68f263b60ead719e26cc9164d240c1da3e2bd347c67a` |

`validate-local.sh full` regenerated the production runtime, metadata, descriptors and generated client evidence without candidate-worktree drift. EXP-0095 owns benchmark and artifact provenance; the current v0.7.24-to-v0.7.25 Weight ledger owns multidimensional source comparison.

## Remaining External Gate

The authorized release flow must rewrite the dedicated version branch to its final single release commit, rerun exact-tree full validation against that rewritten tree, package the declared release artifacts, generate and verify release `SHA256SUMS`, and obtain platform or signing attestation. No local preparation authorizes those actions.
