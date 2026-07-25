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
    $securePassword = Read-Host '请输入 Tauri updater 私钥密码（输入不会显示）' -AsSecureString
    $passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
    $plainPassword = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)
    if ([string]::IsNullOrEmpty($plainPassword)) {
        throw 'updater 私钥密码不能为空'
    }

    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $resolvedPrivateKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plainPassword
    Set-Location -LiteralPath $repoRoot

    & npm.cmd run release:build
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri release build 失败，退出码：$LASTEXITCODE"
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
