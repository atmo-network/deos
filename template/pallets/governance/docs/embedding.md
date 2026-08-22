# Embedding `pallet-governance`

## Scope

This guide states the host-runtime obligations for embedding the reusable Governance pallet. The [specification](./specification.en.md) owns intended semantics and [package architecture](./architecture.en.md) owns implementation topology. Concrete DEOS domains, assets, accounts, origin bindings, parameters, and payload adapters remain in the root integration architecture.

The `0.7.22` package is fresh-genesis source only. It defines no migration from the `0.7.21` Governance storage baseline and must not be used to upgrade a deployed chain; a downstream live lineage owns its own explicit bounded migration before adopting this storage baseline.

## Authority and Payload Wiring

- Bind `AdminOrigin` to an explicit privileged origin; never use an unrestricted signed origin.
- Keep signed proposal submission behind `ProposalSubmissionAuthorityProvider` and `ProposalSubmissionEligibilityProvider`; eligibility admits proposal creation only and must not become direct payload dispatch authority.
- Define every domain through one coherent `GovernanceDomainPolicyProvider`, track-family provider, power-profile provider, urgent-policy provider, and submission-authority provider.
- Make `ProposalPayloadExecutor` exhaustive over admitted payload kinds. Unsupported domain/payload combinations fail closed and advisory kinds remain non-executable.
- Keep runtime-upgrade authorization typed through `ProposalRuntimeUpgradeAuthorizationProvider`; proposal success must not accept or dispatch arbitrary code bytes directly.
- Make preimage existence, note cost, and execution ownership explicit through the preimage providers. Missing or invalid preimages must not produce successful execution state.
- Implement bounded typed witness validation with a defensible encoded ceiling for every payload kind. Witness preparation validates caller-supplied globally bounded bytes against their runtime hash and compact noted-preimage status; neither preparation nor signed proposal submission may read the generic preimage value.

## Epoch Service and Bounds

- `EpochProvider` must return one monotonic epoch domain that round-trips exactly through `u32` for every admitted value.
- Bind `MaxEpochCatchUpPerBlock` to `MAX_EPOCH_CATCH_UP_HARD_LIMIT`, currently one chronological epoch. Raising this constant requires a new measured service composition rather than a configuration-only change.
- Choose maturity, pending-enactment, finalized-outcome, and reward-expiry per-block caps no larger than their corresponding per-epoch storage bounds.
- Reserve `on_initialize` capacity for the generated base catch-up path plus the maximum configured composition of every phase owner reachable in one call, in both RefTime and ProofSize.
- Do not advance or externally rewrite `LastProcessedEpoch` or `CurrentEpochServicePhase`. The pallet advances only after every ordered suffix in the owned epoch drains.
- Use checked epoch deadlines and reject an unrepresentable horizon. A host must not clamp proposal, confirmation, enactment, retention, or reward-expiry deadlines.

## Voting Power and Custody

- `ProposalVoteWeightProvider` and `VetoVotePowerProvider` must return bounded deterministic power from canonical host state and must not depend on an off-chain index.
- For transferable voting sources, `VotePowerCustody::target_amount` returns the checked total position the account may reuse, while `custodied_amount` reports the exact host-ledger balance already held for that lock ID.
- Custody locking admits only the positive checked difference between target and already custodied value. Replacement and concurrent ballots reuse the same source position and extend release only to the maximum horizon.
- Custody lock and release operations must participate in the Governance transaction. A late storage, bucket, tally, or adapter failure must restore voter balance, custody balance, position, horizon, ballot, and event state together.
- Use distinct stable custody identities for independent source ledgers and prove they cannot alias fee, staking, Actor, liquidity, or other protocol custody accounts.
- If a host has no transferable source, its adapter may report no custody requirement, but it must not fabricate transferable custody or weaken the configured power source.

## Storage, Fees, and Read Surfaces

- `ProposalOpeningFee` and `ProposalFeeRecipient` must use a deposit-capable host account and preserve fee transfer atomicity with proposal admission; `PayloadAdmissionWitnessDeposit` must be nonzero and economically bound every retained compact witness until successful signed submission consumes it.
- Configure every active, per-author, per-epoch, lookback, account-batch, finalized-retention, and expiry bound from defensible host limits.
- Treat bounded current proposal, tally, policy, reward-memory, and retained-outcome queries as canonical chain truth. Permanent history, search, and analytics belong outside consensus storage.
- Do not infer missing or malformed proposal, ballot, reward, custody, or phase state as an empty successful value where the public API exposes typed failure.

## Weight and Integrity Evidence

- Bind host-generated `WeightInfo` measured against the composed production runtime. The package's zero `()` implementation is mock-only and is not production evidence.
- Benchmark maximum voters, winning accounts, active proposals, custody branches, and each epoch-service family with worst-case database topology and ProofSize.
- Assert that the base epoch path plus each maximum active phase fits the host's `on_initialize` budget independently in both Weight dimensions.
- Enable try-state in upgrade and release validation. It reconciles bounded proposal indexes, epoch buckets and phase ownership, reward windows, retained outcomes, locks, payload-witness semantic keys/contracts, and host-reported aggregate custody.
- Keep benchmark helpers behind `runtime-benchmarks`; they may construct worst-case state but must not define production policy.

## Minimum Validation

- Prove every privileged origin rejects an ordinary signer and every admitted typed payload executes only under its declared authority.
- Exercise delayed-clock catch-up, full same-epoch suffix retention, terminal epoch arithmetic, and eventual chronological convergence.
- Exercise concurrent and replacement ballots, incremental custody growth, maximum-horizon extension, mature release, aggregate reconciliation, and exact rollback after lock and unlock faults.
- Run package tests, host integration tests, benchmark compilation, production Weight generation, try-runtime checks, and workspace Clippy with warnings denied.
