# Scripts Layer

This directory is the operator/developer automation layer of the DEOS reference stack. In the current repository topology it supports `/docs`, `/template`, and `/web-client`; it is not the primary conceptual control plane.

This directory is intentionally split into two classes:

- `Numbered scripts` are atomic leaf operations. They may depend on `_common.sh`, external tools, and artifacts on disk, but they should not orchestrate other numbered scripts.
- `Named scripts` are orchestrators or admin utilities. They may compose numbered scripts into larger local workflows.
- `Named/admin entrypoints` should follow one shell skeleton: `usage -> parse_args -> check_prerequisites/plan -> main`, with `_common.sh` providing the shared execution harness.
- `All entrypoints` should expose `--help` and keep declared environment/behavior contracts honest.

## Atomic Scripts

- [01-download-binaries.sh](./01-download-binaries.sh)
  Download local Polkadot SDK binaries into `./bin`. The current default release tag is `polkadot-stable2606` (node v1.24.0).

- [02-install-tools.sh](./02-install-tools.sh)
  Install local cargo-based tooling (`zombienet`, `chain-spec-builder`, `try-runtime`).

- [03-build-runtime.sh](./03-build-runtime.sh)
  Build the current DEOS reference runtime WASM artifact (`deos-runtime`) from the locked dependency graph and repository-pinned Rust toolchain, then report its size and SHA-256 digest.

- [04-generate-chain-spec.sh](./04-generate-chain-spec.sh)
  Generate and patch `template/chain_spec.json`, including the current local-dev bootstrap surfaces used by the web-client seeding/probe flow (foreign asset, router tracking, native curve).

- [05-spawn-zombienet.sh](./05-spawn-zombienet.sh)
  Spawn the local Zombienet network from the prepared chain spec.

- [06-zombienet-e2e.sh](./06-zombienet-e2e.sh)
  Run the runtime-facing E2E scenario set against a live local network.

## Orchestrators

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: binaries -> tools -> runtime build -> chain spec -> Zombienet. Start the web client directly from `web-client` with `npm run dev`.

- [validate-local.sh](./validate-local.sh)
  Run the local script-entrypoint/template-readiness/numeric-parsing/simulator-determinism/simulator-consistency/code-suppression/backlog/release-line/portability/domain-DAG/wiki-trust/dependency/CI/build/E2E validation workflow. The fast audit leaves live under the repo-local `alignment` skill and are orchestrated from here. Use `--audit-only` for the fast local audit stack and `--dependency-audit` when network-backed npm posture checks are desired.

- [aaa-release-gate.sh](./aaa-release-gate.sh)
  Run the heavy AAA scheduler stress gate used by the scheduled stress lane.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `deos-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

Project-local audit leaves are documented in `/.agents/skills/alignment/SKILL.md` and are normally reached through `./scripts/validate-local.sh --audit-only`.

Commands executed through the shared script harness use compact output by default: successful test, build, lint, documentation, metadata, and benchmark steps print only their label, duration, and result. A failed step prints the last 80 lines and retains its complete output in a temporary log whose path appears in the error. Set `DEOS_VERBOSE=1` to restore live full output, or set `DEOS_FAILURE_TAIL_LINES=N` to change the failure excerpt without enabling verbose mode.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. Supports `--extra` for AAA diagnostics, `--extrinsic NAME --output FILE` for focused evidence that must not replace complete production weights, and `--skip-build` when reusing a freshly built benchmark runtime.

- [ci-local.sh](./ci-local.sh)
  Reproduce the local CI workflow, run one compact validation class with `--only clippy|tests|docs|format|check`, or apply Rust formatting with `--only format --fix`. Agents should prefer this entrypoint over raw Cargo validation commands.

## Admin Utilities

- [seed-web-client-state.sh](./seed-web-client-state.sh)
  Idempotently prepare the composite live-chain state needed for local wallet, swap, and native-staking UI testing: verify genesis prerequisites, fund Alice, and create or top up the Native/foreign and `NTVE/stNTVE` pools. This is a named admin workflow rather than an atomic numbered leaf because it coordinates several state checks and transactions.

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
