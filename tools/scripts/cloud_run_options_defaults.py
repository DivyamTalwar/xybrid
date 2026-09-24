"""Default only the new trailing RunOptions constructor arguments.

These are source-compatibility defaults, not wire-version negotiation: generated
bindings and the native library must still be shipped as a matching pair.
"""
from __future__ import annotations

import re


def add_cloud_run_options_defaults(source: str, language: str) -> str:
    """Fail on generator drift instead of silently losing optional arguments."""
    names = ("Provider", "Model", "GatewayUrl")
    for suffix in names:
        if language == "swift":
            pattern = rf"^(        cloud{suffix}: String\?)(?=,?$)"
            default = "nil"
        elif language == "kotlin":
            pattern = rf"^(    val cloud{suffix}: String\?)(?=,?$)"
            default = "null"
        elif language == "csharp":
            pattern = rf"(string\? Cloud{suffix})(?=,|\))"
            default = "null"
        elif language == "python":
            snake = {"Provider": "provider", "Model": "model", "GatewayUrl": "gateway_url"}[suffix]
            pattern = rf"^(    cloud_{snake}: str \| None)$"
            default = "None"
        else:
            raise ValueError(f"unsupported binding language: {language}")
        source, count = re.subn(pattern, rf"\1 = {default}", source, flags=re.MULTILINE)
        if count != 1:
            raise ValueError(f"expected one {language} RunOptions cloud{suffix} parameter, got {count}")
    return source
