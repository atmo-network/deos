# Scripts Layer

This directory is the operator/developer automation layer of the DEOS reference stack. In the current repository topology it supports `/docs`, `/template`, and `/web-client`; it is not the primary conceptual control plane.

This directory is intentionally split into two classes:

- `Numbered scripts` are atomic leaf operations. They may depend on `_common.sh`, external tools, and artifacts on disk, but they should not orchestrate other numbered scripts.
- `Named scripts` are orchestrators or admin utilities. They may compose numbered scripts into larger local workflows.
- `Named/admin entrypoints` should follow one shell skeleton: `usage -> parse_args -> check_prerequisites/plan -> main`, with `_common.sh` providing the shared execution harness.
- `All entrypoints` should expose `--help` and keep declared environment/behavior contracts honest.

## Atomic Scripts

- [01-download-binaries.sh](./01-download-binaries.sh)
  Download local Polkadot SDK binaries into `./bin`. The current default release line is `polkadot-v1.22.0`.

- [02-install-tools.sh](./02-install-tools.sh)
  Install local cargo-based tooling (`zombienet`, `chain-spec-builder`, `try-runtime`).

- [03-build-runtime.sh](./03-build-runtime.sh)
  Build the current DEOS reference runtime WASM artifact (`tmctol-runtime`).

- [04-generate-chain-spec.sh](./04-generate-chain-spec.sh)
  Generate and patch `template/chain_spec.json`, including the current local-dev bootstrap surfaces used by the web-client seeding/probe flow (foreign asset, router tracking, native curve).

- [05-spawn-zombienet.sh](./05-spawn-zombienet.sh)
  Spawn the local Zombienet network from the prepared chain spec.

- [06-zombienet-e2e.sh](./06-zombienet-e2e.sh)
  Run the runtime-facing E2E scenario set against a live local network.

- [07-start-web-client.sh](./07-start-web-client.sh)
  Start the local `web-client` Vite dev server in the foreground.

- [08-seed-web-client-state.sh](./08-seed-web-client-state.sh)
  Top up the remaining live local parachain state needed for real `web-client` wallet and swap testing. The current dev/local chain specs are expected to already ship the foreign asset, router tracking, and native curve in genesis, so this script now focuses on Alice funding plus pool/liquidity bootstrapping rather than Root/Sudo-driven runtime setup.

- [09-web-client-live-probe.sh](./09-web-client-live-probe.sh)
  Run live wallet + swap probes against the local parachain using the same Alice/Bob dev identities and signing stack as the web-client.

- [10-web-client-bundle-report.sh](./10-web-client-bundle-report.sh)
  Report the largest generated web-client client-side bundle artifacts, annotate them back to Vite manifest entries/sources plus both static and dynamic manifest importers, surface each top asset's own static/dynamic dependency fan-out, and highlight chunks above a warning threshold.

## Orchestrators

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: binaries -> tools -> runtime build -> chain spec -> web-client dev server -> Zombienet.

- [validate-local.sh](./validate-local.sh)
  Run the local CI/release/E2E validation workflow.

- [aaa-release-gate.sh](./aaa-release-gate.sh)
  Run the heavy AAA scheduler release gate.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `tmctol-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. Supports `--extra` to include AAA circular-chain diagnostics outside the default production-weight set.

- [ci-local.sh](./ci-local.sh)
  Reproduce the local CI workflow.

- [release-local.sh](./release-local.sh)
  Reproduce the local release workflow.

## Admin Utilities

- [check-authorized-upgrade-local.sh](./check-authorized-upgrade-local.sh)
  Read the chain's current canonical governance/runtime authorized-upgrade view, optionally build the local runtime first, hash a local runtime WASM blob, and report whether the local code matches the pending authorized runtime-upgrade hash. It can also emit offline `apply_authorized_upgrade` call data on request. The helper makes the current launch-line role split explicit, emits a machine-readable operator phase (`awaiting-governance-authorization`, `authorized-hash-mismatch`, `ready-to-relay-code`), and remains plan-only without submitting the live call.

- [apply-authorized-upgrade-local.sh](./apply-authorized-upgrade-local.sh)
  Operator-facing companion to the verifier above. It re-checks the currently authorized hash against a local WASM blob and, only with explicit `--submit`, relays matching code bytes through `System.apply_authorized_upgrade { code }` using a local signer URI. Default mode remains plan-only.

- [teardown-local-network.sh](./teardown-local-network.sh)
  Stop local `zombienet` / `polkadot*` / `vite` dev-server processes and remove Zombienet temp directories.

- [clean-local-artifacts.sh](./clean-local-artifacts.sh)
  Remove generated local artifacts (`chain_spec.json`, optionally `target/` and `bin/`).

- [_common.sh](./_common.sh)
  Shared path, logging, timed-step, and background-process helpers used by the orchestrator/admin script layer.
