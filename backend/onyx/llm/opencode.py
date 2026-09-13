"""OpenCode Zen gateway helpers.

The Zen free tier only serves clients that identify as OpenCode: every
request must carry the official client's headers, including a session id
(`MissingSessionID` otherwise). Onyx derives a stable session id per scope
(one Onyx chat session maps to one Zen session: affinity plus per-session
quotas) and mints a fresh request id per LLM build (one build per turn).
This mirrors `packages/opencode/src/session/llm/request.ts` for
`opencode/*` providers.
"""

import hashlib
import secrets

OPENCODE_PROVIDER_NAME = "opencode"
OPENCODE_API_BASE = "https://opencode.ai/zen/v1"
# Anonymous bearer for the keyless free tier. A blank placeholder (" ")
# produces an illegal `Authorization` header that httpx refuses to send.
OPENCODE_PUBLIC_API_KEY = "public"
# Mirrors the official client's User-Agent so the free-tier gate lets
# requests through. Keep in sync with a released opencode version.
OPENCODE_USER_AGENT = "opencode/1.18.18"
OPENCODE_CLIENT = "cli"

SESSION_ID_PREFIX = "ses_"
REQUEST_ID_PREFIX = "msg_"


def is_opencode_gateway(provider: str | None, api_base: str | None) -> bool:
    """True for the native `opencode` provider and for generic
    openai-compatible rows pointed at the Zen gateway.

    Existing rows created before the native provider (e.g. an
    `openai_compatible` provider with the Zen base URL) get the same
    client identity, session handling, and video wire format.
    """
    if provider == OPENCODE_PROVIDER_NAME:
        return True
    return bool(api_base) and "opencode.ai/zen" in api_base


# Xiaomi MiMo video limits, enforced client-side so an oversized or
# unsupported upload degrades to a text marker instead of an upstream 400.
OPENCODE_SUPPORTED_VIDEO_MIMES = frozenset(
    {
        "video/mp4",
        "video/quicktime",
        "video/avi",
        "video/x-ms-wmv",
    }
)
OPENCODE_MAX_VIDEO_BASE64_CHARS = 50_000_000


def opencode_session_id(scope: str) -> str:
    """Stable `ses_<32 hex>` id for a scope (e.g. a chat session id).

    Deterministic so every worker and every turn of the same scope lands
    on the same upstream session.
    """
    digest = hashlib.sha256(f"onyx-opencode-session:{scope}".encode()).hexdigest()
    return f"{SESSION_ID_PREFIX}{digest[:32]}"


def opencode_request_id() -> str:
    """Fresh `msg_<24 hex>` id, one per LLM build (i.e. per turn)."""
    return f"{REQUEST_ID_PREFIX}{secrets.token_hex(12)}"


def opencode_request_headers(scope: str | None = None) -> dict[str, str]:
    """Headers identifying the request as an OpenCode client call.

    Without a scope the session id is ephemeral (a fresh upstream session
    per call); pass a stable scope for session affinity.
    """
    raw_scope = scope if scope is not None else secrets.token_hex(16)
    return {
        "User-Agent": OPENCODE_USER_AGENT,
        "x-opencode-client": OPENCODE_CLIENT,
        "x-opencode-session": opencode_session_id(raw_scope),
        "x-opencode-request": opencode_request_id(),
    }
