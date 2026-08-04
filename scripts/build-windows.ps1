$ErrorActionPreference = "Stop"

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

npm run check
npm test
npm run build

$command = "call `"$vsDevCmd`" -no_logo -arch=x64 && set `"PATH=$env:USERPROFILE\.cargo\bin;%PATH%`" && npm run tauri build"
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0) {
    throw "Tauri build failed with exit code $LASTEXITCODE"
}

$releaseDir = Join-Path $PSScriptRoot "..\src-tauri\target\release"
$portableDir = Join-Path $releaseDir "portable"
$package = Get-Content (Join-Path $PSScriptRoot "..\package.json") -Raw | ConvertFrom-Json
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
Copy-Item -LiteralPath (Join-Path $releaseDir "qingjian.exe") -Destination (Join-Path $portableDir "qingjian.exe") -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "..\README.md") -Destination (Join-Path $portableDir "README.md") -Force
Compress-Archive -Path (Join-Path $portableDir "*") -DestinationPath (Join-Path $releaseDir "qingjian-$($package.version)-portable.zip") -Force
