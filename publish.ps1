# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (C) 2026 Fufeng Project
# ---------------------------------------------------------------
# 建置 Release 並整理可散布的 publish\ 資料夾（供 Inno Setup 打包）
#   用法：powershell -ExecutionPolicy Bypass -File publish.ps1
#   需要：Rust（rustup，x86_64-pc-windows-msvc）與 Visual Studio Build Tools（連結器）
# 產出只有兩個程式檔：SimplePdfEditor.exe 與 SimplePdfPreview.dll，
# C 執行環境已靜態連結（.cargo\config.toml），不需要另外安裝任何執行環境。
# ---------------------------------------------------------------
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$out  = Join-Path $root 'publish'
$bin  = Join-Path $root 'target\release'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

Push-Location $root
try {
    cargo build --release --workspace
    if ($LASTEXITCODE) { throw '建置失敗' }
} finally { Pop-Location }

if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item $out -ItemType Directory | Out-Null

Copy-Item (Join-Path $bin 'SimplePdfEditor.exe') $out
Copy-Item (Join-Path $bin 'SimplePdfPreview.dll') $out
# 授權文件：MIT 與 Apache-2.0 條款、聲明、第三方元件清單與授權原文
foreach ($f in 'README.md', 'CHANGELOG.md', 'NOTICE.txt', 'THIRD_PARTY_NOTICES.md') {
    Copy-Item (Join-Path $root $f) $out
}
# 加上 .txt 副檔名，使用者雙擊即可用記事本開啟
Copy-Item (Join-Path $root 'LICENSE-MIT') (Join-Path $out 'LICENSE-MIT.txt')
Copy-Item (Join-Path $root 'LICENSE-APACHE') (Join-Path $out 'LICENSE-APACHE.txt')
Copy-Item (Join-Path $root 'third_party_licenses') $out -Recurse

# 確認沒有依賴 VC++ 執行環境（vcruntime140.dll／api-ms-win-crt-*）
foreach ($f in 'SimplePdfEditor.exe', 'SimplePdfPreview.dll') {
    $bytes = [System.IO.File]::ReadAllBytes((Join-Path $out $f))
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    if ($text -match '(?i)vcruntime140|api-ms-win-crt-|msvcp140') { throw "$f 依賴 VC++ 執行環境" }
}

# 煙霧測試：在 publish\ 內以 --selftest 輸出一份含註記的 PDF，再以外掛測試程式產生縮圖與預覽
$tmp = Join-Path $env:TEMP ('spe_publish_test_' + [guid]::NewGuid())
New-Item $tmp -ItemType Directory | Out-Null
$p = Start-Process (Join-Path $out 'SimplePdfEditor.exe') -ArgumentList '--selftest', (Join-Path $root 'test\sample.pdf'), $tmp -Wait -PassThru
$ok = ($p.ExitCode -eq 0) -and (Test-Path (Join-Path $tmp 'full.pdf')) -and (Test-Path (Join-Path $tmp 'pages_enc.pdf'))
if ($ok) {
    $log = & (Join-Path $bin 'shellext_test.exe') (Join-Path $out 'SimplePdfPreview.dll') (Join-Path $tmp 'full.pdf') (Join-Path $tmp 'shell')
    $ok = ($log -match 'thumbnail hr=0x00000000') -and ($log -match 'preview hr=0x00000000') -and
          (Test-Path (Join-Path $tmp 'shell_preview1.bmp'))
}
Remove-Item $tmp -Recurse -Force
if (-not $ok) { throw "publish 煙霧測試失敗（exit=$($p.ExitCode)）" }

$size = (Get-ChildItem $out -Recurse | Measure-Object Length -Sum).Sum / 1MB
'publish 完成：{0}（{1} 個檔案，{2:N1} MB），煙霧測試通過' -f $out, (Get-ChildItem $out -Recurse -File).Count, $size
