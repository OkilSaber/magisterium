<#
  Build Windows : installeurs .msi (WiX) et .exe (NSIS).

    .\scripts\build-windows.ps1                 les deux installeurs
    .\scripts\build-windows.ps1 -Bundles nsis   un seul (msi ou nsis)
    .\scripts\build-windows.ps1 -Fresh          réinstalle les dépendances npm avant

  À lancer sur Windows (PowerShell 5.1 ou 7). Si les scripts sont bloqués :
    powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
#>
param(
  [string]$Bundles = "msi,nsis",
  [switch]$Fresh
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

function Step($msg) { Write-Host "`n> $msg" -ForegroundColor Magenta }
function Fail($msg) { Write-Host "x $msg" -ForegroundColor Red; exit 1 }

if ($env:OS -ne "Windows_NT") { Fail "Ce script se lance sur Windows (macOS : scripts/build-macos.sh)." }

Step "Vérification des outils"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Fail "Rust est introuvable : https://rustup.rs" }
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { Fail "Node.js est introuvable : https://nodejs.org (version 20 ou plus)" }

# Tauri a besoin du compilateur C++ de Visual Studio (Build Tools, charge « Développement Desktop en C++ »).
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$hasMsvc = (Test-Path $vswhere) -and (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if (-not $hasMsvc) {
  Fail "Compilateur MSVC absent. Installe « Visual Studio Build Tools » avec « Développement Desktop en C++ » : https://visualstudio.microsoft.com/visual-cpp-build-tools/"
}
Write-Host ("cargo {0} · node {1} · {2}" -f ((cargo --version) -split ' ')[1], (node --version), $env:PROCESSOR_ARCHITECTURE)

if ($Fresh -or -not (Test-Path node_modules)) {
  Step "Installation des dépendances npm"
  npm ci
  if ($LASTEXITCODE -ne 0) { Fail "npm ci a échoué." }
}

Step "Compilation (quelques minutes la première fois)"
npm run tauri build -- --bundles $Bundles
if ($LASTEXITCODE -ne 0) { Fail "La compilation a échoué." }

Step "Terminé"
Get-ChildItem -Path "src-tauri\target\release\bundle" -Recurse -Include *.msi, *-setup.exe |
  ForEach-Object { "  {0}  ({1:N1} Mo)" -f $_.FullName, ($_.Length / 1MB) }
