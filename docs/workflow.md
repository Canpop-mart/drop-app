# How to work on Drop with a small context window

Written for the owner and for local models (about 64K tokens of context). The same rules make Claude sessions
cheaper too.

## The idea
The chat is not the memory. Files are. Knowledge about the code lives in `docs/map/`, planned work lives in
`docs/UPCOMING.md`, and the state of the current task lives in `tasks/current.md`. A session reads those, does one
small task, writes down what it did, and ends.

## Budget
Roughly where 64K tokens go:
- harness instructions: 5-15K (depends on the tool);
- `AGENTS.md`: about 1.5K; one map: 1.5-3K;
- the model's own thinking: often 2-10K per answer (lower reasoning effort to cut this);
- what's left for code and command output: about 30-40K.
One line of code is about 10 tokens, so a 2,000-line file is about 20-25K: never read a big file whole.

## One session, one task
1. **Pick a small task.** Good: "In `src-tauri/src/settings.rs`, add setting X with default Y and show it on the
   desktop and BPM settings pages." Too big: "fix cloud saves". If an item in `UPCOMING.md` is too big, split it
   first (a bigger model is good at this).
2. **Start the task file.** Copy `tasks/TEMPLATE.md` to `tasks/current.md`, fill Goal, Map, Plan.
3. **Find, don't browse.** Open the map, then search for the exact function (`rg -n "fn name" src-tauri`), then read
   just that line range (your harness's file-read tool takes a start line and a count).
4. **Change only what the task needs.** No tidying nearby code.
5. **Check** with the filtered commands in `AGENTS.md`. Paste only failures back into context.
6. **Write it down** in `tasks/current.md` (Done, Not checked, Strings, Next).
7. **End the session** when the task is done, or when context is around 70% full. The next session starts with:
   "Read AGENTS.md and tasks/current.md, then continue."

Avoid letting the harness auto-compact (summarise) the chat more than once. Each summary loses detail. A fresh
session that reads `tasks/current.md` loses nothing.

## Writing a good request
- Say where: "In `main/pages/community.vue`, ..." beats "in the community tab".
- Say what not to do: "Don't touch anything else."
- Say how to check: "Run the typecheck."
- For UI: say both surfaces, or say "desktop only, BPM later".

## When to hand off to a bigger model (Claude)
- Designing a feature, or splitting a big `UPCOMING.md` item into small tasks.
- Anything touching: the local database format (`pot`, `serde(default)`), BPM modals and the input lock, the download
  and update engine, cloud-save conflict logic, auth/ACL scopes, Prisma migrations.
- Reviewing a finished diff before the owner commits. Paste the `git diff` plus `tasks/current.md`.

## Keeping the docs true
- If a map is wrong, fix the map in the same change as the code.
- New area or big feature: add or extend a map, and add a line to `docs/INDEX.md`.
- Finished an `UPCOMING.md` item: move it to the "Done, waiting for release" section.
- Run `node scripts/check-docs.mjs` (in drop-server: `node tools/check-docs.mjs`) to catch paths in docs that no
  longer exist.
