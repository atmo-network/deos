<!--
Domain: UI Kit primitive
Owns: Accessible progressive-disclosure section structure, summary affordance, and shared spacing.
Excludes: Product disclosure policy, domain state, and content ordering.
Zone: Foundation UI; composes native details semantics and Lucide without product imports.
-->
<script lang="ts">
  import { ChevronDown } from '@lucide/svelte';
  import type { Snippet } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import Icon from './Icon.svelte';
  import { mergeClasses } from './class';

  type Props = {
    title: string;
    open?: boolean;
    class?: ClassValue | null;
    bodyClass?: ClassValue | null;
    children: Snippet;
  };

  let {
    title,
    open = false,
    class: cls = '',
    bodyClass = '',
    children,
  }: Props = $props();
</script>

<details class={mergeClasses('group rounded-xl bg-white p-3', cls)} {open}>
  <summary
    class="flex cursor-pointer list-none items-center justify-between gap-2"
  >
    <span
      class="text-[10px] font-semibold uppercase tracking-wider text-(--mono-muted)"
    >
      {title}
    </span>
    <Icon
      icon={ChevronDown}
      size="sm"
      class="text-(--mono-muted) transition-transform group-open:rotate-180"
    />
  </summary>
  <div class={mergeClasses('mt-3 grid gap-3', bodyClass)}>
    {@render children()}
  </div>
</details>
