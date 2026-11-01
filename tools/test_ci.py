#!/usr/bin/env python3
"""Check that CI clones/checks out the compatibility manifest's exact sources."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CiCompatibilityTests(unittest.TestCase):
    def test_native_setup_uses_manifest_sources(self):
        compat = json.loads((ROOT / 'compatibility.json').read_text(encoding='utf-8'))
        workflow = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        self.assertIn('horizon_mixins_repository', workflow)
        self.assertIn('horizon_mixins_revision', workflow)
        self.assertIn('horizon_repository', workflow)
        self.assertIn('horizon_revision', workflow)
        self.assertTrue(compat['horizon_revision'].isalnum())
        self.assertTrue(compat['horizon_mixins_revision'].isalnum())
        self.assertEqual(compat['bbs_mc_version'], '26.3')
        self.assertEqual(compat['bbs_data_version'], 5023)
        self.assertEqual(compat['hooks'], 1)


if __name__ == '__main__':
    unittest.main()
