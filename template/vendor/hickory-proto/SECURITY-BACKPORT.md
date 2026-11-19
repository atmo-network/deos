# Hickory Wire Security Backport

## Source Identity

- Base: crates.io `hickory-proto 0.24.4`, archive SHA256 `92652067c9ce6f66ce53cc38d1169daa36e6e7eb7dd3b63b5103bd9d97117248`, upstream commit `b00cc1052cf769250063005e9fa268519522778f`, package path `crates/proto`.
- Preserve upstream Apache-2.0 OR MIT licensing and notices. This is a locally modified source package, not an upstream fixed-version release.
- Compression fix: <https://github.com/hickory-dns/hickory-dns/pull/3613>, specifically commits `c7c366bffc742b10d3608b694078dc2fe38e7491`, `560f7d6ab57d41c83222be5dbdf5584d43841c30`, and `21208ce33a92c7eb9b1f0a947ce361f05b14caf4`. Retrieved combined patch SHA256 `a07a24aff968e688b6841db416f83524820bc5c9fbec8644d5776103cb20030c`.
- Allocation fix: <https://github.com/hickory-dns/hickory-dns/commit/280c0e5c4e3002d73ba0762835ef82272d1d33a6>, retrieved patch SHA256 `1f5c2a42060a3eda3e97477b834f735229b9f1064067805a7c9dc80479e89038`.

## Changes and Compatibility

- `RUSTSEC-2026-0119`: Bound stored compression candidates to the upstream 64 and attempted compressed names to the upstream 120. Adapt the newer `NameEncoding` policy to the 0.24 `canonical` flag; canonical names do not search or consume compression attempts, but can seed bounded pointers for later eligible names. These are upstream security limits, not measured runtime prices.
- `RUSTSEC-2026-0491`: Reserve at most the upstream eight entries for peer-controlled question and record counts; grow only after successful decoding. Route the duplicate question decoder through `read_queries` so both public and complete-message decoding receive the fix. All four header sections retain their full decoding/error semantics.
- Public function signatures, package version, dependencies and features consumed by the reference workspace remain unchanged. Large messages can become less compressed and reach an existing wire-size limit earlier, as with the upstream mitigation; no universal byte-for-byte encoding promise is made.
- The standalone manifest adds an isolated workspace and a regression test target. Remove the unconditional duplicate OpenSSL dev dependency: its optional normal dependency remains available to OpenSSL-feature tests. This permits default wire tests without system OpenSSL; it does not qualify those optional tests. The standalone lock is test-only and does not select production workspace dependencies.

## Validation

From the repository root:

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true ./scripts/ci-local.sh --only tests --manifest-path template/vendor/hickory-proto/Cargo.toml --package hickory-proto
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true ./scripts/ci-local.sh --only check --package libp2p-dns
```

The default standalone suite covers the 64-candidate boundary, 120-attempt boundary and wire roundtrip, canonical seeding, all four malformed header-count sections, direct question/record readers, and valid growth beyond eight entries. Allocation tests compare the largest actual allocation for advertised counts9/65535 with count8, not an invented byte ceiling. Parent checks use the reference workspace lock; standalone tests use their own locked graph. Neither establishes emitted-Wasm, external node-binary or worker qualification.

## Advisory Limits

The 2026-11-12 RustSec database `af7b3ec33460b8e2de8d422e16d628518891a519` also lists DNSSEC findings `RUSTSEC-2026-0505`, `0511`, `0512` and resolver `0514`. They are not fixed here. The reference workspace all-feature/all-target feature graph has no Hickory `dnssec`; `src/rr/mod.rs` and `src/xfer/mod.rs` gate the affected validation modules. Resolver0.24.4 gates construction of the DNSSEC handle in `async_resolver.rs`. Enabling those features requires separate remediation/review. This is a source-prerequisite observation, not an installed exception or package-wide security guarantee.

Cargo Audit does not analyze these local path sources. Disappearance of their registry findings is not proof of remediation, and must not discard the independent review of text-parser or DNSSEC exclusions. Review this source delta and its exact upstream identities before acceptance; global dependency assurance and exception decisions remain with the release-assurance owner.
