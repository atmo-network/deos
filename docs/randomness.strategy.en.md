# Randomness Strategy: Relay-Beacon-First Simplification

> Status: `pallet-vrf` has been retired from the current DEOS reference runtime and workspace (as of 2026-03-24)
> Current direction: stay on trusted-collator previous-block-hash fallback unless a production-ready relay-chain **per-block protocol beacon** path (Safrole / Sassafras or equivalent) becomes parachain-consumable
> Scope: explain the post-VRF simplification decision, the current previous-block-hash fallback contract, and the conditions under which a future relay-beacon replacement would become acceptable
>
> This file is an operational launch note, not the core product narrative. DEOS currently ships with TMCTOL as its flagship economic standard, so randomness remains a secondary infrastructure detail relative to that economic line for the current launch path.

## 1. Canonical Position

DEOS no longer treats a parachain-local randomness pallet as part of the launch or near-term production contract.
The previous `pallet-vrf` commit/reveal line was intentionally removed rather than evolved further.

The new preferred direction is:

- Keep the runtime simple
- Keep native `$NTVE`-weighted collator security, but only across a trusted invulnerable collator set for the current line
- Accept that the first mainnet may begin with a team-operated collator set while the broader permissionless path stays deferred
- Drop same-block randomness as a requirement
- Accept deterministic previous-block-hash sampling as the temporary fallback until a qualifying relay beacon exists
- Only treat a new relay-chain **per-block protocol beacon** as a valid replacement target
- Only if that relay-beacon path fails, reconsider whether any local threshold runtime work is justified

In other words, the project now prefers **external relay randomness over a second local entropy economy**.

### 1.1 Immediate DEOS impact memo

The current implementation consequences are explicit:

- Previous-block hash is the accepted temporary fallback entropy source for local consumers
- The fallback is accepted only in combination with a trusted invulnerable, team-operated collator set
- Permissionless collators stay inactive until a relay-beacon-backed **per-block** randomness contract exists
- Native `$NTVE` binding remains the collator-weighting signal, not a randomness market
- Operator commission remains staking/economic metadata only
- Local threshold / hidden / ring-VRF work remains fallback research rather than the active roadmap

A recent upstream research signal points the same way: the Polkadot/JAM post-quantum roadmap frames future consensus randomness around a randomness beacon rather than around parachain-local VRF ownership. That reinforces TMCTOL's relay-beacon-first simplification, but it does **not** yet count as a production-ready parachain consumption contract.

## 2. Why the local VRF line was retired

The old local line solved a real problem, but it also carried a large amount of protocol-owned complexity:

- Commit/reveal timing
- Entropy membership lifecycle
- Missed-reveal accountability
- Reveal committee sizing and quorum tuning
- Draw quality handling
- Entropy reward routing
- Operator/delegator attribution for entropy payouts
- Local research pressure toward threshold / hidden / ring-VRF evolution

That complexity no longer matches the current product requirements.

The decisive simplification inputs were:

- Same-block fairness is no longer required
- The protocol is willing to rewrite around relay-chain randomness if that becomes production-ready
- A second local entropy economy is not worth maintaining just to preserve optional future cryptographic ambition

## 3. Current runtime contract

The current runtime line now assumes:

1. there is **no** pallet-owned local randomness subsystem in the workspace
2. there is **no** local entropy-provider reward surface
3. there is **no** local entropy membership lifecycle
4. there is **no** authored-block entropy obligation in the parachain runtime
5. collator security remains delegated-native-backed through staking, native binding, and session-time collator selection
6. the active collator set is intentionally trusted/permissioned (`Invulnerables`) until relay randomness exists
7. permissionless collators are not part of the current launch contract

This means the runtime still has an economic security path, but that path is now only about:

- Native binding attribution
- Trusted collator/operator targeting
- Session-time collator selection weighting inside the current permissioned collator set

It is no longer also responsible for maintaining a separate local randomness market.

## 4. AAA probability behavior after VRF removal

AAA remains able to use probabilistic timers, but the contract is intentionally simpler now.

Current behavior:

- The runtime does **not** bind AAA to a secure external entropy provider
- Probabilistic timers may fall back to deterministic previous-block-hash sampling
- That fallback is accepted only for the current trusted-collator phase and is not treated as a permissionless fairness claim
- Same-block consumer-safe randomness is not promised
- Probabilistic financial automation remains a convenience mechanism, not a cryptographic fairness claim

This is acceptable under the new product posture because the runtime no longer claims that local AAA probability gates are backed by a dedicated secure entropy subsystem.

## 5. Staking after VRF removal

`pallet-staking` remains important, but its role is narrower and clearer.

It still provides:

- Multi-asset share-vault staking pools
- Native binding attribution
- Operator commission configuration for staking/economic accounting only
- Delegated native backing queries
- The weighting input for session-time collator selection inside the trusted collator set

It does **not** currently provide:

- A local entropy membership layer
- A local reveal committee weighting surface
- Entropy reward routing
- Any local threshold witness contract

So the staking design should now be understood as:

- Generic economic staking substrate for many assets
- Native binding as the collator/operator weighting signal for the current trusted collator set
- No hidden second responsibility as a local randomness substrate

## 6. Preferred future: relay-beacon replacement

The preferred future protocol shape is now:

- Relay chain provides the canonical beacon
- Parachain derives domain-separated local randomness from relay inputs when needed
- Permissionless collators are considered only after that beacon path is real enough to support them
- TMCTOL does not maintain a second independent entropy protocol unless forced to

The project is specifically interested in whether parachain runtime logic can safely consume relay-chain randomness through the relay-state proof surface.

Relevant relay-chain surfaces already visible in the Polkadot SDK ecosystem include keys such as:

- `CURRENT_BLOCK_RANDOMNESS`
- `ONE_EPOCH_AGO_RANDOMNESS`
- `TWO_EPOCHS_AGO_RANDOMNESS`
- `CURRENT_SLOT`
- `EPOCH_INDEX`

Current SDK audit result:

- `cumulus-client-parachain-inherent` already requests `CURRENT_BLOCK_RANDOMNESS`, `ONE_EPOCH_AGO_RANDOMNESS`, `TWO_EPOCHS_AGO_RANDOMNESS`, `CURRENT_SLOT`, and `para_head(para_id)` as part of the relay proof key set used for parachain inherents
- `cumulus_pallet_parachain_system::RelayChainStateProof` already exposes generic runtime-side `read_entry` / `read_optional_entry` access over the proved relay storage backend, in addition to dedicated helpers like `read_slot()` and `read_included_para_head()`
- Therefore the main missing piece is no longer proof transport; it is product acceptance: should any future relay surface count as a real per-block protocol beacon for TMCTOL, or should the runtime stay on trusted-collator previous-block-hash fallback until such a beacon actually exists

The strategic question is no longer `how do we build a better local commit/reveal beacon?`.
It is now `when, if ever, does a relay-provided per-block beacon become real enough to replace the current fallback contract?`.

### 6.1 Canonical future gate

The future relay-beacon contract is **not** currently locked to any existing relay randomness item.
TMCTOL will only revisit relay-beacon adoption if the relay ecosystem exposes a **new per-block protocol beacon** that is parachain-consumable with a stable production contract.

Current visible relay randomness items such as:

- `CURRENT_BLOCK_RANDOMNESS`
- `ONE_EPOCH_AGO_RANDOMNESS`
- `TWO_EPOCHS_AGO_RANDOMNESS`

are therefore **audited inputs, not accepted product targets**.
They do not currently qualify as TMCTOL's canonical replacement for local previous-block-hash fallback.

### 6.2 Current accepted contract until that gate is met

Until such a new per-block protocol beacon exists:

- Local consumers use deterministic previous-block-hash sampling
- That fallback is accepted **only** during the trusted, invulnerable collator phase
- The fallback MUST NOT be described as same-block fair entropy
- The fallback MUST NOT be used as justification for activating permissionless collators
- Existing epoch-scale relay randomness surfaces MUST NOT be promoted into the runtime as a pretend replacement beacon just because they are technically readable

This is the actual current TMCTOL contract.
The project prefers an honest trusted-collator fallback over pretending that an epoch-scale relay randomness item already solves the product problem.

### 6.3 Conditional future ingestion pattern

If a real per-block parachain-consumable relay beacon appears later, the preferred runtime integration pattern is still:

1. ingest it through a **weight-accounted parachain-system `ConsensusHook` wrapper** rather than through hot-path proof reconstruction
2. keep the existing `FixedVelocityConsensusHook` logic as the inner/base consensus rule for slot and unincluded-segment validation
3. materialize one compact per-block snapshot for downstream consumers
4. let the runtime `EntropyProvider` derive subject-specific entropy from that snapshot later in the block

That topology remains a future implementation preference, not a current runtime task against the existing epoch-scale relay randomness items.

## 7. What must be true before adopting relay randomness as the main path

The relay-beacon path becomes the preferred production solution only if all of the following are true.

### 7.1 Availability

Parachain runtimes must be able to access the relevant relay randomness with a stable, documented, production-grade contract.

### 7.2 Semantics

The minimum acceptable future semantics are now clearer than before:

- The beacon must be **per-block**, not merely epoch-scale
- The beacon must be parachain-consumable through a stable production contract
- Same-block fairness is still **not** a required product claim
- Until that gate is met, the runtime stays on previous-block-hash fallback under trusted collators

What remains open is the concrete shape of that future per-block beacon, because the currently visible relay randomness items do not yet satisfy the required contract.

### 7.3 Collator-knowledge acceptance

The project does **not** currently accept the existing epoch-scale relay randomness items as sufficient to widen the collator set.
Permissionless collators remain deferred until a future per-block relay/protocol beacon exists and its consumer semantics are explicitly accepted.

### 7.4 Activation gate

The project must be comfortable making relay randomness the gate for widening the collator set.
Until that gate exists, TMCTOL keeps the current trusted, team-operated invulnerable collator set and does not activate permissionless collators.

### 7.5 Economic acceptance

The project must be comfortable with the fact that moving to relay randomness means:

- No local entropy-provider rewards
- No local missed-reveal penalties
- No local entropy-operator accountability surface
- Dependence on relay-chain beacon quality and availability

If those trade-offs are acceptable, relay randomness is the cleaner architecture.

## 8. Fallback if relay-beacon replacement stalls

Local threshold / hidden / ring-VRF work is no longer the default roadmap.
It should only be reconsidered if the relay-beacon replacement path proves unusable.

Only then should the project reopen questions such as:

- Local BLS-like compact proof verification
- Local signer-bitmap accountability
- Receipt sidecars
- Hidden participation or ring-VRF experimentation
- Whether permissionless collators need a different temporary activation path

Until then, those lines are optional fallback research, not the active product path.

## 9. Non-goals of the current line

The current simplified line explicitly does **not** promise:

- Same-block fair randomness
- A local entropy reward economy
- Hidden committee selection in the parachain runtime
- Ring-VRF anonymity in the parachain runtime
- Threshold aggregation in the parachain runtime
- Permissionless collator activation before the relay-beacon question is settled
- A local `pallet-vrf` resurrection before the relay-beacon question is settled

## 10. Code anchors

Current repository anchors for the simplified posture:

- `template/runtime/src/configs/aaa_config.rs`
- `template/runtime/src/configs/staking_config.rs`
- `template/runtime/src/configs/mod.rs`
- `template/pallets/staking/src/lib.rs`
- `template/runtime/src/lib.rs`
- `BACKLOG.md`

Upstream integration surfaces to monitor in `paritytech/polkadot-sdk`:

- Relay-chain well-known randomness keys
- Relay-state proof consumption surfaces
- Safrole / Sassafras production-readiness and parachain-consumable beacon support
