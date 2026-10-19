# Economic Simulators

This directory contains independently scoped economic models. Each simulator owns its mathematical assumptions, configuration, scenarios, tests and usage instructions; the directory does not impose a shared simulation engine.

## Shipped Model

- [TMCTOL](./tmctol/README.md): Deterministic JavaScript/BigInt reference for minting-curve mathematics, treasury-owned liquidity and the modeled economic invariants.

From the repository root:

```bash
node ./simulators/tmctol/tests.js
```

## Adding a Model

- Place a distinct economic model in its own meaningfully named directory with a `README.md`, an explicit proof boundary and independently runnable validation.
- Use the existing model's supported configuration and scenarios when only its parameters differ; a different mechanism may justify a separate simulator.
- Keep model-specific policy and tests with that model. Extract shared code only after a real second consumer establishes a reusable mathematical contract.
- A downstream model may depend on an upstream model's public interface; upstream models must not depend on downstream directories or require registration of downstream identities.
- The project validation entrypoint explicitly runs the shipped TMCTOL suite. Adding a directory does not silently add an executable to project or upstream CI.
- Economic simulation does not replace runtime, storage, authority, scheduling, Weight, host or client validation.

## Related Entrypoints

- [Documentation namespaces](../docs/README.md)
- [Repository overview](../README.md)
