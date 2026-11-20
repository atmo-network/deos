# DEOS — Deterministic Economic Operating System

DEOS is a reusable FRAME framework for developers building downstream protocol economies. It supplies bounded Actors, asset routing, issuance, staking, governance and observation mechanisms. The reference runtime composes these into a development instance; your fork owns its economics, identities, authorities and rollout policy. See the [framework/instance contract](docs/deos/framework-instance.contract.en.md).

## What is included

- [`template/primitives`](template/primitives), [`template/pallets`](template/pallets), and [`template/runtime`](template/runtime): the reusable crates, reference composition, runtime-owned genesis presets and regression tests.
- [`template/vendor`](template/vendor/README.md) and [`template/tools`](template/tools/README.md): selected SDK corrections and security backports, indexed by which workspace patches each package and why, plus the independent exact-tag PVF validation host.
- [Scripts](scripts/README.md) for toolchain setup, framework validation, Wasm builds, metadata export, chain-spec preparation and PVF. [Notices](notices/) accompany the [project license](LICENSE) and vendor license files.
- [Contracts and implementation guides](docs/deos/README.md), pallet embedding fixtures, and [TMCTOL mathematical tests](simulators/tmctol/).

## Build and validate

Use Linux x86-64, Bash 4.3+, Rustup, Python 3.11+ with `tomllib`, a C/C++ toolchain with make, Perl (including `FindBin` and `IPC::Cmd`), `pkg-config`, OpenSSL development libraries, `protoc`, `ripgrep`, `jq`, GNU coreutils/tar and Git for Cargo's Git dependencies. Guest checks require a Wasm-capable C toolchain (`clang` and `llvm-ar` by default; override with `CC_wasm32v1_none` and `AR_wasm32v1_none`). Use the Node version in [`.node-version`](.node-version) for framework scripts and the simulator. Toolchain and locked dependency acquisition may use the network. No frontend npm packages or installed Skills are needed.

```bash
./scripts/setup-environment.sh framework
DEOS_TEST_PROFILE=release DEOS_TEST_TIMEOUT_MINUTES=60 ./scripts/validate-local.sh framework
./scripts/03-build-runtime.sh --output "$PWD/deos-development.wasm"
./scripts/export-papi-metadata.sh --wasm "$PWD/deos-development.wasm" --output "$PWD/deos.scale"
```

The Wasm build uses the production **compiler profile**, not production-qualified weights or an accepted network artifact. Metadata export calls that Wasm's Metadata API, checks byte equality, and refuses native fallback. `--native-only` is a separate schema preparation route and does not establish correspondence to Wasm.

## Configure or embed

Fork [`template/runtime/src/configs`](template/runtime/src/configs) and the [runtime-owned genesis configuration](template/runtime/src/genesis_config_presets.rs) for a composed instance. The `development` and `local_testnet` presets are development profiles; scripts select a named preset without patching economic policy or authorities. Consult the [reference runtime guide](template/README.md) for composition and configuration owners.

For a smaller integration, follow the [Router embedding guide](template/pallets/router/docs/embedding.md). The [separate fixture crate](template/pallets/router/embedding-runtime) implements the host traits with its own assets and route policy, without the DEOS runtime. Run its hosted behavior and guest compile checks:

```bash
SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package pallet-deos-router-embedding-fixture
SKIP_WASM_BUILD=1 RUSTFLAGS='--cfg substrate_runtime' CC_wasm32v1_none=clang AR_wasm32v1_none=llvm-ar \
  ./scripts/ci-local.sh --only check --package pallet-deos-router-embedding-fixture --no-default-features --target wasm32v1-none
```

## Demonstrated and unavailable capabilities

The [0.7.28 distribution and support table](docs/deos/framework-distribution.en.md) distinguishes framework development, finite reference-Wasm execution and production network qualification. The signed Assets blocks and proof-bound PVF validation use prepared parent states; they do not establish authenticated relay ancestry, production resource bounds or a runnable node bundle. Passing framework validation does not renew the historical Root and Host source-bound security decisions. Chain-spec generation and local node operations require separately accepted producer artifacts, not just binaries with matching version strings. No downstream production launch is supported before DEOS 1.0.

## Optional browser and Wiki workspaces

The [experimental browser client](https://github.com/atmo-network/deos/tree/7cf526687d4503d24dea1f19a8a3a47f28965971/web-client) lives in `web-client/` in the full repository, outside the default framework distribution. It consumes framework metadata through its optional PAPI descriptor workflow. Its setup, validation and advisory results are separate from framework results.

Read the bilingual [DEOS Wiki](https://atmo-network.github.io/deos/). The [Wiki workspace](https://github.com/atmo-network/deos/tree/7cf526687d4503d24dea1f19a8a3a47f28965971/wiki) in the full repository owns the corpus and standalone mdBook site. Use `./scripts/validate-local.sh wiki` to check it independently; framework validation does not require Wiki tooling.
