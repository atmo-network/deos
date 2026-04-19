# pallet-axial-router

`pallet-axial-router` is the DEOS deterministic routing and swap pallet for the current TMCTOL route families.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2603 / node 1.22.0` line.
The 2603 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/XCM/session integration surfaces rather than in `pallet-axial-router` core logic.

## Scope

The current kernel/runtime slice provides:

- User-facing `swap` execution and pallet-facing `execute_swap_for(...)`
- Route comparison across direct XYK, direct TMC mint, and Native-anchored multi-hop paths
- Oracle-aware pricing and pre-swap EMA updates
- Deterministic route selection through efficiency scoring
- Router fee calculation and routing through a runtime adapter
- Tracked-asset management for oracle monitoring
- Fee exemption for designated system accounts

## Key rule

The router is a **decision engine**, not a generic policy layer.
It chooses among bounded route families using deterministic price/impact comparisons and the Native asset as the universal anchor.

## Execution rule

Execution should remain trustless and economically honest:

- Oracle state updates happen before execution
- Route selection is deterministic from runtime-visible liquidity inputs
- Fees are applied through the configured fee-routing adapter
- System account flows avoid recursive self-taxation

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- Asset-conversion adapter
- TMC interface wiring
- Fee-routing adapter
- Oracle parameters and tracked-asset limits
- Admin origin, Native asset, and router fee defaults

## Non-goals of the current slice

The current kernel does not yet include:

- Arbitrary graph routing across unrestricted path lengths
- External DEX aggregation beyond configured in-runtime liquidity surfaces
- Governance policy over treasury deployment or bucket strategy
- Generalized intent settlement outside the bounded TMCTOL route families

See `docs/axial-router.architecture.en.md` for the current contract.
