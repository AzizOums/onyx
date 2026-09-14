#!/bin/sh
# Import by numeric credential id. Lumen only returns the payload masked, so
# credential_json stays at its configured value and is never refreshed.
terraform import lumen_credential.confluence 12
