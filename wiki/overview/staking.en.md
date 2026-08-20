---
type: overview
title: Staking
description: DEOS staking uses multi-asset share vaults with transferable `stXXX` receipts and explicit `TrustedSet` or `LpBackedSelection` native-security mode.
locale: en
canonical_page_id: staking
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/staking/docs/specification.en.md
  - resource: ../../template/pallets/staking/docs/architecture.en.md
  - resource: ../../template/pallets/governance/docs/specification.en.md
status: stable
audience: newcomer
tags:
  - overview
  - staking
  - receipts
related:
  - Governance Domains
  - Routing and Minting Loop
  - Core Terms
  - Newcomer FAQ
---

# Staking

## Summary

DEOS staking is a multi-asset share-vault system. Each registered staking asset has one deterministic pool account and share/receipt accounting so backing can rise without writing rewards to every holder.

The native staking contract separates liquid `$NTVE -> stNTVE` share-vault accounting from collator nomination. The current `TrustedSet` mode uses permissioned collators and prevents new LP-backed nomination obligations. `LpBackedSelection` uses locked `NTVE/stNTVE` LP; a plain `stNTVE` balance never serves as the collator-security signal.

## Share-Vault Model

For each staking asset, the system keeps:

- One deterministic pool account
- One pool state object
- Transferable receipt supply when a `stXXX` asset exists
- Bounded read surfaces for exchange rate, account value, custody, security mode, readiness, and session identity

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
  -> Staking::stake(NativeStakingAssetId, amount)
  -> mint stNTVE receipt shares
```

This is a vault deposit and receipt mint, not an ordinary AMM swap. It increases native staking backing and mints receipt shares according to staking-pool accounting.

## Native Security Modes

`TrustedSet` uses permissioned collators and rejects new LP nomination, redelegation, certified reward funding, candidate selection, and compound operations while preserving retained claims and custody exits.

`LpBackedSelection` uses LP custody rather than live `stNTVE` balances or transfer-driven native bindings:

```text
$NTVE + stNTVE
  -> add liquidity to NTVE/stNTVE
  -> receive NTVE/stNTVE LP
  -> lock_native_lp_for_collator(lp_asset_id, amount, operator)
```

Switching to `LpBackedSelection` is an explicit runtime upgrade. The runtime contains bounded custody, valuation, readiness, session-snapshot, funding, liability, claim, expiry, and compound surfaces for that mode.

## Governance Custody

The same native value surface can also be locked for governance-only `NativeVotePower` without nominating a collator. The current runtime includes separate LP and native-asset custody paths for tactical protection voting, with unlock requests blocked while governance lock horizons are active.

## LP-Backed Native Nomination Rewards

In `LpBackedSelection`, the runtime freezes one atomic session-native eligibility snapshot containing bounded participants, candidate-eligible operators, conservative LP values, governance coefficients, account weights, and the total denominator. Certified funding creates exact liability; session-owned retention, liquid claims, expiry, and bounded atomic compound preserve settlement truth. `TrustedSet` cannot create these obligations but preserves settlement of retained Finalized pots.

The legacy generic block-based reward engine, rollover cursor, reward-account inference, bootstrap call, and claim surfaces are absent.

## Relationship to Governance Rewards

Staking and governance remain separate subsystems:

- Staking owns pool math, receipts, locked LP custody, and session security snapshots
- Governance owns bounded participation memory, vote-power policy, execution state, and exported reward coefficients

Generic non-native share-vault yield remains receipt appreciation after direct backing inflow and `sync_pool`; it creates no reward pot, liability, claim, or event-ingress dependency. Native `$NTVE` nomination rewards remain a dedicated mode-gated flow: new obligations require `LpBackedSelection`, while retained settlement survives `TrustedSet`.

## Related

- [Governance Domains](../concepts/governance-domains.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Core Terms](../glossary/core-terms.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
