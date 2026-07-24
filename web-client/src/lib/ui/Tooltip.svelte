<!--
Domain: UI Kit primitive
Owns: Project tooltip trigger, floating content, and shared tooltip styling around Bits UI.
Excludes: Essential instructions, mobile-only disclosure, domain policy, and interactive tooltip content.
Zone: Foundation UI; may wrap Bits UI but must not import product slices.
-->
<script lang="ts">
  import { Tooltip as BitsTooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<BitsTooltip.TriggerProps, 'children' | 'class'> & {
    children?: Snippet;
    content: Snippet;
    class?: ClassValue | null;
    contentClass?: ClassValue | null;
    side?: 'top' | 'right' | 'bottom' | 'left';
    align?: 'start' | 'center' | 'end';
    sideOffset?: number;
    delayDuration?: number;
  };

  let {
    child,
    children,
    content,
    class: cls = '',
    contentClass = '',
    side = 'top',
    align = 'center',
    sideOffset = 7,
    delayDuration,
    type = 'button',
    ...rest
  }: Props = $props();
</script>

<BitsTooltip.Root {delayDuration} disableHoverableContent>
  <BitsTooltip.Trigger
    {type}
    {child}
    class={mergeClasses('cursor-help', cls)}
    {...rest}
  >
    {#if children}
      {@render children()}
    {/if}
  </BitsTooltip.Trigger>
  <BitsTooltip.Portal>
    <BitsTooltip.Content
      {side}
      {align}
      {sideOffset}
      class={mergeClasses(
        'z-50 max-w-72 rounded-xl border border-(--mono-border) bg-white px-3 py-2 text-left text-[11px] leading-relaxed text-(--mono-text) shadow-[0_8px_24px_rgba(44,50,30,0.1)]',
        contentClass,
      )}
    >
      {@render content()}
      <BitsTooltip.Arrow />
      <BitsTooltip.Arrow
        class="text-white data-[side=top]:-translate-y-0.75 data-[side=bottom]:translate-y-0.75 data-[side=left]:-translate-x-0.75 data-[side=right]:translate-x-0.75"
      />
    </BitsTooltip.Content>
  </BitsTooltip.Portal>
</BitsTooltip.Root>
