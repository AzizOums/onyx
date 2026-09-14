LABEL_SANDBOX_ID = "lumen.app/sandbox-id"
LABEL_TENANT_ID = "lumen.app/tenant-id"
# Provisioning attempt that created the resource. Operator-facing orphan
# attribution only (kubectl/docker inspect) — never read programmatically;
# correctness comes from the attempt-number condition on sandbox status writes.
LABEL_PROVISIONING_ATTEMPT = "lumen.app/provisioning-attempt"
LABEL_K8S_COMPONENT = "app.kubernetes.io/component"
LABEL_K8S_COMPONENT_SANDBOX = "sandbox"
LABEL_K8S_MANAGED_BY = "app.kubernetes.io/managed-by"
LABEL_K8S_MANAGED_BY_LUMEN = "lumen"

# Docker-backend equivalents of the K8s component label. The proxy's
# DockerEventsLookup filters on these; ``docker_sandbox_manager`` stamps them
# onto every sandbox container it creates.
LABEL_DOCKER_COMPONENT = "lumen.app/component"
LABEL_DOCKER_COMPONENT_SANDBOX = "craft-sandbox"
