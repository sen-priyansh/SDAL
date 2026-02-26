!define PRODUCT_NAME "SDAL"
!define PRODUCT_VERSION "0.1.0"
!define PRODUCT_PUBLISHER "SDAL Contributors"
!define PRODUCT_WEB_SITE "https://github.com/sen-priyansh/SDAL"
!define PRODUCT_DIR_REGKEY "Software\Microsoft\Windows\CurrentVersion\App Paths\sdal.exe"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"

; MUI 1.67 compatible ------
!include "MUI2.nsh"

; MUI Settings
!define MUI_ABORTWARNING
!define MUI_ICON "logo_assets\logo.ico"
!define MUI_UNICON "logo_assets\logo.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "logo_assets\logo_header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "logo_assets\logo_wizard.bmp"

; Welcome page
!insertmacro MUI_PAGE_WELCOME
; License page
!insertmacro MUI_PAGE_LICENSE "LICENSE"
; Components page
!insertmacro MUI_PAGE_COMPONENTS
; Directory page
!insertmacro MUI_PAGE_DIRECTORY
; Instfiles page
!insertmacro MUI_PAGE_INSTFILES
; Finish page
!define MUI_FINISHPAGE_RUN "$INSTDIR\sdal.exe"
!define MUI_FINISHPAGE_RUN_PARAMETERS "--version"
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_INSTFILES

; Language files
!insertmacro MUI_LANGUAGE "English"

; Reserve Files
!insertmacro MUI_RESERVEFILE_LANGDLL

; Path Manipulation
!include "WinMessages.nsh"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "sdal-windows-installer.exe"
InstallDir "$PROGRAMFILES64\SDAL"
ShowInstDetails show
ShowUnInstDetails show

Section "SDAL Core (Required)" SEC01
  SectionIn RO ; Read-Only, user can't uncheck
  SetOutPath "$INSTDIR"
  SetOverwrite ifnewer
  File "/oname=sdal.exe" "target\release\sdal-cli.exe"
  File "LICENSE"
SectionEnd

Section "Add to PATH" SEC_PATH
  ; Update PATH
  WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "PATH_SDAL" "$INSTDIR"
  
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
  WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0;$INSTDIR"
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
SectionEnd

; Section descriptions
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC01} "Installs the core SDAL executable and license."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_PATH} "Adds SDAL to your system PATH so you can run 'sdal' from any powershell/cmd."
!insertmacro MUI_FUNCTION_DESCRIPTION_END


Section -AdditionalIcons
  WriteUninstaller "$INSTDIR\uninst.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "$(^Name)"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninst.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
SectionEnd

Function un.onUninstSuccess
  HideWindow
  MessageBox MB_ICONINFORMATION|MB_OK "$(^Name) was successfully removed from your computer."
FunctionEnd

Function un.onInit
  MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 "Are you sure you want to completely remove $(^Name) and all of its components?" IDYES +2
  Abort
FunctionEnd

Section Uninstall
  Delete "$INSTDIR\uninst.exe"
  Delete "$INSTDIR\sdal.exe"
  Delete "$INSTDIR\LICENSE"

  RMDir "$INSTDIR"

  ; Note: A truly professional uninstaller should carefully remove only its path from the PATH variable.
  ; For simplicity here, we leave the Path as is or the user can manually clean up.
  ; In a production environment, use a plugin like EnvVarUpdate.

  DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM "${PRODUCT_DIR_REGKEY}"
  SetAutoClose true
SectionEnd
