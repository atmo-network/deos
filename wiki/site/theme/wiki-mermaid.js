/*
The page's Mermaid diagrams are rendered in the browser, because Mermaid needs real text metrics that
a static build does not have. This loader runs only on a page that declares diagrams: it resolves the
vendored distribution from the site root, takes its palette from the live theme, and re-renders when
the reader switches theme, so a diagram never keeps the colours of the theme it was first drawn in.
*/
(() => {
  const blocks = [...document.querySelectorAll('.wiki-mermaid')];
  if (!blocks.length) return;

  // The authored source is captured once, before anything is drawn: rendering replaces the element's
  // children with SVG, so reading the element again would feed the previous drawing back to Mermaid.
  const diagrams = blocks.map(block => ({
    block,
    source: block.textContent,
    id: `wiki-diagram-${block.dataset.diagram}`,
  }));

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

  // A theme switch changes the palette and nothing else that this loader observes, so the palette is
  // the signature: an unrelated class change on <html> redraws nothing, and no real switch is skipped.
  const signature = () => {
    const { text, line, accent, surface } = palette();
    return [document.documentElement.className, text, line, accent, surface].join('\u0000');
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
  let drawn = 0;
  let drawnSignature = null;
  let queue = Promise.resolve();

  const draw = async () => {
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
    const signatureNow = signature();
    let failed = false;
    for (const { block, source, id } of diagrams) {
      try {
        const { svg } = await api.render(`${id}-${++drawn}`, source);
        block.innerHTML = svg;
        block.dataset.rendered = 'true';
        delete block.dataset.failed;
        delete block.dataset.stale;
      } catch (error) {
        failed = true;
        // A diagram that cannot be drawn keeps its meaning: a page that never drew anything shows the
        // authored source as text again, and a page that already drew one keeps the drawing and says
        // that its colours belong to the previous theme.
        if (block.dataset.rendered === 'true') block.dataset.stale = 'true';
        else {
          block.textContent = source;
          block.dataset.failed = 'true';
        }
        console.warn(`Mermaid diagram ${id} failed to render`, error);
      }
    }
    drawnSignature = failed ? null : signatureNow;
  };

  const redraw = () => {
    if (signature() === drawnSignature) return queue;
    queue = queue.then(draw, draw);
    return queue;
  };

  queue = queue.then(draw, draw).catch(error => console.warn('Mermaid is unavailable', error));
  // The reader's theme switch only changes attributes on <html>, so watch that one element.
  new MutationObserver(() => {
    redraw().catch(error => console.warn('Mermaid could not follow the theme', error));
  }).observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'data-theme'] });
})();
