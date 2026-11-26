@echo off
rem Image Viewer - register as a default-app candidate (HKCU only, no admin).
setlocal
set "EXE=%~dp0image-viewer.exe"
if not exist "%EXE%" (
    echo image-viewer.exe not found next to install.bat
    pause
    exit /b 1
)
"%EXE%" --register
pause
