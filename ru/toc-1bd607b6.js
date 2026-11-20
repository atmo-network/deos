// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="index.html">Содержание</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/index.html">Вики DEOS</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><span>С чего начать</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/start-here.html">С чего начать</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/deos-in-60-seconds.html">DEOS за 60 секунд</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/partner-pitch.html">Предложение для партнёров</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/executive-summary.html">Краткое резюме</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/deos-framework.html">Обзор фреймворка DEOS</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/first-steps.html">Первые шаги</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/glossary/core-terms.html">Базовые термины</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/reading-paths.html">Маршруты чтения</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/faq/newcomer-faq.html">Вопросы новичков</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Ключевые концепции</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/tmctol-standard.html">Стандарт TMCTOL</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/token-driven-automation.html">Токен-управляемая автоматизация</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/read-model-split.html">Разделение данных для чтения</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/governance-domains.html">Домены управления</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/generated-wiki.html">Собранная вики</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/domain-map.html">Карта доменов</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/end-to-end-flows.html">Сквозные сценарии</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/architecture-diagrams.html">Архитектурные схемы</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/token-surfaces.html">Роли токенов</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/builder-economy.html">Экономика созидателей $BLDR</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/economic-thresholds.html">Экономические пороги</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/parachain-context.html">DEOS в экосистеме парачейнов</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/tol-bucket-scenarios.html">Сценарии корзин TOL</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/invariant-map.html">Карта инвариантов и угроз</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/economic-claim-levels.html">Уровни экономических утверждений</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Среда исполнения и продукт</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/actor-system.html">Система Actors</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/actor.html">AA-Актор</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/router.html">DEOS Router</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/typed-observations.html">Типизированные наблюдения</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/token-minting-curve.html">Token Minting Curve</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/governance.html">Управление</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/asset-identity.html">Идентичность активов</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/randomness-strategy.html">Стратегия использования случайности</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/runtime-patterns.html">Принципы среды исполнения</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/routing-and-minting-loop.html">Контур маршрутизации и эмиссии</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/staking.html">Стейкинг</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/reference-client.html">Эталонный клиент</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Техническая реализация</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/implementation/repository-structure.html">Структура репозитория</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/implementation/tech-stack.html">Технологический стек</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Математические модели</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/math/tmctol-formulas.html">Формулы TMCTOL</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Практическое использование</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/usage/scripts-layer.html">Слой скриптов</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/usage/forking-deos.html">Создание форка DEOS</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Разработка</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/development/three-layer-validation.html">Трёхуровневая проверка</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/development/status.html">Статус разработки</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Философия и ориентация</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/comparisons/physics-vs-politics.html">Экономическая физика прежде политики</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Сообщество и управление</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/community/contributing.html">Руководство участника</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);

