output "agent_id" {
  description = "Id of the documentation agent."
  value       = lumen_persona.docs.id
}

output "cc_pair_id" {
  description = "Id of the indexing connector-credential pair."
  value       = lumen_cc_pair.docs.id
}
