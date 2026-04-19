---
page_type: process
title: Contributing Guidelines
summary: How to contribute to the DEOS framework, understand the philosophy, and navigate the validation gates.
locale: en
canonical_page_id: contributing
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
status: active
audience: developer
tags:
  - community
  - contributing
  - workflow
related:
  - Three-Layer Validation
  - DEOS Framework Overview
last_compiled: 2026-04-15
confidence: 0.95
---

# Contributing Guidelines

## Summary

Contributing to the DEOS framework requires understanding its dual nature: it is both a mathematical economic engine and a modern Polkadot SDK Parachain runtime. Contributions must pass rigorous validation and respect the "Physics-First" philosophy.

## Before Writing Code

### 1. Understand the Philosophy

The DEOS framework is built on a "Physics-First" paradigm. Code should reinforce structural economic guarantees (the "physics") rather than implementing "politics-by-default" (manual interventions).

### 2. Read the Normative Contracts

Before touching the `/template` (runtime) or `/web-client` (frontend), you must read the authoritative contracts in `/docs`.

- Implementation follows the spec, not the other way around.
- See `docs/README.md` for the index of required reading, especially the `Polkadot SDK 2603 Best Practices`.

## Development Workflow

### The Adoption Model

DEOS is designed as a "Foundation in a Box". Downstream ecosystems are expected to fork DEOS to build their products. Upstream contributions to DEOS should focus on:

- Framework hardening
- Read-model improvements
- Tooling and Scripts
- Universal stabilizations

Business logic should stay in downstream forks, not accrete inside the core repo.

### Zero Warnings Policy

All Rust code must compile with zero `clippy` warnings. The repository maintains a strict linting hygiene tracker matching the upstream Polkadot SDK.

### Three-Layer Validation

If your PR touches economic logic, it must pass all three layers of validation:

1. **Simulation**: JavaScript/BigInt math checks (`/simulator`)
2. **Implementation**: Rust unit tests and benchmarks (`/template/pallets`)
3. **Integration**: Parachain E2E and XCM tests (`/template/runtime`)

## Documentation Discipline

When contributing features or fixes, you are also responsible for the knowledge sync:

1. Update the authoritative `/docs` if logic changes.
2. The `/wiki` will be automatically recompiled from `/docs` by the `wiki-sync` agent, or you can run the sync manually.
3. Keep the terminology exact. Do not use colloquial terms (e.g., "agents" or "smart contracts") for precise architecture concepts like "Account Abstraction Actors" (AAA) or "Pallets".

## Getting Help

Check the `Scripts Layer` for local automation tools to easily spin up a dev environment, seed state, and run tests. Use the issue tracker for architectural discussions before starting large PRs.
