# Native DNS question allocation backport proposal

## Source identity

- Base: crates.io hickory-proto 0.26.2, archive SHA256 7e2da0694c15b44c6f68a6b05e0233617008c54080e31d6eb848d858a9c5b38d. The complete upstream package is retained, with its MIT/Apache licensing and notices.
- RUSTSEC-2026-0491 primary fix 280c0e5c4e3002d73ba0762835ef82272d1d33a6 caps the helper question and record decoders at eight initial entries. The actual 0.26.2 and 0.26.3 complete-message question decoder independently still reserves the peer-supplied count.

## Delta

- In src/op/message.rs, change the single remaining Vec::with_capacity(count) in Message::read to Vec::with_capacity(Ord::min(count, 8)), matching the upstream helper. Preserve the loop, error conversion, decoded questions and all public APIs. Growth after successfully decoded questions remains permitted.
- This is a local modified source, not a claim that the upstream 0.26.2 release fully fixes question amplification. No dependency, feature, cryptography or compiler change is introduced.

## Falsifier

- The owning standalone probes/src/lib.rs installs a thread-local allocation meter and tests all four advertised header counts8/9/65535 in Message::from_vec against an otherwise empty packet. The largest initial allocation is constant after the patch. The identical control against registry0.26.2 fails for section4 (question count8 versus9 allocates704 versus792 bytes).
- The same probe tests malformed SVCB/HTTPS values, long text tokens, ring-only TLS parent APIs/invalid CRL parsing and actual in-memory HTTP/2 empty-DATA behavior. Its separate h2 0.4.5 control fails the empty-DATA regression. This does not qualify a complete native executable.
