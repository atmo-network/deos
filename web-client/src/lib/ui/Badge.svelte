<!--
Domain: UI Kit primitive
Owns: Compact status/category badge presentation.
Excludes: Product status derivation and domain-specific wording.
Zone: Foundation UI; accepts caller content without importing product slices.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = {
    variant?: 'tmc' | 'xyk' | 'info';
    class?: ClassValue | null;
    children: Snippet;
  };

  let { variant = 'info', children, class: cls = '' }: Props = $props();

  const variants = {
    tmc: 'bg-(--mono-green)/15 text-(--mono-green)',
    xyk: 'bg-(--mono-purple)/15 text-(--mono-purple)',
    info: 'bg-(--mono-border) text-white',
  };
</script>

<span
  class={mergeClasses(
    'text-xs px-2 py-0.5 rounded-full font-medium text-nowrap',
    variants[variant],
    cls,
  )}
>
  {@render children()}
</span>
