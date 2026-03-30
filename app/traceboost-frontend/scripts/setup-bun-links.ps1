$ErrorActionPreference = "Stop"

$frontendRoot = Split-Path -Parent $PSScriptRoot
$traceBoostRoot = Split-Path -Parent (Split-Path -Parent $frontendRoot)
$geovizRoot = Resolve-Path (Join-Path $traceBoostRoot "..\\geoviz")

$linkTargets = @(
  (Join-Path $traceBoostRoot "contracts\\ts\\seis-contracts"),
  (Join-Path $geovizRoot "packages\\svelte")
)

foreach ($target in $linkTargets) {
  if (-not (Test-Path $target)) {
    throw "Missing Bun link target: $target"
  }

  Push-Location $target
  try {
    bun link | Out-Host
  }
  finally {
    Pop-Location
  }
}
