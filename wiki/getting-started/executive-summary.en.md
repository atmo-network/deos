---
page_type: getting-started
title: Executive Summary
summary: A one-page external summary of what DEOS is, why it matters, why Polkadot/Substrate is the right substrate, what is shipped, what is not shipped, and how adoption starts.
locale: en
canonical_page_id: executive-summary
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../AGENTS.md
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
status: active
audience: partner
tags:
  - onboarding
  - positioning
  - executive
  - adoption
related:
  - DEOS in 60 Seconds
  - Partner Pitch
  - DEOS vs DAO Treasury
  - Minimal Fork Profile
last_compiled: 2026-05-17
confidence: 0.84
---

# Executive Summary

## What it is

DEOS is a forkable runtime framework for programmable token economies. Its core idea is simple: replace discretionary treasury operations with deterministic economic circuits that live inside the protocol.

TMCTOL is the first economic standard running on DEOS. It combines a mint-only token curve, treasury-owned liquidity, fee burn, bucketed policy, staking, routing, bounded governance, and automated actors.

## Why it matters

Many token economies rely on a future committee to manage liquidity, treasury funds, emissions, incentives, and upgrades well. DEOS narrows that trust surface by moving repeated economic behavior into explicit runtime mechanisms.

The result is not a price promise. It is a clearer contract: this part is protocol-managed, this part is governed, this part is indexed/materialized, and this part remains product and market risk.

## Why Polkadot/Substrate

DEOS needs a runtime-first environment where economic rules, assets, automation, governance, and XCM-facing asset identity can be encoded as first-class protocol logic. Substrate and Polkadot provide that runtime construction surface without forcing DEOS to become a general-purpose smart-contract app.

## What is already shipped

- Rust runtime workspace with DEOS pallets, primitives, and runtime configuration.
- TMCTOL reference mechanics and specifications.
- AAA actor automation model.
- SvelteKit reference client with domain slices and wiki rendering.
- Operator scripts, validation gates, and generated wiki metadata.
- `0.5.0` baseline release tag for the current framework line.

## What is not shipped

- A finished consumer ecosystem product.
- Permissionless collator onboarding as the current launch default.
- Full portfolio UX beyond currently available chain/read-model surfaces.
- A guarantee of market demand, price appreciation, or risk-free treasury behavior.

## Adoption path

A partner team starts by reading the external entry pages, choosing a minimal fork profile, validating whether TMCTOL fits its ecosystem, and defining product-specific dApps and user-facing philosophy downstream.

## Next pages

- [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
- [Partner Pitch](partner-pitch.en.md)
- [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.en.md)
- [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
