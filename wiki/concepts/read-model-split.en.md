---
page_type: concept
title: Read-Model Split
summary: DEOS classifies user-facing data as either bounded on-chain canonical projection or indexed/materialized view. The split exists to avoid both silent indexer dependency and pushing dashboard history into consensus state.
locale: en
canonical_page_id: read-model-split
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/read-model.contract.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - concept
  - data
  - read-model
related:
  - First Steps
  - DEOS Framework Overview
  - Governance Domains
  - Reference Client
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.96
---

# Read-Model Split

## Summary

DEOS uses an explicit two-class read model for product and operator data. Every public datum should be treated as either a bounded on-chain canonical projection or an indexed/materialized view.

This is an architectural rule, not just a frontend wording preference.

## The Two Classes

### On-Chain Canonical Projection

Use this class when the data is part of the live protocol contract, needs to be consumed directly by clients, and can stay bounded in storage and servicing cost.

Examples include active governance state, live balances, current pool reserves, and other bounded helper views needed for canonical product flows.

### Indexed or Materialized View

Use this class when the data is archival, historical, search-heavy, analytical, or otherwise unbounded.

Examples include full proposal archives, long-range chart history, trading dashboards, and wallet-performance analytics.

## Why the Split Exists

The docs reject two failure modes:

- `Silent indexer dependency`, where the product claims to be chain-native but actually depends on off-chain infrastructure for core UX
- `Consensus-state dashboard creep`, where unbounded history or analytics are pushed into chain state just to avoid external tooling

## Product Honesty Rule

If a user-facing screen depends on materialized data, that dependency should be explicit. The product should not present it as if it were indistinguishable from bounded protocol truth.

## Why This Matters for New Work

When adding a new query surface, the first question is not only “what data does the UI want?” A better question is:

- Should this become a bounded on-chain projection?
- Or should it become an external materialized view?

That decision protects both protocol integrity and product honesty.

## Related

- [First Steps](../getting-started/first-steps.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Governance Domains](governance-domains.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/read-model.contract.en.md`
- `docs/core.architecture.en.md`
