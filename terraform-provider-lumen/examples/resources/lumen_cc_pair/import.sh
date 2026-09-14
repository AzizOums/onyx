#!/bin/sh
# Import by numeric connector-credential pair id.
#
# Lumen does not report groups, auto_sync_options or processing_mode, so they
# are empty after an import. Setting them afterwards replaces the pair.
terraform import lumen_cc_pair.confluence_wiki 12
