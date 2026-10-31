from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "import_prompt_library.py"
SPEC = importlib.util.spec_from_file_location("import_prompt_library", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PromptLibraryImportTests(unittest.TestCase):
    def test_classifies_reference_edit_and_variables(self) -> None:
        cases = {
            "Turn the reference photo into a watercolor portrait.": "reference-image",
            "Edit this image and replace the red mug with a blue cup.": "image-edit",
            "Create a poster featuring [PRODUCT].": "text-to-image",
        }
        for prompt, expected in cases.items():
            kind, variables, notes, _ = MODULE.classify_usage(prompt)
            self.assertEqual(kind, expected)
            self.assertEqual(variables, "[PRODUCT]" in prompt)
            if expected != "text-to-image" or variables:
                self.assertTrue(notes)

    def test_rejects_incomplete_jpeg(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "case1.jpg"
            image.write_bytes(b"\xff\xd8truncated")
            with self.assertRaisesRegex(ValueError, "不是完整 JPEG 或 PNG"):
                MODULE.image_metadata(image)

    def test_checked_in_catalog_is_complete(self) -> None:
        root = Path(__file__).resolve().parents[2]
        catalog = json.loads(
            (root / "resources/prompt-library/catalog.json").read_text(
                encoding="utf-8"
            )
        )
        MODULE.validate_checked_in_catalog(
            root / "resources/prompt-library/catalog.json"
        )
        ids = [case["sourceCaseId"] for case in catalog["cases"]]
        self.assertEqual(len(ids), 541)
        self.assertEqual(len(set(ids)), 541)
        self.assertEqual(sorted(set(range(1, 545)) - set(ids)), [12, 169, 170])
        self.assertEqual(
            {case["category"]["sourceLabel"] for case in catalog["cases"]},
            set(MODULE.CATEGORY_CONFIG),
        )


if __name__ == "__main__":
    unittest.main()
