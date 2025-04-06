# ADR 001: Do not use `systemd` socket units for daemon communication

**Date:** 2025-04-08

## Context

Initially, `systemd` socket activation was considered for managing daemon communication via UNIX domain sockets. This
approach delegates socket lifecycle and discovery entirely to `systemd`. However, the `fractory` daemon has specific
requirements:

- Must support both system-wide (`--system`) and per-user (`--user`) execution contexts.
- Must allow manual ad-hoc daemon runs without involvement from `systemd`.
- Must be able to self-terminate after a configurable period of inactivity when run manually.
- Needs a single, clearly defined source of truth for socket location and management.

Using `systemd` socket units alongside self-managed socket handling introduces multiple sources of truth and complicates
socket discovery, potentially leading to ambiguous daemon state and race conditions.

## Decision

I decided to **not use `systemd` socket units**. Instead, the `fractory` daemon will directly manage UNIX domain sockets
internally.

This ADR supersedes any earlier assumptions or experiments involving `fractory.socket` units.

### Rationale

- A single internal management source simplifies socket lifecycle, reduces complexity, and avoids ambiguity.
- Enables consistent socket handling for both automated (`systemd`-initiated) and ad-hoc daemon invocations.
- Allows `fractory` to reliably implement internal logic for auto-termination after idle periods, which would be
  challenging with external socket management.

## Consequences

- `fractory` assumes full responsibility for socket setup, discovery, and teardown.
- Systemd units become simpler, handling only process execution, logging, and restart logic.
- Slightly increased initial complexity in `fractory`’s internal socket management logic is balanced by significantly
  reduced operational complexity.

