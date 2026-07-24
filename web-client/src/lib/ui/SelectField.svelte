<!--
Domain: UI Kit primitive
Owns: Labeled native select presentation with helper copy and bindable value plumbing.
Excludes: Option derivation, domain selection semantics, and complex combobox behavior.
Zone: Foundation UI; use Bits UI wrappers instead when select behavior needs custom accessibility/state.
-->
<script lang="ts">
  import { Label } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import type { ClassValue, HTMLSelectAttributes } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLSelectAttributes, 'children' | 'class'> & {
    label?: string;
    helper?: string;
    value?: string | number | string[] | null | undefined;
    class?: ClassValue | null;
    selectClass?: ClassValue | null;
    children: Snippet;
  };

  let {
    label,
    helper,
    value = $bindable(),
    class: cls = '',
    selectClass = '',
    children,
    id,
    ...rest
  }: Props = $props();

  const generatedId = $props.id();
  const selectId = $derived(id ?? generatedId);
</script>

<label class={mergeClasses('grid gap-1', cls)} for={selectId}>
  {#if label}
    <Label.Root for={selectId} class="text-xs text-(--mono-muted)"
      >{label}</Label.Root
    >
  {/if}
  <select
    id={selectId}
    bind:value
    class={mergeClasses(
      'w-full cursor-pointer rounded-xl border border-(--mono-border) bg-white px-3 py-2 text-sm text-(--mono-text) focus:border-(--mono-purple) focus:outline-none disabled:cursor-not-allowed',
      selectClass,
    )}
    {...rest}
  >
    {@render children()}
  </select>
  {#if helper}
    <div class="text-[10px] text-(--mono-muted)">{helper}</div>
  {/if}
</label>
