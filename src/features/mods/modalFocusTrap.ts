type TrappedFocusIndexInput = {
  currentIndex: number;
  focusableCount: number;
  backwards: boolean;
};

export function getTrappedFocusIndex({ currentIndex, focusableCount, backwards }: TrappedFocusIndexInput) {
  if (focusableCount <= 0) {
    return -1;
  }

  if (currentIndex < 0) {
    return backwards ? focusableCount - 1 : 0;
  }

  if (backwards && currentIndex === 0) {
    return focusableCount - 1;
  }

  if (!backwards && currentIndex === focusableCount - 1) {
    return 0;
  }

  return null;
}
