$token = '4b9500c4-1488-4d47-9d8f-1403d5169fe9'
$base  = 'https://drop.canpop.synology.me'
$h     = @{ Authorization = "Bearer $token" }
$gameId = '0cef0064-d5de-4331-ba6a-e9bed73e40a8'

# Try to get versions for this specific game
$endpoints = @(
    "/api/v1/admin/games/$gameId/versions",
    "/api/v1/admin/library/$gameId",
    "/api/v1/admin/library/$gameId/versions",
    "/api/v1/games/$gameId/versions"
)

foreach ($ep in $endpoints) {
    Write-Host "=== GET $ep ==="
    try {
        $r = Invoke-RestMethod -Uri "$base$ep" -Headers $h
        $r | ConvertTo-Json -Depth 5
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)"
    }
    Write-Host ""
}
