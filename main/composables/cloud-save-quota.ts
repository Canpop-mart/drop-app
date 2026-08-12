/**
 * Rendering the cloud-save quota, in the one place all four surfaces share.
 *
 * The number shows up in the settings page, the Big Picture settings section,
 * the per-game panel and its Big Picture twin. Four copies of the formatter is
 * four chances for the same bytes to be printed two different ways, and the
 * user is meant to read this figure as the same figure everywhere.
 *
 * The byte formatter deliberately matches `formatBytes` in the server's
 * `internal/cloudsaves/quota.ts`. The server writes the same number into its
 * rejection message ("Save quota exceeded: would be 1.20 GiB / 1.00 GiB"), and
 * a client that rounded differently would look like it was talking about
 * something else.
 *
 * Colour is left to the caller: the desktop surfaces use Tailwind classes and
 * Big Picture uses its theme's CSS variables.
 */
import type { CloudSaveQuota } from "~/composables/use-server-api";

export function formatCloudSaveBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

/**
 * Fill percentage, 0 to 100. A limit of zero reads as 100: an administrator
 * set the cap to nothing, so there is no room, and drawing an empty bar would
 * say the opposite.
 */
export function cloudSaveQuotaPercent(quota: CloudSaveQuota | null): number {
  if (!quota || quota.limitBytes <= 0) return 100;
  return Math.min(100, Math.round((quota.usedBytes / quota.limitBytes) * 100));
}

/** "1.20 GiB of 5.00 GiB used", or an empty string when the quota is unknown. */
export function cloudSaveQuotaLine(quota: CloudSaveQuota | null): string {
  if (!quota) return "";
  return `${formatCloudSaveBytes(quota.usedBytes)} of ${formatCloudSaveBytes(
    quota.limitBytes,
  )} used`;
}
