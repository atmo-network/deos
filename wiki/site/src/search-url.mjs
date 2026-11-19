/*
Domain: Native reader search state
Owns: Opaque query encoding, the open/closed overlay fragment state, read-only restoration and guarded native UI asset adaptation.
Excludes: Search indexing/ranking, Markdown rendering, global history patches and layer policy.
*/
import { readHashState, writeHashState } from './hash-state.mjs';

export function readSearchLocation(href) {
  const { search, highlight } = readHashState(href);
  return { query: search, highlight };
}

// `search` presence is the overlay state: a value (possibly empty) means the sheet is open, its
// absence means closed. `close` is the only transition that removes it, so opening with no query,
// typing, clearing the field and closing are all restorable from the fragment and browser history.
// The article's own section survives every transition; the sheet hides the article, so a search
// never needs to move the reader's anchor.
export function nextSearchLocation(href, query, action) {
  const previous = readHashState(href).search;
  if (action === 'close') return { href: writeHashState(href, { search: null, highlight: null }), action: previous ? 'push' : 'replace' };
  if (!['push', 'replace', 'push_if_new_search_else_replace'].includes(action)) throw new Error('Unsupported native search history action');
  if (typeof query !== 'string') throw new Error('Native search query must be one opaque string');
  return { href: writeHashState(href, { search: query, highlight: null }), action: action === 'push' || action === 'push_if_new_search_else_replace' && previous === null ? 'push' : 'replace' };
}

export function withoutSearchHighlight(href) {
  return writeHashState(href, { highlight: null });
}

export function searchResultLocation(href, target, terms) {
  const current = new URL(href), url = new URL(target, href);
  if (url.origin !== current.origin) throw new Error('Native search destination left the site');
  const state = readHashState(href), destination = readHashState(url.href);
  // Unrelated fragment keys describe the reader's own context, so a result keeps them.
  const extras = new URLSearchParams(current.hash.slice(2));
  for (const key of ['layer', 'search', 'highlight', 'at']) extras.delete(key);
  url.hash = '';
  const resolved = new URL(writeHashState(url.href, { layer: state.layer, search: null, highlight: terms.join(' '), at: destination.at }));
  if (extras.toString() !== '') {
    const params = new URLSearchParams(resolved.hash.slice(2));
    for (const [key, value] of extras) params.append(key, value);
    resolved.hash = `!${params.toString()}`;
  }
  return resolved.href;
}

export function rewriteNativeSearchUrls(source) {
  function range(start, end, replacement) {
    const first = source.indexOf(start), last = source.indexOf(end, first + start.length);
    if (first < 0 || last < 0 || source.indexOf(start, first + start.length) !== -1 || source.indexOf(end, last + end.length) !== -1) throw new Error('Pinned native search URL structure changed');
    source = source.slice(0, first) + replacement + source.slice(last);
  }
  function replaceOnce(previous, next) {
    if (!source.includes(previous) || source.indexOf(previous) !== source.lastIndexOf(previous)) throw new Error('Pinned native search state transition changed');
    source = source.replace(previous, next);
  }
  const helpers = [readHashState, writeHashState, readSearchLocation, nextSearchLocation, withoutSearchHighlight, searchResultLocation].map(fn => fn.toString()).join('\n\n');
  range('    // Helper to parse a url into its building blocks.', '    // Helper to escape html special chars', helpers + `

    let highlightTimer;
    // Native highlighting can replace marked nodes; delegate from the stable content root.
    content.addEventListener('click', event => {
        if (!event.target.closest?.('mark') || !readHashState(window.location.href).highlight === null) return;
        for (const item of content.querySelectorAll('mark')) item.classList.add('fade-out');
        window.clearTimeout(highlightTimer);
        highlightTimer = window.setTimeout(() => marker.unmark(), 300);
        history.replaceState({}, document.title, withoutSearchHighlight(window.location.href));
    });

`);
  range('    function doSearchOrMarkFromUrl() {', '    // Eventhandler for keyevents on `document`', `    function resetSearchResults() {
        current_searchterm = '';
        searchbar.classList.remove('active');
        searchbar_outer.classList.remove('searching');
        showResults(false);
        removeChildren(searchresults);
        searchresults_header.textContent = '';
    }

    function doSearchOrMarkFromUrl() {
        const state = readSearchLocation(window.location.href);
        window.clearTimeout(highlightTimer);
        marker.unmark();
        if (state.query !== null) {
            showSearch(true);
            searchbar.value = state.query;
            const term = state.query.trim();
            if (term !== '') {
                searchbar.classList.add('active');
                doSearch(term);
            } else resetSearchResults();
        } else {
            showSearch(false);
            searchbar.value = '';
            resetSearchResults();
        }
        if (state.highlight !== null && state.highlight.trim() !== '') {
            marker.mark(state.highlight.split(' '), { exclude: mark_exclude });
        }
    }

`);
  range('    function setSearchUrlParameters(searchterm, action) {', '    function doSearch(searchterm) {', `    function setSearchUrlParameters(searchterm, action) {
        const next = nextSearchLocation(window.location.href, searchterm, action);
        if (next.action === 'push') history.pushState({}, document.title, next.href);
        else history.replaceState({}, document.title, next.href);
    }

`);
  replaceOnce("            searchbar.classList.remove('active');\n            showResults(false);\n            removeChildren(searchresults);", '            resetSearchResults();');
  // Paste and the browser's search-field clear button change the value without a keyup.
  replaceOnce("searchbar.addEventListener('keyup', () => {\n            searchbarKeyUpHandler();", "searchbar.addEventListener('input', () => {\n            searchbarInputHandler();");
  replaceOnce('// Eventhandler for keyevents while the searchbar is focused\n    function searchbarKeyUpHandler()', '// Apply changes to the search field value.\n    function searchbarInputHandler()');
  replaceOnce("        URL_SEARCH_PARAM = 'search',\n", '');
  // Reader state lives in the fragment, so a same-document fragment change is a state change too.
  replaceOnce('        window.onpopstate = () => {\n            doSearchOrMarkFromUrl();\n        };', '        window.onpopstate = () => {\n            doSearchOrMarkFromUrl();\n        };\n        window.addEventListener(\'hashchange\', () => {\n            doSearchOrMarkFromUrl();\n        });');
  replaceOnce("        marker = new Mark(content),\n        URL_MARK_PARAM = 'highlight';", '        marker = new Mark(content);');
  // Opening the sheet is a URL state and must not move the reading position: the pinned renderer
  // both scrolls the document to the top and leaves the URL untouched on open.
  const openTransition = /showSearch\(true\);\n([ \t]*)window\.scrollTo\(0, 0\);\n([ \t]*)searchbar\.select\(\);/g;
  if ((source.match(openTransition) ?? []).length !== 2) throw new Error('Pinned native search open transition changed');
  source = source.replace(openTransition, (_, indent, selectIndent) => `showSearch(true);\n${indent}setSearchUrlParameters(searchbar.value.trim(), 'push');\n${selectIndent}searchbar.select();`);
  replaceOnce("            setSearchUrlParameters('',\n                searchbar.value.trim() !== '' ? 'push' : 'replace');", "            setSearchUrlParameters('', 'close');");
  replaceOnce('        } else {\n            showSearch(false);\n        }', "        } else {\n            showSearch(false);\n            setSearchUrlParameters('', 'close');\n        }");
  replaceOnce('    // Update current url with ?URL_SEARCH_PARAM= parameter, remove ?URL_MARK_PARAM and\n    // `#heading-anchor`. `action` can be one of "push", "replace",\n    // "push_if_new_search_else_replace" and replaces or pushes a new browser history item.\n    // "push_if_new_search_else_replace" pushes if there is no `?URL_SEARCH_PARAM=abc` yet.', '    // Record the open search sheet as `search=<opaque query>`; an empty value is an open, empty\n    // sheet. Remove ?URL_MARK_PARAM and keep the article fragment. `action` is "push", "replace",\n    // "push_if_new_search_else_replace" or "close", which removes the state again.\n    // "push_if_new_search_else_replace" pushes only if the URL does not record search yet.');
  replaceOnce('    if (!Mark || !elasticlunr) {', `    if (window.wikiRequiresContext && (!window.wikiContext || window.wikiContext.selection === null)) {
        window.search.hasFocus = () => true;
        return;
    }
    if (!Mark || !elasticlunr) {`);
  range('        // The ?URL_MARK_PARAM=', '    function makeTeaser(body, searchterms) {', `        const href = searchResultLocation(window.location.href, path_to_root + doc_urls[result.ref], searchterms);
        return '<a href="' + escapeHTML(href) + '" aria-details="mdbook-teaser_' + teaser_count + '">'
            + result.doc.breadcrumbs + '</a>'
            + '<span class="teaser" id="mdbook-teaser_' + teaser_count
            + '" aria-label="Search Result Teaser">' + teaser + '</span>';
    }

`);
  if (/\b(?:parseURL|renderURL)\(/.test(source)) throw new Error('Legacy native URL codec still has consumers');
  return source;
}

export function rebindScriptAsset(html, previous, next) {
  let bindings = 0;
  const content = html.replace(/<script\b[^>]*\bsrc="([^"]+)"[^>]*><\/script>/g, (script, path) => {
    if (path !== previous && !path.endsWith('/' + previous)) return script;
    bindings++;
    return script.replace(`src="${path}"`, `src="${path.slice(0, -previous.length)}${next}"`);
  });
  return { content, bindings };
}
