export const APP_BRAND_LOGO_SRC = "/branding/hmm-logo.png";

export function AppBrandMark({ className }: { className?: string }) {
  return (
    <img
      className={className}
      src={APP_BRAND_LOGO_SRC}
      alt=""
      aria-hidden="true"
      draggable="false"
    />
  );
}
