param(
  [string]$DownloadUrl = "",
  [string]$Sha256 = "auto",
  [string]$Version = "",
  [string]$OutDir = (Join-Path $PSScriptRoot "..\dist-installer"),
  [string]$InstallerName = "LightNovelReaderSetup.exe"
)

$ErrorActionPreference = "Stop"

function Find-Csc {
  $cmd = Get-Command csc.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }

  $roots = @(
    (Join-Path $env:WINDIR "Microsoft.NET\Framework64"),
    (Join-Path $env:WINDIR "Microsoft.NET\Framework")
  )
  foreach ($root in $roots) {
    if (-not (Test-Path -LiteralPath $root)) { continue }
    $candidate = Get-ChildItem -LiteralPath $root -Recurse -Filter csc.exe -ErrorAction SilentlyContinue |
      Sort-Object FullName -Descending |
      Select-Object -First 1
    if ($candidate) { return $candidate.FullName }
  }

  throw "csc.exe not found. Install .NET SDK or .NET Framework developer tools."
}

function ConvertTo-CSharpLiteral([string]$Value) {
  if ($null -eq $Value) { $Value = "" }
  return '"' + ($Value.Replace('\', '\\').Replace('"', '\"')) + '"'
}

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$packageJson = Get-Content -Raw -Encoding UTF8 (Join-Path $repo "package.json") | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = $packageJson.version
}

if ([string]::IsNullOrWhiteSpace($DownloadUrl)) {
  $latestZip = Get-ChildItem -LiteralPath (Join-Path $repo "dist-beta") -Filter *.zip -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
  if (-not $latestZip) {
    throw "No -DownloadUrl was provided and no dist-beta\\*.zip package exists."
  }
  $DownloadUrl = $latestZip.FullName
}

$localPackage = $null
if (Test-Path -LiteralPath $DownloadUrl) {
  $localPackage = Resolve-Path -LiteralPath $DownloadUrl
  $DownloadUrl = $localPackage.Path
} elseif ($DownloadUrl.StartsWith("file:", [StringComparison]::OrdinalIgnoreCase)) {
  $uri = [Uri]$DownloadUrl
  if (Test-Path -LiteralPath $uri.LocalPath) {
    $localPackage = Resolve-Path -LiteralPath $uri.LocalPath
  }
}

if ($Sha256 -eq "auto") {
  if ($localPackage) {
    $Sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $localPackage.Path).Hash.ToLowerInvariant()
  } else {
    $Sha256 = ""
  }
}

$source = Join-Path $repo "tools\installer\LightNovelReaderSetup.cs"
if (-not (Test-Path -LiteralPath $source)) {
  throw "Installer source not found: $source"
}

$out = if ([System.IO.Path]::IsPathRooted($OutDir)) {
  $OutDir
} else {
  Join-Path $repo $OutDir
}
$obj = Join-Path $out "obj"
New-Item -ItemType Directory -Force $out, $obj | Out-Null

$configSource = Join-Path $obj "InstallerConfig.g.cs"
$config = @(
  "namespace LightNovelReaderInstaller",
  "{",
  "    internal static partial class InstallerConfig",
  "    {",
  "        public const string DefaultDownloadUrl = $(ConvertTo-CSharpLiteral $DownloadUrl);",
  "        public const string DefaultSha256 = $(ConvertTo-CSharpLiteral $Sha256);",
  "        public const string DefaultVersion = $(ConvertTo-CSharpLiteral $Version);",
  "    }",
  "}"
)
[System.IO.File]::WriteAllLines($configSource, $config, [System.Text.UTF8Encoding]::new($false))

$installerPath = Join-Path $out $InstallerName
$csc = Find-Csc
$args = @(
  "/nologo",
  "/target:exe",
  "/optimize+",
  "/platform:anycpu",
  "/out:$installerPath",
  "/r:System.IO.Compression.dll",
  "/r:System.IO.Compression.FileSystem.dll",
  $source,
  $configSource
)

& $csc @args
if ($LASTEXITCODE -ne 0) {
  throw "csc.exe failed with exit code $LASTEXITCODE"
}

$manifest = @(
  "name=lightnovel-reader-web-installer",
  "version=$Version",
  "download_url=$DownloadUrl",
  "sha256=$Sha256",
  "built_at=$([DateTimeOffset]::Now.ToString('o'))",
  "csc=$csc",
  "installer=$installerPath"
)
[System.IO.File]::WriteAllLines(
  (Join-Path $out "LightNovelReaderSetup.manifest.txt"),
  $manifest,
  [System.Text.UTF8Encoding]::new($false)
)

Write-Host "Web installer created:"
Write-Host "- $installerPath"
Write-Host "- $(Join-Path $out 'LightNovelReaderSetup.manifest.txt')"
if ([string]::IsNullOrWhiteSpace($Sha256)) {
  Write-Host "Warning: no SHA-256 is embedded. Pass -Sha256 when building for public downloads."
}
