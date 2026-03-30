# download-emulators.ps1
# Downloads Windows + Linux emulators for each system into Z:\media\emulation\Emulators\
# Skips any folder that already has an .exe or .AppImage in it.

$BASE    = "Z:\media\emulation\Emulators"
$TEMP    = "$env:TEMP\emu-dl"
$HEADERS = @{ "User-Agent" = "Mozilla/5.0"; "Accept" = "application/vnd.github+json" }

# 7-Zip for extracting .7z archives
$7Z = @("C:\Program Files\7-Zip\7z.exe","C:\Program Files (x86)\7-Zip\7z.exe") |
      Where-Object { Test-Path $_ } | Select-Object -First 1

New-Item -ItemType Directory -Path $TEMP -Force | Out-Null

# ── Helpers ───────────────────────────────────────────────────────────────────

function gh-latest($repo) {
    try { Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers $HEADERS }
    catch { $null }
}

function gh-latest-pre($repo) {
    # Use /releases (list) to get newest including pre-releases
    try {
        $list = Invoke-RestMethod "https://api.github.com/repos/$repo/releases?per_page=1" -Headers $HEADERS
        $list | Select-Object -First 1
    } catch { $null }
}

function find-asset($release, [string[]]$patterns) {
    foreach ($p in $patterns) {
        $a = $release.assets | Where-Object { $_.name -match $p } | Select-Object -First 1
        if ($a) { return $a }
    }
    $null
}

function dl($url, $label) {
    $file = "$TEMP\$([IO.Path]::GetFileName($url.Split('?')[0]))"
    Write-Host "      Downloading $label ... " -NoNewline
    try {
        (New-Object Net.WebClient).DownloadFile($url, $file)
        $mb = [math]::Round((Get-Item $file).Length/1MB,1)
        Write-Host "$mb MB" -ForegroundColor Gray
        return $file
    } catch {
        Write-Host "FAILED" -ForegroundColor Red; return $null
    }
}

function extract($archive, $dest) {
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    $ext = [IO.Path]::GetExtension($archive).ToLower()
    if ($ext -eq ".zip") {
        Expand-Archive -Path $archive -DestinationPath $dest -Force
        return $true
    } elseif ($ext -eq ".7z") {
        if ($7Z) { & $7Z x $archive "-o$dest" -y | Out-Null; return $true }
        else { Write-Host "      SKIP: 7-Zip not found for $archive" -ForegroundColor Yellow; return $false }
    }
    # Unknown — just leave the file
    Copy-Item $archive $dest; return $true
}

function flatten-if-single($dir) {
    # If extraction created a single subfolder, move its contents up
    $items = Get-ChildItem $dir
    if ($items.Count -eq 1 -and $items[0].PSIsContainer) {
        $sub = $items[0].FullName
        Get-ChildItem $sub | Move-Item -Destination $dir -Force
        Remove-Item $sub -Recurse -Force
    }
}

function has-content($dir) {
    if (-not (Test-Path $dir -PathType Container)) { return $false }
    (Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0
}

function install-emu($system, $platform, $url, $label) {
    $dir = if ($platform -eq "Linux") { "$BASE\$system Linux" } else { "$BASE\$system" }

    if (has-content $dir) {
        Write-Host "   [$system $platform] already populated — skipping" -ForegroundColor Gray
        return
    }

    Write-Host "   [$system $platform] $label" -ForegroundColor Cyan
    New-Item -ItemType Directory -Path $dir -Force | Out-Null

    $file = dl $url $label
    if (-not $file) { return }

    $ext = [IO.Path]::GetExtension($file).ToLower()
    if ($ext -in @(".zip", ".7z")) {
        $tmp = "$TEMP\ex_$system`_$platform"
        if (extract $file $tmp) {
            flatten-if-single $tmp
            Get-ChildItem $tmp | Copy-Item -Destination $dir -Recurse -Force
            Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }
    } elseif ($ext -eq ".appimage" -or $file -match "AppImage") {
        Copy-Item $file $dir
    } else {
        Copy-Item $file $dir
    }
    Remove-Item $file -ErrorAction SilentlyContinue
    Write-Host "      -> $dir" -ForegroundColor Green
}

function emu($system, $winRepo, [string[]]$winPat, $linRepo, [string[]]$linPat,
             $winFallback=$null, $linFallback=$null) {
    # Windows
    $winDir = "$BASE\$system"
    if (-not (has-content $winDir)) {
        $rel = gh-latest $winRepo
        if (-not $rel) { $rel = gh-latest-pre $winRepo }
        $asset = if ($rel) { find-asset $rel $winPat } else { $null }
        if ($asset) { install-emu $system "Windows" $asset.browser_download_url $asset.name }
        elseif ($winFallback) { install-emu $system "Windows" $winFallback "fallback" }
        else { Write-Host "   [$system Windows] no asset matched" -ForegroundColor Yellow }
    } else {
        Write-Host "   [$system Windows] already populated — skipping" -ForegroundColor Gray
    }

    # Linux
    $linDir = "$BASE\$system Linux"
    if (-not (has-content $linDir)) {
        $lRel = if ($linRepo -eq $winRepo) { $rel } else { gh-latest $linRepo }
        if (-not $lRel) { $lRel = gh-latest-pre $linRepo }
        $lasset = if ($lRel) { find-asset $lRel $linPat } else { $null }
        if ($lasset) { install-emu $system "Linux" $lasset.browser_download_url $lasset.name }
        elseif ($linFallback) { install-emu $system "Linux" $linFallback "fallback" }
        else { Write-Host "   [$system Linux] no asset matched" -ForegroundColor Yellow }
    } else {
        Write-Host "   [$system Linux] already populated — skipping" -ForegroundColor Gray
    }
}

# ── Download each emulator ────────────────────────────────────────────────────

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host " Emulator Downloader" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host " Destination: $BASE"
if (-not $7Z) { Write-Host " NOTE: 7-Zip not found — .7z archives will be skipped" -ForegroundColor Yellow }
Write-Host ""

# NES / SNES / GBC — Mesen2 (handles all three in one exe)
emu "NES"  "SourMesen/Mesen2" @("Windows\.zip$") `
          "SourMesen/Mesen2" @("Linux_x64\.zip$")

emu "SNES" "snes9xgit/snes9x" @("win32-x64\.zip$", "windows.*x64\.zip$") `
           "snes9xgit/snes9x" @("x86_64.*AppImage$", "AppImage$")

# Game Boy / GBA — mGBA
emu "GBA"  "mgba-emu/mgba" @("win64\.(7z|zip)$") `
           "mgba-emu/mgba"  @("appimage.*x64\.appimage$", "x64.*appimage$")

# Nintendo 64 — simple64 (Windows only; no Linux AppImage released)
emu "N64"  "simple64/simple64" @("win64.*\.zip$") `
           "simple64/simple64" @("x86_64.*AppImage$","linux.*x86_64")

# Nintendo DS — melonDS
emu "DS"   "melonDS-emu/melonDS" @("windows-x86_64\.zip$") `
           "melonDS-emu/melonDS"  @("appimage-x86_64\.zip$")

# GameCube / Wii — Dolphin (no GitHub releases; scrape download page)
$dolphinDir     = "$BASE\Dolphin"
$dolphinLinDir  = "$BASE\Dolphin Linux"
if (-not (has-content $dolphinDir)) {
    try {
        $pg = Invoke-WebRequest "https://dolphin-emu.org/download/" -UserAgent "Mozilla/5.0" -UseBasicParsing
        $winUrl = ($pg.Links | Where-Object { $_.href -match "dl\.dolphin-emu\.org.*/dolphin-\d+[a-z]?-x64\.7z$" } |
                   Select-Object -First 1).href
        if ($winUrl) { install-emu "Dolphin" "Windows" $winUrl ([IO.Path]::GetFileName($winUrl)) }
        else { Write-Host "   [Dolphin Windows] no download URL found" -ForegroundColor Yellow }
    } catch { Write-Host "   [Dolphin Windows] page fetch failed: $_" -ForegroundColor Yellow }
} else { Write-Host "   [Dolphin Windows] already populated — skipping" -ForegroundColor Gray }

if (-not (has-content $dolphinLinDir)) {
    try {
        $pg = Invoke-WebRequest "https://dolphin-emu.org/download/" -UserAgent "Mozilla/5.0" -UseBasicParsing
        $linUrl = ($pg.Links | Where-Object { $_.href -match "dl\.dolphin-emu\.org.*/dolphin-\d+[a-z]?-x86_64\.flatpak$" } |
                   Select-Object -First 1).href
        if ($linUrl) { install-emu "Dolphin" "Linux" $linUrl ([IO.Path]::GetFileName($linUrl)) }
        else { Write-Host "   [Dolphin Linux] no download URL found" -ForegroundColor Yellow }
    } catch { Write-Host "   [Dolphin Linux] page fetch failed: $_" -ForegroundColor Yellow }
} else { Write-Host "   [Dolphin Linux] already populated — skipping" -ForegroundColor Gray }

# PS1 — DuckStation
emu "PS1"  "stenzek/duckstation" @("windows-x64-release\.zip$") `
           "stenzek/duckstation" @("x64\.AppImage$")

# PS2 — PCSX2
emu "PS2"  "PCSX2/pcsx2" @("windows-x64-Qt\.(7z|zip)$") `
           "PCSX2/pcsx2"  @("linux-appimage-x64-Qt\.AppImage$")

# PS3 — RPCS3 (separate binary repos)
emu "PS3"  "RPCS3/rpcs3-binaries-win"   @("win64\.(7z|zip)$", "\.7z$", "\.zip$") `
           "RPCS3/rpcs3-binaries-linux" @("AppImage$", "\.AppImage$")

# PSP — PPSSPP
emu "PSP"  "hrydgard/ppsspp" @("Windows-x64\.zip$") `
           "hrydgard/ppsspp" @("anylinux-x86_64\.AppImage$")

# Wii U — CEMU
emu "WiiU" "cemu-project/Cemu" @("windows-x64\.zip$") `
           "cemu-project/Cemu" @("ubuntu.*x64\.zip$","linux.*x64\.zip$")

# Xbox OG — xemu
emu "Xbox" "xemu-project/xemu" @("windows-x86_64\.zip$") `
           "xemu-project/xemu" @("x86_64\.AppImage$")

# ── Done ─────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host " Done! Emulators in:" -ForegroundColor Green
Write-Host "   $BASE" -ForegroundColor White
Write-Host "============================================" -ForegroundColor Green
Get-ChildItem $BASE | Sort-Object Name | ForEach-Object {
    $files = (Get-ChildItem $_.FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count
    $status = if ($files -gt 0) { "[OK] $files files" } else { "[EMPTY]" }
    Write-Host ("   {0,-20} {1}" -f $_.Name, $status)
}

Remove-Item $TEMP -Recurse -Force -ErrorAction SilentlyContinue
