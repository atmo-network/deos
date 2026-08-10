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

- [02-install-tools.sh](./02-install-tools.sh)
  Install local cargo-based tooling (`zombienet`, `chain-spec-builder`, `try-runtime`).

- [03-build-runtime.sh](./03-build-runtime.sh)
  Stage the template at one fixed fail-closed physical build root, build the current `deos-runtime` Wasm from the locked graph and pinned Rust toolchain with the existing fat-LTO/single-codegen-unit production profile, remap source/Cargo/Rustup roots to canonical virtual prefixes, atomically publish only the successful artifact, then report its size and SHA-256 digest.

- [04-generate-chain-spec.sh](./04-generate-chain-spec.sh)
  Generate and patch a selected chain-spec output from a selected DEOS runtime Wasm, including the current local economic bootstrap state. Isolated temporary generation prevents an alternate baseline artifact from overwriting `template/chain_spec.json` before successful output selection.

- [05-spawn-zombienet.sh](./05-spawn-zombienet.sh)
  Spawn the local Zombienet network from a selected work directory, topology, and matching chain spec. Defaults remain the current `template` candidate; deployed downstream runtimes may select an explicit persisted work directory and matching upgrade artifact after `1.0`.

- [06-network-smoke.sh](./06-network-smoke.sh)
  Observe bounded finalized progression from the relay chain and both configured collator RPC views without submitting state transitions. This smoke check never substitutes for collator-author participation or the open composed-path acceptance contract.

- [07-network-e2e.sh](./07-network-e2e.sh)
  Submit one signed Alice-to-Bob native transfer through a running node and verify finalized dispatch success, the live transfer event, and finalized recipient storage. This proves the network assertion path but not the open Router/Oracle/Actors composed scenario.

- [08-session-transition.sh](./08-session-transition.sh)
  Observe one finalized session-index transition through both collator RPC views, requiring continued finalized progress and an equal non-empty validator set. The read-only proof may run for several hours.

- [09-composed-economic-path.sh](./09-composed-economic-path.sh)
  Prepare local pool state, execute one finalized Native-to-foreign Router swap, and reconcile Router and Oracle events, exact pool and balance deltas, one Burn Actor cycle, no duplicate execution, and native issuance reduction from finalized storage.

## Deterministic Compositions

- [setup-environment.sh](./setup-environment.sh)
  Prepare the repository-pinned Rust environment, or clear generated SvelteKit state and install the pinned Node/npm client from its lockfile; full mode prepares both environments.

- [network-assurance-local.sh](./network-assurance-local.sh)
  Prepare pinned tools/artifacts, spawn the canonical two-validator/two-collator topology, verify both collators prepare blocks, pause one collator while the other preserves finality, restart the second collator from its existing base path, and execute signed finalized transfers before and after restart. `SESSION_TRANSITION=1` adds the multi-hour finalized session proof, while `COMPOSED_PATH=1` adds the mutating Router/Oracle/Burn Actor proof; the default composition claims neither. No mode claims runtime-upgrade acceptance.

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: tools -> runtime build -> chain spec -> Zombienet, using Polkadot and Omni Node binaries already available in `PATH` or `./bin`. Start the web client directly from `web-client` with `npm run dev`.

- [validate-local.sh](./validate-local.sh)
  Run the canonical `fast`, `heavy`, or fail-closed `full` release-validation profile. The entrypoint owns stage inventory and pass/fail semantics while distinct proof implementations remain with their package, script, or alignment owner.

- [actors-assurance.sh](./actors-assurance.sh)
  Shared Actors proof contract for semantic-manifest and fee-envelope-vector freshness, cross-language semantics, scheduler stress, capacity, and independent-runtime embedding. Quick mode validates the reactive corpus contract; heavy release validation executes every corpus anchor and required occupancy profile. The `actors-delivery` skill owns evidence interpretation and handoff without creating another public release gate.

- [reactive-operations-corpus.sh](./reactive-operations-corpus.sh)
  Validate all or one family of the machine-readable Actors reactive-operations corpus. `--execute` runs every selected Rust anchor, with optional `--release`; validation alone checks composition but does not execute tests. The contract enforces runtime identity, invariants, ordered checkpoints, rollback/weight ownership, deterministic seeds, and live anchors. Failures emit selected seed/initial-state evidence under `${TMPDIR:-/tmp}`.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `deos-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

Project-local audit leaves and targeted routes are documented in `/.agents/skills/alignment/SKILL.md`. Use the diff-aware completion gate for changed-scope work and `./scripts/validate-local.sh fast|heavy|full` only for the corresponding release-validation boundary.

Commands executed through the shared script harness use compact output by default: successful test, build, lint, documentation, metadata, and benchmark steps print only their label, duration, and result. A failed step prints the last 80 lines and retains its complete output in a temporary log whose path appears in the error. Set `DEOS_VERBOSE=1` to restore live full output, or set `DEOS_FAILURE_TAIL_LINES=N` to change the failure excerpt without enabling verbose mode.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. Supports `--extra` for Actors diagnostics, `--extrinsic NAME --output FILE` for focused evidence that must not replace complete production weights, and `--skip-build` when reusing a freshly built benchmark runtime. The [`benchmarking` skill](../.agents/skills/benchmarking/SKILL.md) owns case selection, evidence interpretation, weight handoff, and claim boundaries without duplicating this command surface.

- [ci-local.sh](./ci-local.sh)
  Reproduce local CI or select one compact check with `--only`; narrow Cargo work further with `--package NAME`, `--test-filter NAME`, and explicit feature mode. Apply Rust formatting with `--only format --fix`. Agents should prefer this entrypoint over raw Cargo commands.

## Admin Utilities

- [seed-web-client-state.sh](./seed-web-client-state.sh)
  Idempotently prepare the composite live-chain state needed for local wallet, swap, and native-staking UI testing: verify genesis prerequisites, fund Alice, and create or top up the Native/foreign and `NTVE/stNTVE` pools. This is a named admin workflow rather than an atomic numbered leaf because it coordinates several state checks and transactions.

- [export-papi-metadata.sh](./export-papi-metadata.sh)
  Export native runtime metadata through the committed `deos-runtime` metadata example, regenerate PAPI descriptors, and project observation-inspector runtime evidence from the exact metadata, compressed runtime-code Wasm, runtime constants, production Actors weights, and descriptor identity. This replaces ad hoc metadata export and independently maintained inspector constants.

- [bootstrap-native-staking-local.sh](./bootstrap-native-staking-local.sh)
  Consolidated native staking bootstrap helper. `check` reads live readiness for the canonical `NTVE/stNTVE` pool and Native Staking Liquidity Actor skeleton; `prepare-calls` emits plan-only Root/governance or signed-operator call data for staking registration, pool creation, and liquidity seeding. It never signs or submits transactions. The [`staking-delivery` skill](../.agents/skills/staking-delivery/SKILL.md) owns readiness sequencing, authority boundaries, and activation handoff without duplicating this command.

- [authorized-upgrade-local.sh](./authorized-upgrade-local.sh)
  Consolidated authorized runtime-upgrade helper. `check` pins one finalized block, compares live and local runtime code, inspects protocol `L1RootAction` submission authority and `$VETO` issuance, verifies the pending authorized hash, and can emit offline relay call data. `prepare-authorization` emits candidate-bound stake, preimage, and signed proposal call data only after reporting finalized item, balance, fee, and authority checks; it withholds the protection `Pass` call unless the lifecycle is ready. `apply` stays plan-only unless explicit `--submit` relays matching code bytes through `System.apply_authorized_upgrade { code }`. `snapshot` captures finalized non-empty Router/Oracle/Actors baseline state, and `verify` checks runtime identity, selected-state preservation, and live code equality with the candidate Wasm.

- [teardown-local-network.sh](./teardown-local-network.sh)
  Stop local `zombienet` / `polkadot*` / `vite` dev-server processes and remove Zombienet temp directories.

- [clean-local-artifacts.sh](./clean-local-artifacts.sh)
  Remove generated local artifacts (`chain_spec.json`, optionally `target/` and `bin/`).

- [_common.sh](./_common.sh)
  Shared path, logging, timed-step, and background-process helpers used by deterministic root commands and project-skill script leaves. A co-located skill script supplies `DEOS_PROJECT_ROOT` before sourcing it.
