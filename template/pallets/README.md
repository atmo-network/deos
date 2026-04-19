# Pallets Directory

This directory contains the custom pallets that implement the DEOS runtime kernel and its current reference economic configuration. Each pallet is designed with modern FRAME patterns and follows the project's architectural principles for production-ready blockchain applications.

The current pallet set is maintained against the repository's `Polkadot SDK 2603 / node 1.22.0` line. Most of the 2603 migration fallout landed in runtime/session/XCM integration surfaces rather than in pallet cores, and the pallet-local README files now record where pallet-specific cleanup was or was not needed.

## 🏗️ Available Pallets

### [AAA](./aaa/README.md)

Deterministic account-abstraction actor runtime for bounded execution plans, scheduling, event-driven triggers, and lifecycle management.

### [Asset Registry](./asset-registry/README.md)

Governance-controlled registry for foreign assets and XCM location mappings.

### [Axial Router](./axial-router/README.md)

Multi-AMM trading infrastructure providing intelligent routing across different automated market makers. Implements trait-based architecture for extensible AMM support with optimal price discovery and execution.

### [Governance](./governance/README.md)

Bounded governance reward-memory kernel for winning-vote sliding windows, item-scoped uniqueness within the live reward window, bounded resolution-once item ingress, a bounded active proposal lifecycle, bounded ballot casting with runtime-configured weight/policy, bounded automatic proposal finalization, bounded batch winner ingress, sparse zero-sum eviction, and exported reward coefficients.

### [Token Minting Curve](./tmc/README.md)

Unidirectional minting pallet implementing the TMCTOL standard's curve mechanics on DEOS.

### [Staking](./staking/README.md)

Multi-asset share-vault staking pallet with one sovereign backing channel per registered asset, proportional inflow distribution via share ownership, and a planned second governance-conditioned reward channel that compounds into fresh same-asset `stXXX`.

## 🎯 Pallet Architecture Philosophy

Our pallets implement several key architectural patterns:

- **Trait-Based Extensibility**: Clean interfaces for cross-pallet communication
- **KISS Principle**: Simple, maintainable implementations that scale
- **Production-Ready Design**: Economic security, error handling, and operational excellence
- **Automated Execution**: Deterministic scheduling, bounded execution, and event-driven reaction capabilities

## 📚 Technical Implementation Guides

For detailed technical implementation, architectural decisions, and production deployment patterns, see the comprehensive guides in the [documentation directory](../../docs/):

- **[Axial Router Architecture Guide](../../docs/axial-router.architecture.en.md)** - Modern multi-token routing system optimized for TMC ecosystems
- **[Randomness Strategy](../../docs/randomness.strategy.en.md)** - Post-VRF simplification note covering the relay-beacon-first direction and the conditions for replacing local entropy logic with a relay-chain beacon adapter
- **[Staking Specification](../../docs/staking.specification.en.md)** - Multi-asset share-vault staking contract with per-asset sovereign pool accounts, lazy sync, share-based ownership accounting, and isolated native delegation for the security path

## 🚀 Quick Start

Each pallet directory contains:

- **Source code** (`src/`) with comprehensive implementation
- **Local README** with pallet-specific orientation and quick reference
- **Tests** demonstrating functionality and integration patterns

Direct local entrypoints:

- [AAA README](./aaa/README.md)
- [Asset Registry README](./asset-registry/README.md)
- [Axial Router README](./axial-router/README.md)
- [Governance README](./governance/README.md)
- [Staking README](./staking/README.md)
- [TMC README](./tmc/README.md)

Navigate to individual pallet directories for component-specific orientation and development guidance.

## 🔧 Development Integration

These pallets are designed for seamless integration with the runtime configuration located in [`../runtime/src/configs/`](../runtime/src/configs/mod.rs). The modular design enables flexible deployment scenarios while maintaining architectural consistency.

For development workflow and contribution guidelines, refer to the [Documentation index](../../docs/README.md) for comprehensive technical guides and architectural patterns.
