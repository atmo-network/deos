# Documentation Namespaces

This directory contains independently owned documentation spaces. A namespace identifies who maintains a contract or explanation; it is not an implicit override or inheritance mechanism.

## Shipped Documentation

- [DEOS documentation](./deos/README.md): Framework contracts, cross-system architecture and the DEOS reference composition.

## Ownership and Extension

- Keep upstream DEOS documentation in `docs/deos/` and instance-specific documentation in a separately named namespace with its own `README.md`.
- A downstream namespace may reference DEOS contracts and describe explicit configuration, policy or mechanism differences without copying the upstream documents.
- DEOS documentation must not depend on a downstream namespace. Fork authors do not need to add their namespace to an upstream registry or edit the DEOS index.
- A directory name does not resolve conflicting guarantees. Each instance must state which contracts apply and which claims its changes no longer preserve.
- Package specifications, architecture and embedding guidance remain with their packages; workspace implementation maps remain with their workspaces. Link to those owners instead of relocating or duplicating them here.
- Keep this container focused on placement and navigation. Each namespace owns its own document index, terminology and applicability.

## Related Entrypoints

- [Repository overview](../README.md)
- [Economic simulators](../simulators/README.md)
- [Wiki namespaces](../wiki/index.md)
