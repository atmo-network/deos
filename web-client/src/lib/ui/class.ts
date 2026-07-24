/*
Domain: UI Kit class helpers
Owns: Local class-value flattening/merging for UI Kit primitives that accept Svelte-style class values.
Excludes: Styling tokens, product/domain class policy, Tailwind configuration, and component-specific variants.
Zone: Foundation UI helper; safe for UI Kit primitives and presentation-only components to import.
*/
import type { ClassValue } from 'svelte/elements';
import { twMerge } from 'tailwind-merge';

export function flattenClass(
  value: ClassValue | null | undefined | false,
): string {
  if (!value) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (Array.isArray(value)) {
    const parts: string[] = [];
    for (const item of value) {
      const flattened = flattenClass(item);
      if (flattened) {
        parts.push(flattened);
      }
    }
    return parts.join(' ');
  }
  if (typeof value === 'object') {
    return Object.entries(value)
      .filter(([, enabled]) => Boolean(enabled))
      .map(([className]) => className)
      .join(' ');
  }
  return '';
}

export function mergeClasses(
  ...values: Array<ClassValue | null | undefined | false>
): string {
  return twMerge(values.map(flattenClass).filter(Boolean).join(' '));
}
