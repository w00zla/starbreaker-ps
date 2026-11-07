# 0017. Tests on pull requests only, release builds on tags only

- Status: accepted
- Date: 2026-11-08
- Scope: CI (`test-ps.yml`, `release-ps.yml`)
- Supersedes in part: [0012](0012-own-test-and-release-workflows.md) (tests "on pushes to `main`"; the release
  workflow's warm-up build on `main`, added in d71b8207)

## Context
`main` changes only through pull requests, whose test runs come first. After every merge the tests ran again on
`main`, and a merge that changed `Cargo.toml` (every version bump) also ran the release build on `main` to warm its
cache, before the tag built the same once more: three runs of the same code per release.

## Decision
- `test-ps.yml` runs on pull requests and by hand, not on pushes to `main`; a pull request run saves its Rust cache
  for its later pushes, and a new push cancels the stale run.
- `release-ps.yml` runs on release tags only; it saves no cache, since no later tag can read a tag's cache.

## Alternatives
- Keep the runs on `main`: they fill the caches that pull requests and tags read, so those start warm.

## Consequences
- A new pull request and a release start with a cold Rust cache and take longer.
- A pull request should be up to date with `main` before it is merged, so what its run tested is what lands.
