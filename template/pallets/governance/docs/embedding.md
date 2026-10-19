# Embedding `pallet-governance`

## Scope

This guide states the host-runtime obligations for embedding the reusable Governance pallet. The [specification](./specification.en.md) owns intended semantics and [package architecture](./architecture.en.md) owns implementation topology. Concrete DEOS domains, assets, accounts, origin bindings, parameters, and payload adapters remain in the root integration architecture.

The `0.7.22` package is fresh-genesis source only. It defines no migration from the `0.7.21` Governance storage baseline and must not be used to upgrade a deployed chain; a downstream live lineage owns its own explicit bounded migration before adopting this storage baseline.

## Authority and Payload Wiring

- Bind `AdminOrigin` to an explicit privileged origin; never use an unrestricted signed origin.
- Keep signed proposal submission behind `ProposalSubmissionAuthorityProvider` and `ProposalSubmissionEligibilityProvider`; eligibility admits proposal creation only and must not become direct payload dispatch authority.
- Define every domain through one coherent `GovernanceDomainPolicyProvider`, track-family provider, power-profile provider, urgent-policy provider, and submission-authority provider.
- Make `ProposalPayloadExecutor` exhaustive over admitted payload kinds. Unsupported domain/payload combinations fail closed and advisory kinds remain non-executable.
- Bind the queryable `MaxCidBytes`; the executor's CID type is `CidBytesOf<Runtime>`, shared with receipts, retained details and events. Include the resulting maximum encoded record in storage/ProofSize qualification.
- For Invoice payloads, validate complete canonical CIDv1 bytes and the explicit domain-approved treasury at witness preparation and execution; cached signed admission must recheck current authority. Populate target, actual and capacity-limited receipt fields from the actual atomic payout, not from an approval-time reservation.
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
- Bind the cfg-only `BenchmarkHelper` CID parameter to `CidBytesOf<Runtime>`. Its `prepare_maximum_finalized_execution` must return coherent host domain/kind/hash and a reachable maximum-sized success detail; the finalized-cleanup fixture rejects an unconfigured helper or any detail/record below `MaxEncodedLen`. Independently verify the template against real host execution. A seeded terminal record is not proof of its complete public history or physical cost.
- In native `std + runtime-benchmarks` tests, `Pallet::verify_maximum_finalized_cleanup_benchmark(recording)` validates the existing fixture at the configured maximum through SDK `BenchmarkingSetup::instance`, verification enabled. Supply an SDK `Recording` adapter: no-op for semantics or explicit before/after counter observation outside verification. The helper refuses unsupported bounds rather than shrinking them and is absent from ordinary/no-std builds. Counter, timing or proof observations are not automatically complete production resource bounds.
- Native `verify_pending_enactment_benchmark(cleanup_refusal, before_service)` selects the existing simple fixture with `None` or full topology with `Some(false/true)`. The read-only observer runs via SDK `TestRecording`, not numerical recording. Ordinary due must equal approval plus configured delay; full topology skips zero-delay/future-peer combinations because future approval would not yet exist. Test supported and unsupported host cases without changing production policy.
- With `std + runtime-benchmarks`, `prepare_full_pending_enactment_hook(epoch, cleanup_refusal, payload_source)` reuses full-topology setup without service and returns its one-shot verifier. Use a representable epoch with positive due approval (`epoch−delay`) and already-approved future peers. `None` selects the default helper; `Some((domain, kind, hash, winning_option))` supplies an already-noted executable payload and optional primary winner without invoking it. The host owns concrete payload/domain semantics. Unsupported kinds/missing preimages skip before Governance seeding. Seeded approval is not public authority, recorded resources or production qualification.
- `prepare_full_pending_enactment_parent(child_epoch, payload_source)` uses predecessor P=E−1, dates terminal peers at P and retains them in P+retention separately from the initially empty child E+retention destination. It offers no artificial dense-destination refusal. Every approval must already exist by P; full future cohorts incompatible with the delay/clock reject before host preparation. The host must match its epoch/block relation and preserve the verifier's retained peer bucket, or own a different complete historical caller. Dates alone do not establish producer/admission reachability, System/header validity, public ancestry or a sealed/PVF-ready parent. Existing same-epoch and simple diagnostics remain available.
- Call the verifier with `&[]` for the unchanged all-success requirement, or a bounded explicit slice of unique `(due_item_id, exact_failure_reason)` expectations. Unlisted due items must succeed; listed items must commit `ExecutionFailed` with that reason. No arbitrary terminal result is accepted. Future peers retain winning options as well as records/metadata/due. The refusal profile requires an empty slice and exact root apart from cursor normalization: unrelated mutations require a separate caller-root composition, not suppressed assertions.

## Minimum Validation

- Prove every privileged origin rejects an ordinary signer and every admitted typed payload executes only under its declared authority.
- Exercise delayed-clock catch-up, full same-epoch suffix retention, terminal epoch arithmetic, and eventual chronological convergence.
- Exercise concurrent and replacement ballots, incremental custody growth, maximum-horizon extension, mature release, aggregate reconciliation, and exact rollback after lock and unlock faults.
- Run package tests, host integration tests, benchmark compilation, production Weight generation, try-runtime checks, and workspace Clippy with warnings denied.
