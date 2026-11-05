# 0006. Add-on material rules are fitted against the game's inventory tiles

- Status: accepted
- Date: 2026-11-02
- Recorded: 2026-11-05
- Scope: blender_addon (material builders)

## Context

The icons should look like the game's own inventory tiles. The add-on builds the node networks from the exporter's
sidecars, and several default rules produced visibly wrong results (glowing visors, turquoise glass, near-black palette
metals, missing decals, grey organics). Fits were measured in linear channel ratios against tiles seen in the game, not
guessed; each rule applies to a material category, not to one item.

## Decision

- Emission strength is `Glow` x `EmissiveOpacity` for contract materials; Illum keeps its old rule (the Glow rule fitted worse there).
- Slot names with a trailing space match their submaterial.
- Organic colour is layer-1 tint x the layer material's Diffuse x2, with the layer's coloured specular. Ore veins come from the blend map in TexSlot8 (smoothstep 0.6-0.8).
- A palette metal's F0 is the channel's finish specular x the slot's TintColor.
- POM decals (TexSlot1/2/4/8) are laid over via UV2 as colour, specular, roughness and normal; no ray-marched parallax (invisible at icon size).
- GlassPBR: neutral hue node, palette tint only with `%USE_TINT_PALETTE` else Diffuse, `RefractionGloss` 0 is clear.
- Monitor: glows texture x Diffuse, specular 0.35, rendered dithered so refraction sees it.
- Flat-layer glow uses the geometric mean of Emissive and the layer tint. This is fitted to a single item and flagged as such.

## Alternatives

- A rule based on the HAL control mask (R below 0.49) for glow without Emissive: built, then refuted by decoding the shader (the mask drives hue/AO/luminance, not glow); committed in `cc0c3c91` and flagged wrong.
- A `TintMode` rule for metals without palette: no visible effect.
- Per-item overrides: the stance is general rules over special cases.

## Consequences

Open: LayerBlend_V2 Glow per the decoded shader (`Glow` x layer-1 albedo x blend R) is the intended next general rule and
is not built; it would replace the flat-layer and HAL rules. The palette-metal tint rule is fitted to photos with a factor-2
scatter and not proven by shader code.

## References

`2435146f`, `bfb057e9`, `510a72f9`, `cc0c3c91`, `45212517`.
