/**
 * Controller Bridge — injected into server:// iframes so that
 * gamepad events from the Tauri parent window can drive basic
 * spatial navigation inside the embedded page.
 *
 * Listens for `postMessage` events from the Big Picture shell
 * and translates them into focus movement and clicks.
 */
(function () {
  "use strict";

  // ── Focusable selector ──────────────────────────────────────────────────
  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  const FOCUS_CLASS = "bp-iframe-focused";

  // Inject minimal focus styles
  const style = document.createElement("style");
  style.textContent = `
    .${FOCUS_CLASS} {
      outline: none !important;
      box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.8), 0 0 16px rgba(59, 130, 246, 0.25) !important;
      position: relative;
      z-index: 9999;
    }
  `;
  document.head.appendChild(style);

  let currentFocused = null;

  function getFocusableElements() {
    return Array.from(document.querySelectorAll(FOCUSABLE)).filter(
      function (el) {
        const rect = el.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0;
      },
    );
  }

  function getCenter(el) {
    const rect = el.getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  }

  function setFocus(el) {
    if (currentFocused) {
      currentFocused.classList.remove(FOCUS_CLASS);
    }
    currentFocused = el;
    if (el) {
      el.classList.add(FOCUS_CLASS);
      el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }

  function navigate(direction) {
    const elements = getFocusableElements();
    if (elements.length === 0) return;

    if (!currentFocused || !document.body.contains(currentFocused)) {
      setFocus(elements[0]);
      return;
    }

    const from = getCenter(currentFocused);
    var best = null;
    var bestScore = Infinity;

    for (var i = 0; i < elements.length; i++) {
      var el = elements[i];
      if (el === currentFocused) continue;

      var to = getCenter(el);
      var dx = to.x - from.x;
      var dy = to.y - from.y;

      var inDir =
        (direction === "up" && dy < -10) ||
        (direction === "down" && dy > 10) ||
        (direction === "left" && dx < -10) ||
        (direction === "right" && dx > 10);

      if (!inDir) continue;

      var primary =
        direction === "up" || direction === "down"
          ? Math.abs(dy)
          : Math.abs(dx);
      var secondary =
        direction === "up" || direction === "down"
          ? Math.abs(dx)
          : Math.abs(dy);

      var score = primary + secondary * 2;
      if (score < bestScore) {
        bestScore = score;
        best = el;
      }
    }

    if (best) setFocus(best);
  }

  // ── Message handler ─────────────────────────────────────────────────────
  window.addEventListener("message", function (event) {
    if (!event.data || event.data.type !== "bp-controller") return;

    var action = event.data.action;

    if (action === "navigate") {
      navigate(event.data.direction);
    } else if (action === "select") {
      if (currentFocused) {
        currentFocused.click();
        // Also focus for inputs so on-screen keyboard can work
        if (
          currentFocused.tagName === "INPUT" ||
          currentFocused.tagName === "TEXTAREA" ||
          currentFocused.tagName === "SELECT"
        ) {
          currentFocused.focus();
        }
      }
    } else if (action === "back") {
      window.history.back();
    } else if (action === "scroll") {
      window.scrollBy(0, event.data.amount || 0);
    }
  });

  // Notify parent that bridge is loaded
  try {
    window.parent.postMessage({ type: "bp-bridge-ready" }, "*");
  } catch (e) {
    // Swallow cross-origin errors
  }
})();
