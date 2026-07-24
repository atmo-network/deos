<!--
Domain: UI Kit primitive
Owns: Rich single-select dropdown presentation for compact option rows with optional badges and details.
Excludes: Product/domain option construction, asset semantics, and store ownership.
Zone: Foundation UI; wraps Bits UI select for reusable non-native select surfaces.
-->
<script lang="ts">
  import { Check, ChevronDown } from '@lucide/svelte';
  import { Select } from 'bits-ui';
  import type { ClassValue } from 'svelte/elements';

  import Icon from './Icon.svelte';
  import { mergeClasses } from './class';

  export type RichSelectItem = {
    value: string;
    label: string;
    detail?: string;
  };

  type Props = {
    items: RichSelectItem[];
    value: string;
    label: string;
    allowDeselect?: boolean;
    dense?: boolean;
    triggerClass?: ClassValue | null;
    onOpenChange?: (open: boolean) => void;
    onValueChange: (value: string) => void;
  };

  let {
    items,
    value,
    label,
    allowDeselect = false,
    dense = false,
    triggerClass = '',
    onOpenChange,
    onValueChange,
  }: Props = $props();

  const selectItems = $derived(
    items.map((item) => ({ value: item.value, label: item.label })),
  );
  const selectedItem = $derived(
    items.find((item) => item.value === value) ?? items[0],
  );
  const triggerClassName = $derived(
    mergeClasses(
      'inline-flex w-fit min-w-0 max-w-full cursor-pointer items-center gap-1.5 rounded-lg border border-(--mono-border) font-medium transition-colors hover:border-(--mono-purple) data-[state=open]:border-(--mono-purple)',
      dense ? 'px-2 py-0.5 text-[11px]' : 'px-2 py-1 text-xs',
      triggerClass,
    ),
  );
</script>

<Select.Root
  type="single"
  {value}
  items={selectItems}
  {allowDeselect}
  {onOpenChange}
  {onValueChange}
>
  <Select.Trigger class={triggerClassName} aria-label={label}>
    <span class="min-w-0 truncate">{selectedItem?.label ?? label}</span>
    <Icon icon={ChevronDown} size="sm" class="text-current" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content
      sideOffset={3}
      class="z-50 min-w-44 rounded-xl border border-(--mono-border) bg-[linear-gradient(135deg,#ffffff_0%,#f7fbef_46%,#edf6fa_100%)] p-1 shadow-[0_8px_24px_rgba(44,50,30,0.06)]"
    >
      <Select.Viewport class="grid gap-1">
        {#each items as item}
          <Select.Item
            value={item.value}
            label={item.label}
            class="group grid cursor-pointer grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-lg border border-transparent px-2.5 py-2 text-xs outline-none transition-colors data-highlighted:border-(--mono-purple)/20 data-highlighted:bg-(--mono-bg) data-selected:border-(--mono-purple)/25 data-selected:bg-(--mono-bg)"
          >
            <span class="min-w-0">
              <span class="block truncate font-medium text-(--mono-text)"
                >{item.label}</span
              >
              {#if item.detail}
                <span class="block truncate text-[10px] text-(--mono-muted)"
                  >{item.detail}</span
                >
              {/if}
            </span>
            <Icon
              icon={Check}
              size="sm"
              class="text-(--mono-purple) opacity-0 transition-opacity group-data-selected:opacity-100"
            />
          </Select.Item>
        {/each}
      </Select.Viewport>
    </Select.Content>
  </Select.Portal>
</Select.Root>
