---
page_type: implementation
title: Repository Structure
summary: Detailed description of the DEOS framework repository directories and their purpose.
locale: en
canonical_page_id: repository-structure
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../README.md
status: active
audience: developer
tags:
  - implementation
  - repository
  - architecture
related:
  - Tech Stack
  - DEOS Framework Overview
last_compiled: 2026-07-20
confidence: 0.9
---

# Repository Structure

## Summary

The DEOS framework repository is organized into distinct surfaces separating documentation, runtime implementation, testing/simulation, client interfaces, and automation. This structure prevents logic overlap and clarifies where different types of work should occur.

## Core Directories

The project topology follows a strict separation of concerns:

### `/docs/`

The Knowledge Base and conceptual control plane. Contains normative contracts, architecture guides, specifications, and mathematical rationales.

### `/template/`

The primary Rust workspace containing the runtime kernel and pallets.

- `/template/runtime/`: The Parachain assembly, connecting pallets and defining weights.
- `/template/pallets/`: The business logic of the network (e.g., `aaa`, `asset-registry`, `axial-router`, `governance`, `staking`, `tmc`).
- `/template/primitives/`: Unified types and constants used across pallets to prevent magic numbers.

### `/web-client/`

The reference browser-facing client built with SvelteKit. It provides an `on-chain-first` interface to interact with the DEOS network, separating domain-facing widgets from layout infrastructure.

### `/scripts/`

The automation layer containing Bash scripts for local node management, testing, and deployment. Organized into atomic operations and named orchestrators.

### `/simulator/`

The historical and authoritative mathematical proving ground written in JavaScript/BigInt. This is the source of truth for all tokenomic formulas, thresholds, and invariants before they are implemented in Rust.

### `/wiki/`

The generated bilingual semantic projection of `/docs`, with frontend-renderable pages and shared navigation, graph, alias, locale, confidence, and provenance metadata.

### `/.agents/`

The repository-local cognitive and validation infrastructure layer containing portable skills, audits, and project alignment workflows.

## Operational Priority

Day-to-day work usually flows through these directories in a specific order:

1. Establish intent in `/docs/`
2. Implement in `/template/`
3. Expose in `/web-client/`
4. Automate in `/scripts/`

When dealing with core economic logic, the path shifts to validate math in `/simulator/` first.

## Related

- [Tech Stack](tech-stack.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Runtime Patterns](../overview/runtime-patterns.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Scripts Layer](../usage/scripts-layer.en.md)
- [Agent Coordination](../usage/agent-coordination.en.md)
