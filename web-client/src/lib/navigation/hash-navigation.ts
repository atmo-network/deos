/*
Domain: Hash navigation
Owns: Stable URL-hash parsing, serialization, and browser subscription for widget deep links.
Excludes: Layout-tree mutation, widget state ownership, route history beyond the hash, and domain data loading.
Zone: Browser navigation contract; consumed by the layout composition root and deep-linkable widgets.
*/

export type SwapAssetLinkValue = 'native' | 'foreign';

export type WidgetDeepLink =
  | { widget: 'wiki'; pageId: string | null }
  | {
      widget: 'swap';
      input: SwapAssetLinkValue;
      output: SwapAssetLinkValue;
    };

function decodeSegment(value: string): string | null {
  try {
    const decoded = decodeURIComponent(value).trim();
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

export function parseWidgetDeepLink(hash: string): WidgetDeepLink | null {
  const path = hash.replace(/^#\/?/, '');
  const [widget, first, second] = path.split('/');
  if (widget === 'wiki') {
    return { widget: 'wiki', pageId: first ? decodeSegment(first) : null };
  }
  if (
    widget === 'swap' &&
    (first === 'native' || first === 'foreign') &&
    (second === 'native' || second === 'foreign') &&
    first !== second
  ) {
    return { widget: 'swap', input: first, output: second };
  }
  return null;
}

export function currentWidgetDeepLink(): WidgetDeepLink | null {
  return typeof window === 'undefined'
    ? null
    : parseWidgetDeepLink(window.location.hash);
}

function setHash(hash: string) {
  if (typeof window === 'undefined' || window.location.hash === hash) {
    return;
  }
  window.location.hash = hash;
}

export function navigateToWiki(pageId: string | null) {
  setHash(pageId ? `#wiki/${encodeURIComponent(pageId)}` : '#wiki');
}

export function navigateToSwap(
  input: SwapAssetLinkValue,
  output: SwapAssetLinkValue,
) {
  if (input !== output) {
    setHash(`#swap/${input}/${output}`);
  }
}

export function subscribeToWidgetDeepLinks(
  listener: (link: WidgetDeepLink | null) => void,
) {
  if (typeof window === 'undefined') {
    return () => {};
  }
  const notify = () => listener(currentWidgetDeepLink());
  window.addEventListener('hashchange', notify);
  notify();
  return () => window.removeEventListener('hashchange', notify);
}
