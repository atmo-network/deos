---
page_type: concept
title: Staking Pools
summary: DEOS staking uses a multi-asset share-vault model. Each asset has its own pool and sovereign inflow channel, backing raises share value instead of triggering fan-out reward writes, and the long-term direction is transferable `stXXX` receipts with a native-special-case security path.
locale: en
canonical_page_id: staking-pools
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/staking.specification.en.md
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
last_compiled: 2026-04-16
confidence: 0.92
---

# Staking Pools

## Summary

DEOS staking is not modeled as a classic era-reward fan-out system. The specification defines a multi-asset share-vault design where each registered asset gets its own staking pool and sovereign inflow channel.

The core idea is simple: when backing arrives at the pool, the value of existing ownership goes up. The runtime should not need to iterate over every staker just to distribute one inflow.

## Share-Vault Model

For each staking asset, the system keeps:

- One deterministic pool account
- One pool state object
- Many positions representing share ownership

Ownership is represented by shares. Pool inflows increase what each share is worth instead of writing rewards into every user account.

## Why This Scales Better

The staking specification explicitly rejects iterating all stakers on every inflow. Instead, inflows show up as share-price appreciation.

That means the important bounded reads are things like:

- Total shares
- Accounted balance
- Per-account shares
- Current stake value derived from those numbers

## Sovereign Inflow Channel

Each staking asset has a deterministic sovereign account. Funds transferred to that account in the same asset become backing for the pool.

This keeps the inflow path explicit and matches the broader DEOS pattern of token-driven coordination.

## `stXXX` Direction

The long-term direction is tokenized staking receipts:

- Local and native receipts in the `TYPE_STAKED` namespace
- Foreign receipts in the `TYPE_STAKED_FOREIGN` namespace
- Receipt supply tracking outstanding pool shares
- Share price rising when pool backing grows while receipt supply stays constant

In short, `stXXX` is meant to represent transferable, yield-bearing ownership of the pool.

## Native Special Case

The native token path is special. Non-native staking stays economic-only, but native `$NTVE` also participates in the collator or operator security path.

The target design keeps one native receipt, `stNTVE`, while requiring explicit operator-aware entry or rebinding for the security surface.

## Relationship to Governance Rewards

The staking and governance specs are linked, but they are not collapsed into one subsystem:

- Staking owns pool math, receipts, and reward settlement
- Governance owns bounded participation memory and the exported reward coefficient

The intended reward direction is same-asset auto-compound into fresh `stXXX` rather than a separate liquid payout leg.

## Related

- [Governance Domains](governance-domains.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [Core Terms](../glossary/core-terms.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)

## Sources

- `docs/staking.specification.en.md`
- `docs/governance.specification.en.md`
