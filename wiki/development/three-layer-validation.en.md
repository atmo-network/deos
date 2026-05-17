---
page_type: process
title: Three-Layer Validation
summary: The required testing strategy for validating protocol and economic changes in DEOS.
locale: en
canonical_page_id: three-layer-validation
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
status: active
audience: developer
tags:
  - development
  - testing
  - validation
related:
  - Repository Structure
  - Tech Stack
last_compiled: 2026-04-15
confidence: 0.95
---

# Three-Layer Validation

## Summary

In DEOS, truth for protocol and economic changes is established across three distinct layers: Simulation, Implementation, and Integration. When a change affects the core economic contract, all three layers must be used to guarantee mathematical, behavioral, and systemic correctness.

## Layer 1: Simulation (Mathematical Truth)

**Location:** `/simulator`
**Stack:** JavaScript / BigInt / PPB

Before writing any runtime logic, formulas and invariants are verified in the simulator. This layer proves the mathematical model is sound, ensuring thresholds (like the Gravity Well or Elasticity Inversion) behave exactly as the `TMCTOL` specification demands.

## Layer 2: Implementation (Behavioral Truth)

**Location:** `/template/pallets`
**Stack:** Rust / `Perbill` / Unit Tests / `frame_benchmarking::v2`

This layer verifies that the Rust runtime code matches the mathematical model and respects block-weight constraints.

- **Unit Tests:** Mock the environment (using stateful `RefCell<BTreeMap>` for realistic AMM/TMC simulation) to verify logic mechanisms.
- **Benchmarks:** Enforce strict limits on `RefTime` (computation) and `ProofSize` (storage access) under worst-case state assumptions.

## Layer 3: Integration (Systemic Truth)

**Location:** `/template/runtime`
**Stack:** Rust / Integration Tests / XCM

This layer verifies that the individual pallets and components coordinate correctly within the full Parachain runtime. It tests end-to-end scenarios, cross-pallet interactions, and XCM messaging to guarantee systemic safety.

## Validation Priority Rules

- **Targeted Scope:** Run the smallest meaningful validation set for the touched surface first.
- **Escalation:** If a diff crosses layer boundaries (e.g., changes an economic formula), escalate to the full Three-Layer validation.
- **Simulator Mandatory:** Routine support biases toward `/docs`, `/template`, and `/web-client`, but `/simulator` becomes mandatory the moment tokenomics or invariant math moves.

## Related

- [Repository Structure](../implementation/repository-structure.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [Validation Troubleshooting](../usage/validation-troubleshooting.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Development Status](status.en.md)
