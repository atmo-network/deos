# Scripts Layer

This directory is the operator/developer automation layer of the DEOS reference stack. In the current repository topology it supports `/docs`, `/template`, and `/web-client`; it is not the primary conceptual control plane.

This directory contains deterministic command surfaces, not agent strategy:

- `Numbered scripts` are reusable atomic operations. They may depend on `_common.sh`, external tools, and artifacts on disk, but they do not orchestrate other numbered scripts.
- `Named implementations` are deterministic operator utilities or compositions whose mode and outcome need no agent interpretation.
- `Shared implementations` stay here whenever humans, GitHub Actions, CI, root compositions, or multiple skills invoke them.
- `Agent-owned workflows` live under `/.agents/skills/<domain>` when they require scope selection, coordination, evidence interpretation, knowledge synchronization, or handoff judgment; they call public root scripts for shared execution.
- `Full named/admin implementations` follow `usage -> parse_args -> check_prerequisites/plan -> main`.
- `All entrypoints` expose `--help` and keep declared environment/behavior contracts honest.

## Executable Ownership Inventory

Path classes provide the inventory without duplicating the per-command map below:

| Path class | Classification and owner | Consumer contract |
| --- | --- | --- |
| `/scripts/[0-9][0-9]-*.sh` | Shared human-callable atoms; root scripts layer | Humans, CI, workflows, and skills call the owning file directly |
| `/scripts/<name>.sh` | Shared deterministic compositions/admin utilities; root scripts layer | Shared consumers call one canonical implementation; `_common.sh` remains support-only |
| `/.agents/skills/alignment/scripts/*` | Project audit and completion capability; `alignment` | Root validation and agents may call the public audit contract; no audit implementation belongs in `/scripts` |
| `/.agents/skills/domain-dag/scripts/*` | Portable graph-validator capability; `domain-dag` | Package bridges and agents call the owning validator rather than copy its rules |
| `/.agents/skills/wiki-sync/scripts/*` | Portable wiki trust/consolidation capability; `wiki-sync` | Package bridges and agents call the owning validator rather than copy its rules |
| `/web-client/scripts/*.mjs` | Client-package entrypoints or thin capability bridges; web client | npm owns invocation; bridges contain no duplicated validator semantics |

GitHub workflows invoke root shared implementations only. Skills never call sibling skill internals, support files are not public entrypoints, and a consumer references the canonical owner rather than maintaining a second executable copy.

## Human-Callable Atomic Scripts

Each numbered command is independently callable by a human or CI from any working directory. Its `--help` declares inputs, outputs, side effects, and configurable environment. The command checks its own prerequisites and never invokes another numbered command. Numbers show the common local-network sequence only; they do not create a hidden requirement to run earlier scripts.

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

## Deterministic Compositions

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: binaries -> tools -> runtime build -> chain spec -> Zombienet. Start the web client directly from `web-client` with `npm run dev`.

- [validate-local.sh](./validate-local.sh)
  Run only explicitly selected broad compositions for audits, CI, runtime build, dependency posture, or E2E. No plan runs by default: agents choose changed-scope routes through `alignment`, while humans/releases opt into `--audit-only`, one `--*-only` mode, or `--all`. Audit leaves remain owned by `alignment` and are orchestrated here without duplicating their implementation.

- [aaa-release-gate.sh](./aaa-release-gate.sh)
  Shared human/GitHub/CI/skill implementation of the AAA scheduler stress and independent-runtime embedding gate. The `aaa-delivery` skill owns quick/full selection, occupancy policy, evidence interpretation, and delivery handoff without duplicating execution.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `deos-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

Project-local audit leaves and targeted routes are documented in `/.agents/skills/alignment/SKILL.md`. Use the diff-aware completion gate by default; reserve `./scripts/validate-local.sh --audit-only` for an explicitly broad audit pass.

Commands executed through the shared script harness use compact output by default: successful test, build, lint, documentation, metadata, and benchmark steps print only their label, duration, and result. A failed step prints the last 80 lines and retains its complete output in a temporary log whose path appears in the error. Set `DEOS_VERBOSE=1` to restore live full output, or set `DEOS_FAILURE_TAIL_LINES=N` to change the failure excerpt without enabling verbose mode.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. Supports `--extra` for AAA diagnostics, `--extrinsic NAME --output FILE` for focused evidence that must not replace complete production weights, and `--skip-build` when reusing a freshly built benchmark runtime. The [`benchmarking` skill](../.agents/skills/benchmarking/SKILL.md) owns case selection, evidence interpretation, weight handoff, and claim boundaries without duplicating this command surface.

- [ci-local.sh](./ci-local.sh)
  Reproduce local CI or select one compact check with `--only`; narrow Cargo work further with `--package NAME`, `--test-filter NAME`, and explicit feature mode. Apply Rust formatting with `--only format --fix`. Agents should prefer this entrypoint over raw Cargo commands.

## Admin Utilities

- [seed-web-client-state.sh](./seed-web-client-state.sh)
  Idempotently prepare the composite live-chain state needed for local wallet, swap, and native-staking UI testing: verify genesis prerequisites, fund Alice, and create or top up the Native/foreign and `NTVE/stNTVE` pools. This is a named admin workflow rather than an atomic numbered leaf because it coordinates several state checks and transactions.

- [export-papi-metadata.sh](./export-papi-metadata.sh)
  Export native runtime metadata through the committed `deos-runtime` metadata example and optionally regenerate the web-client PAPI descriptors. This replaces the old ad hoc temporary-test metadata export workflow.

- [bootstrap-native-staking-local.sh](./bootstrap-native-staking-local.sh)
  Consolidated native staking bootstrap helper. `check` reads live readiness for the canonical `NTVE/stNTVE` pool and Native Staking LP Farmer skeleton; `prepare-calls` emits plan-only Root/governance or signed-operator call data for staking registration, pool creation, and liquidity seeding. It never signs or submits transactions. The [`staking-delivery` skill](../.agents/skills/staking-delivery/SKILL.md) owns readiness sequencing, authority boundaries, and activation handoff without duplicating this command.

- [authorized-upgrade-local.sh](./authorized-upgrade-local.sh)
  Consolidated authorized runtime-upgrade helper. `check` verifies a local WASM hash against the chain's pending authorized hash and can emit offline call data; `apply` stays plan-only unless explicit `--submit` relays matching code bytes through `System.apply_authorized_upgrade { code }`. The [`upgrade-delivery` skill](../.agents/skills/upgrade-delivery/SKILL.md) owns evidence sequencing, approval boundaries, and post-upgrade handoff without duplicating this command.

- [teardown-local-network.sh](./teardown-local-network.sh)
  Stop local `zombienet` / `polkadot*` / `vite` dev-server processes and remove Zombienet temp directories.

- [clean-local-artifacts.sh](./clean-local-artifacts.sh)
  Remove generated local artifacts (`chain_spec.json`, optionally `target/` and `bin/`).

- [_common.sh](./_common.sh)
  Shared path, logging, timed-step, and background-process helpers used by deterministic root commands and project-skill script leaves. A co-located skill script supplies `DEOS_PROJECT_ROOT` before sourcing it.
