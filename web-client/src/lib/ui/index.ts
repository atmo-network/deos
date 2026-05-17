/*
Domain: UI Kit public barrel
Owns: Stable export surface for repository-local UI primitives.
Excludes: Product/domain exports, widget exports, and implementation-specific grouping policy.
Zone: Foundation UI entrypoint; re-exports UI Kit components only.
*/
export { flattenClass, mergeClasses } from './class';
export { default as Badge } from './Badge.svelte';
export { default as Button } from './Button.svelte';
export { default as Card } from './Card.svelte';
export { default as DetailRow } from './DetailRow.svelte';
export { default as IconButton } from './IconButton.svelte';
export { default as Notice } from './Notice.svelte';
export { default as NumberInput } from './NumberInput.svelte';
export { default as PopoverPanel } from './PopoverPanel.svelte';
export { default as ReadModelBadge } from './ReadModelBadge.svelte';
export { default as RichSelect } from './RichSelect.svelte';
export type { RichSelectItem } from './RichSelect.svelte';
export { default as SectionCard } from './SectionCard.svelte';
export { default as SelectableTile } from './SelectableTile.svelte';
export { default as SelectField } from './SelectField.svelte';
export { default as SidePanelDialog } from './SidePanelDialog.svelte';
export { default as Sparkline } from './Sparkline.svelte';
export { default as StatCard } from './StatCard.svelte';
export { default as TextArea } from './TextArea.svelte';
export { default as TextField } from './TextField.svelte';
export type { NoticeVariant } from './notice-contract';
