/**
 * Client-side plugin that emits [DEV:ROUTE] / [DEV:WINDOW] traces.
 *
 * Route changes go through the Nuxt router hook so they're tracked on
 * EVERY navigation app-wide, not just within the BPM layout (which has
 * its own route-change log in `layouts/bigpicture.vue`).
 *
 * Window events (resize / focus / blur / visibility) are always worth
 * tracing when chasing rendering bugs, especially on gamescope where
 * the compositor can aggressively unfocus the window.
 */

import { devLog } from "~/composables/dev-mode";

export default defineNuxtPlugin((nuxtApp) => {
  if (!import.meta.client) return;

  // ── Route changes ────────────────────────────────────────────────────
  const router = nuxtApp.$router as any;
  if (router && typeof router.beforeEach === "function") {
    router.beforeEach((to: any, from: any) => {
      devLog("route", `${from?.fullPath ?? "?"} -> ${to?.fullPath ?? "?"}`);
    });
    router.afterEach((to: any, from: any, failure?: any) => {
      if (failure) {
        devLog(
          "route",
          `after ${from?.fullPath ?? "?"} -> ${to?.fullPath ?? "?"} FAILED ${stringifyFailure(failure)}`,
        );
      }
    });
  }

  // ── Window events ────────────────────────────────────────────────────
  window.addEventListener("resize", () => {
    devLog("window", `resize ${window.innerWidth}x${window.innerHeight}`);
  });
  window.addEventListener("focus", () => {
    devLog("window", "focus");
  });
  window.addEventListener("blur", () => {
    devLog("window", "blur");
  });
  document.addEventListener("visibilitychange", () => {
    devLog("window", `visibility=${document.visibilityState}`);
  });
});

function stringifyFailure(f: unknown): string {
  if (!f) return "";
  if (typeof f === "string") return f;
  try {
    return JSON.stringify(f);
  } catch {
    return String(f);
  }
}
