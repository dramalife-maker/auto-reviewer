@echo off
python "%~dp0write_draft_then_hang.py"
exit /b %ERRORLEVEL%
