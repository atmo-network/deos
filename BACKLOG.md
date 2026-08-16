# DEOS Backlog

## 0.7.18 — Semantic Compression and Contract Truth

### Release Gates

- [ ] `Validation / Pull Request`: Obtain one successful same-repository pull-request `validation-gate` artifact for the final one-commit release tree.
- [ ] `Validation / Main Gate`: Require exactly `validation-gate` on `main`, then verify that the merged tree reuses trusted pull-request evidence and that an authority-change or unavailable-evidence case falls back to canonical `--fresh fast`.
- [ ] `Validation / Tag-Bound Full`: Pass canonical fresh `full` from the exact immutable release tag and publish its candidate handoff.
- [ ] `Validation / Tag-Bound Network`: Pass finalized multi-collator smoke, failover/restart, and composed Router/Oracle/Burn Actor evidence against the exact candidate handoff.
- [ ] `Release / Evidence Bundle`: Publish and independently reverify the exact 13-file Wasm, metadata, descriptor, semantic-evidence, validation, network, SBOM, manifest, and checksum bundle.
- [ ] `Release / Provenance`: Publish verifiable provenance attestations for exactly every final release asset.
- [ ] `Closure / Backlog Truth`: Close 0.7.18 only when pull-request, main, tag, network, bundle, provenance, GitHub Release, and canonical project truth agree on the released tree.
