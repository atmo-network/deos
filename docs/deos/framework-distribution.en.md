# DEOS 0.7.28 framework distribution

This is a pre-1.0 source/development release for downstream instance builders, not a production-network release. The [framework/instance contract](framework-instance.contract.en.md) defines which decisions a fork owns. The reference runtime is not cut down for this distribution; the fresh-genesis ABI and storage may change before 1.0. No downstream production network may launch from 0.7.28.

| Scope | Support and limits |
| --- | --- |
| Reusable components | Primitives, FRAME pallets, specifications, embedding contracts, vendored SDK corrections, external-consumer fixtures and TMCTOL mathematics. |
| Reference runtime | Runtime-owned `development` and `local_testnet` presets; Actors, routing, TMC, staking, governance, Oracle, local asset accounting and XCM composition. User Actor creation starts disabled; no `Live` preset. |
| Configuration | Fork the reference runtime's configuration, adapters and genesis code, or embed individual pallets using their host traits and embedding fixtures. Instance policy is not part of the reusable pallet contract. |
| Development environment | Linux x86-64 with the pinned Rust and Node toolchains, using the [declared native prerequisites](../../README.md#build-and-validate). Framework validation checks native and guest builds, the independent Host, embedding fixtures and TMCTOL without npm, Git checkout state or Skills. Other platforms are not qualified here. |
| Execution | The signed Assets diagnostic covers eleven signed calls per named preset, events, state, fees and nonces, and independent proof replay. Secure PVF matched two complete positives and six typed negatives with the pinned 1.25.2 worker family. Both sealed blocks use the same prepared state transition; no relay authentication. |
| Optional or unavailable | Browser/PAPI, network compositions, research campaigns and Wiki stay in the full repository. No production fee/cost certificates, production Wasm acceptance or runnable canonical node bundle are supplied. User Actor admission capacity, public AwaitFunding and production genesis are not declared by this package. |
| Source-bound security | Framework checks are not security approval. The earlier finite Root and isolated Host source decisions do not apply to the current inputs; the installed routes refuse. This package does not attest current security applicability, Native secret-bearing callers or external producers. |
| Production qualification | Requires complete resource/Weight and witness envelopes, supported host/profile transfer, exact artifact binding, independently qualified external producer/platform inputs and final composite acceptance. Neither packaging nor finite replay satisfies these gates. |

## Artifacts and qualification

The [Actors performance closeout](actors.integration.en.md#actors-performance-closeout-for-the-framework-source-distribution) distinguishes the retained logical-write reduction from the inconclusive timing result and historical admission observations. This release claims no general speedup or sustained-throughput increase.

The source archive contains committed files only. `contents.json` lists its source commit/tree, file modes, sizes and SHA-256 digests; `SHA256SUMS` binds the archive and contents manifest. Build outputs, private operational evidence and caches are not part of the payload. The root license, vendor and Unicode notices, and the verbatim upstream `str0m-openssl` MIT notice are included. A checksum is not a signature or publisher attestation.

The separate diagnostic pair, if supplied, is the production-compiler-profile guest `b42b46030f7bbdafa78185ecf035236496bdae4ec76c4c3408c78dd6f07afbc4` and matching SCALE v16 `2a16a1fcdcc71e6411c163f82b3a809bab625620ea076ee92ba60c6a21ac6882`. The package can independently compare metadata through the actual Wasm Metadata API. Neither schema equality nor the finite PVF result qualifies a resource envelope or the original worker producers.

Chain-spec tooling and runtime presets are retained. The [maintained Builder19 receipt](../../scripts/inputs/chain-spec-builder/README.md) approves the earlier caller, not the new client-free closure. A reviewed construction basis, exact producer receipt and artifact decision for the new caller are required before Script 04 will invoke it. Do not use an unverified Builder, a version-only node selection or the diagnostic guest as a launch substitute. Published relay/worker producer provenance, supported-platform evidence, and exact canonical Linux operator inputs remain separate gates. The source distribution does not clear general Zombienet use or Native secret-bearing cryptography (`RUSTSEC-2026-0387`).
