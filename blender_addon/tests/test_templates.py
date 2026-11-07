from __future__ import annotations

from pathlib import Path
import sys
import unittest
import subprocess


ADDON_ROOT = Path(__file__).resolve().parents[1]
STARBREAKER_ROOT = ADDON_ROOT.parent
REPO_ROOT = STARBREAKER_ROOT.parent

sys.path.insert(0, str(ADDON_ROOT))

from starbreaker_addon.manifest import FeatureFlags, LayerManifestEntry, MaterialSidecar, PaletteRouting, SubmaterialRecord, TextureReference
from dataclasses import replace

from starbreaker_addon.templates import (
    GLASS_TINT_HUE,
    active_submaterials,
    glass_refraction_gloss,
    glass_tints_from_palette,
    has_virtual_input,
    layer_blend_decal_sheet,
    layer_blend_glow_color,
    layer_blend_glow_slots,
    layer_blend_pom_decals,
    layer_blend_slots,
    material_palette_channels,
    monitor_emission_tint,
    monitor_specular,
    representative_textures,
    smoothness_texture_reference,
    template_plan_for_submaterial,
)


ARGO_EXTERIOR = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/ARGO/MOLE/argo_mole_exterior_TEX0.materials.json"
ARGO_INTERIOR = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/ARGO/MOLE/argo_mole_coramor_TEX0.materials.json"
COMPONENT_MASTER = REPO_ROOT / "ships/Data/Materials/vehicles/components/component_master_01_TEX0.materials.json"
_STARBREAKER_BIN = STARBREAKER_ROOT / "target/debug/starbreaker"


def _ensure_argo_mole_exported() -> None:
    """Run the decomposed export for ARGO MOLE if TEX0 fixture files are absent."""
    if ARGO_EXTERIOR.is_file() and ARGO_INTERIOR.is_file() and COMPONENT_MASTER.is_file():
        return
    if not _STARBREAKER_BIN.is_file():
        return  # binary not present; tests will skip naturally
    subprocess.run(
        [str(_STARBREAKER_BIN), "entity", "export", "ARGO MOLE", str(REPO_ROOT / "ships"), "--kind", "decomposed", "--lod", "0"],
        check=False,
        capture_output=True,
    )


_ensure_argo_mole_exported()
VULTURE_BASE = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/DRAK/Vulture/DRAK_Vulture_TEX0.materials.json"
VULTURE_ALT_A = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/DRAK/Vulture/drak_vulture_alt_a_TEX0.materials.json"
VULTURE_PIRATE_SKULL = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/DRAK/Vulture/DRAK_Vulture_Pirate_Skull_TEX0.materials.json"
SCORPIUS_BASE = REPO_ROOT / "ships/Data/Objects/Spaceships/Ships/RSI/Scorpius/RSI_Scorpius_TEX0.materials.json"


def synthetic_submaterial(shader_family: str, *, tokens: list[str] | None = None, active: bool = True) -> SubmaterialRecord:
    return SubmaterialRecord(
        index=0,
        submaterial_name=f"synthetic_{shader_family.lower()}",
        blender_material_name=None,
        shader=shader_family,
        shader_family=shader_family,
        activation_state="active" if active else "inactive",
        activation_reason="visible" if active else "nodraw",
        decoded_feature_flags=FeatureFlags(
            tokens=tokens or [],
            has_decal="DECAL" in (tokens or []),
            has_iridescence=False,
            has_parallax_occlusion_mapping="PARALLAX_OCCLUSION_MAPPING" in (tokens or []),
            has_stencil_map="STENCIL_MAP" in (tokens or []),
            has_vertex_colors=False,
        ),
        direct_textures=[],
        derived_textures=[],
        texture_slots=[],
        layer_manifest=[],
        palette_routing=PaletteRouting(material_channel=None, layer_channels=[]),
        public_params={},
        variant_membership={},
        virtual_inputs=[],
        raw={},
    )


class TemplateTests(unittest.TestCase):
    def test_untextured_placeholder_mesh_decal_is_nodraw(self) -> None:
        # The F8C's placeholder_livery_999th: no textures, waits for a paint.
        placeholder = SubmaterialRecord.from_value(
            {
                "submaterial_name": "placeholder_livery_999th",
                "shader_family": "MeshDecal",
                "decoded_feature_flags": {"has_stencil_map": True, "tokens": ["STENCIL_MAP"]},
            }
        )
        self.assertEqual(template_plan_for_submaterial(placeholder).template_key, "nodraw")

        untextured = SubmaterialRecord.from_value(
            {
                "submaterial_name": "stripes",
                "shader_family": "MeshDecal",
                "decoded_feature_flags": {"has_stencil_map": True, "tokens": ["STENCIL_MAP"]},
            }
        )
        self.assertEqual(template_plan_for_submaterial(untextured).template_key, "decal_stencil")

    @unittest.skipUnless(
        ARGO_EXTERIOR.is_file() and ARGO_INTERIOR.is_file() and COMPONENT_MASTER.is_file(),
        "ARGO MOLE fixtures not present; skipping fixture-dependent template test",
    )
    def test_fixture_submaterials_map_to_expected_template_families(self) -> None:
        exterior = MaterialSidecar.from_file(ARGO_EXTERIOR)
        pom = next(submaterial for submaterial in exterior.submaterials if submaterial.submaterial_name == "pom_decals")
        self.assertEqual(template_plan_for_submaterial(pom).template_key, "decal_stencil")

        interior = MaterialSidecar.from_file(ARGO_INTERIOR)
        screen = next(submaterial for submaterial in interior.submaterials if submaterial.shader_family == "DisplayScreen")
        self.assertEqual(template_plan_for_submaterial(screen).template_key, "screen_hud")
        self.assertTrue(has_virtual_input(screen, "$RenderToTexture"))

        component = MaterialSidecar.from_file(COMPONENT_MASTER)
        layered = next(
            submaterial
            for submaterial in component.submaterials
            if submaterial.shader_family == "LayerBlend_V2"
            and any(layer.palette_channel is not None for layer in submaterial.layer_manifest)
        )
        self.assertEqual(template_plan_for_submaterial(layered).template_key, "layered_wear")
        self.assertEqual(template_plan_for_submaterial(synthetic_submaterial("Monitor")).template_key, "screen_hud")
        self.assertTrue(material_palette_channels(layered))

    def test_synthetic_support_covers_biology_hair_and_effect_templates(self) -> None:
        self.assertEqual(template_plan_for_submaterial(synthetic_submaterial("HumanSkin_V2")).template_key, "biological")
        self.assertEqual(template_plan_for_submaterial(synthetic_submaterial("HairPBR")).template_key, "hair")
        self.assertEqual(template_plan_for_submaterial(synthetic_submaterial("Hologram")).template_key, "effects")

    def test_layer_blend_pom_decals_are_not_the_base_textures(self) -> None:
        # Morozov-SH core (rrs specialist heavy): the POM decal set in slots 1/2/4/8 beside the base normal in 3.
        def ref(slot: str, role: str, name: str) -> TextureReference:
            return TextureReference(role=role, source_path=None, export_path=f"t/{name}.png", export_kind="source", slot=slot)

        pom_color = ref("TexSlot1", "base_color", "poms_decals_diffa")
        pom_normal = ref("TexSlot2", "normal_gloss", "poms_decals_ddna")
        pom_spec = ref("TexSlot4", "specular_support", "poms_decals_spec")
        pom_height = ref("TexSlot8", "height", "poms_decals_displ")
        base_normal = ref("TexSlot3", "normal_gloss", "core_ddna")
        core = replace(
            synthetic_submaterial("LayerBlend_V2", tokens=["NORMAL_MAP", "BLEND_MAP", "DECALS", "HEIGHT_MAP"]),
            texture_slots=[pom_color, pom_normal, base_normal, pom_spec, pom_height],
            direct_textures=[replace(pom_color, slot=None, role="diffuse"), replace(pom_normal, slot=None)],
            derived_textures=[
                TextureReference(role="roughness", source_path=None, export_path="t/poms_decals_ddna_roughness.png", export_kind="derived"),
                TextureReference(role="roughness", source_path=None, export_path="t/core_ddna_roughness.png", export_kind="derived"),
            ],
        )
        pom = layer_blend_pom_decals(core)
        self.assertEqual((pom.color, pom.normal, pom.specular, pom.height), (pom_color, pom_normal, pom_spec, pom_height))
        self.assertEqual(pom.roughness.export_path, "t/poms_decals_ddna_roughness.png")
        textures = representative_textures(core)
        self.assertEqual(textures["normal"], "t/core_ddna.png")
        self.assertEqual(textures["roughness"], "t/core_ddna_roughness.png")
        self.assertIsNone(textures["base_color"])
        self.assertIsNone(textures["height"])
        # Without %DECALS slot 1 is no decal: plain LayerBlend keeps its textures.
        plain = replace(core, decoded_feature_flags=replace(core.decoded_feature_flags, tokens=["BLEND_MAP"]))
        self.assertIsNone(layer_blend_pom_decals(plain))
        self.assertEqual(representative_textures(plain)["base_color"], "t/poms_decals_diffa.png")

    def test_layer_blend_slots_take_flat_layers_and_fill_empty_ones_from_below(self) -> None:
        layer = lambda name, **extra: LayerManifestEntry.from_value({"name": name, **extra})
        textured = [layer("BaseLayer2"), layer("BaseLayer3"), layer("BaseLayer4"), layer("WearLayer2")]
        # The Artimex Chairman's Club helmet's face01_m: BaseLayer1 without a layer file.
        face = replace(
            synthetic_submaterial("LayerBlend_V2", tokens=["BLEND_MAP"]),
            layer_manifest=textured,
            flat_layers=[replace(layer("BaseLayer1", tint_color=[0.27, 0.0137, 0.0]), flat=True)],
        )
        slots = layer_blend_slots(face)
        self.assertEqual({index: entry.name for index, entry in slots.items()},
                         {1: "BaseLayer1", 2: "BaseLayer2", 3: "BaseLayer3", 4: "BaseLayer4"})
        self.assertTrue(slots[1].flat)
        # Without it slot 1 takes the layer underneath (the Zeus shell has BaseLayer2-4).
        shell = replace(face, flat_layers=[])
        self.assertEqual(layer_blend_slots(shell)[1].name, "BaseLayer2")
        # One layer has nothing to blend.
        self.assertEqual(layer_blend_slots(replace(face, layer_manifest=[layer("BaseLayer2")], flat_layers=[])), {})

    def test_layer_blend_glow_slots_are_the_flat_ones_of_a_glowing_material(self) -> None:
        layer = lambda name: LayerManifestEntry.from_value({"name": name})
        attributes = lambda emissive, glow: {"authored_attributes": [
            {"name": "Emissive", "value": emissive}, {"name": "Glow", "value": glow}]}
        face = replace(
            synthetic_submaterial("LayerBlend_V2", tokens=["BLEND_MAP"]),
            layer_manifest=[layer("BaseLayer2"), layer("BaseLayer3"), layer("BaseLayer4")],
            flat_layers=[replace(layer("BaseLayer1"), flat=True)],
            raw=attributes("0.93011099,0.30946895,0.022173885", "0.18000001"),
        )
        self.assertEqual(layer_blend_glow_slots(face), frozenset({1}))
        # No Emissive colour, no Glow, no flat layer or no LayerBlend: nothing glows.
        self.assertEqual(layer_blend_glow_slots(replace(face, raw=attributes("0,0,0", "0.18"))), frozenset())
        self.assertEqual(layer_blend_glow_slots(replace(face, raw=attributes("1,1,1", "0"))), frozenset())
        self.assertEqual(layer_blend_glow_slots(replace(face, flat_layers=[])), frozenset())
        self.assertEqual(layer_blend_glow_slots(replace(face, shader_family="Illum")), frozenset())
        # An empty slot over a flat one takes it, glow and all: the CLDA shoulder lights are BaseLayer2-4 without files.
        lights = replace(face, layer_manifest=[layer("BaseLayer1")],
                         flat_layers=[replace(layer(name), flat=True) for name in ("BaseLayer2", "BaseLayer3", "BaseLayer4")])
        self.assertEqual(layer_blend_glow_slots(lights), frozenset({2, 3, 4}))

    def test_layer_blend_glow_color_meets_the_glowing_layers_tint(self) -> None:
        # The Chairman's Club's eye strip: Emissive orange (R/G 3.0), its flat layer's tint deep red (R/G 19.7); the
        # inventory tile shows R/G about 7. The geometric mean of both (the tint at Emissive's peak) gave the tile's
        # 196/79/39 sRGB as 210/84/33; Emissive alone drew it yellowish (207/119/44).
        layer = lambda name: LayerManifestEntry.from_value({"name": name})
        face = replace(
            synthetic_submaterial("LayerBlend_V2", tokens=["BLEND_MAP"]),
            layer_manifest=[layer("BaseLayer2"), layer("BaseLayer3"), layer("BaseLayer4")],
            flat_layers=[replace(layer("BaseLayer1"), flat=True, tint_color=(0.27049783, 0.013702084, 0.0))],
            raw={"authored_attributes": [
                {"name": "Emissive", "value": "0.93011099,0.30946895,0.022173885"}, {"name": "Glow", "value": "0.18"}]},
        )
        color = layer_blend_glow_color(face, tint_weight=0.5)
        self.assertAlmostEqual(color[0], 0.93011099, places=5)
        self.assertAlmostEqual(color[1], 0.12075, places=4)
        self.assertAlmostEqual(color[2], 0.0, places=6)
        # A glowing layer without a tint keeps Emissive.
        untinted = replace(face, flat_layers=[replace(layer("BaseLayer1"), flat=True, tint_color=None)])
        self.assertEqual(layer_blend_glow_color(untinted, tint_weight=0.5), (0.93011099, 0.30946895, 0.022173885))
        # Nothing glows: no colour.
        self.assertIsNone(layer_blend_glow_color(replace(face, flat_layers=[]), tint_weight=0.5))

    def test_layer_blend_decals_take_the_decal_sheet(self) -> None:
        sheet = TextureReference(role="decal_sheet", source_path=None, export_path="t/sheet.png", export_kind="source", slot="TexSlot9")
        helmet = replace(
            synthetic_submaterial("LayerBlend_V2", tokens=["NORMAL_MAP", "BLEND_MAP", "DECALS"]),
            texture_slots=[sheet],
        )
        self.assertIs(layer_blend_decal_sheet(helmet), sheet)
        # The vertex colour is wear, dirt and AO there, not the second UV set a skin keeps in it.
        worn = replace(helmet, decoded_feature_flags=replace(helmet.decoded_feature_flags, tokens=["BLEND_MAP", "DECALS", "WEAR_DIRT_AO_VERTEX"]))
        self.assertIsNone(layer_blend_decal_sheet(worn))
        plain = replace(helmet, decoded_feature_flags=replace(helmet.decoded_feature_flags, tokens=["NORMAL_MAP", "BLEND_MAP"]))
        self.assertIsNone(layer_blend_decal_sheet(plain))
        illum = replace(synthetic_submaterial("Illum", tokens=["DECAL"]), texture_slots=[sheet])
        self.assertIsNone(layer_blend_decal_sheet(illum))

    def test_hard_surface_stencil_material_stays_on_hard_surface_path(self) -> None:
        hard_surface_stencil = synthetic_submaterial("HardSurface", tokens=["STENCIL_MAP", "STENCIL_AS_STICKER"])
        self.assertEqual(template_plan_for_submaterial(hard_surface_stencil).template_key, "physical_surface")

    @unittest.skipUnless(
        VULTURE_BASE.is_file() and VULTURE_ALT_A.is_file() and VULTURE_PIRATE_SKULL.is_file(),
        "Vulture fixtures not present; skipping livery_decal template tests",
    )
    def test_empty_vulture_livery_decal_downgrades_to_nodraw(self) -> None:
        sidecar = MaterialSidecar.from_file(VULTURE_ALT_A)
        livery_decal = next(
            submaterial
            for submaterial in sidecar.submaterials
            if submaterial.submaterial_name == "livery_decal"
        )

        self.assertEqual(template_plan_for_submaterial(livery_decal).template_key, "nodraw")

    @unittest.skipUnless(
        VULTURE_BASE.is_file() and VULTURE_ALT_A.is_file() and VULTURE_PIRATE_SKULL.is_file(),
        "Vulture fixtures not present; skipping livery_decal template tests",
    )
    def test_base_vulture_livery_decal_without_authored_inputs_stays_nodraw(self) -> None:
        sidecar = MaterialSidecar.from_file(VULTURE_BASE)
        livery_decal = next(
            submaterial
            for submaterial in sidecar.submaterials
            if submaterial.submaterial_name == "livery_decal"
        )

        self.assertEqual(template_plan_for_submaterial(livery_decal).template_key, "nodraw")

    @unittest.skipUnless(
        VULTURE_BASE.is_file() and VULTURE_ALT_A.is_file() and VULTURE_PIRATE_SKULL.is_file(),
        "Vulture fixtures not present; skipping livery_decal template tests",
    )
    def test_textured_vulture_livery_decal_stays_on_decal_path(self) -> None:
        sidecar = MaterialSidecar.from_file(VULTURE_PIRATE_SKULL)
        livery_decal = next(
            submaterial
            for submaterial in sidecar.submaterials
            if submaterial.submaterial_name == "livery_decal"
        )

        self.assertEqual(template_plan_for_submaterial(livery_decal).template_key, "decal_stencil")

    @unittest.skipUnless(
        VULTURE_BASE.is_file() and VULTURE_ALT_A.is_file() and VULTURE_PIRATE_SKULL.is_file(),
        "Vulture fixtures not present; skipping livery_decal template tests",
    )
    def test_inactive_vulture_ext_livery_logo_stays_on_decal_path(self) -> None:
        sidecar = MaterialSidecar.from_file(VULTURE_ALT_A)
        ext_livery = next(
            submaterial
            for submaterial in sidecar.submaterials
            if submaterial.submaterial_name == "Ext_livery_01"
        )

        self.assertEqual(ext_livery.activation_state, "inactive")
        self.assertEqual(template_plan_for_submaterial(ext_livery).template_key, "decal_stencil")

    def test_inactive_non_decal_material_stays_nodraw(self) -> None:
        self.assertEqual(template_plan_for_submaterial(synthetic_submaterial("Illum", active=False)).template_key, "nodraw")

    @unittest.skipUnless(
        SCORPIUS_BASE.is_file(),
        "Scorpius fixture not present; skipping Scorpius livery_decal template test",
    )
    def test_scorpius_missing_texture_livery_decal_downgrades_to_nodraw(self) -> None:
        sidecar = MaterialSidecar.from_file(SCORPIUS_BASE)
        livery_decal = next(
            submaterial
            for submaterial in sidecar.submaterials
            if submaterial.submaterial_name == "Livery_Decal"
        )

        self.assertEqual(livery_decal.activation_state, "inactive")
        self.assertEqual(livery_decal.activation_reason, "missing_base_color_texture")
        self.assertEqual(template_plan_for_submaterial(livery_decal).template_key, "nodraw")

    @unittest.skipUnless(COMPONENT_MASTER.is_file(), "component_master fixture not present; skipping")
    def test_representative_textures_pick_exportable_maps(self) -> None:
        component = MaterialSidecar.from_file(COMPONENT_MASTER)
        hard_surface = next(submaterial for submaterial in component.submaterials if submaterial.shader_family == "HardSurface")
        textures = representative_textures(hard_surface)
        self.assertIsNotNone(textures["base_color"])
        self.assertIsNotNone(textures["normal"])

    def test_representative_textures_include_opacity_roles(self) -> None:
        decal = synthetic_submaterial("MeshDecal", tokens=["DECAL"])
        decal = SubmaterialRecord(
            **{
                **decal.__dict__,
                "direct_textures": [
                    TextureReference(
                        role="opacity",
                        source_path="Data/Textures/test/decal_diff.dds",
                        export_path="Data/Textures/test/decal_diff.png",
                        export_kind="source",
                    )
                ],
            }
        )
        textures = representative_textures(decal)
        self.assertEqual(textures["opacity"], "Data/Textures/test/decal_diff.png")

    @unittest.skipUnless(COMPONENT_MASTER.is_file(), "component_master fixture not present; skipping")
    def test_smoothness_texture_reference_falls_back_to_layer_texture_slots(self) -> None:
        component = MaterialSidecar.from_file(COMPONENT_MASTER)
        layered_hard_surface = next(
            submaterial
            for submaterial in component.submaterials
            if submaterial.shader_family == "HardSurface"
            and any(layer.texture_slots for layer in submaterial.layer_manifest)
        )

        texture = smoothness_texture_reference(layered_hard_surface)

        self.assertIsNotNone(texture)
        self.assertEqual(texture.alpha_semantic, "smoothness")
        self.assertEqual(texture.texture_identity, "ddna_normal")

    def test_active_submaterials_filter_hidden_entries(self) -> None:
        active = synthetic_submaterial("HardSurface")
        inactive = synthetic_submaterial("NoDraw", active=False)
        self.assertEqual(active_submaterials([active, inactive]), [active])

    def test_glass_tints_from_palette_only_with_use_tint_palette(self) -> None:
        # Strata microTech visor (%USE_TINT_PALETTE): the palette's glass colour. ORC-mkX Singularity visor (no
        # flag): its own near-black Diffuse; tinted by the palette's white glass it drew clear.
        strata = synthetic_submaterial("GlassPBR", tokens=["NORMAL_MAP", "THIN_FILM", "DEPTH_PASS", "USE_TINT_PALETTE"])
        orc = synthetic_submaterial("GlassPBR", tokens=[])
        self.assertTrue(glass_tints_from_palette(strata))
        self.assertFalse(glass_tints_from_palette(orc))
        # Other shader families keep their palette routing.
        self.assertTrue(glass_tints_from_palette(synthetic_submaterial("HardSurface", tokens=[])))

    def test_glass_tint_hue_is_neutral(self) -> None:
        # The template's hue node turned every glass tint by +25° (0.57): the Strata microTech's lime visor drew teal.
        self.assertEqual(GLASS_TINT_HUE, 0.5)

    def test_screens_are_not_alpha_blended(self) -> None:
        # Blended surfaces are invisible to Eevee's raytraced refraction: the Fieldsbury Dark Bear's glowing pupils
        # (Monitor) behind their frosted lenses drew as grey discs.
        self.assertNotEqual(template_plan_for_submaterial(synthetic_submaterial("Monitor")).blend_method, "BLEND")

    def test_monitor_emission_is_tinted_by_its_diffuse(self) -> None:
        # The Dark Bear Grape's eye: a white pupil dot as texture, Diffuse purple; in game the pupils glow purple.
        eye = replace(
            synthetic_submaterial("Monitor"),
            raw={"authored_attributes": [{"name": "Diffuse", "value": "0.37626216,0,0.83879912"}]},
        )
        self.assertEqual(monitor_emission_tint(eye), (0.37626216, 0.0, 0.83879912))
        self.assertIsNone(monitor_emission_tint(replace(synthetic_submaterial("Monitor"), raw={})))
        hard_surface = replace(synthetic_submaterial("HardSurface"), raw=eye.raw)
        self.assertIsNone(monitor_emission_tint(hard_surface))

    def test_monitor_surface_reflects_its_specular(self) -> None:
        # The Dark Bear's eyeballs (Monitor, Specular 1,1,1) are silver in game; emission alone drew them black.
        eye = replace(
            synthetic_submaterial("Monitor"),
            raw={"authored_attributes": [{"name": "Specular", "value": "1,1,1"}]},
        )
        self.assertEqual(monitor_specular(eye), (1.0, 1.0, 1.0))
        dark = replace(synthetic_submaterial("Monitor"), raw={"authored_attributes": [{"name": "Specular", "value": "0,0,0"}]})
        self.assertIsNone(monitor_specular(dark))
        self.assertIsNone(monitor_specular(replace(synthetic_submaterial("HardSurface"), raw=eye.raw)))

    def test_glass_refraction_gloss_zero_is_clear(self) -> None:
        # The Dark Bear's eye lenses author RefractionGloss 0 and look clear in game; taken as frosted they hid the
        # glowing pupils behind grey discs. Other values stay as authored.
        self.assertEqual(glass_refraction_gloss(0.0), 1.0)
        self.assertEqual(glass_refraction_gloss(1.0), 1.0)
        self.assertEqual(glass_refraction_gloss(0.6), 0.6)


if __name__ == "__main__":
    unittest.main()