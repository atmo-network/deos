---
page_type: getting-started
title: Start Here
summary: A short onboarding spine that routes newcomers into one of three paths - understand DEOS, run it locally, or fork it and change the economy safely.
locale: en
canonical_page_id: start-here
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../template/README.md
  - ../../web-client/README.md
  - ../../scripts/README.md
  - ../../simulator/README.md
status: active
audience: newcomer
tags:
  - onboarding
  - getting-started
  - forkability
  - validation
related:
  - DEOS in 60 Seconds
  - Partner Evaluation Route
  - Forking DEOS
  - Minimal Fork Profile
  - Three-Layer Validation
  - Validation Troubleshooting
last_compiled: 2026-05-28
confidence: 0.9
---

# Start Here

## Summary

DEOS has deep runtime, economics, client, documentation, and agent-maintenance layers. You do not need to learn them all before you begin.

Choose the path that matches your intent. Each path gives you a short route, a clear done condition, and the smallest validation surface that proves you are moving in the right layer.

You do not need to read `AGENTS.md` to evaluate or fork DEOS. Treat `AGENTS.md` as maintainer and agent operating context. Use this page, the wiki, and the local workspace README files for human onboarding.

## Path A - Understand DEOS in 10 minutes

Use this path if you are an ecosystem reader, partner, investor, or curious builder trying to decide whether DEOS is worth deeper evaluation.

If you are evaluating a partnership or fork, start with [Partner Pitch](partner-pitch.en.md) first. It answers why DEOS matters before the deeper architecture route.

Read:

1. [Partner Pitch](partner-pitch.en.md), if you need the external adoption case
2. [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
3. [What DEOS Is Not](../concepts/what-deos-is-not.en.md)
4. [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.en.md)
5. [TMCTOL Standard](../concepts/tmctol-standard.en.md), starting with the summary and core mechanics
6. [Economic Claim Levels](../concepts/economic-claim-levels.en.md) if you need to distinguish mechanism claims from market promises

Done when you can answer:

- What does DEOS replace in discretionary DAO treasury management?
- Why is TMCTOL a standard on top of DEOS rather than the whole framework?
- Which claims are deterministic protocol behavior, and which claims still depend on market and liveness conditions?

Next if still interested: [Partner Evaluation Route](../usage/partner-evaluation-route.en.md).

## Path B - Run DEOS locally in 30 minutes

Use this path if you are a developer checking whether the repository can run on your machine.

Prerequisites:

- Rust installed
- Node.js installed
- Enough disk and build time for a Polkadot SDK runtime workspace

Terminal 1 from the repository root:

```sh
./scripts/bootstrap-local-network.sh
```

Expected result:

- Local Polkadot SDK binaries are available
- The reference runtime builds
- A local chain spec is generated
- Zombienet starts an Omni Node based local network
- The parachain begins producing blocks

Terminal 2 from the repository root:

```sh
npm --prefix web-client install
npm --prefix web-client run dev
```

Expected result:

- Vite prints a local URL
- The reference client opens
- The wiki surface loads
- Wallet, chain status, and bounded live surfaces can connect once the local network is ready

Optional local demo state:

```sh
./scripts/07-seed-web-client-state.sh
```

If you get stuck, use:

- [Scripts Layer](../usage/scripts-layer.en.md) for what each script owns
- [Validation Troubleshooting](../usage/validation-troubleshooting.en.md) for common gate failures
- `./scripts/teardown-local-network.sh` to stop local services
- `./scripts/clean-local-artifacts.sh` to remove generated local artifacts

Done when the local network is producing blocks and the web client loads against it.

## Path C - Fork and change the economy safely

Use this path if you are a partner team or protocol builder asking how to turn DEOS into a downstream ecosystem.

Start with reading:

1. [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
2. [Forking DEOS](../usage/forking-deos.en.md)
3. [TMCTOL Formulas](../math/tmctol-formulas.en.md)
4. [Three-Layer Validation](../development/three-layer-validation.en.md)

Then make the first economic experiment in the simulator before touching runtime code:

```sh
node simulator/tests.js
```

Safe first-change map:

| Change | Start | Then touch | Minimum validation |
| --- | --- | --- | --- |
| TMC price/slope | Simulator + formulas | Runtime config after math holds | Simulator, then TMC tests |
| TOL split/reserves | TMCTOL spec + simulator | AAA topology, runtime config, docs | Simulator + runtime tests |
| Router fee policy | Axial Router | Router config, governance bounds | Router tests + claims |
| Governance domains/payloads | Governance overview | Gov pallet/config, client | Governance + client checks |
| UI copy/onboarding | `web-client/` + `wiki/` | Runtime only if data contract changes | Client validate + wiki trust |

Do not casually change:

- Launch-time curve physics after curve creation
- Floor, compression, or guarantee wording without updating claim preconditions
- Bucket accounting invariants
- The read-model split between bounded on-chain projections and materialized/indexed views
- Governance protection-track authority boundaries

Done when you know which choices are product narrative, simulator parameters, runtime constants, governance policy, client presentation, or externally materialized data.

## Minimum validation by path

Use the smallest meaningful gate first. You do not need every gate for every change.

| Path or change | Minimum validation |
| --- | --- |
| Understanding only | No command required |
| Wiki/onboarding text | `npm --prefix web-client run validate:wiki` |
| Web client behavior | `npm --prefix web-client run validate` |
| Web client boundaries | `npm --prefix web-client run validate:dag` |
| Tokenomics/formulas | `node simulator/tests.js` |
| TMC runtime | `cargo test --manifest-path template/Cargo.toml -p pallet-tmc --locked` |
| Broad runtime | `cargo test --manifest-path template/Cargo.toml --workspace --locked` |
| Cross-domain change | Simulator, cargo tests, client validation, completion gate |

## Related

- [DEOS in 60 Seconds](deos-in-60-seconds.en.md)
- [Partner Evaluation Route](../usage/partner-evaluation-route.en.md)
- [Forking DEOS](../usage/forking-deos.en.md)
- [Minimal Fork Profile](../usage/minimal-fork-profile.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [Validation Troubleshooting](../usage/validation-troubleshooting.en.md)
