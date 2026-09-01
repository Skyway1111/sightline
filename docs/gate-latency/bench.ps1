# Frozen ruler for the single-file gate hillclimb (2026-08-31).
# Metric: median wall of `sightline gate ROOT --files FILE [--config C]`,
# N=15 warm runs per case, fresh process each. Also captures stdout+exit as
# the equivalence golden. Usage: pwsh bench.ps1 [-Label name]
param([string]$Label = "run")
# Roots are siblings of this workspace, as `crates/xtask/corpus.toml` says.
$repo = (git -C $PSScriptRoot rev-parse --show-toplevel)
$cc = Split-Path -Parent $repo
$sl = "$repo/target/release/sightline.exe"
$out = "$repo/target/hillclimb"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$cases = @(
  @{n="pt-py";   root="$cc/powertools-lambda-python"; file="aws_lambda_powertools/utilities/parameters/base.py"; cfg=$null},
  @{n="mc-py";   root="$cc/merged-calculator";  file="src/calculator/damage.py"; cfg="$repo/corpus/merged-calculator.toml"},
  @{n="tur-rs";  root="$cc/turmoil";            file="crates/turmoil-fs/src/lib.rs"; cfg="$repo/corpus/turmoil.toml"},
  @{n="sal-rs";  root="$cc/salvo";              file="crates/oapi/src/openapi/components.rs"; cfg="$repo/corpus/salvo.toml"}
)
$results = @()
foreach ($c in $cases) {
  $argv = @("gate", $c.root, "--files", "$($c.root)/$($c.file)")
  if ($c.cfg) { $argv += @("--config", $c.cfg) }
  $txt = & $sl @argv 2>&1 | Out-String
  $code = $LASTEXITCODE
  Set-Content -Path "$out/golden-$($c.n)-$Label.txt" -Value "exit=$code`n$txt" -NoNewline
  $walls = @()
  for ($i=0; $i -lt 15; $i++) {
    $t = Measure-Command { & $sl @argv 2>&1 | Out-Null }
    $walls += $t.TotalMilliseconds
  }
  $sorted = $walls | Sort-Object
  $results += "$($c.n): median $([math]::Round($sorted[7],1)) ms  (min $([math]::Round($sorted[0],1)), max $([math]::Round($sorted[14],1)))"
}
$walls = @()
for ($i=0; $i -lt 15; $i++) { $t = Measure-Command { & $sl --version | Out-Null }; $walls += $t.TotalMilliseconds }
$sorted = $walls | Sort-Object
$results += "spawn-floor(--version): median $([math]::Round($sorted[7],1)) ms"
$results | Tee-Object -FilePath "$out/walls-$Label.txt"
