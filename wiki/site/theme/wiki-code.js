/*
The corpus writes protocol procedures as pseudocode, which the bundled highlighter does not know. This
script registers that language and applies it to the blocks already on the page; a page without any
pseudocode does not load it at all.
*/
(() => {
  const selector = 'pre code.language-pseudocode, pre code.language-pseudo';
  const blocks = [...document.querySelectorAll(selector)];
  if (!blocks.length || !window.hljs) return;
  const hljs = window.hljs;

  hljs.registerLanguage('pseudocode', () => ({
    name: 'DEOS pseudocode',
    aliases: ['pseudo'],
    keywords: {
      keyword: 'IF THEN ELSE ELSEIF WHEN OTHERWISE FOR EACH IN WHILE DO RETURN MATCH ADMIT REJECT KEEP COMMIT EMIT ASSERT REQUIRE OPEN CLOSE READ WRITE STEP PHASE SET LET AND OR NOT',
      built_in: 'Actor Step Pipeline Trigger Continuation Deadline Weight RefTime ProofSize Service Custody Cycle Route Quote Candidate Burn Mint Transfer Stake Governance Treasury Bucket Anchor Router Oracle',
      literal: 'true false None Some Ok Err',
    },
    contains: [
      hljs.C_LINE_COMMENT_MODE,
      hljs.QUOTE_STRING_MODE,
      hljs.C_NUMBER_MODE,
      { className: 'comment', begin: /--/, end: /$/ },
      { className: 'comment', begin: /#/, end: /$/ },
      { className: 'operator', begin: /→|->|<-|:=|==|!=|<=|>=|\+|-(?!-)/ },
      // A capitalised name with a lowercase tail reads as a type or an owner, never as a keyword,
      // because a keyword here is written in capitals.
      { className: 'type', begin: /\b[A-Z][a-z]\w*\b/ },
    ],
  }));

  const highlight = hljs.highlightElement ?? hljs.highlightBlock;
  for (const block of blocks) {
    block.classList.remove('language-pseudocode', 'language-pseudo');
    block.classList.add('language-pseudocode');
    highlight.call(hljs, block);
  }
})();
