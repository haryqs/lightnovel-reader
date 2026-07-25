param(
    [Parameter(Mandatory = $true)]
    [string]$PrivateKeyPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$resolvedPrivateKey = (Resolve-Path -LiteralPath $PrivateKeyPath).Path
$previousLocation = (Get-Location).Path
$passwordPointer = [IntPtr]::Zero
$plainPassword = $null

try {
    $securePassword = Read-Host 'Enter the Tauri updater private-key password (input is hidden)' -AsSecureString
    $passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
    $plainPassword = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)
    if ([string]::IsNullOrEmpty($plainPassword)) {
        throw 'The updater private-key password must not be empty.'
    }

    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $resolvedPrivateKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plainPassword
    Set-Location -LiteralPath $repoRoot

    & npm.cmd run release:build:updater
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri signed NSIS build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
    $plainPassword = $null
    if ($passwordPointer -ne [IntPtr]::Zero) {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPointer)
    }
    Set-Location -LiteralPath $previousLocation
}

Write-Output 'build-signed-updater: OK'
Write-Output (Join-Path $repoRoot 'target\release\bundle')
