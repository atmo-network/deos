/*
The page's Mermaid diagrams are rendered in the browser, because Mermaid needs real text metrics that
a static build does not have. This loader runs only on a page that declares diagrams: it resolves the
vendored distribution from the site root, takes its palette from the live theme, and re-renders when
the reader switches theme, so a diagram never keeps the colours of the theme it was first drawn in.
*/
(() => {
  const blocks = [...document.querySelectorAll('.wiki-mermaid')];
  if (!blocks.length) return;

  const palette = () => {
    const style = getComputedStyle(document.documentElement);
    const read = (name, fallback) => (style.getPropertyValue(name).trim() || fallback);
    const body = getComputedStyle(document.body);
    return {
      text: read('--fg', body.color),
      line: read('--sidebar-spacer', read('--table-border-color', '#7a8699')),
      accent: read('--sidebar-active', read('--links', '#2b6cb0')),
      surface: read('--theme-hover', 'transparent'),
    };
  };

  // The vendored distribution lives at the site root, once for every locale. The reader context knows
  // that root; before it exists, this script's own staged URL is the reliable anchor, because mdBook
  // copies it into each book's theme directory.
  const distributionUrl = () => {
    if (window.wikiContext?.siteRoot) return new URL('theme/mermaid/mermaid.esm.min.js', window.wikiContext.siteRoot);
    const self = document.currentScript?.src;
    if (self) return new URL('../../theme/mermaid/mermaid.esm.min.js', self);
    return new URL('theme/mermaid/mermaid.esm.min.js', document.baseURI);
  };

  let api = null;
  const render = async (force) => {
    if (!api) api = (await import(distributionUrl().href)).default;
    const { text, line, accent, surface } = palette();
    api.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: 'base',
      fontFamily: 'inherit',
      htmlLabels: false,
      // A diagram sits in a reading column, so it scales to that column and labels stay at body size.
      flowchart: { htmlLabels: false, useMaxWidth: true, nodeSpacing: 40, rankSpacing: 55, curve: 'basis' },
      themeVariables: {
        fontSize: '15px',
        background: 'transparent',
        primaryColor: surface,
        primaryBorderColor: accent,
        primaryTextColor: text,
        secondaryColor: surface,
        secondaryBorderColor: line,
        secondaryTextColor: text,
        tertiaryColor: 'transparent',
        tertiaryTextColor: text,
        lineColor: line,
        textColor: text,
        edgeLabelBackground: 'transparent',
        clusterBkg: 'transparent',
        clusterBorder: line,
      },
    });
    for (const block of blocks) {
      const source = block.textContent;
      const id = `wiki-diagram-${block.dataset.diagram}`;
      try {
        const { svg } = await api.render(`${id}-${force ? 'again' : 'first'}`, source);
        block.innerHTML = svg;
        block.dataset.rendered = 'true';
      } catch (error) {
        // A diagram that cannot be drawn keeps its source visible: the page still says what it means.
        block.dataset.failed = 'true';
        console.warn(`Mermaid diagram ${id} failed to render`, error);
      }
    }
  };

  render(false).catch(error => console.warn('Mermaid is unavailable', error));
  // The reader's theme switch only toggles a class on <html>, so watch that one attribute.
  let rendered = false;
  new MutationObserver(() => {
    if (!rendered) { rendered = true; return; }
    render(true).catch(() => {});
  }).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
})();
