data "lumen_settings" "current" {}

output "license_tier" {
  value = data.lumen_settings.current.tier
}
