import { ValidSources } from "@/lib/types";
import { MinimalLumenDocument, LumenDocument } from "@/lib/search/interfaces";

// If we have a link, open it in a new tab (including if it's a file)
// If above fails and we have a file, update the presenting document
export const openDocument = (
  document: LumenDocument,
  updatePresentingDocument?: (document: MinimalLumenDocument) => void
) => {
  if (document.link) {
    window.open(document.link, "_blank");
  } else if (
    document.source_type === ValidSources.File ||
    document.source_type === ValidSources.UserFile
  ) {
    updatePresentingDocument?.(document);
  }
};
