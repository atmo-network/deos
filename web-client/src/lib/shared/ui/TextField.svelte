<script lang="ts">
  import { Label } from "bits-ui";
  import type { HTMLInputAttributes } from "svelte/elements";

  type Props = Omit<HTMLInputAttributes, "children"> & {
    label?: string;
    helper?: string;
    value?: string | number;
    inputClass?: string;
  };

  let {
    label,
    helper,
    value = $bindable(),
    class: cls = "",
    inputClass = "",
    id,
    ...rest
  }: Props = $props();

  const generatedId = `field-${Math.random().toString(36).slice(2, 10)}`;
  const inputId = $derived(id ?? generatedId);
</script>

<label class={["grid gap-1", cls]} for={inputId}>
  {#if label}
    <Label.Root for={inputId} class="text-xs text-(--mono-muted)">{label}</Label.Root>
  {/if}
  <input
    id={inputId}
    bind:value
    class={[
      "w-full rounded-xl border border-(--mono-border) bg-white px-3 py-2 text-sm focus:border-(--mono-purple) focus:outline-none",
      inputClass,
    ]}
    {...rest}
  />
  {#if helper}
    <div class="text-[10px] text-(--mono-muted)">{helper}</div>
  {/if}
</label>
