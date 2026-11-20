/*
Domain: mdBook site chrome
Owns: Safe external tabs, the light/dark switch over mdBook's native themes, outside-click search dismissal, whole-row sidebar group folding and an embedded-only current-page action.
Excludes: Markdown rendering, graph parsing, wallet access and parent-window messaging.
*/
(() => {
const ru = document.documentElement.lang === 'ru';

// The search panel is centred in the viewport, so it leaves the page wrapper: mdBook slides that
// wrapper during sidebar transitions, and a transformed ancestor would capture the panel's position.
const panel = document.getElementById('mdbook-search-wrapper');
if (panel) {
  document.body.append(panel);
  // Reuse the native toggle so dismissal also clears the search state in the fragment.
  const searchToggle = document.getElementById('mdbook-search-toggle');
  document.addEventListener('click', event => {
    if (panel.classList.contains('hidden') || panel.contains(event.target) || searchToggle?.contains(event.target)) return;
    searchToggle?.click();
  }, true);
}

for (const anchor of document.querySelectorAll('a[href]')) {
  const url = new URL(anchor.href, location.href);
  if (['http:', 'https:', 'mailto:'].includes(url.protocol) && url.origin !== location.origin) {
    anchor.target = '_blank';
    anchor.rel = 'noopener noreferrer';
  }
}

// One light/dark switch replaces mdBook's five-theme palette. It drives the native theme buttons,
// so persistence, code highlighting and the system-preference default stay mdBook's own.
const dark = new Set(['navy', 'coal', 'ayu']);
const icon = paths => `<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${paths}</svg>`;
const sun = icon('<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>');
const moon = icon('<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>');
const palette = document.querySelector('#mdbook-theme-toggle');
if (palette) {
  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'icon-button wiki-theme-toggle';
  const isDark = () => Array.from(document.documentElement.classList).some(name => dark.has(name));
  const sync = () => {
    const label = isDark() ? (ru ? 'Светлая тема' : 'Light theme') : (ru ? 'Тёмная тема' : 'Dark theme');
    toggle.innerHTML = isDark() ? sun : moon;
    toggle.title = label;
    toggle.setAttribute('aria-label', label);
    toggle.setAttribute('aria-pressed', String(isDark()));
  };
  toggle.addEventListener('click', () => {
    document.querySelector(`#mdbook-theme-${isDark() ? 'light' : 'navy'}`)?.click();
    sync();
  });
  new MutationObserver(sync).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
  sync();
  palette.after(toggle);
}

// Only an embedded reader needs a way out to the actual current page; a standalone page already is one.
const menu = document.querySelector('#mdbook-menu-bar .right-buttons');
if (menu && window.self !== window.top) {
  const open = document.createElement('a');
  open.className = 'icon-button wiki-open-current';
  open.target = '_blank';
  open.rel = 'noopener noreferrer';
  const label = ru ? 'Открыть страницу в новой вкладке' : 'Open this page in a new tab';
  open.title = label;
  open.setAttribute('aria-label', label);
  open.innerHTML = icon('<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>');
  const sync = () => { open.href = location.href; };
  sync();
  addEventListener('hashchange', sync);
  addEventListener('popstate', sync);
  // Native search uses pushState without a navigation event; read the URL on use.
  for (const event of ['pointerenter', 'pointerdown', 'focus', 'click', 'auxclick', 'contextmenu']) open.addEventListener(event, sync);
  menu.append(open);
}
// Sidebar groups fold from their whole row, not only the chevron: the row drives mdBook's own toggle.
// Groups start folded; mdBook and the layer controller expand the current page's group.
for (const wrapper of document.querySelectorAll('#mdbook-sidebar .chapter-link-wrapper')) {
  const toggle = wrapper.querySelector(':scope > .chapter-fold-toggle');
  const label = wrapper.querySelector(':scope > span');
  if (!toggle || !label) continue;
  const item = wrapper.parentElement;
  wrapper.classList.add('wiki-group');
  label.setAttribute('role', 'button');
  label.tabIndex = 0;
  const sync = () => label.setAttribute('aria-expanded', String(item.classList.contains('expanded')));
  const flip = () => { toggle.click(); sync(); };
  wrapper.addEventListener('click', event => {
    if (event.target.closest('.chapter-fold-toggle')) queueMicrotask(sync);
    else flip();
  });
  label.addEventListener('keydown', event => {
    if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); flip(); }
  });
  new MutationObserver(sync).observe(item, { attributes: true, attributeFilter: ['class'] });
  sync();
}
document.documentElement.dataset.wikiChrome = 'ready';
})();
