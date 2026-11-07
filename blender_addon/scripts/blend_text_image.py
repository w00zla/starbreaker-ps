"""Text image of a .blend node library (fork ADR 0016).

Writes, or checks, a JSON dump of every node group and material node tree in a
library: interface, nodes with their type, settings and unlinked input
defaults, links, and the nested groups they use. The image lies next to the
library as `<name>.blend.json`, so a change to the binary file shows as a
readable diff and a comparison with upstream's file is a text diff.

Run with Blender, headless:

    blender -b --factory-startup --python blender_addon/scripts/blend_text_image.py -- <lib.blend>... [--check]

Without `--check` the images are (re)written; with it nothing is written and the
exit code is 1 when an image is missing or differs from its library.
Node layout (location, width, selection, colours) is left out: it is not
behaviour, and it would bury real changes in noise.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import bpy

FORMAT = 1

# bpy_struct properties that describe layout, UI state or identity, not behaviour
_SKIP_NODE_PROPS = {
    "rna_type", "name", "label", "location", "location_absolute", "width", "height", "dimensions", "select",
    "show_options", "show_preview", "show_texture", "hide", "mute", "use_custom_color", "color", "color_tag",
    "parent", "inputs", "outputs", "internal_links", "type", "bl_idname", "bl_label", "bl_description",
    "bl_icon", "bl_static_type", "bl_width_default", "bl_width_min", "bl_width_max", "bl_height_default",
    "bl_height_min", "bl_height_max", "is_active_output", "warning_propagation",
}
_SKIP_SOCKET_ITEM_PROPS = {
    "rna_type", "name", "identifier", "item_type", "in_out", "position", "index", "parent", "select",
    "bl_socket_idname", "socket_type", "attribute_domain", "default_attribute_name", "layer_selection_field",
}
# nodes whose setting is their output's value: kept even when the output is linked
_VALUE_OUTPUT_NODES = {"ShaderNodeValue", "ShaderNodeRGB"}


def _num(value):
    if isinstance(value, float):
        value = round(value, 6)
        return 0.0 if value == 0 else value
    return value


def _value(value):
    """A property value as plain JSON: numbers rounded, vectors as lists, IDs by name"""
    if isinstance(value, (bool, int, str)) or value is None:
        return value
    if isinstance(value, float):
        return _num(value)
    if isinstance(value, bpy.types.ID):
        return {"id": type(value).__name__, "name": value.name}
    if isinstance(value, bpy.types.ColorRamp):
        return {
            "color_mode": value.color_mode,
            "interpolation": value.interpolation,
            "hue_interpolation": value.hue_interpolation,
            "elements": [{"position": _num(e.position), "color": _value(e.color)} for e in value.elements],
        }
    if isinstance(value, bpy.types.CurveMapping):
        return {
            "use_clip": value.use_clip,
            "clip": [_value(value.clip_min_x), _value(value.clip_min_y), _value(value.clip_max_x), _value(value.clip_max_y)],
            "curves": [
                {"extend": c.extend, "points": [{"location": _value(p.location), "handle_type": p.handle_type} for p in c.points]}
                for c in value.curves
            ],
        }
    if isinstance(value, bpy.types.ImageUser):
        return {"frame_start": value.frame_start, "frame_duration": value.frame_duration, "use_auto_refresh": value.use_auto_refresh}
    if isinstance(value, bpy.types.bpy_struct):
        # nested settings such as an image node's TexMapping / ColorMapping
        return {"struct": type(value).__name__, **_props(value, {"rna_type"})}
    try:
        return [_value(v) for v in value]
    except TypeError:
        return repr(value)


def _props(struct, skip):
    out = {}
    for prop in struct.bl_rna.properties:
        key = prop.identifier
        if key in skip or prop.type == "COLLECTION":
            continue
        try:
            out[key] = _value(getattr(struct, key))
        except (AttributeError, RuntimeError):
            continue
    return out


def _socket(socket, keep_default=False):
    out = {"identifier": socket.identifier, "name": socket.name, "type": socket.bl_idname}
    if not socket.enabled:
        out["enabled"] = False
    if socket.hide_value:
        out["hide_value"] = True
    if (keep_default or not socket.is_linked) and hasattr(socket, "default_value"):
        out["default"] = _value(socket.default_value)
    return out


def _interface(tree):
    items = []
    for item in tree.interface.items_tree:
        entry = {"kind": item.item_type, "name": item.name}
        if item.item_type == "SOCKET":
            entry.update({"in_out": item.in_out, "type": item.socket_type, "identifier": item.identifier})
        if item.parent is not None and item.parent.name:
            entry["panel"] = item.parent.name
        entry.update(_props(item, _SKIP_SOCKET_ITEM_PROPS))
        items.append(entry)
    return items


def _node(node):
    out = {"name": node.name, "type": node.bl_idname}
    if node.label:
        out["label"] = node.label
    if node.mute:
        out["mute"] = True
    if node.parent is not None:
        out["frame"] = node.parent.name
    settings = _props(node, _SKIP_NODE_PROPS)
    if settings:
        out["settings"] = settings
    out["inputs"] = [_socket(s) for s in node.inputs]
    out["outputs"] = [_socket(s, node.bl_idname in _VALUE_OUTPUT_NODES) for s in node.outputs]
    return out


def _link(link):
    """One line per link: `node:socket -> node:socket` by socket identifier"""
    text = f"{link.from_node.name}:{link.from_socket.identifier} -> {link.to_node.name}:{link.to_socket.identifier}"
    return text + (" (muted)" if link.is_muted else "") + ("" if link.is_valid else " (invalid)")


def _tree(tree):
    nodes = sorted(tree.nodes, key=lambda n: n.name)
    groups = sorted({n.node_tree.name for n in nodes if getattr(n, "node_tree", None) is not None})
    return {
        "type": tree.bl_idname,
        "interface": _interface(tree),
        "nodes": [_node(n) for n in nodes],
        "links": sorted(_link(link) for link in tree.links),
        "uses_groups": groups,
    }


def image(library: Path) -> str:
    bpy.ops.wm.open_mainfile(filepath=str(library), load_ui=False)
    data = {
        "format": FORMAT,
        "library": library.name,
        "node_groups": {g.name: _tree(g) for g in sorted(bpy.data.node_groups, key=lambda g: g.name) if g.library is None},
        "materials": {
            m.name: _tree(m.node_tree)
            for m in sorted(bpy.data.materials, key=lambda m: m.name)
            if m.library is None and m.node_tree is not None
        },
    }
    return json.dumps(data, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def image_path(library: Path) -> Path:
    return library.with_name(library.name + ".json")


def main(argv: list[str]) -> int:
    check = "--check" in argv
    libraries = [Path(a).resolve() for a in argv if a != "--check"]
    if not libraries:
        print("usage: blender -b --factory-startup --python blend_text_image.py -- <lib.blend>... [--check]", file=sys.stderr)
        return 2
    failed = 0
    for library in libraries:
        text = image(library)
        target = image_path(library)
        if check:
            # git may check the image out with CRLF (core.autocrlf on Windows)
            current = target.read_text(encoding="utf-8").replace("\r\n", "\n") if target.exists() else None
            if current != text:
                failed += 1
                print(f"{target.name}: {'missing' if current is None else 'differs from its library'}", file=sys.stderr)
            else:
                print(f"{target.name}: matches")
        else:
            target.write_text(text, encoding="utf-8", newline="\n")
            print(f"{target.name}: written")
    return 1 if failed else 0


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    sys.exit(main(args))
