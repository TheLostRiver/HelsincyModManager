import { ArrowUp } from "lucide-react";

type BackToTopButtonProps = {
  onClick: () => void;
};

export function BackToTopButton({ onClick }: BackToTopButtonProps) {
  return (
    <button
      type="button"
      className="mod-library__back-to-top"
      aria-label="返回顶部"
      title="返回顶部"
      onClick={onClick}
    >
      <ArrowUp size={20} strokeWidth={2.5} aria-hidden="true" />
    </button>
  );
}
