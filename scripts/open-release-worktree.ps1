[CmdletBinding()]
param(
    [string]$Ref,
    [string]$Path,
    [string]$Branch,
    [switch]$OpenFrontendShell
)

$ErrorActionPreference = "Stop"

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $oldNativeErrorPreference = $PSNativeCommandUseErrorActionPreference

    try {
        $PSNativeCommandUseErrorActionPreference = $false
        $output = @(& git @Arguments 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw ($output -join [Environment]::NewLine)
        }

        return $output
    }
    finally {
        $PSNativeCommandUseErrorActionPreference = $oldNativeErrorPreference
    }
}

function Get-ReleaseRef {
    $refs = Invoke-Git -Arguments @(
        "for-each-ref",
        "--format=%(refname:short)",
        "--sort=-committerdate",
        "refs/remotes/origin/release"
    )

    $refs = @($refs | Where-Object { $_ -and $_.Trim() -ne "" })
    if ($refs.Count -eq 0) {
        throw "No remote release branches were found under origin/release/*."
    }

    return $refs[0].Trim()
}

function Get-WorktreeMap {
    $lines = Invoke-Git -Arguments @("worktree", "list", "--porcelain")
    $map = @{}
    $currentPath = $null

    foreach ($line in $lines) {
        if (-not $line) {
            $currentPath = $null
            continue
        }

        if ($line.StartsWith("worktree ")) {
            $currentPath = [System.IO.Path]::GetFullPath($line.Substring(9).Trim())
            $map[$currentPath] = @{}
            continue
        }

        if ($currentPath -and $line.StartsWith("branch ")) {
            $map[$currentPath]["branch"] = $line.Substring(7).Trim()
            continue
        }

        if ($currentPath -and $line.StartsWith("HEAD ")) {
            $map[$currentPath]["HEAD"] = $line.Substring(5).Trim()
        }
    }

    return $map
}

$repoRoot = (Invoke-Git -Arguments @("rev-parse", "--show-toplevel") | Select-Object -First 1).Trim()
$repoRoot = [System.IO.Path]::GetFullPath($repoRoot)
$repoName = Split-Path -Leaf $repoRoot
$repoParent = Split-Path -Parent $repoRoot

Push-Location $repoRoot

try {
    if (-not $Ref) {
        $Ref = Get-ReleaseRef
    }

    $refForName = $Ref
    if ($refForName.StartsWith("origin/")) {
        $refForName = $refForName.Substring(7)
    }

    if (-not $Path) {
        $safeSuffix = ($refForName -replace "[\\/]+", "-")
        $Path = Join-Path $repoParent "$repoName-$safeSuffix"
    }

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $frontendPath = Join-Path $resolvedPath "app\traceboost-frontend"
    $worktrees = Get-WorktreeMap

    if ($worktrees.ContainsKey($resolvedPath)) {
        Write-Host "Using existing worktree: $resolvedPath"
    }
    else {
        if (Test-Path -LiteralPath $resolvedPath) {
            throw "Target path already exists and is not a registered git worktree: $resolvedPath"
        }

        $gitArgs = @("worktree", "add")

        if ($Branch) {
            $gitArgs += @("-b", $Branch)
        }

        $gitArgs += @($resolvedPath, $Ref)
        Invoke-Git -Arguments $gitArgs | Out-Null
        Write-Host "Created worktree: $resolvedPath"
    }

    Write-Host "Frontend path: $frontendPath"
    Write-Host ""
    Write-Host "Next commands:"
    Write-Host "  Set-Location `"$frontendPath`""
    Write-Host "  bun install"
    Write-Host "  bun run tauri:build"
    Write-Host ""
    Write-Host "If you want dev mode from both checkouts, change one Tauri/Vite dev port first."

    if ($OpenFrontendShell) {
        $escapedFrontendPath = $frontendPath.Replace("'", "''")
        Start-Process powershell.exe -ArgumentList @(
            "-NoExit",
            "-Command",
            "Set-Location -LiteralPath '$escapedFrontendPath'"
        ) | Out-Null
    }
}
finally {
    Pop-Location
}
