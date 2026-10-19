# pallet-deos-asset-registry

`pallet-deos-asset-registry` (Rust crate `pallet_asset_registry`) is the DEOS foreign-asset identity and mapping pallet.

## SDK baseline

This pallet follows the DEOS workspace's pinned Polkadot SDK line; [`template/Cargo.toml`](../../Cargo.toml) owns the exact dependency selection.

## Scope

The current kernel/runtime slice provides:

- Governance-controlled foreign asset registration from XCM `Location`
- Deterministic foreign `AssetId` generation for new registrations
- Persistent bidirectional `Location <-> AssetId` storage as the runtime source of truth
- Manual registration with explicit `AssetId` override for collision handling
- Linking of pre-existing local assets to foreign `Location` keys
- Location-key migration for XCM version / encoding changes
- Optional runtime hook for token-domain bootstrap after registration

## Key rule

The stored mapping is canonical.
Deterministic hashing is used to derive an ID **only at registration time**; after that, runtime lookups must trust storage rather than re-hashing live XCM locations.

## Namespace rule

Foreign assets are an identity/namespace surface, not a policy engine:

- Registrations must land in the foreign namespace
- Collisions against live ledger assets or retained reverse identities must be rejected or explicitly resolved by governance
- Mapping creation and token-domain notification must commit atomically
- Location migration must preserve balances and identity
- Economic activation remains downstream runtime policy

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- `RegistryOrigin`
- `AssetIdGenerator`
- `AssetOwner`
- `TokenDomainHook`
- Runtime-specific weight bridge wiring

## Non-goals of the current slice

The current kernel does not yet include:

- Automatic creation of staking or liquidity policy for newly registered assets
- Treasury strategy decisions
- Router/zap/burning behavior beyond optional runtime glue hooks
- Broader orchestration of the full token lifecycle

See [`docs/architecture.en.md`](./docs/architecture.en.md) for the current contract.
