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

