@echo off
setlocal
cd /d "%~dp0"
if not exist "target\debug\nzxt-cam.exe" (
    echo Compile a previa primeiro, conforme docs\USO.md.
    pause
    exit /b 1
)
start "" "target\debug\nzxt-cam.exe" --preview
