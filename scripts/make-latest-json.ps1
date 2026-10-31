# Writes latest.json, the file installed copies of Encore check for updates:
# https://github.com/<owner>/<repo>/releases/latest/download/latest.json
param(
  [Parameter(Mandatory = $true)][string]$Tag,
  [Parameter(Mandatory = $true)][string]$Repo
)
$ErrorActionPreference = 'Stop'
$dir = 'target/release/bundle/nsis'
$setup = Get-ChildItem $dir -Filter '*-setup.exe' | Select-Object -First 1
if (-not $setup) { throw "No installer found in $dir" }
$sigPath = "$($setup.FullName).sig"
if (-not (Test-Path $sigPath)) { throw "No update signature ($sigPath). Is TAURI_SIGNING_PRIVATE_KEY set?" }

$version = $Tag.TrimStart('v')
$manifest = [ordered]@{
  version   = $version
  notes     = "Encore $version. See the release page for what's new."
  pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = (Get-Content $sigPath -Raw).Trim()
      url       = "https://github.com/$Repo/releases/download/$Tag/$($setup.Name)"
    }
  }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -Path "$dir/latest.json" -Encoding utf8NoBOM
Write-Host "latest.json for $version -> $($setup.Name)"
Get-Content "$dir/latest.json"
