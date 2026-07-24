/*
Domain: Tab animation helper
Owns: Custom tab-strip flip animation timing and fallback transform behavior.
Excludes: Tab ordering, drag/drop policy, and visual tab markup.
Zone: Layout animation helper; depends on Svelte animation/easing primitives only.
*/
import { flip } from 'svelte/animate';
import { elasticOut } from 'svelte/easing';

export type FlipAnimation = {
  from: DOMRect;
  to: DOMRect;
};

export type TabFlipAnimate = (
  node: Element,
  animation: FlipAnimation,
) => ReturnType<typeof flip>;

type FlipControllerOptions = {
  flipDurationMs: number;
  suppressionMs: number;
};

let layoutResizeFlipSuppressedUntil = 0;

export function suppressTabFlipForLayoutResize(durationMs = 160): void {
  layoutResizeFlipSuppressedUntil = Math.max(
    layoutResizeFlipSuppressedUntil,
    Date.now() + durationMs,
  );
}

export function createTabFlipController(options: FlipControllerOptions) {
  const { flipDurationMs, suppressionMs } = options;
  let flipSuppressedByResize = false;
  let lastTabBarWidth = 0;
  let lastTabBarHeight = 0;
  let lastContainerWidth = 0;
  let lastContainerHeight = 0;
  let resizeSuppressionTimer: ReturnType<typeof setTimeout> | null = null;

  function suppressFlipDuringResize(): void {
    flipSuppressedByResize = true;
    if (resizeSuppressionTimer !== null) {
      clearTimeout(resizeSuppressionTimer);
    }
    resizeSuppressionTimer = setTimeout(() => {
      flipSuppressedByResize = false;
      resizeSuppressionTimer = null;
    }, suppressionMs);
  }

  function animate(node: Element, animation: FlipAnimation) {
    if (
      flipSuppressedByResize ||
      Date.now() < layoutResizeFlipSuppressedUntil
    ) {
      return flip(node, animation, { duration: 0 });
    }
    return flip(node, animation, {
      duration: flipDurationMs,
      easing: elasticOut,
    });
  }

  function observe(
    tabBarEl: HTMLDivElement,
    containerEl: HTMLDivElement,
  ): () => void {
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        if (entry.target === tabBarEl) {
          if (lastTabBarWidth === 0 && lastTabBarHeight === 0) {
            lastTabBarWidth = width;
            lastTabBarHeight = height;
            continue;
          }
          if (
            Math.abs(width - lastTabBarWidth) > 0.5 ||
            Math.abs(height - lastTabBarHeight) > 0.5
          ) {
            suppressFlipDuringResize();
          }
          lastTabBarWidth = width;
          lastTabBarHeight = height;
          continue;
        }
        if (entry.target === containerEl) {
          if (lastContainerWidth === 0 && lastContainerHeight === 0) {
            lastContainerWidth = width;
            lastContainerHeight = height;
            continue;
          }
          if (
            Math.abs(width - lastContainerWidth) > 0.5 ||
            Math.abs(height - lastContainerHeight) > 0.5
          ) {
            suppressFlipDuringResize();
          }
          lastContainerWidth = width;
          lastContainerHeight = height;
        }
      }
    });
    resizeObserver.observe(tabBarEl);
    resizeObserver.observe(containerEl);
    return () => {
      resizeObserver.disconnect();
      if (resizeSuppressionTimer !== null) {
        clearTimeout(resizeSuppressionTimer);
        resizeSuppressionTimer = null;
      }
      flipSuppressedByResize = false;
      lastTabBarWidth = 0;
      lastTabBarHeight = 0;
      lastContainerWidth = 0;
      lastContainerHeight = 0;
    };
  }

  return {
    animate,
    observe,
  };
}
