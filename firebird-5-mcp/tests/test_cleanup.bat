@echo off
rem Removes the test database files created by test_createdb.bat.
rem
rem Usage:
rem   tests\test_cleanup.bat
rem
rem NOTE: this deletes files directly from the server's allowed path.
rem Make sure no client is attached to those databases first.
rem Adjust the paths below to match your config.json allowed_paths.
del /f /q "C:\database\ps_default.fdb" "C:\database\ps_4k.fdb" "C:\database\ps_bad.fdb" 2>nul
echo cleaned
endlocal
