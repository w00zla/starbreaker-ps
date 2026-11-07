# 0009. Publish tagged releases for packshot to pin

- Status: accepted; release names per [0010](0010-version-suffix-ps.md) (`0.3.2-ps.n`), process per [0012](0012-own-test-and-release-workflows.md)
- Date: 2026-11-05
- Recorded: 2026-11-05
- Scope: release process

## Context

The README says "no releases, build from source". packshot (sc-packshot) needs the exporter binary and the Blender add-on
of one exact state on every node, for Windows and Linux. Building the fork on each node is slow, needs zig on Linux
(ADR 0003) and makes the binary depend on the node. Every node must run the same state or no run starts.

## Decision

The fork publishes tagged releases named like the version, `0.3.2-qm.n`, each with the Windows binary, the Linux binary
(built per ADR 0003) and the add-on as a zip. packshot pins one release by tag and SHA256 (packshot ADR 0026). A tag marks a
commit of `main`; the binaries are built from that committed HEAD.

## Alternatives

- Keep building from source on every node: not recorded as rejected in detail, but it is what the README still says.
- Pin by commit hash only: not recorded.

## Consequences

No release process exists yet: no workflow, no checksum file format, no cross-build automation. The README line "no
releases" is outdated once the first release is published. Until then packshot uses locally built binaries, identified by
their commit (ADR 0003). The version bump policy (a rebuild-worthy step, not per fix) decides how often a release makes
sense.

## References

ADR 0003; packshot ADR 0026 (in the packshot repository).
