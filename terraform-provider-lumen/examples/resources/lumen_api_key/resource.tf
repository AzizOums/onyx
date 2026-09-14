variable "ingest_group_id" {
  type        = number
  description = "Id of an existing Lumen user group; ids are assigned per deployment."
}

# A key for an internal integration. The key material is in
# lumen_api_key.ingest.api_key (sensitive, state-only).
resource "lumen_api_key" "ingest" {
  name      = "ingest-pipeline"
  group_ids = [var.ingest_group_id]
}
