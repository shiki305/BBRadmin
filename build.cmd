@echo off
rem Compila BBRadmin.exe em Rust (cargo build --release).
rem O servidor do co-op (vendor\, BBSC: Yharnam Together 1.0.0) e o app.manifest sao embutidos
rem automaticamente pelo build.rs no executavel unico: build\BBRadmin.exe
setlocal

set PROJ=%~dp0
where cargo >nul 2>nul
if errorlevel 1 (
  echo cargo nao encontrado: instale o toolchain Rust em https://rustup.rs
  exit /b 1
)

if not exist "%PROJ%vendor\server\shadnet.exe" (
  echo vendor\server\shadnet.exe nao encontrado: os arquivos do servidor nao estao neste clone
  exit /b 1
)

pushd "%PROJ%"
cargo build --release
if errorlevel 1 (
  popd
  echo.
  echo BUILD FAILED
  exit /b 1
)
popd

set OUT=%PROJ%build
if not exist "%OUT%" mkdir "%OUT%"
copy /y "%PROJ%target\release\BBRadmin.exe" "%OUT%\BBRadmin.exe" >nul
if errorlevel 1 (
  echo.
  echo BUILD FAILED: nao foi possivel copiar target\release\BBRadmin.exe
  exit /b 1
)

echo.
echo OK: %OUT%\BBRadmin.exe
