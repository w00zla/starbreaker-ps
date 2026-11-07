# 0002. The fork serves packshot and the catalog extract; syncs are merges checked by sample renders

- Status: superseded in part by [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) (add-on changes are taken selectively, not merged)
- Date: 2026-11-01
- Recorded: 2026-11-05
- Scope: repository policy

## Context

This fork is not a general replacement for upstream. It exists for two consumers: packshot (sc-packshot), a parallel
Blender renderer of inventory-style item icons, and the catalog extract of the consuming org tool sc-quartermaster (a
private tool), which uses the same binary. Both depend on the item and material path (ADR 0001), and a change in
exporter or add-on moves pixels of thousands of icons.

## Decision

The fork's focus is the decomposed item export and how its materials come out in Blender; ship, character and audio work
is inherited, not developed here. The README states this, names the changes and says that the fork is not affiliated with
either upstream project and that problems with it belong here. A sync with upstream or fleetyards is a merge (the remotes
exist for that). Before it counts as done, a fixed sample of 45 entities (weapons with magazine, armour, paint variants,
ship parts, minerals) is rendered and compared against the current renders; the first merge was checked this way (deltas
mostly below 2 on the 0-255 scale, eight items between 2.5 and 5.2, all explained). Fixes that icons need are made in the
fork, not as workarounds in the renderer; pose, light rig, fit and camera stay in the renderer.

## Alternatives

- Contribute everything upstream and drop the fork: not recorded as discussed. The item path conflicts with upstream behaviour in several places (ADR 0001).
- Rebase onto upstream instead of merging: not recorded.

## Consequences

Every sync costs a sample render and a review of the pixel deltas. The sample list lives with the consumer, not in this
repository. A binary of the fork is only comparable to icons rendered with the same binary and add-on (ADR 0003).

## References

`9aa7ce15`, `c80113c9`; ADR 0001, 0003.
