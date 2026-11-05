# Architecture decision records

One decision per file. Accepted records are not rewritten: a changed decision gets a new record that supersedes the old
one, and only the old record's status line and this index change. Records 0001-0008 were written afterwards, on
2026-11-05, from commit messages and working notes; their dates are the dates of the decisions.

| No. | Title | Status | Date |
|---|---|---|---|
| [0001](0001-fork-base-fleetyards-plus-upstream.md) | Base the fork on the fleetyards fork and merge upstream into it | accepted | 2026-11-01 |
| [0002](0002-purpose-and-upstream-sync.md) | The fork serves packshot and the catalog extract; syncs are merges checked by sample renders | accepted | 2026-11-01 |
| [0003](0003-versioning-and-build-from-head.md) | Version as 0.3.2-qm.n, binaries built from the committed HEAD | accepted | 2026-11-04 |
| [0004](0004-export-batch-and-stdin.md) | entity export-batch, then a long-lived --stdin mode | accepted | 2026-11-01 |
| [0005](0005-material-resolution-by-generic-rules.md) | Resolve materials in the exporter by generic, data-driven rules | accepted | 2026-11-01 |
| [0006](0006-addon-material-rules-from-game-tiles.md) | Add-on material rules are fitted against the game's inventory tiles | accepted | 2026-11-02 |
| [0007](0007-dds-read-only-needed-mips.md) | Read only the DDS mip siblings the requested mip needs | accepted | 2026-11-04 |
| [0008](0008-addon-stays-python.md) | The material build stays in the Python add-on | accepted | 2026-11-05 |
| [0009](0009-tagged-releases-for-packshot.md) | Publish tagged releases for packshot to pin | accepted | 2026-11-05 |
