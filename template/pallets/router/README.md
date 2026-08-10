# pallet-deos-router

`pallet-deos-router` (Rust crate `pallet_axial_router`) is the DEOS deterministic routing and swap pallet for the current TMCTOL route families.

## Scope

The current kernel/runtime slice provides:

- Exact-input `swap` plus pallet-facing exact-input and exact-output execution
- Direct XYK, direct TMC mint, and Native-anchored XYK routes bounded to two legs
- Per-leg directional reference validation and pre-execution observation publication
- Deterministic maximum-output or minimum-input route selection
- Router fee calculation and routing through a runtime adapter
- Canonical outcomes built from measured spend and recipient deltas
- Fee exemption for configured host accounts

## Key rule

The router is a **decision engine**, not a generic policy layer.
It chooses among bounded route families by maximum recipient output and uses the Native asset as the universal multi-hop anchor. Price-impact and fee fields remain informational quote metadata rather than route-selection inputs.

## Execution rule

Execution should remain trustless and economically honest:

- Execution prepares current state rather than trusting an earlier quote
- Every actual XYK leg validates and publishes before that leg executes
- Authored input/output bounds apply to measured committed facts
- Fees, market effects, observations, and success events share one transaction

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- Asset-conversion adapter
- TMC interface wiring
- Fee-routing adapter
- Directional price-observation adapter
- Admin origin, account topology, Native asset, LP-index bound, and fee bounds

## Non-goals of the current slice

The current kernel does not yet include:

- Arbitrary graph routing across unrestricted path lengths
- External DEX aggregation beyond configured in-runtime liquidity surfaces
- Governance policy over treasury deployment or bucket strategy
- Generalized intent settlement outside the bounded TMCTOL route families

See the [DEOS Router specification](./docs/specification.en.md) for intended semantics, the [package architecture](./docs/architecture.en.md) for shipped implementation truth, and the [embedding contract](./docs/embedding.md) for independent host obligations.
