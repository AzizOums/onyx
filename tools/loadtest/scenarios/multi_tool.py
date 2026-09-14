"""Chat turn that calls several retrieval tools in parallel (the `-tools3`
knob), exercising concurrent tool execution within one turn. Degrades to a
single search if the persona offers fewer tools. Part of the default mix.
"""

from __future__ import annotations

import os

from lumen_client.chat_user import LumenChatUser


class MultiToolUser(LumenChatUser):
    abstract = False
    weight = 8

    scenario_prefix: str = "multitool"
    mock_model: str | None = os.environ.get("LUMEN_MULTITOOL_MODEL", "mock-tools3")
