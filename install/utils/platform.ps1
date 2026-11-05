function Test-Windows {
    if ($PSVersionTable.PSVersion.Major -ge 6) {
        return ($PSVersionTable.Platform -eq 'Win3NT')
    }
    return ($env:OS -eq 'Windows_NT')
}

function Initialize-Platform {
    if (-not (Test-Windows)) {
        Stop-Install 'run install/install.sh on Linux/macOS'
        return
    }

    # GitHub requires TLS 1.2 on Windows PowerShell 5.1
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch { }

    $arch = $env:PROCESSOR_ARCHITECTURE
    if (-not $arch) {
        try {
            $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        } catch { $arch = '' }
    }
    switch -Regex ($arch.ToUpperInvariant()) {
        '^(X64|AMD64)$' { $script:Target = 'x86_64-pc-windows-msvc'; break }
        '^ARM64$' {
            # No Windows ARM64 build is published; x86_64 runs under emulation.
            $script:Target = 'x86_64-pc-windows-msvc'
            Write-Warn 'ARM64 host: installing the x86_64 build (runs under emulation)'
            break
        }
        default { Stop-Install "unsupported architecture: $arch"; break }
    }

    if (-not $BinDir) {
        if ($env:ANDROID_TOOLS_BIN) { $BinDir = $env:ANDROID_TOOLS_BIN }
        elseif ($env:LOCALAPPDATA) { $BinDir = Join-Path $env:LOCALAPPDATA 'android-tools\bin' }
        else { $BinDir = Join-Path $HOME '.local\bin' }
    }
    $script:BinDir = $BinDir.TrimEnd('\')

    if ($env:ANDROID_TOOLS_STATE) { $stateDir = $env:ANDROID_TOOLS_STATE }
    elseif ($env:LOCALAPPDATA) { $stateDir = Join-Path $env:LOCALAPPDATA 'android-tools' }
    else { $stateDir = Join-Path $HOME '.android-tools' }
    $script:StateFile = Join-Path $stateDir 'installed.tsv'
}

# ---------------------------------------------------------------- network

function Get-VersionFromRedirect {
    $savedPreference = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'
    try {
        $resp = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -UseBasicParsing -TimeoutSec 30 -ErrorAction Stop
        $final = $null
        if ($resp.BaseResponse -and $resp.BaseResponse.PSObject.Properties['ResponseUri']) {
            $final = [string]$resp.BaseResponse.ResponseUri
        } elseif ($resp.BaseResponse -and $resp.BaseResponse.PSObject.Properties['RequestMessage']) {
            $final = [string]$resp.BaseResponse.RequestMessage.RequestUri
        }
        if ($final -and $final -match '/tag/v([0-9][0-9.]*)$') { return $Matches[1] }
    } catch { }
    finally { $ProgressPreference = $savedPreference }
    return ''
}

# ----------------------------------------------------------------- catalog

