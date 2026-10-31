# Downloads an LGPL Windows build of FFmpeg 8.1 or newer from BtbN/FFmpeg-Builds, checks it has the
# gfxcapture source (window/monitor capture), and stages it in src-tauri/resources for bundling.
# Fails the build if no suitable FFmpeg is found.

$ErrorActionPreference = 'Stop'
$headers = @{ 'User-Agent' = 'encore-ci' }
if ($env:GH_TOKEN) { $headers['Authorization'] = "Bearer $env:GH_TOKEN" }

$release = Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/latest'

$versioned = $release.assets |
  Where-Object { $_.name -match '^ffmpeg-n(\d+)\.(\d+)-latest-win64-lgpl-\d+\.\d+\.zip$' } |
  ForEach-Object { [pscustomobject]@{ Asset = $_; Major = [int]$Matches[1]; Minor = [int]$Matches[2] } } |
  Where-Object { $_.Major -gt 8 -or ($_.Major -eq 8 -and $_.Minor -ge 1) } |
  Sort-Object -Property Major, Minor -Descending

$candidates = @($versioned | ForEach-Object { $_.Asset })
$master = $release.assets | Where-Object { $_.name -eq 'ffmpeg-master-latest-win64-lgpl.zip' }
if ($master) { $candidates += $master }

Write-Host "Candidates: $(($candidates | ForEach-Object { $_.name }) -join ', ')"

$dest = Join-Path $PSScriptRoot '..\src-tauri\resources'
New-Item -ItemType Directory -Force $dest | Out-Null

foreach ($asset in $candidates) {
  $zip = Join-Path $env:RUNNER_TEMP $asset.name
  $dir = Join-Path $env:RUNNER_TEMP ([IO.Path]::GetFileNameWithoutExtension($asset.name))
  Write-Host "Trying $($asset.name)"
  Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $zip
  Expand-Archive -Path $zip -DestinationPath $dir -Force
  $exe = Get-ChildItem -Path $dir -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1
  if (-not $exe) { Write-Host '  no ffmpeg.exe inside'; continue }

  $help = (& $exe.FullName -hide_banner -h filter=gfxcapture 2>&1 | Out-String)
  if ($help -match 'Unknown filter' -or $help -notmatch 'gfxcapture') {
    Write-Host '  missing gfxcapture, skipping'
    continue
  }

  Copy-Item $exe.FullName (Join-Path $dest 'ffmpeg.exe') -Force
  $licence = Get-ChildItem -Path $dir -Recurse -Filter 'LICENSE.txt' | Select-Object -First 1
  if ($licence) {
    Copy-Item $licence.FullName (Join-Path $dest 'FFMPEG-LICENSE.txt') -Force
  } else {
    Set-Content -Path (Join-Path $dest 'FFMPEG-LICENSE.txt') -Value 'FFmpeg is licensed under the GNU LGPL v2.1 or later. Source: https://ffmpeg.org/download.html'
  }

  Write-Host "Using $($asset.name)"
  & $exe.FullName -hide_banner -version | Select-Object -First 1
  Write-Host 'GPU encoders compiled in:'
  & $exe.FullName -hide_banner -encoders 2>&1 | Select-String -Pattern '_(nvenc|amf|qsv)\b'
  Write-Host 'gfxcapture options:'
  Write-Host $help
  return
}

throw 'No LGPL FFmpeg 8.1+ build with gfxcapture was found. Encore cannot capture without it.'
