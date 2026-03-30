# Drop OSS — Game Import Master Reference

> Server: `https://drop.canpop.synology.me`
> Admin token: `fe7a5c9c-8e18-4e6b-b4a2-ec5913c2ff11`
> NAS root (Windows): `Z:\media\emulation\`
> NAS root (Linux/Drop): `/emulation/`

---

## API Quick Reference

### Auth Headers
```powershell
$hJson = @{ Authorization = "Bearer $TOKEN"; "Content-Type" = "application/json" }
$hGet  = @{ Authorization = "Bearer $TOKEN" }
```
> ⚠️ Always use `Authorization: Bearer` — NOT `X-Api-Key`. The X-Api-Key header returns 403 on most endpoints.

### Core Endpoints
| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/v1/admin/library?query={q}&limit={n}` | Search games across all libraries |
| GET | `/api/v1/admin/library?libraryId={id}&limit={n}` | List games in a specific library |
| GET | `/api/v1/admin/game/{gameId}/versions` | Get version + launch info for a game |
| GET | `/api/v1/admin/import/game?limit={n}` | List all unimported ROMs (shows library IDs) |
| POST | `/api/v1/admin/import/game` | Register/create a game entry |
| POST | `/api/v1/admin/import/version` | Create/update a version with launch config |
| PATCH | `/api/v1/admin/game/{gameId}` | Update game metadata (text fields only — no images) |
| DELETE | `/api/v1/admin/game/{gameId}` | Delete a game entirely |

> ⚠️ There is NO working PATCH/PUT endpoint for updating a launch command after the fact.
> To change a launch: DELETE the game → re-import/game → re-import/version.

---

## CRITICAL: Always Attach Metadata

> **Never import a game without metadata.** A game entry with no cover art, description, or genre looks broken in the Drop UI and creates a poor user experience.

**Before calling `import/version`**, ensure the game has metadata via one of these methods:

1. **Drop's built-in metadata matcher (UI only)** — After `POST /import/game`, open the Drop admin UI and use the metadata search/match feature to pull cover art, description, genres, and release info from Drop's metadata providers (IGDB, etc.). Confirm the match before proceeding to `import/version`.

2. **Partial API metadata via `PATCH /game/{id}`** — You can set text fields through the API:
   ```bash
   curl -X PATCH "$BASE/game/{gameId}" \
     -H "Authorization: Bearer $TOKEN" \
     -H "Content-Type: application/json" \
     -d '{
       "mName": "Game Title",
       "mShortDescription": "One-liner summary.",
       "mDescription": "# Full markdown description.",
       "mReleased": "1997-03-21T00:00:00.000Z"
     }'
   ```
   **Accepted fields**: `mName`, `mShortDescription`, `mDescription`, `mReleased`, `metadataSource`

3. **Manual metadata** — If Drop's matcher can't find the game (romhacks, obscure titles, fan translations), create your own metadata: at minimum a **cover image**, **description**, and **genre tags**. Upload cover art through the Drop admin UI.

### Full API Import with Metadata + Images

The **recommended** API import flow uses IGDB metadata, which automatically downloads cover art, banners, icons, screenshots, and all text metadata:

```bash
# Step 1: Search IGDB for the game (supports game name OR IGDB numeric ID)
curl "$BASE/admin/import/game/search?q=Chrono+Trigger" -H "$AUTH"
# Returns: [{id: "1234", name: "Chrono Trigger", sourceId: "IGDB", ...}]

# Step 2: Import game WITH metadata (auto-downloads all images from IGDB)
curl -X POST "$BASE/admin/import/game" -H "$AUTH" -H "Content-Type: application/json" \
  -d '{
    "library": "<libraryId>",
    "path": "Chrono Trigger (USA)",
    "type": "Game",
    "metadata": {
      "id": "1234",
      "sourceId": "IGDB",
      "name": "Chrono Trigger"
    }
  }'
# Returns: {taskId: "..."}  — game is created with ALL metadata + images

# Step 3: Import version (same as before)
curl -X POST "$BASE/admin/import/version" -H "$AUTH" -H "Content-Type: application/json" \
  -d '{ "id": "<gameId>", "version": {...}, "launches": [...], "setups": [] }'
```

### Additional Image APIs
```bash
# Upload custom icon: POST /admin/game/{id}/metadata (multipart form)
# Add to image carousel: POST /admin/game/image (multipart form with id + file)
# Upload raw object: POST /object/{uuid} (raw binary body)
```

> **Import order**: Search IGDB → `import/game` with metadata → verify in UI → `import/version`

---

## import/game Body
```json
{
  "library": "<libraryId>",
  "path":    "<ROM filename without extension OR emulator name>",
  "type":    "Game"
}
```
- For emulator entries: `"type": "Emulator"`
- `path` should be the bare ROM name as it appears in the library folder (no extension, no directory prefix)
- The server auto-assigns a `gameId` — retrieve it via `GET /library?query=<name>`

## import/version Body
```json
{
  "id":      "<gameId>",
  "version": { "name": "<emulator name or ROM title>", "identifier": "<shell-escaped ROM filename + ext>", "type": "local" },
  "launches": [
    {
      "name":       "Switch Linux",
      "platform":   "Linux",
      "launch":     "<shell-escaped ROM filename + ext>",
      "extensions": [".nsp", ".xci"],
      "emulatorId": "<launchId of the emulator>"
    }
  ],
  "setups": []
}
```
- `version.name` — **use the emulator's name** (e.g. `"Ryujinx"`, `"melonDS"`, `"Dolphin"`). Drop displays this as the version label in the UI folder view. If left as `"default"` or `"v1"`, it shows as a random-looking string in the folder. For ROM game versions, use the ROM title.
- `identifier` and `launch` are the **shell-escaped** ROM filename (e.g. `Luigi\'s\ Mansion.nsp`)
- `emulatorId` is the **launchId** of the emulator entry (NOT the gameId) — see Emulator table below
- `extensions` tells Drop which file extensions this launch handles
- `setups` array is **silently ignored** by the server — do not use it for automation

### Shell-escaping ROM filenames
Spaces → `\ `, apostrophes → `\'`, parens → `\(` `\)`, brackets → `\[` `\]`

```powershell
function Escape-Rom($name) {
    $name -replace " ", "\ " `
          -replace "'", "\'" `
          -replace "\(", "\(" `
          -replace "\)", "\)" `
          -replace "\[", "\[" `
          -replace "\]", "\]"
}
# e.g. "Luigi's Mansion 2 HD (Switch) (Rom).nsp"
#   -> "Luigi\'s\ Mansion\ 2\ HD\ \(Switch\)\ \(Rom\).nsp"
```

---

## Libraries

| System | Library ID | NAS Path (Linux) | ROM Extensions |
|--------|-----------|-----------------|----------------|
| **Emulators** | `39b8bf15-88bd-42f5-9aa0-7f93aaf1feef` | `/emulation/Emulators` | — |
| **Switch** | `eb0dc687-1d72-4c7e-92cc-32f4734d1111` | `/emulation/Roms/Nintendo Switch` | `.nsp` `.xci` |
| **Wii** | `92043d91-6033-4aaa-8adf-7c4ba1f9340a` | `/emulation/Roms/Wii` | `.rvz` `.iso` `.wbfs` |
| **GameCube** | `63082341-154f-4504-b825-751e7684508d` | `/emulation/Roms/GameCube` | `.rvz` `.iso` `.gcm` |
| **Nintendo 64** | `ee9ed329-b992-4ca7-8fd7-8c9d7e02fad6` | `/emulation/Roms/N64` | `.z64` `.n64` `.v64` |
| **PS1** | `b7d3ac34-9360-40dd-8ead-d58f0a65f556` | `/emulation/Roms/PS1` | `.cue` `.bin` `.chd` |
| **PS2** | `a154482f-2f37-404e-aaf3-ae30df22572a` | `/emulation/Roms/PS2` | `.iso` `.chd` |
| **SNES** | `cdf43891-9b96-4c2d-ae18-f989bb2d99fe` | `/emulation/Roms/SNES` | `.sfc` `.smc` |
| **GBA** | `9e6c2116-da44-4a57-934b-4dbb155a3858` | `/emulation/Roms/GBA` | `.gba` |
| **GBC** | `f41632c9-ec19-439b-a45e-38a35a6b6a18` | `/emulation/Roms/GBC` | `.gbc` `.gb` |
| **PC Games** | `66173337-4c40-4e6d-9371-07fa761d70c2` | `/library` | `.exe` |
| DS | *(no library yet — create when needed)* | `/emulation/Roms/DS` | `.nds` |
| PSP | *(no library yet)* | `/emulation/Roms/PSP` | `.iso` `.cso` |
| PS3 | *(no library yet)* | `/emulation/Roms/PS3` | *(folder-based)* |
| WiiU | *(no library yet)* | `/emulation/Roms/WiiU` | `.wud` `.wux` `.rpx` |
| Xbox | *(no library yet)* | `/emulation/Roms/Xbox` | `.iso` `.xiso` |

> ℹ️ DS, PSP, PS3, WiiU, Xbox: emulators are configured but no Drop library exists yet.
> When adding these, first check with `GET /import/game?limit=500` to see if a library ID appears.

---

## Emulators

All emulators live in the **Emulators** library (`39b8bf15`).
The `launchId` is what you put in `emulatorId` when importing a game version.

| System | Platform | Emulator Game ID | Launch ID | Launch Command |
|--------|----------|-----------------|-----------|----------------|
| **Switch** | Linux | `dfd8ee2a-524e-4704-93cb-99782511b0ea` | `5013549a-6d5c-4925-9e06-67159b2dd355` | `publish/Ryujinx.sh {rom}` |
| **Switch** | Windows | `eab3b4a1-30ca-4d1d-9804-2958c6b80ee5` | `249c2705-9523-4aa7-a887-84831b85c1a3` | `publish\Ryujinx.exe {rom}` |
| **DS** | Linux | `0daa9a24-d1e4-4405-88a4-3049d953c00c` | `8f3ebd7a-7af6-4b5c-983f-945ebb640cce` | `melonDS-x86_64.AppImage --appimage-extract-and-run {rom}` |
| **DS** | Windows | `ae8cd300-8025-4674-ac90-0638a62174b0` | `7ebb2a63-e378-4b71-b0d2-ad60dffa70e6` | `melonDS.exe {rom}` |
| **GBA** | Linux | `9d76d870-a361-4932-b636-a7119f43c02f` | `72a44928-8ab0-475c-b653-ddc9bb007120` | `mGBA.AppImage --appimage-extract-and-run {rom}` |
| **GBA** | Windows | `79acbe65-f26b-473e-b042-35aa6cecd4d3` | `fd126b65-2ab2-485c-aa82-0bee15aa037e` | `mGBA.exe {rom}` |
| **GBC** | Linux | *(use mGBA — supports GBC)* | same as GBA Linux | — |
| **GBC** | Windows | *(use mGBA — supports GBC)* | same as GBA Windows | — |
| **GameCube** | Linux | `c82d7406-d5a4-4e34-a1e5-662d9d6a19a7` | `04fd91ca-7215-40cf-a186-b16a84f96161` | `Dolphin.AppImage --appimage-extract-and-run --batch --exec={rom}` |
| **GameCube** | Windows | `ce4569ed-2a5c-4cd9-a234-5d9b215f95eb` | `1183c06c-5912-434a-b8dc-921be94ead34` | `Dolphin.exe --batch --exec={rom}` |
| **Wii** | Linux | same as GameCube Linux (`c82d7406`) | `04fd91ca-7215-40cf-a186-b16a84f96161` | `Dolphin.AppImage --appimage-extract-and-run --batch --exec={rom}` |
| **Wii** | Windows | same as GameCube Windows (`ce4569ed`) | `1183c06c-5912-434a-b8dc-921be94ead34` | `Dolphin.exe --batch --exec={rom}` |
| **N64** | Linux | *(no Linux emulator configured)* | — | — |
| **N64** | Windows | `a55c5e85-41ae-43ad-b7f5-b2e3bef13965` | `4de70384-9a07-43b8-b72d-e3fefe1079b8` | `simple64-gui.exe {rom}` |
| **PS1** | Linux | `937bb93f-3845-4d68-b946-1974c4330f7e` | `c835b57b-1138-42f1-ad38-d8c74003f1f7` | `DuckStation.AppImage --appimage-extract-and-run -batch {rom}` |
| **PS1** | Windows | `008a8f8f-94be-4e15-9c40-dea80c9fb285` | `5ec190ab-c17c-4dbf-9e48-91bbd283c617` | `duckstation-qt-x64-ReleaseLTCG.exe -batch {rom}` |
| **PS2** | Linux | `d2a1ce41-b438-42a6-9d50-80aa33a361a0` | `2f589e0a-df79-4140-b876-ce5d8c32031e` | `PCSX2.AppImage --appimage-extract-and-run {rom}` |
| **PS2** | Windows | `5713aca1-edd7-46c5-afb3-569bc6d96391` | `4ce89b89-87fc-417f-a292-f16b8747ba65` | `pcsx2-qt.exe {rom}` |
| **PS3** | Linux | `851c2076-d0da-4bdd-bfee-6ae4d222732a` | `c9b95987-ff08-4972-bf90-1a11444b12ca` | `RPCS3.AppImage --appimage-extract-and-run {rom}` |
| **PS3** | Windows | `6d4b13cf-95ec-40aa-abe1-f6e1cdac450d` | `1910451d-3522-40cc-bb82-04a0944bae69` | `rpcs3.exe {rom}` |
| **PSP** | Linux | `64e01054-fd36-469d-9b54-ee848b4a35a8` | `1460868e-fcae-4ac4-9522-8207e8fdfed4` | `PPSSPP.AppImage --appimage-extract-and-run {rom}` |
| **PSP** | Windows | `19cf425a-c071-41c3-bfc5-a15ac2013840` | `fde689ca-6760-4126-aece-b16de97dcc2a` | `PPSSPPWindows64.exe {rom}` |
| **SNES** | Linux | `237882cf-8072-4f70-8fd1-dffd3036f892` | `ca79d2c0-9ada-4177-91dd-d44483f3500f` | `Snes9x.AppImage --appimage-extract-and-run {rom}` |
| **SNES** | Windows | `51c2c133-bb66-44e2-a5da-4f7de7adb290` | `83d82018-2c3f-4719-84be-1e4fe7a7dd38` | `snes9x-x64.exe {rom}` |
| **WiiU** | Linux | `6cf96f07-2459-48f6-bf32-2ed3277581c1` | `75755a5b-15a9-4656-a2e0-99dbd58993b7` | `Cemu.AppImage --appimage-extract-and-run -g {rom}` |
| **WiiU** | Windows | `1fee30c1-97cc-4410-b7e4-4c5376b5bb94` | `f98daa6e-1f59-49f5-bac6-74cae07050a8` | `Cemu.exe -g {rom}` |
| **Xbox OG** | Linux | `1b178437-020f-4c06-9e1c-82d2d37e5467` | `d9bc7a81-a553-406d-aaa0-76e1ff5a4de4` | `xemu.AppImage --appimage-extract-and-run -dvd_path {rom}` |
| **Xbox OG** | Windows | `99e937ff-5f2b-4a04-bdb5-61d1eddd63ac` | `7e92576c-62ce-4777-9a0d-f37049974bf1` | `xemu.exe -dvd_path {rom}` |

---

## Per-System Import Rules

### Nintendo Switch
- **Library ID**: `eb0dc687-1d72-4c7e-92cc-32f4734d1111`
- **Extensions**: `.nsp` (preferred), `.xci`
- **ROM naming**: `<Title> (Switch) (Rom).nsp`
- **Linux emulator launchId**: `5013549a-6d5c-4925-9e06-67159b2dd355`
- **Windows emulator launchId**: `249c2705-9523-4aa7-a887-84831b85c1a3`
- ⚠️ **Special**: Linux uses `Ryujinx.sh` wrapper (not `Ryujinx` directly). The wrapper auto-installs firmware from `publish/portable/` into `~/.config/Ryujinx/` on first run.
- ⚠️ **Firmware**: `~/.config/Ryujinx/bis/system/Contents/registered/` must be populated (234 NCA files). `setup.sh` handles this.
- ⚠️ **Keys**: `prod.keys` + `title.keys` live in `publish/portable/system/` and are copied to `~/.config/Ryujinx/system/` by `setup.sh`.

```powershell
# Switch game import template (both platforms)
$romName = "Luigi's Mansion 2 HD (Switch) (Rom)"  # no extension
$romEsc  = "Luigi\'s\ Mansion\ 2\ HD\ \(Switch\)\ \(Rom\).nsp"
$swLinLid = "5013549a-6d5c-4925-9e06-67159b2dd355"
$swWinLid = "249c2705-9523-4aa7-a887-84831b85c1a3"
$switchLib = "eb0dc687-1d72-4c7e-92cc-32f4734d1111"

Invoke-WebRequest -Uri "$BASE/import/game" -Method Post -Headers $hJson `
    -Body (@{ library=$switchLib; path=$romName; type="Game" } | ConvertTo-Json) | Out-Null
Start-Sleep -Seconds 4
$game = (Invoke-RestMethod -Uri "$BASE/library?query=luigi" -Headers $hGet).results[0].game

$vBody = @{
    id      = $game.id
    version = @{ name="default"; identifier=$romEsc; type="local" }
    launches = @(
        @{ name="Switch Windows"; platform="Windows"; launch=$romEsc; extensions=@(".nsp",".xci"); emulatorId=$swWinLid }
        @{ name="Switch Linux";   platform="Linux";   launch=$romEsc; extensions=@(".nsp",".xci"); emulatorId=$swLinLid }
    )
    setups = @()
} | ConvertTo-Json -Depth 6
Invoke-WebRequest -Uri "$BASE/import/version" -Method Post -Headers $hJson -Body $vBody | Out-Null
```

---

### GameCube
- **Library ID**: `63082341-154f-4504-b825-751e7684508d`
- **Extensions**: `.rvz` (preferred compressed format), `.iso`, `.gcm`
- **ROM naming**: `<Title> (USA).rvz` or `<Title> (USA, Canada).rvz`
- **Linux emulator launchId**: `04fd91ca-7215-40cf-a186-b16a84f96161` (Dolphin)
- **Windows emulator launchId**: `1183c06c-5912-434a-b8dc-921be94ead34` (Dolphin)
- Note: Dolphin handles both GameCube AND Wii — same emulator IDs for both libraries.

---

### Wii
- **Library ID**: `92043d91-6033-4aaa-8adf-7c4ba1f9340a`
- **Extensions**: `.rvz`, `.iso`, `.wbfs`
- **ROM naming**: `<Title> (USA).rvz`
- **Linux emulator launchId**: `04fd91ca-7215-40cf-a186-b16a84f96161` (same Dolphin as GameCube)
- **Windows emulator launchId**: `1183c06c-5912-434a-b8dc-921be94ead34`

---

### Nintendo 64
- **Library ID**: `ee9ed329-b992-4ca7-8fd7-8c9d7e02fad6`
- **Extensions**: `.z64` (byte-swapped big-endian), `.n64`, `.v64`
- **ROM naming**: `<Title> (USA).z64`
- **Windows emulator launchId**: `4de70384-9a07-43b8-b72d-e3fefe1079b8` (simple64)
- ⚠️ No Linux emulator currently configured for N64.

---

### PS1
- **Library ID**: `b7d3ac34-9360-40dd-8ead-d58f0a65f556`
- **Extensions**: `.cue` + `.bin` pairs (use `.cue` as launch target), `.chd`
- **ROM naming**: `<Title> (USA).cue` / `<Title> (USA).bin`
- **Linux emulator launchId**: `c835b57b-1138-42f1-ad38-d8c74003f1f7` (DuckStation)
- **Windows emulator launchId**: `5ec190ab-c17c-4dbf-9e48-91bbd283c617` (DuckStation)
- ⚠️ **Always import the `.cue` file**, not `.bin`. DuckStation reads the cue sheet to load multi-track games.
- ⚠️ Multi-bin games: one game = one `.cue` + multiple `.bin` files. Import the `.cue` only.

---

### PS2
- **Library ID**: `a154482f-2f37-404e-aaf3-ae30df22572a`
- **Extensions**: `.iso` (most common), `.chd`, `.bin`/`.cue`
- **ROM naming**: `<Title> (USA) (En,Ja).iso` or `<Title> (USA).iso`
- **Linux emulator launchId**: `2f589e0a-df79-4140-b876-ce5d8c32031e` (PCSX2)
- **Windows emulator launchId**: `4ce89b89-87fc-417f-a292-f16b8747ba65` (PCSX2)

---

### PS3
- **Library**: *(not yet configured in Drop)*
- **Emulator**: RPCS3
- **Linux emulator launchId**: `c9b95987-ff08-4972-bf90-1a11444b12ca`
- **Windows emulator launchId**: `1910451d-3522-40cc-bb82-04a0944bae69`
- ⚠️ PS3 games are folder-based (e.g. `BLUS12345/`), not single files. RPCS3 needs path to the `EBOOT.BIN` or the game folder.
- ⚠️ Create the Drop library for PS3 before importing games.

---

### PSP
- **Library**: *(not yet configured in Drop)*
- **Emulator**: PPSSPP
- **Linux emulator launchId**: `1460868e-fcae-4ac4-9522-8207e8fdfed4`
- **Windows emulator launchId**: `fde689ca-6760-4126-aece-b16de97dcc2a`
- **Extensions**: `.iso`, `.cso` (compressed ISO)
- Create the Drop library for PSP before importing games.

---

### SNES
- **Library ID**: `cdf43891-9b96-4c2d-ae18-f989bb2d99fe`
- **Extensions**: `.sfc`, `.smc`
- **ROM naming**: `<Title> (USA).sfc`
- **Linux emulator launchId**: `ca79d2c0-9ada-4177-91dd-d44483f3500f` (Snes9x)
- **Windows emulator launchId**: `83d82018-2c3f-4719-84be-1e4fe7a7dd38` (Snes9x)

---

### GBA
- **Library ID**: `9e6c2116-da44-4a57-934b-4dbb155a3858`
- **Extensions**: `.gba`
- **ROM naming**: `<Title> (USA).gba`
- **Linux emulator launchId**: `72a44928-8ab0-475c-b653-ddc9bb007120` (mGBA)
- **Windows emulator launchId**: `fd126b65-2ab2-485c-aa82-0bee15aa037e` (mGBA)

---

### GBC
- **Library ID**: `f41632c9-ec19-439b-a45e-38a35a6b6a18`
- **Extensions**: `.gbc`, `.gb`
- **ROM naming**: `<Title> (USA).gbc`
- **Linux emulator launchId**: same as GBA — mGBA handles GBC (`72a44928`)
- **Windows emulator launchId**: same as GBA — mGBA handles GBC (`fd126b65`)

---

### DS
- **Library**: *(not yet configured in Drop)*
- **Emulator**: melonDS
- **Linux emulator launchId**: `8f3ebd7a-7af6-4b5c-983f-945ebb640cce`
- **Windows emulator launchId**: `7ebb2a63-e378-4b71-b0d2-ad60dffa70e6`
- **Extensions**: `.nds`
- Create the Drop library for DS before importing games.

---

### Wii U
- **Library**: *(not yet configured in Drop)*
- **Emulator**: Cemu
- **Linux emulator launchId**: `75755a5b-15a9-4656-a2e0-99dbd58993b7`
- **Windows emulator launchId**: `f98daa6e-1f59-49f5-bac6-74cae07050a8`
- **Extensions**: `.wud`, `.wux`, `.rpx` (use `-g` flag in Cemu)
- Create the Drop library for WiiU before importing games.

---

### Xbox Original
- **Library**: *(not yet configured in Drop)*
- **Emulator**: xemu
- **Linux emulator launchId**: `d9bc7a81-a553-406d-aaa0-76e1ff5a4de4`
- **Windows emulator launchId**: `7e92576c-62ce-4777-9a0d-f37049974bf1`
- **Extensions**: `.iso` (xISO format — not standard ISO)
- Create the Drop library for Xbox before importing games.

---

## Master Import Script Template (PowerShell)

```powershell
# ── Constants ─────────────────────────────────────────────────────────────────
$TOKEN = "fe7a5c9c-8e18-4e6b-b4a2-ec5913c2ff11"
$BASE  = "https://drop.canpop.synology.me/api/v1/admin"
$hJson = @{ Authorization = "Bearer $TOKEN"; "Content-Type" = "application/json" }
$hGet  = @{ Authorization = "Bearer $TOKEN" }

function Escape-Rom($name) {
    $name -replace " ", "\ " `
          -replace "'", "\'" `
          -replace "\(", "\(" `
          -replace "\)", "\)" `
          -replace "\[", "\[" `
          -replace "\]", "\]"
}

function Import-EmulatorGame {
    param(
        [string]$RomName,         # e.g. "Banjo-Kazooie (USA)"  (no extension)
        [string]$RomFile,         # e.g. "Banjo-Kazooie (USA).z64"
        [string]$LibraryId,
        [string]$LinuxLaunchId,   # can be $null
        [string]$WindowsLaunchId,
        [string[]]$Extensions,
        [string]$VersionName = "" # emulator name (e.g. "Dolphin") or game title — avoids random string in UI
    )

    # ── Fix #6: Duplicate check ──────────────────────────────────────────────
    $qCheck = [uri]::EscapeDataString($RomName)
    $existing = (Invoke-RestMethod -Uri "$BASE/library?query=$qCheck&libraryId=$LibraryId&limit=10" -Headers $hGet).results |
                Where-Object { $_.game.libraryPath -eq $RomName } |
                Select-Object -First 1
    if ($existing) {
        Write-Host "SKIP (already exists): $RomName ($($existing.game.id))" -ForegroundColor Yellow
        return
    }

    $romEsc = Escape-Rom $RomFile

    # ── Fix #5: Error handling on import/game ────────────────────────────────
    try {
        Invoke-WebRequest -Uri "$BASE/import/game" -Method Post -Headers $hJson `
            -Body (@{ library=$LibraryId; path=$RomName; type="Game" } | ConvertTo-Json) -ErrorAction Stop | Out-Null
    } catch {
        Write-Error "import/game FAILED for '$RomName': $($_.Exception.Message)"
        return
    }
    Start-Sleep -Seconds 4

    # ── Fix #4: Search by full escaped name, fall back to broader query ──────
    $qFull = [uri]::EscapeDataString($RomName)
    $game  = (Invoke-RestMethod -Uri "$BASE/library?query=$qFull&libraryId=$LibraryId&limit=10" -Headers $hGet).results |
             Where-Object { $_.game.libraryPath -eq $RomName } |
             Select-Object -First 1

    if (-not $game) {
        # Broader fallback: first word only
        $qShort = [uri]::EscapeDataString(($RomName -split " ")[0])
        $game   = (Invoke-RestMethod -Uri "$BASE/library?query=$qShort&libraryId=$LibraryId&limit=50" -Headers $hGet).results |
                  Where-Object { $_.game.libraryPath -eq $RomName } |
                  Select-Object -First 1
    }

    if (-not $game) { Write-Error "Could not find '$RomName' after import — check server logs"; return }

    # ── Fix #1: Default VersionName to game title if not provided ────────────
    if (-not $VersionName) { $VersionName = $RomName }

    # Build launches array
    $launches = @()
    if ($WindowsLaunchId) {
        $launches += @{ name="Windows"; platform="Windows"; launch=$romEsc; extensions=$Extensions; emulatorId=$WindowsLaunchId }
    }
    if ($LinuxLaunchId) {
        $launches += @{ name="Linux"; platform="Linux"; launch=$romEsc; extensions=$Extensions; emulatorId=$LinuxLaunchId }
    }

    $vBody = @{
        id       = $game.game.id
        version  = @{ name=$VersionName; identifier=$romEsc; type="local" }
        launches = $launches
        setups   = @()
    } | ConvertTo-Json -Depth 6

    # ── Fix #5: Error handling on import/version ─────────────────────────────
    try {
        Invoke-WebRequest -Uri "$BASE/import/version" -Method Post -Headers $hJson -Body $vBody -ErrorAction Stop | Out-Null
    } catch {
        Write-Error "import/version FAILED for '$RomName': $($_.Exception.Message)"
        return
    }

    # ── Fix #3: Wait after import/version per Known Issue #8 ─────────────────
    Start-Sleep -Seconds 8

    Write-Host "Imported: $RomName ($($game.game.id))" -ForegroundColor Green
}

# ── Example usage ─────────────────────────────────────────────────────────────
# N64 game (Windows only)
Import-EmulatorGame `
    -RomName         "Banjo-Kazooie (USA)" `
    -RomFile         "Banjo-Kazooie (USA).z64" `
    -LibraryId       "ee9ed329-b992-4ca7-8fd7-8c9d7e02fad6" `
    -LinuxLaunchId   $null `
    -WindowsLaunchId "4de70384-9a07-43b8-b72d-e3fefe1079b8" `
    -Extensions      @(".z64", ".n64") `
    -VersionName     "simple64"

# PS1 game (always use .cue file)
Import-EmulatorGame `
    -RomName         "Ace Combat 2 (USA)" `
    -RomFile         "Ace Combat 2 (USA).cue" `
    -LibraryId       "b7d3ac34-9360-40dd-8ead-d58f0a65f556" `
    -LinuxLaunchId   "c835b57b-1138-42f1-ad38-d8c74003f1f7" `
    -WindowsLaunchId "5ec190ab-c17c-4dbf-9e48-91bbd283c617" `
    -Extensions      @(".cue", ".chd") `
    -VersionName     "DuckStation"

# GameCube game
Import-EmulatorGame `
    -RomName         "Animal Crossing (USA, Canada)" `
    -RomFile         "Animal Crossing (USA, Canada).rvz" `
    -LibraryId       "63082341-154f-4504-b825-751e7684508d" `
    -LinuxLaunchId   "04fd91ca-7215-40cf-a186-b16a84f96161" `
    -WindowsLaunchId "1183c06c-5912-434a-b8dc-921be94ead34" `
    -Extensions      @(".rvz", ".iso", ".gcm") `
    -VersionName     "Dolphin"
```

---

## Known Issues & Workarounds

### 0. Version name shows as random string in UI folder
The `version.name` field in `import/version` is what Drop displays as the version label. Always set it to the **emulator name** for emulator entries (e.g. `"Ryujinx"`, `"Dolphin"`, `"melonDS"`) or the **game title** for ROM entries. Never leave it as `"default"` or `"v1"` — those appear as unintelligible strings in the Drop folder view.

### 1. `setups` array is silently ignored
The `import/version` body accepts a `setups` array but the server discards it — it never appears in the stored version. **Workaround**: Use a wrapper shell script as the launch target instead. The wrapper can do setup on first run.

### 2. No endpoint to update a launch command
There is no working PATCH/PUT for changing a launch command after the fact. **Workaround**: `DELETE /game/{gameId}` then re-import.

### 3. ROM filenames with apostrophes
ROM paths with `'` (e.g. "Luigi's Mansion") break `bash -c '...'` style wrappers because Drop substitutes `{rom}` before bash sees it. **Workaround**: Pass the ROM as `$@` to a proper wrapper script — never embed `{rom}` inside a quoted bash -c string.

### 4. umu-launcher path resolution (Linux)
On the Steam Deck, umu-launcher resolves binary paths relative to the game's install folder. A bare command like `bash` looks for `/home/deck/Documents/Drop/{uuid}/bash`. **Always use absolute paths** (`/usr/bin/bash`) or paths relative to the game root that actually exist.

### 5. Switch Linux firmware
Ryujinx requires firmware in `~/.config/Ryujinx/bis/system/Contents/registered/`. The Launch flow:
1. Drop runs `publish/Ryujinx.sh {rom}`
2. `Ryujinx.sh` checks if firmware dir is empty
3. If empty → runs `publish/setup.sh` (copies 234 NCA files + keys from `publish/portable/`)
4. Execs `publish/Ryujinx "$@"` with the ROM path

### 6. Switch Linux AppImage vs tarball
Do **not** use the AppImage with `--appimage-extract-and-run` for Ryujinx — it extracts to a temp dir so `portable/` is never adjacent to the binary. Use the self-contained tarball binary directly (`publish/Ryujinx`).

### 7. `GET /admin/game/{gameId}` returns 403
Use `GET /admin/game/{gameId}/versions` instead to check version/launch state.

### 8. Timing after import
After `POST /import/game`, wait ~4 seconds before querying. After `POST /import/version`, wait ~8 seconds. The server indexes asynchronously.

### 9. PS1 multi-track games
Always import the `.cue` file, not the `.bin`. The cue sheet tells DuckStation how to load multi-track CD images. A game with `game.cue + game (Track 1).bin + game (Track 2).bin` should be imported as one entry pointing at `game.cue`.

---

## Current Game IDs (as of 2026-03-21)

| Game | Library | Game ID |
|------|---------|---------|
| Luigi's Mansion 2 HD | Switch | `3e287b5f-d5f5-4dcb-b3ab-cabc0a5719dd` |
| Luigi's Mansion (GCN) | GameCube | `541264aa-ca98-4ecf-9653-9812d296383d` |
| Metroid Prime Trilogy | Wii | `a6075b64-0d6e-4721-9bdf-619b8d3a81c8` |
| Ace Combat 5 | PS2 | `84b601ea-11c4-44d4-b5a2-8cbaff8bc952` |
| Ace Combat 2 | PS1 | `c6da919b-7a38-41f1-973e-1efddc16eee0` |
| Pokemon Fool's Gold | GBC | `8167a229-c652-4ce1-815f-5832356e1b09` |
| Metroid Zero Mission | GBA | `c6ec63cf-9d62-447f-8963-e745447695f3` |
| ActRaiser 2 | SNES | `58de21be-e4d3-47cb-b4d2-b49952858c49` |
