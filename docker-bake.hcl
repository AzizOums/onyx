group "default" {
  targets = ["backend", "model-server", "web"]
}

variable "BACKEND_REPOSITORY" {
  default = "lumendotapp/lumen-backend"
}

variable "WEB_SERVER_REPOSITORY" {
  default = "lumendotapp/lumen-web-server"
}

variable "MODEL_SERVER_REPOSITORY" {
  default = "lumendotapp/lumen-model-server"
}

variable "CLI_REPOSITORY" {
  default = "lumendotapp/lumen-cli"
}

variable "MCP_SERVER_REPOSITORY" {
  default = "lumendotapp/lumen-mcp-server"
}

variable "DEVCONTAINER_REPOSITORY" {
  default = "lumendotapp/lumen-devcontainer"
}

variable "TAG" {
  default = "latest"
}

# Registry prefix for base images (the Dockerfiles' BASE_IMAGE_REGISTRY ARG). Defaults to
# Docker Hub; CI overrides this (via the matching environment variable, which bake reads
# automatically) to the ECR pull-through cache so base-image pulls avoid Docker Hub rate limits.
variable "BASE_IMAGE_REGISTRY" {
  default = "docker.io"
}

target "backend" {
  context    = "backend"
  dockerfile = "Dockerfile"

  cache-from = [
    "type=registry,ref=${BACKEND_REPOSITORY}:latest",
    "type=registry,ref=${BACKEND_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${BACKEND_REPOSITORY}:${TAG}"]
}

target "web" {
  context    = "web"
  dockerfile = "Dockerfile"

  cache-from = [
    "type=registry,ref=${WEB_SERVER_REPOSITORY}:latest",
    "type=registry,ref=${WEB_SERVER_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${WEB_SERVER_REPOSITORY}:${TAG}"]
}

target "model-server" {
  context = "backend"

  dockerfile = "Dockerfile.model_server"

  cache-from = [
    "type=registry,ref=${MODEL_SERVER_REPOSITORY}:latest",
    "type=registry,ref=${MODEL_SERVER_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${MODEL_SERVER_REPOSITORY}:${TAG}"]
}

target "cli" {
  context    = "cli"
  dockerfile = "Dockerfile"

  cache-from = [
    "type=registry,ref=${CLI_REPOSITORY}:latest",
    "type=registry,ref=${CLI_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${CLI_REPOSITORY}:${TAG}"]
}

# The Rust MCP server. Not in the default group: the Python MCP server still
# ships inside the backend image, and this one is opt-in until a deployment has
# accepted the parity evidence (backend/native/lumen-mcp-server/README.md).
target "mcp-server" {
  context    = "backend/native"
  dockerfile = "lumen-mcp-server/Dockerfile"

  cache-from = [
    "type=registry,ref=${MCP_SERVER_REPOSITORY}:latest",
    "type=registry,ref=${MCP_SERVER_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${MCP_SERVER_REPOSITORY}:${TAG}"]
}

target "devcontainer" {
  context    = ".devcontainer"
  dockerfile = "Dockerfile"

  cache-from = [
    "type=registry,ref=${DEVCONTAINER_REPOSITORY}:latest",
    "type=registry,ref=${DEVCONTAINER_REPOSITORY}:edge",
  ]
  cache-to   = ["type=inline"]

  args = {
    BASE_IMAGE_REGISTRY = "${BASE_IMAGE_REGISTRY}"
  }

  tags      = ["${DEVCONTAINER_REPOSITORY}:${TAG}"]
}
