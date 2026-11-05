function Write-Ok([string]$Message) { Write-Host " ok $Message" -ForegroundColor Green }
function Write-Warn([string]$Message) { Write-Host "warning: $Message" -ForegroundColor Yellow }

function Stop-Install([string]$Message) {
    Write-Host "error: $Message" -ForegroundColor Red
    $script:Failed = $true
}

