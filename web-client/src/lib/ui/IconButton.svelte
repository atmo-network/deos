<!--
Domain: UI Kit primitive
Owns: Icon-only button accessibility label/title behavior on top of Button.
Excludes: Product command semantics and icon selection policy.
Zone: Foundation UI; depends only on UI Kit primitives and platform types.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ClassValue, HTMLButtonAttributes } from 'svelte/elements';

  import Button from './Button.svelte';

  type Props = Omit<HTMLButtonAttributes, 'class'> & {
    label: string;
    class?: ClassValue | null;
    children: Snippet;
  };

  let { label, children, class: cls = '', ...rest }: Props = $props();
</script>

<Button
  size="icon"
  variant="ghost"
  aria-label={label}
  title={label}
  class={cls}
  {...rest}
>
  {@render children()}
</Button>
