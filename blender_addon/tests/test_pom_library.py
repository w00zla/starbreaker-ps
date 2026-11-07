"""The fork's interface and upstream's POM fixes live in `pom_library.blend` (fork ADR 0016).

Reads the library's text image (`pom_library.blend.json`); `test_blend_text_images.py`
checks that the image matches the binary file.
"""

from __future__ import annotations

import json
from pathlib import Path
import unittest

IMAGE = Path(__file__).resolve().parents[1] / "starbreaker_addon" / "resources" / "pom_library.blend.json"


def _group(name: str) -> dict:
    return json.loads(IMAGE.read_text(encoding="utf-8"))["node_groups"][name]


def _node(tree: dict, name: str) -> dict:
    return next(n for n in tree["nodes"] if n["name"] == name)


class PomVectorInterfaceTests(unittest.TestCase):
    """The root group carries UV Scale X/Y and the detail control instead of a Layers input."""

    def setUp(self) -> None:
        self.tree = _group("POM_Vector")
        self.inputs = {
            item["name"]: item
            for item in self.tree["interface"]
            if item["kind"] == "SOCKET" and item["in_out"] == "INPUT"
        }

    def _from_group_input(self, socket_name: str, target: str) -> bool:
        identifier = self.inputs[socket_name]["identifier"]
        return any(
            link.startswith("Group Input") and link.split(" -> ")[0].endswith(f":{identifier}") and link.endswith(f" -> {target}")
            for link in self.tree["links"]
        )

    def test_inputs_are_scale_bias_non_planar_and_uv_scale(self) -> None:
        self.assertEqual(list(self.inputs), ["Scale", "Bias", "Non-planar", "UV Scale X", "UV Scale Y"])
        for axis in ("UV Scale X", "UV Scale Y"):
            self.assertEqual(self.inputs[axis]["default_value"], 1.0)
            self.assertEqual(self.inputs[axis]["min_value"], 0.001)

    def test_detail_values_default_to_the_default_profile(self) -> None:
        # the runtime sets them from the scene's profile; the file starts at MEDIUM (50 layers, 50**2 / 40**2)
        layers = _node(self.tree, "StarBreaker Detail Layers")
        multiplier = _node(self.tree, "StarBreaker Detail Scale Multiplier")
        self.assertEqual(layers["type"], "ShaderNodeValue")
        self.assertEqual(multiplier["type"], "ShaderNodeValue")
        self.assertEqual(layers["outputs"][0]["default"], 50.0)
        self.assertEqual(multiplier["outputs"][0]["default"], 1.5625)

    def test_detail_layers_drive_the_march(self) -> None:
        scale = _node(self.tree, "StarBreaker POM Detail Scale")
        self.assertEqual(scale["settings"]["operation"], "MULTIPLY")
        links = set(self.tree["links"])
        self.assertTrue(self._from_group_input("Scale", "StarBreaker POM Detail Scale:Value"))
        self.assertIn("StarBreaker Detail Scale Multiplier:Value -> StarBreaker POM Detail Scale:Value_001", links)
        self.assertIn("StarBreaker POM Detail Scale:Value -> Math.003:Value", links)
        self.assertIn("StarBreaker Detail Layers:Value -> Math.003:Value_001", links)
        self.assertIn("StarBreaker Detail Layers:Value -> Vector Math.002:Vector_001", links)
        # first POM_10x_Layer_Steps: num layers from the detail value, the raw Scale and Bias
        self.assertIn("StarBreaker Detail Layers:Value -> Group.001:Input_8", links)
        self.assertTrue(self._from_group_input("Scale", "Group.001:Input_11"))
        self.assertTrue(self._from_group_input("Bias", "Group.001:Input_12"))

    def test_uv_scale_scales_start_uv_and_delta(self) -> None:
        combine = _node(self.tree, "UV Scale Vector")
        self.assertEqual(combine["type"], "ShaderNodeCombineXYZ")
        self.assertEqual(next(s for s in combine["inputs"] if s["identifier"] == "Z")["default"], 1.0)
        for name in ("UV Scale UV", "UV Scale Delta"):
            self.assertEqual(_node(self.tree, name)["settings"]["operation"], "MULTIPLY")
        links = set(self.tree["links"])
        self.assertTrue(self._from_group_input("UV Scale X", "UV Scale Vector:X"))
        self.assertTrue(self._from_group_input("UV Scale Y", "UV Scale Vector:Y"))
        self.assertIn("Texture Coordinate:UV -> UV Scale UV:Vector", links)
        self.assertIn("UV Scale Vector:Vector -> UV Scale UV:Vector_001", links)
        self.assertIn("Vector Math.002:Vector -> UV Scale Delta:Vector", links)
        self.assertIn("UV Scale Vector:Vector -> UV Scale Delta:Vector_001", links)
        self.assertIn("UV Scale UV:Vector -> Group.001:Input_7", links)
        self.assertIn("UV Scale Delta:Vector -> Group.001:Input_1", links)


class UpstreamPomFixTests(unittest.TestCase):
    """upstream d2b93f0e (REPEAT height sampler) and a6c9d44d (tangent of the render UV map)"""

    def test_height_sampler_repeats_and_holds_no_image(self) -> None:
        sampler = _node(_group("POM_disp"), "pom_displ")
        self.assertEqual(sampler["settings"]["extension"], "REPEAT")
        self.assertIsNone(sampler["settings"]["image"])

    def test_tangent_reads_the_render_uv_map(self) -> None:
        self.assertEqual(_node(_group("tangent_space"), "Tangent")["settings"]["uv_map"], "")


if __name__ == "__main__":
    unittest.main()
