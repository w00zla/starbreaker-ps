# 0001. Base the fork on the fleetyards fork and merge upstream into it

- Status: superseded in part by [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) (the add-on takes upstream's self-contained parts)
- Date: 2026-11-01
- Recorded: 2026-11-05
- Scope: whole repository (exporter crates, blender_addon, licence)

## Context

Item icons (weapons with magazines, armour, paint variants, ores) need the item and material path of the exporter, not the
ship path. Two lines existed: the release 0.3.2 and the fleetyards fork (117 commits of item and material work, 61 of them
in the add-on) on one side, upstream `main` (799 commits newer, skinning, characters, fixes) on the other. A 45-entity
sample (weapons with magazine, armour, paint variants, ship parts, minerals) was rendered with both and compared pixel by
pixel. Upstream `main` rendered armour as loot crates (a worn model only via `--geometry-tag Male`, with T-posed arms and
mostly white palettes), lost paint variants and drew visors flat. The release 0.3.2 misplaced weapon magazines and its
add-on did not stack LayerBlend layers.

## Decision

The base is the fleetyards fork. Upstream was merged into it once (merge `9aa7ce15`, version `0.3.2-qm.1` in `12998ce0`).
Where both sides solved the same problem the item path keeps the fleetyards behaviour: CDF weapon parts, `--worn` and
`--variant-tags` first, per-owner animation clip naming. From upstream come skinning (`mesh_rig::Skin` as the only
representation), characters, `--geometry-tag` and its fixes. The Blender add-on is the fleetyards add-on plus own fixes;
upstream's add-on additions (skin, hair, screens, control-only POM decals) are not taken, because both sides had rebuilt
the decal system across several modules and a mix broke every material build in the first test. The licence stays MIT with
three copyright lines (upstream author, fleetyards, this fork). The remotes are `fleetyards`, `upstream` and `origin`.

## Alternatives

- Release 0.3.2: magazines misplaced, no LayerBlend stacking in the add-on.
- Plain fleetyards fork: loses upstream's decal and material fixes (the Sion compensator texture fix arrived only via the merge).
- Plain upstream `main`: armour as crates, lost paint variants, flat visors.
- Upstream as base with the fleetyards item commits cherry-picked (about 50-60): nearly every pick conflicts and dependencies between commits get lost.
- Mixing the two add-ons: failed on all 45 sample items.

## Consequences

Upstream add-on features (skin, hair, screens) are missing here; decals are handled by this fork's own add-on work
(ADR 0005, 0006). Later syncs are merges (ADR 0002). The first probe merge showed 28 conflict files; the resolution is
recorded in the merge commit message.

## References

`9aa7ce15`, `12998ce0`, `c80113c9` (README and licence lines).
