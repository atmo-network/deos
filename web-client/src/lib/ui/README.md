# UI Kit

UI Kit is the DEOS web-client UI kit. It owns reusable presentation primitives and low-level interaction wrappers used by widgets, layout, and product slices.

## Owns

- Project-local UI primitives such as `Button`, `IconButton`, `SelectableTile`, cards, notices, rows, text fields, numeric fields, textareas, native select fields, badges, and popovers.
- Safe default interaction semantics for reusable controls. Buttons default to `type="button"` unless a caller explicitly overrides the type.
- Presentation-only helpers such as `ui/format.ts`.
- Thin wrappers around Bits UI primitives when the primitive provides accessibility or interaction behavior that should be centralized.

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

## Field semantics

`TextField`, `NumberInput`, `TextArea`, and `SelectField` own repeated label/control wiring. `RichSelect` owns compact rich-option select presentation for dropdowns that need badges or secondary detail text. Use `NumberInput` for numeric browser input semantics, while domain parsing and validation stay in the owning slice.

Numeric domain inputs must validate complete literals before conversion instead of relying on JavaScript prefix/coercion behavior. Token-denominated fields should use `parseTokenInputAmount` / `formatTokenInputAmount` from `format.ts`; other numeric domains should use the complete-literal helpers in `../format/numeric.ts` rather than open-coding local regex/`Number` parsing.

## Button semantics

`Button`, `IconButton`, and `SelectableTile` default to non-submit behavior. Use `type="submit"` only at a real form boundary.

## Class value policy

UI Kit controls type caller-facing class props with Svelte's `ClassValue` and accept Svelte-style class values, including strings, arrays, and object maps. Shared class merging lives in `class.ts` so wrappers do not stringify array classes into comma-separated text.
