# Distrusted Root Backport

## Source Identity

- Base: crates.io `webpki-roots 0.25.4`, archive SHA256 `5f20c57d8d7db6d3b86154206ae5d8fba62dd39573114de97c2cb0578251f8e1`, upstream commit `56a48f107273719904b3749010c35ef40e737039`.
- Upstream removal: <https://github.com/rustls/webpki-roots/pull/133>, commit `949c169900cedd2ab8614c3dcf5dc2ca455326ea`, retrieved combined patch SHA256 `67f952b4289c65dac3ee0bfad2a18ced5516dd932bf46a41000f3b2db92d3804`.
- Preserve upstream MPL-2.0 license and certificate-data notices. This is a locally modified source package, not an upstream fixed-version release.

## Change and Compatibility

- Remove exactly `emSign ECC Root CA - G3`, certificate SHA256 `86A1ECBA089C4A8D3BBE2734C612BA341D813E043CF9E8A862CD5C57A36BBE6B`, matching the upstream removal for `RUSTSEC-2026-0381`.
- Keep all other trust-anchor bytes unchanged and retain the 0.25 public `TrustAnchor` fields and `TLS_SERVER_ROOTS` slice. No new anchors, cryptography, dependencies or parent/API migration are introduced. Chains that rely solely on the distrusted root intentionally stop validating.
- The manifest is reduced to the existing production package identity and an isolated workspace. Upstream network-backed certificate-generation/verification integration tests and their dev dependencies are not selected. The original manifest and test sources remain for provenance; the bounded regression has no dependencies.
- This backport does not refresh the entire historical Mozilla bundle or claim that every retained root is currently trusted. It closes only the named upstream root-removal defect.

## Validation

From the repository root:

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true ./scripts/ci-local.sh --only tests --manifest-path template/vendor/webpki-roots/Cargo.toml --package webpki-roots --test-filter security_wire
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true ./scripts/ci-local.sh --only check --package libp2p-websocket
```

The regression rejects the removed subject and exact public-key bytes and checks the 147-to-146 anchor-count reduction. The selected parent check uses the reference workspace lock; standalone tests use their own dependency-free lock. Source comparison against the archived 0.25.4 package must show only the named trust-anchor removal and test code in `src/lib.rs`.

Cargo Audit omits local path sources; disappearance of the registry advisory is not independent source acceptance. The release-assurance owner must review this exact data backport, preserve license distribution obligations, and refresh downstream artifact qualification where applicable. No production node binary, worker or Wasm is reissued here.
