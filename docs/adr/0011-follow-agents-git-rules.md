# 0011. Changes follow AGENTS.md: pull requests, squash merge, no trailers

- Status: superseded in part by [0014](0014-co-authored-by-trailers-allowed.md) (the trailer rule)
- Date: 2026-11-06
- Scope: repository policy

## Context
AGENTS.md (inherited from upstream) says changes land on `main` through pull requests with squash merge, and commits
carry no `Co-Authored-By` or other generated trailers. Until 2026-11-06 the fork's own commits went straight to
`main` and carried a trailer, which contradicted that file.

## Decision
The fork follows AGENTS.md: work on a branch from `main`, open a pull request, squash-merge it; no trailers in
commit messages or pull request bodies. Upstream's rules take priority over local habit, so the fork stays easy to
merge with and contribute back to ([0002](0002-purpose-and-upstream-sync.md)).

## Alternatives
- Keep committing on `main`: simpler for one maintainer, but contradicts the rules every contributor and agent reads.

## Consequences
- Earlier commits on `main` (with trailers) stay as they are; the history is not rewritten.
- Binaries are built from `main` after the merge ([0003](0003-versioning-and-build-from-head.md)).
