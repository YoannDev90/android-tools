# Install an android-tools agent skill into the canonical skills directory
# (%USERPROFILE%\.agents\skills\<tool>\SKILL.md) and junction it into any
# detected agent dirs.
#
# Usage: install-skills.ps1 <tool> <path-to-SKILL.md>
param(
    [Parameter(Mandatory=$true)][string]$Tool,
    [Parameter(Mandatory=$true)][string]$SkillFile
)

$agentsHome = if ($env:AGENTS_HOME) { $env:AGENTS_HOME } else { Join-Path $env:USERPROFILE ".agents" }
$canonical = Join-Path $agentsHome "skills\$Tool"
New-Item -ItemType Directory -Force -Path $canonical | Out-Null
Copy-Item -Force $SkillFile (Join-Path $canonical "SKILL.md")

foreach ($agent in @("claude","cursor","codex","gemini")) {
    $agentDir = Join-Path $env:USERPROFILE ".$agent"
    if (-not (Test-Path $agentDir)) { continue }
    $skillsDir = Join-Path $agentDir "skills"
    New-Item -ItemType Directory -Force -Path $skillsDir | Out-Null
    $link = Join-Path $skillsDir $Tool
    if (Test-Path $link) { Remove-Item -Recurse -Force $link }
    New-Item -ItemType Junction -Path $link -Target $canonical | Out-Null
}
Write-Host "Installed skill for $Tool -> $canonical"
