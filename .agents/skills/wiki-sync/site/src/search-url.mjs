/*
Domain: Native reader search URLs
Owns: Opaque query encoding, read-only restoration and guarded native UI asset adaptation.
Excludes: Search indexing/ranking, Markdown rendering, global history patches and layer policy.
*/
export function readSearchLocation(href) {
  const url = new URL(href);
  return { query: url.searchParams.get('search'), highlight: url.searchParams.get('highlight') };
}

export function nextSearchLocation(href, query, action) {
  if (!['push', 'replace', 'push_if_new_search_else_replace'].includes(action)) throw new Error('Unsupported native search history action');
  const url = new URL(href);
  const first = !url.searchParams.has('search');
  if (query !== '' || action === 'push_if_new_search_else_replace') {
    url.searchParams.set('search', query);
    url.hash = '';
  } else url.searchParams.delete('search');
  url.searchParams.delete('highlight');
  return { href: url.href, action: action === 'push' || action === 'push_if_new_search_else_replace' && first ? 'push' : 'replace' };
}

export function withoutSearchHighlight(href) {
  const url = new URL(href);
  url.searchParams.delete('highlight');
  return url.href;
}

export function searchResultLocation(href, target, terms) {
  const current = new URL(href), url = new URL(target, href);
  if (url.origin !== current.origin) throw new Error('Native search destination left the site');
  url.search = current.search;
  url.searchParams.delete('search');
  url.searchParams.set('highlight', terms.join(' '));
  return url.href;
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
  const helpers = [readSearchLocation, nextSearchLocation, withoutSearchHighlight, searchResultLocation].map(fn => fn.toString()).join('\n\n');
  range('    // Helper to parse a url into its building blocks.', '    // Helper to escape html special chars', helpers + `

    let highlightTimer;
    // Native highlighting can replace marked nodes; delegate from the stable content root.
    content.addEventListener('click', event => {
        if (!event.target.closest?.('mark') || !new URL(window.location.href).searchParams.has('highlight')) return;
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
  replaceOnce("        URL_SEARCH_PARAM = 'search',\n", '');
  replaceOnce("        marker = new Mark(content),\n        URL_MARK_PARAM = 'highlight';", '        marker = new Mark(content);');
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
