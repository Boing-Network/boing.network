/**
 * Boing motion runtime — pairs with styles/motion.css.
 *
 * Layout.astro adds `motion-ok` to <html> from an inline head script when motion is allowed,
 * so reveal targets are hidden from the first paint. This module then:
 *   1. reveals [data-reveal] elements and [data-reveal-stagger] children as they scroll in,
 *      staggering whatever enters the viewport together;
 *   2. counts [data-count-to] numbers up once revealed;
 *   3. drives the cursor spotlight + optional 3D tilt on .fx-lift (fine pointers only);
 *   4. adds pointer parallax to elements marked [data-parallax-root].
 */

declare global {
  interface Window {
    __boingMotion?: boolean;
  }
}

const root = document.documentElement;
const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const finePointer = window.matchMedia('(hover: hover) and (pointer: fine)').matches;
window.__boingMotion = true;

/* ---------- 1. Scroll reveals ---------- */
function revealTargets(): HTMLElement[] {
  const list: HTMLElement[] = [];
  document.querySelectorAll<HTMLElement>('[data-reveal]').forEach((el) => list.push(el));
  document.querySelectorAll<HTMLElement>('[data-reveal-stagger]').forEach((parent) => {
    Array.from(parent.children).forEach((c) => list.push(c as HTMLElement));
  });
  return list;
}

function onRevealed(el: HTMLElement) {
  el.querySelectorAll<HTMLElement>('[data-count-to]').forEach(countUp);
  if (el.hasAttribute('data-count-to')) countUp(el);
}

let watch: (el: HTMLElement) => void;
if (!root.classList.contains('motion-ok') || !('IntersectionObserver' in window)) {
  root.classList.remove('motion-ok');
  watch = (el) => el.classList.add('is-in');
} else {
  const io = new IntersectionObserver(
    (entries) => {
      const entering = entries
        .filter((e) => e.isIntersecting)
        .map((e) => e.target as HTMLElement)
        // Stagger in document order, whatever order the observer reports.
        .sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
      entering.forEach((el, i) => {
        el.style.setProperty('--i', String(Math.min(i, 8)));
        el.classList.add('is-in');
        io.unobserve(el);
        onRevealed(el);
      });
    },
    { rootMargin: '0px 0px -6% 0px', threshold: 0.08 }
  );
  watch = (el) => io.observe(el);
}
revealTargets().forEach((el) => watch(el));

// Stagger lists that are re-rendered client-side (e.g. /downloads after the GitHub API answers):
// new children must be watched too, or the CSS would keep them hidden.
if ('MutationObserver' in window) {
  const mo = new MutationObserver((records) => {
    for (const r of records) {
      r.addedNodes.forEach((n) => {
        if (n instanceof HTMLElement && !n.classList.contains('is-in')) watch(n);
      });
    }
  });
  document.querySelectorAll('[data-reveal-stagger]').forEach((parent) => mo.observe(parent, { childList: true }));
}

/* ---------- 2. Count-up numbers ---------- */
function countUp(el: HTMLElement) {
  if (el.dataset.counted) return;
  el.dataset.counted = '1';
  const to = Number(el.dataset.countTo);
  if (reduce || !Number.isFinite(to)) return;
  const suffix = el.dataset.suffix ?? '';
  const decimals = Number(el.dataset.decimals ?? 0);
  const final = el.textContent;
  const dur = 1400;
  const start = performance.now();
  const tick = (now: number) => {
    const t = Math.min(1, (now - start) / dur);
    const eased = 1 - Math.pow(1 - t, 3);
    el.textContent = (to * eased).toFixed(decimals) + suffix;
    if (t < 1) requestAnimationFrame(tick);
    else el.textContent = final;
  };
  requestAnimationFrame(tick);
}

/* ---------- 3. Card spotlight + tilt ---------- */
if (!reduce && finePointer) {
  let active: HTMLElement | null = null;
  let pending: PointerEvent | null = null;
  let frame = 0;

  const reset = (el: HTMLElement) => {
    el.classList.remove('is-tilting');
    el.style.removeProperty('--rx');
    el.style.removeProperty('--ry');
  };

  const apply = () => {
    frame = 0;
    if (!active || !pending) return;
    const r = active.getBoundingClientRect();
    const x = (pending.clientX - r.left) / r.width;
    const y = (pending.clientY - r.top) / r.height;
    active.style.setProperty('--mx', `${(x * 100).toFixed(1)}%`);
    active.style.setProperty('--my', `${(y * 100).toFixed(1)}%`);
    if (active.hasAttribute('data-tilt')) {
      const max = Number(active.dataset.tilt) || 5;
      active.classList.add('is-tilting');
      active.style.setProperty('--rx', `${((x - 0.5) * max * 2).toFixed(2)}deg`);
      active.style.setProperty('--ry', `${((0.5 - y) * max * 1.4).toFixed(2)}deg`);
    }
  };

  document.addEventListener(
    'pointermove',
    (ev) => {
      if (ev.pointerType !== 'mouse') return;
      const card = (ev.target as Element | null)?.closest<HTMLElement>('.fx-lift') ?? null;
      if (active && active !== card) reset(active);
      active = card;
      if (!card) return;
      pending = ev;
      if (!frame) frame = requestAnimationFrame(apply);
    },
    { passive: true }
  );
  document.addEventListener('pointerleave', () => {
    if (active) reset(active);
    active = null;
  });
}

/* ---------- 4. Hero pointer parallax ---------- */
if (!reduce && finePointer) {
  document.querySelectorAll<HTMLElement>('[data-parallax-root]').forEach((hero) => {
    let frame = 0;
    let px = 0;
    let py = 0;
    hero.addEventListener(
      'pointermove',
      (ev) => {
        const r = hero.getBoundingClientRect();
        px = (ev.clientX - r.left) / r.width - 0.5;
        py = (ev.clientY - r.top) / r.height - 0.5;
        if (frame) return;
        frame = requestAnimationFrame(() => {
          frame = 0;
          hero.style.setProperty('--px', px.toFixed(3));
          hero.style.setProperty('--py', py.toFixed(3));
        });
      },
      { passive: true }
    );
    hero.addEventListener('pointerleave', () => {
      hero.style.setProperty('--px', '0');
      hero.style.setProperty('--py', '0');
    });
  });
}

export {};
