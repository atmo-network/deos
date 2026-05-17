---
page_type: overview
title: Governance Overview
summary: A newcomer-facing map of DEOS Governance as a bounded constitutional layer that separates protocol physics, strategic protection, tactical coordination, and live read-model truth.
locale: en
canonical_page_id: governance-overview
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/manifesto.en.md
status: active
audience: newcomer
tags:
  - overview
  - governance
  - domains
related:
  - Governance Domains
  - Physics-First vs Politics-First
  - Staking Pools
  - Core Terms
last_compiled: 2026-05-17
confidence: 0.94
---

# Governance Overview

## Summary

DEOS Governance is the framework's bounded constitutional layer. It does not replace protocol physics with politics-by-default; it governs the strategic, tactical, and social surfaces that remain after the economic kernel mechanizes what it can.

The whole subsystem is easiest to read through three separations:

- Strategy is not tactics;
- Approval is not protection;
- Live on-chain governance truth is not archive history.

## Current Shape

The current reference line uses explicit governance domains instead of one undifferentiated referendum pool. Each domain keeps its authority, payload families, cadence, and protection surface visible.

At overview level, the shape is:

- Strategic governance protects protocol and network-level subjects;
- Tactical governance handles narrower domain-local spending and coordination;
- A primary decision lane decides the proposal itself;
- A protection lane decides whether the proposal should be blocked or allowed through;
- Proposal payloads are typed rather than hidden inside opaque bytes;
- Live governance UX reads bounded runtime views, while archive search and long timelines belong to indexed or materialized layers.

This is why DEOS Governance looks more like a constitutional layer above a deterministic kernel than a generic voting portal.

## Public Lifecycle

From a newcomer perspective, a proposal should be read as a typed lifecycle, not as a single yes/no event:

1. submission opens the item and its protection window;
2. ordinary primary voting starts after the configured lead-in;
3. approval can still wait through enactment delay;
4. execution can fail and that failure is visible state;
5. recent finalized outcomes remain on-chain only for bounded live observability.

The intent is honest live state, not an unbounded social archive inside runtime storage.

## Relationship to Staking

Governance can feed bounded participation-quality signals into staking rewards, but governance and staking remain separate subsystems. The connection means governance quality is economically relevant; it does not mean the staking pallet owns governance history or that governance stores infinite reward memory.

## Reading the Governance Cluster

Use the governance wiki cluster in this order:

1. `Governance Overview` — why the subsystem exists;
2. [Governance Domains](../concepts/governance-domains.en.md) — how one governance cell is typed;
3. [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md) — why protocol physics stays protected;
4. [Staking Pools](../concepts/staking-pools.en.md) — where governance-conditioned reward signals meet staking;
5. [Core Terms](../glossary/core-terms.en.md) — recurring vocabulary.

## Related

- [Governance Domains](../concepts/governance-domains.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [Staking Pools](../concepts/staking-pools.en.md)
- [Core Terms](../glossary/core-terms.en.md)
