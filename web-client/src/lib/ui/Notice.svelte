<!--
Domain: UI Kit primitive
Owns: Inline notice presentation for muted, warning, and success messages.
Excludes: Error source ownership, domain-specific remediation, and logging.
Zone: Foundation UI; accepts caller content without importing product slices.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';
  import type { NoticeVariant } from './notice-contract';

  type Props = {
    variant?: NoticeVariant;
    class?: ClassValue | null;
    children: Snippet;
  };

  let { variant = 'muted', class: cls = '', children }: Props = $props();

  const variants = {
    muted: 'bg-white text-(--mono-muted)',
    warn: 'border border-(--mono-orange)/30 bg-(--mono-orange)/10 text-(--mono-orange)',
    success:
      'border border-(--mono-green)/30 bg-(--mono-green)/10 text-(--mono-green)',
  };
</script>

<div
  class={mergeClasses(
    'rounded-lg px-2 py-1 text-[10px] leading-4',
    variants[variant],
    cls,
  )}
>
  {@render children()}
</div>
