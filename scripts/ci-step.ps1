# Runs one build step and reports its output as a GitHub annotation: always for -Always steps
# (diagnostics), otherwise only when the step fails. Annotations are readable through the API,
# which makes build failures diagnosable without opening the web log.
param(
  [Parameter(Mandatory = $true)][string]$Title,
  [Parameter(Mandatory = $true)][string]$Run,
  [switch]$Always,
  [int]$Lines = 60
)

$ErrorActionPreference = 'Continue'
$log = Join-Path $env:RUNNER_TEMP ("step-" + [guid]::NewGuid().ToString() + ".log")
$code = 0
try {
  $global:LASTEXITCODE = 0
  Invoke-Expression "$Run *>&1" | ForEach-Object { "$_" } | Tee-Object -FilePath $log
  $code = $LASTEXITCODE
} catch {
  "$_" | Tee-Object -FilePath $log -Append
  $code = 1
}

function Encode([string]$s) { ($s -replace '%', '%25') -replace "`r", '' -replace "`n", '%0A' }

$tail = @()
if (Test-Path $log) {
  $tail = Get-Content $log | Where-Object { $_ -notmatch '^\s*(Compiling|Downloaded|Downloading|Checking|Fresh) ' } | Select-Object -Last $Lines
}
$body = Encode ($tail -join "`n")

if ($code -ne 0) {
  Write-Host "::error title=$Title failed::$body"
  exit $code
}
if ($Always) {
  Write-Host "::notice title=$Title::$body"
}
exit 0
