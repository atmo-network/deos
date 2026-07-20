---
page_type: concept
title: Staking Pools
summary: DEOS staking uses multi-asset share vaults with transferable `stXXX` receipts. The Phase 1 launch line enables liquid `$NTVE -> stNTVE` accounting but keeps user LP nomination and claimable nomination rewards inactive; those belong to an explicit Phase 2 runtime-upgrade boundary.
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
last_compiled: 2026-07-20
confidence: 0.85
---

# Staking Pools

## Summary

DEOS staking is a multi-asset share-vault system. Each registered staking asset has a pool, deterministic accounts, and share/receipt accounting so backing can rise without writing rewards to every holder.

The native staking contract separates liquid `$NTVE -> stNTVE` share-vault accounting from collator nomination. The Phase 1 launch line uses trusted permissioned collators and keeps user LP nomination and claimable nomination rewards inactive. Phase 2 may use locked `NTVE/stNTVE` LP; a plain `stNTVE` balance never serves as the collator-security signal.

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

## Phase Boundary for Collator Security

Phase 1 uses trusted permissioned collators. It does not expose active user nomination economics or claimable nomination rewards.

The explicit Phase 2 contract uses LP custody rather than live `stNTVE` balances or transfer-driven native bindings:

```text
$NTVE + stNTVE
  -> add liquidity to NTVE/stNTVE
  -> receive NTVE/stNTVE LP
  -> lock_native_lp_for_collator(lp_asset_id, amount, operator)
```

The runtime contains bounded custody and valuation surfaces for locked `NTVE/stNTVE` LP, but the launch contract keeps nomination and its reward flow inactive until an explicit Phase 2 runtime upgrade.

## Governance Custody

The same native value surface can also be locked for governance-only `NativeVotePower` without nominating a collator. The current runtime includes separate LP and native-asset custody paths for tactical protection voting, with unlock requests blocked while governance lock horizons are active.

## Phase 2 Native Nomination Rewards

The specification reserves native-specific claim paths for Phase 2. Generic same-asset reward settlement rejects the native staking asset so `$NTVE` nomination rewards cannot escape through legacy auto-compound semantics.

The implemented settlement surface includes:

- `claim_nomination_reward(epoch)` for liquid `$NTVE` payout
- `claim_and_compound_nomination_reward(epoch, operator)` for turning payout into locked LP
- `claim_nomination_reward_batch(epochs)` for bounded multi-epoch native claims

## Relationship to Governance Rewards

Staking and governance remain separate subsystems:

- Staking owns pool math, receipts, locked LP custody, reward snapshots, and settlement
- Governance owns bounded participation memory, vote-power policy, execution state, and exported reward coefficients

For non-native assets, same-asset reward settlement can still auto-compound into fresh receipts. Native `$NTVE` nomination rewards remain a dedicated, phase-gated flow and stay inactive on the trusted-collator Phase 1 launch line.

## Related

- [Governance Domains](governance-domains.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [Core Terms](../glossary/core-terms.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
