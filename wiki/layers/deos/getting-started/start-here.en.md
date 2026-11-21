---
type: getting-started
title: Start Here
description: Build and inspect the DEOS runtime from source, or run a local reference network with separately accepted operator inputs.
locale: en
canonical_page_id: start-here
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../../../README.md
  - resource: ../../../../docs/deos/framework-distribution.en.md
  - resource: ../../../../web-client/README.md
  - resource: ../../../../scripts/README.md
status: stable
audience: newcomer
tags:
  - onboarding
  - getting-started
  - validation
related:
  - DEOS in 60 Seconds
  - Forking DEOS
  - Three-Layer Validation
---

# Start Here

The source distribution can build and validate the reference runtime without a local relay chain or a browser client. Starting a network is a separate operator task.

## Build and inspect the runtime

On Linux x86-64 with the source package's native build prerequisites installed, run from the source root:

```bash
./scripts/setup-environment.sh framework
./scripts/03-build-runtime.sh --output "$PWD/deos-development.wasm"
./scripts/export-papi-metadata.sh \
  --wasm "$PWD/deos-development.wasm" --output "$PWD/deos.scale"
```

Choose a new Wasm output file. The exporter reads the selected Wasm's Metadata API and checks exact byte equality without falling back to native metadata. This produces a development Wasm and its metadata, not a qualified network artifact. Use `./scripts/validate-local.sh framework` to check the source and independent runtime consumers.

## Run a qualified local network

Use the full repository and an independently accepted Linux operator bundle. A source build cannot supply that bundle. Check the bundle using Script 01's `--check-reference-bundle` and import it using `--import-reference-bundle` with `--bundle-output`. Select the bundle's exact Node, Zombienet CLI, Omni Node and raw chain spec with its accepted Wasm; install the separately pinned relay and worker binaries as well. Do not substitute version strings or a freshly built development Wasm for accepted inputs.

With those inputs selected, choose an existing owned work directory without a `zombienet-network` output and a fresh evidence path. See `./scripts/05-spawn-zombienet.sh --help` for the exact environment inputs. Run Script 05's default reference mode in the foreground, then use `./scripts/06-network-smoke.sh` against its configured local RPC endpoints from another terminal. The smoke check observes bounded finalized relay and parachain progress without submitting transactions.

## Open the optional client

With the repository-pinned Node and npm active in your shell, the full repository can also start a standalone development client:

```bash
./scripts/setup-environment.sh client
npm --prefix web-client run papi:generate
npm --prefix web-client run dev
```

The client can open independently; live chain views require a reachable chain with compatible metadata.

## Related

- [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
- [Scripts Layer](../scripts-layer.en.md)
- [Forking DEOS](../forking-deos.en.md)
- [Three-Layer Validation](../three-layer-validation.en.md)
