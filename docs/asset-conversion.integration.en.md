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

The runtime carries the minimal full-balance correction from Polkadot SDK PR `#12408`, merge commit `408895c27a5aff4bac99e956df5426983566f8cb`. Provenance, exact source digests, removal condition, and review expiry are recorded in `template/vendor/pallet-asset-conversion/DEOS-PATCH.md`. Cargo resolves exactly one implementation.

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
