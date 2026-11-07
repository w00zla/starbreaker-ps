"""Top-level constants used across the StarBreaker runtime.

These names were formerly module-level in ``runtime.py``. They are imported
back into :mod:`._legacy` (and eventually into the split modules) so any
``from .runtime import PROP_*`` keeps working.
"""

from __future__ import annotations

import math

from mathutils import Matrix, Quaternion


PROP_PACKAGE_ROOT = "starbreaker_package_root"
PROP_SCENE_PATH = "starbreaker_scene_path"
PROP_EXPORT_ROOT = "starbreaker_export_root"
PROP_PACKAGE_NAME = "starbreaker_package_name"
PROP_ENTITY_NAME = "starbreaker_entity_name"
PROP_DECAL_OFFSET_EXTERNAL = "starbreaker_decal_offset_external"
PROP_DECAL_OFFSET_INTERNAL = "starbreaker_decal_offset_internal"
PROP_INSTANCE_JSON = "starbreaker_instance_json"
PROP_MESH_ASSET = "starbreaker_mesh_asset"
PROP_MATERIAL_SIDECAR = "starbreaker_material_sidecar"
PROP_PALETTE_ID = "starbreaker_palette_id"
PROP_PALETTE_SCOPE = "starbreaker_palette_scope"
PROP_PALETTE_SCOPE_MAP = "starbreaker_palette_scope_map"
PROP_SHADER_FAMILY = "starbreaker_shader_family"
PROP_TEMPLATE_KEY = "starbreaker_template_key"
PROP_SUBMATERIAL_JSON = "starbreaker_submaterial_json"
PROP_MATERIAL_IDENTITY = "starbreaker_material_identity"
PROP_IMPORTED_SLOT_MAP = "starbreaker_imported_slot_map"
PROP_TEMPLATE_PATH = "starbreaker_template_path"
# Records the active paint variant's exterior material sidecar on the package root.
# Set when a paint variant with a different material file is applied; used by
# _effective_exterior_material_sidecars() so that subsequent palette changes still
# reach the newly-built materials.
PROP_PAINT_VARIANT_SIDECAR = "starbreaker_paint_variant_sidecar"
PROP_SOURCE_NODE_NAME = "starbreaker_source_node_name"
PROP_MISSING_ASSET = "starbreaker_missing_asset"
PROP_SURFACE_SHADER_MODE = "starbreaker_surface_shader_mode"
PROP_DECAL_HOST_CHANNEL = "starbreaker_decal_host_channel"
PROP_DECAL_HOST_RGB = "starbreaker_decal_host_rgb"
# True when the source submaterial's StringGenMask decoded to
# ``has_parallax_occlusion_mapping``. Used by the MeshDecal host-tint
# rebinder to keep host tint on POM decals only (phases 10+11).
PROP_HAS_POM = "starbreaker_has_pom"
# Phase 28: per-light state switching. Lights imported from Star Citizen
# carry the full set of authored `<defaultState|auxiliaryState|emergencyState
# |cinematicState>` snapshots. The runtime switcher stores the JSON-encoded
# map on the Light datablock and the currently-applied state name on the
# same datablock so we can restore values when the user toggles states.
PROP_LIGHT_STATES_JSON = "starbreaker_light_states"
PROP_LIGHT_ACTIVE_STATE = "starbreaker_light_active_state"
PROP_LIGHT_SEMANTIC_KIND = "starbreaker_light_semantic_kind"
# True when the entity's authored default state is "off" (e.g. an
# `SCItemDisplayScreen` whose `defaultView` resolved to a view with no
# concrete canvas content). The exporter still renders a "switched on" image
# so the texture is non-blank, but downstream tools can use this flag to gate
# initial visibility of the UI surface.
PROP_UI_DEFAULT_STATE_IS_OFF = "starbreaker_ui_default_state_is_off"
PROP_ENGINE_GLOW_CONTROL_JSON = "starbreaker_engine_glow_control"
PROP_ENGINE_GLOW_STRENGTH = "starbreaker_engine_glow_strength"
PROP_SHARED_GLOW_CONTROL_JSON = "starbreaker_shared_glow_control"
PROP_SHARED_GLOW_STRENGTH = "starbreaker_shared_glow_strength"
SCENE_POM_DETAIL_PROP = "starbreaker_pom_detail"
SCENE_WEAR_STRENGTH_PROP = "starbreaker_wear_strength"
SCENE_ENGINE_GLOW_PROP = "starbreaker_engine_glow_strength"
SCENE_SHARED_GLOW_PROP = "starbreaker_shared_glow_strength"
SURFACE_SHADER_MODE_PRINCIPLED = "principled_first"
SURFACE_SHADER_MODE_GLASS = "glass_bsdf"
DECAL_OFFSET_MODIFIER_NAME = "StarBreaker Decal Offset"
DECAL_OFFSET_EXTERNAL_DEFAULT = 0.0005
DECAL_OFFSET_INTERNAL_DEFAULT = 0.001

# Parallax-occlusion depth: the POM_Vector "Scale" input is the authored CryEngine
# PomDisplacement times POM_SCALE_MULTIPLIER (calibrated so the reference
# pom.blend's single-decal POM, Scale ~1.5, matches PomDisplacement 0.05), capped at
# POM_SCALE_MAX so the march does not swim. Look knobs, not game data.
POM_SCALE_MULTIPLIER = 30.0
POM_SCALE_MAX = 3.0

POM_DETAIL_DEFAULT = "MEDIUM"
POM_DETAIL_ITEMS = (
    ("LOW", "Low", "20 layers with reduced scale for faster viewport playback"),
    ("MEDIUM", "Medium", "50 layers with balanced scale; default"),
    ("HIGH", "High", "100 layers with extra scale for maximum detail"),
)
_POM_DETAIL_LAYERS = {
    "LOW": 20.0,
    "MEDIUM": 50.0,
    "HIGH": 100.0,
}


def pom_detail_settings(mode: str) -> tuple[int, float]:
    """Resolve a POM detail mode into ``(num_layers, scale_multiplier)``.

    The per-step delta entering ``Group.001`` inside each top-level POM
    root is ``parallax_dir * Scale / Layers**2`` (because ``Math.003``
    divides Scale by Layers, then ``Vector Math.002`` divides the result
    vector by Layers a second time). To keep the total march distance
    visually constant as ``Layers`` varies we scale ``Scale`` by
    ``Layers**2 / 40**2`` (40 is the original default layer count and
    the effective cap on the fixed 4-block iteration chain).
    """

    normalized = str(mode or POM_DETAIL_DEFAULT).upper()
    layers = _POM_DETAIL_LAYERS.get(normalized, _POM_DETAIL_LAYERS[POM_DETAIL_DEFAULT])
    scale_multiplier = (layers * layers) / (40.0 * 40.0)
    return int(layers), float(scale_multiplier)

PACKAGE_ROOT_PREFIX = "StarBreaker"
TEMPLATE_COLLECTION_NAME = "StarBreaker Template Cache"
GLTF_PBR_WATTS_TO_LUMENS = 683.0
LUMENS_PER_WATT_WHITE = 120.0
# Conversion from Star Citizen light intensity to Blender Point/Spot/Area
# radiant-flux Watts. SC intensities are treated as KHR_lights_punctual-style
# candela values (matching Blender's own glTF importer behaviour): total
# luminous flux = intensity * 4π, which divided by 683 lm/W gives Watts.
# Updated to 1500.0 for brighter interior lighting
# (Phase 36+: aligned with Max script's PowerModifier and user-verified brightness).
# See ``docs/star-citizen-lighting-research.md``.
import math as _math

SC_LIGHT_CANDELA_SCALE = 1500.0
LIGHT_CANDELA_TO_WATT = (4.0 * _math.pi) / GLTF_PBR_WATTS_TO_LUMENS
HEADLIGHT_GOBO_THROW_GAIN = 10.0
# Empirical visual-brightness multiplier. Star Citizen's in-engine light
# response is much brighter than a bare KHR conversion suggests; without this
# multiplier Aurora interiors render nearly black. Tuned against Aurora Mk2
# cabin default lighting to give a usable (but not over-exposed) first pass.
LIGHT_VISUAL_GAIN = 20.0
SCENE_AXIS_CONVERSION = Matrix(
    (
        (1.0, 0.0, 0.0, 0.0),
        (0.0, 0.0, -1.0, 0.0),
        (0.0, 1.0, 0.0, 0.0),
        (0.0, 0.0, 0.0, 1.0),
    )
)
SCENE_AXIS_CONVERSION_INV = SCENE_AXIS_CONVERSION.inverted()
GLTF_LIGHT_BASIS_CORRECTION = Quaternion((math.sqrt(0.5), 0.0, -math.sqrt(0.5), 0.0))
NON_COLOR_INPUT_KEYWORDS = ("normal", "roughness", "gloss", "mask", "height", "specular", "opacity", "id_map")
MATERIAL_IDENTITY_SCHEMA = "runtime_material_v13"

# HumanSkin_V2 → Principled BSDF. CryEngine's LIGHTINGMODEL_SKIN (screen-space
# skin scattering + thin-region transmittance) has no direct Principled
# equivalent; skin is rendered as random-walk subsurface with these knobs.
SKIN_SUBSURFACE_RADIUS = (1.0, 0.35, 0.2)  # per-channel scatter ratio; red travels furthest
SKIN_SUBSURFACE_SCALE = 0.004  # metres (packages are metre-scale)
# HumanSkin_V2 transmittance = TexSlot12.r * TranslucencyMultiplier * 0.25;
# widens the subsurface distance in thin regions (ears, nostrils).
SKIN_TRANSMITTANCE_SCALE_GAIN = 4.0

# HairPBR -> Principled BSDF. HairPBR diffuse is a wrapped term
# lerp((N.L + 1)/4, KajiyaKay, k) * hairColor * 0.5 - about half the energy of
# a Lambertian with albedo hairColor - so fibre/cap colour is scaled by this.
HAIR_DIFFUSE_SCALE = 0.5

# Look knobs (fork ADR 0015): values tuned by eye against the game's inventory tiles, not read from game data.
# Consumers read them as ``constants.NAME`` when they build a material, so a packshot profile can set one before a
# file opens: ``from starbreaker_addon.runtime import constants; constants.MONITOR_ROUGHNESS = 0.5``.

# Roughness of a Monitor's reflecting surface: the material authors no gloss; 0.35 matched the Dark Bear's silver
# eyeballs in the inventory tile (0.15 mirror-like, 0.6 matt).
MONITOR_ROUGHNESS = 0.35
# Glow colour of a LayerBlend's flat glowing layer: the weighted geometric mean Emissive^(1-w) * tint^w, the tint
# scaled to Emissive's peak (0 = Emissive alone, 1 = the tint). 0.5 matched the Chairman's Club's eye strip.
LAYER_BLEND_GLOW_TINT_WEIGHT = 0.5
# Range (smoothstep) of an ore thumbnail's vein map (TexSlot8 red) that turns rock into vein: Hephaestanite's
# orange covered half the rock at 0.3-0.7 and only flecks at 0.7-0.9, the tiles show patches in between.
ORE_VEIN_SMOOTHSTEP = (0.6, 0.8)
# Gain on an Organic material's (minerals: Hadanite, Beradom) layer tint x layer diffuse, for its grey detail-map
# diffuse (sRGB ~100): at 1 every mineral came out darker than the tiles.
ORGANIC_TINT_GAIN = 2.0
# A metal layer's tint below this (brightest channel) keeps only its hue on the specular (F0), above it scales it:
# the Hurston Attrition's copper rails (tint 0.061) drew black at full strength; the Ghost Mk II's 0.138 greys
# its aluminium as in the shop shots.
METAL_TINT_HUE_ONLY_BELOW = 0.1
# LayerBlend edge wear: the blend map's red (the top layer) is weighted by the wear mask (TexSlot11) x this gain,
# clamped, so only the darkest mask lets the layer below through (the Ace Interceptor's speckled shell kept fine
# scratches at 4). None leaves the red as authored: the material's edge wear shows (Morozov-SH plate edges).
LAYER_BLEND_WEAR_MASK_GAIN: float | None = 4.0
# Dynamic wear of the two-layer wear materials (WearLayer, vertex colours): None takes the scene's Wear Strength
# slider (default 0, a pristine catalogue item); a number (0-2) replaces it. LayerBlend blend-map materials leave
# their wear layers out and ignore it.
WEAR_STRENGTH_OVERRIDE: float | None = None
