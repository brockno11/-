param(
    [Parameter(Mandatory=$true)][string]$Before,
    [Parameter(Mandatory=$true)][string]$After,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [ValidateRange(3,10)][int]$Pairs = 5
)
$ErrorActionPreference='Stop'
$workspace=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$reviewRoot=$workspace+'\.reviews\'
$output=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($OutputDirectory)){$OutputDirectory}else{Join-Path $workspace $OutputDirectory}))
if (-not $output.StartsWith($reviewRoot,[StringComparison]::OrdinalIgnoreCase)) { throw 'Comparison output must be inside .reviews' }
if ((Test-Path -LiteralPath $output) -and @(Get-ChildItem -LiteralPath $output -Force).Count) { throw 'Use a fresh comparison directory to preserve previous evidence' }
New-Item -ItemType Directory -Path $output -Force | Out-Null
$beforePath=(Resolve-Path -LiteralPath $Before).Path
$afterPath=(Resolve-Path -LiteralPath $After).Path
$bindings=@(Get-Content -LiteralPath ($beforePath+'.qa.json') -Raw | ConvertFrom-Json;
    Get-Content -LiteralPath ($afterPath+'.qa.json') -Raw | ConvertFrom-Json)
if ($bindings[0].identifier -ne $bindings[1].identifier -or $bindings[0].identifier -notmatch '^com\.qingjian\.qa\.[a-z0-9-]+$') { throw 'Both builds must use the same isolated identity/profile' }
$identifier=$bindings[0].identifier
$data=Join-Path $env:LOCALAPPDATA $identifier
if (-not (Test-Path -LiteralPath (Join-Path $data '.qingjian-qa-fixture'))) { throw 'Disposable data marker missing' }
$rows=@()
function Sample([string]$which,[string]$phase,[int]$index) {
    $exe=if($which -eq 'before'){$beforePath}else{$afterPath}
    $destination=Join-Path $output "$phase-$index-$which.json"
    & (Join-Path $PSScriptRoot 'measure-windows-resources.ps1') -ExecutablePath $exe -DataIdentifier $identifier -OutputPath $destination -NoteCounts @(1) -WarmupSeconds 5 | Out-Host
    $record=Get-Content -LiteralPath $destination -Raw | ConvertFrom-Json
    if (-not $record.samples[0].complete) { throw 'Incomplete readiness sample' }
    [pscustomobject]@{version=$which;phase=$phase;index=$index;firstUiReadyMs=$record.samples[0].firstUiReadyMs;runId=$record.samples[0].runId}
}

# Cold here means a fresh WebView cache, not an OS file-cache flush.
# Move only the explicitly named disposable cache; preserve every moved cache.
$cache=Join-Path $data 'EBWebView'
$saved=Join-Path $output 'cache-original'
if (Test-Path -LiteralPath $saved) { throw 'Comparison directory already contains a prior cache; choose a fresh output' }
if (Test-Path -LiteralPath $cache) {
    $resolvedCache=(Resolve-Path -LiteralPath $cache).Path
    if ($resolvedCache -ne [IO.Path]::GetFullPath($cache)) { throw 'Unexpected cache path' }
    Move-Item -LiteralPath $resolvedCache -Destination $saved
}
try {
    foreach($which in @('before','after')) {
        $rows+=Sample $which 'cold-cache' 0
        if(Test-Path -LiteralPath $cache){Move-Item -LiteralPath $cache -Destination (Join-Path $output ('cache-cold-'+$which))}
    }
} finally {
    if(Test-Path -LiteralPath $cache){Move-Item -LiteralPath $cache -Destination (Join-Path $output ('cache-interrupted-'+[guid]::NewGuid().ToString('N')))}
    if(Test-Path -LiteralPath $saved){Move-Item -LiteralPath $saved -Destination $cache}
}
# Prime both resource sets before comparing warm starts. Alternate AB/BA order.
$null=Sample 'before' 'prime' 0
$null=Sample 'after' 'prime' 0
for($pair=1;$pair -le $Pairs;$pair++){
    $order=if($pair % 2){@('before','after')}else{@('after','before')}
    foreach($which in $order){$rows+=Sample $which 'warm' $pair}
}
function Median([object[]]$values){$sorted=@($values | Sort-Object);$n=$sorted.Count;if($n%2){return $sorted[[int][Math]::Floor($n/2)]};return ($sorted[$n/2-1]+$sorted[$n/2])/2}
$old=Median @($rows|Where-Object {$_.phase -eq 'warm' -and $_.version -eq 'before'}|ForEach-Object {$_.firstUiReadyMs})
$new=Median @($rows|Where-Object {$_.phase -eq 'warm' -and $_.version -eq 'after'}|ForEach-Object {$_.firstUiReadyMs})
[ordered]@{identifier=$identifier;beforeMedianMs=$old;afterMedianMs=$new;changePercent=[Math]::Round(100*($new/$old-1),2);
    withinTenPercent=($new -le $old*1.1);metric='DOM readiness proxy, same isolated profile, cold cache separate; not native visible/input/wake latency';samples=$rows} |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'comparison.json') -Encoding utf8
