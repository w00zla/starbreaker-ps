from __future__ import annotations

from pathlib import Path
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))

# test_decal_offset stubs bpy/mathutils and the addon packages on import
from test_decal_offset import BuildersMixin, FakeMesh, FakeObject, FakePolygon


class FoWithForeachGet(list):
    """A polygon collection like Blender's: foreach_get fills a numpy array."""

    def __init__(self, indices: list[int], fail: bool = False):
        super().__init__(FakePolygon(i, [0, 1, 2]) for i in indices)
        self.calls: list[str] = []
        self._fail = fail

    def foreach_get(self, attribute: str, out) -> None:
        self.calls.append(attribute)
        if self._fail:
            raise RuntimeError("no foreach_get here")
        out[:] = np.array([p.material_index for p in self], dtype=out.dtype)


class Importer(BuildersMixin):
    def __init__(self):
        self.mesh_polygon_counts_cache = {}


def _object(polygons) -> FakeObject:
    return FakeObject(material_slots=[], mesh=FakeMesh(polygons=polygons, vertex_count=0))


class PolygonCountTests(unittest.TestCase):
    INDICES = [2, 0, 0, 7, 2, 2, 0, 5]

    def test_foreach_get_counts_equal_the_python_loop(self) -> None:
        vectorised = FoWithForeachGet(self.INDICES)
        plain = [FakePolygon(i, [0, 1, 2]) for i in self.INDICES]
        counts = Importer()._polygon_counts_by_material_index(_object(vectorised))
        self.assertEqual(vectorised.calls, ["material_index"])
        self.assertEqual(counts, Importer()._polygon_counts_by_material_index(_object(plain)))
        self.assertEqual(counts, {0: 3, 2: 3, 5: 1, 7: 1})
        self.assertTrue(all(type(k) is int and type(v) is int for k, v in counts.items()))

    def test_fake_meshes_without_foreach_get_use_the_loop(self) -> None:
        plain = [FakePolygon(i, [0, 1, 2]) for i in self.INDICES]
        self.assertEqual(Importer()._polygon_counts_by_material_index(_object(plain)), {0: 3, 2: 3, 5: 1, 7: 1})

    def test_a_failing_foreach_get_falls_back_to_the_loop(self) -> None:
        broken = FoWithForeachGet(self.INDICES, fail=True)
        self.assertEqual(Importer()._polygon_counts_by_material_index(_object(broken)), {0: 3, 2: 3, 5: 1, 7: 1})

    def test_no_polygons_and_the_cache(self) -> None:
        importer = Importer()
        obj = _object(FoWithForeachGet([]))
        self.assertEqual(importer._polygon_counts_by_material_index(obj), {})
        obj = _object(FoWithForeachGet(self.INDICES))
        first = importer._polygon_counts_by_material_index(obj)
        self.assertIs(importer._polygon_counts_by_material_index(obj), first)
        self.assertEqual(obj.data.polygons.calls, ["material_index"])


if __name__ == "__main__":
    unittest.main()
