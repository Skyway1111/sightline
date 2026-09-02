# Peak working set of one process tree, in MB, sampled until the root exits.
# The pid arrives as SIGHTLINE_ROOT_PID so nothing has to be quoted.
# `xtask audit-bench` drives this for every binary it compares, so the two
# numbers are taken the same way.
$root = [int]$env:SIGHTLINE_ROOT_PID
$max = 0
while ($true) {
    $live = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, WorkingSetSize
    $tree = @{ $root = $true }
    # four passes reach any depth these tools spawn (audit -> cargo -> rustc)
    for ($i = 0; $i -lt 4; $i++) {
        foreach ($p in $live) {
            if ($tree.ContainsKey([int]$p.ParentProcessId)) { $tree[[int]$p.ProcessId] = $true }
        }
    }
    $sum = 0
    foreach ($p in $live) {
        if ($tree.ContainsKey([int]$p.ProcessId)) { $sum += [int64]$p.WorkingSetSize }
    }
    if ($sum -gt $max) { $max = $sum }
    if (-not ($live | Where-Object { [int]$_.ProcessId -eq $root })) { break }
    Start-Sleep -Milliseconds 100
}
[int]($max / 1MB)
