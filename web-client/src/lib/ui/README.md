# UI Kit

UI Kit is the DEOS web-client UI kit. It owns reusable presentation primitives and low-level interaction wrappers used by widgets, layout, and product slices.

## Owns

- Project-local UI primitives such as `Button`, `Icon`, `BackButton`, `SelectableTile`, `DisclosureSection`, cards, notices, rows, text fields, numeric fields, textareas, native select fields, badges, and tooltips.
- Safe default interaction semantics for reusable controls. Buttons default to `type="button"` unless a caller explicitly overrides the type.
- Presentation-only helpers such as `ui/format.ts`.
- Thin wrappers around Bits UI primitives when the primitive provides accessibility or interaction behavior that should be centralized, including shared tooltip timing and presentation.

## Excludes

- Product/domain state, stores, adapters, wallet logic, market logic, governance logic, and chain contracts.
- Widget-specific composition and business wording.
- Transport or persistence policy; browser/session infrastructure belongs under `system/`.

## Bits UI policy

- Use Bits UI for interactive primitives with non-trivial accessibility or state behavior: dialogs, popovers, custom selects/comboboxes, menus, tabs, tooltips, and delegated button behavior.
- Keep Bits UI imports inside UI Kit or highly specialized local components when possible.
- Widgets should prefer UI Kit components over raw primitives for repeated controls.
- Raw HTML controls are acceptable for one-off specialized markup, but repeated patterns should graduate into UI Kit.
- A layout surface may keep a raw control when it needs direct DOM-element ownership that a component wrapper cannot expose safely, such as drag geometry via `bind:this` on an `HTMLButtonElement`; keep that exception local and documented in code.

## Surface semantics

The tile manager and reserved layout lanes own the strong outer separation between widgets. Inside a widget, prefer spacing, typography, alternating white/`--mono-bg` fills, and progressive disclosure over repeated card borders and shadows. `SectionCard`, `StatCard`, `Notice`, and `SelectableTile` follow this border-light hierarchy. Keep visible outlines where they communicate interaction or state: form controls, focus rings, selected choices, data-visualization tracks, overlays, errors that need distinct emphasis, and trusted markdown structures such as tables or code blocks.

## Field semantics

`TextField`, `NumberInput`, `TextArea`, and `SelectField` own repeated label/control wiring. `RichSelect` owns compact rich-option select presentation for dropdowns that need secondary detail text. Use `NumberInput` for numeric browser input semantics and its optional non-interactive `suffix` for compact units such as `%`, while domain parsing and validation stay in the owning slice.

Numeric domain inputs must validate complete literals before conversion instead of relying on JavaScript prefix/coercion behavior. Token-denominated fields should use `parseTokenInputAmount` / `formatTokenInputAmount` from `format.ts`; other numeric domains should use the complete-literal helpers in `../format/numeric.ts` rather than open-coding local regex/`Number` parsing.

## Tooltip semantics

`Tooltip` may hide only supplementary, non-interactive information that users can safely miss on touch devices. Use its delegated `child` trigger mode when an existing `Button` or other control must own the actual trigger element; do not recreate floating tooltip positioning inside widgets. Keep essential instructions, risk, state, and recovery visible; use a popover or inline disclosure when content must remain available without hover. The root `Tooltip.Provider` from Bits UI lives directly in `src/routes/+layout.svelte`; do not add another project wrapper around the provider.

## Icon semantics

Render Lucide glyphs through `Icon` using the canonical `sm` (14px), default `md` (20px), and `lg` (24px) scale rather than local numeric sizes. Icon meaning and accessible action labels remain owned by the caller.

## Button semantics

`Button` and `SelectableTile` default to non-submit behavior. `Button size="icon" label="…"` owns accessible icon-only controls without a parallel button wrapper. Use `type="submit"` only at a real form boundary.

## Class value policy

UI Kit controls type caller-facing class props with Svelte's `ClassValue` and accept Svelte-style class values, including strings, arrays, and object maps. Shared class merging lives in `class.ts`; it flattens Svelte string/array/object class values and uses `tailwind-merge` to remove superseded default, variant, responsive, state, and caller-provided utilities before they reach the DOM. Keep caller classes last and prefer ordinary trailing overrides over `!`; when importance remains necessary, use Tailwind v4 trailing syntax such as `bg-black!` and `hover:bg-black!`, never the legacy leading `!bg-black` form.
