# Scripts Layer

This directory is the operator/developer automation layer of the DEOS reference stack. In the current repository topology it supports `/docs`, `/template`, and `/web-client`; it is not the primary conceptual control plane.

This directory is intentionally split into two classes:

- `Numbered scripts` are atomic leaf operations. They may depend on `_common.sh`, external tools, and artifacts on disk, but they should not orchestrate other numbered scripts.
- `Named scripts` are orchestrators or admin utilities. They may compose numbered scripts into larger local workflows.
- `Named/admin entrypoints` should follow one shell skeleton: `usage -> parse_args -> check_prerequisites/plan -> main`, with `_common.sh` providing the shared execution harness.
- `All entrypoints` should expose `--help` and keep declared environment/behavior contracts honest.

## Atomic Scripts

- [01-download-binaries.sh](./01-download-binaries.sh)
  Download local Polkadot SDK binaries into `./bin`. The current default release tag is `polkadot-stable2603-1` (node v1.22.1).

- [02-install-tools.sh](./02-install-tools.sh)
  Install local cargo-based tooling (`zombienet`, `chain-spec-builder`, `try-runtime`).

- [03-build-runtime.sh](./03-build-runtime.sh)
  Build the current DEOS reference runtime WASM artifact (`deos-runtime`).

- [04-generate-chain-spec.sh](./04-generate-chain-spec.sh)
  Generate and patch `template/chain_spec.json`, including the current local-dev bootstrap surfaces used by the web-client seeding/probe flow (foreign asset, router tracking, native curve).

- [05-spawn-zombienet.sh](./05-spawn-zombienet.sh)
  Spawn the local Zombienet network from the prepared chain spec.

- [06-zombienet-e2e.sh](./06-zombienet-e2e.sh)
  Run the runtime-facing E2E scenario set against a live local network.

- [07-seed-web-client-state.sh](./07-seed-web-client-state.sh)
  Top up the remaining live local parachain state needed for real `web-client` wallet, swap, and native-staking testing. The current dev/local chain specs are expected to already ship the foreign asset, local native-staking asset, `stNTVE` receipt, router tracking, native curve, and staking registration in genesis, so this script focuses on Alice funding plus native/foreign and `NTVE/stNTVE` pool/liquidity bootstrapping rather than Root/Sudo-driven runtime setup.

## Orchestrators

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: binaries -> tools -> runtime build -> chain spec -> Zombienet. Start the web client directly from `web-client` with `npm run dev`.

- [validate-local.sh](./validate-local.sh)
  Run the local CI/build/E2E validation workflow.

- [aaa-release-gate.sh](./aaa-release-gate.sh)
  Run the heavy AAA scheduler stress gate used by the scheduled stress lane.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `deos-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

- [audit-template-readiness.sh](./audit-template-readiness.sh)
  Run lightweight static checks for template launch-readiness smells such as fallback XCM weights, unclassified runtime weight placeholders, stale staking aliases, and asset-conversion test naming drift.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. Supports `--extra` to include AAA circular-chain diagnostics outside the default production-weight set.

- [ci-local.sh](./ci-local.sh)
  Reproduce the local CI workflow.

## Admin Utilities

- [export-papi-metadata.sh](./export-papi-metadata.sh)
  Export native runtime metadata through the committed `deos-runtime` metadata example and optionally regenerate the web-client PAPI descriptors. This replaces the old ad hoc temporary-test metadata export workflow.

- [bootstrap-native-staking-local.sh](./bootstrap-native-staking-local.sh)
  Consolidated native staking bootstrap helper. `check` reads live readiness for the canonical `NTVE/stNTVE` pool and Native Staking LP Farmer skeleton; `prepare-calls` emits plan-only Root/governance or signed-operator call data for staking registration, pool creation, and liquidity seeding. It never signs or submits transactions.

- [authorized-upgrade-local.sh](./authorized-upgrade-local.sh)
  Consolidated authorized runtime-upgrade helper. `check` verifies a local WASM hash against the chain's pending authorized hash and can emit offline call data; `apply` stays plan-only unless explicit `--submit` relays matching code bytes through `System.apply_authorized_upgrade { code }`.

- [teardown-local-network.sh](./teardown-local-network.sh)
  Stop local `zombienet` / `polkadot*` / `vite` dev-server processes and remove Zombienet temp directories.

- [clean-local-artifacts.sh](./clean-local-artifacts.sh)
  Remove generated local artifacts (`chain_spec.json`, optionally `target/` and `bin/`).

- [_common.sh](./_common.sh)
  Shared path, logging, timed-step, and background-process helpers used by the orchestrator/admin script layer.
