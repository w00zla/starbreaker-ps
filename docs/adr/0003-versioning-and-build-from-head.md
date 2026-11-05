# 0003. Version as 0.3.2-qm.n, binaries built from the committed HEAD

- Status: accepted
- Date: 2026-11-04
- Recorded: 2026-11-05
- Scope: build, versioning

## Context

Several machines (Windows and Linux render nodes, a development PC) must run exactly the same exporter, and exports must
be reproducible from a commit. A binary built from a dirty working tree cannot be identified afterwards. The working tree
on Windows is a CRLF checkout, so file contents differ from the committed blobs.

## Decision

- The version is `0.3.2-qm.n` in the root `Cargo.toml` (`0.3.2-qm.1` for the merge, `-qm.2` with `export-batch`). It is
  bumped for a rebuild-worthy step, not per fix.
- A binary is built from the committed HEAD via `git archive`, never from the working tree. The commit hash identifies the
  binary; the consumer's build script writes it next to the binary as `COMMIT`.
- Windows builds natively or from Linux via mingw. Linux builds use cargo-zigbuild against glibc 2.31; the binary needs
  glibc 2.30 or newer. A native build on a current distribution needed a much newer glibc and would not run on older nodes.
- The fork is built and committed first, then the consumers rebuild their bundles.

## Alternatives

- musl build for Linux (runs anywhere): exports were 58 % slower (4.3 against 2.7 s per armour piece, allocator), and export is the limit.
- Native glibc build on the development distribution: too new a glibc.
- Bumping the version per fix: rejected, the commit hash already identifies the binary.
- Build from the working tree: not reproducible.

## Consequences

Uncommitted fork changes never leak into a binary. Linux builds need zig and cargo-zigbuild. The build scripts live in the
consuming repository, not here; this ADR only fixes the rules they follow.

## References

`12998ce0`, `c0cb40ea`; the consumer's `scripts/starbreaker-build.{sh,ps1}`.
