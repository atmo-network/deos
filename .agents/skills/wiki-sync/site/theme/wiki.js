/*
Domain: mdBook embedding-safe link behavior
Owns: Safe external tabs and a website-owned current-page action.
Excludes: Markdown rendering, graph parsing, wallet access and parent-window messaging.
*/
for (const anchor of document.querySelectorAll('a[href]')) {
  const url = new URL(anchor.href, location.href);
  if (['http:', 'https:', 'mailto:'].includes(url.protocol) && url.origin !== location.origin) {
    anchor.target = '_blank';
    anchor.rel = 'noopener noreferrer';
  }
}
const menu = document.querySelector('#mdbook-menu-bar .right-buttons');
if (menu) {
  const open = document.createElement('a');
  open.className = 'wiki-open-current';
  open.target = '_blank';
  open.rel = 'noopener noreferrer';
  open.textContent = document.documentElement.lang === 'ru' ? 'Открыть страницу ↗' : 'Open page ↗';
  open.title = open.textContent;
  const sync = () => { open.href = location.href; };
  sync();
  addEventListener('hashchange', sync);
  addEventListener('popstate', sync);
  // Native search uses pushState without a navigation event; read the URL on use.
  for (const event of ['pointerenter', 'pointerdown', 'focus', 'click', 'auxclick', 'contextmenu']) open.addEventListener(event, sync);
  menu.append(open);
}
