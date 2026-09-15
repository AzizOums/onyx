//! Turns a GraphQL HTTP body into the `(operation_type, root_field)` pairs the
//! matcher reasons about.
//!
//! Port of `backend/lumen/external_apps/matching/graphql_parsing.py`. Isolated
//! behind one function so the GraphQL library is the only thing this module
//! knows about.

use std::collections::{HashMap, HashSet};

use graphql_parser::query::{
    Definition, Document, FragmentDefinition, OperationDefinition, Selection, SelectionSet,
};

/// The `(operation_type, root_field)` pairs a GraphQL request body invokes.
///
/// Handles a single GraphQL POST body or a batched array of them. Returns an
/// empty list when the body is not JSON, is not a GraphQL request, or cannot be
/// parsed — such a request simply matches no GraphQL rule.
pub fn parse_invocations(body: Option<&[u8]>) -> Vec<(String, String)> {
    let Some(body) = body.filter(|bytes| !bytes.is_empty()) else {
        return Vec::new();
    };
    // Python's `json.loads` detects the body's encoding before parsing, so a
    // BOM-prefixed or UTF-16 body is real JSON to the gate. Decoding the same
    // way is what stops such a body evading a recognised action's policy.
    let Some(decoded) = super::json_bytes::decode(body) else {
        return Vec::new();
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&decoded) else {
        return Vec::new();
    };

    let documents: Vec<&serde_json::Value> = match &payload {
        serde_json::Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };

    let mut invocations = Vec::new();
    for document in documents {
        let Some(object) = document.as_object() else {
            continue;
        };
        let Some(query) = object.get("query").and_then(serde_json::Value::as_str) else {
            continue;
        };
        invocations.extend(operation_invocations(query));
    }
    invocations
}

/// `(operation_type, root_field)` for every root field of every operation in a
/// GraphQL document.
///
/// Fragment spreads and inline fragments at the top level are resolved, so a
/// field cannot be hidden from matching inside a fragment. Returns an empty
/// list on a syntax error.
fn operation_invocations(query: &str) -> Vec<(String, String)> {
    let Ok(document) = graphql_parser::parse_query::<&str>(query) else {
        return Vec::new();
    };
    let fragments = fragments_by_name(&document);

    let mut invocations = Vec::new();
    for definition in &document.definitions {
        let Definition::Operation(operation) = definition else {
            continue;
        };
        let (operation_type, selection_set) = match operation {
            // A shorthand document (`{ field }`) is a query, which is how
            // graphql-core reports it too.
            OperationDefinition::SelectionSet(set) => ("query", set),
            OperationDefinition::Query(query) => ("query", &query.selection_set),
            OperationDefinition::Mutation(mutation) => ("mutation", &mutation.selection_set),
            OperationDefinition::Subscription(subscription) => {
                ("subscription", &subscription.selection_set)
            }
        };
        for field in root_field_names(selection_set, &fragments, &HashSet::new()) {
            invocations.push((operation_type.to_string(), field));
        }
    }
    invocations
}

type Fragments<'a> = HashMap<&'a str, &'a FragmentDefinition<'a, &'a str>>;

fn fragments_by_name<'a>(document: &'a Document<'a, &'a str>) -> Fragments<'a> {
    document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            Definition::Fragment(fragment) => Some((fragment.name, fragment)),
            Definition::Operation(_) => None,
        })
        .collect()
}

/// Field names directly under `selection_set`, expanding inline fragments and
/// (cycle-guarded) named fragment spreads at this level.
fn root_field_names<'a>(
    selection_set: &SelectionSet<'a, &'a str>,
    fragments: &Fragments<'a>,
    seen: &HashSet<String>,
) -> Vec<String> {
    let mut names = Vec::new();
    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => names.push(field.name.to_string()),
            Selection::InlineFragment(inline) => {
                names.extend(root_field_names(&inline.selection_set, fragments, seen));
            }
            Selection::FragmentSpread(spread) => {
                let name = spread.fragment_name;
                let Some(fragment) = fragments.get(name) else {
                    continue;
                };
                if seen.contains(name) {
                    continue;
                }
                let mut deeper = seen.clone();
                deeper.insert(name.to_string());
                names.extend(root_field_names(
                    &fragment.selection_set,
                    fragments,
                    &deeper,
                ));
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocations(body: &str) -> Vec<(String, String)> {
        parse_invocations(Some(body.as_bytes()))
    }

    #[test]
    fn a_body_that_is_not_graphql_invokes_nothing() {
        assert!(parse_invocations(None).is_empty());
        assert!(parse_invocations(Some(b"")).is_empty());
        assert!(invocations("not json at all").is_empty());
        assert!(invocations(r#"{"no_query_key": 1}"#).is_empty());
        assert!(invocations(r#"{"query": 42}"#).is_empty());
        assert!(invocations(r#"{"query": "this is not graphql {{{"}"#).is_empty());
    }

    #[test]
    fn a_named_query_reports_its_root_fields() {
        assert_eq!(
            invocations(r#"{"query": "query Me { viewer { id } issues { id } }"}"#),
            vec![
                ("query".into(), "viewer".into()),
                ("query".into(), "issues".into()),
            ]
        );
    }

    #[test]
    fn a_shorthand_document_is_a_query() {
        assert_eq!(
            invocations(r#"{"query": "{ viewer { id } }"}"#),
            vec![("query".into(), "viewer".into())]
        );
    }

    #[test]
    fn a_mutation_is_distinguished_from_a_query_on_the_same_field() {
        assert_eq!(
            invocations(r#"{"query": "mutation { issueCreate(input: {}) { id } }"}"#),
            vec![("mutation".into(), "issueCreate".into())]
        );
    }

    #[test]
    fn a_batched_post_reports_every_documents_fields() {
        assert_eq!(
            invocations(r#"[{"query": "{ a }"}, {"query": "mutation { b }"}]"#),
            vec![
                ("query".into(), "a".into()),
                ("mutation".into(), "b".into()),
            ]
        );
    }

    #[test]
    fn a_field_cannot_hide_from_matching_inside_a_fragment() {
        // The whole point of expanding spreads: `issueCreate` must still match.
        assert_eq!(
            invocations(
                r#"{"query": "mutation { ...Danger } fragment Danger on Mutation { issueCreate { id } }"}"#
            ),
            vec![("mutation".into(), "issueCreate".into())]
        );
    }

    #[test]
    fn an_inline_fragment_is_expanded_too() {
        assert_eq!(
            invocations(r#"{"query": "query { ... on Query { viewer { id } } }"}"#),
            vec![("query".into(), "viewer".into())]
        );
    }

    #[test]
    fn a_recursive_fragment_terminates_instead_of_hanging() {
        // A cycle is a syntactically valid document, so the guard is what stops
        // the matcher spinning on a hostile body.
        assert_eq!(
            invocations(
                r#"{"query": "query { ...A } fragment A on Query { x ...B } fragment B on Query { y ...A }"}"#
            ),
            vec![("query".into(), "x".into()), ("query".into(), "y".into())]
        );
    }

    #[test]
    fn a_spread_of_an_undefined_fragment_is_skipped() {
        assert_eq!(
            invocations(r#"{"query": "query { ...Missing viewer }"}"#),
            vec![("query".into(), "viewer".into())]
        );
    }
}
