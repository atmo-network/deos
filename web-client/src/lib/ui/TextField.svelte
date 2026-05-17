<!--
Domain: UI Kit primitive
Owns: Labeled text/input field presentation with helper copy and bindable value plumbing.
Excludes: Domain validation, parsing policy, and form submission behavior.
Zone: Foundation UI; may wrap Bits UI label but must not import product slices.
-->
<script lang="ts">
  import { Label } from 'bits-ui';
  import type { ClassValue, HTMLInputAttributes } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLInputAttributes, 'children' | 'class'> & {
    class?: ClassValue | null;
    label?: string;
    helper?: string;
    value?: string | number;
    inputClass?: ClassValue | null;
  };

  let {
    label,
    helper,
    value = $bindable(),
    class: cls = '',
    inputClass = '',
    id,
    ...rest
  }: Props = $props();

  const generatedId = $props.id();
  const inputId = $derived(id ?? generatedId);
</script>

<label class={mergeClasses('grid gap-1', cls)} for={inputId}>
  {#if label}
    <Label.Root for={inputId} class="text-xs text-(--mono-muted)"
      >{label}</Label.Root
    >
  {/if}
  <input
    id={inputId}
    bind:value
    class={mergeClasses(
      'w-full rounded-xl border border-(--mono-border) bg-white px-3 py-2 text-sm focus:border-(--mono-purple) focus:outline-none',
      inputClass,
    )}
    {...rest}
  />
  {#if helper}
    <div class="text-[10px] text-(--mono-muted)">{helper}</div>
  {/if}
</label>
