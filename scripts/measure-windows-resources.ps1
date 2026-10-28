param(
    [Parameter(Mandatory=$true)][string]$ExecutablePath,
    [Parameter(Mandatory=$true)][string]$DataIdentifier,
    [Parameter(Mandatory=$true)][string]$OutputPath,
    [int[]]$NoteCounts = @(0,1,5,10),
    [ValidateRange(5,120)][int]$WarmupSeconds = 20
)
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$reviewRoot = [IO.Path]::GetFullPath((Join-Path $workspace '.reviews')) + [IO.Path]::DirectorySeparatorChar
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$output = [IO.Path]::GetFullPath($(if ([IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $workspace $OutputPath }))
if (-not $executable.StartsWith($reviewRoot,[StringComparison]::OrdinalIgnoreCase) -or -not $output.StartsWith($reviewRoot,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'QA executable and output must be inside this workspace .reviews directory'
}
if ($DataIdentifier -notmatch '^com\.qingjian\.qa\.[a-z0-9-]+$') { throw 'An isolated QA identifier is required' }
$binding = Get-Content -LiteralPath "$executable.qa.json" -Raw | ConvertFrom-Json
if ($binding.identifier -ne $DataIdentifier -or $binding.sha256 -ne (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash) {
    throw 'QA executable identity/hash does not match its build record'
}
$dataDirectory = Join-Path $env:LOCALAPPDATA $DataIdentifier
if (-not (Test-Path -LiteralPath (Join-Path $dataDirectory '.qingjian-qa-fixture'))) { throw 'Missing disposable-fixture marker' }
function Read-ProcessTree([int]$rootId) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId)
    $ids = [Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($rootId)
    do {
        $changed = $false
        foreach ($entry in $all) {
            if ($ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $changed = $true }
        }
    } while ($changed)
    @(Get-Process -Id @($ids) -ErrorAction SilentlyContinue | Select-Object Id,CPU,WorkingSet64,PrivateMemorySize64)
}
$running = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -like 'qingjian*.exe' })
foreach ($entry in $running) {
    if ($entry.ExecutablePath -eq $executable) { throw 'Close this QA executable before measuring' }
    $record = "$($entry.ExecutablePath).qa.json"
    if ($entry.ExecutablePath -and (Test-Path -LiteralPath $record)) {
        $other = Get-Content -LiteralPath $record -Raw | ConvertFrom-Json
        if ($other.identifier -eq $DataIdentifier) { throw 'Another executable uses this QA data identifier' }
    }
}
$results = @()
foreach ($count in $NoteCounts) {
    $fixtureText = & python (Join-Path $PSScriptRoot 'qa-fixtures.py') $dataDirectory --notes $count
    if ($LASTEXITCODE -ne 0) { throw 'Fixture setup failed' }
    $fixture = $fixtureText | ConvertFrom-Json
    $expectedIds = @($fixture.noteIds)
    $runId = [guid]::NewGuid().ToString('N')
    $ownedIds = @()
    $startedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $testProcess = Start-Process -FilePath $executable -ArgumentList '--hidden' -WindowStyle Hidden -PassThru
    $ownedIds = @($testProcess.Id)
    try {
        Start-Sleep -Seconds $WarmupSeconds
        if ($testProcess.HasExited) { throw 'QA process exited unexpectedly' }
        $before = @(Read-ProcessTree $testProcess.Id)
        $ownedIds = @($before.Id)
        $sampling = [Diagnostics.Stopwatch]::StartNew()
        Start-Sleep -Seconds 5
        $after = @(Read-ProcessTree $testProcess.Id)
        $ownedIds = @(@($ownedIds)+@($after.Id) | Select-Object -Unique)
        $elapsed = $sampling.Elapsed.TotalSeconds
        $ready = @()
        if ($binding.timingsDirectory -and (Test-Path -LiteralPath $binding.timingsDirectory)) {
            $ready = @(Get-ChildItem -LiteralPath $binding.timingsDirectory -Filter '*.json' | ForEach-Object {
                $timing = Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
                if ($null -ne $timing.uiReadyMs -and (($count -eq 0 -and -not $timing.noteId) -or ($timing.noteId -in $expectedIds))) {
                    $epoch = [double]$timing.timeOrigin + [double]$timing.uiReadyMs
                    if ($epoch -ge $startedAt) {
                        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path (Split-Path -Parent $output) ($runId+'-'+$_.Name))
                        $epoch - $startedAt
                    }
                }
            })
        }
        $cpuDelta = 0.0
        foreach ($entry in $after) {
            $previous = $before | Where-Object Id -eq $entry.Id | Select-Object -First 1
            if ($null -ne $previous) { $cpuDelta += [Math]::Max(0, [double]$entry.CPU - [double]$previous.CPU) }
        }
        $row = [ordered]@{
            runId=$runId; notes=$count; processes=$after.Count; warmupSeconds=$WarmupSeconds
            readyCount=$ready.Count; complete=($ready.Count -eq [Math]::Max(1,$count))
            workingSetMiB=[Math]::Round(($after | Measure-Object WorkingSet64 -Sum).Sum/1MB,2)
            privateMiB=[Math]::Round(($after | Measure-Object PrivateMemorySize64 -Sum).Sum/1MB,2)
            idleCpuOneCorePercent=[Math]::Round(100*$cpuDelta/$elapsed,2)
            firstUiReadyMs=if ($ready.Count) { [Math]::Round(($ready | Measure-Object -Minimum).Minimum,2) } else { $null }
            allNotesReadyMs=if ($ready.Count) { [Math]::Round(($ready | Measure-Object -Maximum).Maximum,2) } else { $null }
            sampledAt=[DateTime]::UtcNow.ToString('o')
        }
        $results += $row
        Write-Output ($row | ConvertTo-Json -Compress)
        [ordered]@{ executable=$executable; sha256=$binding.sha256; identifier=$DataIdentifier;
            metric='Sum of entire process-tree working sets/private bytes; shared pages may be counted more than once';
            latency='QA-only readiness probe when present; no readiness claim if value is null. Input/wake samples are in the separate timing directory.'; samples=$results } |
            ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $output -Encoding utf8
    } finally {
        if (-not $testProcess.HasExited) { Stop-Process -Id $testProcess.Id -Force }
        $deadline = [DateTime]::UtcNow.AddSeconds(8)
        do {
            Start-Sleep -Milliseconds 200
            $remaining = @(Get-Process -Id $ownedIds -ErrorAction SilentlyContinue)
        } while ($remaining.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline)
        if ($remaining.Count -gt 0) { throw 'QA descendants did not exit; refusing another sample' }
    }
}
