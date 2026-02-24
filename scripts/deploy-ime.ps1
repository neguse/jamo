# deploy-ime.ps1 - jamo IME DLL デプロイスクリプト
#
# Windows はロード中の DLL をリネーム可能 (削除は不可)。
# これを利用して explorer を殺さずにビルド済み DLL を差し替える。
#
# 使い方:
#   cargo build --release -p jamo-ime
#   .\scripts\deploy-ime.ps1
#
# 管理者権限が必要 (Program Files への書き込み)

#Requires -RunAsAdministrator

param(
    [string]$DllDir = "C:\Program Files\Jamo",
    [string]$BuildDir = "target\release"
)

$ErrorActionPreference = "Stop"

$dll = Join-Path $DllDir "jamo_ime.dll"
$old = Join-Path $DllDir "jamo_ime.dll.old"
$src = Join-Path $BuildDir "jamo_ime.dll"

# ビルド成果物の存在確認
if (-not (Test-Path $src)) {
    Write-Error "Build artifact not found: $src"
    Write-Error "Run: cargo build --release -p jamo-ime"
    exit 1
}

# インストール先ディレクトリの作成
if (-not (Test-Path $DllDir)) {
    New-Item -ItemType Directory -Path $DllDir -Force | Out-Null
    Write-Host "Created directory: $DllDir"
}

# 前回の .old を削除試行
if (Test-Path $old) {
    Remove-Item $old -ErrorAction SilentlyContinue
}

# ロード中の DLL をリネーム (ロック中でも可能)
if (Test-Path $dll) {
    Rename-Item $dll $old
    Write-Host "Renamed existing DLL to .old"
}

# 新しい DLL をコピー
Copy-Item $src $dll
Write-Host "Deployed: $src -> $dll"

# explorer 再起動で新 DLL を読み込み
Write-Host "Restarting explorer.exe..."
Stop-Process -Name explorer -Force
# explorer は Windows Shell なので自動的に再起動する

Write-Host "Done. New DLL will be loaded by applications."
