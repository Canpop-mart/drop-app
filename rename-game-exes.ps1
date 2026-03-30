# rename-game-exes.ps1
# Renames .exe files that contain spaces by removing all spaces.
# Run from PowerShell. Preview mode is on by default — pass -Apply to actually rename.

param(
    [string]$Path = "Z:\media\emulation\Games",
    [switch]$Apply
)

$exes = Get-ChildItem -Path $Path -Recurse -Filter "*.exe" |
        Where-Object { $_.Name -match " " }

if ($exes.Count -eq 0) {
    Write-Host "No .exe files with spaces found under $Path" -ForegroundColor Green
    exit
}

Write-Host ""
Write-Host "Found $($exes.Count) .exe file(s) with spaces:" -ForegroundColor Cyan
Write-Host ""

$renames = @()
foreach ($exe in $exes) {
    $newName = $exe.Name -replace " ", ""
    $newPath = Join-Path $exe.DirectoryName $newName
    $renames += [PSCustomObject]@{
        Folder  = $exe.DirectoryName
        OldName = $exe.Name
        NewName = $newName
        Conflict = Test-Path $newPath
    }
}

# Print preview table
$renames | Format-Table -AutoSize OldName, NewName, Conflict, Folder

$conflicts = $renames | Where-Object { $_.Conflict }
if ($conflicts.Count -gt 0) {
    Write-Host "WARNING: $($conflicts.Count) conflict(s) - a file with the new name already exists. These will be skipped." -ForegroundColor Yellow
    Write-Host ""
}

if (-not $Apply) {
    Write-Host "--- DRY RUN (nothing renamed) ---" -ForegroundColor Yellow
    Write-Host "Run again with -Apply to perform the renames:" -ForegroundColor Yellow

    Write-Host "  .\rename-game-exes.ps1 -Apply" -ForegroundColor White
    Write-Host ""
    exit
}

# Apply renames
$ok = 0
$skip = 0
foreach ($r in $renames) {
    if ($r.Conflict) {
        Write-Host "SKIP (conflict): $($r.OldName)" -ForegroundColor Yellow
        $skip++
        continue
    }
    $oldPath = Join-Path $r.Folder $r.OldName
    $newPath = Join-Path $r.Folder $r.NewName
    Rename-Item -LiteralPath $oldPath -NewName $r.NewName
    Write-Host "Renamed: $($r.OldName)  ->  $($r.NewName)" -ForegroundColor Green
    $ok++
}

Write-Host ""
Write-Host "Done. $ok renamed, $skip skipped." -ForegroundColor Cyan
Write-Host ""
