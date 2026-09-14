"""Constants for release notes functionality."""

import os

# Changelog source. Empty by default: this build publishes no changelog feed of
# its own. Point RELEASE_NOTES_CHANGELOG_URL at a raw changelog.mdx to turn the
# release-notes notifications back on, and DOCS_CHANGELOG_BASE_URL at the page
# those notifications should link to.
GITHUB_RAW_BASE_URL = os.environ.get("RELEASE_NOTES_RAW_BASE_URL", "")
GITHUB_CHANGELOG_RAW_URL = os.environ.get(
    "RELEASE_NOTES_CHANGELOG_URL",
    f"{GITHUB_RAW_BASE_URL}/changelog.mdx" if GITHUB_RAW_BASE_URL else "",
)

# Base URL for changelog documentation (used for notification links)
DOCS_CHANGELOG_BASE_URL = os.environ.get("DOCS_CHANGELOG_BASE_URL", "")

FETCH_TIMEOUT = 60.0

# Redis keys (in shared namespace)
REDIS_KEY_PREFIX = "release_notes:"
REDIS_KEY_FETCHED_AT = f"{REDIS_KEY_PREFIX}fetched_at"
REDIS_KEY_ETAG = f"{REDIS_KEY_PREFIX}etag"

# Cache TTL: 24 hours
REDIS_CACHE_TTL = 60 * 60 * 24

# Auto-refresh threshold: 1 hour
AUTO_REFRESH_THRESHOLD_SECONDS = 60 * 60
