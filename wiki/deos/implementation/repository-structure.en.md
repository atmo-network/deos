---
type: implementation
title: Repository Structure
description: Detailed description of the DEOS framework repository directories and their purpose.
locale: en
canonical_page_id: repository-structure
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../../AGENTS.md
  - resource: ../../../README.md
status: stable
audience: developer
tags:
  - implementation
  - repository
  - architecture
related:
  - Tech Stack
  - DEOS Framework Overview
---

# Repository Structure

## Summary

The DEOS framework repository is organized into distinct surfaces separating documentation, runtime implementation, testing/simulation, client interfaces, and automation. This structure prevents logic overlap and clarifies where different types of work should occur.

## Core Directories

The project topology follows a strict separation of concerns:

### `/docs/`

A container for independently owned documentation namespaces. `docs/deos/` contains the DEOS contracts, cross-system architecture and reference composition; `docs/README.md` explains neutral placement and navigation. A fork can add its own namespace without modifying the DEOS index. Package and workspace documents remain with their code owners.

### `/template/`

The primary Rust workspace containing the runtime kernel and pallets.

- `/template/runtime/`: The Parachain assembly, connecting pallets and defining weights.
- `/template/pallets/`: The business logic of the network (e.g., `actor`, `asset-registry`, `router`, `governance`, `staking`, `tmc`).
- `/template/primitives/`: Unified types and constants used across pallets to prevent magic numbers.

### `/web-client/`

The reference browser-facing client built with SvelteKit. It provides an `on-chain-first` interface to interact with the DEOS network, separating domain-facing widgets from layout infrastructure.

### `/scripts/`

The automation layer containing Bash scripts for local node management, testing, and deployment. Organized into atomic operations and named orchestrators.

### `/simulators/`

Independent economic models with their own assumptions, tests and usage instructions. The shipped `simulators/tmctol/` model is the JavaScript/BigInt reference for TMCTOL formulas, thresholds and modeled invariants. Other mechanisms may own sibling models; no shared engine or automatic CI enrollment is implied.

### `/wiki/`

The generated bilingual semantic projection of canonical documentation. `wiki/deos/` is the DEOS knowledge bundle; the outer directory provides namespace navigation. Page identity, graph topology, locales and source provenance remain separate from storage layout.

### `/.agents/`

The repository-local cognitive and validation infrastructure layer containing portable skills, audits, and project alignment workflows.

## Operational Priority

Day-to-day work usually flows through these directories in a specific order:

1. Establish intent in the owning documentation namespace, `docs/deos/` for DEOS
2. Implement in `/template/`
3. Expose in `/web-client/`
4. Automate in `/scripts/`

When dealing with core economic logic, the path shifts to validate math in `/simulators/tmctol/` first.

## Related

- [Tech Stack](tech-stack.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Runtime Patterns](../overview/runtime-patterns.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Scripts Layer](../usage/scripts-layer.en.md)
