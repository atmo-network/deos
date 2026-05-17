<!--
Domain: UI Kit primitive
Owns: Labeled textarea presentation with helper copy and bindable value plumbing.
Excludes: Domain validation, serialization policy, and form submission behavior.
Zone: Foundation UI; wraps native textarea attributes without importing product slices.
-->
<script lang="ts">
  import { Label } from 'bits-ui';
  import type { ClassValue, HTMLTextareaAttributes } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = Omit<HTMLTextareaAttributes, 'children' | 'class'> & {
    label?: string;
    helper?: string;
    value?: string;
    class?: ClassValue | null;
    textareaClass?: ClassValue | null;
  };

  let {
    label,
    helper,
    value = $bindable(),
    class: cls = '',
    textareaClass = '',
    id,
    ...rest
  }: Props = $props();

  const generatedId = $props.id();
  const textareaId = $derived(id ?? generatedId);
</script>

<label class={mergeClasses('grid gap-1', cls)} for={textareaId}>
  {#if label}
    <Label.Root for={textareaId} class="text-xs text-(--mono-muted)"
      >{label}</Label.Root
    >
  {/if}
  <textarea
    id={textareaId}
    bind:value
    class={mergeClasses(
      'w-full rounded-xl border border-(--mono-border) bg-white px-3 py-2 text-sm focus:border-(--mono-purple) focus:outline-none',
      textareaClass,
    )}
    {...rest}
  ></textarea>
  {#if helper}
    <div class="text-[10px] text-(--mono-muted)">{helper}</div>
  {/if}
</label>
