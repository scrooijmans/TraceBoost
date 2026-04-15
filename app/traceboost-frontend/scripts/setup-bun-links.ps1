$ErrorActionPreference = "Stop"

$frontendRoot = Split-Path -Parent $PSScriptRoot
$traceBoostRoot = Split-Path -Parent (Split-Path -Parent $frontendRoot)
$ophioliteRoot = Resolve-Path (Join-Path $traceBoostRoot "..\\ophiolite")

$linkTargets = @(
  (Join-Path $traceBoostRoot "contracts\\ts\\seis-contracts"),
  (Join-Path $ophioliteRoot "charts\\packages\\svelte")
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
