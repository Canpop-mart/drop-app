# download-retroarch.ps1
# Downloads RetroArch + all cores into a single directory per platform.
# Output is ready for Drop server import as an Emulator-type game.
#
# Usage:  .\scripts\download-retroarch.ps1
#
# Cores are fetched from the libretro nightly buildbot.
# RetroArch itself is fetched from GitHub releases.

$BASE    = "Z:\media\emulation\Emulators"
$TEMP    = "$env:TEMP\retroarch-dl"
$HEADERS = @{ "User-Agent" = "Mozilla/5.0"; "Accept" = "application/vnd.github+json" }

$7Z = @("C:\Program Files\7-Zip\7z.exe","C:\Program Files (x86)\7-Zip\7z.exe") |
      Where-Object { Test-Path $_ } | Select-Object -First 1

New-Item -ItemType Directory -Path $TEMP -Force | Out-Null

# Buildbot URLs
$BUILDBOT_WIN   = "https://buildbot.libretro.com/nightly/windows/x86_64/latest"
$BUILDBOT_LINUX = "https://buildbot.libretro.com/nightly/linux/x86_64/latest"

# RetroArch stable download (1.22.2)
$RA_WIN_URL   = "https://buildbot.libretro.com/stable/1.22.2/windows/x86_64/RetroArch.7z"
$RA_LINUX_URL = "https://buildbot.libretro.com/stable/1.22.2/linux/x86_64/RetroArch.7z"

# Cores to download
$CORES = @(
    @{ Name = "mesen_libretro";              Desc = "NES / Famicom" },
    @{ Name = "snes9x_libretro";             Desc = "SNES / Super Famicom" },
    @{ Name = "gambatte_libretro";           Desc = "GB / GBC" },
    @{ Name = "mgba_libretro";               Desc = "GBA" },
    @{ Name = "mupen64plus_next_libretro";   Desc = "N64" },
    @{ Name = "swanstation_libretro";        Desc = "PS1" },
    @{ Name = "pcsx2_libretro";              Desc = "PS2" },
    @{ Name = "ppsspp_libretro";             Desc = "PSP" },
    @{ Name = "melonds_libretro";            Desc = "DS" },
    @{ Name = "dolphin_libretro";            Desc = "GameCube / Wii" },
    @{ Name = "genesis_plus_gx_libretro";    Desc = "Genesis / MD / SMS / GG" },
    @{ Name = "mednafen_saturn_libretro";     Desc = "Saturn" },
    @{ Name = "flycast_libretro";            Desc = "Dreamcast" },
    @{ Name = "fbneo_libretro";              Desc = "Arcade (FBNeo)" },
    @{ Name = "picodrive_libretro";          Desc = "32X / Sega CD" }
)

# Helpers

function dl($url, $label) {
    $file = "$TEMP\$([IO.Path]::GetFileName($url.Split('?')[0]))"
    Write-Host "      Downloading $label ... " -NoNewline
    try {
        (New-Object Net.WebClient).DownloadFile($url, $file)
        $mb = [math]::Round((Get-Item $file).Length/1MB,1)
        Write-Host "$mb MB" -ForegroundColor Gray
        return $file
    } catch {
        Write-Host "FAILED ($_)" -ForegroundColor Red; return $null
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
        else { Write-Host "      SKIP - 7-Zip not found for $archive" -ForegroundColor Yellow; return $false }
    } elseif ($ext -eq ".xz") {
        if ($7Z) { & $7Z x $archive "-o$TEMP" -y | Out-Null
            $tar = $archive -replace '\.xz$',''
            if (Test-Path $tar) { & $7Z x $tar "-o$dest" -y | Out-Null; Remove-Item $tar -ErrorAction SilentlyContinue }
            return $true
        } else { Write-Host "      SKIP - 7-Zip not found for $archive" -ForegroundColor Yellow; return $false }
    }
    Copy-Item $archive $dest; return $true
}

function flatten-if-single($dir) {
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

# Download RetroArch base

function Install-RetroArch($platform, $url) {
    $dir = if ($platform -eq "Linux") { "$BASE\RetroArch Linux" } else { "$BASE\RetroArch Windows" }

    Write-Host ""
    Write-Host "   [RetroArch $platform] Base installation" -ForegroundColor Cyan

    $file = dl $url "RetroArch $platform"
    if (-not $file) { return $false }

    $tmp = "$TEMP\ra_$platform"
    if (extract $file $tmp) {
        flatten-if-single $tmp
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        Get-ChildItem $tmp | Copy-Item -Destination $dir -Recurse -Force
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
    Remove-Item $file -ErrorAction SilentlyContinue

    @("cores", "system", "autoconfig", "assets") | ForEach-Object {
        New-Item -ItemType Directory -Path "$dir\$_" -Force | Out-Null
    }

    Write-Host "      -> $dir" -ForegroundColor Green
    return $true
}

# Download cores

function Install-Cores($platform) {
    $dir = if ($platform -eq "Linux") { "$BASE\RetroArch Linux" } else { "$BASE\RetroArch Windows" }
    $coresDir = "$dir\cores"
    $buildbot = if ($platform -eq "Linux") { $BUILDBOT_LINUX } else { $BUILDBOT_WIN }
    $coreExt = if ($platform -eq "Linux") { ".so" } else { ".dll" }

    Write-Host ""
    Write-Host "   [RetroArch $platform] Downloading cores" -ForegroundColor Cyan

    foreach ($core in $CORES) {
        $coreName = $core.Name
        $coreDesc = $core.Desc
        $coreFile = "$coresDir\${coreName}${coreExt}"

        if (Test-Path $coreFile) {
            Write-Host "      ${coreName} (${coreDesc}) -- already exists" -ForegroundColor Gray
            continue
        }

        $zipUrl = "${buildbot}/${coreName}${coreExt}.zip"
        $file = dl $zipUrl "${coreName} (${coreDesc})"
        if (-not $file) { continue }

        $tmp = "$TEMP\core_$coreName"
        if (extract $file $tmp) {
            $coreLib = Get-ChildItem $tmp -Filter "*${coreExt}" -Recurse | Select-Object -First 1
            if ($coreLib) {
                Copy-Item $coreLib.FullName $coreFile -Force
                Write-Host "      -> ${coreName}${coreExt}" -ForegroundColor Green
            } else {
                Write-Host "      ${coreName} -- no ${coreExt} found in archive" -ForegroundColor Yellow
            }
            Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }
        Remove-Item $file -ErrorAction SilentlyContinue
    }
}

# Create BIOS readme

function Create-BiosReadme($platform) {
    $dir = if ($platform -eq "Linux") { "$BASE\RetroArch Linux" } else { "$BASE\RetroArch Windows" }
    $systemDir = "$dir\system"
    New-Item -ItemType Directory -Path $systemDir -Force | Out-Null

    $readme = @'
BIOS Files for RetroArch Cores
==============================

Place required BIOS files in this directory (system/).
RetroArch cores will look here automatically.

PS1 (swanstation / beetle_psx)
  scph5500.bin  (JP)
  scph5501.bin  (US)
  scph5502.bin  (EU)

PS2 (pcsx2)
  ps2-0230a-20080220.bin  (or other BIOS dump)
  Place in system/pcsx2/bios/

DS (melonds)
  bios7.bin
  bios9.bin
  firmware.bin
  Optional: melonDS can run without BIOS in some modes

GBA (mgba)
  gba_bios.bin
  Optional: mGBA has a built-in open-source BIOS replacement

Saturn (beetle_saturn)
  sega_101.bin  (JP)
  mpr-17933.bin (US/EU)

Dreamcast (flycast)
  dc_boot.bin
  dc_flash.bin
  Place in system/dc/

Arcade (fbneo)
  neogeo.zip  (Neo Geo BIOS)
  Place in the same directory as your ROM files, or here.

GameCube / Wii (dolphin)
  No BIOS required. Dolphin includes HLE.

NES, SNES, GB/GBC, GBA, N64, Genesis, 32X
  No BIOS required for most games.

Note: BIOS files cannot be distributed due to copyright.
You must dump them from your own consoles.
'@

    Set-Content -Path "$systemDir\README-BIOS.txt" -Value $readme -Encoding UTF8
    Write-Host "      Created README-BIOS.txt" -ForegroundColor Green
}

# Main

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host " RetroArch + Cores Downloader" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host " Destination: $BASE"
Write-Host " Cores: $($CORES.Count)"
if (-not $7Z) { Write-Host " WARNING: 7-Zip not found. .7z/.xz archives will fail." -ForegroundColor Yellow }
Write-Host ""

# Windows
if (Install-RetroArch "Windows" $RA_WIN_URL) {
    Install-Cores "Windows"
    Create-BiosReadme "Windows"
}

# Linux
if (Install-RetroArch "Linux" $RA_LINUX_URL) {
    Install-Cores "Linux"
    Create-BiosReadme "Linux"
}

# Summary

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host " Done! RetroArch packages" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green

foreach ($p in @("RetroArch Windows", "RetroArch Linux")) {
    $d = "$BASE\$p"
    if (Test-Path $d) {
        $cc = (Get-ChildItem "$d\cores" -File -ErrorAction SilentlyContinue | Measure-Object).Count
        $tf = (Get-ChildItem $d -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count
        Write-Host "   $p -- $cc cores, $tf total files" -ForegroundColor White
    } else {
        Write-Host "   $p -- NOT CREATED" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Next step: import into Drop server" -ForegroundColor Cyan

Remove-Item $TEMP -Recurse -Force -ErrorAction SilentlyContinue
