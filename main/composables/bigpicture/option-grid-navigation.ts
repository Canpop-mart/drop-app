/** Navigate the visible options columns without losing an item's identity. */
export function moveOptionGrid(columns: readonly (readonly string[])[], id: string | null,
  direction: "up" | "down" | "left" | "right"): string | null {
  const live = columns.filter((column) => column.length > 0);
  if (!live.length) return null;
  let col = live.findIndex((column) => id !== null && column.includes(id));
  if (col < 0) return live[0][0];
  const row = live[col].indexOf(id!);
  if (direction === "left" || direction === "right") {
    col = Math.max(0, Math.min(live.length - 1, col + (direction === "left" ? -1 : 1)));
    return live[col][Math.min(row, live[col].length - 1)];
  }
  return live[col][Math.max(0, Math.min(live[col].length - 1, row + (direction === "up" ? -1 : 1)))];
}
