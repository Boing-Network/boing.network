/**
 * Sitewide semantic link classification + click-to-copy for CLI/code.
 * Colors: docs cyan, PDF violet, apps teal; gold code uses toast feedback.
 */
(function () {
  'use strict';

  var APP_HOSTS = [
    'boing.express',
    'boing.finance',
    'boing.observer',
    'discord.com',
    'discord.gg',
    'twitter.com',
    'x.com',
    'npmjs.com',
  ];

  var SKIP_CLOSEST =
    '.site-header, .skip-link, .toc-nav, .toc-list, .sidebar-content, nav.social-icons, .pagination, [data-bn-skip-semantic]';
  var SKIP_SELF =
    '.btn, .button, .cta, .nav-cta, .nav-link, .toc-link, .sl-link-button, .site-title, .copy-btn';

  function copyText(text) {
    if (!text) return Promise.reject(new Error('empty'));
    var fallback = function () {
      return new Promise(function (resolve, reject) {
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.setAttribute('readonly', '');
        ta.style.position = 'fixed';
        ta.style.left = '-9999px';
        document.body.appendChild(ta);
        ta.select();
        try {
          document.execCommand('copy') ? resolve() : reject(new Error('copy failed'));
        } catch (err) {
          reject(err);
        } finally {
          document.body.removeChild(ta);
        }
      });
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).catch(function () {
        return fallback();
      });
    }
    return fallback();
  }

  var toastEl = null;
  var toastTimer = null;
  function showCopiedToast() {
    if (!toastEl) {
      toastEl = document.createElement('div');
      toastEl.className = 'bn-copy-toast docs-copy-toast';
      toastEl.setAttribute('role', 'status');
      toastEl.setAttribute('aria-live', 'polite');
      document.body.appendChild(toastEl);
    }
    toastEl.textContent = 'Copied to clipboard';
    toastEl.classList.add('is-visible');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () {
      toastEl.classList.remove('is-visible');
    }, 1600);
  }

  window.BoingShowCopiedToast = showCopiedToast;
  window.BoingCopyText = function (text) {
    return copyText(text).then(function () {
      showCopiedToast();
    });
  };

  function classifyHref(href) {
    var kind = 'docs';
    try {
      if (/\.pdf($|\?)/i.test(href) || href.indexOf('/pdfs/') !== -1) {
        kind = 'pdf';
      } else if (
        href.startsWith('#') ||
        href.startsWith('/#') ||
        (href.startsWith('/') && !href.startsWith('//'))
      ) {
        kind = 'docs';
      } else if (/^https?:\/\//i.test(href)) {
        var url = new URL(href, window.location.origin);
        var host = url.hostname.replace(/^www\./, '');
        var path = url.pathname;
        var isGithubDocs =
          (host === 'github.com' || host.endsWith('.github.com')) &&
          (/\.md($|\/)/i.test(path) ||
            /\/blob\/[^/]+\/docs\//i.test(path) ||
            /\/tree\/[^/]+\/docs(\/|$)/i.test(path));
        var isProductApp =
          host === 'boing.express' ||
          host === 'boing.finance' ||
          host === 'boing.observer' ||
          APP_HOSTS.some(function (h) {
            return host === h || host.endsWith('.' + h);
          }) ||
          host === 'github.com' ||
          host.endsWith('.github.com');
        if (isGithubDocs) kind = 'docs';
        else if (isProductApp) kind = 'app';
        else if (host === 'boing.network' || host === 'docs.boing.network') kind = 'docs';
        else kind = 'app';
      }
    } catch (_) {
      kind = 'docs';
    }
    return kind;
  }

  function shouldSkipAnchor(a) {
    if (!a || !a.getAttribute) return true;
    if (a.closest(SKIP_CLOSEST)) return true;
    if (a.matches(SKIP_SELF)) return true;
    if (a.getAttribute('role') === 'button' && a.classList.contains('btn')) return true;
    return false;
  }

  function classifyLinks(root) {
    var scope = root || document;
    var nodes = scope.querySelectorAll(
      'main a[href], .site-footer a[href], .sl-markdown-content a[href], .content a[href]'
    );
    nodes.forEach(function (a) {
      if (shouldSkipAnchor(a)) return;
      if (
        a.classList.contains('resource-list-item') ||
        a.classList.contains('resource-card')
      ) {
        a.classList.add('bn-link-pdf', 'doc-link-pdf');
        a.classList.remove('bn-link-docs', 'doc-link-docs', 'bn-link-app', 'doc-link-app');
        return;
      }
      var href = a.getAttribute('href') || '';
      var kind = classifyHref(href);
      a.classList.remove(
        'bn-link-docs',
        'bn-link-pdf',
        'bn-link-app',
        'doc-link-docs',
        'doc-link-pdf',
        'doc-link-app'
      );
      a.classList.add('bn-link-' + kind, 'doc-link-' + kind);
    });
  }

  function enhanceCodeBlocks(root) {
    var scope = root || document;
    var selectors = [
      'main pre',
      '.docs-content pre',
      '.sl-markdown-content pre',
      '.content pre',
    ];
    scope.querySelectorAll(selectors.join(',')).forEach(function (pre) {
      if (pre.closest('.code-block-wrapper, .bn-code-block-wrapper, .copy-wrapper, .expressive-code')) {
        return;
      }
      if (pre.dataset.bnCopyReady) return;
      pre.dataset.bnCopyReady = '1';
      var wrapper = document.createElement('div');
      wrapper.className = 'bn-code-block-wrapper code-block-wrapper doc-copyable';
      wrapper.setAttribute('role', 'button');
      wrapper.setAttribute('tabindex', '0');
      wrapper.setAttribute('title', 'Click to copy');
      wrapper.setAttribute('aria-label', 'Copy command');
      if (!pre.parentNode) return;
      pre.parentNode.insertBefore(wrapper, pre);
      wrapper.appendChild(pre);
      wrapper.dataset.copyText = (pre.textContent || '').trim();

      function copyBlock(e) {
        if (e) e.preventDefault();
        var text = wrapper.dataset.copyText || (pre.textContent || '').trim();
        if (!text) return;
        copyText(text)
          .then(showCopiedToast)
          .catch(function () {});
      }

      wrapper.addEventListener('click', function (e) {
        if (e.target.closest('a, button, .copy-btn')) return;
        copyBlock(e);
      });
      wrapper.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          copyBlock(e);
        }
      });
    });
  }

  function enhanceInlineCode(root) {
    var scope = root || document;
    var selectors = [
      'main code',
      '.docs-content code',
      '.sl-markdown-content code',
      '.content code',
    ];
    scope.querySelectorAll(selectors.join(',')).forEach(function (code) {
      if (code.closest('a, pre, .copy-wrapper, button')) return;
      if (code.classList.contains('bn-copyable') || code.classList.contains('doc-copyable')) return;
      var inlineText = (code.textContent || '').trim();
      if (!inlineText) return;
      // Skip huge blobs (likely not a “command”)
      if (inlineText.length > 240) return;
      code.classList.add('bn-copyable', 'bn-copyable-inline', 'doc-copyable', 'doc-copyable-inline');
      code.dataset.copyText = inlineText;
      code.setAttribute('role', 'button');
      code.setAttribute('tabindex', '0');
      code.setAttribute('title', 'Click to copy');
      code.setAttribute('aria-label', 'Copy ' + inlineText);

      function copyInline(e) {
        e.preventDefault();
        e.stopPropagation();
        var text = code.dataset.copyText || '';
        if (!text) return;
        copyText(text)
          .then(showCopiedToast)
          .catch(function () {});
      }

      code.addEventListener('click', copyInline);
      code.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' || e.key === ' ') copyInline(e);
      });
    });
  }

  function init() {
    classifyLinks();
    enhanceCodeBlocks();
    enhanceInlineCode();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  // Astro/Starlight client navigations
  document.addEventListener('astro:page-load', init);
})();
