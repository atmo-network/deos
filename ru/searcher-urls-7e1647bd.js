'use strict';

/* global Mark, elasticlunr, path_to_root */

window.search = window.search || {};
(function search() {
    // Search functionality
    //
    // You can use !hasFocus() to prevent keyhandling in your key
    // event handlers while the user is typing their search.

    if (window.wikiRequiresContext && (!window.wikiContext || window.wikiContext.selection === null)) {
        window.search.hasFocus = () => true;
        return;
    }
    if (!Mark || !elasticlunr) {
        return;
    }

    // eslint-disable-next-line max-len
    // IE 11 Compatibility from https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/startsWith
    if (!String.prototype.startsWith) {
        String.prototype.startsWith = function(search, pos) {
            return this.substr(!pos || pos < 0 ? 0 : +pos, search.length) === search;
        };
    }

    const search_wrap = document.getElementById('mdbook-search-wrapper'),
        searchbar_outer = document.getElementById('mdbook-searchbar-outer'),
        searchbar = document.getElementById('mdbook-searchbar'),
        searchresults = document.getElementById('mdbook-searchresults'),
        searchresults_outer = document.getElementById('mdbook-searchresults-outer'),
        searchresults_header = document.getElementById('mdbook-searchresults-header'),
        searchicon = document.getElementById('mdbook-search-toggle'),
        content = document.getElementById('mdbook-content'),

        // SVG text elements don't render if inside a <mark> tag.
        mark_exclude = ['text'],
        marker = new Mark(content);

    let current_searchterm = '',
        doc_urls = [],
        search_options = {
            bool: 'AND',
            expand: true,
            fields: {
                title: {boost: 1},
                body: {boost: 1},
                breadcrumbs: {boost: 0},
            },
        },
        searchindex = null,
        results_options = {
            teaser_word_count: 30,
            limit_results: 30,
        },
        teaser_count = 0;

    function hasFocus() {
        return searchbar === document.activeElement;
    }

    function removeChildren(elem) {
        while (elem.firstChild) {
            elem.removeChild(elem.firstChild);
        }
    }

function readHashState(href) {
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

function writeHashState(href, patch, source = href) {
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

function readSearchLocation(href) {
  const { search, highlight } = readHashState(href);
  return { query: search, highlight };
}

function nextSearchLocation(href, query, action) {
  const previous = readHashState(href).search;
  if (action === 'close') return { href: writeHashState(href, { search: null, highlight: null }), action: previous ? 'push' : 'replace' };
  if (!['push', 'replace', 'push_if_new_search_else_replace'].includes(action)) throw new Error('Unsupported native search history action');
  if (typeof query !== 'string') throw new Error('Native search query must be one opaque string');
  return { href: writeHashState(href, { search: query, highlight: null }), action: action === 'push' || action === 'push_if_new_search_else_replace' && previous === null ? 'push' : 'replace' };
}

function withoutSearchHighlight(href) {
  return writeHashState(href, { highlight: null });
}

function searchResultLocation(href, target, terms) {
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

    let highlightTimer;
    // Native highlighting can replace marked nodes; delegate from the stable content root.
    content.addEventListener('click', event => {
        if (!event.target.closest?.('mark') || !readHashState(window.location.href).highlight === null) return;
        for (const item of content.querySelectorAll('mark')) item.classList.add('fade-out');
        window.clearTimeout(highlightTimer);
        highlightTimer = window.setTimeout(() => marker.unmark(), 300);
        history.replaceState({}, document.title, withoutSearchHighlight(window.location.href));
    });

    // Helper to escape html special chars for displaying the teasers
    const escapeHTML = (function() {
        const MAP = {
            '&': '&amp;',
            '<': '&lt;',
            '>': '&gt;',
            '"': '&#34;',
            '\'': '&#39;',
        };
        const repl = function(c) {
            return MAP[c];
        };
        return function(s) {
            return s.replace(/[&<>'"]/g, repl);
        };
    })();

function formatSearchMetric(count, term) {
  if (count === 0) return `По запросу «${term}» ничего не найдено.`;
  const form = new Intl.PluralRules('ru').select(count);
  const noun = { one: 'результат', few: 'результата', many: 'результатов', other: 'результата' }[form];
  return `${count} ${noun} по запросу «${term}»:`;
}

    function formatSearchResult(result, searchterms) {
        const teaser = makeTeaser(escapeHTML(result.doc.body), searchterms);
        teaser_count++;

        const href = searchResultLocation(window.location.href, path_to_root + doc_urls[result.ref], searchterms);
        return '<a href="' + escapeHTML(href) + '" aria-details="mdbook-teaser_' + teaser_count + '">'
            + result.doc.breadcrumbs + '</a>'
            + '<span class="teaser" id="mdbook-teaser_' + teaser_count
            + '" aria-label="Фрагмент совпадения">' + teaser + '</span>';
    }

    function makeTeaser(body, searchterms) {
        // The strategy is as follows:
        // First, assign a value to each word in the document:
        //  Words that correspond to search terms (stemmer aware): 40
        //  Normal words: 2
        //  First word in a sentence: 8
        // Then use a sliding window with a constant number of words and count the
        // sum of the values of the words within the window. Then use the window that got the
        // maximum sum. If there are multiple maximas, then get the last one.
        // Enclose the terms in <em>.
        const stemmed_searchterms = searchterms.map(function(w) {
            return elasticlunr.stemmer(w.toLowerCase());
        });
        const searchterm_weight = 40;
        const weighted = []; // contains elements of ["word", weight, index_in_document]
        // split in sentences, then words
        const sentences = body.toLowerCase().split('. ');
        let index = 0;
        let value = 0;
        let searchterm_found = false;
        for (const sentenceindex in sentences) {
            const words = sentences[sentenceindex].split(' ');
            value = 8;
            for (const wordindex in words) {
                const word = words[wordindex];
                if (word.length > 0) {
                    for (const searchtermindex in stemmed_searchterms) {
                        if (elasticlunr.stemmer(word).startsWith(
                            stemmed_searchterms[searchtermindex])
                        ) {
                            value = searchterm_weight;
                            searchterm_found = true;
                        }
                    }
                    weighted.push([word, value, index]);
                    value = 2;
                }
                index += word.length;
                index += 1; // ' ' or '.' if last word in sentence
            }
            index += 1; // because we split at a two-char boundary '. '
        }

        if (weighted.length === 0) {
            return body;
        }

        const window_weight = [];
        const window_size = Math.min(weighted.length, results_options.teaser_word_count);

        let cur_sum = 0;
        for (let wordindex = 0; wordindex < window_size; wordindex++) {
            cur_sum += weighted[wordindex][1];
        }
        window_weight.push(cur_sum);
        for (let wordindex = 0; wordindex < weighted.length - window_size; wordindex++) {
            cur_sum -= weighted[wordindex][1];
            cur_sum += weighted[wordindex + window_size][1];
            window_weight.push(cur_sum);
        }

        let max_sum_window_index = 0;
        if (searchterm_found) {
            let max_sum = 0;
            // backwards
            for (let i = window_weight.length - 1; i >= 0; i--) {
                if (window_weight[i] > max_sum) {
                    max_sum = window_weight[i];
                    max_sum_window_index = i;
                }
            }
        } else {
            max_sum_window_index = 0;
        }

        // add <em/> around searchterms
        const teaser_split = [];
        index = weighted[max_sum_window_index][2];
        for (let i = max_sum_window_index; i < max_sum_window_index + window_size; i++) {
            const word = weighted[i];
            if (index < word[2]) {
                // missing text from index to start of `word`
                teaser_split.push(body.substring(index, word[2]));
                index = word[2];
            }
            if (word[1] === searchterm_weight) {
                teaser_split.push('<em>');
            }
            index = word[2] + word[0].length;
            teaser_split.push(body.substring(word[2], index));
            if (word[1] === searchterm_weight) {
                teaser_split.push('</em>');
            }
        }

        return teaser_split.join('');
    }

    function init(config) {
        results_options = config.results_options;
        search_options = config.search_options;
        doc_urls = config.doc_urls;
        searchindex = elasticlunr.Index.load(config.index);

        searchbar_outer.classList.remove('searching');

        searchbar.focus();

        const searchterm = searchbar.value.trim();
        if (searchterm !== '') {
            searchbar.classList.add('active');
            doSearch(searchterm);
        }
    }

    function initSearchInteractions(config) {
        // Set up events
        searchicon.addEventListener('click', () => {
            searchIconClickHandler();
        }, false);
        searchbar.addEventListener('input', () => {
            searchbarInputHandler();
        }, false);
        document.addEventListener('keydown', e => {
            globalKeyHandler(e);
        }, false);
        // If the user uses the browser buttons, do the same as if a reload happened
        window.onpopstate = () => {
            doSearchOrMarkFromUrl();
        };
        window.addEventListener('hashchange', () => {
            doSearchOrMarkFromUrl();
        });
        // Suppress "submit" events so the page doesn't reload when the user presses Enter
        document.addEventListener('submit', e => {
            e.preventDefault();
        }, false);

        // If reloaded, do the search or mark again, depending on the current url parameters
        doSearchOrMarkFromUrl();

        // Exported functions
        config.hasFocus = hasFocus;
    }

    initSearchInteractions(window.search);

    function unfocusSearchbar() {
        // hacky, but just focusing a div only works once
        const tmp = document.createElement('input');
        tmp.setAttribute('style', 'position: absolute; opacity: 0;');
        searchicon.appendChild(tmp);
        tmp.focus();
        tmp.remove();
    }

    // On reload or browser history backwards/forwards events, parse the url and do search or mark
    function resetSearchResults() {
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

    // Eventhandler for keyevents on `document`
    function globalKeyHandler(e) {
        if (e.altKey ||
            e.ctrlKey ||
            e.metaKey ||
            e.shiftKey ||
            e.target.type === 'textarea' ||
            e.target.type === 'text' ||
            !hasFocus() && mdbook_something_else_has_focus(e)
        ) {
            return;
        }

        if (e.key === 'Escape') {
            e.preventDefault();
            searchbar.classList.remove('active');
            setSearchUrlParameters('', 'close');
            if (hasFocus()) {
                unfocusSearchbar();
            }
            showSearch(false);
            marker.unmark();
        } else if (!hasFocus() && (e.key === 's' || e.key === '/')) {
            e.preventDefault();
            showSearch(true);
            setSearchUrlParameters(searchbar.value.trim(), 'push');
            searchbar.select();
        } else if (hasFocus() && (e.key === 'ArrowDown'
                               || e.key === 'Enter')) {
            e.preventDefault();
            const first = searchresults.firstElementChild;
            if (first !== null) {
                unfocusSearchbar();
                first.classList.add('focus');
                if (e.key === 'Enter') {
                    window.location.assign(first.querySelector('a'));
                }
            }
        } else if (!hasFocus() && (e.key === 'ArrowDown'
                                || e.key === 'ArrowUp'
                                || e.key === 'Enter')) {
            // not `:focus` because browser does annoying scrolling
            const focused = searchresults.querySelector('li.focus');
            if (!focused) {
                return;
            }
            e.preventDefault();
            if (e.key === 'ArrowDown') {
                const next = focused.nextElementSibling;
                if (next) {
                    focused.classList.remove('focus');
                    next.classList.add('focus');
                }
            } else if (e.key === 'ArrowUp') {
                focused.classList.remove('focus');
                const prev = focused.previousElementSibling;
                if (prev) {
                    prev.classList.add('focus');
                } else {
                    searchbar.select();
                }
            } else { // Enter
                window.location.assign(focused.querySelector('a'));
            }
        }
    }

    function loadSearchScript(url, id) {
        if (document.getElementById(id)) {
            return;
        }
        searchbar_outer.classList.add('searching');

        const script = document.createElement('script');
        script.src = url;
        script.id = id;
        script.onload = () => init(window.search);
        script.onerror = error => {
            console.error(`Failed to load \`${url}\`: ${error}`);
        };
        document.head.append(script);
    }

    function showSearch(yes) {
        if (yes) {
            loadSearchScript(
                window.path_to_searchindex_js ||
                (window.wikiContext?.indexAsset || path_to_root + 'searchindex-unicode-64be3128.js'),
                'mdbook-search-index');
            search_wrap.classList.remove('hidden');
            searchicon.setAttribute('aria-expanded', 'true');
        } else {
            search_wrap.classList.add('hidden');
            searchicon.setAttribute('aria-expanded', 'false');
            const results = searchresults.children;
            for (let i = 0; i < results.length; i++) {
                results[i].classList.remove('focus');
            }
        }
    }

    function showResults(yes) {
        if (yes) {
            searchresults_outer.classList.remove('hidden');
        } else {
            searchresults_outer.classList.add('hidden');
        }
    }

    // Eventhandler for search icon
    function searchIconClickHandler() {
        if (search_wrap.classList.contains('hidden')) {
            showSearch(true);
            setSearchUrlParameters(searchbar.value.trim(), 'push');
            searchbar.select();
        } else {
            showSearch(false);
            setSearchUrlParameters('', 'close');
        }
    }

    // Apply changes to the search field value.
    function searchbarInputHandler() {
        const searchterm = searchbar.value.trim();
        if (searchterm !== '') {
            searchbar.classList.add('active');
            doSearch(searchterm);
        } else {
            resetSearchResults();
        }

        setSearchUrlParameters(searchterm, 'push_if_new_search_else_replace');

        // Remove marks
        marker.unmark();
    }

    // Record the open search sheet as `search=<opaque query>`; an empty value is an open, empty
    // sheet. Remove ?URL_MARK_PARAM and keep the article fragment. `action` is "push", "replace",
    // "push_if_new_search_else_replace" or "close", which removes the state again.
    // "push_if_new_search_else_replace" pushes only if the URL does not record search yet.
    function setSearchUrlParameters(searchterm, action) {
        const next = nextSearchLocation(window.location.href, searchterm, action);
        if (next.action === 'push') history.pushState({}, document.title, next.href);
        else history.replaceState({}, document.title, next.href);
    }

    function doSearch(searchterm) {
        // Don't search the same twice
        if (current_searchterm === searchterm) {
            return;
        }
        searchbar_outer.classList.add('searching');
        if (searchindex === null) {
            return;
        }

        current_searchterm = searchterm;

        // Do the actual search
        const results = searchindex.search(searchterm, search_options);
        const resultcount = Math.min(results.length, results_options.limit_results);

        // Display search metrics
        searchresults_header.innerText = formatSearchMetric(resultcount, searchterm);

        // Clear and insert results
        const searchterms = searchterm.split(' ');
        removeChildren(searchresults);
        for (let i = 0; i < resultcount ; i++) {
            const resultElem = document.createElement('li');
            resultElem.innerHTML = formatSearchResult(results[i], searchterms);
            searchresults.appendChild(resultElem);
        }

        // Display results
        showResults(true);
        searchbar_outer.classList.remove('searching');
    }

    // Exported functions
    search.hasFocus = hasFocus;
})(window.search);
