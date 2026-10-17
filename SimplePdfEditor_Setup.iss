; SPDX-License-Identifier: AGPL-3.0-or-later
; Copyright (C) 2026 Fufeng Project
; ---------------------------------------------------------------
; 簡易 PDF 編輯器 - Inno Setup 安裝程式腳本
;   先執行 publish.ps1 產生 publish\，再以 ISCC 編譯本檔：
;   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" SimplePdfEditor_Setup.iss
; ---------------------------------------------------------------

#define AppName "SimplePdfEditor"
#define AppDisplay "簡易 PDF 編輯器"
#define AppVersion "2.2.0"
#define AppPublisher "Fufeng Project"
#define AppExe "SimplePdfEditor.exe"
#define ShellDll "SimplePdfPreview.dll"
#define PreviewGuid "9E089D14-B74E-4DDD-A2B7-D1821458115C"
#define ThumbGuid "7D86EEC3-4516-48CB-90C9-35EC10436737"

[Setup]
AppId={{A23E82A7-464F-4769-A8F2-F4B023D12079}}
AppName={#AppDisplay}
AppVersion={#AppVersion}
AppVerName={#AppDisplay} {#AppVersion}
AppPublisher={#AppPublisher}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppDisplay}
DisableProgramGroupPage=yes
OutputBaseFilename=SimplePdfEditor_Setup_v{#AppVersion}
OutputDir=dist
Compression=lzma2/ultra64
SolidCompression=yes
PrivilegesRequired=admin
SetupIconFile=icons\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppDisplay}
WizardStyle=modern
; 授權頁：本程式為 AGPL-3.0-or-later 自由軟體（NOTICE.txt 為中英文說明，完整條款安裝於 {app}\LICENSE.txt）
LicenseFile=NOTICE.txt
; 檔案關聯變更後通知檔案總管更新
ChangesAssociations=yes

[Languages]
Name: "ChineseTraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"

[Tasks]
Name: "desktopicon"; Description: "建立桌面捷徑"; GroupDescription: "其他選項："
Name: "openwith"; Description: "加入 PDF 檔的「開啟檔案」選單（不會變更預設的 PDF 開啟程式）"; GroupDescription: "其他選項："
Name: "previewpane"; Description: "在檔案總管預覽窗格顯示 PDF 內容（取代目前的 PDF 預覽程式，解除安裝時還原）"; GroupDescription: "檔案總管整合："
Name: "thumbnails"; Description: "以 PDF 第 1 頁作為檔案總管縮圖（取代目前的 PDF 縮圖程式，解除安裝時還原）"; GroupDescription: "檔案總管整合："

[Files]
; publish.ps1 整理好的執行檔與文件。Rust 版的 C 執行環境已靜態連結，
; 只有 SimplePdfEditor.exe 與 SimplePdfPreview.dll 兩個程式檔，安裝時不需下載或安裝任何額外套件
; （不需要 Microsoft Visual C++ 可轉發套件，也沒有 Qt 等 DLL）。
Source: "publish\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion; Excludes: "{#ShellDll}"
; 檔案總管外掛可能正被 explorer.exe / prevhost.exe 載入，無法覆蓋時改為重新開機後替換
Source: "publish\{#ShellDll}"; DestDir: "{app}"; Flags: ignoreversion restartreplace uninsrestartdelete

[InstallDelete]
; 從 1.x（C++／Qt 版）升級：移除不再需要的 Qt、MuPDF 相依 DLL 與 Qt 外掛資料夾
; （外掛 SimplePdfPreview.dll 由上方 [Files] 以 restartreplace 覆蓋，不在此刪除）
Type: files; Name: "{app}\Qt6*.dll"
Type: files; Name: "{app}\icu*.dll"
Type: files; Name: "{app}\brotli*.dll"
Type: files; Name: "{app}\bz2.dll"
Type: files; Name: "{app}\double-conversion.dll"
Type: files; Name: "{app}\freetype.dll"
Type: files; Name: "{app}\harfbuzz.dll"
Type: files; Name: "{app}\jpeg62.dll"
Type: files; Name: "{app}\libpng16.dll"
Type: files; Name: "{app}\md4c.dll"
Type: files; Name: "{app}\openjp2.dll"
Type: files; Name: "{app}\pcre2-16.dll"
Type: files; Name: "{app}\zlib1.dll"
Type: files; Name: "{app}\zstd.dll"
Type: files; Name: "{app}\mupdf*.dll"
Type: filesandordirs; Name: "{app}\platforms"
Type: filesandordirs; Name: "{app}\styles"

[Icons]
Name: "{group}\{#AppDisplay}"; Filename: "{app}\{#AppExe}"
Name: "{group}\使用說明 (README)"; Filename: "{app}\README.md"
Name: "{group}\授權條款 (LICENSE)"; Filename: "{app}\LICENSE.txt"; Comment: "GNU AGPL-3.0"
Name: "{group}\解除安裝 {#AppDisplay}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppDisplay}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; 「開啟檔案」選單：註冊 ProgID 並加入 .pdf 的 OpenWithProgids（只加入候選清單，不搶預設程式）
Root: HKA; Subkey: "Software\Classes\{#AppName}.pdf"; ValueType: string; ValueData: "PDF 文件"; \
  Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\{#AppName}.pdf\DefaultIcon"; ValueType: string; ValueData: "{app}\{#AppExe},0"; \
  Tasks: openwith
Root: HKA; Subkey: "Software\Classes\{#AppName}.pdf\shell\open\command"; ValueType: string; \
  ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.pdf"; \
  ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; \
  ValueData: "{#AppDisplay}"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".pdf"; \
  ValueData: ""; Tasks: openwith

; 檔案總管 PDF 預覽處理常式（在 prevhost.exe 64 位元代理行程中執行）
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#PreviewGuid}}"; ValueType: string; ValueData: "SimplePdfEditor PDF Preview Handler"; \
  Flags: uninsdeletekey; Tasks: previewpane
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#PreviewGuid}}"; ValueType: string; ValueName: "DisplayName"; \
  ValueData: "{#AppDisplay} PDF 預覽"; Tasks: previewpane
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#PreviewGuid}}"; ValueType: string; ValueName: "AppID"; \
  ValueData: "{{6d2b5079-2f0b-48dd-ab7f-97cec514d30b}"; Tasks: previewpane
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#PreviewGuid}}\InprocServer32"; ValueType: string; \
  ValueData: "{app}\{#ShellDll}"; Tasks: previewpane
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#PreviewGuid}}\InprocServer32"; ValueType: string; \
  ValueName: "ThreadingModel"; ValueData: "Apartment"; Tasks: previewpane
Root: HKLM64; Subkey: "Software\Microsoft\Windows\CurrentVersion\PreviewHandlers"; ValueType: string; \
  ValueName: "{{{#PreviewGuid}}"; ValueData: "{#AppDisplay} PDF 預覽"; Flags: uninsdeletevalue; Tasks: previewpane

; 檔案總管 PDF 縮圖提供者
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#ThumbGuid}}"; ValueType: string; ValueData: "SimplePdfEditor PDF Thumbnail Provider"; \
  Flags: uninsdeletekey; Tasks: thumbnails
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#ThumbGuid}}\InprocServer32"; ValueType: string; \
  ValueData: "{app}\{#ShellDll}"; Tasks: thumbnails
Root: HKLM64; Subkey: "Software\Classes\CLSID\{{{#ThumbGuid}}\InprocServer32"; ValueType: string; \
  ValueName: "ThreadingModel"; ValueData: "Apartment"; Tasks: thumbnails

[Run]
Filename: "{app}\{#AppExe}"; Description: "立即啟動 {#AppDisplay}"; Flags: nowait postinstall skipifsilent

[Code]
// ---- 檔案總管整合：.pdf 的預覽／縮圖處理常式 ----
// 安裝時把原本的處理常式（例如 PDF-XChange、Adobe）記在 BackupKey，再換成本程式的；
// 解除安裝或取消勾選時，若目前仍是本程式的，就還原成原本的。

const
  PreviewKey = 'Software\Classes\.pdf\shellex\{8895b1c6-b41f-4c1c-a562-0d564250836f}';
  ThumbKey   = 'Software\Classes\.pdf\shellex\{e357fccd-a995-4576-b01f-234630154e96}';
  BackupKey  = 'Software\SimplePdfEditor\ShellBackup';
  NoneMark   = '<none>';

procedure TakeOver(const Key, Clsid, BackupName: String);
var
  Current: String;
begin
  if not RegQueryStringValue(HKLM64, Key, '', Current) then
    Current := '';
  if CompareText(Current, Clsid) <> 0 then
  begin
    if Current = '' then
      Current := NoneMark;
    RegWriteStringValue(HKLM64, BackupKey, BackupName, Current);
  end;
  RegWriteStringValue(HKLM64, Key, '', Clsid);
end;

procedure RestoreHandler(const Key, Clsid, BackupName: String);
var
  Current, Saved: String;
begin
  if not RegQueryStringValue(HKLM64, Key, '', Current) then
    Exit;
  if CompareText(Current, Clsid) <> 0 then
    Exit;  // 已被其他程式改掉，不要動
  if RegQueryStringValue(HKLM64, BackupKey, BackupName, Saved) and (Saved <> NoneMark) and (Saved <> '') then
    RegWriteStringValue(HKLM64, Key, '', Saved)
  else
    RegDeleteKeyIncludingSubkeys(HKLM64, Key);
  RegDeleteValue(HKLM64, BackupKey, BackupName);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('previewpane') then
      TakeOver(PreviewKey, '{{#PreviewGuid}}', 'Preview')
    else
      RestoreHandler(PreviewKey, '{{#PreviewGuid}}', 'Preview');
    if WizardIsTaskSelected('thumbnails') then
      TakeOver(ThumbKey, '{{#ThumbGuid}}', 'Thumbnail')
    else
      RestoreHandler(ThumbKey, '{{#ThumbGuid}}', 'Thumbnail');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RestoreHandler(PreviewKey, '{{#PreviewGuid}}', 'Preview');
    RestoreHandler(ThumbKey, '{{#ThumbGuid}}', 'Thumbnail');
    RegDeleteKeyIncludingSubkeys(HKLM64, 'Software\SimplePdfEditor');
  end;
end;
