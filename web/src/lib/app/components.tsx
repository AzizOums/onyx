"use client";

import { useTranslations } from "next-intl";
import { useSettings } from "@/lib/settings/hooks";
import {
  DEFAULT_LOGO_SIZE_PX,
  NEXT_PUBLIC_DO_NOT_USE_TOGGLE_OFF_DANSWER_POWERED,
} from "@/lib/constants";
import { cn } from "@opal/utils";
import Text from "@/refresh-components/texts/Text";
import Truncated from "@/refresh-components/texts/Truncated";
import { SvgLumenLogo, SvgLumenLogoTyped } from "@opal/logos";

export interface LogoProps {
  folded?: boolean;
  size?: number;
  className?: string;
  // Always render the real Lumen logo, ignoring enterprise white-label settings
  // (custom logo / application name). Used by Lumen-branded surfaces like Craft.
  lumenBranded?: boolean;
}

export function Logo({ folded, size, className, lumenBranded }: LogoProps) {
  const t = useTranslations("common");
  const resolvedSize = size ?? DEFAULT_LOGO_SIZE_PX;
  const { enterprise, logoUrl } = useSettings();
  const logoDisplayStyle = enterprise?.logo_display_style;
  const applicationName = enterprise?.application_name;

  if (lumenBranded) {
    return folded ? (
      <SvgLumenLogo size={resolvedSize} className={cn("shrink-0", className)} />
    ) : (
      <SvgLumenLogoTyped size={resolvedSize} className={className} />
    );
  }

  const logo = logoUrl ? (
    <div
      className={cn(
        "aspect-square rounded-full overflow-hidden relative shrink-0",
        className
      )}
      style={{ height: resolvedSize }}
    >
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        alt={t("logo.image.alt")}
        src={logoUrl}
        className="object-cover object-center w-full h-full"
      />
    </div>
  ) : (
    <SvgLumenLogo size={resolvedSize} className={cn("shrink-0", className)} />
  );

  const renderNameAndPoweredBy = (opts: {
    includeLogo: boolean;
    includeName: boolean;
  }) => {
    return (
      <div className="flex min-w-0 gap-2">
        {opts.includeLogo && logo}
        {!folded && (
          /* H3 text is 4px larger (28px) than the Logo icon (24px), so negative margin hack. */
          <div className="flex flex-1 flex-col -mt-0.5">
            {opts.includeName && (
              <Truncated headingH3>{applicationName}</Truncated>
            )}
            {!NEXT_PUBLIC_DO_NOT_USE_TOGGLE_OFF_DANSWER_POWERED &&
              !enterprise?.hide_lumen_branding && (
                <Text
                  secondaryBody
                  text03
                  className={"line-clamp-1 truncate"}
                  nowrap
                >
                  {t("logo.poweredBy.label")}
                </Text>
              )}
          </div>
        )}
      </div>
    );
  };

  // Handle "logo_only" display style
  if (logoDisplayStyle === "logo_only") {
    return renderNameAndPoweredBy({ includeLogo: true, includeName: false });
  }

  // Handle "name_only" display style
  if (logoDisplayStyle === "name_only") {
    return renderNameAndPoweredBy({ includeLogo: false, includeName: true });
  }

  // Handle "logo_and_name" or default behavior
  return applicationName ? (
    renderNameAndPoweredBy({ includeLogo: true, includeName: true })
  ) : folded ? (
    <SvgLumenLogo size={resolvedSize} className={cn("shrink-0", className)} />
  ) : (
    <SvgLumenLogoTyped size={resolvedSize} className={className} />
  );
}
