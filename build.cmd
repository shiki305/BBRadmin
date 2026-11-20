@echo off
rem Builds BBRadmin.exe with the C# compiler that ships with Windows.
rem No .NET SDK, no Visual Studio, nothing to install: %SystemRoot%\Microsoft.NET\Framework64\v4.0.30319\csc.exe
rem (C# 5, .NET Framework 4.8). The output is one exe: build\BBRadmin.exe
rem
rem The co-op server (vendor\, BBSC: Yharnam Together 1.0.0's Windows server) is packed into one zip
rem and carried inside the exe as the resource BBRadmin.server.zip, so a friend downloads one file.
setlocal

set PROJ=%~dp0
set FWDIR=%SystemRoot%\Microsoft.NET\Framework64\v4.0.30319
if not exist "%FWDIR%\csc.exe" set FWDIR=%SystemRoot%\Microsoft.NET\Framework\v4.0.30319
set CSC=%FWDIR%\csc.exe
if not exist "%CSC%" (
  echo csc.exe not found: this build needs Windows .NET Framework 4.x
  exit /b 1
)

set OUT=%PROJ%build
if not exist "%OUT%" mkdir "%OUT%"

rem --- the co-op server that travels inside the exe (ServerAssets unpacks it on the first host) ---
if not exist "%PROJ%vendor\server\shadnet.exe" (
  echo vendor\server\shadnet.exe is missing: the co-op server files are not in this checkout
  exit /b 1
)
set ZIP=%OUT%\server.zip
if exist "%ZIP%" del "%ZIP%"
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory('%PROJ%vendor','%ZIP%',[System.IO.Compression.CompressionLevel]::Optimal,$false)"
if not exist "%ZIP%" (
  echo.
  echo BUILD FAILED: could not pack vendor\ into server.zip
  exit /b 1
)

"%CSC%" /nologo /target:winexe /platform:anycpu /optimize+ /codepage:65001 ^
  /out:"%OUT%\BBRadmin.exe" ^
  /win32manifest:"%PROJ%app.manifest" ^
  /resource:"%ZIP%",BBRadmin.server.zip ^
  /reference:"%FWDIR%\System.dll","%FWDIR%\System.Core.dll","%FWDIR%\System.Drawing.dll","%FWDIR%\System.Windows.Forms.dll","%FWDIR%\System.Security.dll","%FWDIR%\System.IO.Compression.dll","%FWDIR%\System.IO.Compression.FileSystem.dll" ^
  "%PROJ%src\Program.cs" "%PROJ%src\MainForm.cs" "%PROJ%src\Core\*.cs"

if errorlevel 1 (
  echo.
  echo BUILD FAILED
  exit /b 1
)

echo.
echo OK: %OUT%\BBRadmin.exe
