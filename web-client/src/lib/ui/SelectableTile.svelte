<!--
Domain: UI Kit primitive
Owns: Selectable card-button presentation and safe non-submit default.
Excludes: Selection state ownership and product-specific option content.
Zone: Foundation UI; accepts caller content without importing product slices.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ClassValue, HTMLButtonAttributes } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLButtonAttributes, 'class'> & {
    class?: ClassValue | null;
    selected?: boolean;
    children: Snippet;
  };

  let {
    selected = false,
    type = 'button',
    children,
    class: cls = '',
    ...rest
  }: Props = $props();
</script>

<button
  {type}
  class={mergeClasses(
    'rounded-xl border px-3 py-2 text-left transition-colors',
    selected
      ? 'border-(--mono-purple) bg-(--mono-bg)'
      : 'border-(--mono-border) bg-white hover:border-(--mono-purple)/40',
    cls,
  )}
  {...rest}
>
  {@render children()}
</button>
