terraform {
  required_providers {
    lumen = {
      source = "lumen-dot-app/lumen"
    }
  }
}

variable "lumen_api_key" {
  type      = string
  sensitive = true
}

# Credentials can also come from LUMEN_SERVER_URL / LUMEN_API_KEY env vars.
provider "lumen" {
  endpoint = "https://lumen.internal.example.com"
  api_key  = var.lumen_api_key # an API key in the Admin group ("on_...")
}
