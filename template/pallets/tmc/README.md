# pallet-tmc

`pallet-tmc` is the DEOS unidirectional minting-curve pallet implementing the TMCTOL standard's minting primitive.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2606 / node 1.24.0` line.
The 2606 upgrade did not require further pallet-local semantic changes here; the previous 2603 `RuntimeDebug` cleanup remains in place, while the current compatibility fallout landed in runtime/parachain-system/asset-conversion integration surfaces.

## Scope

The current kernel/runtime slice provides:

- Governance-controlled curve creation
- No post-creation curve-parameter mutation in the current launch line, while forks may widen that surface deliberately
- Live curve lookup per minted asset
- Deterministic spot-price and integral-based mint calculations
- `mint_with_distribution(...)` with user/zap split routing
- Live effective-supply tracking through current native issuance
- Runtime glue hook on curve creation for downstream token-domain activation
- Runtime-configured mint output resolution

## Key rule

TMC is a **unidirectional minting engine**.
It can mint according to the configured curve, but it does not implement a reverse redemption path that extracts reserve value back out of the curve.

## Pricing rule

Mint amounts must be justified by the curve integral, not by ad hoc quoting:

- Current price comes from the configured linear ceiling
- Effective supply is read from live issuance minus initial issuance
- Mint output is solved from the integral with overflow-safe intermediate arithmetic
- Downstream distribution happens after mint amount is fixed

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- Initial price, slope, and precision defaults
- User allocation ratio
- Treasury and mint-output resolver accounts
- Runtime glue hook for token-domain activation
- Admin origin and weight bridge wiring

## Non-goals of the current slice

The current kernel does not yet include:

- Reverse selling / redemption into curve reserves
- Liquidity provisioning logic inside the pallet itself
- Router path selection
- Broader treasury policy beyond the configured post-mint distribution split

See `docs/tmc.architecture.en.md` for the current contract.
