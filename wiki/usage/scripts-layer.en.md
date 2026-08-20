---
type: usage
title: Scripts Layer
description: Operator and developer automation workflows using the local scripts layer, including local bootstrap, runtime metadata export, authorized-upgrade checks, and native staking bootstrap readiness/call-preparation helpers.
locale: en
canonical_page_id: scripts-layer
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../scripts/README.md
status: stable
audience: developer
tags:
  - usage
  - automation
  - tooling
related:
  - Repository Structure
  - DEOS Framework Overview
---

# Scripts Layer

## Summary

The `/scripts` directory provides operator and developer automation for the DEOS reference stack. It contains atomic bash scripts, high-level orchestrators, and admin utilities to simplify the lifecycle of building, testing, deploying, probing, and maintaining the DEOS parachain.

## Script Classifications

The architecture intentionally splits automation into two distinct classes to maintain predictability.

### Atomic Scripts

Numbered scripts perform specific leaf operations and do not orchestrate each other. Their contiguous order follows the local-network evidence ladder. They handle direct tasks such as:

- `01-download-binaries.sh`: Download and checksum-verify the pinned Polkadot, Omni Node, and worker bundle
- `02-install-tools.sh`: Install the required Cargo-based operator tools
- `03-build-runtime.sh`: Compile the WASM artifact
- `04-generate-chain-spec.sh`: Generate a verified ChainSpec directly from the complete runtime-owned preset
- `05-spawn-zombienet.sh`: Launch the local network
- `06-network-smoke.sh`: Observe bounded finalized relay and parachain progress
- `07-network-e2e.sh`: Prove one signed finalized transfer against live event and storage truth
- `08-session-transition.sh`: Observe one finalized session transition through both collator RPC views
- `09-composed-economic-path.sh`: Reconcile a finalized Router, Oracle, and Burn Actor execution path against events and storage

### Orchestrators

Named workflow scripts compose atomic steps into larger developer flows:

- `bootstrap-local-network.sh`: Build the runtime, generate the spec, and spin up the local chain and web client
- `validate-local.sh`: Run the selected local audit, build, and E2E validation plan
- `actors-assurance.sh`: Run heavy stress and capacity proofs for the Actors scheduler
- `network-assurance-local.sh`: Compose topology, finality, failover, restart, and signed-transfer evidence; `SESSION_TRANSITION=1` adds the multi-hour session proof and `COMPOSED_PATH=1` adds finalized Router, Oracle, and Burn Actor evidence
- `benchmarks.sh`: Run runtime benchmark compilation and weight-generation flows

## Admin Utilities

Admin scripts assist operators in managing local or live-chain readiness without hiding authority boundaries.

Important examples include:

- `seed-web-client-state.sh`: Prepare wallet, swap, and native-staking state for live web-client testing
- `export-papi-metadata.sh`: Export Rust runtime metadata and regenerate PAPI descriptors for the web client
- `bootstrap-native-staking-local.sh check`: Read native staking bootstrap readiness without submitting transactions
- `bootstrap-native-staking-local.sh prepare-calls`: Emit the next plan-only Root/governance staking-admin or signed operator call data needed to register/initialize native staking, create the canonical `NTVE/stNTVE` pool, or seed initial liquidity
- `authorized-upgrade-local.sh check`: Pin finalized runtime identity, compare live and local code, inspect strategic submission authority and `$VETO` issuance, and verify any pending authorized hash without submitting
- `authorized-upgrade-local.sh prepare-authorization`: Emit candidate-bound stake, preimage, and strategic proposal call data without signing; protection `Pass` remains unavailable until the lifecycle is ready
- `authorized-upgrade-local.sh apply`: Relay already-authorized runtime code bytes only with explicit `--submit`
- `authorized-upgrade-local.sh snapshot|verify`: Capture finalized non-empty baseline state and verify exact Router, Oracle, Actors, runtime-version, and candidate-code preservation after an upgrade
- `teardown-local-network.sh`: Safely terminate background processes and remove temporary network state

## Native Staking Bootstrap Helpers

The native staking bootstrap path is split into two operator-safe tools:

1. `bootstrap-native-staking-local.sh prepare-calls` reads live state and prepares the next call data for the production/operator path
2. `bootstrap-native-staking-local.sh check` verifies that the canonical `NTVE/stNTVE` pool, native staking exchange rate, and dormant Native Staking Liquidity Actor are ready

Both helpers are plan/read-only by default. The preparation helper never signs or submits transactions; it only emits call data plus the expected authority for each step.

## Shared Conventions

All named/admin scripts follow a consistent shell skeleton:

1. `usage`
2. `parse_args`
3. `check_prerequisites/plan`
4. `main`

They rely on `_common.sh` for logging, step tracking, and background process management, ensuring a uniform developer experience. All scripts support the `--help` flag for detailed usage instructions.

## Related

- [Repository Structure](../implementation/repository-structure.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [Development Status](../development/status.en.md)
