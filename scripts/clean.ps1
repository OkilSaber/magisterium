<#
  Nettoie les artefacts de build (Windows).

    .\scripts\clean.ps1              dist\, src-tauri\target\, src-tauri\gen\
    .\scripts\clean.ps1 -Bundles     seulement les installeurs produits (.msi, .exe)
    .\scripts\clean.ps1 -All         tout, y compris node_modules\
    .\scripts\clean.ps1 -DryRun      affiche ce qui serait supprimé, sans rien toucher

  Ne touche jamais aux données de l'app (discussions, fournisseurs, clés API).
#>
param(
  [switch]$Bundles,
  [switch]$All,
  [switch]$DryRun
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if ($Bundles) {
  $targets = @("src-tauri\target\release\bundle")
} elseif ($All) {
  $targets = @("dist", "src-tauri\target", "src-tauri\gen", "node_modules")
} else {
  $targets = @("dist", "src-tauri\target", "src-tauri\gen")
}

$total = 0
$found = $false
foreach ($t in $targets) {
  if (-not (Test-Path $t)) { continue }
  $found = $true
  $size = (Get-ChildItem $t -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
  $total += $size
  "  {0,-45} {1:N0} Mo" -f $t, ($size / 1MB)
  if (-not $DryRun) { Remove-Item $t -Recurse -Force }
}

if (-not $found) {
  "Rien à nettoyer."
} elseif ($DryRun) {
  "Simulation : {0:N0} Mo seraient libérés (relance sans -DryRun pour supprimer)." -f ($total / 1MB)
} else {
  "Libéré : {0:N0} Mo." -f ($total / 1MB)
}
