param(
  [ValidateSet("debug", "release")]
  [string]$Configuration = "release",
  [switch]$SkipBuild,
  [switch]$NoSamples,
  [string]$OutDir = (Join-Path $PSScriptRoot "..\dist-beta")
)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$packageJson = Get-Content -Raw -Encoding UTF8 (Join-Path $repo "package.json") | ConvertFrom-Json
$version = $packageJson.version
$runtime = "windows-x64"
$packageName = "lightnovel-reader-v$version-$Configuration-$runtime"
$stage = Join-Path $OutDir $packageName
$zipPath = "$stage.zip"
$targetDir = if ($Configuration -eq "release") { "release" } else { "debug" }
$exePath = Join-Path $repo "target\$targetDir\reader.exe"

if (-not $SkipBuild) {
  Push-Location $repo
  try {
    if ($Configuration -eq "release") {
      & npm.cmd run tauri -- build --no-bundle
    } else {
      & npm.cmd run tauri -- build --debug --no-bundle
    }
    if ($LASTEXITCODE -ne 0) {
      throw "Tauri build failed with exit code $LASTEXITCODE"
    }
  } finally {
    Pop-Location
  }
}

if (-not (Test-Path -LiteralPath $exePath)) {
  throw "reader.exe not found: $exePath"
}

Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $zipPath -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item -LiteralPath $exePath -Destination (Join-Path $stage "reader.exe") -Force

if (-not $NoSamples) {
  $sampleDir = Join-Path $stage "samples"
  & powershell -ExecutionPolicy Bypass -File (Join-Path $repo "scripts\new-smoke-epubs.ps1") -OutDir $sampleDir
  if ($LASTEXITCODE -ne 0) {
    throw "Smoke sample generation failed with exit code $LASTEXITCODE"
  }
}

$launcher = @(
  "@echo off",
  "setlocal",
  "cd /d ""%~dp0""",
  "title LightNovel Reader Launcher",
  "",
  ":menu",
  "cls",
  "echo.",
  "echo   LightNovel Reader",
  "echo   =================",
  "echo.",
  "echo   [1] Start reader",
  "echo   [2] Open smoke samples",
  "echo   [3] Open README",
  "echo   [Q] Quit",
  "echo.",
  "choice /c 123Q /n /m ""Select: """,
  "if errorlevel 4 exit /b 0",
  "if errorlevel 3 goto readme",
  "if errorlevel 2 goto samples",
  "if errorlevel 1 goto launch",
  "",
  ":launch",
  "start """" ""%~dp0reader.exe""",
  "goto menu",
  "",
  ":samples",
  "if exist ""%~dp0samples"" (",
  "  start """" ""%~dp0samples""",
  ") else (",
  "  echo.",
  "  echo Smoke samples are not included.",
  "  pause",
  ")",
  "goto menu",
  "",
  ":readme",
  "if exist ""%~dp0README.txt"" (",
  "  start """" notepad ""%~dp0README.txt""",
  ") else (",
  "  echo.",
  "  echo README.txt is missing.",
  "  pause",
  ")",
  "goto menu"
)
[System.IO.File]::WriteAllLines(
  (Join-Path $stage "LightNovel Reader Launcher.cmd"),
  $launcher,
  [System.Text.UTF8Encoding]::new($false)
)

$readme = @(
  "LightNovel Reader v$version ($Configuration)",
  "========================================",
  "",
  "This is a Windows portable beta package for lightnovel-reader.",
  "",
  "Start:",
  "- Double-click LightNovel Reader Launcher.cmd.",
  "- Or double-click reader.exe directly.",
  "",
  "Smoke samples:",
  "- samples\one\smoke-test-lightnovel-vol1.epub",
  "- samples\folder\Smoke Test Series\Vol01\smoke-test-lightnovel-vol1-copy.epub",
  "- samples\folder\Smoke Test Series\Vol02\smoke-test-lightnovel-vol2.epub",
  "",
  "Current boundary:",
  "- This package is for v$version smoke testing.",
  "- Manual P0 is still required before a real beta release.",
  "- The app does not bundle online novel text sources and does not bypass paid access, login, or DRM.",
  "",
  "Data:",
  "- Reading state, annotations, and the local library are written to the system app data directory.",
  "- Record symptoms and reproduction steps before switching test builds."
)
[System.IO.File]::WriteAllLines(
  (Join-Path $stage "README.txt"),
  $readme,
  [System.Text.UTF8Encoding]::new($false)
)

$versionText = @(
  "name=lightnovel-reader",
  "version=$version",
  "configuration=$Configuration",
  "runtime=$runtime",
  "built_at=$([DateTimeOffset]::Now.ToString('o'))"
)
[System.IO.File]::WriteAllLines(
  (Join-Path $stage "VERSION.txt"),
  $versionText,
  [System.Text.UTF8Encoding]::new($false)
)

Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -Force

Write-Host "Beta package created:"
Write-Host "- $stage"
Write-Host "- $zipPath"
