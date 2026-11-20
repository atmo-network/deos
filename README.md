# DEOS

**Build a protocol economy, not a runtime from scratch.**

The Deterministic Economic Operating System is a FRAME framework for on-chain automation, asset routing, issuance, staking, governance and observations. Start with the reference runtime or embed individual pallets. Your fork decides the [economics, identities, authorities and rollout policy](docs/deos/framework-instance.contract.en.md).

**[Read the DEOS Wiki (EN / RU)](https://atmo-network.github.io/deos/)** · [Build from source](#build-and-validate) · [Embed a pallet](#configure-or-embed)

> **Pre-1.0 · Source and development only.** This is not a production network or a runnable node bundle.

## What is included

- **On-chain mechanisms:** [DEOS Actors and the other FRAME pallets](template/pallets), [shared primitives](template/primitives), and their specifications and embedding fixtures.
- **A working reference composition:** The [runtime](template/runtime) brings those mechanisms together with development genesis presets and regression tests. [TMCTOL](simulators/tmctol/) is the reference economic model, not a kernel requirement.
- **Build and verification tools:** [Scripts](scripts/README.md) for validation, Wasm, metadata, chain-spec preparation and PVF; [SDK backports](template/vendor/README.md) and the [independently locked PVF Host](template/tools/README.md). See the [license](LICENSE) and [third-party notices](notices/).

The [framework documentation](docs/deos/README.md) is the technical index for contracts, architecture and integration guides.

## Build and validate

**Prerequisites:** Linux x86-64, Bash 4.3+, Rustup, Python 3.11+ with `tomllib`, and [the pinned Node version](.node-version). Install a C/C++ toolchain with `make`, Perl (`FindBin` and `IPC::Cmd`), `pkg-config`, OpenSSL development libraries, `protoc`, `ripgrep`, `jq`, GNU coreutils/tar and Git for Cargo dependencies. Guest checks use `clang` and `llvm-ar` by default; override them with `CC_wasm32v1_none` and `AR_wasm32v1_none`.

**1. Validate the source.** The framework profile needs neither frontend npm packages nor installed Skills. Toolchain and locked dependency acquisition may use the network.

```bash
./scripts/setup-environment.sh framework
DEOS_TEST_PROFILE=release DEOS_TEST_TIMEOUT_MINUTES=60 \
  ./scripts/validate-local.sh framework
```

**2. Build the development Wasm and export its metadata.** The build uses the production compiler profile, not production-qualified weights. Metadata export calls the Wasm's Metadata API, checks byte equality and refuses native fallback; `--native-only` prepares a schema separately and does not verify Wasm correspondence.

```bash
./scripts/03-build-runtime.sh --output "$PWD/deos-development.wasm"
./scripts/export-papi-metadata.sh \
  --wasm "$PWD/deos-development.wasm" --output "$PWD/deos.scale"
```

## Configure or embed

**Fork the runtime** when you want a composed instance. Start with the [reference runtime guide](template/README.md), then adjust the [runtime configuration](template/runtime/src/configs) and [genesis presets](template/runtime/src/genesis_config_presets.rs). The scripts select `development` or `local_testnet` without overriding economic policy or authorities.

**Embed a pallet** when you want one mechanism in your own runtime. The [Router embedding guide](template/pallets/router/docs/embedding.md) and its [fixture crate](template/pallets/router/embedding-runtime) show a host with its own assets and routing policy, without the DEOS runtime.

```bash
SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests \
  --package pallet-deos-router-embedding-fixture
SKIP_WASM_BUILD=1 RUSTFLAGS='--cfg substrate_runtime' \
  CC_wasm32v1_none=clang AR_wasm32v1_none=llvm-ar \
  ./scripts/ci-local.sh --only check --package pallet-deos-router-embedding-fixture \
  --no-default-features --target wasm32v1-none
```

## Demonstrated and unavailable capabilities

See the [0.7.28 distribution and support table](docs/deos/framework-distribution.en.md) for the exact scope.

- **Source:** Framework validation supports development; it does not renew the historical Root and Host source-bound security decisions.
- **Execution:** Signed Assets blocks and proof-bound PVF validation demonstrate behavior on prepared parents, not authenticated relay ancestry or production resource bounds.
- **Network:** Chain-spec generation and local node operations require separately accepted producer artifacts, not merely matching version strings. No downstream production launch is supported before DEOS 1.0.

## Optional browser and Wiki workspaces

- **[Experimental browser client](https://github.com/atmo-network/deos/tree/7cf526687d4503d24dea1f19a8a3a47f28965971/web-client):** Uses framework metadata through its optional PAPI descriptor workflow. Setup, validation and advisory results are separate from the framework's.
- **[Wiki workspace](https://github.com/atmo-network/deos/tree/7cf526687d4503d24dea1f19a8a3a47f28965971/wiki):** Owns the bilingual corpus and standalone mdBook site. Run `./scripts/validate-local.sh wiki` in the full repository to validate it without framework builds.

Both workspaces are available in the full repository, not in the framework source archive.
