# Dependency Provenance Review

## 1. Scope and Owners

This Skill reference records how release assurance reviews input identity, licenses, checksums, and advisories without becoming a project validation dependency. `template/Cargo.lock` and `web-client/package-lock.json` own resolved dependency identity; `template/rust-toolchain.toml` and `web-client/package.json` own compiler and Node/npm identity; project bootstrap scripts own executable pins used by operators and builds.

`config/deny.toml` owns this Skill's Rust license and registry policy. `config/dependency-provenance-exceptions.json` owns its dated advisory and missing-license decisions. `scripts/dependency-provenance.sh` is the private deterministic review entrypoint; no project script or workflow invokes it.

## 2. Fail-Closed Review Contract

The review rejects an unlocked Cargo or npm graph, a mismatched Rust/Node/npm or audit-tool version, a missing or checksum-mismatched Polkadot SDK executable, an unknown Cargo registry, an unapproved dependency license, a registry npm package without lockfile integrity, an unreviewed Rust vulnerability or unsoundness advisory, and an unreviewed npm high or critical finding.

Every retained material finding states runtime reachability, rationale, and an expiry no more than 90 days after review. New, stale, duplicate, expired, or fixed-but-retained exceptions fail. npm critical findings fail even when listed. Low npm findings and RustSec maintenance notices remain visible in raw tool output but do not bypass the material vulnerability gate.

## 3. Current Release Evidence

The 2026-09-05 review used Cargo Audit `0.22.2`, Cargo Deny `0.20.2`, RustSec database commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`, Node `22.22.0`, and npm `11.7.0`. It includes the runtime's dev-only Wasm replay dependencies. Cargo metadata resolved with `--locked`, npm's lock graph reproduced with `npm ci --ignore-scripts --dry-run`, Cargo licenses and sources passed, and every npm registry package carried lockfile integrity.

The review retained 17 unique material finding identities: 13 RustSec vulnerability/unsoundness IDs and four npm high aggregate package entries. No npm critical finding existed. The active h2 line remains `0.4.16`; the native Wasmtime toolchain and coupled Pulley/Winch/Cranelift family were advanced to Wasmtime `36.0.14` for `RUSTSEC-2026-0269`, without adding an advisory exception. The affected WASI filesystem capability is absent from the resolved DEOS graph; the patch nevertheless removes the affected Wasmtime version. `template/Cargo.lock` owns the exact patched identities.

No retained RustSec finding reaches the no-std consensus runtime graph. Twelve IDs occur only in lockfile alternatives absent from the all-feature, all-target workspace graph. `RUSTSEC-2025-0055` reaches native Polkadot SDK tracing only, is excluded from runtime Wasm, and cannot be patched independently because `sc-tracing` pins `tracing-subscriber = 0.3.19` exactly.

The npm high aggregate resolves to `GHSA-ggr8-5vv4-36mx` in `deepmerge-ts` through the descriptor-generation CLI. Exploitation requires a recursive attacker-controlled object graph; DEOS invokes that path only with locally exported runtime metadata and repository-owned generation configuration, and the vulnerable merge package is not called by browser runtime flows. The exception expires on 2026-09-30 and must be removed or renewed against fresh upstream evidence.

The npm graph has one missing upstream license declaration: `svelte-toolbelt 0.10.6`. Its pinned upstream source carries an MIT license, and the time-bounded exception requires re-verification or removal when that package identity changes. Generated first-party descriptors are exempt from third-party license-field requirements.

## 4. Bootstrap Identity

The Rust toolchain is exact rather than channel-floating. Node and npm are exact through `volta.node` and `packageManager`. Zombienet installs by exact package version and repository-recorded registry integrity; Chain Spec Builder, Cargo Audit, and Cargo Deny install by exact locked crate version; Try Runtime installs from immutable Git commit `6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be`. Existing commands are accepted only when their reported versions match.

The Polkadot SDK binary bundle is fixed to release `polkadot-stable2606-1`. Each supported platform has one repository-recorded SHA-256 digest per executable, and `scripts/01-download-binaries.sh --check` verifies the release marker, every digest, executable bits, and reported binary identities without downloading or replacing files.

## 5. Falsification

Run `./.agents/skills/release-assurance/scripts/dependency-provenance.sh`. A pass proves only the current locked graphs, configured platforms, fetched advisory state, declared license policy, recorded exception horizon, and local checksum-pinned binary bundle. It does not prove an upstream package free of undisclosed vulnerabilities, certify downstream product dependencies, or replace the exact-tree signed release provenance gate.
