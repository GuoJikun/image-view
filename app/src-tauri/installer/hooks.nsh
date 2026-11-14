; Extra Windows integration for the NSIS installer.
;
; Tauri's bundle.fileAssociations writes a ProgID per extension plus the
; extension's default value (backing up the previous handler). It does not
; declare the application as a *candidate* handler, so Windows never lists it
; under "Open with" or "Default apps" once another program owns the extension.
; These hooks add the three missing declarations:
;
;   1. Software\Classes\.<ext>\OpenWithProgids        -> "Open with" candidates
;   2. Software\Classes\Applications\<exe>\SupportedTypes -> per-exe candidates
;   3. Software\RegisteredApplications + <app>\Capabilities -> "Default apps"
;
; Only declarations are written. The per-extension default handler is owned by
; the fileAssociations section and is never touched here.
;
; The file is pure ASCII on purpose: it is !include'd without a BOM, so non-ASCII
; literals would be decoded with the build machine's ANSI code page.

!define IV_PROGID_PREFIX "ImageView"

; ProgID per extension; must match "name" in bundle.fileAssociations.
!macro IV_DeclareExt EXT
  WriteRegStr SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${IV_PROGID_PREFIX}.${EXT}" ""
  WriteRegStr SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe\SupportedTypes" ".${EXT}" ""
  WriteRegStr SHCTX "Software\image-view\Capabilities\FileAssociations" ".${EXT}" "${IV_PROGID_PREFIX}.${EXT}"
!macroend

!macro IV_UndeclareExt EXT
  DeleteRegValue SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${IV_PROGID_PREFIX}.${EXT}"
  DeleteRegKey /ifempty SHCTX "Software\Classes\.${EXT}\OpenWithProgids"
  DeleteRegValue SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe\SupportedTypes" ".${EXT}"
  DeleteRegValue SHCTX "Software\image-view\Capabilities\FileAssociations" ".${EXT}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Display name and command for the "Open with" entry that is keyed by exe.
  WriteRegStr SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe" "" "${PRODUCTNAME}"
  WriteRegStr SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe" "FriendlyAppName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""

  ; Capabilities block, consumed by Settings > Apps > Default apps.
  WriteRegStr SHCTX "Software\image-view\Capabilities" "ApplicationName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "Software\image-view\Capabilities" "ApplicationDescription" "Fast, clean cross-platform image viewer"
  WriteRegStr SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}" "Software\image-view\Capabilities"

  !insertmacro IV_DeclareExt "png"
  !insertmacro IV_DeclareExt "jpg"
  !insertmacro IV_DeclareExt "jpeg"
  !insertmacro IV_DeclareExt "gif"
  !insertmacro IV_DeclareExt "webp"
  !insertmacro IV_DeclareExt "bmp"
  !insertmacro IV_DeclareExt "tif"
  !insertmacro IV_DeclareExt "tiff"
  !insertmacro IV_DeclareExt "ico"
  !insertmacro IV_DeclareExt "qoi"
  !insertmacro IV_DeclareExt "tga"
  !insertmacro IV_DeclareExt "pnm"
  !insertmacro IV_DeclareExt "hdr"
  !insertmacro IV_DeclareExt "exr"
  !insertmacro IV_DeclareExt "dds"
  !insertmacro IV_DeclareExt "avif"
  !insertmacro IV_DeclareExt "heic"
  !insertmacro IV_DeclareExt "heif"
  !insertmacro IV_DeclareExt "svg"

  ; SHCNE_ASSOCCHANGED / SHCNF_FLUSH: let Explorer pick the change up at once.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, i 0, i 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Runs before the uninstaller reverts the extension defaults, so only the
  ; declarations added above are removed here.
  !insertmacro IV_UndeclareExt "png"
  !insertmacro IV_UndeclareExt "jpg"
  !insertmacro IV_UndeclareExt "jpeg"
  !insertmacro IV_UndeclareExt "gif"
  !insertmacro IV_UndeclareExt "webp"
  !insertmacro IV_UndeclareExt "bmp"
  !insertmacro IV_UndeclareExt "tif"
  !insertmacro IV_UndeclareExt "tiff"
  !insertmacro IV_UndeclareExt "ico"
  !insertmacro IV_UndeclareExt "qoi"
  !insertmacro IV_UndeclareExt "tga"
  !insertmacro IV_UndeclareExt "pnm"
  !insertmacro IV_UndeclareExt "hdr"
  !insertmacro IV_UndeclareExt "exr"
  !insertmacro IV_UndeclareExt "dds"
  !insertmacro IV_UndeclareExt "avif"
  !insertmacro IV_UndeclareExt "heic"
  !insertmacro IV_UndeclareExt "heif"
  !insertmacro IV_UndeclareExt "svg"

  DeleteRegKey SHCTX "Software\image-view\Capabilities"
  DeleteRegKey /ifempty SHCTX "Software\image-view"
  DeleteRegValue SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}"
  DeleteRegKey SHCTX "Software\Classes\Applications\${MAINBINARYNAME}.exe"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, i 0, i 0)'
!macroend
