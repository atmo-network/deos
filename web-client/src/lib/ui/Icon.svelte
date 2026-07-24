<!--
Domain: UI Kit primitive
Owns: Canonical Lucide icon sizing, stroke defaults, and decorative accessibility semantics.
Excludes: Icon selection, action labeling, product meaning, and button behavior.
Zone: Foundation UI; wraps caller-provided icon components without product imports.
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type IconSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

  type Props = {
    icon: Component<any>;
    size?: IconSize;
    strokeWidth?: number;
    class?: ClassValue | null;
    ariaHidden?: boolean;
  };

  const pixels: Record<IconSize, number> = {
    xs: 12,
    sm: 16,
    md: 20,
    lg: 24,
    xl: 28,
  };

  let {
    icon,
    size = 'md',
    strokeWidth = 2,
    class: cls = '',
    ariaHidden = true,
  }: Props = $props();

  const Glyph = $derived(icon);
</script>

<Glyph
  size={pixels[size]}
  {strokeWidth}
  class={mergeClasses('shrink-0', cls)}
  aria-hidden={ariaHidden}
/>
