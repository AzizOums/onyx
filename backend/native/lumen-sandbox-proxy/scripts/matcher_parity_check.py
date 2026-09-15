#!/usr/bin/env python3
"""Check that the Rust matcher reaches the same verdict as the Python one.

The gate's verdict decides whether a sandbox's outbound request is forwarded,
blocked, or held for approval. A divergence here is not a formatting
difference: it is a request that one implementation gates and the other lets
through.

This builds a corpus from the real provider catalog — every REST route and
GraphQL rule, plus the near-misses that a matcher gets wrong (a changed method,
an extra segment, an empty segment, a field hidden in a fragment) — runs it
through both implementations, and compares the verdicts field by field,
including the order of the matched actions.

Run from the repo root:

    cargo build --manifest-path backend/native/Cargo.toml \
        -p lumen-sandbox-proxy --example match_corpus
    uv run python backend/native/lumen-sandbox-proxy/scripts/matcher_parity_check.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from unittest import mock

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
_NATIVE_DIR = _CRATE_DIR.parent
sys.path.append(str(_NATIVE_DIR.parent))

from lumen.db.enums import EndpointPolicy, ExternalAppType, GatedAppKind  # noqa: E402
from lumen.external_apps.matching import engine as engine_module  # noqa: E402
from lumen.external_apps.matching.engine import (  # noqa: E402
    apply_credential_gate,
    recognize_actions,
)
from lumen.external_apps.matching.request import ProxiedRequest  # noqa: E402
from lumen.external_apps.providers.actions import GraphQLOp, RestRoute  # noqa: E402
from lumen.external_apps.providers.registry import (  # noqa: E402
    PROVIDERS,
    get_endpoint_catalog,
)

APP_ID = 1


@dataclass
class Case:
    """One request, plus the context both matchers evaluate it in."""

    label: str
    app_type: str
    method: str
    path: str
    body: str | None = None
    body_bytes: bytes | None = None
    stored: dict[str, str] = field(default_factory=dict)
    is_available: bool = True

    def raw_body(self) -> bytes | None:
        """The body exactly as the sandbox would have put it on the wire."""
        if self.body_bytes is not None:
            return self.body_bytes
        return self.body.encode() if self.body is not None else None

    def as_json(self) -> dict[str, Any]:
        return {
            "app_type": self.app_type,
            "app_id": APP_ID,
            "method": self.method,
            "path": self.path,
            "body": self.body,
            "body_bytes": list(self.body_bytes)
            if self.body_bytes is not None
            else None,
            "stored": self.stored,
            "is_available": self.is_available,
        }


class _StubApp:
    """The two attributes `recognize_actions` and `_app_name` read off an app.

    A real `ExternalApp` row would need a database; the matcher itself is pure,
    so the stub keeps this check runnable anywhere.
    """

    def __init__(self, app_type: ExternalAppType) -> None:
        self.id = APP_ID
        self.app_type = app_type
        self.name = app_type.value


def _fill(path: str) -> str:
    """A concrete request path for a route template."""
    segments = []
    for index, segment in enumerate(path.split("/")):
        if segment.startswith("{") and segment.endswith("...}"):
            segments.append(f"tail{index}/deeper{index}")
        elif segment.startswith("{") and segment.endswith("}"):
            segments.append(f"id{index}")
        else:
            segments.append(segment)
    return "/".join(segments)


def _rest_cases(app_type: str, action_id: str, rule: RestRoute) -> list[Case]:
    """The exact route, then the near-misses that separate a correct matcher
    from one that is merely close."""
    exact = _fill(rule.path)
    other_method = "POST" if rule.method.upper() != "POST" else "GET"
    cases = [
        Case(f"{action_id} exact", app_type, rule.method, exact),
        Case(f"{action_id} lowercase method", app_type, rule.method.lower(), exact),
        Case(f"{action_id} other method", app_type, other_method, exact),
        Case(f"{action_id} trailing slash", app_type, rule.method, exact + "/"),
        Case(f"{action_id} extra segment", app_type, rule.method, exact + "/extra"),
        Case(
            f"{action_id} dropped segment",
            app_type,
            rule.method,
            exact.rsplit("/", 1)[0] or "/",
        ),
        Case(f"{action_id} query string kept", app_type, rule.method, exact + "?a=b"),
        Case(f"{action_id} case-changed path", app_type, rule.method, exact.upper()),
    ]
    if any(seg.endswith("...}") for seg in rule.path.rstrip("/").split("/")):
        # The case that separates a one-or-more wildcard from a zero-or-more
        # one: the tail swallows nothing. A matcher that accepts this gates a
        # whole collection endpoint as if it were a single resource.
        prefix = _fill(rule.path.rstrip("/").rsplit("/", 1)[0])
        cases.append(
            Case(f"{action_id} wildcard tail absent", app_type, rule.method, prefix)
        )
        cases.append(
            Case(
                f"{action_id} wildcard tail empty", app_type, rule.method, prefix + "/"
            )
        )
        cases.append(
            Case(
                f"{action_id} wildcard tail with empty segment",
                app_type,
                rule.method,
                prefix + "/a//b",
            )
        )

    if "{" in rule.path:
        # An empty placeholder: `//` must not satisfy a resource id.
        emptied = "/".join(
            "" if segment.startswith("{") else segment
            for segment in rule.path.split("/")
        )
        cases.append(
            Case(f"{action_id} empty placeholder", app_type, rule.method, emptied)
        )
        # A placeholder value that looks like a path separator when decoded.
        cases.append(
            Case(
                f"{action_id} encoded slash in id",
                app_type,
                rule.method,
                _fill(rule.path).replace("id1", "a%2Fb", 1),
            )
        )
    return cases


def _graphql_cases(app_type: str, action_id: str, rule: GraphQLOp) -> list[Case]:
    """Each GraphQL rule, plus the bodies a matcher can be fooled by."""
    op = rule.operation_type
    other_op = "mutation" if op != "mutation" else "query"
    field_name = rule.field
    bodies = {
        "exact": json.dumps({"query": f"{op} {{ {field_name} {{ id }} }}"}),
        "other operation type": json.dumps(
            {"query": f"{other_op} {{ {field_name} {{ id }} }}"}
        ),
        "named operation": json.dumps(
            {"query": f"{op} Named($x: ID) {{ {field_name}(id: $x) {{ id }} }}"}
        ),
        "hidden in a fragment": json.dumps(
            {
                "query": (
                    f"{op} {{ ...F }} "
                    f"fragment F on Whatever {{ {field_name} {{ id }} }}"
                )
            }
        ),
        "hidden in an inline fragment": json.dumps(
            {"query": f"{op} {{ ... on Whatever {{ {field_name} {{ id }} }} }}"}
        ),
        "batched": json.dumps(
            [
                {"query": "query { somethingElse }"},
                {"query": f"{op} {{ {field_name} {{ id }} }}"},
            ]
        ),
        "nested not root": json.dumps(
            {"query": f"{op} {{ outer {{ {field_name} {{ id }} }} }}"}
        ),
        "aliased": json.dumps({"query": f"{op} {{ alias: {field_name} {{ id }} }}"}),
        "syntax error": json.dumps({"query": f"{op} {{ {field_name} "}),
        "not json": "this is not json",
        "empty body": "",
        "json but not graphql": json.dumps({"variables": {"a": 1}}),
        # Where two GraphQL parsers are most likely to disagree. A body one
        # accepts and the other rejects changes the verdict, so these are
        # measured rather than assumed.
        "comments around the field": json.dumps(
            {"query": f"{op} {{ # a comment\n  {field_name} {{ id }} \n}}"}
        ),
        "commas as whitespace": json.dumps(
            {"query": f"{op} {{ {field_name}(a: 1, b: 2) {{ id, name }} }}"}
        ),
        "directive on the field": json.dumps(
            {"query": f"{op} {{ {field_name} @include(if: true) {{ id }} }}"}
        ),
        "block string argument": json.dumps(
            {"query": f'{op} {{ {field_name}(text: """multi\nline""") {{ id }} }}'}
        ),
        "unicode escape in a string argument": json.dumps(
            {"query": f'{op} {{ {field_name}(text: "\\u00e9") {{ id }} }}'}
        ),
        "variable definitions with defaults": json.dumps(
            {
                "query": (
                    f'{op} N($a: String = "x", $b: [Int!]! = [1,2]) '
                    f"{{ {field_name}(a: $a) {{ id }} }}"
                )
            }
        ),
        "byte order mark": "\ufeff"
        + json.dumps({"query": f"{op} {{ {field_name} {{ id }} }}"}),
        "two operations one matching": json.dumps(
            {
                "query": (
                    f"query A {{ somethingElse }} {other_op} B {{ x }} "
                    f"{op} C {{ {field_name} {{ id }} }}"
                )
            }
        ),
        "fragment cycle": json.dumps(
            {
                "query": (
                    f"{op} {{ ...F }} fragment F on T {{ {field_name} ...G }} "
                    "fragment G on T {{ ...F }}"
                )
                .replace("{{", "{")
                .replace("}}", "}")
            }
        ),
        "deeply nested selection": json.dumps(
            {"query": f"{op} {{ {field_name} " + "{ a " * 40 + "}" * 40 + " }"}
        ),
        "null byte in the body": json.dumps(
            {"query": f"{op} {{ {field_name} {{ id }} }}"}
        ).replace("id", "i\u0000d"),
    }
    cases = [
        Case(f"{action_id} graphql {name}", app_type, "POST", "/graphql", body)
        for name, body in bodies.items()
    ]

    # The same body in the encodings Python's `json.loads` detects and decodes.
    # A body the Python gate recognises and the Rust one does not falls through
    # to the whole-domain ASK, which never consults this action's policy — so an
    # explicit DENY would be evaded by a choice of encoding.
    exact = bodies["exact"]
    cases.extend(
        Case(
            f"{action_id} graphql body in {encoding}",
            app_type,
            "POST",
            "/graphql",
            body_bytes=exact.encode(encoding),
        )
        for encoding in (
            "utf-8-sig",
            "utf-16",
            "utf-16-be",
            "utf-16-le",
            "utf-32",
            "utf-32-be",
        )
    )
    return cases


def build_corpus() -> list[Case]:
    cases: list[Case] = []
    for app_type in sorted(PROVIDERS, key=lambda t: t.value):
        catalog = get_endpoint_catalog(app_type)
        for endpoint in catalog:
            action_id = endpoint.id.value
            for rule in endpoint.matches:
                if isinstance(rule, RestRoute):
                    cases.extend(_rest_cases(app_type.value, action_id, rule))
                elif isinstance(rule, GraphQLOp):
                    cases.extend(_graphql_cases(app_type.value, action_id, rule))
                else:
                    raise TypeError(f"no corpus builder for {type(rule).__name__}")

        # The credential gate, over a request that does match and one that
        # does not, under each availability.
        matching = next(
            (
                _fill(rule.path)
                for endpoint in catalog
                for rule in endpoint.matches
                if isinstance(rule, RestRoute)
            ),
            None,
        )
        method = next(
            (
                rule.method
                for endpoint in catalog
                for rule in endpoint.matches
                if isinstance(rule, RestRoute)
            ),
            "GET",
        )
        unmatched = "/definitely/not/a/catalog/route"
        for available in (True, False):
            cases.append(
                Case(
                    f"{app_type.value} unmatched available={available}",
                    app_type.value,
                    "GET",
                    unmatched,
                    is_available=available,
                )
            )
            if matching is not None:
                cases.append(
                    Case(
                        f"{app_type.value} matched available={available}",
                        app_type.value,
                        method,
                        matching,
                        is_available=available,
                    )
                )
                # The same request with the matched action denied by an admin:
                # a DENY must survive an unavailable credential.
                denied_id = next(
                    endpoint.id.value
                    for endpoint in catalog
                    for rule in endpoint.matches
                    if isinstance(rule, RestRoute)
                )
                cases.append(
                    Case(
                        f"{app_type.value} matched DENY override available={available}",
                        app_type.value,
                        method,
                        matching,
                        stored={denied_id: EndpointPolicy.DENY.value},
                        is_available=available,
                    )
                )
                cases.append(
                    Case(
                        f"{app_type.value} matched ALWAYS override available={available}",
                        app_type.value,
                        method,
                        matching,
                        stored={denied_id: EndpointPolicy.ALWAYS.value},
                        is_available=available,
                    )
                )
    return cases


def python_verdict(case: Case) -> Any:
    """The verdict the running proxy would reach, through the real matcher."""
    app_type = ExternalAppType(case.app_type)
    app = _StubApp(app_type)
    stored = {
        action_id: EndpointPolicy(value) for action_id, value in case.stored.items()
    }
    request = ProxiedRequest(
        method=case.method,
        # Mirrors the evaluator: catalog path matchers test the path only.
        path=case.path.split("?", 1)[0],
        body=case.raw_body(),
    )

    with mock.patch.object(
        engine_module, "get_action_policies", return_value=stored
    ) as patched:
        matched = recognize_actions(None, app, request)  # type: ignore[arg-type]
        patched.assert_called_once()
        assert patched.call_args.args[1] is GatedAppKind.EXTERNAL_APP

    gated = apply_credential_gate(
        app,  # type: ignore[arg-type]
        request,
        matched,
        is_available=case.is_available,
    )
    if gated is None:
        return None
    return {
        "actions": [
            {
                "action_type": action.action_type,
                "display_name": action.display_name,
                "description": action.description,
                "policy": action.policy.value,
            }
            for action in gated.actions
        ],
        "target": {
            "kind": gated.target.kind.value,
            "id": gated.target.id,
            "app_name": gated.target.app_name,
        },
        "payload": {},
    }


def rust_verdicts(cases: list[Case]) -> list[Any]:
    binary = _NATIVE_DIR / "target" / "debug" / "examples" / "match_corpus"
    if not binary.is_file():
        raise SystemExit(
            f"missing {binary}; build it with:\n"
            f"  cargo build --manifest-path {_NATIVE_DIR}/Cargo.toml "
            "-p lumen-sandbox-proxy --example match_corpus"
        )
    corpus = json.dumps({"cases": [case.as_json() for case in cases]})
    result = subprocess.run(  # noqa: S603
        [str(binary)],
        input=corpus,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def main() -> int:
    cases = build_corpus()
    print(f"corpus: {len(cases)} cases over {len(PROVIDERS)} providers")

    expected = [python_verdict(case) for case in cases]
    actual = rust_verdicts(cases)

    if len(expected) != len(actual):
        print(
            f"the Rust runner returned {len(actual)} verdicts for {len(expected)} cases"
        )
        return 1

    divergences: list[str] = []
    gated = 0
    for case, want, got in zip(cases, expected, actual, strict=True):
        if want is not None:
            gated += 1
        if want == got:
            continue
        divergences.append(
            f"{case.label}\n"
            f"    {case.method} {case.path}\n"
            f"    python: {json.dumps(want, sort_keys=True)}\n"
            f"    rust:   {json.dumps(got, sort_keys=True)}"
        )

    print(f"gated by Python: {gated}; forwarded ungated: {len(cases) - gated}")
    if divergences:
        print(f"\ndivergences: {len(divergences)}")
        for divergence in divergences:
            print(f"  - {divergence}")
        return 1
    print("the two matchers reached an identical verdict on every case")
    return 0


if __name__ == "__main__":
    sys.exit(main())
