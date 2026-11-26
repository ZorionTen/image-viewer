@echo off
rem Image Viewer - remove the registration created by install.bat.
setlocal
set "EXE=%~dp0image-viewer.exe"
if not exist "%EXE%" (
    echo image-viewer.exe not found next to uninstall.bat
    pause
    exit /b 1
)
"%EXE%" --unregister
pause
