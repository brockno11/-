param(
    [string]$ExecutablePath = "release/current/qingjian.exe",
    [ValidateRange(1, 500)]
    [int]$Rounds = 50
)

$ErrorActionPreference = "Stop"
$resolvedExecutable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
if (-not $resolvedExecutable.StartsWith($workspace, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "The test executable must be inside the current workspace"
}

$packagePath = Join-Path $workspace "package.json"
$expectedVersion = [string](Get-Content -LiteralPath $packagePath -Raw | ConvertFrom-Json).version
$versionInfo = (Get-Item -LiteralPath $resolvedExecutable).VersionInfo
$actualFileVersion = [string]$versionInfo.FileVersion
$actualProductVersion = [string]$versionInfo.ProductVersion
if ($actualFileVersion -ne $expectedVersion -or $actualProductVersion -ne $expectedVersion) {
    throw "QA executable version mismatch: expected $expectedVersion, file=$actualFileVersion, product=$actualProductVersion"
}

$binaryText = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($resolvedExecutable)
)
if ($binaryText.Contains("localhost:1420")) {
    throw "Production executable contains localhost:1420"
}

$workbenchTitle = -join [char[]](0x8F7B, 0x7B3A, 0x5DE5, 0x4F5C, 0x53F0)
$closeButtonName = -join [char[]](0x9690, 0x85CF, 0x5230, 0x7CFB, 0x7EDF, 0x6258, 0x76D8)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class QingjianLifecycleWindow {
    [DllImport("user32.dll", EntryPoint = "FindWindowW", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string className, string windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")]
    public static extern bool IsIconic(IntPtr window);
}
'@

function Get-TargetProcesses {
    @(Get-Process -Name "qingjian" -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -eq $resolvedExecutable } catch { $false }
    })
}

function Get-CloseButton([IntPtr]$window) {
    if ($window -eq [IntPtr]::Zero) { return $null }
    try {
        $root = [Windows.Automation.AutomationElement]::FromHandle($window)
        return $root.FindAll(
            [Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.Condition]::TrueCondition
        ) | Where-Object {
            $_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and
            $_.Current.Name -eq $closeButtonName
        } | Select-Object -First 1
    } catch {
        return $null
    }
}

function Wait-ForWorkbench([int]$expectedProcessId, [int]$timeoutSeconds = 8) {
    $deadline = [DateTime]::UtcNow.AddSeconds($timeoutSeconds)
    do {
        Start-Sleep -Milliseconds 80
        $processes = Get-TargetProcesses
        $window = [QingjianLifecycleWindow]::FindWindow("Tauri Window", $workbenchTitle)
        $button = if (
            $processes.Count -eq 1 -and
            $processes[0].Id -eq $expectedProcessId -and
            $window -ne [IntPtr]::Zero -and
            [QingjianLifecycleWindow]::IsWindowVisible($window) -and
            -not [QingjianLifecycleWindow]::IsIconic($window)
        ) { Get-CloseButton $window } else { $null }
    } until ($null -ne $button -or [DateTime]::UtcNow -gt $deadline)
    if ($null -eq $button) {
        throw "Workbench did not become the unique visible non-minimized window within ${timeoutSeconds}s"
    }
    @{ Process = $processes[0]; Window = $window; CloseButton = $button }
}

# Stop only the exact test executable, leaving other development copies alone.
Get-TargetProcesses | Stop-Process -Force
Start-Sleep -Milliseconds 300
Start-Process -FilePath $resolvedExecutable

$deadline = [DateTime]::UtcNow.AddSeconds(20)
do {
    Start-Sleep -Milliseconds 100
    $processes = Get-TargetProcesses
} until ($processes.Count -eq 1 -or [DateTime]::UtcNow -gt $deadline)
if ($processes.Count -ne 1) { throw "Cold start did not create exactly one instance" }

$primaryProcessId = $processes[0].Id
$state = Wait-ForWorkbench $primaryProcessId 20
for ($round = 1; $round -le $Rounds; $round++) {
    $workbench = $state.Window
    $state.CloseButton.GetCurrentPattern(
        [Windows.Automation.InvokePattern]::Pattern
    ).Invoke()

    $deadline = [DateTime]::UtcNow.AddSeconds(3)
    do {
        Start-Sleep -Milliseconds 40
        $hidden = -not [QingjianLifecycleWindow]::IsWindowVisible($workbench)
    } until ($hidden -or [DateTime]::UtcNow -gt $deadline)
    if (-not $hidden) { throw "Round ${round}: the real close button did not hide the workbench" }

    Start-Process -FilePath $resolvedExecutable
    $state = Wait-ForWorkbench $primaryProcessId
    if (-not $state.Process.Responding) { throw "Round ${round}: primary process is not responding" }
    if ($round % 10 -eq 0 -or $round -eq $Rounds) {
        Write-Output "round=$round pid=$primaryProcessId count=1 visible=true minimized=false responding=true"
    }
}

$stream = [System.IO.File]::OpenRead($resolvedExecutable)
try {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = [System.BitConverter]::ToString($sha256.ComputeHash($stream)).Replace("-", "")
    } finally {
        $sha256.Dispose()
    }
} finally {
    $stream.Dispose()
}
Write-Output "lifecycle=passed rounds=$Rounds pid=$primaryProcessId sha256=$hash"
