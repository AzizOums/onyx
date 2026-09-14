# A day-one Lumen configuration: a chat model, one indexed source, a document
# set built from it, and an agent that answers from that set.
#
# Everything here works on Community Edition. The user group at the bottom is
# the one exception and stays off unless you enable it.

terraform {
  required_version = ">= 1.5"
  required_providers {
    lumen = {
      source = "lumen-dot-app/lumen"
    }
  }
}

# Reads LUMEN_SERVER_URL and LUMEN_API_KEY when the variables are unset.
provider "lumen" {
  endpoint = var.lumen_server_url
  api_key  = var.lumen_api_key
}

# ---------------------------------------------------------------------------
# Workspace
# ---------------------------------------------------------------------------

# Only the attributes set here are managed. Destroying this resource does not
# reset the settings it wrote.
resource "lumen_settings" "workspace" {
  company_name = var.company_name
}

# ---------------------------------------------------------------------------
# Chat model
# ---------------------------------------------------------------------------

resource "lumen_llm_provider" "openai" {
  name          = "openai"
  provider_type = "openai"
  api_key       = var.openai_api_key

  # The complete set of enabled models: anything omitted is removed on apply.
  model_configurations = [
    { name = "gpt-5" },
    { name = "gpt-5-mini" },
  ]
}

# Referencing the provider id also orders destroys correctly: the default is
# released before the provider holding it is deleted.
resource "lumen_llm_provider_default" "this" {
  provider_id = lumen_llm_provider.openai.id
  model_name  = "gpt-5"
}

# ---------------------------------------------------------------------------
# Knowledge
# ---------------------------------------------------------------------------

# The web connector reads public pages, so its credential holds no secret.
resource "lumen_credential" "web" {
  source          = "web"
  name            = "public-web"
  credential_json = jsonencode({})
}

resource "lumen_connector" "docs" {
  name       = "docs-site"
  source     = "web"
  input_type = "load_state"

  # Re-index once a day.
  refresh_freq = 24 * 60 * 60

  connector_specific_config = jsonencode({
    base_url           = var.docs_base_url
    web_connector_type = "recursive"
  })
}

# The pair is the object that indexes. It also carries the access control for
# the documents it produces.
resource "lumen_cc_pair" "docs" {
  name          = "docs-site"
  connector_id  = lumen_connector.docs.id
  credential_id = lumen_credential.web.id
  access_type   = "public"
}

resource "lumen_document_set" "docs" {
  name        = "docs"
  description = "Public product documentation"
  cc_pair_ids = [lumen_cc_pair.docs.id]
}

# ---------------------------------------------------------------------------
# Agent
# ---------------------------------------------------------------------------

resource "lumen_persona" "docs" {
  name        = "Docs"
  description = "Answers product questions from the documentation"

  system_prompt = <<-EOT
    You answer questions from the product documentation.
    If the documentation does not cover the question, say so.
  EOT

  document_set_ids = [lumen_document_set.docs.id]

  starter_messages = [
    {
      name    = "Getting started"
      message = "How do I get started?"
    },
  ]
}

# ---------------------------------------------------------------------------
# Access control (Enterprise Edition)
# ---------------------------------------------------------------------------

# User groups need Enterprise Edition. Leave enable_enterprise_features off on
# Community Edition: Lumen rejects these routes there.
resource "lumen_user_group" "platform" {
  count = var.enable_enterprise_features ? 1 : 0

  name = "Platform"

  # Permissions use Lumen's own tokens, not the enum names.
  permissions = [
    "manage:connectors",
    "manage:document_sets",
  ]
}
