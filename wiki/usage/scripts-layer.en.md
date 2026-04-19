---
page_type: usage
title: Scripts Layer
summary: Operator and developer automation workflows using the local scripts layer.
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
last_compiled: 2026-04-15
confidence: 0.95
---

# Scripts Layer

## Summary

The `/scripts` directory provides operator and developer automation for the DEOS reference stack. It contains atomic bash scripts, high-level orchestrators, and admin utilities to simplify the lifecycle of building, testing, deploying, and maintaining the DEOS parachain.

## Script Classifications

The architecture intentionally splits automation into two distinct classes to maintain predictability:

### Atomic Scripts (Numbered)

These perform specific leaf operations and do not orchestrate each other. They handle direct tasks such as:

- `01-download-binaries.sh`: Fetch Polkadot SDK binaries.
- `03-build-runtime.sh`: Compile the WASM artifact.
- `05-spawn-zombienet.sh`: Launch the local network.
- `08-seed-web-client-state.sh`: Bootstraps initial state, accounts, and pools for local testing.

### Orchestrators (Named)

These compose atomic scripts into larger developer workflows.

- `bootstrap-local-network.sh`: The main entrypoint that builds the runtime, generates the spec, and spins up the local chain and web client.
- `validate-local.sh`: Runs the local CI, release checks, and E2E validation.
- `aaa-release-gate.sh`: Runs heavy stress tests for the AAA scheduler.

## Admin Utilities

Admin scripts assist operators in managing the live chain or local environment.

- `check-authorized-upgrade-local.sh`: Verifies if the locally compiled WASM hash matches the pending authorized runtime upgrade on-chain.
- `apply-authorized-upgrade-local.sh`: Submits the authorized upgrade blob.
- `teardown-local-network.sh`: Safely terminates background processes and removes temporary network state.

## Shared Conventions

All named/admin scripts follow a consistent shell skeleton:

1. `usage`
2. `parse_args`
3. `check_prerequisites/plan`
4. `main`

They rely on `_common.sh` for logging, step tracking, and background process management, ensuring a uniform developer experience. All scripts support the `--help` flag for detailed usage instructions.
