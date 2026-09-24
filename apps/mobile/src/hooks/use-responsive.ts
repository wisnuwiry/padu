import { useMemo } from 'react';
import { useWindowDimensions } from 'react-native';

import { Breakpoints } from '@/constants/layout';

export type SizeClass = 'compact' | 'regular' | 'wide';

/**
 * Adaptive size class driven by window width only.
 *
 * `useWindowDimensions()` tracks the live window, so a fold unfolding, an
 * iPad entering Split View, or Android split-screen all re-lay-out through
 * this one hook. Never branch on device model, orientation, or hinge state.
 */
export function useResponsive(): {
  width: number;
  height: number;
  sizeClass: SizeClass;
  isWide: boolean;
} {
  const { width, height } = useWindowDimensions();
  return useMemo(() => {
    const sizeClass: SizeClass = width >= Breakpoints.wide
      ? 'wide'
      : width >= Breakpoints.regular
        ? 'regular'
        : 'compact';
    return { width, height, sizeClass, isWide: sizeClass === 'wide' };
  }, [height, width]);
}
