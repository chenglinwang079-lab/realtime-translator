@echo off
call "E:\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64
cd /d "C:\Users\mu\Documents\Codex\2026-04-25-1-realtimetranslator-2-3-v1-0\realtime-translator\src-tauri"
"C:\Users\mu\.cargo\bin\cargo.exe" check
