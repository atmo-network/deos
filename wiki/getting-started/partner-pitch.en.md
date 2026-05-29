---
page_type: getting-started
title: Partner Pitch
summary: A concise external pitch for partner teams evaluating DEOS as a forkable foundation for deterministic protocol economies.
locale: en
canonical_page_id: partner-pitch
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../AGENTS.md
  - ../../docs/manifesto.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: partner
tags:
  - onboarding
  - positioning
  - partners
  - adoption
related:
  - DEOS in 60 Seconds
  - Who DEOS Is For
  - Minimal Fork Profile
  - What DEOS Is Not
last_compiled: 2026-05-17
confidence: 0.85
---

# Partner Pitch

## One sentence

DEOS gives partner teams a forkable runtime foundation where treasury behavior is not a committee habit, but a set of deterministic economic circuits.

## The problem

Most token economies start with a promise: the team will manage treasury funds, liquidity, emissions, incentives, upgrades, and governance responsibly.

That promise is fragile. It depends on discretionary operators, off-chain coordination, incomplete dashboards, and political interpretation during stress.

DEOS changes the default. It moves the core economic loop into the runtime: minting, protocol-owned liquidity, routing, staking, governance boundaries, and automated actors become explicit protocol surfaces.

## What a partner gets

A partner fork does not start from a blank chain template. It starts from a reference framework with:

- Runtime pallets for asset identity, routing, staking, governance, TMC, and AAA automation
- TMCTOL as the first economic standard: mint-only curve, treasury-owned liquidity, fee burn, bucketed policy, and bounded governance
- A reference client that separates direct on-chain truth from materialized views
- Operator scripts and validation gates for local networks, docs, wiki, and runtime work
- Architecture and specification docs that explain what is contract, implementation, or roadmap

## What you can change safely

Partner value comes from changing the downstream product layer without weakening the protocol substrate.

Good first changes:

- Product narrative, ecosystem thesis, and dApp roadmap
- Launch parameters after simulator validation
- Client copy, onboarding, and visual identity
- Governance handoff plan and operator runbooks
- Materialized/indexed views for dashboards, search, and analytics

Changes that need protocol review:

- Curve physics, bucket accounting, reserve semantics, or guarantee wording
- Governance protection-track authority
- Runtime read-model boundaries
- Collator/security assumptions and reward routing

## Why it matters

The pitch is not “price goes up.”

The pitch is: economic claims become bounded, inspectable, and forkable. A partner team can still choose product narrative, dApps, ecosystem culture, and launch policy, but the core treasury/liquidity machine is no longer hidden in ad-hoc operations.

## The first 30/90 days

**First 30 days:** understand the framework, choose the fork profile, validate whether TMCTOL fits the intended ecosystem, and define product-specific surface outside the core framework.

**First 90 days:** adapt launch parameters, client copy, read-model/indexer needs, operator runbooks, governance handoff, and concrete dApps for the downstream ecosystem.

## What DEOS does not remove

DEOS does not remove market risk, product risk, community risk, launch execution risk, or governance responsibility. It makes the protocol-managed part of the economy explicit enough to inspect, test, fork, and constrain.

## Next pages

- [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
- [Who DEOS Is For](who-deos-is-for.en.md)
- [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
- [What DEOS Is Not](../concepts/what-deos-is-not.en.md)
