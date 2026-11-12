# Signs one file with Azure Artifact Signing (SignTool + the Artifact Signing
# dlib). Tauri calls this for every binary it bundles (courier.exe, the
# uninstaller and the installer) through bundle.windows.signCommand.
#
# Nothing account-specific lives in the repo. Signing is configured per
# machine, either by three environment variables:
#   ARTIFACT_SIGNING_ENDPOINT, ARTIFACT_SIGNING_ACCOUNT, ARTIFACT_SIGNING_PROFILE
# or by a metadata.json file (Endpoint, CodeSigningAccountName,
# CertificateProfileName), looked up in this order:
#   1. $env:COURIER_SIGNING_METADATA   (Courier only)
#   2. $env:ARTIFACT_SIGNING_METADATA  (machine-wide, shared with other projects)
#   3. %APPDATA%\ArtifactSigning\metadata.json
# The dlib likewise comes from COURIER_SIGNING_DLIB, ARTIFACT_SIGNING_DLIB,
# or the Client Tools install folder.
# Authentication: DefaultAzureCredential, i.e. AZURE_TENANT_ID /
# AZURE_CLIENT_ID / AZURE_CLIENT_SECRET (an app registration) if set,
# otherwise your `az login`.
#
# Without any signing settings the file is left unsigned and the build continues,
# so anyone can build Courier. Set COURIER_REQUIRE_SIGNING=1 (release builds)
# to make a missing or failed signature stop the build instead.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File tools\sign.ps1 <file>
param([Parameter(Mandatory = $true)][string]$File)

$ErrorActionPreference = 'Stop'
$required = $env:COURIER_REQUIRE_SIGNING -eq '1'
$name = Split-Path -Leaf $File

function Skip([string]$why) {
    if ($required) { Write-Error "[sign] $name NOT signed: $why (COURIER_REQUIRE_SIGNING=1)"; exit 1 }
    Write-Host "[sign] $name left unsigned: $why"
    exit 0
}

# Account settings: three environment variables, or a metadata.json.
$fromEnv = $env:ARTIFACT_SIGNING_ENDPOINT -and $env:ARTIFACT_SIGNING_ACCOUNT -and $env:ARTIFACT_SIGNING_PROFILE
if (-not $fromEnv) {
    $metadata = $env:COURIER_SIGNING_METADATA
    if (-not $metadata) { $metadata = $env:ARTIFACT_SIGNING_METADATA }
    if (-not $metadata) { $metadata = Join-Path $env:APPDATA 'ArtifactSigning\metadata.json' }
    if (-not (Test-Path $metadata)) { Skip "no signing settings (ARTIFACT_SIGNING_ENDPOINT/ACCOUNT/PROFILE, or a metadata.json at $metadata)" }
}

# SignTool: newest x64 build from the Windows SDK (Artifact Signing needs
# 10.0.22621.755 or later; SDK 20348 is not supported by the dlib).
$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\signtool.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch '10\.0\.20348' } |
    Sort-Object { [version]($_.Directory.Parent.Name) } | Select-Object -Last 1 -ExpandProperty FullName
if (-not $signtool) { Skip 'signtool.exe not found (install the Windows SDK)' }

# Artifact Signing dlib (x64, to match SignTool).
$dlib = $env:COURIER_SIGNING_DLIB
if (-not $dlib) { $dlib = $env:ARTIFACT_SIGNING_DLIB }
if (-not $dlib) {
    $roots = @("$env:LOCALAPPDATA\Microsoft\MicrosoftArtifactSigningClientTools",
               "$env:ProgramFiles\Microsoft\ArtifactSigningClientTools",
               "${env:ProgramFiles(x86)}\Microsoft\ArtifactSigningClientTools",
               "$env:LOCALAPPDATA\Microsoft\MicrosoftTrustedSigningClientTools")
    $dlib = $roots | Where-Object { Test-Path $_ } |
        ForEach-Object { Get-ChildItem $_ -Recurse -Filter Azure.CodeSigning.Dlib.dll -ErrorAction SilentlyContinue } |
        Where-Object { $_.FullName -notmatch '\\x86\\' } | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $dlib -or -not (Test-Path $dlib)) { Skip 'Azure.CodeSigning.Dlib.dll not found (install Microsoft.Azure.ArtifactSigningClientTools)' }

if ($fromEnv) {
    # The dlib only reads a file: write a temporary one for this signature.
    $metadata = Join-Path $env:TEMP ("artifact-signing-" + [guid]::NewGuid().ToString('N') + ".json")
    [ordered]@{
        Endpoint               = $env:ARTIFACT_SIGNING_ENDPOINT
        CodeSigningAccountName = $env:ARTIFACT_SIGNING_ACCOUNT
        CertificateProfileName = $env:ARTIFACT_SIGNING_PROFILE
    } | ConvertTo-Json | Set-Content -Path $metadata -Encoding ASCII
}

Write-Host "[sign] signing $name"
try {
    & $signtool sign /fd SHA256 /tr 'http://timestamp.acs.microsoft.com' /td SHA256 /dlib $dlib /dmdf $metadata $File
} finally {
    if ($fromEnv) { Remove-Item $metadata -ErrorAction SilentlyContinue }
}
if ($LASTEXITCODE -ne 0) {
    if ($required) { Write-Error "[sign] signing $name failed (exit $LASTEXITCODE)"; exit 1 }
    Write-Host "[sign] signing $name failed (exit $LASTEXITCODE); left unsigned"
    exit 0
}
& $signtool verify /pa $File | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Error "[sign] $name was signed but does not verify"; exit 1 }
Write-Host "[sign] $name signed and verified"

# The installer is signed right after NSIS packs it, and before Tauri puts
# its unsigned working copy of courier.exe back in target\release. So at this
# moment target\release\courier.exe is exactly the file inside the installer:
# make sure it's signed too.
if ($name -like '*-setup.exe') {
    $app = Join-Path (Split-Path -Parent $File) '..\..\courier.exe'
    $sig = if (Test-Path $app) { Get-AuthenticodeSignature $app } else { $null }
    if ($sig -and $sig.Status -eq 'Valid') {
        Write-Host "[sign] packaged courier.exe: signed by $($sig.SignerCertificate.Subject -replace ',.*$','')"
    } else {
        $status = if ($sig) { $sig.Status } else { 'not found' }
        if ($required) { Write-Error "[sign] packaged courier.exe is not signed ($status)"; exit 1 }
        Write-Host "[sign] warning: packaged courier.exe is not signed ($status)"
    }
}
