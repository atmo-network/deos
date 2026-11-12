# Dependency Provenance Review

## 1. Scope and Owners

This Skill reference records how release assurance reviews input identity, licenses, checksums, and advisories without becoming a project validation dependency. `template/Cargo.lock` and `web-client/package-lock.json` own resolved dependency identity; `template/rust-toolchain.toml` and `web-client/package.json` own compiler and Node/npm identity; project bootstrap scripts own executable pins used by operators and builds.

`config/deny.toml` owns this Skill's Rust license and registry policy. `config/dependency-provenance-exceptions.json` owns its dated advisory and missing-license decisions. `scripts/dependency-provenance.sh` is the private deterministic review entrypoint; no project script or workflow invokes it.

## 2. Fail-Closed Review Contract

The review rejects an unlocked Cargo or npm graph, a mismatched Rust/Node/npm or audit-tool version, a missing or checksum-mismatched Polkadot SDK executable, an unknown Cargo registry, an unapproved dependency license, a registry npm package without lockfile integrity, an unreviewed Rust vulnerability or unsoundness advisory, and an unreviewed npm high or critical finding.

Every retained material finding states runtime reachability, rationale, and an expiry no more than 90 days after review. New, stale, duplicate, expired, or fixed-but-retained exceptions fail. npm critical findings fail even when listed. Low npm findings and RustSec maintenance notices remain visible in raw tool output but do not bypass the material vulnerability gate.

## 3. Current Dependency Qualification — Blocked

The 2026-11-12 review starts from committed source tree `4b5291b55d889b4ef403cf335bd7f011e6c8e3a0` and evaluates the retained security-lock changes below. Cargo Audit0.22.2, Cargo Deny0.20.2, Rust1.94.1, Node22.22.0 and npm11.7.0 are pinned. RustSec is `fd6f39814c4d3c359fdbe223d0280dec4933f78f`, updated2026-11-11, with1,458 advisories. No advisory result, graph label or native check establishes emitted-Wasm, worker or physical applicability.

| Current input | SHA256 |
| --- | --- |
| Cargo lock | `320a277502b357c33e82dd25ee9e35dbb9a4d51713cb9f31adb6d1776a8bf187` |
| npm lock | `f69b59d58479febae674d2d1f921a94860c5973f15303f49f406353d030df4b0` |
| Current Cargo Audit JSON | `854187ef0153113c431d02e2c4d9b9fd0f19050b7b02acf74658dc44f5d2dc20` |
| Current npm Audit JSON | `3b9c5115f21576a514f16e80679427a7eb30b4018f3c06db92fb27842d2ab3a6` |
| Current all-target Hickory feature proof | `351d88e8efa2cbc4f502becd119723ecd0597a6c60f0aebfb8d84b76b2e39be5` |
| Current all-target zstd6 feature proof | `1f406219cd20266ae7e4e1db8b7554a1f825f09a25d944415bc57022096848fc` |

**Completed compatible remediation:** The Cargo graph retains the SDK package line/local patches and changes36 security-package versions without package additions/removals. Wasmtime36.0.20 and its coupled16-package family use Cranelift0.123.20 across13 packages. Remaining updates are Rustls0.23.46, WebPKI0.103.16, Tokio1.53.3, rustix1.1.6, zstd0.13.4, zstd-safe7.4.0 and zstd-sys2.1.2. Tokio is a compatible1.x minor update, not a patch-only change. Twenty-three incidental dependency reselections in18 unchanged parents are restored to their baseline choices; complete locked Cargo metadata accepts that minimized selection. No generated checksums or SDK patch sources change.

The npm lock alone updates brace-expansion5.0.12, devalue5.9.3, source-map-js1.2.2 and Vite8.2.4, with required Rolldown1.2.4/platform bindings and OXC types0.144.0. Manifest ranges, minimatch10.2.5 override, package identity and metadata remain unchanged. Clean install and complete client validation pass with195 automation,9 Governance and6 Wiki tests, zero Svelte diagnostics and a successful static build; these are client preparation, not runtime qualification.

| Material coordinate | Before remediation | Retained graph |
| --- | --- | --- |
| Cargo affected-version records / unique IDs | 65 / 52 | 39 / 36 |
| Cargo unique scope | One target-normal,36 native/host,15 lock-only | One target-normal,19 native/host,16 lock-only |
| npm aggregate high / moderate / low / critical | 8 / 2 / 8 / 0 | 4 / 2 / 8 / 0 |
| Unexcepted material findings | 40 | Four Rust IDs below |

**Unresolved, not excepted:**

| RustSec ID | Selected source and exact constraint |
| --- | --- |
| `RUSTSEC-2026-0387` | crypto-bigint0.5.5 through elliptic-curve0.13.8/k2560.13.4 is target-normal. Fixed bigint≥0.7.6 crosses parent/API lines. Selected sc-network0.60.0 also contains secret-derived P-256 DTLS certificate signing in `webrtc.rs:64–145`. No whole-runtime/PVF secret-free proof is established. |
| `RUSTSEC-2026-0119` | Hickory0.24.4 wire name-compression CPU exhaustion is not excluded by disabled text parsing; fixed≥0.26.1 is outside the selected libp2p parent requirement. |
| `RUSTSEC-2026-0491` | Hickory0.24.4 wire header-count allocation remains selected; fixed≥0.26.2 crosses its parent API line. |
| `RUSTSEC-2026-0381` | libp2p-websocket0.44 requires webpki-roots0.25, while fixed roots require≥1.0.10. Fixed WebPKI is not a fix for the separate roots finding. |

These constraints require supported parent/API remediation, a reviewed backport, or a genuinely evidenced narrower call-domain exclusion; they do not prove that a whole-SDK migration is the only remedy. No unsupported crypto, DNS or trust-root exception is installed.

**Installed bounded decisions:** The ledger contains36 material exceptions and one npm license exception, reviewed2026-11-12 through2026-12-12:16 lock-only Rust IDs,15 source-prerequisite exclusions, one native-observability risk acceptance and four trusted-generation npm aggregates. A classification is a reviewed claim, not an automatic proof. Re-review consumed source, features, callers, graph or advisory changes; no critical npm finding may be excepted.

| Source-prerequisite exclusions | Actual exclusion and limit |
| --- | --- |
| Hickory `0487/0489/0490` | `hickory-proto0.24.4/src/serialize/mod.rs:20–22` gates the affected zone/text parser behind absent `text-parsing`. Does not cover the two wire findings. |
| zstd-safe6 `0395/0399/0402/0404` | Affected context/dictionary/cloning APIs require absent `experimental`; these are6.0.6 exclusions, not a package-wide guarantee. |
| zstd-safe6 `0401` | `train_from_buffer` requires absent `zdict_builder`. |
| zstd-safe6 `0406` | Absent sys `zstdmt` excludes the unfinished multithreaded-frame prerequisite; the `NbWorkers` Rust API itself remains available. |
| zstd-safe6 `0403` | Legacy support is absent; independently, `sp-maybe-compressed-blob11.1.0` uses the advisory-excluded read decoder. |
| zstd-safe6 `0397/0400` | Selected zstd0.12.4 wrappers use `OutBuffer::around`, not `around_pos`; SDK blob operations create private fresh stream contexts and do not interleave one-shot dictionary operations. New callers/context sharing invalidate the exclusion. |
| lru `0253` | Selected0.7.8 tracing-log keys are Copy integer fields; selected0.12.5 PeerId/Multiaddr keys have no panicking custom destructor. The advisory requires that destructor panic/unwind/recovery. Alternative0.16.4 is absent. |
| lru `0002` | Selected PeerAddresses uses shared `iter()`, not affected `IterMut`; Identify delegates to that cache. |
| memmap2 `0186` | Sole selected0.5.10 consumer parity-db0.4.13 calls `flush_range(META_SIZE, map.len()-META_SIZE)` after successful size/mapping checks. This is not a guarantee for arbitrary mmap APIs, malformed databases generally or externally truncated files. |

IDs abbreviated in the table have prefix `RUSTSEC-2026-`. Independent review response SHA256 `c960930b9845ce774590b7036f28bfebf198acfe3a6e6181e3be8c399149723c` accepts those15 exclusions and additional lock-only WebPKI`0382`, within their source/version/feature limits. The selected WebPKI0.103.16 is fixed; affected0.101.7 remains absent. All16 lock-only identities are retained in the single ledger rather than repeated here.

The existing `RUSTSEC-2025-0055` decision accepts bounded native terminal/log-consumer risk, not absence of vulnerability. sc-tracing48 pins tracing-subscriber exactly0.3.19; a normal patch update is rejected. Untrusted logs must not be treated as commands or authoritative evidence. The four npm high aggregates remain exclusively `GHSA-ggr8-5vv4-36mx` through deepmerge-ts7.1.5/write-package/PAPI CLI. Reviewed generator source supplies repository-owned package/config objects, not attacker-controlled recursive graphs; browser imports do not invoke that CLI path. A fixed deepmerge major8 requires parent compatibility work.

**License decision:** ISC is admitted after inspecting the selected bundled permission/notice terms. Ring requires Apache-2.0 AND ISC, not an either/or choice; forwarded-header-value, WebPKI and untrusted use ISC. Cargo Deny licenses/sources now passes; this is not distribution-notice packaging attestation. svelte-toolbelt0.10.6's missing manifest field remains excepted against its integrity-matching archive and bundled MIT LICENSE SHA256 `8914f144af4333312a07b30438dc6b75dd8da7a665c3ae9fa751f229eb615e8b`.

**Tool boundary:** The existing private JSON validator recognizes `source-prerequisite-excluded` without misclassifying selected code as lock-only or observability. Its isolated self-test exercises the actual CLI's accepted case, unknown classification, unreviewed finding, and critical npm refusal even when the aggregate counter disagrees. The canonical Bash route invokes that self-test. Actual current audit inputs still exit1 on exactly the four unexcepted Rust IDs; no expired/stale decision or critical suppression is accepted.

**Bootstrap remains blocked:** The canonical shell review stops at missing `bin/.polkadot-sdk-release`; only bencher and two workers are present. One authorized `01-download-binaries.sh` restoration attempt reaches its600s deadline while downloading the pinned Polkadot asset; it publishes no complete marker and is not retried unchanged. Supply the actual checksum-pinned complete bundle through that owner before rerunning the composed shell gate. Distribution identity alone still does not qualify corrected Omni, current-graph worker execution or physical resource transfer.

Independent final integration response SHA256 `595bb502f16a0d461482a1158ced36966f97112dc011d31b35d192e16a94d5c5` accepts the minimized graph, installed dispositions and negative-test behavior. It correctly rejects the first no-fetch capture because Cargo Audit0.22.2 omits its database identity. Recollection through ordinary `cargo audit --json` supplies the verified identity without modifying JSON fields; its byte-identical audit digest is the identified response above. The helper now reports exactly the four material blockers, not a successful dependency gate.

The final raw inputs are `/tmp/deos-security-audit-identified-final.json`, `/tmp/deos-client-audit-after.json` and the two `/tmp/deos-security-*-minimal-features.txt` files for the active acceptance consumer. Older `96e55953…` and intermediate `5396bd5c…` lock receipts retain only their original scopes. No production Wasm, generated Weight or worker binary is reissued by these source/dependency checks.

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
