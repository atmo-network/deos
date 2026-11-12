# Dependency Provenance Review

## 1. Scope and Owners

This Skill reference records how release assurance reviews input identity, licenses, checksums, and advisories without becoming a project validation dependency. `template/Cargo.lock` and `web-client/package-lock.json` own resolved dependency identity; `template/rust-toolchain.toml` and `web-client/package.json` own compiler and Node/npm identity; project bootstrap scripts own executable pins used by operators and builds.

`config/deny.toml` owns this Skill's Rust license and registry policy. `config/dependency-provenance-exceptions.json` owns its dated advisory and missing-license decisions. `scripts/dependency-provenance.sh` is the private deterministic review entrypoint; no project script or workflow invokes it.

## 2. Fail-Closed Review Contract

The review rejects an unlocked Cargo or npm graph, a mismatched Rust/Node/npm or audit-tool version, a missing or checksum-mismatched Polkadot SDK executable, an unknown Cargo registry, an unapproved dependency license, a registry npm package without lockfile integrity, an unreviewed Rust vulnerability or unsoundness advisory, and an unreviewed npm high or critical finding.

Every retained material finding states runtime reachability, rationale, and an expiry no more than 90 days after review. New, stale, duplicate, expired, or fixed-but-retained exceptions fail. npm critical findings fail even when listed. Low npm findings and RustSec maintenance notices remain visible in raw tool output but do not bypass the material vulnerability gate.

## 3. Current Dependency Qualification — One Unexcepted Crypto Blocker

The source-aware 2026-11-12 review uses RustSec `af7b3ec33460b8e2de8d422e16d628518891a519` with1,490 advisories, Cargo Audit0.22.2, Cargo Deny0.20.2, Rust1.94.1, Node22.22.0 and npm11.7.0. Earlier `fd6f3981…` snapshots retain their own scopes; a current-looking filename is not source correspondence. The retained npm response below is byte-identical to the previously accepted post-remediation response.

| Input / representation | SHA256 |
| --- | --- |
| Actual Cargo lock | `75f20fadc68d1f80adfc99bc8dff545cf4876d845d9902244b1704fa6aafb2b3` |
| npm lock | `f69b59d58479febae674d2d1f921a94860c5973f15303f49f406353d030df4b0` |
| Actual path-source Cargo Audit JSON | `bac30d53518d13a5f09767edf55ec9c112fe1dbe05925361b3250fed336af1b0` |
| Registry-version-view Audit JSON | `ba2aad96165d8e0821d45c3b75ff743b5797144e27d42e262d7de96a1e8081d2` |
| Review-only projected lock | `007b638949123870a433cdc851256e71754c62369310c56084ed1869076dfa1d` |
| Full all-feature metadata | `349fb1fefef95c5b4e89c68c6a8dd9f83c35e8f4b261a851d4b268a1ab1c2591` |
| npm Audit JSON | `3b9c5115f21576a514f16e80679427a7eb30b4018f3c06db92fb27842d2ab3a6` |

**Retained compatible updates:** The preceding `320a2775…` graph changed36 security-package versions without package additions/removals: Wasmtime36.0.20/coupled16-package family, Cranelift0.123.20 across13 packages, Rustls0.23.46, WebPKI0.103.16, Tokio1.53.3, rustix1.1.6 and zstd0.13.4/safe7.4.0/sys2.1.2. Twenty-three incidental reselections in18 unchanged parents were removed and standard Cargo resolution retained that selection. The current lock changes only Hickory0.24.4 and webpki-roots0.25.4 registry-to-path sources; all package versions and dependency edges remain unchanged. The SDK line and existing client/XCMP patches remain.

| Newly accepted source treatment | Scope and limit |
| --- | --- |
| `RUSTSEC-2026-0119` | Local Hickory0.24.4 backports upstream PR3613:64 compression candidates/120 eligible attempts, including canonical-name handling. Compression can decrease and existing size errors occur earlier. |
| `RUSTSEC-2026-0491` | The same package reserves at most eight initial entries for each header-controlled section/public reader and grows only after successful decoding; the complete query path delegates to the fixed reader. This is not an eight-record cap or universal memory ceiling. |
| `RUSTSEC-2026-0381` | Local roots0.25.4 removes exactly upstream-distrusted emSign ECC Root CA-G3, retaining all other anchor bytes and public API. No claim of a fully refreshed trust store or TLS-handshake attestation follows. |

Each package's `SECURITY-BACKPORT.md` owns archive/commit/patch provenance, compatibility and licenses. Independent source response SHA256 `eb8f7a4f47cd0317e0b2b38da40d34cc92718237ee193ec0d05075e186d2ea64` accepts those three treatments. Hickory default/release receipts each contain162 unit,2 allocation/growth and44 doc tests; roots has one default/release security regression. Parent `libp2p-dns` and `libp2p-websocket` checks pass. Independent focused checks cover both packages; optional OpenSSL/DNSSEC and production node/worker/Wasm behavior are not qualified.

**Prevent path-source blindness:** Cargo Audit omits local path packages. Its raw vulnerability count drops29→20 solely because nine records disappear; only three defects are fixed. The canonical private route obtains full locked/all-feature metadata and derives a temporary registry-identity lock for the four non-workspace local packages. Only source fields are inserted; exact reconstruction proves package names, versions and edges unchanged. Standard Cargo audits both views; the validator unions their untouched reports, requires equal database/settings/graph counts and retains local-source findings. The projection is counterfactual version matching, never actual source provenance or a production lock.

The union contains43 material affected-version records/40 unique Rust IDs, including unsound warnings. The ledger has43 material decisions:16 lock-only,19 source-prerequisite, one native-observability risk acceptance, four trusted-generation npm aggregates and three reviewed source backports. One Rust ID remains unexcepted. Reviewed-at/expiry remain2026-11-12/2026-12-12; package/source/feature/caller changes invalidate applicable decisions before that date.

| Conditional exclusions | Bound prerequisite |
| --- | --- |
| Hickory text `0487/0489/0490` | Local0.24.4 retains the `text-parsing` module gate; current all-feature metadata lacks activation. Source hashes and expected local-source identity are bound. |
| Hickory DNSSEC `0505/0511/0512` | Affected validation modules remain gated by absent `dnssec` in `rr/mod.rs` and `xfer/mod.rs`; they are excluded, not repaired. |
| Resolver `0514` | Registry resolver0.24.4 constructs `DnssecDnsHandle` only under absent `dnssec`. Expected registry identity and feature absence are enforced; an unreviewed local replacement fails. Validation requested without DNSSEC falls back to ordinary resolution, not DNSSEC security. |
| Retained prior exclusions | Their16 lock-only identities and selected zstd6/lru/memmap caller/feature prerequisites remain in the single ledger at unchanged package versions. No broad native-safety renewal is implied. |

IDs abbreviated above have prefix `RUSTSEC-2026-`. Source-backport decisions require exact current Cargo.toml and patched-file hashes; source-bound local exclusions additionally retain gate-file hashes. Feature guards require exact package/version/source and reject newly enabled prerequisites. Hash agreement proves identity, not correctness. CLI regression coverage includes hidden path findings, version/edge drift, DB/filter mismatch, unknown/stale/expired decisions, source hash/substitution drift, feature activation and critical npm refusal despite inconsistent summary counters. Missing view or old argument shape fails closed. Independent remedy response SHA256 `98511176c6abe68398574451f56dcd661bea37f90f53dd85d41e159942bf1787` accepts the registry-to-local source-drift guard and prefix-preserving acquisition tests; the helper passes5 positive/24 negative CLI cases. This acceptance does not qualify external executable behavior or whole-release security.

**Unresolved crypto:** `RUSTSEC-2026-0387`, crypto-bigint0.5.5 through elliptic-curve/K2560.13 and P2560.13, remains unexcepted. The newer UintRef/BoxedMontyForm APIs and two-overflowing-sub implementation are absent; selected widened `Limb::sbb` differs. Count/division/Montgomery exclusions are operation/caller-specific, not package-wide. Real secret callers include P256 DTLS certificate signing and K256 signing/decomposition; the selected no-std/full_crypto graph is not emitted-Wasm reachability proof.

Pinned Rust1.94.1/LLVM21.1.8 arithmetic-only compilation finds an older zero-mask-to-branch lowering in `Uint::neg_mod → CtChoice::if_true`, both in K256 scalar Neg and inlined secret `lincomb`. Native assembly `e3d7abc23b0ca6ce85f85ea3fed652d03ff6f34a512f5e379f43f5a36a23c7ce` has zero-dependent branches; corresponding Wasm scalar Neg uses `br_if`. This is not proof of the newer PR1358 implementation, practical exploitability or final production-code behavior. Fixture profile is opt3/codegen1/panic-abort/no-LTO, unlike full-feature production/LTO. A supported narrow correction with applicable caller/codegen evidence, or matching production-profile evidence excluding the branch, remains required; functional tests or a guessed mask change cannot substitute.

The reproducibility input is retained only for that active acceptance consumer under `evidence/crypto-operations/`: manifest `11daedb3777fe5c1944a833ebad3d04131c17bf058e237f3ffca64498023fcda`, lock `161d1dad8aa48d7921b3090a818486a10fd0b09bbc83de1db0da8e3767f893cd`, source `cb945c4df5ad4f2f5981883658525db82262da64f7e623874fb0f9bfce2dd954`. Reproduce with pinned `cargo +1.94.1 rustc --offline --locked --manifest-path PATH/Cargo.toml --target-dir /tmp/NEW --release --target TARGET -p PKG -- --emit=asm,llvm-ir`, for x86_64-unknown-linux-gnu/wasm32-unknown-unknown and fixture/p256/k256 packages. This private fixture is not project build/CI infrastructure or a physical timing campaign. Assessment response SHA256 `0e44fc0f770f909a626011f6d10bddcc8698ff8ea042b52f77fed3886878f0ea` owns the finite inspection result.

**npm and licenses:** Retained lock fixes brace-expansion5.0.12, devalue5.9.3, source-map-js1.2.2 and Vite8.2.4/Rolldown1.2.4. Current npm has14 aggregates:4 high,2 moderate,8 low,0 critical. The four high aggregates remain only the reviewed deepmerge-ts7.1.5/write-package/PAPI CLI path with repository-owned configuration; no arbitrary-input or browser safety promise follows. ISC admission preserves Ring's Apache-2.0 AND ISC obligations and other bundled notices. Cargo Deny licenses/sources passes, not distribution packaging attestation. The single svelte-toolbelt0.10.6 license exception retains its archive integrity/MIT notice binding.

**Executable prerequisite recovered:** The previous stable2609-1 timeout remains historical. The stable2609-2 Linux bundle is installed, and a stable-source `01-download-binaries.sh --check` verifies all five pinned executables and the complete marker. Polkadot/Omni report1.25.2-9448fe4f252; bencher reports0.29.0. The acquisition Run itself exits2 after publication because its running Bash source was edited; the subsequent read-only verification, not that terminal result, establishes distribution identity. The resumable cache retains untrusted prefixes until full checksum verification; corrupt/oversized cached files require removal of only the reported file. No corrected DEOS Omni is selected and no compiler/worker/physical transfer is qualified.

**Current gate result:** The complete canonical shell route passes pinned tools/binaries, locked graphs and licenses, then its source-aware validation exits1 solely for crypto `0387`; that is not a passed dependency gate. No current production Wasm, Weight, metadata or worker is reissued here. Prior `96e55953…`, `5396bd5c…` and `320a2775…` execution receipts retain only their original scopes.

## 4. Invoice CID Source-Graph Review

The 2026-10-16 offline subset review covers the new direct `deos-runtime -> cid =0.11.3` edge, not a fresh whole-release dependency pass. `Cargo.lock` changes only the runtime dependency row; the selected registry package versions/checksums were already locked. Pinned Cargo/Rust 1.94.1 resolves the `wasm32v1-none` normal/build graph with only `cid/alloc`; native defaults additionally activate `cid/std`. Graph labels are not emitted-Wasm reachability or code-generation proof.

The CID branch contains 16 unique registry packages: ten on normal target paths and seven on host proc-macro paths, with `data-encoding` in both contexts. All declared license identities are admitted by this Skill's existing policy. Each archive SHA256 matches its selected lock record, and sorted per-file SHA256/path fingerprints match all 385 archived files against local extracted source, excluding only Cargo's generated `.cargo-ok` marker. This checks byte identity, not upstream signatures or vulnerability absence.

`cid::Cid` fixes digest capacity at 64 bytes. Its upstream parser accepts `/ipfs/` path aliases and can leave decoded trailing bytes; the runtime must retain its existing CIDv1 and canonical spelling equality checks. `l2_treasury_spend_rejects_noncanonical_invoice_identity` now demonstrates upstream acceptance of path/trailing aliases followed by runtime admission/execution rejection with unchanged root. Native tests use `cid/std`; no-std uses separate bounded varint readers and in-memory `multihash::no_std_io`. Their compiled runtime/target agreement remains part of coordinated Wasm acceptance.

Finite commands, package provenance and source identities live in `template/target/release-0727-cid-runtime-assurance/qualification.json` and its sibling receipts. No advisory refresh, exception renewal, bootstrap verification or Wasm generation occurred. At that subset review all 18 advisory/license exception rows still expired on 2026-09-30 and could not authorize release acceptance. Section 3 now owns the fresh whole-graph review and remaining blockers; this historical subset does not independently refresh it.

## 5. Bootstrap Identity

The Rust toolchain is exact rather than channel-floating. Node and npm are exact through `volta.node` and `packageManager`. Zombienet installs by exact package version and repository-recorded registry integrity; Chain Spec Builder, Cargo Audit, and Cargo Deny install by exact locked crate version; Try Runtime installs from immutable Git commit `6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be`. Existing commands are accepted only when their reported versions match.

The September review used SDK bundle `polkadot-stable2606-1`; `scripts/01-download-binaries.sh` owns the current selected release and platform-specific executable pins. Its `--check` verifies the release marker, every digest, executable bits and reported binary identities without downloading or replacing files. The offline CID subset does not refresh bootstrap evidence.

## 6. Falsification

Run `./.agents/skills/release-assurance/scripts/dependency-provenance.sh`. A pass proves only the current locked graphs, configured platforms, fetched advisory state, declared license policy, recorded exception horizon, and local checksum-pinned binary bundle. It does not prove an upstream package free of undisclosed vulnerabilities, certify downstream product dependencies, or replace the exact-tree signed release provenance gate.
