# DEOS Backlog

> Open framework work only. Completed delivery history belongs in `CHANGELOG.md`; normative semantics remain owned by specifications, architecture documents, code, generated metadata, and production Weight.
>
> Pre-`1.0` boundary: no DEOS network will launch before `1.0`. The `0.7.x` line remains fresh-genesis and may change storage, metadata, runtime APIs, validation topology, and release mechanics without deployed-lineage migration or live-network ceremony.

## DEOS 0.7.24 — High-Frequency Actor Efficiency

- [ ] `Actors / Differential Overhead`: Establish paired production benchmarks for an identical bounded Router swap invoked manually and through a one-step price-reactive Actor; report only marginal detection, canonical-probe, materialization/FIFO, interpreter, fee-settlement, completion, and retry overhead after subtracting shared Router/Oracle/AMM work. Use the evidence to minimize generic orchestration cost, amortize shared feed/market reads across bounded cohorts, and evaluate a compiled `ObservationCrossing → bounded SwapOut → Complete/Retry` execution family without weakening FIFO fairness, cutoff, loss bounds, atomicity, or retry semantics.
