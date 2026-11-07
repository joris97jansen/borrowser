"""The dependency source check must reject changes outside the reviewed patch."""

import importlib.util
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("check_ureq_patch", ROOT / "tools/ci/check_ureq_patch.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class UreqPatchIntegrity(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        shutil.copytree(ROOT / "vendor/ureq", self.root / "vendor/ureq")

    def test_reviewed_sources_pass(self):
        CHECK.check_sources(self.root)

    def test_unpatched_source_change_is_rejected(self):
        source = self.root / "vendor/ureq/src/middleware.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        with self.assertRaisesRegex(ValueError, "source hash mismatch"):
            CHECK.check_sources(self.root)

    def test_patched_source_change_is_rejected(self):
        source = self.root / "vendor/ureq/src/header.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        with self.assertRaisesRegex(ValueError, "source hash mismatch"):
            CHECK.check_sources(self.root)

    def test_unreviewed_file_is_rejected(self):
        (self.root / "vendor/ureq/src/extra.rs").write_text("// unreviewed file\n")
        with self.assertRaisesRegex(ValueError, "file inventory changed"):
            CHECK.check_sources(self.root)

    def test_rewriting_the_patch_record_cannot_admit_unreviewed_source(self):
        source = self.root / "vendor/ureq/src/header.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        path = self.root / "vendor/ureq/UPSTREAM.json"
        record = json.loads(path.read_text())
        record["patched_files"]["src/header.rs"] = hashlib.sha256(source.read_bytes()).hexdigest()
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "reviewed patch record changed"):
            CHECK.check_sources(self.root)


if __name__ == "__main__":
    unittest.main()
