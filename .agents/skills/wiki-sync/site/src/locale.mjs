/*
Domain: Native reader localization
Owns: Russian native chrome/metrics and pinned accessible-label corrections outside article content.
Excludes: Markdown rendering, article translation, search ranking and additional UI implementations.
*/
function replaceCount(source, previous, next, allowed) {
  const count = source.split(previous).length - 1;
  if (!(Array.isArray(allowed) ? allowed : [allowed]).includes(count)) throw new Error(`Pinned reader localization changed: ${previous}`);
  return source.replaceAll(previous, next);
}

export function formatRussianSearchMetric(count, term) {
  if (count === 0) return `По запросу «${term}» ничего не найдено.`;
  const form = new Intl.PluralRules('ru').select(count);
  const noun = { one: 'результат', few: 'результата', many: 'результатов', other: 'результата' }[form];
  return `${count} ${noun} по запросу «${term}»:`;
}

export function localizeSearchScript(source, locale) {
  if (locale !== 'ru') return source;
  const start = '    function formatSearchMetric(count, searchterm) {';
  const end = '    function formatSearchResult(result, searchterms) {';
  const first = source.indexOf(start), last = source.indexOf(end, first + start.length);
  if (first < 0 || last < 0 || source.indexOf(start, first + 1) !== -1 || source.indexOf(end, last + 1) !== -1) throw new Error('Pinned reader metric formatter changed');
  source = source.slice(0, first) + formatRussianSearchMetric.toString().replace('formatRussianSearchMetric', 'formatSearchMetric') + '\n\n' + source.slice(last);
  return replaceCount(source, 'aria-label="Search Result Teaser"', 'aria-label="Фрагмент совпадения"', 1);
}

export function localizeBookScript(source, locale) {
  // Nested SVG targets do not own the button's accessible label.
  source = replaceCount(source, "this.setAttribute('aria-label', e.target.title);", "this.setAttribute('aria-label', this.title);", 2);
  if (locale !== 'ru') return source;
  for (const [previous, next, count] of [
    ['Show hidden lines', 'Показать скрытые строки', 4],
    ['Hide lines', 'Скрыть строки', 2],
    ['Copy to clipboard', 'Копировать в буфер обмена', 2],
    ['Copied!', 'Скопировано!', 1],
    ['Clipboard error!', 'Не удалось скопировать', 1],
  ]) source = replaceCount(source, previous, next, count);
  return source;
}

export function localizeReaderChrome(html, locale) {
  const first = html.indexOf('<main>'), last = html.indexOf('</main>');
  if (first < 0 && last < 0) return html; // Native standalone TOC contains no article chrome.
  if (first < 0 || last < first || html.indexOf('<main>', first + 1) !== -1 || html.indexOf('</main>', last + 1) !== -1) throw new Error('Pinned reader article boundary changed');
  let before = html.slice(0, first), after = html.slice(last + 7);
  before = replaceCount(before, 'aria-describedby="searchresults-header"', 'aria-describedby="mdbook-searchresults-header"', 1);
  before = replaceCount(before, 'class="sidebar-iframe-outer"', `class="sidebar-iframe-outer" title="${locale === 'ru' ? 'Оглавление' : 'Table of contents'}"`, 1);
  if (locale === 'ru') {
    for (const [previous, next, count] of [
      ['Keyboard shortcuts', 'Горячие клавиши', 1],
      ['Press <kbd>←</kbd> or <kbd>→</kbd> to navigate between chapters', 'Нажмите <kbd>←</kbd> или <kbd>→</kbd> для перехода между главами', 1],
      ['Press <kbd>S</kbd> or <kbd>/</kbd> to search in the book', 'Нажмите <kbd>S</kbd> или <kbd>/</kbd> для поиска', 1],
      ['Press <kbd>?</kbd> to show this help', 'Нажмите <kbd>?</kbd>, чтобы открыть справку', 1],
      ['Press <kbd>Esc</kbd> to hide this help', 'Нажмите <kbd>Esc</kbd>, чтобы закрыть справку', 1],
      ['Table of contents', 'Оглавление', 1],
      ['Toggle Table of Contents', 'Показать или скрыть оглавление', 2],
      ['Change theme', 'Выбрать тему', 2],
      ['aria-label="Themes"', 'aria-label="Темы оформления"', 1],
      ['>Auto</button>', '>Системная</button>', 1],
      ['>Light</button>', '>Светлая</button>', 1],
      ['>Coal</button>', '>Угольная</button>', 1],
      ['>Navy</button>', '>Тёмно-синяя</button>', 1],
      ['Search (`/`)', 'Поиск (/)', 1],
      ['Toggle Searchbar', 'Показать или скрыть поиск', 1],
      ['Print this book', 'Печать всей книги', 2],
      ['Search this book ...', 'Поиск по материалам…', 1],
    ]) before = replaceCount(before, previous, next, count);
    after = replaceCount(after, 'Page navigation', 'Переходы между главами', 2);
    after = replaceCount(after, 'Previous chapter', 'Предыдущая глава', [0, 4]);
    after = replaceCount(after, 'Next chapter', 'Следующая глава', [0, 4]);
  }
  return before + html.slice(first, last + 7) + after;
}
