# Tools

Independent Cargo workspaces that support validation but are not part of the runtime. The runtime workspace excludes them, and each owns its own `Cargo.toml`, `Cargo.lock` and patch table.

| Tool | Purpose | Local patches |
| --- | --- | --- |
| [`pvf-validation`](pvf-validation/README.md) | Offline acceptance of exported PVF fixtures through the SDK `ValidationHost` and real, version-checked prepare and execute workers. It is not a node, relay client, benchmark or production certificate. | Shared packages from [`../vendor`](../vendor/README.md), plus `tracing-subscriber` 0.3.19 from its own `vendor/` |

The PVF host pins an exact SDK tag (`polkadot-sdk` revision `9448fe4f…`). Its dependency graph includes client-side networking and database crates that the runtime workspace never builds. That is why most security backports in `../vendor` serve only this tool.
