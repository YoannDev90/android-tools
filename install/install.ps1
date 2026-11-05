# Shared helpers for the android-tools installers.
function Log($msg) { Write-Host $msg }
function Err($msg) { Write-Error $msg }
function Write-Ok([string]$Message) { Write-Host " ok $Message" -ForegroundColor Green }
function Write-Warn([string]$Message) { Write-Host "warning: $Message" -ForegroundColor Yellow }

function Stop-Install([string]$Message) {
    Write-Host "error: $Message" -ForegroundColor Red
    $script:Failed = $true
}


function Show-Help {
    Write-Host @'
android-tools installer — pick the tools you want, keep them updated.

Usage: install.ps1 [command] [options] [tool...]

Commands:
  install [tool... | all]    install tools (interactive selector by default)
  update  [tool... | all]    update the tools already installed
  list                       show installed / available tools
  uninstall [tool... | all]  remove tools
  help                       print this help

Options:
  -All             every tool, no selector
  -BinDir DIR      install directory (default: %LOCALAPPDATA%\android-tools\bin)
  -NoPath          do not touch your user PATH
  -Help            print this help

Examples:
  install.ps1                          # selector
  install.ps1 install all              # everything, no prompt
  install.ps1 install apkctl svg2vd    # a subset
  install.ps1 update                   # updater for what is installed
  install.ps1 list
  install.ps1 uninstall apkctl

From a one-liner, pass arguments with a scriptblock:
  & ([scriptblock]::Create((irm https://raw.githubusercontent.com/YoannDev90/android-tools/main/install/install.ps1))) update
'@
}

# ---------------------------------------------------------------- platform
# Shared SHA-256 helper.
function Get-Sha256File($path) {
    return (Get-FileHash -Path $path -Algorithm SHA256).Hash.ToLower()
}
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
#Requires -Version 5.1
<#
.SYNOPSIS
  android-tools — global installer with an interactive tool selector and a
  built-in updater.

.DESCRIPTION
  Downloads the tools you pick from the latest GitHub release, checks their
  SHA-256 checksums, installs them into a directory of your choice (added to
  your user PATH) and records what was installed so that "update" can refresh
  exactly those tools later.

.PARAMETER Command
  install (default), update, list, uninstall or help. The first argument may
  also be a tool name, which is then treated as an install target.

.PARAMETER Names
  Tools to work on; "all" means every tool.

.PARAMETER All
  Work on every tool, no selector.

.PARAMETER BinDir
  Install directory. Defaults to %LOCALAPPDATA%\android-tools\bin.

.PARAMETER NoPath
  Do not add the install directory to your user PATH.

.PARAMETER Help
  Print this help.

.EXAMPLE
  irm https://raw.githubusercontent.com/YoannDev90/android-tools/main/install/install.ps1 | iex

.EXAMPLE
  .\install.ps1 install apkctl svg2vd

.EXAMPLE
  .\install.ps1 update
#>
param(
    [Parameter(Position = 0)

]
    [string]$Command = 'install',

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$Names = @(),

    [switch]$All,
    [string]$BinDir,
    [switch]$NoPath,
    [switch]$Help
)

$script:IsFile = [bool]$PSCommandPath
$script:Failed = $false
$script:LastError = ''

$Repo = 'YoannDev90/android-tools'
$IndexUrl = "https://raw.githubusercontent.com/$Repo/main/index.json"
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download"
# Kept in sync with index.json by hand: used when the catalog cannot be
# fetched. Library crates stay out — they ship no binary.
$FallbackTools = @(
    'apkctl', 'assetgen', 'dp2px', 'iconkit', 'layoutlint', 'logstream',
    'perfpoll', 'resclean', 'scenediff', 'startperf', 'svg2vd', 'uitool'
)
$Commands = @('install', 'update', 'list', 'uninstall', 'help')

$script:Tools = @()
$script:LatestVersion = ''
$script:State = @{}
$script:Target = ''
$script:TempDir = ''

# ------------------------------------------------------------------ output

function Get-Catalog {
    $raw = Get-Text $IndexUrl
    if ($raw) {
        try {
            $index = $raw | ConvertFrom-Json
            $tools = @()
            foreach ($t in @($index.tools)) {
                if ($t.category -eq 'library') { continue }
                $tools += [pscustomobject]@{ Id = [string]$t.id; Description = [string]$t.description }
            }
            if ($tools.Count -gt 0) {
                $script:Tools = $tools
                $script:LatestVersion = [string]$index.version
                return
            }
        } catch { }
    }
    $script:Tools = @($FallbackTools | ForEach-Object { [pscustomobject]@{ Id = $_; Description = '' } })
    $script:LatestVersion = Get-VersionFromRedirect
    Write-Warn 'catalog unavailable — falling back to the built-in tool list'
}

function Test-KnownTool([string]$Name) {
    foreach ($t in $script:Tools) {
        if ($t.Id -eq $Name) { return $true }
    }
    return $false
}

function Get-ToolNames {
    $out = @()
    foreach ($t in $script:Tools) { $out += $t.Id }
    return $out
}

# ------------------------------------------------------------------- state

function Read-State {
    $script:State = @{}
    if (-not (Test-Path $script:StateFile)) { return }
    foreach ($line in Get-Content $script:StateFile -ErrorAction SilentlyContinue) {
        if (-not $line -or $line.StartsWith('#')) { continue }
        $parts = @($line -split '\s+' | Where-Object { $_ })
        if ($parts.Count -eq 0) { continue }
        $version = ''
        $sha = ''
        if ($parts.Count -ge 2) { $version = $parts[1] }
        if ($parts.Count -ge 3) { $sha = $parts[2] }
        $script:State[$parts[0]] = [pscustomobject]@{ Version = $version; Sha = $sha }
    }
}

function Save-State {
    $dir = Split-Path $script:StateFile -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $lines = @('# android-tools installer state: <tool> <version> <sha256>')
    foreach ($key in @($script:State.Keys | Sort-Object)) {
        $lines += "$key $($script:State[$key].Version) $($script:State[$key].Sha)"
    }
    Set-Content -Path $script:StateFile -Value $lines -Encoding ASCII
}

function Set-StateEntry([string]$Tool, [string]$Version, [string]$Sha) {
    $script:State[$Tool] = [pscustomobject]@{ Version = $Version; Sha = $Sha }
    Save-State
}

# ------------------------------------------------------------ tool checks

function Test-Installed([string]$Tool) {
    return [bool](Test-Path -LiteralPath (Join-Path $script:BinDir "$Tool.exe"))
}

function Get-InstalledVersion([string]$Tool) {
    if ($script:State.ContainsKey($Tool)) {
        $v = $script:State[$Tool].Version
        if ($v -and $v -ne 'unknown') { return $v }
    }
    $exe = Join-Path $script:BinDir "$Tool.exe"
    if (Test-Path -LiteralPath $exe) {
        try {
            $out = & $exe --version 2>$null | Select-Object -First 1
            if ("$out" -match '(\d+\.\d+(?:\.\d+)*)') { return $Matches[1] }
        } catch { }
    }
    return 'unknown'
}

function Get-InstalledTools {
    $list = @()
    foreach ($key in @($script:State.Keys)) {
        if (Test-Installed $key) { $list += $key }
    }
    foreach ($t in $script:Tools) {
        if ((Test-Installed $t.Id) -and ($list -notcontains $t.Id)) { $list += $t.Id }
    }
    return $list
}

# ------------------------------------------------------- install / update

function Install-Tool([string]$Tool) {
    $asset = "$Tool-$($script:Target).zip"
    $url = "$ReleaseUrl/$asset"
    $work = Join-Path $script:TempDir $Tool
    if (Test-Path $work) { Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue }
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    $zip = Join-Path $work $asset

    if (-not (Invoke-Download $url $zip)) {
        Write-Warn "cannot download $asset — $($script:LastError)"
        return $false
    }

    $sha = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLower()
    $remote = ''
    $remoteText = Get-Text "$url.sha256"
    if ($remoteText) { $remote = (($remoteText -split '\s+')[0]).ToLower() }
    if ($remote -and $remote -ne $sha) {
        Write-Warn "checksum mismatch for $asset (expected $remote, got $sha)"
        return $false
    }

    try {
        Expand-Archive -Path $zip -DestinationPath $work -Force -ErrorAction Stop
    } catch {
        Write-Warn "cannot extract $asset — $($_.Exception.Message)"
        return $false
    }

    $src = Get-ChildItem -Path $work -Recurse -File -Filter "$Tool.exe" -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $src) {
        Write-Warn "binary $Tool.exe not found inside $asset"
        return $false
    }

    if (-not (Test-Path $script:BinDir)) {
        New-Item -ItemType Directory -Path $script:BinDir -Force | Out-Null
    }
    Copy-Item -LiteralPath $src.FullName -Destination (Join-Path $script:BinDir "$Tool.exe") -Force

    $version = $script:LatestVersion
    if (-not $version) { $version = 'unknown' }
    Set-StateEntry $Tool $version $sha

    $exe = Join-Path $script:BinDir "$Tool.exe"
    $ran = $false
    try {
        & $exe --version 2>$null | Out-Null
        $ran = ($LASTEXITCODE -eq 0)
    } catch { $ran = $false }
    if (-not $ran) { Write-Warn "$Tool is installed but '$Tool --version' failed to run" }

    return $true
}

function Test-NeedsUpdate([string]$Tool) {
    if (-not $script:State.ContainsKey($Tool)) {
        # Binary present but untracked (installed by hand or another channel):
        # refresh it so the next run has a baseline.
        return $true
    }
    $local = $script:State[$Tool]
    $remote = ''
    $remoteText = Get-Text "$ReleaseUrl/$Tool-$($script:Target).zip.sha256"
    if ($remoteText) { $remote = (($remoteText -split '\s+')[0]).ToLower() }
    if ($remote -and $local.Sha) { return ($remote -ne $local.Sha) }
    if ($local.Version -and $script:LatestVersion) { return ($local.Version -ne $script:LatestVersion) }
    return $true # cannot tell apart → refresh
}

# --------------------------------------------------------------- selector

# Reads the console. With `irm … | iex` the pipeline is not the script, so the
# console is still the input — redirected stdin is detected up front instead.
function Test-Interactive {
    try { return -not [Console]::IsInputRedirected } catch { return $false }
}

# Returns the selected tool names, @() when nothing is selected, $null when
# there is no terminal to prompt on.
function Select-Tools {
    if (-not (Test-Interactive)) { return $null }

    $selected = @()
    for ($i = 0; $i -lt $script:Tools.Count; $i++) {
        if (-not (Test-Installed $script:Tools[$i].Id)) { $selected += $i }
    }

    while ($true) {
        Write-Host ''
        Write-Host "android-tools — install to $($script:BinDir)"
        for ($i = 0; $i -lt $script:Tools.Count; $i++) {
            $t = $script:Tools[$i]
            $mark = ' '
            if ($selected -contains $i) { $mark = 'x' }
            $line = ' [{0}] {1,2}  {2,-11} {3}' -f $mark, ($i + 1), $t.Id, $t.Description
            if (Test-Installed $t.Id) {
                Write-Host $line -NoNewline
                Write-Host ' (installed)' -ForegroundColor DarkGray
            } else {
                Write-Host $line
            }
        }

        $answer = $null
        try {
            $answer = Read-Host ' numbers toggle (e.g. 1 4 7), a=all, n=none, Enter=install, q=quit'
        } catch {
            return $null
        }
        if ($null -eq $answer) { return $null }

        $tokens = @($answer -split '[\s,;]+' | Where-Object { $_ })
        if ($tokens.Count -eq 0) { break } # Enter confirms

        foreach ($token in $tokens) {
            switch -Regex ($token) {
                '^(q|quit|exit)$' {
                    Write-Host ''
                    Write-Host 'Aborted, nothing installed.'
                    return @()
                    break
                }
                '^(a|all)$' {
                    $selected = @(0..($script:Tools.Count - 1))
                    break
                }
                '^(n|none|clear)$' {
                    $selected = @()
                    break
                }
                '^\d+$' {
                    $n = [int]$token
                    if ($n -ge 1 -and $n -le $script:Tools.Count) {
                        $idx = $n - 1
                        if ($selected -contains $idx) {
                            $selected = @($selected | Where-Object { $_ -ne $idx })
                        } else {
                            $selected += $idx
                        }
                    } else {
                        Write-Warn "out of range: $token"
                    }
                    break
                }
                default {
                    $found = $false
                    for ($i = 0; $i -lt $script:Tools.Count; $i++) {
                        if ($script:Tools[$i].Id -eq $token) {
                            $found = $true
                            if ($selected -contains $i) {
                                $selected = @($selected | Where-Object { $_ -ne $i })
                            } else {
                                $selected += $i
                            }
                        }
                    }
                    if (-not $found) { Write-Warn "unknown tool: $token" }
                    break
                }
            }
        }
    }

    if ($selected.Count -eq 0) {
        Write-Host ''
        Write-Host 'No tool selected.'
        return @()
    }
    $out = @()
    foreach ($i in ($selected | Sort-Object)) { $out += $script:Tools[$i].Id }
    return $out
}

# ------------------------------------------------------------- PATH handling

function Add-SessionPath {
    $parts = @($env:Path -split ';' | Where-Object { $_ })
    if ($parts -notcontains $script:BinDir) {
        $env:Path = ($parts + $script:BinDir) -join ';'
    }
}

function Add-UserPath {
    if ($NoPath) { return }
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @()
    if ($user) { $parts = @($user -split ';' | Where-Object { $_ }) }
    if ($parts -contains $script:BinDir) { return }

    $new = ($parts + $script:BinDir) -join ';'
    if ($new.Length -gt 1500) {
        Write-Warn "your user PATH is too long to change automatically — add this directory by hand:"
        Write-Host "    $($script:BinDir)"
        return
    }
    try {
        [Environment]::SetEnvironmentVariable('Path', $new, 'User')
        Write-Host "added $($script:BinDir) to your user PATH (restart the terminal to pick it up)"
    } catch {
        Write-Warn "could not update your user PATH — add $($script:BinDir) by hand"
    }
}

function Show-PathNote {
    $parts = @($env:Path -split ';' | Where-Object { $_ })
    if ($parts -contains $script:BinDir) { return }
    Write-Warn "$($script:BinDir) is not in your PATH — add it to your user PATH (Win+R, run sysdm.cpl, Advanced, Environment variables)."
    Write-Host "    $($script:BinDir)"
}

# --------------------------------------------------------------- commands

function Resolve-Names([string[]]$Requested) {
    $out = @()
    foreach ($name in $Requested) {
        if ($name -eq 'all') { return Get-ToolNames }
        if (-not (Test-KnownTool $name)) {
            Stop-Install "unknown tool: $name (known: $((Get-ToolNames) -join ' '))"
            return @()
        }
        $out += $name
    }
    return $out
}

function Invoke-Install {
    $targets = @()
    if ($All) {
        $targets = @(Get-ToolNames)
    } elseif ($Names.Count -gt 0) {
        $targets = @(Resolve-Names $Names)
        if ($script:Failed) { return }
    } else {
        $targets = Select-Tools
        if ($null -eq $targets) {
            Stop-Install 'no terminal to prompt on — pass -All or name the tools (install.ps1 install apkctl svg2vd)'
            return
        }
        $targets = @($targets)
    }
    if ($targets.Count -eq 0) { return }

    Write-Host "Installing $($targets.Count) tool(s) → $($script:BinDir)"
    $done = 0
    $failed = 0
    foreach ($tool in $targets) {
        if (Install-Tool $tool) {
            Write-Ok "$tool $($script:LatestVersion)"
            $done++
        } else {
            $failed++
        }
    }

    Write-Host ''
    if ($done -gt 0) {
        if ($NoPath) {
            Show-PathNote
        } else {
            Add-SessionPath
            Add-UserPath
        }
    }
    if ($failed -eq 0) {
        Write-Host "Run 'install.ps1 update' any time to refresh them."
    } else {
        Write-Warn "$done installed, $failed failed"
        $script:Failed = $true
    }
}

function Invoke-Update {
    $installed = @(Get-InstalledTools)
    $targets = @()
    if ($All) {
        $targets = $installed
    } elseif ($Names.Count -gt 0) {
        $targets = @(Resolve-Names $Names)
        if ($script:Failed) { return }
    } else {
        $targets = $installed
    }

    if ($targets.Count -eq 0) {
        Write-Host "Nothing installed yet — run 'install.ps1' first."
        return
    }

    Write-Host "Checking $($targets.Count) installed tool(s) against the latest release"
    $updated = 0
    $unchanged = 0
    $failed = 0
    foreach ($tool in $targets) {
        if (-not (Test-Installed $tool)) {
            Write-Warn "$tool is not installed — skipped"
            $failed++
            continue
        }
        if (Test-NeedsUpdate $tool) {
            if (Install-Tool $tool) {
                Write-Ok "$tool → $($script:LatestVersion)"
                $updated++
            } else {
                $failed++
            }
        } else {
            Write-Host "  $tool up to date"
            $unchanged++
        }
    }

    Write-Host ''
    Write-Host "$updated updated, $unchanged unchanged, $failed failed"
    if ($failed -gt 0) { $script:Failed = $true }
}

function Show-List {
    $head = 'android-tools'
    if ($script:LatestVersion) { $head += " $($script:LatestVersion)" }
    Write-Host "$head — $($script:BinDir)"

    $shown = 0
    foreach ($t in $script:Tools) {
        if (-not (Test-Installed $t.Id)) { continue }
        $version = Get-InstalledVersion $t.Id
        if ($script:LatestVersion -and $version -ne 'unknown' -and $version -ne $script:LatestVersion) {
            Write-Host ('  {0,-11} {1,-9} update available ({1} → {2})' -f $t.Id, $version, $script:LatestVersion)
        } else {
            Write-Host ('  {0,-11} {1,-9} up to date' -f $t.Id, $version)
        }
        $shown++
    }
    if ($shown -eq 0) { Write-Host '  nothing installed yet' }

    Write-Host ''
    Write-Host 'Available:'
    foreach ($t in $script:Tools) {
        if (-not (Test-Installed $t.Id)) {
            Write-Host ('  {0,-11} {1}' -f $t.Id, $t.Description)
        }
    }
}

function Invoke-Uninstall {
    $targets = @()
    if ($All) {
        $targets = @(Get-InstalledTools)
    } elseif ($Names.Count -gt 0) {
        $targets = @(Resolve-Names $Names)
        if ($script:Failed) { return }
    } else {
        Stop-Install 'name the tools to remove, or pass -All'
        return
    }

    if ($targets.Count -eq 0) {
        Write-Host 'Nothing installed.'
        return
    }

    foreach ($tool in $targets) {
        $exe = Join-Path $script:BinDir "$tool.exe"
        if (Test-Path -LiteralPath $exe) {
            Remove-Item -LiteralPath $exe -Force
            if ($script:State.ContainsKey($tool)) {
                $script:State.Remove($tool)
                Save-State
            }
            Write-Ok "$tool removed"
        } else {
            Write-Host "  $tool was not installed"
        }
    }
}

# -------------------------------------------------------------------- main

function Invoke-Main {
    if ($Help -or $Command -eq 'help') {
        Show-Help
        return
    }

    Initialize-Platform
    if ($script:Failed) { return }

    Get-Catalog
    Read-State

    switch ($Command) {
        'install' { Invoke-Install }
        'update' { Invoke-Update }
        'list' { Show-List }
        'uninstall' { Invoke-Uninstall }
        default {
            Stop-Install "unknown command: $Command (see install.ps1 -Help)"
        }
    }
}

# The first argument may be a tool name rather than a command.
if ($Commands -notcontains $Command) {
    $Names = @(@($Command) + @($Names))
    $Command = 'install'
}

$script:TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $script:TempDir -Force | Out-Null
    Invoke-Main
} catch {
    $message = $_.Exception.Message
    if (-not $message) { $message = "$_" }
    Stop-Install $message
} finally {
    if ($script:TempDir -and (Test-Path $script:TempDir)) {
        Remove-Item -Path $script:TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($script:Failed -and $script:IsFile) { exit 1 }
