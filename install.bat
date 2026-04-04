@echo off
REM Wax Windows bootstrap — downloads and runs install.ps1 from GitHub.
REM Usage: run from repo root, or paste this one line into cmd.exe:
REM
REM   powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/semitechnological/wax/master/install.ps1 | iex"

setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/semitechnological/wax/master/install.ps1 | iex"
endlocal
