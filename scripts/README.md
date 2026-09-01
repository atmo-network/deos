# Scripts Layer

This directory is the deterministic operator/developer automation layer of the DEOS reference stack. It supports testing, validation, benchmarking, generation, build, deployment preparation, local-network operation, release work, and repeatable coordination across `/docs`, `/template`, and `/web-client`; it is not the primary conceptual control plane.

The pre-`1.0` release line accepts only fresh-genesis candidates. Local full validation does not certify an upgrade from an earlier release, a deployed storage lineage, or a network launch.

This directory contains deterministic command surfaces, not agent strategy:

- `Numbered scripts` are reusable atomic operations. They may depend on `_common.sh`, external tools, and artifacts on disk, but they do not orchestrate other numbered scripts.
- `Named implementations` are deterministic operator utilities or compositions whose mode and outcome need no agent interpretation.
- `Shared implementations` stay here whenever humans, GitHub Actions, CI, root compositions, or multiple skills invoke them.
- `Agent-owned workflows` live under `/.agents/skills/<domain>` when they require scope selection, coordination, evidence interpretation, knowledge synchronization, or handoff judgment; they call public root scripts for shared execution.
- `Validation entrypoints` distinguish focused domain/package tests, development profiles, CI checks, and release/full validation while retaining one comprehensive project-owned route. They compose only project-owned surfaces; a required check found inside a Skill must move here or to its package owner rather than be invoked across the boundary.
- `Full named/admin implementations` follow `usage -> parse_args -> check_prerequisites/plan -> main`.
- `All entrypoints` expose `--help` and keep declared environment/behavior contracts honest.

## Language Boundary

Public root orchestration uses Bash for process lifecycle, environment and toolchain control, filesystem operations, signal-safe cleanup, and composition of existing commands. JavaScript ES modules are support leaves for deterministic structural transformation or validation when JSON, metadata, graph traversal, exact integer handling, or testable data semantics would be unsafe or obscure in shell. They remain behind an owning Bash or package entrypoint and must not grow into a parallel orchestration layer.

The split follows workload semantics rather than directory or product language: Bash controls actions, while JavaScript understands structured data. Do not replace a clear shell composition with Node merely to unify extensions, and do not encode structural programs as `sed`/`awk`/`jq` pipelines merely to keep a `.sh` suffix. A root JavaScript support leaf must have a concrete structural owner that cannot be represented safely by the runtime, package, or an existing native tool; `04-generate-chain-spec.sh`, for example, consumes the complete runtime-owned preset directly instead of maintaining a second JavaScript genesis projection.

Do not add Python automation. One bootstrap exception exists: `setup-environment.sh` reads `rust-toolchain.toml` through `python3` with `tomllib`, because Node has no built-in TOML parser and the Rust toolchain must be installable before any Node runtime is pinned. That exception covers TOML parsing only, and the script checks for `tomllib` as a named prerequisite. Every JSON read in that script uses Node, matching `_common.sh`.

## Executable Ownership Inventory

Path classes provide the inventory without duplicating the per-command map below:

| Path class | Classification and owner | Consumer contract |
| --- | --- | --- |
| `/scripts/[0-9][0-9]-*.sh` | Shared human-callable atoms; root scripts layer | Humans, CI, workflows, and skills call the owning file directly |
| `/scripts/<name>.sh` | Shared deterministic compositions/admin utilities; root scripts layer | Shared consumers call one canonical implementation; `_common.sh` remains support-only |
| `/.agents/skills/alignment/scripts/*` | Agent-only project audit and completion capability | The Alignment Skill invokes its own leaves; project validation does not depend on them |
| `/.agents/skills/domain-dag/scripts/*` | Independent graph-validator capability | The Domain DAG Skill invokes its own validator; project packages and scripts do not depend on it |
| `/web-client/scripts/*.mjs` | Client-package entrypoints or thin capability bridges; web client | npm owns invocation; bridges contain no duplicated validator semantics |

GitHub workflows invoke root shared implementations only. Skills never call sibling skill internals, support files are not public entrypoints, and a consumer references the canonical owner rather than maintaining a second executable copy.

## Human-Callable Atomic Scripts

Each numbered command is independently callable by a human or CI from any working directory. Its `--help` declares inputs, outputs, side effects, and configurable environment. The command checks its own prerequisites and never invokes another numbered command. Numbers form the logical local-network evidence ladder—binary prerequisites, Cargo tools, runtime, ChainSpec, network, liveness, basic mutation, temporal consensus, then composed economics—without making an atom depend implicitly on earlier scripts.

- [01-download-binaries.sh](./01-download-binaries.sh)
  Download the pinned Polkadot SDK `stable2606-1` relay node, Omni Node, preparation/execution workers, and `frame-omni-bencher` for the supported host, verify repository-recorded SHA-256 digests before publishing the complete bundle under ignored `/bin`, and reject unsupported platforms rather than selecting an approximate asset. `--check` verifies the existing release marker, checksums, executable bits, and binary identities without downloading.

- [02-install-tools.sh](./02-install-tools.sh)
  Install exact versions of Zombienet and Chain Spec Builder plus Try Runtime from one immutable Git revision. Existing commands are reused only when their reported version matches the repository pin.

- [03-build-runtime.sh](./03-build-runtime.sh)
  Stage the template at one fixed fail-closed physical build root, build the current `deos-runtime` Wasm from the locked graph and pinned Rust toolchain with the existing fat-LTO/single-codegen-unit production profile, remap source/Cargo/Rustup roots to canonical virtual prefixes, atomically publish only the successful artifact, then report its size and SHA-256 digest.

- [04-generate-chain-spec.sh](./04-generate-chain-spec.sh)
  Generate and verify a selected chain-spec output directly from the complete runtime-owned Development or Local preset in the selected DEOS runtime Wasm. Isolated temporary generation prevents a failed candidate from overwriting `template/chain_spec.json`; the script supplies only outer ChainSpec metadata, rejects a para ID that disagrees with the reference preset, and exposes no pseudo-Live profile or second economic-policy projection.

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
  Prepare the repository-pinned Rust environment, or clear generated SvelteKit state and install the pinned Node/npm client from its lockfile; full mode prepares both environments. Wiki projection and OKF tooling remain owned and prepared by the `wiki-sync` Skill rather than project validation.

- [network-assurance-local.sh](./network-assurance-local.sh)
  Prepare pinned tools/artifacts, spawn the canonical two-validator/two-collator topology, verify both collators prepare blocks, pause one collator while the other preserves finality, restart the second collator from its existing base path, and execute signed finalized transfers before and after restart. `SESSION_TRANSITION=1` adds the multi-hour finalized session check, while `COMPOSED_PATH=1` adds the mutating Router/Oracle/Burn Actor check. The command terminates and verifies every owned process unless `--keep-network` is selected. It makes no tag-bound, runtime-upgrade, or publication claim.

- [bootstrap-local-network.sh](./bootstrap-local-network.sh)
  Run the local bootstrap chain: tools -> runtime build -> chain spec -> Zombienet, using Polkadot and Omni Node binaries already available in `PATH` or `./bin`. Start the web client directly from `web-client` with `npm run dev`.

- [validate-local.sh](./validate-local.sh)
  Prepare pinned repository dependencies and directly run the selected `fast`, `heavy`, or `full` validation profile. Fast owns simulator tests and workspace CI; heavy adds client, Actors, and benchmark checks; full additionally prepares the checksum-verified binary bundle, then builds the production runtime and regenerates metadata and client artifacts with zero worktree drift. GitHub Actions runs heavy validation through the required pull-request `validation-gate` with `SKIP_WASM_BUILD=1`; this explicitly omits only Wasm identity while retaining Actors source, Weight, metadata, behavioral, and heavy-profile assurance. Local full validation owns pre-`1.0` release acceptance. No Skill audit, cache, hidden authority, network run, or release action is involved.

- [actors-assurance.sh](./actors-assurance.sh)
  Shared current-tree Actors proof contract for semantic-manifest and fee-envelope-vector freshness, cross-language semantics, scheduler stress, capacity, and independent-runtime embedding. By default it reports and preserves exact source, production Weight, production Wasm, and metadata identities. A caller that intentionally cannot build Wasm may set `REQUIRE_WASM_IDENTITY=0`; the script reports that narrower evidence boundary while retaining every non-Wasm check. Historical transition replay is release history rather than a routine validation dependency.

  Its built-in `self-test` mode checks source-content identity in temporary Git fixtures: committed, staged, unstaged, untracked, deleted and executable-mode changes, stable commit packaging, artifact exclusions and fail-closed symlink handling. The assurance gate runs the same self-test automatically; standalone self-test needs no Cargo, network access or build artifacts.

- [audit-asset-conversion-boundaries.sh](./audit-asset-conversion-boundaries.sh)
  Read-only fail-closed audit requiring exactly one production direct Asset Conversion pool-creation owner inside the atomic DEOS lifecycle and prohibiting LP-binding repair outside that owner. It accepts no arguments except `--help` and may run from any working directory.

- [try-runtime-local.sh](./try-runtime-local.sh)
  Build `deos-runtime` with `try-runtime` and optionally execute live dry-runs against the local parachain RPC.

Use the owning tool's focused checks for changed-scope work and `./scripts/validate-local.sh fast|heavy|full` for the corresponding project-validation boundary. These commands do not require agent skills; any agent-specific audits are optional additional review.

Before creating a release tag, run `./scripts/validate-local.sh full` against the intended commit. After that commit is accepted as `main` and tagged, verify with Git that `main`, the peeled `vX.Y.Z` tag, and the validated commit/tree are identical and that no parallel plain version tag exists. Release verification must not depend on an agent skill, hidden validation state, or inferred GitHub authority.

Commands executed through the shared script harness use compact output by default: successful test, build, lint, documentation, metadata, and benchmark steps print only their label, duration, and result. A failed step prints the last 80 lines and retains its complete output in a temporary log whose path appears in the error. Set `DEOS_VERBOSE=1` to restore live full output, or set `DEOS_FAILURE_TAIL_LINES=N` to change the failure excerpt without enabling verbose mode.

- [benchmarks.sh](./benchmarks.sh)
  Run pallet benchmarking flows and weight generation helpers. `--check` compiles benchmark surfaces and rejects generated Weight files that name Actors storage absent from the current pallet; `--extra` enables Actors diagnostics, `--extrinsic NAME --output FILE` produces focused evidence that must not replace complete production weights, and `--skip-build` reuses a freshly built benchmark runtime. The [`architecture-experiments` skill](../.agents/skills/architecture-experiments/SKILL.md) owns case selection, controlled comparison, evidence interpretation, decision lineage, Weight handoff, and claim boundaries without duplicating this command surface.

- [ci-local.sh](./ci-local.sh)
  Reproduce local CI or select one compact check with `--only`; narrow Cargo work further with `--package NAME`, `--test-filter NAME`, and explicit feature mode. Apply Rust formatting with `--only format --fix`. Agents should prefer this entrypoint over raw Cargo commands.

## Admin Utilities

- [seed-web-client-state.sh](./seed-web-client-state.sh)
  Idempotently prepare the composite live-chain state needed for local wallet, swap, and native-staking UI testing: verify genesis prerequisites, fund Alice, and create or top up the Native/foreign and `NTVE/stNTVE` pools. This is a named admin workflow rather than an atomic numbered leaf because it coordinates several state checks and transactions.

- [export-papi-metadata.sh](./export-papi-metadata.sh)
  Export native runtime metadata through the committed `deos-runtime` metadata example in an isolated Cargo target, regenerate PAPI descriptors, and project observation-inspector runtime evidence from the exact metadata, compressed production runtime-code Wasm, runtime constants, production Actors weights, and descriptor identity. Isolation prevents metadata compilation from replacing the production Wasm artifact. This replaces ad hoc metadata export and independently maintained inspector constants.

- [bootstrap-native-staking-local.sh](./bootstrap-native-staking-local.sh)
  Consolidated native staking bootstrap helper. `check` reads canonical `NativeSecurityMode`, pool readiness and bounded `ActorEligibilityApi` state at one finalized block. JSON `stakingLiquidityActor` contains the Active activation projection, or null for Dormant/NotRegistered; classification errors fail closed. Matching runtime metadata/descriptors are required. `prepare-calls` emits plan-only registration, pool and liquidity calls without mode-inactive LP-security actions. It never signs or submits transactions; the staking specification and package architecture own sequencing and authority boundaries.

- [authorized-upgrade-local.sh](./authorized-upgrade-local.sh)
  Consolidated authorized runtime-upgrade helper. `check` pins one finalized block, compares live and local runtime code, inspects protocol `L1RootAction` submission authority and `$VETO` issuance, verifies the pending authorized hash, and can emit offline relay call data. `prepare-authorization` emits candidate-bound stake, preimage, and signed proposal call data only after reporting finalized item, balance, fee, and authority checks; it withholds the protection `Pass` call unless the lifecycle is ready. `apply` stays plan-only unless explicit `--submit` relays matching code bytes through `System.apply_authorized_upgrade { code }`. `snapshot` captures finalized non-empty Router/Oracle/Actors baseline state, and `verify` checks runtime identity, selected-state preservation, and live code equality with the candidate Wasm.

- [teardown-local-network.sh](./teardown-local-network.sh)
  Stop local `zombienet` / `polkadot*` / `vite` dev-server processes and remove Zombienet temp directories.

- [clean-local-artifacts.sh](./clean-local-artifacts.sh)
  Remove generated local artifacts (`chain_spec.json`, optionally `target/` and `bin/`).

- [_common.sh](./_common.sh)
  Shared path, logging, timed-step, and background-process helpers used by deterministic root commands and project-skill script leaves. A co-located skill script supplies `DEOS_PROJECT_ROOT` before sourcing it.
