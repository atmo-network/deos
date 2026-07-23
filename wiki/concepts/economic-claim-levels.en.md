---
page_type: concept
title: Economic Claim Levels
summary: A ladder and audit posture for classifying DEOS/TMCTOL economic claims across formulas, simulations, runtime enforcement, governance dependency, and market assumptions.
locale: en
canonical_page_id: economic-claim-levels
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/tmctol.specification.en.md
  - ../../simulator/README.md
  - ../../.agents/skills/alignment/economic-claims.json
status: active
audience: newcomer
tags:
  - concept
  - economics
  - validation
  - claims
related:
  - Economic Thresholds
  - Invariant Map
  - Invariant and Threat Map
  - Three-Layer Validation
last_compiled: 2026-07-20
confidence: 0.85
---

# Economic Claim Levels

## Summary

DEOS/TMCTOL claims should not mix math, simulation, implementation, governance, and market behavior in one sentence. This page introduces a wiki-local explanatory ladder for those evidence classes; the labels are not a canonical runtime or specification enum. The alignment audit separately keeps a small inventory of load-bearing economic claims with explicit proof kinds.

## Levels

| Level | Meaning | Example |
| --- | --- | --- |
| Level 0 | Formula-defined | TMC price follows the curve equation |
| Level 1 | Simulator-supported | Threshold holds in vectors |
| Level 2 | Runtime-enforced | Tests pin the transition |
| Level 3 | Governance-dependent | True inside bounded policy |
| Level 4 | Market assumption | Depends on users/liquidity/demand |

## Reading Rule

A stronger-looking phrase is not a stronger claim. If a statement depends on market behavior, call it Level 4 even if the underlying formula is Level 0.

Example: floor mechanics can be formula-defined and simulator-supported, but user demand, arbitrage timing, or liquidity-provider behavior may still be Level 4 assumptions.

## Audit Inventory

The repository-local alignment layer also tracks selected economic claims in `economic-claims.json`. Each entry should point to evidence such as formulas, tests, runtime symbols, architecture claims, or guard scripts. The inventory is not a marketing checklist. Its job is to keep strong prose tied to falsifiable support and to make missing proof surfaces visible.

## Related

- [Economic Thresholds](economic-thresholds.en.md)
- [Invariant Map](invariant-map.en.md)
- [Invariant and Threat Map](invariant-map.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
