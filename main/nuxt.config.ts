// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: "2024-04-03",

  postcss: {
    plugins: {
      tailwindcss: {},
      autoprefixer: {},
    },
  },

  css: ["~/assets/main.scss"],

  ssr: false,
  devtools: false,

  // Nuxt telemetry prompts for consent on first run via consola, which
  // requires a TTY. `pnpm tauri dev` spawns the dev server through
  // beforeDevCommand without one — the prompt crashes with
  // ERR_TTY_INIT_FAILED before Nuxt can boot. Opt out explicitly so the
  // prompt is never attempted.
  telemetry: false,

  extends: [["../libs/drop-base"]],

  app: {
    baseURL: "/main",
    head: {
      meta: [
        // Ensure consistent viewport scaling across desktop, Gamescope, and
        // docked modes. Without this, WebKitGTK defaults to a ~980px virtual
        // viewport in some compositor contexts, making BPM look "zoomed out."
        {
          name: "viewport",
          content:
            "width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no",
        },
      ],
    },
  },
});
