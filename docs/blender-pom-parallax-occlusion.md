# Blender POM (Parallax Occlusion Mapping) — Architecture & Runbook

How the fork's add-on renders parallax-occlusion relief for MeshDecal / FPS-weapon
materials, the parameters that drive it, and the failure modes (with fixes) that
have actually bitten us. **Read this before changing anything POM-related** — the
pipeline is subtle and several "obvious" levers are the wrong ones.

The fork keeps its own POM chain (fork ADR 0016): upstream's in-file fixes are
built into the fork's `pom_library.blend`, which also carries the fork's interface
(UV Scale, detail control). Upstream's chain differs: a `Layers` input, a Scale
floor, a deferred Bias pass (see History).

## Source of truth

The whole POM system is a port of the **old StarFab `scorg_tools` addon**. It
is not vendored in this repo; the upstream source is
[scorg-tools/Blender-Tools](https://github.com/scorg-tools/Blender-Tools)
(clone it locally to diff against):

- `scorg_tools/pom.blend` — the authored node groups
  (`POM_Vector`, `POM_10x_Layer_Steps`, `POM_parallax`, `POM_disp`,
  `tangent_space`) and a working `scorg_pom` material.
- `scorg_tools/blender_utils.py` — `replace_pom_materials`
  (copies `scorg_pom`, swaps textures) and `find_and_set_displacement_image`
  (sets the height image **and** the `POM_Vector` `Bias` from the height map's
  top-left pixel, or the material's `HeightBias` property).

When POM behaviour looks wrong, diff against this reference. The displace
*modifiers* in that addon are only a z-fighting offset — **not** POM; ignore them.

## Architecture

- Bundled node library: `blender_addon/starbreaker_addon/resources/pom_library.blend`,
  with its text image `pom_library.blend.json` next to it (a JSON dump of the node
  groups; `tests/test_blend_text_images.py` checks that both agree,
  `tests/test_pom_library.py` checks the interface and the fixes below).
  Chain: `POM_Vector` → `POM_10x_Layer_Steps` → `POM_parallax` → `POM_disp`
  (+ shared `tangent_space`). `POM_disp` holds the height `ShaderNodeTexImage` +
  a `MapRange` (height 0..`Bias` → depth 1..0); the chain is an unrolled
  ray-march (4 × 10 steps).
- The library is edited in Blender, by hand or by a headless script, never
  reshaped at load time. `scripts/pom_library_fork_interface.py` is the one-time
  edit that put the fork's interface and upstream's fixes into the fork-point
  file; rewrite the text image after every change
  (`scripts/blend_text_image.py`). Comparing with upstream's file is a diff of
  the two text images.
- Because a Blender node group cannot take an image datablock as an *input*,
  `POM_disp` (and everything that references it up the chain) is **copied
  per-height-image**; samplerless helpers (`tangent_space`, `Clamp`) are collapsed
  to one shared canonical copy. See `_ensure_runtime_parallax_group`
  (`runtime/importer/groups.py`), which also binds the height image and the
  wrap mode to every sampler of the copy.
- The per-image root is named `StarBreaker POM [<height image>] [REPEAT]`.
- Code map (all in `runtime/importer/` unless noted):
  - `builders._wire_runtime_parallax` — instantiates the POM group on a material,
    sets `Scale`/`Bias`/`Non-planar`/`UV Scale X/Y`, wires the offset Vector into
    the coverage/normal/colour samplers.
  - `builders._resolve_parallax_bias` / `_height_image_background_bias` — resolve
    the reference plane (see Bias below).
  - `groups.apply_pom_detail_mode` / `_set_pom_detail_on_tree` — set the detail
    values inside every root (the scene's POM Detail setting, `ui.py`).
  - `runtime/constants.py`: `POM_SCALE_MULTIPLIER`, `POM_SCALE_MAX`,
    `POM_DETAIL_*`, `pom_detail_settings`.
  - `types._bake_bitangent_sign_attribute` + `orchestration._bake_pom_mesh_bitangent_signs`
    — the mirrored-UV correction (see Vertical inversion below); the exporter's
    side is `crates/starbreaker-3d/src/pipeline/blend_assembly/bitangent_signs.rs`.
- Small decals can be culled at lower LODs; the FPS weapons checked on
  2026-11-07 carry their POM decals at LOD1 (packshot's export) too.

## Parameters

`POM_Vector` inputs:

| Input | Meaning | How it's set |
|---|---|---|
| `Scale` (float) | parallax depth | authored `PomDisplacement` × `POM_SCALE_MULTIPLIER` (30), capped at `POM_SCALE_MAX` (3.0). No floor: behr's PomDisplacement ≈ 0.0015–0.003 gives Scale ≈ 0.04–0.09, almost flat. |
| `Bias` (float) | **reference plane** (height that = flush surface) | an authored `HeightBias` > 0, else the height-map **background** = top-left-pixel luminance (behr atlas 0.694, apar 0.698), else 0.5. |
| `Non-planar` (bool) | curved-surface mode | `True` (reference value). |
| `UV Scale X/Y` (float, default 1) | the layer's UV tiling | the start UV **and** the march delta are multiplied by it, so a tiled material marches in its tiled UV space. Callers must not also tile the target samplers. |

Inside `POM_Vector` (no inputs, set by `_set_pom_detail_on_tree`):

| Node | Meaning |
|---|---|
| `StarBreaker Detail Layers` (Value) | layer count: feeds the layer division (`Math.003`), the delta division (`Vector Math.002`) and `num layers` of the first step group. LOW 20, MEDIUM 50 (default, also the file's value), HIGH 100. |
| `StarBreaker Detail Scale Multiplier` (Value) | `layers² / 40²`, multiplied into Scale (`StarBreaker POM Detail Scale`) before the layer division: the per-step delta is `dir × Scale / layers²`, so the multiplier keeps the march distance of the 40-layer reference while the layer count changes. |

The functional Bias is what lets `Scale` grow without the mid-level swimming.

## Failure modes → fixes (the runbook)

1. **Flat / "only bump", no real depth.** Either `Scale` is ~0 (tiny
   `PomDisplacement`; the fork has no floor, upstream clamps to ≥ 1.5) **or** the
   ray-march is collapsed (see #3). Check the live `POM_Vector.Scale` value first.
2. **"Too deep" / looks like a hole with a plane below (not flush).** This is a
   **Bias / mid-level** problem, *not* Scale. `Bias` must equal the height-map
   background (~0.69 for a gray atlas), not 0.5. Increasing Scale will not fix
   it and is the wrong lever.
3. **Height sampler must be `REPEAT`, never `CLIP`.** The march walks the UV
   across the height field; `CLIP` returns transparent black outside 0–1 and
   collapses the march — the reference plane goes **inert** and the parallax
   degenerates to a constant UV shift (a "SEMI" decal renders offset as
   "EMI-F"). The library's sampler repeats and `_parallax_height_sampler_extension`
   always returns `REPEAT`.
4. **Bias appears inert (changing it does nothing).** First suspect #3 (CLIP).
   Then diff the text image against upstream's (`git show
   upstream/main:blender_addon/starbreaker_addon/resources/pom_library.blend`,
   write its image with `blend_text_image.py`): apart from the fork's interface
   (UV Scale, detail nodes, no `Layers` input) the groups must agree. Verify with
   a **pixel-diff** of two Bias values, using a `Scale` change as a positive
   control.
5. **Atlas height maps (FPS weapons).** The height map is an *atlas* of many
   decals; each face uses a sub-cell. At grazing angles the march can leave the
   cell and sample neighbours. The background-referenced Bias keeps the offset
   small; keep `Scale` modest. True per-cell clamping is unimplemented (future).
6. **Parallax shifts the wrong way vertically (inverted) on some faces.** Models
   mirror one side onto the other → inverted UVs (~25% of faces). On a mirrored
   face the bitangent flips. `tangent_space` multiplies its bitangent by the
   per-corner `starbreaker_bitangent_sign` (±1 MikkTSpace sign) and takes the
   tangent of the render UV map (`Tangent.uv_map` empty). Both halves must be
   present: the `Attribute`+`Multiply` in `tangent_space` **and** the
   attribute on the *rendered* mesh. The exporter writes it into every Blend-format
   mesh with UVs (MikkTSpace's sign computed from the UVs; see the export
   contract), so the meshes a `scene.blend` links carry it (packshot's path:
   linked meshes are read-only, the load-post refresh cannot bake). The add-on
   bakes Blender's MikkTSpace sign when it imports a package (template loader,
   which overwrites the exported one, then `_bake_pom_mesh_bitangent_signs` for
   meshes still without it); both agree on 99.89–100 % of the corners. Without
   the attribute the `Attribute` node reads 0 and the vertical parallax is zero
   on every face: on the Scourge that was ~5 % of the pixels (oblique view).
   Check the attribute against `mesh.calc_tangents()` / `loop.bitangent_sign`
   on a local copy of the mesh.

   **Note:** the exporter takes the sign from the UVs, not from the game's
   tangent frame, because the add-on pairs it with Blender's MikkTSpace tangent
   from the UVs. The game's handedness matched on 98.7–99.9 % of the corners of
   weapons and helmets, but on worn armour cores 5–10 % of the corners carry a game
   sign against their UV orientation (cds heavy core 90.2 %, cds superheavy suit
   94.6 %, ccc shrike 94.7 % agreement; mostly whole faces, not seams; mirrored
   transforms and flipped winding ruled out; cause unknown). The game data is the
   alternative if that ever matters (e.g. normal maps baked against the game's
   frame); `bitangent_signs.rs` has the details.
   This is a **motion** artifact too — mostly invisible in static renders;
   verify by orbiting.

## Reading the height image

`_height_image_background_bias` reads `image.pixels[0:4]` of the material's
height image at build time (`pixels[:]` would copy the whole buffer, ~500 MB for a
2048² image). Upstream found that read unreliable during a full interactive
import (buffers evicted under load, Bias falling back to 0.5) and added a
temporary reload plus a deferred post-import pass. In the fork's load-post
refresh under `blender -b` the direct read works (checked on eight FPS weapons:
every POM node got 0.694 / 0.698); if a Bias of exactly 0.5 shows up without an
authored `HeightBias`, suspect this first.

## Diagnostic methodology

POM bugs are spatial/motion bugs; **use data, not eyeballing renders**:

- Dump the **Bias/Scale/detail distribution** across every `StarBreaker POM […]`
  node (which value each material got).
- **Pixel-diff** two parameter values with a known-good positive control (a
  `Scale` change visibly differs; if `Bias` doesn't, it's inert).
- Check **UV winding/handedness** (signed UV area) and the
  `starbreaker_bitangent_sign` attribute via `bmesh` (note: in Edit mode the
  non-bmesh `mesh.uv_layers`/`mesh.attributes` arrays read empty).
- Motion artifacts (parallax direction, atlas-cell bleed) **cannot** be seen in
  static renders — implement from the diagnosis and verify by orbiting.

## History

- Fork point: the library of upstream's `8d34b8f1`; fleetyards and the fork
  reshaped `POM_Vector` at load time (`groups.py` removed `Layers`, added the
  detail nodes and UV Scale).
- Upstream (2026-06-22, not merged as such): `d2b93f0e` (Bias from the
  background, REPEAT sampler; it also dropped the UV-Scale/detail rewrite and went
  back to a `Layers` input with a Scale floor of 1.5), `a6c9d44d` (mirrored-UV
  bitangent sign), `fdf3569d` (host-variant Bias fallback, deferred Bias pass),
  `30f3f60c` (read only `pixels[0:4]`).
- Fork, 2026-11-07 (ADR 0016): the fork's interface moved into the library;
  taken from upstream: the REPEAT sampler without an embedded image, the
  render-UV-map tangent, the background Bias, the `pixels[0:4]` read, the
  post-import bitangent bake. Not taken: the `Layers` input, the Scale floor,
  `fdf3569d` (the fork's `__host_*` decal clones resolve Bias from their
  submaterial like every other POM material, no stashed 0.5; and the direct read
  works in its path).
