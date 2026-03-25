param(
    [string]$Remote = "seisrefine",
    [string]$Branch = "main",
    [string]$Prefix = "crates/seisrefine"
)

$ErrorActionPreference = "Stop"

Write-Host "Splitting subtree '$Prefix'..."
$splitCommit = git subtree split --prefix=$Prefix HEAD
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($splitCommit)) {
    throw "git subtree split failed"
}

Write-Host "Pushing $splitCommit to $Remote/$Branch..."
git push $Remote "$($splitCommit):refs/heads/$Branch"
if ($LASTEXITCODE -ne 0) {
    throw "git push failed"
}

Write-Host "Published $Prefix to $Remote/$Branch"

