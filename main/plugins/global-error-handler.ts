export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.hook("vue:error", (error, instance, info) => {
    console.error("[Global Error]", error, info);
  });

  if (import.meta.client) {
    window.addEventListener("unhandledrejection", (event) => {
      console.error("[Unhandled Promise Rejection]", event.reason);
    });
  }
});
