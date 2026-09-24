"""Constructor defaults must not rewrite the append-only wire contract."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from cloud_run_options_defaults import add_cloud_run_options_defaults


class CloudRunOptionsDefaultsTests(unittest.TestCase):
    def test_all_generators_default_only_the_new_arguments(self):
        fixtures = {
            "swift": "        cloudProvider: String?,\n        cloudModel: String?,\n        cloudGatewayUrl: String?\n",
            "kotlin": "    val cloudProvider: String?,\n    val cloudModel: String?,\n    val cloudGatewayUrl: String?\n",
            "python": "    cloud_provider: str | None\n    cloud_model: str | None\n    cloud_gateway_url: str | None\n",
            "csharp": "XybridRunOptions(string? CloudProvider, string? CloudModel, string? CloudGatewayUrl) { this.CloudProvider = CloudProvider; }\npublic string? CloudProvider { get; }",
        }
        for language, source in fixtures.items():
            with self.subTest(language=language):
                result = add_cloud_run_options_defaults(source, language)
                default = {"swift": "nil", "python": "None"}.get(language, "null")
                self.assertEqual(result.count(" = " + default), 3)
                self.assertEqual(result.replace(" = " + default, ""), source)

    def test_missing_or_duplicate_fields_fail_closed(self):
        for language in ("swift", "kotlin", "python", "csharp"):
            with self.subTest(language=language), self.assertRaises(ValueError):
                add_cloud_run_options_defaults("", language)
        source = "    cloud_provider: str | None\n" * 2
        with self.assertRaises(ValueError):
            add_cloud_run_options_defaults(source, "python")
