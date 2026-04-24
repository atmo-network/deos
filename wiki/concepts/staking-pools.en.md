---
page_type: concept
title: Staking Pools
summary: DEOS staking uses a multi-asset share-vault model with transferable `stXXX` receipts. Native `$NTVE` now mints liquid `stNTVE`, while collator security and native nomination rewards come from explicitly locked `NTVE/stNTVE` LP rather than from a live `stNTVE` balance binding.
locale: en
canonical_page_id: staking-pools
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/staking.specification.en.md
  - ../../docs/staking.architecture.en.md
  - ../../docs/governance.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - staking
  - receipts
related:
  - Governance Domains
  - Routing and Minting Loop
  - Core Terms
  - Newcomer FAQ
last_compiled: 2026-04-25
confidence: 0.94
---

# Staking Pools

## Summary

DEOS staking is a multi-asset share-vault system. Each registered staking asset has a pool, deterministic accounts, and share/receipt accounting so backing can rise without writing rewards to every holder.

The current native staking line is intentionally split into two surfaces: `$NTVE -> stNTVE` is liquid share-vault staking, while collator nomination and native reward exposure use locked `NTVE/stNTVE` LP. A plain `stNTVE` balance is not the collator-security signal.

## Share-Vault Model

For each staking asset, the system keeps:

- One deterministic pool account
- One pool state object
- Transferable receipt supply when a `stXXX` asset exists
- Bounded read surfaces for exchange rate, account value, and reward claimability

Ownership is represented by shares. Pool inflows increase what each share is worth instead of forcing a fan-out write across every user account.

## `stXXX` Receipts

`stXXX` tokens are yield-bearing receipts for staking pools:

- Local and native receipts use the `TYPE_STAKED` namespace
- Foreign staking receipts use `TYPE_STAKED_FOREIGN`
- Receipt supply tracks outstanding pool shares
- Share value rises when pool backing grows while receipt supply stays fixed

For native staking, the concrete receipt is `stNTVE`.

## Native `$NTVE -> stNTVE`

The native entry path is now liquid and operator-free:

```text
$NTVE
  -> Staking::stake_native(amount)
  -> mint stNTVE receipt shares
```

This is a vault deposit and receipt mint, not an ordinary AMM swap. It increases native staking backing and mints receipt shares according to staking-pool accounting.

## Collator Security Uses Locked LP

Native collator backing is no longer derived from live `stNTVE` balances or transfer-driven native bindings. The current security path is explicit LP custody:

```text
$NTVE + stNTVE
  -> add liquidity to NTVE/stNTVE
  -> receive NTVE/stNTVE LP
  -> lock_native_lp_for_collator(lp_asset_id, amount, operator)
```

Locked `NTVE/stNTVE` LP is valued conservatively through the runtime's native-equivalent read model and feeds collator ranking / native nomination reward exposure.

## Governance Custody

The same native value surface can also be locked for governance-only `NativeVotePower` without nominating a collator. The current runtime includes separate LP and native-asset custody paths for tactical protection voting, with unlock requests blocked while governance lock horizons are active.

## Native Nomination Rewards

Native nomination rewards are settled through native-specific claim paths. Generic same-asset reward settlement rejects the native staking asset so `$NTVE` nomination rewards cannot escape through legacy auto-compound semantics.

The native settlement paths include:

- `claim_nomination_reward(epoch)` for liquid `$NTVE` payout
- `claim_and_compound_nomination_reward(epoch, operator)` for turning payout into locked LP
- `claim_nomination_reward_batch(epochs)` for bounded multi-epoch native claims

## Relationship to Governance Rewards

Staking and governance remain separate subsystems:

- Staking owns pool math, receipts, locked LP custody, reward snapshots, and settlement
- Governance owns bounded participation memory, vote-power policy, execution state, and exported reward coefficients

For non-native assets, same-asset reward settlement can still auto-compound into fresh receipts. Native `$NTVE` nomination rewards use the dedicated native paths above.

## Related

- [Governance Domains](governance-domains.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [Core Terms](../glossary/core-terms.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)

## Sources

- `docs/staking.specification.en.md`
- `docs/staking.architecture.en.md`
- `docs/governance.specification.en.md`
