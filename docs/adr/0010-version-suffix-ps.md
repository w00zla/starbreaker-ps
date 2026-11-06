# 0010. Version suffix -ps.n

- Status: accepted
- Date: 2026-11-06
- Scope: versioning

## Context
The fork was named starbreaker-qm after its first consumer and versioned `0.3.2-qm.n`
([0003](0003-versioning-and-build-from-head.md)). On 2026-11-06 it was renamed to starbreaker-ps, after packshot, the
renderer it now exists for ([0002](0002-purpose-and-upstream-sync.md)).

## Decision
The version is `0.3.2-ps.n` in the root `Cargo.toml`, continuing the counter: `0.3.2-qm.2` becomes `0.3.2-ps.2`, the
next bump is `0.3.2-ps.3`. Tagged releases ([0009](0009-tagged-releases-for-packshot.md)) carry the same name. The
upstream base version (`0.3.2`) stays in front.

## Alternatives
- Keep `-qm`: the name no longer matches the fork.
- Restart the counter at `-ps.1`: would reuse a number for a different state.

## Consequences
- The rename changes no code; binaries built before it report `0.3.2-qm.2` and are otherwise identical.
- packshot identifies the exporter by commit hash (`COMMIT`), not by version, so nodes are unaffected.
