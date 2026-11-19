/*
Domain: Static-site fragment state
Owns: The client-side `#!` state grammar (layer, search, highlight, section) and its merge rules.
Excludes: Server-facing query strings, history actions, layer membership and search ranking.
*/

// A static artifact has no server to read a query string, so the reader's own state belongs to the
// client-side fragment. A plain fragment keeps mdBook's native anchor semantics (`#section`); a
// stateful fragment starts with `#!` and records only non-default values. Handlers pass the same
// shape of plain values, so one reader of `href` sees the whole model.
export function readHashState(href) {
  const raw = new URL(href).hash.slice(1);
  const stateful = raw.startsWith('!');
  const params = new URLSearchParams(stateful ? raw.slice(1) : '');
  const layers = params.getAll('layer');
  let at = null;
  if (stateful) at = params.has('at') ? params.get('at') : null;
  else if (raw !== '') {
    try { at = decodeURIComponent(raw); } catch { at = raw; }
  }
  return {
    stateful,
    layer: layers.length === 0 ? 'all' : layers.length === 1 ? layers[0] : null,
    search: params.has('search') ? params.get('search') : null,
    highlight: params.has('highlight') ? params.get('highlight') : null,
    at,
  };
}

// Only this codec's keys are rewritten; every other fragment key survives an update untouched, and
// the query string is never written because no part of the state belongs to the server. `source`
// names the reader's current page, so a link to another page can inherit that context.
export function writeHashState(href, patch, source = href) {
  const url = new URL(href);
  const current = readHashState(href);
  const next = {
    layer: 'layer' in patch ? patch.layer : current.layer,
    search: 'search' in patch ? patch.search : current.search,
    highlight: 'highlight' in patch ? patch.highlight : current.highlight,
    at: 'at' in patch ? patch.at : current.at,
  };
  const params = new URLSearchParams();
  if (next.layer && next.layer !== 'all') params.set('layer', next.layer);
  if (next.search !== null) params.set('search', next.search);
  if (next.highlight !== null) params.set('highlight', next.highlight);
  const extra = new URLSearchParams();
  const inherited = new URL(source).hash.slice(1);
  for (const [key, value] of new URLSearchParams(inherited.startsWith('!') ? inherited.slice(1) : '')) {
    if (!['layer', 'search', 'highlight', 'at'].includes(key)) extra.append(key, value);
  }
  if (params.toString() === '' && extra.toString() === '') url.hash = next.at === null ? '' : encodeURIComponent(next.at);
  else {
    if (next.at !== null) params.set('at', next.at);
    url.hash = `!${params.toString()}${extra.toString() === '' ? '' : `${params.toString() === '' ? '' : '&'}${extra.toString()}`}`;
  }
  return url.href;
}
