<!--
Domain: UI Kit primitive
Owns: Labeled numeric input styling, helper copy, and bindable value plumbing.
Excludes: Parsing policy, validation semantics, and domain amount units.
Zone: Foundation UI; wraps native number input attributes without importing product slices.
-->
<script lang="ts">
  import { Label } from 'bits-ui';
  import type { ClassValue, HTMLInputAttributes } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLInputAttributes, 'class'> & {
    class?: ClassValue | null;
    label?: string;
    helper?: string;
    ref?: HTMLInputElement | null;
    suffix?: string;
    value?: string | number;
    wrapperClass?: ClassValue | null;
  };

  let {
    value = $bindable(),
    class: cls = '',
    label,
    helper,
    ref = $bindable(null),
    suffix,
    wrapperClass = '',
    id,
    ...rest
  }: Props = $props();

  const generatedId = $props.id();
  const inputId = $derived(id ?? generatedId);
</script>

<label class={mergeClasses('grid gap-1', wrapperClass)} for={inputId}>
  {#if label}
    <Label.Root for={inputId} class="text-xs text-(--mono-muted)"
      >{label}</Label.Root
    >
  {/if}
  <div class="relative min-w-0">
    <input
      id={inputId}
      bind:this={ref}
      type="number"
      bind:value
      class={mergeClasses(
        'w-full bg-white border border-(--mono-border) px-3 py-2 text-sm focus:outline-none focus:border-(--mono-purple)',
        suffix && 'pr-7',
        cls,
      )}
      {...rest}
    />
    {#if suffix}
      <span
        class="pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-xs text-(--mono-muted)"
        aria-hidden="true"
      >
        {suffix}
      </span>
    {/if}
  </div>
  {#if helper}
    <div class="text-[10px] text-(--mono-muted)">{helper}</div>
  {/if}
</label>
