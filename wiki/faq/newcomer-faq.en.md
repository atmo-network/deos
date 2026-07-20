---
page_type: faq
title: Newcomer FAQ
summary: A compact self-contained FAQ for recurring newcomer questions about DEOS, TMCTOL, AAA, governance, staking, data surfaces, wiki domains, and the reference client.
locale: en
canonical_page_id: newcomer-faq
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../docs/manifesto.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/staking.specification.en.md
  - ../../docs/read-model.contract.en.md
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
status: active
audience: newcomer
tags:
  - faq
  - onboarding
related:
  - Domain Map
  - DEOS Framework Overview
  - First Steps
  - Reading Paths
  - AAA System
  - Physics-First vs Politics-First
  - UI Kit and Domain DAG
  - Generated Wiki
  - Core Terms
last_compiled: 2026-07-20
confidence: 0.85
---

# Newcomer FAQ

## Summary

This page answers the questions that usually appear first: what DEOS is, how TMCTOL fits into it, what governance controls, how AAA and staking work at a high level, how the wiki is organized, and how honest the reference client must be.

Use [Domain Map](../concepts/domain-map.en.md) for the larger system shape and [Reading Paths](../getting-started/reading-paths.en.md) when you have a specific task.

## Identity and Starting Point

**Is DEOS the token or the standard?** No. `DEOS` is the framework and reference stack. `TMCTOL` is the current flagship tokenomic standard running on top of it.

**Why is the wiki organized by domains?** Because DEOS is easier to understand as interacting domains than as a pallet list: Economic Physics, autonomous actors, routing, governance, staking, read models, client UX, tooling, and future gates.

**Where should I start?** If you only want the shortest route, read [DEOS Framework Overview](../overview/deos-framework.en.md), [Core Terms](../glossary/core-terms.en.md), and [TMCTOL Standard](../concepts/tmctol-standard.en.md). If you are about to change something, use [Reading Paths](../getting-started/reading-paths.en.md).

## Economics, Governance, and Actors

**Why does TMCTOL avoid redemption?** Because the current standard treats minting as one-way protocol physics instead of a reserve exit door. See [TMCTOL Standard](../concepts/tmctol-standard.en.md) and [Token Minting Curve](../overview/token-minting-curve.en.md).

**Does governance disappear?** No. Governance stays, but its role is narrowed: it steers direction, tactical domains, and bounded upgrade paths instead of manually controlling survival physics. See [Governance Overview](../overview/governance-overview.en.md) and [Governance Domains](../concepts/governance-domains.en.md).

**What does deterministic mean?** Protocol-managed reactions are explicit and repeatable for the same chain state. It does not mean markets become predictable.

**What is AAA versus an AA-Actor?** `AAA` is the whole Account Abstraction Actors system: scheduler, lifecycle rules, execution plans, actor accounts, and task execution. An `AA-Actor` is one concrete runtime instance inside that system. See [AAA System](../overview/aaa-system.en.md) and [AA-Actor](../overview/aa-actor.en.md).

**How does staking work?** Staking is a multi-asset share-vault domain. [Staking Pools](../concepts/staking-pools.en.md) explains native `stNTVE`, LP nomination, and reward snapshots.

## Data, Client, and Wiki Boundaries

**Why on-chain vs materialized data?** Product honesty depends on knowing which data is canonical chain truth and which data is indexed or derived. See [Read-Model Split](../concepts/read-model-split.en.md).

**Is the web client the source of truth?** No. The web client is a reference product surface that must label data provenance honestly. See [Reference Client](../overview/reference-client.en.md).

**Where do release versions and status notes belong?** Release history belongs in the changelog, open work belongs in the backlog, and newcomer-facing current state belongs in [Development Status](../development/status.en.md). Architecture and wiki pages should explain implementation truth and boundaries.

**What are UI Kit and Domain DAG?** They are client-side anti-duplication and ownership disciplines. See [UI Kit and Domain DAG](../concepts/ui-kit-and-domain-dag.en.md).

**Why can the web client render wiki markdown directly?** Wiki markdown is trusted repo-local content guarded by repository validation, not arbitrary user input. See [Generated Wiki](../concepts/generated-wiki.en.md).

## Related

- [Domain Map](../concepts/domain-map.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [First Steps](../getting-started/first-steps.en.md)
- [Reading Paths](../getting-started/reading-paths.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
- [UI Kit and Domain DAG](../concepts/ui-kit-and-domain-dag.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Core Terms](../glossary/core-terms.en.md)
