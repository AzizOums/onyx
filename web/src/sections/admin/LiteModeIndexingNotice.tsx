import { useTranslations } from "next-intl";
import { IllustrationContent } from "@opal/layouts";
import { Section } from "@/layouts/general-layouts";
import SvgUnPlugged from "@opal/illustrations/un-plugged";
import { markdown } from "@opal/utils";
import { docsHref } from "@/lib/docs";

/**
 * Replaces connector/indexing admin pages in Lite mode (no vector DB), where
 * indexing can't run — points users at a Standard-mode deployment instead.
 */
export default function LiteModeIndexingNotice() {
  const t = useTranslations("admin.shared");
  const docsUrl = docsHref("/deployment/getting_started/quickstart");

  return (
    <Section padding={8}>
      <IllustrationContent
        illustration={SvgUnPlugged}
        title={t("liteModeNotice.title")}
        description={markdown(
          docsUrl
            ? t("liteModeNotice.description", { docsUrl })
            : t("liteModeNotice.descriptionNoDocs")
        )}
      />
    </Section>
  );
}
