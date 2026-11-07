# 0012. Own test and release workflows; upstream's stay in the tree, disabled

- Status: accepted; superseded in part by [0017](0017-tests-on-pull-requests-releases-on-tags-only.md) (no runs on pushes to main)
- Date: 2026-11-07
- Scope: CI, release process

## Context
[0009](0009-tagged-releases-for-packshot.md) decided on tagged releases but left the process open: no workflow, no
checksum file. The workflows inherited from upstream did not fit: `release.yml` reacts to `v*` tags and builds the Tauri
app with signing secrets the fork does not have, and `ci.yml` builds the app too (webkit on Linux). None of them had ever
run in the fork. Editing upstream's files would make every upstream merge that touches them conflict
([0002](0002-purpose-and-upstream-sync.md)).

## Decision
- **Own files, upstream's untouched:** `.github/workflows/test-ps.yml` and `release-ps.yml`. Upstream's `ci.yml`,
  `starbreaker-ui-tests.yml` and `deploy-website.yml` stay as they are and are disabled in the repository's Actions
  settings (`gh workflow disable`).
- **Tests** (`test-ps.yml`, on pushes to `main`, pull requests and by hand): `cargo test --workspace --exclude
  starbreaker-app --locked` on Linux and Windows, and the add-on tests (`python -m unittest`, Python 3.13 as in
  Blender 5.2, fakes instead of `bpy`) on Linux.
- **Releases** (`release-ps.yml`): a tag named exactly like the workspace version, without `v` (`0.3.2-ps.3`), on a
  commit of `main`. The workflow refuses other tags. Assets: `starbreaker-<tag>-windows-x86_64.zip` (native build),
  `starbreaker-<tag>-linux-x86_64.tar.gz` (cargo-zigbuild against glibc 2.31, as in
  [0003](0003-versioning-and-build-from-head.md)), `starbreaker_addon-<tag>.zip` (the add-on folder of the tagged
  commit via `git archive`) and `SHA256SUMS`. Published directly, not as a draft; no app.
- The desktop app (`app/`) is not built or tested here; the README says it may be broken or lack the fork's changes.

## Alternatives
- Replace upstream's `release.yml` and `ci.yml`: conflicts on every upstream merge that touches them.
- Releases as drafts, published by hand: an extra step with nothing to check that the workflow does not check already.
- Build the app as well: slow, needs signing secrets, and nothing here uses it.
- Upstream's `ci.yml` as the test workflow: builds the app and was never run in the fork.

## Consequences
- packshot can pin a release by tag and SHA256 ([packshot ADR 0026](https://github.com/w00zla/sc-packshot/blob/main/docs/adr/0026-starbreaker-by-release-pin.md)).
- A release is a version bump plus a tag on `main` after the pull request is merged.
- Upstream's `release.yml` stays active but only reacts to `v*` tags, which the fork does not push.
