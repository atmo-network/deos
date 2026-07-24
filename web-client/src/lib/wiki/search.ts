/*
Domain: Wiki search
Owns: Locale-aware matching and bounded result snippets for the generated Wiki body-search manifest.
Excludes: Manifest generation, navigation metadata search, widget state, and Markdown loading.
Zone: Wiki domain helper; consumes generated plain text without importing Markdown chunks.
*/
import type { WikiSearchManifest } from './metadata-contract';

const MAX_SNIPPET_CHARS = 180;
const SNIPPET_CONTEXT_BEFORE = 56;

export function normalizeWikiSearchText(value: string, locale: string): string {
  return value.trim().toLocaleLowerCase(locale);
}

function boundedSnippet(text: string, matchIndex: number, queryLength: number) {
  const start = Math.max(0, matchIndex - SNIPPET_CONTEXT_BEFORE);
  const end = Math.min(text.length, start + MAX_SNIPPET_CHARS);
  const prefix = start > 0 ? '…' : '';
  const suffix = end < text.length ? '…' : '';
  const excerpt = text.slice(start, Math.max(end, matchIndex + queryLength));
  return `${prefix}${excerpt.trim()}${suffix}`;
}

export function matchWikiSearchBodies(
  manifest: WikiSearchManifest,
  query: string,
  locale: string,
): Map<string, string> {
  const normalizedQuery = normalizeWikiSearchText(query, locale);
  const matches = new Map<string, string>();
  if (!normalizedQuery) {
    return matches;
  }
  for (const page of manifest.pages) {
    const text =
      page.text[locale] ??
      page.text[manifest.default_locale] ??
      Object.values(page.text)[0];
    if (!text) {
      continue;
    }
    const matchIndex = normalizeWikiSearchText(text, locale).indexOf(
      normalizedQuery,
    );
    if (matchIndex >= 0) {
      matches.set(
        page.id,
        boundedSnippet(text, matchIndex, normalizedQuery.length),
      );
    }
  }
  return matches;
}
