# Vendored packages

This directory holds published crates.io packages with narrow local corrections. DEOS does not fork a package for features or convenience. A package lands here only when the selected Polkadot SDK line or one of its dependents pins a version that upstream will not fix, and the fix cannot reach DEOS through a compatible lock update.

Every package keeps its published name and version, so dependents and `Cargo.lock` still see the original identity. Cargo `[patch.crates-io]` swaps in the local source. Each package directory carries its own note with the source archive identity, the exact change, the advisory or defect it closes, its validation and its limits. This index is the map. The notes stay authoritative for detail.

## Who patches what

Two independent Cargo workspaces consume this directory: the runtime workspace (`template/Cargo.toml`) and the isolated PVF validation host (`template/tools/pvf-validation/Cargo.toml`). Each lists its own `[patch.crates-io]` entries.

| Package | Runtime workspace | PVF host | Pulled in by | Note |
| --- | :---: | :---: | --- | --- |
| `pallet-assets` 54.1.0 | ✓ | — | `parachains-common` / SDK umbrella → runtime | [`DEOS-BACKPORT.md`](pallet-assets/DEOS-BACKPORT.md) |
| `sp-api-proc-macro` 29.0.0 | ✓ | — | SDK umbrella → runtime API generation | [`README.md`](sp-api-proc-macro/README.md) |
| `cumulus-pallet-xcmp-queue` 0.35.0 | ✓ | — | SDK umbrella → runtime XCMP queue | [`README.md`](cumulus-pallet-xcmp-queue/README.md) |
| `crypto-bigint` 0.5.5 | ✓ | ✓ | `elliptic-curve` 0.13 → `k256` / `ecdsa` (and `p256` in the host) | [`SECURITY-BACKPORT.md`](crypto-bigint/SECURITY-BACKPORT.md) |
| `hickory-proto` 0.24.4 | lock only | ✓ | `hickory-resolver` 0.24 → `libp2p-dns`, `libp2p-mdns` | [`SECURITY-BACKPORT.md`](hickory-proto/SECURITY-BACKPORT.md) |
| `webpki-roots` 0.25.4 | lock only | ✓ | `libp2p-websocket` → `sc-network`, `sc-telemetry` | [`SECURITY-BACKPORT.md`](webpki-roots/SECURITY-BACKPORT.md) |
| `hickory-proto` 0.26.2 (`hickory-proto-026/`) | — | ✓ | `hickory-net` / `hickory-resolver` 0.26 → `litep2p` | [`SECURITY-BACKPORT.md`](hickory-proto-026/SECURITY-BACKPORT.md) |
| `lru` 0.7.8 (`lru-0.7.8/`) | — | ✓ | `tracing-log` → `sc-tracing` → PVF workers | [`SECURITY-BACKPORT.md`](lru-0.7.8/SECURITY-BACKPORT.md) |
| `lru` 0.12.3 (`lru-0.12.3/`) | — | ✓ | `libp2p-identify` → `sc-network` | [`SECURITY-BACKPORT.md`](lru-0.12.3/SECURITY-BACKPORT.md) |
| `memmap2` 0.5.10 (`memmap2-0.5.10/`) | — | ✓ | `parity-db` → `sc-client-db` | [`SECURITY-BACKPORT.md`](memmap2-0.5.10/SECURITY-BACKPORT.md) |
| `str0m-openssl` 0.6.0 | — | ✓ | `str0m` → `litep2p` → `sc-network` | [`SECURITY-BACKPORT.md`](str0m-openssl/SECURITY-BACKPORT.md) |

"Lock only" means the package appears in the runtime `Cargo.lock` but in no runtime-workspace build graph for any target. The patch keeps that lock entry on the corrected source, so audits of the lock see the fixed package.

The host additionally patches `tracing-subscriber` 0.3.19 from its own [`tools/pvf-validation/vendor/`](../tools/pvf-validation/vendor/tracing-subscriber-0.3.19/SECURITY-BACKPORT.md), because only the host graph contains it.

## What each correction is for

- **SDK behavior corrections.** These packages are used only by the runtime workspace.
  - `pallet-assets`: `transfer_all` credits already-issued units without demanding fresh issuance capacity, and zero mint or burn still requires the issuer or admin.
  - `sp-api-proc-macro`: restores the client transaction-depth counter after explicit commit or rollback inside `execute_in_transaction`.
  - `cumulus-pallet-xcmp-queue`: pays for decoder and page work up front, before admitting it. It also backports upstream PR #13816, which checks outbound channel capacity before marking a notification as sent.
- **Security backports to old version lines.** Dependents pin these lines, so the published fixed versions are unreachable.
  - `crypto-bigint`: RUSTSEC-2026-0387.
  - `hickory-proto` 0.24: RUSTSEC-2026-0119 and RUSTSEC-2026-0491.
  - `hickory-proto` 0.26: RUSTSEC-2026-0491.
  - `webpki-roots`: RUSTSEC-2026-0381, removing a distrusted root.
  - `lru` 0.7.8: RUSTSEC-2026-0253.
  - `lru` 0.12.3: RUSTSEC-2026-0253 and RUSTSEC-2026-0002.
  - `memmap2`: RUSTSEC-2026-0186.
  - `str0m-openssl`: corrects the length reported by its DTLS read.

Several notes describe themselves as local compatibility backport proposals: the official advisory range still marks the version affected. Release-assurance records whether each finding is fixed by source, absent from a role's build graph, or excluded by caller analysis. Its exception ledgers own that decision, not this index.

The `str0m-openssl` note says that there is "no root Cargo patch … or consumer selection". That describes the runtime workspace. The PVF host does patch it.

## Maintenance rules

- Change a package only together with its note, and keep the published version number.
- Release-assurance pins the complete file inventory of every package directory here, notes included. Any edit inside a package directory, even to prose, therefore needs re-review of the affected source proofs. This index sits outside the package directories for that reason.
- Retire a package when a compatible upstream release carries the fix. Remove it from every `[patch.crates-io]` table that lists it, delete the directory, and update this table.
- Verify a consumer chain with `cargo tree --locked --offline -i <name>@<version> -e normal,build`, run from the owning workspace.
