"""Give `pom_library.blend` the fork's interface and upstream's POM fixes (fork ADR 0016).

One-time edit of the fork-point library, kept so the change is reproducible and
reviewable. Until fork ADR 0016 the add-on made the same changes at load time
(`groups.py`: `_ensure_pom_detail_control_on_tree`, `_ensure_uv_scale_on_pom_tree`).

`POM_Vector` (the root the add-on appends per height image):
- the "Layers" input goes; internal Value nodes "StarBreaker Detail Layers" and
  "StarBreaker Detail Scale Multiplier" (set at runtime from the scene's POM detail
  profile, `constants.pom_detail_settings`) feed the layer divisions and, through
  "StarBreaker POM Detail Scale" (Scale * multiplier), the march delta;
- inputs "UV Scale X/Y" scale the start UV and the march delta alike, so a tiled
  material marches in its tiled UV space.

Upstream's fixes:
- d2b93f0e: the height sampler in `POM_disp` repeats (CLIP returned black outside
  0-1 and collapsed the march) and embeds no image (the add-on binds one per copy);
- a6c9d44d: `tangent_space` takes the tangent of the render UV map instead of a
  map named "UVMap"; the sign multiply's label follows upstream.

Run with Blender, headless; the library is saved in place, then rewrite its text image:

    blender -b --factory-startup --python blender_addon/scripts/pom_library_fork_interface.py -- <pom_library.blend>
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import bpy

CONSTANTS = Path(__file__).resolve().parents[1] / "starbreaker_addon" / "runtime" / "constants.py"


def _default_detail() -> tuple[int, float]:
    spec = importlib.util.spec_from_file_location("sb_constants", CONSTANTS)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.pom_detail_settings(module.POM_DETAIL_DEFAULT)


def _input_item(tree, name):
    return next(
        (i for i in tree.interface.items_tree if i.item_type == "SOCKET" and i.in_out == "INPUT" and i.name == name),
        None,
    )


def _new(tree, kind, name, label, location):
    node = tree.nodes.new(kind)
    node.name = name
    node.label = label
    node.location = location
    return node


def detail_control(tree, layers: float, multiplier: float) -> None:
    nodes, links = tree.nodes, tree.links
    tree.interface.remove(_input_item(tree, "Layers"))
    layers_node = _new(tree, "ShaderNodeValue", "StarBreaker Detail Layers", "Detail Layers", (-420, 220))
    layers_node.outputs[0].default_value = float(layers)
    mult_node = _new(tree, "ShaderNodeValue", "StarBreaker Detail Scale Multiplier", "Scale Multiplier", (-420, 80))
    mult_node.outputs[0].default_value = float(multiplier)
    scale_math = _new(tree, "ShaderNodeMath", "StarBreaker POM Detail Scale", "POM Detail Scale", (-200, 160))
    scale_math.operation = "MULTIPLY"

    # the Group Input that fed Scale / Layers into the layer division
    links.new(nodes["Group Input.004"].outputs["Scale"], scale_math.inputs[0])
    links.new(mult_node.outputs[0], scale_math.inputs[1])
    layer_divide = nodes["Math.003"]  # DIVIDE: Scale * multiplier / layers
    links.new(scale_math.outputs[0], layer_divide.inputs[0])
    links.new(layers_node.outputs[0], layer_divide.inputs[1])
    links.new(layers_node.outputs[0], nodes["Vector Math.002"].inputs[1])  # DIVIDE: delta / layers
    first_steps = nodes["Group.001"]  # the first POM_10x_Layer_Steps; Scale and Bias stay on the inputs
    links.new(layers_node.outputs[0], first_steps.inputs["num layers"])


def uv_scale(tree) -> None:
    nodes, links = tree.nodes, tree.links
    for axis in ("UV Scale X", "UV Scale Y"):
        socket = tree.interface.new_socket(name=axis, in_out="INPUT", socket_type="NodeSocketFloat")
        socket.default_value = 1.0
        socket.min_value = 0.001
    group_input = _new(tree, "NodeGroupInput", "Group Input.UV Scale", "", (-1200, -300))
    combine = _new(tree, "ShaderNodeCombineXYZ", "UV Scale Vector", "UV Scale", (-960, -300))
    combine.inputs["Z"].default_value = 1.0
    scale_uv = _new(tree, "ShaderNodeVectorMath", "UV Scale UV", "UV Scale UV", (-720, -200))
    scale_uv.operation = "MULTIPLY"
    scale_delta = _new(tree, "ShaderNodeVectorMath", "UV Scale Delta", "UV Scale Delta", (-720, -400))
    scale_delta.operation = "MULTIPLY"

    links.new(group_input.outputs["UV Scale X"], combine.inputs["X"])
    links.new(group_input.outputs["UV Scale Y"], combine.inputs["Y"])
    links.new(nodes["Texture Coordinate"].outputs["UV"], scale_uv.inputs[0])
    links.new(combine.outputs[0], scale_uv.inputs[1])
    links.new(nodes["Vector Math.002"].outputs[0], scale_delta.inputs[0])
    links.new(combine.outputs[0], scale_delta.inputs[1])
    first_steps = nodes["Group.001"]
    links.new(scale_uv.outputs[0], first_steps.inputs["Vector"])
    links.new(scale_delta.outputs[0], first_steps.inputs["delta coordinates"])


def upstream_fixes() -> None:
    sampler = bpy.data.node_groups["POM_disp"].nodes["pom_displ"]
    sampler.extension = "REPEAT"
    sampler.image = None
    tangent_space = bpy.data.node_groups["tangent_space"]
    tangent_space.nodes["Tangent"].uv_map = ""
    tangent_space.nodes["Bitangent Sign Apply"].label = "Bitangent Sign Apply"


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print("usage: blender -b --factory-startup --python pom_library_fork_interface.py -- <pom_library.blend>", file=sys.stderr)
        return 2
    library = Path(argv[0]).resolve()
    bpy.ops.wm.open_mainfile(filepath=str(library), load_ui=False)
    root = bpy.data.node_groups["POM_Vector"]
    if _input_item(root, "Layers") is None:
        print(f"{library.name}: has no Layers input, the fork's interface is in already", file=sys.stderr)
        return 1
    layers, multiplier = _default_detail()
    detail_control(root, layers, multiplier)
    uv_scale(root)
    upstream_fixes()
    bpy.ops.wm.save_mainfile(filepath=str(library), compress=True)
    print(f"{library.name}: written")
    return 0


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    sys.exit(main(args))
