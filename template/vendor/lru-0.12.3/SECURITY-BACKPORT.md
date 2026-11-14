# Security backport proposal: lru 0.12.3

## Provenance and scope

- Original crates.io archive SHA256: `d3262e75e648fce39813cb56ac41f3c3e3f65217ebf3844d818d1f9398cfb0dc`; archive VCS commit `339020e6f61933687a724110f8c103d346902601`.
- Upstream pop correction: `2776ded569ee89a99c515bca8194f65639182c96`, raw patch SHA256 `ed75f57d04cd77a3cdc987cf164edd5ab229b9af8454b7fe304a290d0e432535`.
- Upstream IterMut correction: `b9bca3492d75139097df3b018b6abdf5825ee868`, raw patch SHA256 `f2d77bd3206d65e3d76f05d89e6119684662ecbde4abb0de4521ff3398caaa0b`.
- The production-source changes detach the node before Box reclamation/key Drop and use shared key references in both mutable-iterator directions. The latter two lines match the upstream correction. Public API, package version, default/hashbrown features, dependency and MIT license remain unchanged.
- Official patched ranges remain >=0.18.2 for RUSTSEC-2026-0253 and >=0.16.3 for RUSTSEC-2026-0002. This is a local compatibility proposal, not an upstream-patched 0.12 release or an approved exception.

## Validation and limitations

- Existing 40 unit/33 doc tests plus two new panic/unwind/eviction and bidirectional iteration/key-lookup tests pass in debug and optimized fat-LTO/one-CGU/unwind profiles under Rust1.94.1.
- Actual selected libp2p-identify0.45.0 delegates to libp2p-swarm0.45.1 PeerAddresses. That cache uses PeerId (Copy) and Multiaddr (Arc<Vec<u8>>) keys, no custom panicking key Drop, and shared iter rather than IterMut. The source changes avoid requiring these caller exclusions for the two corrected operations.
- Native tests do not detect Stacked Borrows violations by themselves; source correspondence supplies that correction evidence. No Miri/sanitizer result, other-target execution, full node build or independent approval is claimed.
- Parent must review the complete diff, install exact source-aware/path projection and reconstruct final Native proof before acceptance.
