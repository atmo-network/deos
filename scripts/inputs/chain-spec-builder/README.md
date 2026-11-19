# Chain-spec-builder source inputs

This directory owns the pinned Builder19 producer inputs and its fail-closed acceptance boundary. It is not a general CLI security approval or an alternative runtime-genesis policy owner.

## Original and maintained source

- `source.json` retains the original `staging-chain-spec-builder 19.0.0` archive, revision, original lock, Rust 1.94.1 compiler, default features, release settings and declared Linux x86_64/macOS arm64 targets. The original archive/lock identity is not rewritten as a maintained release or SDK Builder21.
- `maintenance.json` binds the original manifest/lock, the separate `maintenance.lock`, and three complete project-owned source packages under `template/vendor`: Hickory 0.24.4, LRU 0.12.3 and webpki-roots 0.25.4. They are reused without changing their bytes; neither a Skill nor an operator-selected hash supplies a project input.
- The maintenance lock selects compatible security corrections, including Wasmtime 36.0.20 and its coupled Cranelift line, Tokio 1.53.3, rustix 1.1.6, h2 0.4.16, Rustls 0.23.46/webpki 0.103.16, tracing-subscriber 0.3.20, memmap2 0.9.11, crossbeam-epoch 0.9.20, anyhow 1.0.103 and zstd 0.13.4/safe7.4.0/sys2.1.2. Package selection alone does not close remaining feature/caller findings.
- `accepted.json` identifies the observed producer and its separate artifact-security status; an original identity-only record does not identify a maintained image. `maintenance.json.sourceReview` independently owns target-specific construction approval. Source approval never sets artifact `securityQualified`.

The archived `accepted.json` remains bound to the previous caller. Moving the Node pin out of the browser workspace and updating its value changed the checked caller closure and required verifier runtime; an independently reviewed construction basis, producer receipt and artifact decision for the new closure are needed before Script 04 can execute it. The old receipt must refuse; neither its version nor identical ELF bytes supply the missing caller approval.

## Source-only acquisition

From any working directory, invoke the repository's `scripts/02-install-tools.sh --only chain-spec-builder --builder-source-only` with loader-injection variables unset. This downloads the checksum-pinned original archive, verifies its full extraction, retains `original-Cargo.toml` and `original-Cargo.lock`, then applies only the declared manifest path bindings and separate maintenance lock.

The existing producer records locked metadata/features, compiler/tool versions, restricted environment, complete dependency source inventories and before/after identities. Every registry payload is checked against its lock-checksummed archive; local packages must match their declared complete source digest. Root-source verification permits only the declared manifest/lock replacements, not arbitrary Rust changes. Ancestor/Cargo-home configuration overrides are refused.

Capture source with the exact native-helper environment intended for compilation. Source-only preparation records optional protoc/Perl/loader/Clang paths but does not establish that their execution prerequisites work. Check those prerequisites before review; changing their paths or bytes changes the construction basis. The strict inventory rejects symlinks: where necessary, supply regular-file, byte-verified native inputs with explicit origin provenance instead of relaxing source identity checks.

The source-only cache is `${XDG_CACHE_HOME:-$HOME/.cache}/deos-rc-0728/builder-boundary-correction/source-preparation/attempt.*`. A successful `source-prepared.json` means only that source preparation completed with matching before/after inputs. It contains no ELF, compilation success or trusted artifact-producer authority. Cargo metadata may acquire locked registry dependencies; ordinary shared caches are not a cleanroom. Historical captures and their original archive/receipt identities remain read-only and are never rebound to these inputs.

## Separate approval boundaries

- Independent complete source/API/graph review must accept the exact maintained inputs and each target before `--builder-provenance-only` can compile. The early gate checks target, digest presence and exclusive UTC expiry (`2026-12-12`). Immediately before Cargo, the final gate reconstructs current inputs, checks complete raw attempt stability and compares the construction digest with the project-owned review constant. A new attempt's self-consistency is not independent approval. No source-only operation authorizes a full builder rebuild.
- A subsequently authorized actual producer must generate a new receipt and executable identity. The original receipt and installed historical executable cannot be relabelled or reused by version.
- Independent artifact/security acceptance must bind that new image in `accepted.json`, including `constructionBasisSha256` and the exact `callerClosure` from its reviewed producer receipt. Both script04 verification points compare that map with current canonical files and compare the actual verifier Node with the recorded tool. The immutable receipt digest, subject digest, current target-specific source review and exclusive artifact expiry remain required. `verify-producer` retains the original identity-only route without granting execution. Source review alone never grants artifact security qualification; `source.json.security` describes the original unresolved requirements, not a second mutable approval switch.
- Script04 continues to support its existing runtime-Wasm path selection and complete named presets. It does not prove that an explicitly selected Wasm is current production code. Any finite source/caller disposition must preserve this boundary rather than assume trusted guest code or remove supported input selection.
- Linux source evidence is not macOS build/distribution/notice or target qualification. Declared macOS acquisition remains represented; its acceptance is separate. Other Builder CLI subcommands remain outside a script04-scoped caller disposition.

## Reviewed construction basis

`construction-basis.json` is the explicit schema-1 projection hashed as SHA-256 of compact `JSON.stringify` bytes, not the pretty file bytes. `source-prepared.json.inputs.constructionBasisSha256` names it. The independently reviewed value belongs in `maintenance.json.sourceReview.constructionBasisSha256`; missing or unreviewed approval data refuses compilation. The project file records independent target-specific authority, never a hash argument or an automatically accepted current snapshot.

| Basis dimension | Included inputs |
| --- | --- |
| Original and maintained source | Raw `source.json` SHA, original archive, original/maintained manifest and lock identities, explicit maintenance fields and complete named patches |
| Compilation | Target, release/profile flags, restricted environment, actual Rust/Cargo/native/Node tool paths and hashes, compiler versions, target cfg and host/native-tool versions |
| Configuration | Shared Cargo home, absent ancestor/Cargo-home overrides, optional protoc/Perl/loader/Clang input paths and inventories |
| Dependency construction | Full metadata and resolved feature graph; every package's lock-checksummed registry archive or declared local source, file path/SHA/mode inventory, including macros, build scripts, generators and templates |
| Producer and caller | Exact script02, script04, `_common.sh`, `provenance.mjs`, `source.json` and Node version authority `.node-version`; the loaded common bytes must equal the canonical helper |

Only `maintenance.json.sourceReview`'s explicitly enumerated fields (`status`, `targets`, `expiresAt`, `scope`, `constructionBasisSha256`) are excluded from construction identity. Unknown maintenance/review fields refuse. The whole raw contract object and raw SHA remain in before/after attempt stability and original receipt integrity. Changing only approval metadata preserves the construction digest but changes the raw attempt; an in-progress attempt cannot silently adopt that change.

Only known work-root fields relocate: exact private HOME/target, metadata workspace/target/build directories, root manifest/target-source paths, root `path+file` package IDs and their graph references, and the exact feature-tree root line. Registry homes, project patch roots, tools and optional configuration paths do not relocate. No general string replacement touches package payloads, source text, arbitrary metadata, hashes or other paths. The producer invokes Node by its canonical executable path, not an ephemeral version-manager alias; tool paths/hashes and alias resolutions otherwise remain exact reviewed inputs. Only known local patches exclude package-root Cargo `target` and `.git` state from source inventory. Nested source directories such as `src/target` are included; registry/root archive checks reject unarchived entries, and reviewed generators must not consume undeclared output/VCS inputs.

## Raw output publication

Script04 retains complete runtime-owned presets and arbitrary selected runtime Wasm. Before replacement, the structural leaf requires a regular non-symlink JSON file, required ChainSpec root fields, exclusively raw genesis, hex-byte `top` and `childrenDefault` maps, nonempty `:code` and exact equality to the selected Wasm bytes. This is format/data validation, not a second genesis projection or hardcoded economic policy. Candidate generation occurs on the output filesystem, so the final move publishes a validated complete file without a cross-filesystem copy over the old output. Existing directory/FIFO/symlink destinations refuse before product invocation and again before publication.

All helper reads/hashes reject non-regular or symlink inputs before opening, then use nonblocking/no-follow opens and an fd type check. This includes source/configuration contracts, receipts, tool payloads and generated JSON, preventing FIFO reads from hanging.

## Validation

`02-install-tools.sh --self-test` and `04-generate-chain-spec.sh --self-test` validate routing, receipt substitutions, raw schema/hex/CODE properties, bounded directory/FIFO/symlink refusals and unqualified security refusal. Private approved fixtures prove both public caller verification points pass a baseline and refuse actual script04 invocation/common/MJS drift without changing the receipt or subject. Script04 additionally executes only private shell stubs: valid raw output publishes; exit-zero empty/truncated/plain/invalid raw output preserves the previous output and cleans its own candidate.

`node scripts/inputs/chain-spec-builder/provenance.mjs self-test /absolute/prepared/attempt` adds isolated actual-source admission tests with private copies of the project contract, helpers and source packages. These exercise fresh caller/helper/cfg/features and lock-plus-declared-checksum drift against a retained approval digest, same-graph source/build/manifest changes, unknown packages, untracked generators, configuration/vendor-contract refusal, approval-only construction equivalence and full raw attempt drift. Fake approvals exist only in the temporary fixture project. No test mutates the live project approval, shared registry source, historical evidence or ELF; none compiles or executes the product builder.
