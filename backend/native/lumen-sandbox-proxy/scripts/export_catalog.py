#!/usr/bin/env python3
"""Export the external-app action catalog for the Rust gate.

The Python providers stay the single source of truth for what a connected app
can do and what policy each action defaults to. The Rust matcher decides with
the same catalog, so it reads this file rather than re-declaring the actions.

Exports the *full* catalog with `requires_self_hosted_scope` carried through,
not the cloud-filtered view: the filter depends on `MULTI_TENANT`, which is a
property of the running deployment, not of the export.

`backend/tests/unit/lumen/sandbox_proxy/test_exported_catalog.py` fails if the
committed file drifts from the Python providers.

Run from the repo root:

    uv run python backend/native/lumen-sandbox-proxy/scripts/export_catalog.py
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
sys.path.append(str(_CRATE_DIR.parent.parent))

CATALOG_PATH = _CRATE_DIR / "data" / "action_catalog.json"


def _rule(rule: Any) -> dict[str, Any]:
    """One match rule, in the shape `crate::matching::MatchRule` deserializes."""
    from lumen.external_apps.providers.actions import GraphQLOp, RestRoute

    if isinstance(rule, RestRoute):
        return {"kind": "rest", "method": rule.method, "path": rule.path}
    if isinstance(rule, GraphQLOp):
        return {
            "kind": "graphql",
            "operation_type": rule.operation_type,
            "field": rule.field,
        }
    # A new rule kind needs a Rust matcher before it can be exported; failing
    # here is what stops the gate silently ignoring it.
    raise TypeError(
        f"no exporter for match rule kind {type(rule).__name__}; "
        "add one here and a matcher in lumen-sandbox-proxy/src/matching/engine.rs"
    )


def build_catalog() -> dict[str, Any]:
    from lumen.external_apps.providers.registry import PROVIDERS

    apps: list[dict[str, Any]] = []
    for app_type, provider in sorted(PROVIDERS.items(), key=lambda kv: kv[0].value):
        spec = provider.spec
        apps.append(
            {
                "app_type": app_type.value,
                "app_name": spec.app_name,
                # Built-in providers author regexes, used as-is by
                # `resolve_app_for_url`. CUSTOM apps author globs and are not
                # in this registry.
                "upstream_url_regexes": list(spec.descriptor.upstream_url_patterns),
                "actions": [
                    {
                        # `.value`, never `str()`: these ids are `str`-mixin
                        # enum members, so `str()` renders "ClassName.MEMBER"
                        # while the database, the admin API and the policy map
                        # all key on the value.
                        "id": endpoint.id.value,
                        "normalised_name": endpoint.normalised_name,
                        "description": endpoint.description,
                        "default_policy": endpoint.default_policy.value,
                        "requires_self_hosted_scope": (
                            endpoint.requires_self_hosted_scope
                        ),
                        "matches": [_rule(rule) for rule in endpoint.matches],
                    }
                    for endpoint in spec.endpoint_catalog
                ],
            }
        )
    return {"apps": apps}


def write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    # An explicit path lets the drift test run this exact command into a temp
    # file rather than re-importing the builder.
    destination = Path(sys.argv[1]) if len(sys.argv) > 1 else CATALOG_PATH
    catalog = build_catalog()
    write(destination, catalog)
    actions = sum(len(app["actions"]) for app in catalog["apps"])
    print(f"wrote {destination} ({len(catalog['apps'])} apps, {actions} actions)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
