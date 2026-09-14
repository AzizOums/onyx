import { ReactNode } from "react";
import { DOCS_BASE_URL, HAS_DOCS } from "@/lib/constants";

/**
 * Absolute URL of a documentation page, or null when this deployment has no
 * documentation site configured (see NEXT_PUBLIC_DOCS_BASE_URL).
 */
export function docsHref(path: string = ""): string | null {
  return HAS_DOCS ? `${DOCS_BASE_URL}${path}` : null;
}

/**
 * Rich-text chunk renderer for an inline documentation link. Renders the words
 * as plain text when no documentation site is configured, so the sentence still
 * reads correctly instead of pointing at a dead host.
 */
export function docsChunk(
  path: string = "",
  className?: string
): (chunks: ReactNode) => ReactNode {
  return function renderDocsChunk(chunks: ReactNode): ReactNode {
    const href = docsHref(path);
    if (!href) return <>{chunks}</>;
    return (
      <a
        className={className}
        href={href}
        target="_blank"
        rel="noopener noreferrer"
      >
        {chunks}
      </a>
    );
  };
}
