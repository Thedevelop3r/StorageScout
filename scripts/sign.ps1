# Signs files with the PC Scanner code signing certificate (Authenticode, SHA-256, timestamped).
#
# Usage:  scripts/sign.ps1 <file> [<file> ...]
# Needs:  $env:SIGNING_PFX_PATH        path to pcscanner-codesign.pfx
#         $env:SIGNING_CERT_PASSWORD   its password
# Also used by Inno Setup (SignTool) to sign the installer and the uninstaller.

param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]] $Files
)

$ErrorActionPreference = 'Stop'

$pfx = $env:SIGNING_PFX_PATH
$password = "$env:SIGNING_CERT_PASSWORD".Trim()
if (-not $pfx -or -not (Test-Path $pfx)) { throw "SIGNING_PFX_PATH is not set or the file does not exist." }
if (-not $password) { throw "SIGNING_CERT_PASSWORD is not set." }

$signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
if (-not $signtool) {
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $signtool) { throw "signtool.exe not found (install the Windows SDK)." }

$timestampServers = @(
    'http://timestamp.digicert.com',
    'http://timestamp.sectigo.com',
    'http://timestamp.globalsign.com/tsa/r6advanced1'
)

foreach ($file in $Files) {
    $file = $file.Trim('"')
    $signed = $false
    foreach ($ts in $timestampServers) {
        & $signtool sign /fd SHA256 /f $pfx /p $password /tr $ts /td SHA256 `
            /d "PC Scanner" /du "https://github.com/Thedevelop3r/pcscanner" $file
        if ($LASTEXITCODE -eq 0) { $signed = $true; break }
        Write-Warning "Signing with timestamp server $ts failed; trying the next one."
        Start-Sleep -Seconds 3
    }
    if (-not $signed) { throw "Could not sign $file" }
    Write-Host "Signed $file"
}
