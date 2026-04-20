param(
    [Parameter(Mandatory = $true)][string]$Base,
    [Parameter(Mandatory = $true)][string]$Token,
    [Parameter(Mandatory = $true)][string]$GameId
)

$h = @{ Authorization = "Bearer $Token" }

$endpoints = @(
    "/api/v1/admin/games/$GameId/versions",
    "/api/v1/admin/library/$GameId",
    "/api/v1/admin/library/$GameId/versions",
    "/api/v1/games/$GameId/versions"
)

foreach ($ep in $endpoints) {
    Write-Host "=== GET $ep ==="
    try {
        $r = Invoke-RestMethod -Uri "$Base$ep" -Headers $h
        $r | ConvertTo-Json -Depth 5
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)"
    }
    Write-Host ""
}
