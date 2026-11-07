"""Every bundled .blend library has a text image that matches it (fork ADR 0016).

The presence check runs everywhere; the comparison needs Blender and runs when
`BLENDER` names its executable (headless, `scripts/blend_text_image.py --check`).
"""

from __future__ import annotations

import os
import subprocess
import unittest
from pathlib import Path

ADDON_DIR = Path(__file__).resolve().parents[1]
RESOURCES = ADDON_DIR / "starbreaker_addon" / "resources"
SCRIPT = ADDON_DIR / "scripts" / "blend_text_image.py"


def libraries() -> list[Path]:
    return sorted(RESOURCES.glob("*.blend"))


class BlendTextImageTests(unittest.TestCase):
    def test_every_library_has_a_text_image(self) -> None:
        self.assertTrue(libraries())
        missing = [lib.name for lib in libraries() if not lib.with_name(lib.name + ".json").is_file()]
        self.assertEqual(missing, [], "write them with scripts/blend_text_image.py")

    @unittest.skipUnless(os.environ.get("BLENDER"), "BLENDER (the Blender executable) not set")
    def test_text_images_match_their_libraries(self) -> None:
        result = subprocess.run(
            [os.environ["BLENDER"], "-b", "--factory-startup", "--python", str(SCRIPT), "--", *map(str, libraries()), "--check"],
            capture_output=True,
            text=True,
            timeout=600,
        )
        self.assertEqual(result.returncode, 0, result.stderr[-4000:] or result.stdout[-4000:])


if __name__ == "__main__":
    unittest.main()
