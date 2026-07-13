---
page_type: usage
title: Scripts Layer
summary: Operator and developer automation workflows using the local scripts layer, including local bootstrap, runtime metadata export, authorized-upgrade checks, and native staking bootstrap readiness/call-preparation helpers.
locale: en
canonical_page_id: scripts-layer
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../scripts/README.md
status: active
audience: developer
tags:
  - usage
  - automation
  - tooling
related:
  - Repository Structure
  - DEOS Framework Overview
last_compiled: 2026-04-25
confidence: 0.96
---

# Scripts Layer

## Summary

The `/scripts` directory provides operator and developer automation for the DEOS reference stack. It contains atomic bash scripts, high-level orchestrators, and admin utilities to simplify the lifecycle of building, testing, deploying, probing, and maintaining the DEOS parachain.

## Script Classifications

The architecture intentionally splits automation into two distinct classes to maintain predictability.

### Atomic Scripts

Numbered scripts perform specific leaf operations and do not orchestrate each other. They handle direct tasks such as:

- `01-download-binaries.sh`: Fetch Polkadot SDK binaries
- `03-build-runtime.sh`: Compile the WASM artifact
- `05-spawn-zombienet.sh`: Launch the local network
- `seed-web-client-state.sh`: Top up local wallet, swap, and native-staking state for live web-client testing

### Orchestrators

Named workflow scripts compose atomic steps into larger developer flows:

- `Bootstrap-local-network.sh`: Build the runtime, generate the spec, and spin up the local chain and web client
- `Validate-local.sh`: Run the local CI, runtime build, and E2E validation
- `Aaa-release-gate.sh`: Run heavy stress tests for the AAA scheduler
- `Benchmarks.sh`: Run runtime benchmark compilation and weight-generation flows

## Admin Utilities

Admin scripts assist operators in managing local or live-chain readiness without hiding authority boundaries.

Important examples include:

- `Export-papi-metadata.sh`: Export Rust runtime metadata and regenerate PAPI descriptors for the web client
- `Bootstrap-native-staking-local.sh check`: Read native staking bootstrap readiness without submitting transactions
- `Bootstrap-native-staking-local.sh prepare-calls`: Emit the next plan-only Root/governance staking-admin or signed operator call data needed to register/initialize native staking, create the canonical `NTVE/stNTVE` pool, or seed initial liquidity
- `Authorized-upgrade-local.sh check`: Verify if the locally compiled WASM hash matches the pending authorized runtime upgrade on-chain
- `Authorized-upgrade-local.sh apply`: Relay already-authorized runtime code bytes only with explicit `--submit`
- `Teardown-local-network.sh`: Safely terminate background processes and remove temporary network state

## Native Staking Bootstrap Helpers

The native staking bootstrap path is split into two operator-safe tools:

1. `bootstrap-native-staking-local.sh prepare-calls` reads live state and prepares the next call data for the production/operator path
2. `bootstrap-native-staking-local.sh check` verifies that the canonical `NTVE/stNTVE` pool, native staking exchange rate, and dormant native staking LP provisioning actor are ready

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
- [Validation Troubleshooting](validation-troubleshooting.en.md)
- [Development Status](../development/status.en.md)
