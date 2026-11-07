# Architecture decision records

One decision per file. Accepted records are not rewritten: a changed decision gets a new record that supersedes the old
one, and only the old record's status line and this index change. Records 0001-0008 were written afterwards, on
2026-11-05, from commit messages and working notes; their dates are the dates of the decisions.

| No. | Title | Status | Date |
|---|---|---|---|
| [0001](0001-fork-base-fleetyards-plus-upstream.md) | Base the fork on the fleetyards fork and merge upstream into it | superseded in part by [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) | 2026-11-01 |
| [0002](0002-purpose-and-upstream-sync.md) | The fork serves packshot and the catalog extract; syncs are merges checked by sample renders | superseded in part by [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) | 2026-11-01 |
| [0003](0003-versioning-and-build-from-head.md) | Version as 0.3.2-qm.n, binaries built from the committed HEAD | superseded in part by [0010](0010-version-suffix-ps.md), [0012](0012-own-test-and-release-workflows.md) | 2026-11-04 |
| [0004](0004-export-batch-and-stdin.md) | entity export-batch, then a long-lived --stdin mode | superseded in part by [0013](0013-export-batch-catches-panics-per-job.md) | 2026-11-01 |
| [0005](0005-material-resolution-by-generic-rules.md) | Resolve materials in the exporter by generic, data-driven rules | accepted | 2026-11-01 |
| [0006](0006-addon-material-rules-from-game-tiles.md) | Add-on material rules are fitted against the game's inventory tiles | accepted | 2026-11-02 |
| [0007](0007-dds-read-only-needed-mips.md) | Read only the DDS mip siblings the requested mip needs | accepted | 2026-11-04 |
| [0008](0008-addon-stays-python.md) | The material build stays in the Python add-on | accepted | 2026-11-05 |
| [0009](0009-tagged-releases-for-packshot.md) | Publish tagged releases for packshot to pin | accepted; names per [0010](0010-version-suffix-ps.md), process per [0012](0012-own-test-and-release-workflows.md) | 2026-11-05 |
| [0010](0010-version-suffix-ps.md) | Version suffix -ps.n | accepted | 2026-11-06 |
| [0011](0011-follow-agents-git-rules.md) | Changes follow AGENTS.md: pull requests, squash merge, no trailers | superseded in part by [0014](0014-co-authored-by-trailers-allowed.md) | 2026-11-06 |
| [0012](0012-own-test-and-release-workflows.md) | Own test and release workflows; upstream's stay in the tree, disabled | accepted | 2026-11-07 |
| [0013](0013-export-batch-catches-panics-per-job.md) | entity export-batch catches a panic per job | accepted | 2026-11-06 |
| [0014](0014-co-authored-by-trailers-allowed.md) | Co-authored-by trailers are allowed | accepted | 2026-11-07 |
| [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) | The add-on takes upstream's self-contained parts and keeps the item path | superseded in part by [0016](0016-own-blend-libraries-with-text-image.md) | 2026-11-07 |
| [0016](0016-own-blend-libraries-with-text-image.md) | The add-on owns its .blend libraries, each with a text image and a test | accepted | 2026-11-07 |
