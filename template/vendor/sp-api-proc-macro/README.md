# SDK Client Transaction Backport

This is the published `sp-api-proc-macro 29.0.0` source and normalized manifest, with one correction in `src/impl_runtime_apis.rs`. `ApiExt::execute_in_transaction` now assigns the checked decrement to its logical transaction-depth counter after the closure returns. Explicit commit, rollback and nesting therefore restore the counter before later API calls choose their implicit host transaction.

The upstream source SHA-256 is `5466016552eef8393ba2b15a51c5f23fadd6c5c5f874f2e5439f05bebd9758a8`; the corrected file is `c2d2f02bd4372b402fb15c2bc43046131d2e9a8f6b84ac91789e6507e557be3d`. All other source files and the manifest retain their published bytes. The same upstream bytes and defect are present in the selected final `polkadot-stable2609` source at `7a8daa68e18718c18823c1109513fdc1c8aa71fe`; PR #13439's master correction is absent from this tag. Selecting this tag does not retire the correction. Captured stable2606-3 qualification remains historical.

`template/Cargo.toml` applies this backport to the reference workspace. Its `sdk_client_transactions`, current-domain `actor_wide_close` and terminal `actor_finalization_close` callers own executable checks; the [client transaction context](../../../docs/actors.integration.en.md#client-runtime-api-transaction-context) owns qualification and limitations. This patch does not modify the separately downloaded Omni Node executable. Retire it only when the selected SDK and node source restore transaction depth and the applicable caller regressions pass.

The package remains Apache-2.0 licensed, with original Parity copyright and license notices in every source file. DEOS's workspace release version does not replace this third-party package identity.
