# Asset Conversion Integration

## Status and Ownership

This document owns only concrete DEOS composition of semantic asset identity, physical ledgers, the upstream XYK engine, public Router control, pool lifecycle, LP reverse identity, Oracle topology, and fresh-genesis validation. Reusable Router semantics belong to `template/pallets/router/docs/specification.en.md`; shipped package behavior belongs to `template/pallets/router/docs/architecture.en.md`.

## Identity Boundary

`AssetKind` is semantic/domain identity. `LedgerAssetKey` is physical ledger identity:

```text
ledger_key(Native)     = Native
ledger_key(Local(x))   = Assets(x)
ledger_key(Foreign(x)) = Assets(x)
```

Canonical admission permits `Local(id)` only outside the `TYPE_FOREIGN` namespace and `Foreign(id)` only inside it. The implicit `u32 -> AssetKind` conversion is removed. Market pairs require canonical endpoints and distinct ledger keys. The shared primitive is consumed by DEOS Router, TMC, the runtime pool locator, Oracle pool admission, and topology integrity.

## Underlying XYK Engine

`pallet-asset-conversion` remains the underlying pool, liquidity, quote, reserve, and swap engine. Asset Conversion `Pools<Runtime>` is authoritative pool existence. DEOS does not infer existence from a quote, `PoolEmpty`, an Oracle feed, or an LP binding.

The selected SDK graph resolves upstream `pallet-asset-conversion 32.0.0` from crates.io, with no local override. It includes the full-balance reserve correction from [Polkadot SDK PR #12408](https://github.com/paritytech/polkadot-sdk/pull/12408), merge `408895c27a5aff4bac99e956df5426983566f8cb`. The unused 31.0.0 vendor copy is removed; Cargo resolves exactly one implementation. Runtime upgrade/caller acceptance remains owned by the [SDK dependency line](./actors.integration.en.md#sdk-dependency-line), not implied by this retirement.

The retired backport changed only reserve reads to full `Assets::balance` and routed liquidity withdrawal plus exact-input/exact-output quote reserves through `get_reserves`. Its corrected `src/lib.rs` matched the upstream post-merge file. The original archive and source identities below preserve its provenance without retaining another buildable implementation.

| Historical input | SHA-256 |
| --- | --- |
| Crates.io 31.0.0 archive | `9372e01886c4a9b625929c2549ac50d67cfe0f8e4daf232d884aecc843fe2557` |
| Original `src/lib.rs` | `060af0ccea54c6d970a67b6f65af94c952d995f22885ed27861f9095ec54a010` |
| Corrected upstream/vendor `src/lib.rs` | `cc2cd2d15e190d03b19b564c529ee06f0fe6987979c2c08fd532ca40d734c2f0` |

Quote, liquidity withdrawal, and execution reserve calculations use full physical pool balances. Withdrawability remains a separate execution constraint; full reserve truth is not a promise that every unit can be withdrawn under `Preserve`.

## Canonical Pool Identity

`DeosPoolLocator` is the lowest common Asset Conversion boundary. Every pool call, quote, reserve read, liquidity mutation, swap, and pallet-to-pallet consumer passes through it. It rejects noncanonical endpoints and same-ledger aliases before deriving storage or pool-account identity.

```text
valid_pool_pair(a, b)
  = canonical(a)
  and canonical(b)
  and ledger_key(a) != ledger_key(b)
```

Router LP bindings and Oracle topology consume this configured canonical pool identity rather than independently inventing pair order.

## Public Swap Boundary

DEOS Router is the only public XYK execution surface. Runtime `BaseCallFilter` denies raw Asset Conversion exact-input and exact-output swaps. The allow-list is explicit, so new upstream calls fail closed until reviewed.

```text
user XYK intent
  -> DEOS Router
  -> Router fee
  -> Asset Conversion execution
```

Router exposes signed exact-input call `0` and exact-output call `2`. Exact-output `max_amount_in` includes the Router fee and routed XYK input. User fee conservation is transactional: sender input equals Router fee plus routed input, and the Burn Actor receives exactly the Router fee.

## Pool Lifecycle

Signed Router call `3` is the permissionless DEOS pool lifecycle. It owns one transaction:

```text
canonical pair validation
-> canonical pool identity
-> expected LP identity
-> LP namespace/capacity/collision preflight
-> Oracle topology preflight
-> underlying pool creation
-> actual LP identity verification
-> LP reverse binding
-> forward and reverse Oracle feeds
-> commit
```

Any failure restores the complete state root. Raw public Asset Conversion creation is filtered. Production runtime code contains one direct underlying `create_pool` call inside this lifecycle, enforced by `scripts/audit-asset-conversion-boundaries.sh`. `add_liquidity` never creates or repairs topology.

`PoolIndexExtension` is removed from the signed transaction format. Because the 0.x line is fresh-genesis, `transaction_version` remains `1`; metadata and signed-extension evidence must nevertheless be regenerated from the changed format.

## Fee Domains

The runtime configures independent parameters:

| Domain | Parameter | Launch value | Recipient/effect |
| --- | --- | ---: | --- |
| XYK swap | `XykLpFee` | `0%` | No implicit LP swap revenue |
| Liquidity withdrawal | `LiquidityWithdrawalFee` | `0%` | Independent withdrawal math |
| DEOS Router | `RouterFee` | `0.5%` | Burn Actor fee flow |

Changing one domain must not alter another domain's quote or withdrawal contract.

## Failure Semantics

Noncanonical identity, physical alias, invalid path, absent pool, LP mismatch, and topology corruption are Permanent. Empty or temporarily insufficient liquidity, output withdrawability, and market-dependent slippage may be RetryLater. Unknown Asset Conversion failures fail closed as Permanent.

`PoolEmpty` describes liquidity state only. It does not establish pool registration; `Pools<Runtime>` remains authoritative.

## Genesis Boundary

Raw Asset Conversion genesis pools are unsupported because upstream genesis creation cannot atomically establish DEOS LP reverse identity and Oracle topology. Runtime genesis validation fails hard if an earlier builder creates any pool or orphan LP binding. Every DEOS profile starts with empty pool topology; permissionless complete pools are created after genesis through the canonical lifecycle.

This narrower fresh-genesis baseline excludes one-sided liquidity, same-ledger pairs, duplicate pairs, LP collisions, incomplete reverse identity, and incomplete Oracle topology without migration or repair semantics.

## Integrity and Evidence

Runtime `try_state` validates both topology directions, exact LP identity, LP asset existence, canonical pair storage, physical-pair uniqueness, required forward/reverse Oracle feeds, and exact pool/index cardinality.

Adversarial tests cover semantic aliases, raw swap and creation bypass, unrelated non-sufficient assets, protected balances, LP capacity, LP collision, LP mismatch, post-pool failure, Oracle capacity, missing/orphan bindings, missing feeds, and injected physical-alias pools. Every lifecycle rejection proves no partial mutation; post-mutation cases compare the exact state root.

Production canonical creation Weight generated at 50 steps × 20 repeats is `144,923,000 / 34,255` with 13 reads and 10 writes. Final release closure regenerates metadata, signed-extension evidence, Router/runtime Weight, Wasm, and exact-tree provenance together.
