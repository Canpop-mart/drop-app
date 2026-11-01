# Diagnostics: logs, crashes, the Steam Deck

Get the truth from logs before guessing. **Copy a log before doing anything that might rotate or truncate it.**

## Client logs
| What | Windows | Linux / Deck |
|---|---|---|
| Main log | `%APPDATA%\drop\drop.log` | `~/.local/share/drop/drop.log` |
| Older logs | numbered archives beside it, e.g. `drop.0.log`, `drop.1.log` (rotates at 8 MiB, keeps 3; exact numbering unconfirmed) | same |
| Panics | `%APPDATA%\drop\crash-*.log` (names the panic site; often the whole answer) | same folder |
| Dev client | `%APPDATA%\drop-debug\...` | |
| RetroArch | `logs/` inside the emulator's version folder | same |

Bug reports attach only the live `drop.log`; right after a roll the history is in the archives, so attach those too.

Useful tags to grep for:
- `[LAUNCH]` and `launch_trace`: the exact command, arguments and working folder a game was started with.
- `[GBE-DIAG]`: whether the Goldberg achievement emulator actually loaded for a game.
- `[SAVE-SYNC]`: cloud-save decisions (download, upload, conflict).
- `[ZEROTIER]`, `[COOP]`: co-op rooms and the ZeroTier daemon.
- `[ACH`, `[RA`: achievements and RetroAchievements.
- `update plan for`, `committed in`: in-place updates. `removed`, `not removing`: download cleanup.

"The app opens and closes instantly" with `drop.log` ending abruptly usually means a panic before the window draws:
the panic hook writes `crash-<time>.log`, check it, then look for `.unwrap()`/`.expect()` on the startup path.
A database that fails to load is backed up and replaced with a fresh one, so "everything is gone" after an update
points at a missing `#[serde(default)]`.

## Server logs
- Live: `sudo docker logs drop-drop-1 --tail 200` on the NAS (container names, not `docker compose`, see
  `../drop-server/docs/runbooks/dev-environment.md`).
- Dev: the `pnpm dev` terminal.
- A missing `STEAM_API_KEY` only shows as a warning: Steam metadata and achievement schemas come back empty.

## Steam Deck over SSH
- `ssh deck` uses mDNS (`steamdeck.local`), which breaks when the PC's VPN is on.
- If it fails, find the Deck on the current network (check the PC's own IP first; the network has changed before;
  the Deck was last at `192.168.4.42`) and connect directly:
  `ssh -i ~/.ssh/id_ed25519_deck deck@<ip>`.
- On the Deck, Drop's data and logs are under `~/.local/share/drop/`.

## Defect patterns to suspect first
1. **Config written where the target never reads it**: wrong file, wrong key, wrong units, a Linux path given to a
   Windows program under Proton. Confirm the target actually reads what was written.
2. **Failure that looks like "empty"**: a page that couldn't reach the server showing "nothing here".
3. **Silent failure**: an action that does nothing and says nothing.
4. **A comment promising something the code doesn't do** (a lock "held" that was dropped, a path "contained" by a
   check a symlink walks around). Verify the claim.
5. **Panic on the startup path.**
6. **A local or cached copy trusted over the real source** because it wasn't empty.
7. **A check that passed without running**: `grep -c "error"` returns 0 both when clean and when the command never
   ran. Check the exit code and that output exists.
