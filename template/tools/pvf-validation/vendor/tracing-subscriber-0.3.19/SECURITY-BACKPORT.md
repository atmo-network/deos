# tracing-subscriber 0.3.19: terminal-control sanitization backport

## Source identity and necessity

This is a locally maintained 0.3.19 compatibility backport for RUSTSEC-2025-0055, not an upstream-patched 0.3.19 release. The upstream patched range remains >=0.3.20. The immutable polkadot-sdk revision 9448fe4f252936d241b8d2663506813b884f94f3 requires exactly 0.3.19; a compatible lock update cannot select 0.3.20 without changing that Git source. The isolated host selects this local package through its crates.io patch table.

The baseline is the complete published tracing-subscriber 0.3.19 archive, SHA256 e8189decb5ac0fa7bc8b96b7cb9b2701d60d48805aca84a238004d665fcc4008. Every archived file, including tests, benches, LICENSE and archive VCS identity, is retained. The normalized manifest adds only the explicit local regression-test target; dependency and feature declarations and Cargo.toml.orig remain unchanged. The Cargo extraction marker is not an archive file and is not shipped. License notices are unchanged.

The corrective source is the published 0.3.20 archive, SHA256 2054a14f5307d601f88daf0553e1cbf472acc4f2c51afab632431cdcd72124d5, VCS commit 4c52ca5266a3920fc5dfeebda2accf15ee7fb278, directory tracing-subscriber. This backport does not replace 0.3.19 with relabeled 0.3.20 or change the SDK exact requirement.

## Narrow adaptation

- Add 0.3.20's src/fmt/format/escape.rs, removing trailing whitespace only. The streaming Debug/Display wrapper escapes ESC, BEL, BS, FF, DEL and C1 controls without intermediate allocations.
- In src/fmt/format/mod.rs, import that wrapper and apply the upstream message, error Display and error-source-list corrections. Preserve the existing log-metadata spacing behavior and unrelated documentation instead of importing non-security changes from 0.3.20.
- In src/fmt/format/pretty.rs, apply the three corresponding upstream message/error corrections. Default, compact and pretty formatting remain available. No dependency, default feature, public API, supported target or MSRV declaration changes.
- The explicit tests/security_backport.rs integration target covers control characters and error-source chains through all three formatters, preserving ordinary text. They run with the package's existing dev dependencies and default features.

## Limits

This corrects the reviewed untrusted message/error formatting mechanism. Trusted field names, metadata, targets, thread names, custom FormatFields implementations and caller-written raw output are distinct surfaces; this is not a universal terminal-sanitization promise. Existing arbitrary caller Wasm, worker version verification, secure defaults and execution/preparation timeouts are not restricted or changed by this package.

Source-aware audits must retain the 0.3.19 finding and bind the complete local source, advisory, immediate consumers, features and effective compiler/configuration. Source tests alone do not approve a host executable, worker, other package role or release. Independent source review and the owning host's subsequent build/acceptance remain separate gates.
