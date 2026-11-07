"""Look knobs in ``runtime/constants.py`` (fork ADR 0015).

Values tuned by eye live in ``runtime/constants.py`` and are read from that
module when a material is built, so a packshot profile can set one before a
file opens: ``from starbreaker_addon.runtime import constants;
constants.MONITOR_ROUGHNESS = 0.5``. A ``from ..constants import NAME`` copy
would ignore that, so the guard below forbids it for the knobs.
"""

from __future__ import annotations

import ast
import sys
import types
import unittest
from dataclasses import replace
from pathlib import Path

ADDON_ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = ADDON_ROOT / "starbreaker_addon"
sys.path.insert(0, str(ADDON_ROOT))

for _name, _path in (
    ("starbreaker_addon", PACKAGE_ROOT),
    ("starbreaker_addon.runtime", PACKAGE_ROOT / "runtime"),
):
    if _name not in sys.modules:
        _module = types.ModuleType(_name)
        _module.__path__ = [str(_path)]
        sys.modules[_name] = _module

if "mathutils" not in sys.modules:
    _mathutils = types.ModuleType("mathutils")

    class _Matrix(tuple):
        def __new__(cls, rows):
            return tuple.__new__(cls, rows)

        def inverted(self):
            return self

    class _Quaternion(tuple):
        def __new__(cls, values):
            return tuple.__new__(cls, values)

    _mathutils.Matrix = _Matrix
    _mathutils.Quaternion = _Quaternion
    sys.modules["mathutils"] = _mathutils

if "bpy" not in sys.modules:
    _bpy = types.ModuleType("bpy")
    _bpy.types = types.SimpleNamespace(Nodes=object, NodeLinks=object, Node=object)
    sys.modules["bpy"] = _bpy

from starbreaker_addon.manifest import LayerManifestEntry, SubmaterialRecord  # noqa: E402
from starbreaker_addon.runtime import constants  # noqa: E402
from starbreaker_addon.runtime.importer.builders import BuildersMixin, _metal_tint_times_specular  # noqa: E402
from starbreaker_addon.runtime.importer.layers import LayersMixin  # noqa: E402
from starbreaker_addon.runtime.record_utils import _organic_base_tint  # noqa: E402
from starbreaker_addon.templates import layer_blend_glow_color  # noqa: E402
from test_templates import synthetic_submaterial  # noqa: E402

# name -> default; the defaults are today's look, the renders depend on them.
KNOBS = {
    "POM_SCALE_MULTIPLIER": 30.0,
    "POM_SCALE_MAX": 3.0,
    "SC_LIGHT_CANDELA_SCALE": 1500.0,
    "HEADLIGHT_GOBO_THROW_GAIN": 10.0,
    "LIGHT_VISUAL_GAIN": 20.0,
    "SKIN_SUBSURFACE_RADIUS": (1.0, 0.35, 0.2),
    "SKIN_SUBSURFACE_SCALE": 0.004,
    "SKIN_TRANSMITTANCE_SCALE_GAIN": 4.0,
    "HAIR_DIFFUSE_SCALE": 0.5,
    "MONITOR_ROUGHNESS": 0.35,
    "LAYER_BLEND_GLOW_TINT_WEIGHT": 0.5,
    "ORE_VEIN_SMOOTHSTEP": (0.6, 0.8),
    "ORGANIC_TINT_GAIN": 2.0,
    "METAL_TINT_HUE_ONLY_BELOW": 0.1,
    "LAYER_BLEND_WEAR_MASK_GAIN": 4.0,
    "WEAR_STRENGTH_OVERRIDE": None,
}


class _Socket:
    def __init__(self, name: str) -> None:
        self.name = name
        self.default_value = None
        self.is_linked = False


class _Sockets(dict):
    def __missing__(self, key):
        socket = _Socket(str(key))
        self[key] = socket
        return socket

    def get(self, key, default=None):
        return self[key]


class _Node:
    def __init__(self, bl_idname: str) -> None:
        self.bl_idname = bl_idname
        self.inputs = _Sockets()
        self.outputs = _Sockets()
        self.label = ""
        self.location = (0, 0)


class _Nodes(list):
    def new(self, bl_idname: str) -> _Node:
        node = _Node(bl_idname)
        self.append(node)
        return node


class _Links(list):
    def new(self, from_socket, to_socket):
        to_socket.is_linked = True
        self.append((from_socket, to_socket))


class _Probe(LayersMixin, BuildersMixin):
    def __init__(self, wear_strength: float = 0.0) -> None:
        self.context = types.SimpleNamespace(scene=types.SimpleNamespace(starbreaker_wear_strength=wear_strength))

    def _image_node(self, nodes, path, *, x, y, is_color):
        return nodes.new("ShaderNodeTexImage")

    def _apply_uv_tiling(self, nodes, links, node, tiling, *, x, y):
        return None


class _KnobTestCase(unittest.TestCase):
    def setUp(self) -> None:
        self._saved = {name: getattr(constants, name) for name in KNOBS}

    def tearDown(self) -> None:
        for name, value in self._saved.items():
            setattr(constants, name, value)


class KnobDefaultTests(unittest.TestCase):
    def test_each_knob_has_todays_value(self) -> None:
        for name, default in KNOBS.items():
            with self.subTest(name=name):
                self.assertEqual(getattr(constants, name), default)


class KnobsAreReadAtUseTimeTests(unittest.TestCase):
    @staticmethod
    def _modules():
        for path in sorted(PACKAGE_ROOT.rglob("*.py")):
            if path.name != "constants.py" or path.parent.name != "runtime":
                yield path, ast.parse(path.read_text(encoding="utf-8"))

    def test_no_module_copies_a_knob_by_name(self) -> None:
        for path, tree in self._modules():
            for node in ast.walk(tree):
                if isinstance(node, ast.ImportFrom) and (node.module or "").split(".")[-1] == "constants":
                    copied = sorted(alias.name for alias in node.names if alias.name in KNOBS)
                    with self.subTest(module=str(path.relative_to(ADDON_ROOT))):
                        self.assertEqual(copied, [], "read these as constants.NAME when used")

    def test_each_knob_has_a_consumer(self) -> None:
        used = {
            node.attr
            for _, tree in self._modules()
            for node in ast.walk(tree)
            if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "constants"
        }
        self.assertEqual(sorted(set(KNOBS) - used), [])


class MonitorRoughnessTests(_KnobTestCase):
    def _glossy(self) -> _Node:
        eye = replace(synthetic_submaterial("Monitor"), raw={"authored_attributes": [{"name": "Specular", "value": "1,1,1"}]})
        nodes = _Nodes()
        _Probe()._monitor_surface_shader(nodes, _Links(), eye, _Socket("surface"))
        return next(node for node in nodes if node.bl_idname == "ShaderNodeBsdfGlossy")

    def test_the_monitor_surface_takes_the_knob(self) -> None:
        self.assertEqual(self._glossy().inputs["Roughness"].default_value, 0.35)
        constants.MONITOR_ROUGHNESS = 0.6
        self.assertEqual(self._glossy().inputs["Roughness"].default_value, 0.6)


class LayerBlendGlowTests(unittest.TestCase):
    FACE = replace(
        synthetic_submaterial("LayerBlend_V2", tokens=["BLEND_MAP"]),
        layer_manifest=[LayerManifestEntry.from_value({"name": "BaseLayer2"})],
        flat_layers=[replace(LayerManifestEntry.from_value({"name": "BaseLayer1"}), flat=True, tint_color=(0.27, 0.0137, 0.0))],
        raw={"authored_attributes": [{"name": "Emissive", "value": "0.93,0.31,0.022"}, {"name": "Glow", "value": "0.18"}]},
    )

    def test_the_weight_moves_between_emissive_and_the_tint(self) -> None:
        emissive = (0.93, 0.31, 0.022)
        tint = tuple(t * 0.93 / 0.27 for t in (0.27, 0.0137, 0.0))
        for weight, want in ((0.0, emissive), (1.0, tint), (0.5, tuple((e * t) ** 0.5 for e, t in zip(emissive, tint)))):
            for got, expected in zip(layer_blend_glow_color(self.FACE, tint_weight=weight), want):
                self.assertAlmostEqual(got, expected, places=6)


class OreVeinTests(_KnobTestCase):
    def _range(self) -> tuple[float, float]:
        nodes = _Nodes()
        _Probe()._layer_blend_wear_socket(nodes, _Links(), "veins.png", x=0, y=0)
        ramp = next(node for node in nodes if node.bl_idname == "ShaderNodeMapRange")
        return ramp.inputs["From Min"].default_value, ramp.inputs["From Max"].default_value

    def test_the_vein_smoothstep_takes_the_knob(self) -> None:
        self.assertEqual(self._range(), (0.6, 0.8))
        constants.ORE_VEIN_SMOOTHSTEP = (0.3, 0.7)
        self.assertEqual(self._range(), (0.3, 0.7))


class OrganicTintTests(unittest.TestCase):
    # record_utils stays bpy- and mathutils-free: the builder passes constants.ORGANIC_TINT_GAIN.
    def test_the_organic_tint_takes_the_gain(self) -> None:
        crystal = SubmaterialRecord.from_value(
            {"shader_family": "Organic", "layer_manifest": [{"index": 0, "name": "Layer1", "tint_color": [0.4, 0.2, 0.1]}]}
        )
        for gain, want in ((2.0, (0.8, 0.4, 0.2)), (1.0, (0.4, 0.2, 0.1))):
            for got, expected in zip(_organic_base_tint(crystal, gain=gain), want):
                self.assertAlmostEqual(got, expected)


class MetalTintTests(_KnobTestCase):
    def test_the_hue_only_threshold_takes_the_knob(self) -> None:
        # A tint under the threshold keeps only its hue: (0.05, 0.05, 0.05) leaves the specular as it is.
        self.assertEqual(_metal_tint_times_specular((0.05, 0.05, 0.05), (0.5, 0.5, 0.5)), (0.5, 0.5, 0.5))
        constants.METAL_TINT_HUE_ONLY_BELOW = 0.01
        for got in _metal_tint_times_specular((0.05, 0.05, 0.05), (0.5, 0.5, 0.5)):
            self.assertAlmostEqual(got, 0.025)


class WearTests(_KnobTestCase):
    def test_the_override_replaces_the_scene_wear_strength(self) -> None:
        self.assertEqual(_Probe(0.7)._wear_strength(), 0.7)
        constants.WEAR_STRENGTH_OVERRIDE = 1.5
        self.assertEqual(_Probe(0.7)._wear_strength(), 1.5)
        constants.WEAR_STRENGTH_OVERRIDE = 5.0
        self.assertEqual(_Probe(0.0)._wear_strength(), 2.0)  # the slider's range still applies

    def _mask_gains(self) -> list[float]:
        nodes = _Nodes()
        blend = _Socket("blend")
        result = _Probe()._layer_blend_unworn(nodes, _Links(), synthetic_submaterial("LayerBlend_V2"), blend, 1.0, x=0, y=0)
        gains = [n.inputs[1].default_value for n in nodes if n.bl_idname == "ShaderNodeMath" and n.inputs[1].default_value]
        return gains if result is not blend else []

    def test_the_layer_blend_wear_mask_takes_the_gain(self) -> None:
        self.assertEqual(self._mask_gains(), [4.0])
        constants.LAYER_BLEND_WEAR_MASK_GAIN = 8.0
        self.assertEqual(self._mask_gains(), [8.0])

    def test_without_a_gain_the_authored_edge_wear_shows(self) -> None:
        constants.LAYER_BLEND_WEAR_MASK_GAIN = None
        self.assertEqual(self._mask_gains(), [])


if __name__ == "__main__":
    unittest.main()
