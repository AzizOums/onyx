data "lumen_llm_providers" "all" {}

output "default_model" {
  value = data.lumen_llm_providers.all.default_text
}
