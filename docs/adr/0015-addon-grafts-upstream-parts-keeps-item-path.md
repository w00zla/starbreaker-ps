# 0015. The add-on takes upstream's self-contained parts and keeps the item path

- Status: accepted
- Date: 2026-11-07
- Scope: Blender add-on, syncs with upstream
- Supersedes in part: [0001](0001-fork-base-fleetyards-plus-upstream.md) (add-on base), [0002](0002-purpose-and-upstream-sync.md) (syncs are merges)

## Context
The fork's add-on is fleetyards' one taken whole (0001); upstream's 30 add-on commits since the fork point were left
out. The maintainer wants the fork close to upstream, so that ships, characters and other things render too, not only
items. A comparison on 2026-11-07 found the two add-ons are real forks of the same features: LayerBlend mixing, decal
host clones, the MeshDecal patch, Illum POM decals and the POM chain (a binary `pom_library.blend`) are built in
incompatible ways; a 3-way mix gives 38 conflict hunks. Neither side is a superset: upstream alone has skin, hair,
screen effects/GFX, real POM, slot pruning; fleetyards and this fork alone have the item path (Illum metals,
HardSurface stencils, glass, MeshDecal finish, paint slot mapping, flat layers, POM decals, layer glow). Upstream's
add-on builds no materials under `blender -b` (its load_post only registers a timer). Upstream's add-on work stopped
in July 2026.

## Decision
- Keep fleetyards' add-on plus the fork's own changes as the base for the item path.
- Take upstream's self-contained parts on top: skin and hair mixins, screen effects and the GFX framework, slot
  pruning, the UIPlane index fallback, frame-range auto-extend, POM bias fixes where they do not depend on upstream's
  POM chain.
- Upstream's POM chain and decal-host system stay out for now; whether to adopt them is a separate decision, made on
  a render comparison of the sample set.
- Syncs: the Rust side keeps being merged (0002); add-on changes are taken selectively.
- Look values tuned by eye move into `runtime/constants.py` (upstream's own rule for tuning knobs), so packshot's
  material profiles can set them before a file opens instead of patching code.
- No pull requests to upstream (maintainer's decision); the fork keeps its own fixes such as the background-mode
  material build.

## Alternatives
- Switch to upstream's add-on and port the item path: 45 of 61 fleetyards and all 10 own add-on commits conflict;
  LayerBlend, Illum and decals would be redesigns, not ports.
- Cherry-pick all 30 upstream add-on commits: the POM/decal chain conflicts heavily and its `.blend` cannot be merged.

## Consequences
- The fork's add-on stays divergent in `builders.py`, `groups.py` and POM; each sync names what it took.
- The sample set (45 entities) is the check for every add-on change taken from upstream.
