/** Cloud save conflict — one file with different versions on local and cloud. */
export interface SaveConflict {
  filename: string;
  saveType: string;
  localHash: string;
  localSize: number;
  localModifiedAt: number;
  cloudId: string;
  cloudHash: string;
  cloudSize: number;
  cloudModifiedAt: string;
  cloudUploadedFrom: string;
}

/**
 * Payload of the global `save_sync_conflict` event.
 *
 * Global rather than one topic per game id: a launch can be started from the
 * library page, the Big Picture detail page, or the Big Picture grid's
 * quick-launch, and only an app-level listener hears all three.
 */
export interface SaveConflictEvent {
  gameId: string;
  conflicts: SaveConflict[];
  /** Seconds the Rust side will wait before giving up and syncing nothing. */
  timeoutSecs: number;
}

/**
 * Payload of the `save_sync_error` event: a sync step that failed where the
 * user is the only one who can do anything about it.
 */
export interface SaveSyncError {
  gameId: string;
  /** "check" | "upload" | "download" | "write" | "conflict" */
  phase: string;
  message: string;
  retryable: boolean;
}

/**
 * Payload of the `save_sync_complete` event: saves actually moved.
 *
 * Its own topic rather than a flag on `save_sync_error`, because that event
 * already has one listener and that listener's whole job is to put a modal on
 * screen. A backup that worked has no business interrupting anybody.
 */
export interface SaveSyncComplete {
  gameId: string;
  /** Display name, or null for a game Drop has never fetched metadata for. */
  gameName: string | null;
  /** "upload" after a session, "download" for the pre-launch restore. */
  phase: string;
  count: number;
  /** Composed on the Rust side, so the copy lives in one language. */
  message: string;
}

/** What `backup_saves` returns: how many landed, and why the rest did not. */
export interface BackupResult {
  uploaded: number;
  errors: string[];
}
