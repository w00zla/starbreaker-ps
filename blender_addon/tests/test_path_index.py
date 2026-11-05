from __future__ import annotations

import os
from pathlib import Path
import sys
import tempfile
import unittest


ADDON_ROOT = Path(__file__).resolve().parents[1]

sys.path.insert(0, str(ADDON_ROOT))

from starbreaker_addon import manifest
from starbreaker_addon.manifest import PackageBundle, _scan_tree_like_rglob, share_path_indexes


def _rglob_index(root: Path) -> dict[str, Path]:
    """The path index as the add-on built it with rglob (the reference for the scandir walk)."""
    index: dict[str, Path] = {}
    for candidate in root.rglob("*"):
        if not candidate.is_file():
            continue
        index.setdefault(candidate.relative_to(root).as_posix().lower(), candidate)
    return index


def _touch(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")


def _make_tree(root: Path) -> dict[str, bool]:
    """A synthetic export root; returns which of the awkward parts the file system supports."""
    for name in (
        "Data/Objects/a/Tex_Diff.dds",
        "Data/Objects/a/tex_diff.DDS",  # case collision in one directory
        "Data/Objects/a/sub/deep/Thing.json",
        "Data/Objects/A/tex_diff.dds",  # the same path under a directory that differs by case
        "data/objects/a/TEX_DIFF.dds",  # ... and under another casing of the root
        "Packages/p1/scene.json",
        "Packages/p2/scene.json",
        "Packages/p2/.hidden/x.txt",
        "top.txt",
        "z/y/x/w/v.txt",
    ):
        _touch(root / name)
    (root / "empty/dir").mkdir(parents=True)
    supported = {"case": False, "symlink": False}
    # a case-insensitive file system folds the collisions above, which then test nothing
    supported["case"] = len({p.name for p in (root / "Data/Objects/a").iterdir() if p.is_file()}) == 2
    try:
        outside = root.parent / (root.name + "_outside")
        _touch(outside / "elsewhere.txt")
        os.symlink(root / "top.txt", root / "link_to_file.txt")
        os.symlink(root / "Packages", root / "link_to_dir")  # not descended by rglob
        os.symlink(outside, root / "link_outside")
        os.symlink(root / "does_not_exist", root / "broken_link")
        os.symlink(root, root / "Packages/p1/loop")  # a loop through a directory symlink
        supported["symlink"] = True
    except (OSError, NotImplementedError):
        pass
    return supported


class PathIndexWalkTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name) / "export"
        self.root.mkdir()
        self.supported = _make_tree(self.root)

    def _bundle(self) -> PackageBundle:
        return PackageBundle(self.root, self.root / "Packages/p1/scene.json", None, {}, {}, {})

    def test_walk_yields_the_entries_of_rglob_in_its_order(self) -> None:
        base = os.path.join(str(self.root), "")
        walked = [entry.path for entry in _scan_tree_like_rglob(base)]
        expected = [str(path) for path in self.root.rglob("*")]
        self.assertEqual(walked, expected)
        self.assertGreater(len(walked), 20)

    def test_index_equals_the_rglob_index_in_content_order_and_collision_winners(self) -> None:
        index = self._bundle()._build_path_index()
        expected = _rglob_index(self.root)
        self.assertEqual(list(index.items()), list(expected.items()))
        self.assertIn("data/objects/a/sub/deep/thing.json", index)
        if self.supported["case"]:
            # four files fold into one key; the winner is the one rglob meets first
            self.assertEqual(
                [key for key in index if key.endswith("tex_diff.dds")],
                [key for key in expected if key.endswith("tex_diff.dds")],
            )
            self.assertEqual(index["data/objects/a/tex_diff.dds"], expected["data/objects/a/tex_diff.dds"])

    def test_symlinks_files_count_directory_links_are_not_descended(self) -> None:
        if not self.supported["symlink"]:
            self.skipTest("symlinks not available")
        index = self._bundle()._build_path_index()
        self.assertIn("link_to_file.txt", index)  # a link to a file is a file
        self.assertNotIn("broken_link", index)
        self.assertFalse([key for key in index if key.startswith(("link_to_dir/", "link_outside/"))])
        self.assertNotIn("link_to_dir", index)  # a link to a directory is not a file
        self.assertFalse([key for key in index if "/loop/" in key])
        self.assertEqual(list(index.items()), list(_rglob_index(self.root).items()))

    def test_missing_root_gives_an_empty_index(self) -> None:
        bundle = PackageBundle(self.root / "nope", self.root / "nope/scene.json", None, {}, {}, {})
        self.assertEqual(bundle._build_path_index(), {})


class SharedPathIndexTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.addCleanup(share_path_indexes, False)
        self.root = Path(self._tmp.name)
        _touch(self.root / "Data/Objects/Tex_A.dds")

    def _bundle(self) -> PackageBundle:
        return PackageBundle(self.root, self.root / "Packages/x/scene.json", None, {}, {}, {})

    def test_sharing_is_off_by_default(self) -> None:
        self.assertIsNone(manifest._SHARED_PATH_INDEXES)
        first, second = self._bundle(), self._bundle()
        self.assertEqual(first.resolve_path("data/objects/tex_a.dds"), self.root / "Data/Objects/Tex_A.dds")
        self.assertEqual(second.resolve_path("data/objects/tex_a.dds"), self.root / "Data/Objects/Tex_A.dds")
        self.assertIsNot(first._path_index, second._path_index)

    def test_bundles_of_one_root_share_one_index(self) -> None:
        share_path_indexes(True)
        first, second = self._bundle(), self._bundle()
        self.assertEqual(first.resolve_path("data/objects/tex_a.dds"), self.root / "Data/Objects/Tex_A.dds")
        self.assertEqual(second.resolve_path("DATA/Objects/TEX_A.dds"), self.root / "Data/Objects/Tex_A.dds")
        self.assertIs(first._path_index, second._path_index)
        self.assertTrue(first._path_index_fresh)
        self.assertFalse(second._path_index_fresh)

    def test_other_roots_do_not_share(self) -> None:
        share_path_indexes(True)
        other = Path(self._tmp.name) / "other"
        _touch(other / "Data/Objects/Tex_B.dds")
        first = self._bundle()
        second = PackageBundle(other, other / "Packages/x/scene.json", None, {}, {}, {})
        first.resolve_path("data/objects/tex_a.dds")
        self.assertEqual(second.resolve_path("data/objects/tex_b.dds"), other / "Data/Objects/Tex_B.dds")
        self.assertIsNone(second.resolve_path("data/objects/tex_a.dds"))
        self.assertIsNot(first._path_index, second._path_index)

    def test_a_file_exported_after_the_index_was_built_is_still_found(self) -> None:
        share_path_indexes(True)
        self._bundle().resolve_path("data/objects/tex_a.dds")  # builds the shared index
        _touch(self.root / "Data/Objects/Later.dds")
        late = self._bundle()
        self.assertEqual(late.resolve_path("data/objects/later.dds"), self.root / "Data/Objects/Later.dds")
        self.assertTrue(late._path_index_fresh)
        # the rescan replaced the shared index: the next bundle starts from the new one
        self.assertEqual(self._bundle().resolve_path("data/objects/later.dds"), self.root / "Data/Objects/Later.dds")

    def test_a_file_removed_after_the_index_was_built_is_not_returned(self) -> None:
        share_path_indexes(True)
        self._bundle().resolve_path("data/objects/tex_a.dds")
        (self.root / "Data/Objects/Tex_A.dds").unlink()
        self.assertIsNone(self._bundle().resolve_path("data/objects/tex_a.dds"))

    def test_dropping_the_shared_indexes_returns_to_one_index_per_bundle(self) -> None:
        share_path_indexes(True)
        self._bundle().resolve_path("data/objects/tex_a.dds")
        share_path_indexes(False)
        self.assertIsNone(manifest._SHARED_PATH_INDEXES)
        first, second = self._bundle(), self._bundle()
        first.resolve_path("data/objects/tex_a.dds")
        second.resolve_path("data/objects/tex_a.dds")
        self.assertIsNot(first._path_index, second._path_index)

    def test_empty_path_never_builds_an_index(self) -> None:
        share_path_indexes(True)
        bundle = self._bundle()
        self.assertIsNone(bundle.resolve_path(None))
        self.assertIsNone(bundle._path_index)


if __name__ == "__main__":
    unittest.main()
