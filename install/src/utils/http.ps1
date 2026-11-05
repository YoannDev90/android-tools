function Get-Text([string]$Url) {
    $script:LastError = ''
    $savedPreference = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue' # no progress bar for one-liners
    try {
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                $content = (Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 60 -ErrorAction Stop).Content
                # Assets served as application/octet-stream come back as bytes.
                if ($content -is [byte[]]) { return [System.Text.Encoding]::UTF8.GetString($content) }
                return [string]$content
            } catch {
                $script:LastError = $_.Exception.Message
                Start-Sleep -Milliseconds 400
            }
        }
        return $null
    } finally {
        $ProgressPreference = $savedPreference
    }
}

function Invoke-Download([string]$Url, [string]$OutFile) {
    $script:LastError = ''
    $savedPreference = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'
    try {
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing -TimeoutSec 180 -ErrorAction Stop | Out-Null
                if ((Test-Path $OutFile) -and ((Get-Item $OutFile).Length -gt 0)) { return $true }
                $script:LastError = "empty download: $Url"
            } catch {
                $script:LastError = $_.Exception.Message
                Start-Sleep -Milliseconds 400
            }
        }
        return $false
    } finally {
        $ProgressPreference = $savedPreference
    }
}

# Last-resort version: /releases/latest redirects straight at the tag, needs no
# API quota and no JSON parser.
