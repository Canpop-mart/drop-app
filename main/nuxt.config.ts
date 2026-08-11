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

  // drop-base is a git submodule with its own node_modules, so it resolves its
  // own vue and its own @headlessui/vue. Rollup honours that and inlines a
  // SECOND Vue runtime next to the layer's ModalTemplate. Vue's reactivity is
  // per-copy, so a watchEffect created by one runtime never tracks a props
  // proxy created by the other: HeadlessUI's TransitionRoot seeds its state to
  // "hidden" (every modal mounts closed) and never sees `show` flip, so it
  // renders a comment node forever. That silently killed EVERY modal in
  // packaged builds - Configure, launch options, new collection - with no error
  // and no warning. Vite's dev optimizer pre-bundles a single copy, which is why
  // `tauri dev` worked and only the shipped build broke.
  vite: {
    resolve: {
      dedupe: ["vue", "@headlessui/vue", "@heroicons/vue"],
    },
  },

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
