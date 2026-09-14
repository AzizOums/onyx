import SvgLumenLogo from "@opal/logos/lumen-logo";
import { cn } from "@opal/utils";

interface LumenLogoTypedProps {
  size?: number;
  className?: string;
}

// # NOTE(@raunakab):
// This ratio is not some random, magical number; it is available on Figma.
const HEIGHT_TO_GAP_RATIO = 5 / 16;

// The wordmark is set in the product font rather than drawn as vector paths, so
// renaming the product is a one-line change and no lettering can fall out of
// sync with the brand. The mark itself is unchanged.
const HEIGHT_TO_FONT_SIZE_RATIO = 13 / 16;
const WORDMARK = "Lumen";

const SvgLumenLogoTyped = ({ size: height, className }: LumenLogoTypedProps) => {
  const gap = height != null ? height * HEIGHT_TO_GAP_RATIO : undefined;
  const fontSize =
    height != null ? height * HEIGHT_TO_FONT_SIZE_RATIO : undefined;

  return (
    <div className={cn(`flex flex-row items-center`, className)} style={{ gap }}>
      <SvgLumenLogo size={height} />
      <span
        className="font-semibold tracking-tight leading-none text-text-05"
        style={{ fontSize }}
      >
        {WORDMARK}
      </span>
    </div>
  );
};
export default SvgLumenLogoTyped;
