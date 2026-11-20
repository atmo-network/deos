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
        this.innerHTML = '<ol class="chapter"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="index.html">Contents</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/index.html">DEOS Wiki</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><span>Start Here</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/start-here.html">Start Here</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/deos-in-60-seconds.html">DEOS in 60 Seconds</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/partner-pitch.html">Partner Pitch</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/executive-summary.html">Executive Summary</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/deos-framework.html">DEOS Framework Overview</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/first-steps.html">First Steps</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/glossary/core-terms.html">Core Terms</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/getting-started/reading-paths.html">Reading Paths</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/faq/newcomer-faq.html">Newcomer FAQ</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Core Concepts</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/tmctol-standard.html">TMCTOL Standard</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/token-driven-automation.html">Token-Driven Automation</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/read-model-split.html">Read-Model Split</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/governance-domains.html">Governance Domains</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/generated-wiki.html">Generated Wiki</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/domain-map.html">Domain Map</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/end-to-end-flows.html">End-to-End Flows</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/architecture-diagrams.html">Architecture Diagrams</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/token-surfaces.html">Token Surfaces</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/builder-economy.html">$BLDR Builder Economy</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/economic-thresholds.html">Economic Thresholds</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/parachain-context.html">Parachain Context</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/tol-bucket-scenarios.html">TOL Bucket Scenarios</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/invariant-map.html">Invariant and Threat Map</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/economic-claim-levels.html">Economic Claim Levels</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Runtime and Product</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/actor-system.html">Actors System</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/actor.html">AA-Actor</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/router.html">DEOS Router</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/typed-observations.html">Typed Observations</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/token-minting-curve.html">Token Minting Curve</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/governance.html">Governance</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/asset-identity.html">Asset Identity</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/randomness-strategy.html">Randomness Strategy</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/runtime-patterns.html">Runtime Patterns</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/concepts/routing-and-minting-loop.html">Routing and Minting Loop</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/staking.html">Staking</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/overview/reference-client.html">Reference Client</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Technical Implementation</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/implementation/repository-structure.html">Repository Structure</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/implementation/tech-stack.html">Tech Stack</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Mathematical Models</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/math/tmctol-formulas.html">TMCTOL Formulas</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Practical Usage</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/usage/scripts-layer.html">Scripts Layer</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/usage/forking-deos.html">Forking DEOS</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Development</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/development/three-layer-validation.html">Three-Layer Validation</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/development/status.html">Development Status</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Philosophy and Orientation</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/comparisons/physics-vs-politics.html">Physics-First vs Politics-First</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><span>Community and Governance</span><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="deos/community/contributing.html">Contributing Guidelines</a></span></li></ol></li></ol>';
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

