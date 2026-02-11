/// CSS injected to disable animations, transitions, pointer events, and carets.
pub(crate) const DISABLE_ANIMATIONS_CSS: &str = r#"
*,
*::before,
*::after {
  transition: none !important;
  animation: none !important;
}
* {
  pointer-events: none !important;
}
* {
  caret-color: transparent !important;
}
"#;

/// JavaScript that waits for page readiness:
/// 1. Fonts loaded (document.fonts.ready)
/// 2. DOM stable (no mutations for 100ms)
///
/// All with a 10s timeout.
pub(crate) const WAIT_FOR_READY_JS: &str = r#"
(function waitForReady() {
    return new Promise((resolve, reject) => {
        const TIMEOUT = 10000;
        const DOM_SETTLE_MS = 100;

        const timer = setTimeout(() => {
            reject(new Error('Ready detection timed out after 10s'));
        }, TIMEOUT);

        const fontsReady = document.fonts.ready;

        const domStable = new Promise((res) => {
            let settleTimer = null;
            const observer = new MutationObserver(() => {
                if (settleTimer) clearTimeout(settleTimer);
                settleTimer = setTimeout(() => {
                    observer.disconnect();
                    res();
                }, DOM_SETTLE_MS);
            });
            observer.observe(document.documentElement, {
                childList: true,
                subtree: true,
                attributes: true,
                characterData: true,
            });
            // If DOM is already stable, resolve after settle period
            settleTimer = setTimeout(() => {
                observer.disconnect();
                res();
            }, DOM_SETTLE_MS);
        });

        Promise.all([fontsReady, domStable]).then(() => {
            clearTimeout(timer);
            resolve('ready');
        }).catch((err) => {
            clearTimeout(timer);
            reject(err);
        });
    });
})()
"#;

/// JavaScript to inject a <style> element with the given CSS.
pub(crate) const INJECT_CSS_JS_TEMPLATE: &str = r#"
(function() {
    const style = document.createElement('style');
    style.textContent = `CSS_PLACEHOLDER`;
    document.head.appendChild(style);
})()
"#;

/// JavaScript that finishes or cancels in-progress animations via the Web
/// Animations API. Complements CSS injection (which prevents new CSS
/// animations) by handling JS-driven animations (framer-motion, GSAP, etc.).
///
/// - Finite animations are finished (jumped to end state)
/// - Infinite animations are cancelled (removed)
pub(crate) const FINISH_ANIMATIONS_JS: &str = r#"
(function() {
    document.getAnimations().forEach(function(a) {
        try {
            var timing = a.effect && a.effect.getComputedTiming && a.effect.getComputedTiming();
            if (timing && Number.isFinite(timing.endTime)) {
                a.finish();
            } else {
                a.cancel();
            }
        } catch(e) {}
    });
})()
"#;

/// Poll for the story root selector to exist with non-zero dimensions (100ms interval, 10s timeout).
pub(crate) const WAIT_FOR_STORY_ROOT_JS: &str = r#"
(function waitForStoryRoot() {
    return new Promise(function(resolve, reject) {
        var TIMEOUT = 10000;
        var INTERVAL = 100;
        var selector = '#storybook-root > *, #root > *';
        var timer = setTimeout(function() {
            reject(new Error('Story root selector "' + selector + '" not found or has zero dimensions after 10s'));
        }, TIMEOUT);
        function check() {
            var el = document.querySelector(selector);
            if (el) {
                var rect = el.getBoundingClientRect();
                if (rect.width > 0 && rect.height > 0) {
                    clearTimeout(timer);
                    resolve('found');
                    return;
                }
            }
            setTimeout(check, INTERVAL);
        }
        check();
    });
})()
"#;

/// Visible-child-union walk of Storybook root container.
///
/// Walks visible children of `#storybook-root` or `#root` and unions their
/// rects. Catches absolutely-positioned children that overflow body.
/// Falls back to body rect if no root container found.
pub(crate) const GET_STORY_ROOT_BOUNDS_JS: &str = r#"
(function() {
    var selector = '#storybook-root > *, #root > *';

    function hasOverflow(el) {
        var s = window.getComputedStyle(el);
        var vals = ['auto', 'hidden', 'scroll'];
        return vals.indexOf(s.overflowY) !== -1 ||
               vals.indexOf(s.overflowX) !== -1 ||
               vals.indexOf(s.overflow) !== -1;
    }

    function hasFixedPosition(el) {
        return window.getComputedStyle(el).position === 'fixed';
    }

    function isElementHiddenByOverflow(el, ctx) {
        function isOutOfBounds() {
            try {
                var er = el.getBoundingClientRect();
                var cr = ctx.hasParentOverflowHidden.getBoundingClientRect();
                return er.top < cr.top || er.bottom > cr.bottom ||
                       er.left < cr.left || er.right > cr.right;
            } catch(e) { return false; }
        }
        if (hasFixedPosition(el)) return false;
        if (ctx.parentNotVisible) return true;
        if (ctx.hasParentFixedPosition && ctx.hasParentOverflowHidden &&
            ctx.hasParentFixedPosition === ctx.hasParentOverflowHidden)
            return isOutOfBounds();
        if (ctx.hasParentFixedPosition && ctx.hasParentOverflowHidden &&
            ctx.hasParentOverflowHidden !== ctx.hasParentFixedPosition &&
            ctx.hasParentOverflowHidden.contains(ctx.hasParentFixedPosition))
            return false;
        if (ctx.hasParentOverflowHidden) return isOutOfBounds();
        return false;
    }

    function isVisible(el) {
        var s = window.getComputedStyle(el);
        return !(s.visibility === 'hidden' || s.display === 'none' ||
                 s.opacity === '0' ||
                 ((s.width === '0px' || s.height === '0px') && s.padding === '0px'));
    }

    var elements = [];

    function walk(el, ctx) {
        if (!el) return;
        var ignoreOverflow = el.parentElement === ctx.root && hasOverflow(ctx.root);
        var hidden = ignoreOverflow ? false :
            isElementHiddenByOverflow(el, ctx);
        if (isVisible(el) && !ctx.isRoot && !hidden) {
            elements.push(el);
        }
        for (var node = el.firstChild; node; node = node.nextSibling) {
            if (node.nodeType === 1) {
                walk(node, {
                    root: ctx.root,
                    isRoot: false,
                    parentNotVisible: hidden,
                    hasParentFixedPosition: hasFixedPosition(el) ? el : ctx.hasParentFixedPosition,
                    hasParentOverflowHidden: hasOverflow(el) ? el : ctx.hasParentOverflowHidden,
                });
            }
        }
    }

    // Find root: query selector children, get their parents, pick deepest.
    var roots = Array.from(document.querySelectorAll(selector))
        .map(function(e) { return e.parentElement; });
    var root = null;
    if (roots.length === 1) {
        root = roots[0];
    } else {
        root = roots.reduce(function(r, n) {
            if (!r) return n;
            return (r.contains(n) && r !== n) ? n : r;
        }, null);
    }

    if (!root || !root.children.length) {
        // Fall back to body rect
        var br = document.body.getBoundingClientRect();
        return JSON.stringify({ x: br.x, y: br.y, width: br.width, height: br.height });
    }

    walk(root, {
        isRoot: true,
        root: root,
        hasParentOverflowHidden: null,
        hasParentFixedPosition: null,
        parentNotVisible: false,
    });

    if (elements.length === 0) {
        var br = document.body.getBoundingClientRect();
        return JSON.stringify({ x: br.x, y: br.y, width: br.width, height: br.height });
    }

    var union = null;
    for (var i = 0; i < elements.length; i++) {
        var r = elements[i].getBoundingClientRect();
        if (!union) {
            union = { x: r.x, y: r.y, width: r.width, height: r.height };
        } else {
            var xMin = Math.min(union.x, r.x);
            var yMin = Math.min(union.y, r.y);
            var xMax = Math.max(union.x + union.width, r.x + r.width);
            var yMax = Math.max(union.y + union.height, r.y + r.height);
            union = { x: xMin, y: yMin, width: xMax - xMin, height: yMax - yMin };
        }
    }

    return JSON.stringify({
        x: Math.floor(union.x),
        y: Math.floor(union.y),
        width: Math.ceil(union.width),
        height: Math.ceil(union.height)
    });
})()
"#;
