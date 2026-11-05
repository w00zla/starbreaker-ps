# 0008. The material build stays in the Python add-on

- Status: accepted
- Date: 2026-11-05
- Recorded: 2026-11-05
- Scope: blender_addon, starbreaker-blend

## Context

In render clusters the add-on's material build in Python is the main CPU cost per item. The exporter writes meshes with named
material slots, PNG textures and material JSON sidecars (`scene.json`, palettes, paints, liveries); the add-on builds the node
networks from them. The `.blend` writer in `crates/starbreaker-blend` writes empty material node trees (`node_tree_ptr = 0`)
and targets Blender 5.1; renders run on a newer Blender, which upgrades the file on load. The same add-on is also used
interactively.

## Decision

The material build stays in the Python add-on. Speed work is limited to changes that keep the rendered pixels identical:

- on open, only runtime shared node groups already present in the file are refreshed; missing groups are created on first use (two unused builders were removed);
- the path index is built with `os.scandir` in `rglob`'s order (same collision winners, 10-35x faster);
- polygon counts per material come from `foreach_get` and `np.unique`;
- a shared path index per export root exists but is opt-in (`manifest.share_path_indexes(True)`, default off);
- candidate paths are tried as written before the case-insensitive index is built.

Measured: CPU per item 127 -> 74 ms, load phase with 6 parallel Blenders 0.40 -> 0.22 s per item; 49 of 51 items
pixel-identical, the rest as noisy as two runs of the old code. The maintainer decided that the flexibility of the Python
approach is worth more than further speed.

## Alternatives

A Rust node-tree writer in `starbreaker-blend`, a prebuilt baked node-group library, a material cache, glTF or USD import,
and an external renderer (wgpu, Filament, Godot, three.js) were looked at and not taken. They are to be proposed again only
if speed becomes a problem.

## Consequences

The add-on keeps being edited in Python and tested with Blender's own Python. A group-check cache was not added (the
profile showed it was a profiler artefact).

## References

`38e26bc3`, `655d5f68`.
