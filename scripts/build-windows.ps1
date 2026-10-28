$ErrorActionPreference = "Stop"
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $workspace

function Invoke-Checked([string]$program, [string[]]$arguments) {
    & $program @arguments
    if ($LASTEXITCODE -ne 0) { throw "$program failed with exit code $LASTEXITCODE" }
}

$vsDevCmd = "C:\BuildTools\Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path -LiteralPath $vsDevCmd)) {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere) {
        $installation = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        $vsDevCmd = Join-Path $installation "Common7\Tools\VsDevCmd.bat"
    }
}

if (-not (Test-Path -LiteralPath $vsDevCmd)) {
    throw "Visual Studio C++ Build Tools not found"
}

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Rust Cargo toolchain not found"
}

Invoke-Checked 'npm.cmd' @('run','quality:release')

$oldSigningKey = $env:TAURI_SIGNING_PRIVATE_KEY
$oldSigningPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
$localKey = Join-Path $env:USERPROFILE '.tauri/qingjian.key'
$localPassword = Join-Path $env:USERPROFILE '.tauri/qingjian.key.password'
if (-not $env:TAURI_SIGNING_PRIVATE_KEY -and (Test-Path -LiteralPath $localKey)) {
    $env:TAURI_SIGNING_PRIVATE_KEY = $localKey
    if (Test-Path -LiteralPath $localPassword) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [IO.File]::ReadAllText($localPassword).TrimEnd("`r","`n") }
}

$command = "call `"$vsDevCmd`" -no_logo -arch=x64 && set `"PATH=$env:USERPROFILE\.cargo\bin;%PATH%`" && npm run tauri build"
try {
    & cmd.exe /d /s /c $command
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed with exit code $LASTEXITCODE" }
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY = $oldSigningKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $oldSigningPassword
}

$releaseDir = Join-Path $PSScriptRoot "..\src-tauri\target\release"
$package = Get-Content (Join-Path $PSScriptRoot "..\package.json") -Raw | ConvertFrom-Json
$executable = Join-Path $releaseDir 'qingjian.exe'
$info = (Get-Item -LiteralPath $executable).VersionInfo
if ($info.FileVersion -ne $package.version -or $info.ProductVersion -ne $package.version) { throw 'Built executable version mismatch' }
$installers = @(Get-ChildItem -LiteralPath (Join-Path $releaseDir 'bundle/nsis') -Filter '*-setup.exe' |
    Where-Object { $_.Name -match [regex]::Escape("_$($package.version)_x64-setup.exe") })
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer matching the current version' }
$current = Join-Path $workspace 'release/current'
$staging = Join-Path $workspace 'release/.staging'
if (Test-Path -LiteralPath $staging) { throw 'Previous release staging exists; review it before rebuilding' }
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item -LiteralPath $executable -Destination (Join-Path $staging 'qingjian.exe')
$installerName = "qingjian-$($package.version)-x64-setup.exe"
Copy-Item -LiteralPath $installers[0].FullName -Destination (Join-Path $staging $installerName)
if (Test-Path -LiteralPath ($installers[0].FullName+'.sig')) {
    Copy-Item -LiteralPath ($installers[0].FullName+'.sig') -Destination (Join-Path $staging ($installerName+'.sig'))
}
$sourceCommit = (& git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot read source commit' }
$sourceStatus = @(& git status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect source state' }
$hashes = @(Get-ChildItem -LiteralPath $staging -File | Sort-Object Name | ForEach-Object {
    [ordered]@{file=$_.Name; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
})
$hashes | ForEach-Object { "$($_.sha256)  $($_.file)" } | Set-Content -LiteralPath (Join-Path $staging 'SHA256SUMS.txt') -Encoding utf8
[ordered]@{version=$package.version;sourceCommit=$sourceCommit;sourceDirty=($sourceStatus.Count -gt 0);
    builtAt=[DateTime]::UtcNow.ToString('o');identifier='com.qingjian.desktop';files=$hashes} |
    ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $staging 'build-manifest.json') -Encoding utf8
if (Test-Path -LiteralPath (Join-Path $current 'qingjian.exe')) {
    $currentExe=[IO.Path]::GetFullPath((Join-Path $current 'qingjian.exe'))
    $runningCurrent=@(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $currentExe })
    if ($runningCurrent.Count) {
        throw "Candidate is ready in $staging, but current is running. Exit Qingjian normally before promoting it; no process was terminated."
    }
}
if (Test-Path -LiteralPath $current) {
    $archive = Join-Path $workspace ('.reviews/build-archive/'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Path $archive -Force | Out-Null
    # Both resolved targets are inside this workspace; preserve the old output.
    $resolvedCurrent = (Resolve-Path -LiteralPath $current).Path
    if (-not $resolvedCurrent.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe current output path' }
    if (-not [IO.Path]::GetFullPath($archive).StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe archive path' }
    Move-Item -LiteralPath $resolvedCurrent -Destination (Join-Path $archive 'current')
}
try {
    Move-Item -LiteralPath $staging -Destination $current
} catch {
    if ($archive -and -not (Test-Path -LiteralPath $current) -and (Test-Path -LiteralPath (Join-Path $archive 'current'))) {
        Move-Item -LiteralPath (Join-Path $archive 'current') -Destination $current
    }
    throw
}
Write-Output "Verified candidate: $current"
