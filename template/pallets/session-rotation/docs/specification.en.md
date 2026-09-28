# Session Rotation Specification

## 1. Scope

The pallet owns one bounded `on_initialize` boundary for a host-defined session rotation. The host owns the rotation schedule, validator policy, session implementation, and production Weight evidence.

## 2. Invariants

- A block that is not selected by `SessionRotation::should_rotate` performs no rotation and returns zero Weight.
- A selected block invokes `SessionRotation::rotate` exactly once and returns exactly `WeightInfo::rotate_session`.
- The pallet stores no schedule, session state, validator state, or economic policy.
- A production runtime must bind a generated nonzero Weight owner measured against its maximum admitted session geometry, including all nested callback execution and host bookkeeping within `SessionRotation::rotate`.
- The complete rotation cost has one accounting owner. Host callbacks must not independently charge work already included in the generated complete-call owner; reconciling internal registration must preserve unrelated FRAME charges and all executed work.
- The schedule exposed through client-facing session-rotation prediction must remain logically identical to the schedule used by this hook.
- The underlying session pallet must not independently rotate the same block.

## 3. Benchmark Contract

With `runtime-benchmarks`, the host `BenchmarkHelper` prepares its maximum admitted session geometry before measurement and verifies the resulting rotation after measurement. The measured block contains only `SessionRotation::rotate`; setup and verification are excluded.
