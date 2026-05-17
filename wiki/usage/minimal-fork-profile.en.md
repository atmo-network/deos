---
page_type: usage
title: Minimal Fork Profile
summary: The minimum set of choices a downstream team must make before turning DEOS into a concrete ecosystem.
locale: en
canonical_page_id: minimal-fork-profile
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ./forking-deos.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: developer
tags:
  - usage
  - forkability
  - launch
related:
  - Forking DEOS
  - What DEOS Is Not
  - Invariant Map
  - Parachain Context
last_compiled: 2026-05-17
confidence: 0.84
---

# Minimal Fork Profile

## Summary

A DEOS fork should not launch by only changing names. A downstream team must choose the economic, governance, runtime, client, and operator profile that turns the framework into a real ecosystem.

## Required Choices

| Area                         | Minimum fork decision                                                          |
| ---------------------------- | ------------------------------------------------------------------------------ |
| Native asset                 | Name, ticker, decimals, allocation, role in staking/governance                 |
| Foreign collateral set       | Which assets can be registered, routed, or used as collateral                  |
| TMC curve params             | Initial price, slope, supply assumptions, launch immutability policy           |
| TOL distribution             | Bucket split, paired treasuries, reserve/lane semantics                        |
| Bucket policies              | Which bucket wakes which actor, threshold, retry, and treasury lane            |
| Router fee                   | Fee bounds, burn/sink routing, governance mutability                           |
| Governance domain pairs      | Primary/protection tokens, payload kinds, cadence, execution authority         |
| Staking receipt policy       | Receipt namespaces, native receipt, LP custody, reward paths                   |
| Materialized provider policy | Which user flows need indexers or archive/search providers                     |
| Collator/randomness posture  | Trusted phase, upgrade path, relay/protocol randomness dependency              |
| Client/product surface       | Default endpoints, wallet presets, copy, dApps, risk wording                   |
| Validation baseline          | Simulator, runtime tests, client validation, wiki trust, operator smoke checks |

## Fork Readiness Rule

If a row is undecided, the fork is still a prototype. If a row is decided but not validated, the fork is not launch-ready.

## Related

- [Forking DEOS](forking-deos.en.md)
- [What DEOS Is Not](../concepts/what-deos-is-not.en.md)
- [Invariant Map](../concepts/invariant-map.en.md)
- [Parachain Context](../concepts/parachain-context.en.md)
