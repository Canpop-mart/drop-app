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

/** The full event payload emitted by the Rust side. */
export interface SaveConflictEvent {
  gameId: string;
  conflicts: SaveConflict[];
}
