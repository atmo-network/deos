<!--
Domain: UI Kit primitive
Owns: Project Button wrapper, visual variants, sizes, and safe non-submit default.
Excludes: Product/domain actions, form policy, and widget-specific copy.
Zone: Foundation UI; may wrap Bits UI but must not import product slices.
-->
<script lang="ts">
  import { Button as BitsButton } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLButtonAttributes, 'class'> & {
    variant?: 'primary' | 'secondary' | 'ghost';
    size?: 'sm' | 'md' | 'lg' | 'icon';
    label?: string;
    class?: ClassValue | null;
    children: Snippet;
  };

  let {
    variant = 'secondary',
    size = 'md',
    type = 'button',
    label,
    title = label,
    children,
    class: cls = '',
    ...rest
  }: Props = $props();

  const variants = {
    primary: 'bg-(--mono-purple) text-white font-semibold hover:opacity-90',
    secondary:
      'bg-white hover:bg-(--mono-bg) border border-(--mono-border) text-sm transition-colors shadow-sm',
    ghost: 'text-(--mono-muted) hover:text-(--mono-text) transition-colors',
  };

  const sizes = {
    sm: 'px-2 py-1 text-xs',
    md: 'px-4 py-2',
    lg: 'w-full py-3.5 rounded-xl text-sm',
    icon: 'h-8 w-8 p-0 inline-flex items-center justify-center',
  };

  const className = $derived(
    mergeClasses(
      variants[variant],
      sizes[size],
      'cursor-pointer rounded-xl disabled:cursor-not-allowed',
      cls,
    ),
  );
</script>

<BitsButton.Root {type} aria-label={label} {title} class={className} {...rest}>
  {@render children()}
</BitsButton.Root>
