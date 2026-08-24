# Session Rotation Architecture

## 1. Package Shape

`src/lib.rs` defines the FRAME pallet, the `SessionRotation<BlockNumber>` host boundary, the generated-Weight interface, and focused mock evidence. `src/benchmarking.rs` owns runtime-specific setup, measured rotation, and post-rotation verification.

## 2. Execution

`on_initialize(now)` asks the host schedule whether `now` is a rotation boundary. The false branch returns zero without invoking the rotator. The true branch invokes the rotator once and returns the generated rotation Weight.

## 3. State and Failure Surface

The package has no storage, calls, events, origins, or cleanup path. Rotation behavior and failure semantics remain those of the host adapter. An invalid host adapter is a runtime-composition defect rather than pallet-managed recoverable state.

## 4. Weight Boundary

The package's `WeightInfo` has one owner, `rotate_session`. The benchmark helper can establish host-specific maximum validator/key geometry without moving that policy into the reusable pallet. Database reads and writes performed by the host rotation are captured by generated benchmark evidence.
