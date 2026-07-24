<!--
Domain: UI Kit primitive
Owns: Standard in-widget backward navigation affordance and accessible icon-button sizing.
Excludes: Navigation stack state, route history, destination policy, and product copy.
Zone: Foundation UI; composes Button, Icon, and Lucide without importing product slices.
-->
<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import type { ClassValue, HTMLButtonAttributes } from 'svelte/elements';

  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import { mergeClasses } from './class';

  type Props = Omit<HTMLButtonAttributes, 'aria-label' | 'class' | 'title'> & {
    label: string;
    text?: string;
    class?: ClassValue | null;
  };

  let { label, text, class: cls = '', ...rest }: Props = $props();
</script>

<Button
  size={text ? 'sm' : 'icon'}
  variant="ghost"
  {label}
  class={mergeClasses(
    '-ml-2 inline-flex h-9 shrink-0 items-center rounded-lg',
    text ? 'gap-2 px-2.5' : 'w-9',
    cls,
  )}
  {...rest}
>
  <Icon icon={ArrowLeft} strokeWidth={2.25} />
  {#if text}
    <span>{text}</span>
  {/if}
</Button>
