# Third-party notices

This directory owns the verbatim third-party notice texts that DEOS-distributed artifacts must carry. Each file is named by license family plus the first 12 hex digits of its SHA-256. A text shared by several packages is stored once. `SHA256SUMS` verifies every text from this directory with `sha256sum -c SHA256SUMS`.

| File | License text | Packages |
| --- | --- | --- |
| `mit-23f18e03dc49.txt` | MIT | `unicode-ident` 1.0.11: the selected alternative of `MIT OR Apache-2.0` |
| `unicode-68f5b9f5ea36.txt` | Unicode-DFS-2016 | `unicode-ident` 1.0.11: the mandatory conjunctive Unicode notice |
| `unicode-f367c1b8e1aa.txt` | Unicode-3.0 | ICU, `zerovec`, `yoke` and related packages of the accepted Omni reference bundle |

Each bundle manifest owns the binding between its packages and these files. The accepted Linux Omni reference lists its packages in `notices` in [`scripts/inputs/canonical-linux-reference.json`](../scripts/inputs/canonical-linux-reference.json). Bundle assembly copies the referenced files into `bin/notices/` and verifies each digest.

## `unicode-ident` 1.0.11

- Registry source: `registry+https://github.com/rust-lang/crates.io-index`.
- Source archive: https://static.crates.io/crates/unicode-ident/unicode-ident-1.0.11.crate, SHA-256 `301abaae475aa91687eb82514b328ab47a211a533026cb25fc3e519b86adfc3c`.
- Declared SPDX expression: `(MIT OR Apache-2.0) AND Unicode-DFS-2016`. DEOS selects the MIT alternative and keeps the conjunctive Unicode notice, including its copyright, permission, warranty and name-use terms. Both texts are verbatim archive payloads.
- Locked by the PVF validation host (`template/tools/pvf-validation`), the standalone Assets backport tests, the chain-spec-builder maintenance lock and the Omni reference bundle.

## Distribution obligation

Before promoting a selected binary bundle (relay, workers, corrected Omni, required builder or distributed PVF host), copy every notice text its packages require into the bundle documentation. Include those files and their checksums in the bundle manifest. Any missing or changed file requires refusal and review. Verify a source archive against the identity recorded here when assembling the bundle, not against a replacement identity supplied by the caller.

This is conservative accompaniment. It does not prove which lookup-table bytes survive linking in a binary. It is not a legal warranty, and it does not claim that a verified bundle has already been modified. Private cargo-deny admission does not satisfy distribution. Promotion remains gated on actual notice accompaniment and bundle verification. This directory has no dependency on agent Skills.
