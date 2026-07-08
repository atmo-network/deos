# Polkadot SDK: Comprehensive Architecture Patterns and Best Practices [SDK 2606]

> `Status`: Active Standard (July 2026)
> `Version`: Polkadot SDK 2606.0.0 crate baseline + stable2606 operator line (node v1.24.0)
> `Rust`: 1.93.x stable toolchain line tested upstream

## Overview

This document is a comprehensive "kickstart" guide for developing with the modern Polkadot SDK. It targets AI agents and developers who may have been trained on outdated Substrate/SDK patterns. It integrates critical optimizations from the last year, including Agile Coretime, Omni Node architecture, and XCM v5.

`Critical Mandates`:

1. `Template Compliance is Non-Negotiable`: Deviations from the official `parachain-template` cause silent failures.
2. `Pallet Order is Critical`: Collator pallets MUST be declared in specific order (see Section 1.1).
3. `Async Backing is Default`: All modern parachains MUST support async backing via `FixedVelocityConsensusHook`.
4. `Omni Node is the Standard`: No more custom node boilerplate; use the white-labeled Omni Node.

### 0.1 What Distinguishes SDK 2606 from 2503/2506 Era Baselines

The practical difference is not a single API change but a full operational baseline shift:

- `Consensus startup discipline`: pallet declaration order (Session before Aura) is now a hard runtime liveness gate
- `Genesis authority model`: no direct Aura genesis config when Session manages keys
- `Runtime API execution model`: `LazyBlock` conversion is required in core/block-builder paths
- `Async backing as default posture`: `FixedVelocityConsensusHook` and related slot/velocity settings are first-class
- `XCM stack maturity`: XCM v5 + XCMP queue migration wiring is expected in modern runtimes
- `Migration placement`: single-block migrations are configured in `frame_system::Config`
- `Node architecture`: Omni Node replaces custom parachain node boilerplate for production parity
- `Assets integration`: reserves fields are part of the expected pallet-assets wiring surface

In short, SDK 2606 keeps the 2603-era production baseline and adds a few important execution and operator-surface shifts that matter during upgrades.

### 0.2 What Changes Specifically When Moving from 2603 to 2606

The 2606 line is not a conceptual reset, but it does tighten several concrete contracts:

- `Runtime upgrade staging`: `system_version >= 3` remains required for RFC-123 `:pending_code` staging and next-block promotion to `:code`
- `Relay-parent runtime API`: `RelayParentOffsetApi` now exposes `max_claim_queue_offset`; the current DEOS line returns `1` while keeping `relay_parent_offset = 0`
- `Parachain-system scheduling`: `cumulus_pallet_parachain_system::Config` now requires `SchedulingSignatureVerifier`; use `()` unless V3 scheduling is deliberately enabled after relay/collator readiness
- `Asset-conversion fee type`: `pallet_asset_conversion::Config::LPFee` now expects `Get<Permill>`, not a raw numeric constant wrapper
- `Client/operator line`: the current stable release is `polkadot-stable2606` (node v1.24.0), matching the 2606.0.0 Rust crate baseline

For DEOS specifically, the relevant 2606 deltas are the runtime API method addition, the explicit V3-scheduling verifier type, the asset-conversion LP fee type change, and the local binary tag update. No local V3 scheduling enablement is implied by this upgrade.

---

## 1. Critical Breaking Changes in SDK 2606

These are the immediate technical blockers encountered when upgrading from previous versions.

### 1.1 Pallet Declaration Order (CRITICAL)

`Impact`: `CRITICAL` | `Scope`: `runtime/src/lib.rs`

`The order of collator-related pallet declarations determines genesis initialization order.` `pallet_session` MUST be declared BEFORE `pallet_aura` because Session initializes Aura authorities at genesis.

`Symptom`: Parachain starts but never produces blocks. `AuraApi_authorities` returns empty vector (`0x00`).

`Root Cause`: When Aura is initialized before Session, it receives an empty authorities list.

`Correct Pattern`:

```rust
#[frame_support::runtime]
mod runtime {
    // ... system pallets ...

    // Collator support. THE ORDER OF THESE 4 IS CRITICAL AND SHALL NOT CHANGE.
    #[runtime::pallet_index(20)]
    pub type Authorship = pallet_authorship;
    #[runtime::pallet_index(21)]
    pub type CollatorSelection = pallet_collator_selection;
    #[runtime::pallet_index(22)]
    pub type Session = pallet_session;      // MUST be BEFORE Aura
    #[runtime::pallet_index(23)]
    pub type Aura = pallet_aura;            // MUST be AFTER Session
    #[runtime::pallet_index(24)]
    pub type AuraExt = cumulus_pallet_aura_ext;

    // ... other pallets ...
}
```

`Verification`: After starting zombienet, query aura authorities:

```bash
curl -s -H "Content-Type: application/json" \
  -d '{"id":1, "jsonrpc":"2.0", "method":"state_call", "params":["AuraApi_authorities", "0x"]}' \
  http://localhost:9988 | jq
```

Should return non-empty result (e.g., `0x0490b5ab...` for Alice).

### 1.2 Genesis Config: No AuraGenesisConfig

`Impact`: `High` | `Scope`: `genesis_config_presets.rs`

When using `pallet-session` to manage authorities, do NOT configure `pallet-aura` genesis directly. Session pallet establishes aura authorities during session initialization.

`Wrong Pattern`:

```rust
fn testnet_genesis(...) -> Value {
  build_struct_json_patch!(RuntimeGenesisConfig {
    // ...
    aura: AuraGenesisConfig {
      authorities: invulnerables.iter().map(|(_, aura)| aura.clone()).collect(),
    },
    // ...
  })
}
```

`Correct Pattern`:

```rust
fn testnet_genesis(...) -> Value {
  build_struct_json_patch!(RuntimeGenesisConfig {
    // NO aura config - Session manages authorities
    session: SessionConfig {
      keys: invulnerables
        .into_iter()
        .map(|(acc, aura)| (acc.clone(), acc, template_session_keys(aura)))
        .collect(),
    },
    collator_selection: CollatorSelectionConfig {
      invulnerables: invulnerables.iter().map(|(acc, _)| acc.clone()).collect(),
      candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
    },
    // ...
  })
}
```

### 1.3 Para ID Configuration

`Impact`: `High` | `Scope`: Zombienet config, genesis presets

`Option A: Use Pre-registered Para ID 1000`

`rococo-local` relay chain has Para ID 1000 pre-registered in genesis.

```rust
pub const PARACHAIN_ID: u32 = 1000;
```

```toml
[[parachains]]
id = 1000
chain_spec_path = "./chain_spec.json"
```

`Option B: Use Custom Para ID (e.g., 2000) with Onboarding`

For custom Para IDs, add `onboard_as_parachain = true` to zombienet config.

```rust
pub const PARACHAIN_ID: u32 = 2000;
```

```toml
[[parachains]]
id = 2000
chain_spec_path = "./chain_spec.json"
onboard_as_parachain = true  # Required for non-genesis Para IDs
```

`onboard_as_parachain = true` makes zombienet automatically register the parachain on relay chain via sudo.

### 1.4 `LazyBlock` in Runtime APIs

`Impact`: `High` | `Scope`: `runtime/src/apis.rs`

The `Core`, `BlockBuilder`, and `TryRuntime` APIs now utilize `LazyBlock` for performance optimization. This requires explicit type conversion using `.into()`.

`Pattern`:

```rust
impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn execute_block(block: Block) {
            // MUST convert Block -> LazyBlock
            Executive::execute_block(block.into())
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            // MUST convert Block -> LazyBlock
            data.check_extrinsics(&block.into())
        }
    }
}
```

### 1.5 `pallet-assets` Reserves

`Impact`: `Medium` | `Scope`: Runtime Config & Mocks

`pallet-assets` now supports asset reservation natively.

`Runtime Config`:

```rust
impl pallet_assets::Config for Runtime {
    // ...
    type ReserveData = (); // Use unit type if custom data not needed
}
```

`Mock Genesis`:

```rust
polkadot_sdk::pallet_assets::GenesisConfig::<Test> {
    assets: vec![],
    metadata: vec![],
    accounts: vec![],
    reserves: vec![], // NEW FIELD - MUST BE INITIALIZED
    next_asset_id: None,
}
```

### 1.6 Migration Configuration

`Impact`: `Medium` | `Scope`: `runtime/src/lib.rs`, `configs/mod.rs`

Migrations are now defined in `frame_system::Config`, not in `Executive`.

`SDK 2606 Pattern`:

```rust
// runtime/src/lib.rs
pub type Executive = frame_executive::Executive<
  Runtime,
  Block,
  frame_system::ChainContext<Runtime>,
  Runtime,
  AllPalletsWithSystem,
  // No Migrations parameter!
>;

// runtime/src/configs/mod.rs
impl frame_system::Config for Runtime {
    // For fresh chains, use empty migrations
    type SingleBlockMigrations = ();

    // For upgrades, define migrations tuple:
    // type SingleBlockMigrations = crate::Migrations;
}
```

### 1.7 XCM v5 & XCMP Queue

`Impact`: `High` | `Scope`: Runtime Config

```rust
impl cumulus_pallet_xcmp_queue::migration::v5::V5Config for Runtime {
    type ChannelList = ParachainSystem;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    // ...
}
```

---

## 2. Consensus & Block Production

### 2.1 Why Blocks Aren't Produced

If your parachain starts but doesn't produce blocks, check in order:

1. `Pallet Order`: Is Session declared before Aura? (Section 1.1)
2. `Aura Authorities`: Query `AuraApi_authorities` - should return non-empty list
3. `Para ID`: Does it match relay chain genesis? (1000 for rococo-local)
4. `Collator Keys`: Is the collator (e.g., Charlie) in session keys?
5. `Coretime`: For production, ensure coretime is purchased

### 2.2 Async Backing Configuration

```rust
mod async_backing_params {
    pub(crate) const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
    pub(crate) const BLOCK_PROCESSING_VELOCITY: u32 = 1;
    pub(crate) const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;
}

pub type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
    Runtime,
    RELAY_CHAIN_SLOT_DURATION_MILLIS,
    BLOCK_PROCESSING_VELOCITY,
    UNINCLUDED_SEGMENT_CAPACITY,
>;

// Critical: register_validate_block! macro
cumulus_pallet_parachain_system::register_validate_block! {
    Runtime = Runtime,
    BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
```

### 2.3 Session Configuration

```rust
impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = CollatorSelection;
    // Critical: use KeyTypeIdProviders for proper key handling
    type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type Currency = Balances;
    type KeyDeposit = ();
    type DisablingStrategy = ();
    type WeightInfo = ();
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = ConstU32<100_000>;
    type AllowMultipleBlocksPerSlot = ConstBool<true>; // Required for async backing
    type SlotDuration = ConstU64<SLOT_DURATION>;
}
```

---

## 3. Core Architecture Patterns

### 3.1 Unified Dependency Management

Use workspace-level dependency management with edition 2024.

```toml
[workspace.package]
edition = "2024"

[workspace.dependencies]
polkadot-sdk = { version = "2606.0.0", default-features = false }
frame = { package = "polkadot-sdk-frame", version = "0.18.0", default-features = false }
cumulus-pallet-parachain-system = { version = "0.29.0", default-features = false }
substrate-wasm-builder = "34.0.0"
codec = { package = "parity-scale-codec", version = "3.7.5", default-features = false, features = ["derive"] }
scale-info = { version = "2.11.6", default-features = false, features = ["derive"] }
sp-arithmetic = { version = "28.0.1", default-features = false }

[workspace.lints.clippy]
large_futures = "allow"
type_complexity = "allow"

[workspace.lints.rust]
unsafe_code = "deny"
```

### 3.2 Token Decimals Standard

Use 12-decimal standard (matching DOT/KSM):

```rust
pub const UNIT: Balance = 1_000_000_000_000;      // 10^12
pub const MILLI_UNIT: Balance = 1_000_000_000;    // 10^9
pub const MICRO_UNIT: Balance = 1_000_000;        // 10^6
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_UNIT;
```

### 3.3 Modern Runtime Construction

```rust
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    // System
    #[runtime::pallet_index(0)]
    pub type System = frame_system;
    #[runtime::pallet_index(1)]
    pub type ParachainSystem = cumulus_pallet_parachain_system;
    #[runtime::pallet_index(2)]
    pub type Timestamp = pallet_timestamp;
    #[runtime::pallet_index(3)]
    pub type ParachainInfo = parachain_info;
    #[runtime::pallet_index(4)]
    pub type WeightReclaim = cumulus_pallet_weight_reclaim;

    // Monetary
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type TransactionPayment = pallet_transaction_payment;

    // Governance
    #[runtime::pallet_index(15)]
    pub type Sudo = pallet_sudo;

    // Collator support - ORDER IS CRITICAL
    #[runtime::pallet_index(20)]
    pub type Authorship = pallet_authorship;
    #[runtime::pallet_index(21)]
    pub type CollatorSelection = pallet_collator_selection;
    #[runtime::pallet_index(22)]
    pub type Session = pallet_session;
    #[runtime::pallet_index(23)]
    pub type Aura = pallet_aura;
    #[runtime::pallet_index(24)]
    pub type AuraExt = cumulus_pallet_aura_ext;

    // XCM
    #[runtime::pallet_index(30)]
    pub type XcmpQueue = cumulus_pallet_xcmp_queue;
    #[runtime::pallet_index(31)]
    pub type PolkadotXcm = pallet_xcm;
    #[runtime::pallet_index(32)]
    pub type CumulusXcm = cumulus_pallet_xcm;
    #[runtime::pallet_index(33)]
    pub type MessageQueue = pallet_message_queue;

    // Custom pallets
    #[runtime::pallet_index(50)]
    pub type MyPallet = my_pallet;
}
```

---

## 4. Omni Node & Production Readiness

### 4.1 Required Runtime APIs

Missing runtime APIs don't cause compilation errors but prevent block production.

```rust
// RelayParentOffsetApi - Critical for relay chain communication
impl cumulus_primitives_core::RelayParentOffsetApi<Block> for Runtime {
    fn relay_parent_offset() -> u32 {
        0  // Standard offset
    }

    fn max_claim_queue_offset() -> u8 {
        1  // Keep V3 scheduling disabled unless intentionally adopted
    }
}

// AuraApi - Must return non-empty authorities
impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
    fn slot_duration() -> sp_consensus_aura::SlotDuration {
        Runtime::impl_slot_duration()
    }

    fn authorities() -> Vec<AuraId> {
        pallet_aura::Authorities::<Runtime>::get().into_inner()
    }
}
```

### 4.2 TxExtension Structure (SDK 2606)

```rust
pub type TxExtension = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
  Runtime,
  (
    frame_system::AuthorizeCall<Runtime>,
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
  ),
>;
```

---

## 5. Development Workflow

### 5.1 Zombienet Local Testing

```toml
# zombienet.toml
[relaychain]
default_command = "polkadot"
chain = "rococo-local"

[[relaychain.nodes]]
name = "alice"
validator = true
rpc_port = 9944

[[relaychain.nodes]]
name = "bob"
validator = true
rpc_port = 9955

[[parachains]]
id = 2000
chain_spec_path = "./chain_spec.json"
onboard_as_parachain = true  # Required for custom Para IDs

[parachains.collator]
name = "charlie"
rpc_port = 9988
command = "polkadot-omni-node"
args = ["--force-authoring"]
```

### 5.2 Debugging Checklist

If blocks aren't produced:

| Check            | Command                         | Expected                      |
| ---------------- | ------------------------------- | ----------------------------- |
| Pallet order     | Review `lib.rs`                 | Session before Aura           |
| Aura authorities | `curl ... AuraApi_authorities`  | Non-empty (`0x04...`)         |
| Para ID          | Check zombienet.toml vs genesis | Match + onboard if custom     |
| Epoch            | Wait for relay epoch 1          | ~60 seconds                   |
| Collator keys    | Check chain_spec.json session   | Collator in keys              |
| Onboarding       | Custom Para ID?                 | `onboard_as_parachain = true` |

### 5.3 Useful RPC Queries

```bash
# Check aura authorities (should be non-empty)
curl -s -H "Content-Type: application/json" \
  -d '{"id":1, "jsonrpc":"2.0", "method":"state_call", "params":["AuraApi_authorities", "0x"]}' \
  http://localhost:9988 | jq

# Check parachain heads on relay (should show para 1000)
curl -s -H "Content-Type: application/json" \
  -d '{"id":1, "jsonrpc":"2.0", "method":"paras_heads", "params":[]}' \
  http://localhost:9944 | jq
```

---

## 6. Summary Table

| Issue              | Symptom                  | Root Cause            | Fix                               |
| ------------------ | ------------------------ | --------------------- | --------------------------------- |
| No blocks          | `Idle (0 peers)` forever | Pallet order wrong    | Session before Aura               |
| Empty authorities  | `AuraApi` returns `0x00` | Aura before Session   | Fix pallet indices                |
| Para not found     | Relay ignores para       | Wrong Para ID         | Add `onboard_as_parachain = true` |
| Genesis fail       | Node crashes on start    | Bad AuraGenesisConfig | Remove, let Session manage        |
| High candidacyBond | Collators can't register | Wrong decimals        | Use 12-decimal standard           |

---

## 7. Quick Reference

### Correct Pallet Order

```
System (0) → ParachainSystem (1) → Timestamp (2) → ParachainInfo (3) →
WeightReclaim (4) → Balances (10) → TransactionPayment (11) →
Sudo (15) → Authorship (20) → CollatorSelection (21) →
Session (22) → Aura (23) → AuraExt (24) → XCM pallets (30+)
```

### Key Constants

```rust
pub const PARACHAIN_ID: u32 = 2000;  // Use with onboard_as_parachain = true
pub const SLOT_DURATION: u64 = 6_000;
pub const UNIT: Balance = 1_000_000_000_000;
pub const EXISTENTIAL_DEPOSIT: Balance = 1_000_000_000;
```

Use this guide as the authoritative reference for all code generation tasks targeting the current Polkadot SDK.

---

## 8. Deterministic Pallet Patterns (TMCTOL Insights)

### 8.1 `on_idle` vs `on_initialize` for Deterministic Actors

`on_initialize` runs every block with guaranteed weight. `on_idle` runs only when spare weight exists. Choose based on actor requirements:

- `on_initialize` scan + `on_idle` execute: use when actors need guaranteed state awareness every block; example: liquidity-provisioning actors scanning balances before later execution
- `on_idle` only: use when delayed processing is acceptable and no guaranteed scan is needed; example: Burn Actor checking and processing balances in one call

`Key insight`: `on_initialize` is expensive (runs every block even when idle). Only use it when guaranteed-per-block scanning is essential (e.g., cooldown timers that must be checked before user transactions execute). For pure "process when possible" actors, `on_idle` alone is simpler and wastes zero weight on empty blocks.

### 8.2 LP Token Resolution in `pallet-asset-conversion`

LP token IDs (`PoolAssetId = u32`) are assigned by `PoolLocator` and do NOT encode the constituent asset pair. The `Pools` storage map (`PoolId → PoolInfo { lp_token }`) is the only source of truth.

`Reverse lookup` (LP token → asset pair) requires iterating `Pools`:

```rust
fn get_pool_pair_for_lp(lp_token_id: u32) -> Option<(AssetKind, AssetKind)> {
    for (pool_key, pool_info) in pallet_asset_conversion::Pools::<Runtime>::iter() {
        if pool_info.lp_token == lp_token_id {
            return Some(pool_key);
        }
    }
    None
}
```

O(N) where N = number of pools. Acceptable for ≤100 pools. For larger pool counts, consider a reverse `StorageMap<u32, PoolId>` populated at pool creation.

### 8.3 `pallet-assets` Callback Limitations

`AssetsCallback` (`CallbackHandle`) fires only on `created()` and `destroyed()` — NOT on individual transfers or mints. This means actors cannot use push-based notifications for balance changes. The workaround is poll-based scanning (see §8.1).
