# 0005. Resolve materials in the exporter by generic, data-driven rules

- Status: accepted
- Date: 2026-11-01
- Recorded: 2026-11-05
- Scope: exporter crates (starbreaker-3d, decomposed export)

## Context

Icon renders showed white, empty or wrongly coloured items. Causes were found in the data, not in single assets: 70 items
name their material only in the record's `Material` node, variants sharing one `Data/` overwrote each other's meshes (27
armour pieces rendered empty), worn armour took its node table from the crate geometry (615 records), entities without a
palette stayed white, paint-variant sidecars lost appended part materials, and LayerBlend layers without a file were dropped
(335 of 8426 LayerBlend_V2 armour materials).

## Decision

Rules are generic and read from the game data; no per-asset workarounds.

- Material: the geometry resource's material, else the Material node (`SMaterialNodeParams`, variants by tags). The node
  wins when both differ; only two legacy records differ.
- Item mesh `.blend` files carry the material in the file name when it differs from the geometry's. Character packages keep
  plain mesh names.
- Worn and paint-variant exports take the chosen SubGeometry's mesh and its node table (`selected_subgeometry`).
- Palette fallback: entity palette, else the material's `DefaultPalette`, else `TintPalettes/default`.
- A paint-variant sidecar keeps the part materials appended to the root.
- The second UV set of `%DECALS` LayerBlend materials is decoded from the vertex colour (two 12-bit fields), only where
  the material's LayerBlend decals read it.
- LayerBlend layers without a path are kept as `flat_layers` in the sidecar; the existing layer list stays unchanged.

## Alternatives

- Geometry material first: rejected, the node looks right and the geometry looks like a leftover.
- Choosing a "Default" tag variant by name for ship-turret materials: a name heuristic, not done.
- One export directory per entity instead of unique mesh names: no shared `Data/`, no reuse.
- Decoding the second UV in the add-on: Blender linearises byte colours.

## Consequences

Whether the game falls back to `TintPalettes/default` exactly like this is unproven (one case). The decal UV offset is
empirical.

## References

`182d229f`, `559cb793`, `23679ae6`, `45212517`, `c337b157`, `06d15888`, `ddf92d00`, `c0cb40ea`; `docs/decomposed-export-contract.md`.
