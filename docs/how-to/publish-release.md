# Publicar y verificar un release

La publicación de Columnia es manual. `tools/release.ps1` prepara y verifica la
release, pero no crea tags, no sube archivos y no contacta un canal remoto.
Esto mantiene una barrera explícita antes de cambiar el canal público.

## Preparar el release

Ejecuta el flujo desde un árbol limpio y conserva la evidencia generada:

```powershell
$env:COLUMNIA_UPDATER_ENDPOINT = "https://updates.example/columnia.json"
$env:COLUMNIA_UPDATER_ASSET_BASE_URL = "https://downloads.example/columnia/0.93.0/"
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\ruta-privada\columnia-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run release:updater:dry-run
```

El flujo verifica toolchains, documentación, IPC, cobertura, supply chain,
SBOM, instaladores, el smoke instalado y el par firmado de manifiesto/asset.
La clave privada nunca se copia al repositorio ni a la carpeta de evidencia.

## Verificar lo que quedó publicado

Después de subir el manifiesto y el instalador, vuelve a descargarlos desde el
canal real:

```powershell
npm run updater:verify-published -- --manifest-url https://updates.example/columnia.json --output-dir .local/validation/published-assets/20260828T000000Z --target windows-x86_64 --expected-version 0.93.0
```

La verificación exige HTTPS, descarga el manifiesto y el instalador, valida
tamaño, SHA-256 y la firma minisign Ed25519 sobre BLAKE2b-512 con la clave
pública embebida en la compilación. Un fallo elimina el archivo descargado
parcial y deja un `summary.json` con el motivo; la versión instalada no se
modifica.

## Reanudar después de un fallo

- Si falla antes del tag, corrige el problema y repite el dry-run desde un
  árbol limpio.
- Si ya existe el tag, no lo muevas ni lo fuerces. Reanuda usando exactamente
  ese tag y la misma versión; `--expected-version` evita verificar otro
  manifiesto por error.
- Si faltan assets en el canal, sube únicamente los archivos producidos por el
  tag y vuelve a ejecutar `updater:verify-published`.
- Si un asset o manifiesto remoto difiere del tag, detén la promoción y no
  sobrescribas silenciosamente el canal estable. Conserva la versión estable
  anterior hasta reparar y verificar el canal.
- Una firma inválida, un hash diferente, una red caída o un manifiesto corrupto
  son fallos cerrados: no se instala ni se reemplaza la versión local.

La rotación de claves requiere además una release puente firmada por la clave
anterior. El procedimiento y la retención de la clave histórica están en la
[política de distribución](../reference/legal-distribution-review.md).
