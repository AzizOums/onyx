/**
 * Search API Helper Functions
 */

import type { BaseFilters, SearchDocWithContent } from "@/lib/search/interfaces";

interface SearchFullResponse {
  search_docs: SearchDocWithContent[];
}

/**
 * Perform a document search against the Community Edition `/api/search`
 * endpoint and map the results into the `SearchDocWithContent` shape used by
 * the UI.
 */
export async function searchDocuments(
  query: string,
  options?: {
    filters?: BaseFilters;
    numHits?: number;
    includeContent?: boolean;
    signal?: AbortSignal;
  }
): Promise<SearchFullResponse> {
  const response = await fetch("/api/search", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      query,
      sources: (options?.filters as { source_type?: string[] } | undefined)
        ?.source_type,
    }),
    signal: options?.signal,
  });

  if (!response.ok) {
    throw new Error(`Search failed: ${response.statusText}`);
  }

  const data = (await response.json()) as {
    results: {
      citation_id: number | null;
      title: string;
      content: string;
      link: string | null;
      source_type: string;
      updated_at: string | null;
    }[];
  };

  const search_docs: SearchDocWithContent[] = data.results.map((result) => ({
    document_id: String(result.citation_id ?? result.title),
    chunk_ind: 0,
    semantic_identifier: result.title,
    link: result.link,
    blurb: result.content.slice(0, 200),
    source_type: result.source_type as SearchDocWithContent["source_type"],
    boost: 0,
    hidden: false,
    metadata: {},
    score: null,
    match_highlights: [],
    updated_at: result.updated_at,
    is_internet: false,
    content: options?.includeContent === false ? null : result.content,
  }));

  return { search_docs };
}
