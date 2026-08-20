# Embedding `pallet-deos-staking`

This guide states the host-runtime obligations for embedding the reusable DEOS Staking package. Concrete DEOS asset ids, Actors, Fee Sink topology, collator composition, production parameters, and client realization belong to the reference runtime and integration documents.

## Package entrypoints

- [`specification.en.md`](./specification.en.md) owns intended staking, custody, `SecurityEpoch`, and reward semantics.
- [`architecture.en.md`](./architecture.en.md) maps the shipped package implementation, storage bounds, modules, and executable evidence.
- [`../src/lib.rs`](../src/lib.rs) is the package facade and sole FRAME storage, dispatchable, event, error, view-signature, and public SCALE-model owner.

## Required host bindings

A host runtime must provide the `Config` asset, currency, origin, identity, valuation, governance-coefficient, `NativeSecurityMode`, `SecurityEpoch`, compound, and Weight adapters. Every adapter must preserve the fail-closed contracts documented by the specification; unavailable canonical LP identity, valuation, operator eligibility, reward funding, or compound execution must return failure rather than substitute another asset or inferred state.

The host session owner must implement `SecurityEpochProvider` from its canonical session identity. Block cadence, maintenance progress, or an off-chain index must not redefine `SecurityEpoch`.

The host must select explicit finite values for participant, operator, nomination, claim-horizon, batch-claim, and unlock-delay bounds. Runtime `WeightInfo` must cover those configured bounds and mandatory session work. The `SubstrateWeight` and `()` implementations shipped in `src/weights.rs` are hand-written estimates rather than benchmark output; generate weights against your own runtime and bind those instead.

## Asset and custody obligations

The configured fungibles implementation must support exact receipt mint/burn and base/LP transfer semantics. `StakedAssetIdResolver` and `StakedAssetLifecycle` must preserve a collision-free receipt identity and live reverse lookup.

`NativeStakingLpAssetValidator` must accept only the host's canonical native/receipt LP asset. `NativeStakingReadModelProvider` must return bounded current truth and must not infer backing from transfer events or an indexer.

The native LP lock and reward accounts are deterministic pallet-owned custody. The host must preserve their ledger consequences, existential-deposit requirements, and transactional rollback behavior.

## Security and reward obligations

`NativeSecurityModeProvider` must expose one mode owner. `TrustedSet` preserves settlement and custody exits while rejecting creation of new LP-backed obligations; `LpBackedSelection` enables the bounded security path.

`SecurityRewardFundingOrigin` and `SecurityRewardFundingSource` must identify one certified funding path. Direct reward-account balance is uncredited custody and cannot create liability or claim rights.

Session integration must invoke bounded due settlement and the appropriate activation or Trusted contraction transition in the host's session boundary. Failures must cancel only an unactivated zero-credit plan and retain prior obligations for recovery.

## Evidence

Before accepting an embedding, run package tests, `try-runtime` invariants, benchmark compilation, host runtime integration tests, metadata comparison, and Clippy with warnings denied. Internal module organization must not move public SCALE models away from the package facade or duplicate FRAME storage ownership.
