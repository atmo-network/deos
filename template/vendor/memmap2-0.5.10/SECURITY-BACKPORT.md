# Security backport proposal: memmap2 0.5.10

## Provenance and scope

- Original crates.io archive SHA256: `83faa42c0a078c393f6b29d5db232d8be22776a891f8f56e5284faee4a20b327`; archive VCS commit `17f01286eddfc5ccb11488f39ceceec7c9947f3c`.
- Adapted upstream pointer-range correction: `cee7cf03a9ee095982a3c37b7aac8e3f68f1a00c`, retained raw patch SHA256 `430a170472071cc449a921fd3b51ac1ca1d05b5125350645762d9f5fd9cc774f`.
- Four backend-entry checks reject `offset > self.len || len > self.len - offset` before pointer/alignment arithmetic: Unix flush, flush_async and advise; Windows flush_async, also used by flush. Each check returns InvalidInput without evaluating a potentially overflowing offset+len.
- This transplants the upstream flush invariant into the older backend without unrelated unsafe-lint, mapping or protection refactors. The same strict remaining-range check is deliberately used for advise, stronger than the upstream patch's initial separate offset/len tests. Old public range documentation already requires an in-bounds range.
- Public API, package version, libc/winapi requirements, mapping behavior for valid ranges and dual Apache-2.0/MIT licenses remain unchanged. RUSTSEC-2026-0186 officially remains patched only >=0.9.11; this is a local compatibility proposal, not an upstream-patched old release.

## Validation and limitations

- Existing 20 unit/17 doc tests plus one new regression covering overflowing and out-of-bounds offset/length, zero-length boundary, full and last-byte ranges and read-only advise pass in debug and optimized fat-LTO/one-CGU/unwind profiles under Rust1.94.1 on Linux x86_64.
- Actual parity-db0.4.12 sets index file length before mapping and flushes META_SIZE..map.len(); generic whole-map flush and direct libc madvise use valid mapped extents. This source fix avoids relying only on that caller-specific exclusion.
- Windows source receives the same bounds check but is not compiled/executed here. No macOS execution, full node/database workload, concurrent-file-truncation protection, sanitizer or independent approval is claimed.
- Parent must review the source adaptation, install exact source-aware/path projection and reconstruct final Native proof before acceptance.
