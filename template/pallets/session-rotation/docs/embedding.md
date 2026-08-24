# Embedding DEOS Session Rotation

## Host Obligations

- Implement `SessionRotation<BlockNumber>` with one deterministic bounded schedule and one rotation operation.
- Disable any pre-existing hook that would independently rotate the same session.
- Keep session prediction APIs logically aligned with `should_rotate`.
- Implement `BenchmarkHelper` under `runtime-benchmarks` so setup reaches the host's maximum admitted active and queued session geometry.
- Generate and bind a production `WeightInfo`; `()` is suitable only for tests.
- Include the generated maximum in fixed block-resource accounting.

## Reference Composition

The concrete DEOS runtime adapter, periodic cadence, collator/session implementation, benchmark geometry, pallet index, and generated runtime Weight live in the root runtime integration boundary rather than this reusable package.
