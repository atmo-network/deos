---
page_type: concept
title: Token Surfaces
summary: A compact map of the main DEOS/TMCTOL token surfaces, including Native, VETO, BLDR, stNTVE, LP tokens, and how each token participates in economics, governance, staking, and read-model boundaries.
locale: en
canonical_page_id: token-surfaces
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/staking.specification.en.md
  - ../../template/primitives/src/ecosystem.rs
status: active
audience: newcomer
tags:
  - concept
  - tokens
  - economics
  - governance
related:
  - TMCTOL Standard
  - Staking Pools
  - Governance Domains
  - Token Minting Curve
  - Read-Model Split
last_compiled: 2026-05-17
confidence: 0.86
---

# Token Surfaces

## Summary

DEOS uses several token surfaces at once. They are not interchangeable. Some tokens express protocol economics, some express protection or tactical governance, and some are receipts for positions inside staking or liquidity systems.

This page is a compact map. It does not replace the exact formulas, governance rules, or runtime constants owned by other pages.

## Main Tokens

### Native / `$NTVE`

`$NTVE` is the sovereign base token of the current reference line. It anchors native staking, can pair with `stNTVE` for collator-security LP custody, and participates in the protocol/network governance hierarchy.

### `$VETO`

`$VETO` is a protection token, not a second ordinary governance token. Its purpose is constitutional safety: it can block or protect strategic protocol/network changes instead of acting as a positive day-to-day control surface.

### `$BLDR`

`$BLDR` is the flagship tactical governance token for builder coordination. In the current line it is associated with invoice-style tactical governance and BLDR-specific coordination lanes. Its value story is therefore not just emission; it depends on whether the downstream builder ecosystem gives the tactical coordination surface real demand.

## Receipt and Position Tokens

### `stNTVE`

`stNTVE` is the native liquid staking receipt. It represents share-vault ownership, not direct collator nomination by itself. Collator security uses explicit locked `NTVE/stNTVE` LP custody.

### LP Tokens

LP tokens represent positions in AMM pools. Some LP tokens can become protocol automation inputs: an actor can receive LP, unwind it, split outputs, or use it in treasury/staking flows depending on its execution plan.

### `stXXX`

`stXXX` names the general family of staking receipt assets. Native receipts and foreign receipts use distinct namespaces so receipt derivation remains collision-free.

## Monetary Policy Boundaries

The current wiki intentionally separates three questions:

- **Emission math:** owned by [Token Minting Curve](../overview/token-minting-curve.en.md) and [TMCTOL Formulas](../math/tmctol-formulas.en.md).
- **Governance power:** owned by [Governance Domains](governance-domains.en.md).
- **Receipt value:** owned by [Staking Pools](staking-pools.en.md) and liquidity-position accounting.

Do not infer a full monetary policy for every token from governance role alone. For example, `$BLDR` can have tactical governance importance without the wiki pretending that downstream demand, launch allocation, or ecosystem product-market fit is already solved inside the framework.

## Read-Model Rule

Token balances and bounded receipt/projection data can be direct runtime truth. Long-range holder analytics, historical valuation, portfolio discovery across many assets, and demand narratives are materialized or downstream-product concerns.

Use [Read-Model Split](read-model-split.en.md) to decide which surface a token datum belongs to.

## Related

- [TMCTOL Standard](tmctol-standard.en.md)
- [Staking Pools](staking-pools.en.md)
- [Governance Domains](governance-domains.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [Read-Model Split](read-model-split.en.md)
