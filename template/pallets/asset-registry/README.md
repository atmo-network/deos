# pallet-asset-registry

`pallet-asset-registry` is the DEOS foreign-asset identity and mapping pallet.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2603 / node 1.22.0` line.
The 2603 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/XCM integration surfaces rather than in `pallet-asset-registry` core logic.

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
- Collisions must be rejected or explicitly resolved by governance
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

See `docs/asset-registry.architecture.en.md` for the current contract.
