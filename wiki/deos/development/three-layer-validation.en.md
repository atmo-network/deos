---
type: process
title: Three-Layer Validation
description: The required testing strategy for validating protocol and economic changes in DEOS.
locale: en
canonical_page_id: three-layer-validation
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../../AGENTS.md
  - resource: ../../../docs/deos/state-witness-accounting.contract.en.md
status: stable
audience: developer
tags:
  - development
  - testing
  - validation
related:
  - Repository Structure
  - Tech Stack
---

# Three-Layer Validation

## Summary

In DEOS, evidence for protocol and economic changes is established across three distinct layers: Simulation, Implementation, and Integration. When a change affects the core economic contract across those boundaries, all three layers contribute distinct falsification evidence; none provides an unconditional proof by itself.

## Layer 1: Simulation (Mathematical Truth)

**Location**: `/simulators/tmctol`
**Stack**: JavaScript / BigInt / PPB

Before writing runtime logic, formulas and invariants are exercised in the simulator. This layer checks the mathematical model against explicit vectors and properties, including thresholds such as the Gravity Well or Elasticity Inversion.

## Layer 2: Implementation (Behavioral Truth)

**Location**: `/template/pallets`
**Stack**: Rust / `Perbill` / Unit Tests / `frame_benchmarking::v2`

This layer verifies that the Rust runtime code matches the mathematical model and respects block-weight constraints.

- **Unit Tests**: Mock the environment (using stateful `RefCell<BTreeMap>` for realistic AMM/TMC simulation) to verify logic mechanisms.
- **Benchmarks**: Exercise bounded source/host cases and distinguish observed time/proof from conservative production Weight. Passing a fixture is not a universal price or capacity certificate.

## State Witnesses and Repeated Work

Two independent storage keys can reference identical large encoded values. A compatible content-addressed witness may carry that payload once while retaining both authenticated key paths. One changed element in a monolithic vector can require another complete value; equal length or application meaning is not byte equality.

This sharing does not merge ownership or eliminate each operation's decoding, filtering or encoding. Smaller proof does not imply equal CPU/RAM savings, free fees or permission to change Actor Service order. Treat natural sharing as an input to qualified accounting, not as an automatic admission discount.

Complete witness bytes, the recorder's increase over an interval and charged `ProofSize` are different coordinates. Earlier recorded inputs and overlay changes matter; compare only matching roots, formats and baselines. A test fixture must distinguish shared and near-distinct values while keeping logical work separate. Missing physical price evidence remains a limitation, not zero cost.

## Layer 3: Integration (Systemic Truth)

**Location**: `/template/runtime`
**Stack**: Rust / Integration Tests / XCM

This layer tests how pallets and components coordinate inside the full Parachain runtime. End-to-end scenarios, cross-pallet interactions, and XCM tests provide systemic regression evidence without claiming exhaustive safety.

## Validation Priority Rules

- **Targeted Scope**: Run the smallest meaningful validation set for the touched surface first.
- **Escalation**: If a diff crosses layer boundaries (e.g., changes an economic formula), escalate to the full Three-Layer validation.
- **Simulator Mandatory**: Routine support biases toward `/docs`, `/template`, and `/web-client`, but `/simulators/tmctol` becomes mandatory the moment tokenomics or invariant math moves.

## Related

- [Repository Structure](../implementation/repository-structure.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Development Status](status.en.md)
