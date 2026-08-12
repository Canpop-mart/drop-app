/**
 * One poll of the account's streaming sessions, shared by every surface that
 * needs it.
 *
 * `streaming_list_sessions` used to be polled by each consumer on its own 15s
 * timer, and a BPM game page mounts two of them at once (the stream button and
 * the page's streaming composable), so one idle game page asked the server for
 * the same list twice every fifteen seconds. The poller here is module-level:
 * however many components subscribe, there is exactly one interval and one
 * request, and it stops the moment the last subscriber leaves.
 *
 * Subscribers that need to react faster than the idle cadence (waiting for a
 * host to accept a request) ask for their own interval; the fastest request
 * wins while it is held.
 */

import { useStreaming, type StreamingSession } from "~/composables/useStreaming";

/** Idle cadence. Fast enough that a session appearing feels immediate. */
const IDLE_INTERVAL_MS = 15_000;

const sessions = ref<StreamingSession[]>([]);

/** Cadence each live subscriber is asking for, in ms. */
const requestedIntervals = new Map<symbol, number>();
let timer: ReturnType<typeof setInterval> | null = null;
/** The interval `timer` currently runs at; 0 when there is no timer. */
let runningInterval = 0;

// One `useStreaming()` instance for the module — it only wraps `invoke`, and
// remaking it per poll would allocate refs nobody reads.
let streaming: ReturnType<typeof useStreaming> | null = null;
function streamingApi() {
  return (streaming ??= useStreaming());
}

/**
 * Fetch the session list now and publish it to every subscriber. Never throws:
 * `listRemoteSessions` already turns a failed invoke into an empty list.
 */
export async function refreshRemoteSessions(): Promise<StreamingSession[]> {
  const next = await streamingApi().listRemoteSessions();
  sessions.value = next;
  return next;
}

function retimeIfNeeded() {
  let wanted = 0;
  for (const ms of requestedIntervals.values()) {
    if (wanted === 0 || ms < wanted) wanted = ms;
  }
  if (wanted === runningInterval) return;
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  runningInterval = wanted;
  if (wanted > 0) {
    timer = setInterval(() => void refreshRemoteSessions(), wanted);
  }
}

export function useRemoteSessions() {
  const key = Symbol("remote-sessions-subscriber");

  /** Join the shared poll and fetch once immediately. */
  async function start(intervalMs: number = IDLE_INTERVAL_MS) {
    requestedIntervals.set(key, intervalMs);
    retimeIfNeeded();
    await refreshRemoteSessions();
  }

  /**
   * Change this subscriber's cadence, e.g. down to 3s while waiting on a host.
   * Pass nothing to go back to the idle cadence.
   */
  function setPollInterval(intervalMs: number = IDLE_INTERVAL_MS) {
    if (!requestedIntervals.has(key)) return;
    requestedIntervals.set(key, intervalMs);
    retimeIfNeeded();
  }

  /** Leave the shared poll. The interval stops with the last subscriber. */
  function stop() {
    requestedIntervals.delete(key);
    retimeIfNeeded();
  }

  // Components that forget to call `stop()` still release their slot, so a
  // page that navigated away can never hold the poll open.
  if (getCurrentScope()) onScopeDispose(stop);

  return {
    sessions,
    start,
    stop,
    setPollInterval,
    refresh: refreshRemoteSessions,
    IDLE_INTERVAL_MS,
  };
}
