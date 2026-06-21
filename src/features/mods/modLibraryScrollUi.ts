type ScrollMetrics = {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
};

type ScrollUiState = {
  isScrollable: boolean;
  isAtTop: boolean;
  showScrollUi: boolean;
  thumbStyle: {
    height: string;
    transform: string;
  };
};

const SCROLL_TOP_EPSILON = 1;
const MIN_THUMB_HEIGHT = 36;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function formatPixels(value: number) {
  const rounded = Math.round(value * 10) / 10;
  return `${Number.isInteger(rounded) ? rounded.toFixed(0) : rounded.toFixed(1)}px`;
}

export function getModLibraryScrollUiState({
  scrollTop,
  scrollHeight,
  clientHeight,
}: ScrollMetrics): ScrollUiState {
  const maxScrollTop = Math.max(0, scrollHeight - clientHeight);
  const isScrollable = maxScrollTop > SCROLL_TOP_EPSILON;
  const normalizedScrollTop = clamp(scrollTop, 0, maxScrollTop);
  const isAtTop = normalizedScrollTop <= SCROLL_TOP_EPSILON;

  if (!isScrollable || clientHeight <= 0 || scrollHeight <= 0) {
    return {
      isScrollable: false,
      isAtTop: true,
      showScrollUi: false,
      thumbStyle: {
        height: "0px",
        transform: "translateY(0px)",
      },
    };
  }

  const thumbHeight = clamp((clientHeight / scrollHeight) * clientHeight, MIN_THUMB_HEIGHT, clientHeight);
  const maxThumbTop = Math.max(0, clientHeight - thumbHeight);
  const thumbTop = maxScrollTop === 0 ? 0 : (normalizedScrollTop / maxScrollTop) * maxThumbTop;

  return {
    isScrollable,
    isAtTop,
    showScrollUi: !isAtTop,
    thumbStyle: {
      height: formatPixels(thumbHeight),
      transform: `translateY(${formatPixels(thumbTop)})`,
    },
  };
}
