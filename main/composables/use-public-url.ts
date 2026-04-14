/**
 * Resolves a path from public/ to include the Nuxt baseURL prefix.
 * In dev the base is "/main", so "/img/foo.png" → "/main/img/foo.png".
 * Without this, Tauri's asset resolver can't find the file because
 * frontendDist is ".output" but files live under ".output/main/".
 */
export const usePublicUrl = (path: string): string => {
  const base = useRuntimeConfig().app.baseURL ?? "/";
  // Ensure exactly one slash between base and path
  const cleanBase = base.endsWith("/") ? base.slice(0, -1) : base;
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  return `${cleanBase}${cleanPath}`;
};
