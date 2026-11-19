# Security backport proposal: lru 0.7.8

## Provenance and scope

- Original crates.io archive SHA256: `e999beba7b6e8345721bd280141ed958096a2e4abdf74f67ff4ce49b4b54e47a`; archive VCS commit `febcddb85a5a0cba917b896229a9ae68cfaa98ca`.
- Adapted upstream pop correction: `2776ded569ee89a99c515bca8194f65639182c96`, retained as `../../upstream/lru-pop-2776ded.patch`, SHA256 `ed75f57d04cd77a3cdc987cf164edd5ab229b9af8454b7fe304a290d0e432535`.
- This older implementation owns a Box directly. The only production-source change moves the existing node detach before dropping the stored key. Public API, package version, feature declarations, hashbrown dependency and MIT license remain unchanged.
- Official RUSTSEC-2026-0253 patched range remains >=0.18.2. This is a local compatibility backport proposal, not an upstream-patched 0.7 release or an approved exception. RUSTSEC-2026-0002 explicitly excludes <0.9; its fix is not transplanted here.

## Validation and limitations

- Existing 34 unit/24 doc tests plus one new panic/unwind/iteration/eviction test pass in debug and optimized fat-LTO/one-CGU/unwind profiles under Rust1.94.1.
- The archived optional auto-trait syntax emits a pre-existing future-incompatibility warning even with the feature disabled. No feature removal or warning suppression is included.
- Actual selected tracing-log0.2.0 uses Copy integer keys and u64 values; its current caller already excludes the panicking-key prerequisite. This small compatible source correction removes reliance on that particular exclusion for pop.
- No Miri, sanitizer, macOS/Windows execution, full node build or independent source approval is claimed. Parent must review the source diff, install exact source-aware/path projection and reconstruct final Native proof.
