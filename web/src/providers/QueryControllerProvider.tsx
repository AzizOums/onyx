"use client";

import { createContext, useContext } from "react";
import { SearchDocWithContent, BaseFilters } from "@/lib/search/interfaces";

export type AppMode = "auto" | "search" | "chat";

export type QueryState =
  | { phase: "idle"; appMode: AppMode }
  | { phase: "classifying" }
  | { phase: "searching" }
  | { phase: "search-results" }
  | { phase: "chat" };

export interface QueryControllerValue {
  /** Single state variable encoding both the query lifecycle phase and (when idle) the user's mode selection. */
  state: QueryState;
  /** Update the app mode. Only takes effect when idle. No-op in CE or when search is unavailable. */
  setAppMode: (mode: AppMode) => void;
  /** Search results (empty if chat or not yet searched) */
  searchResults: SearchDocWithContent[];
  /** Document IDs selected by the LLM as most relevant */
  llmSelectedDocIds: string[] | null;
  /** User-facing error message from the last search or classification request, null when idle */
  error: string | null;
  /** Submit a query - routes to search or chat based on app mode */
  submit: (
    query: string,
    onChat: (query: string) => void,
    filters?: BaseFilters
  ) => Promise<void>;
  /** Re-run the current search query with updated server-side filters */
  refineSearch: (filters: BaseFilters) => Promise<void>;
  /** Reset all state to initial values */
  reset: () => void;
}

export const QueryControllerContext = createContext<QueryControllerValue>({
  state: { phase: "idle", appMode: "chat" },
  setAppMode: () => undefined,
  searchResults: [],
  llmSelectedDocIds: null,
  error: null,
  submit: async (_q, onChat) => {
    onChat(_q);
  },
  refineSearch: async () => undefined,
  reset: () => undefined,
});

export function useQueryController(): QueryControllerValue {
  return useContext(QueryControllerContext);
}

// Community build: the paid-tier search controller is removed, so this
// provider is a passthrough and consumers use the context defaults
// (chat-only mode).
export function QueryControllerProvider({
  children,
}: {
  children?: React.ReactNode;
}) {
  return <>{children}</>;
}
