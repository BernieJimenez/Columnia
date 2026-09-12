# Cómo validar la evidencia del release

Este flujo compila el binario Tauri de release, captura cinco ventanas
definidas sobre el WebView2 del binario y comprueba que no cambien los contratos
ni los hashes visuales aprobados.

## Requisitos previos

- Windows x64, WebView2 y las Build Tools de Visual Studio.
- Node.js LTS, Rust estable y dependencias instaladas con `npm install`.
- Una estación interactiva de Windows. La captura abre procesos locales y usa
  CDP solo en loopback.

## Pasos

Para ejecutar todos los gates locales en una sola corrida, con publicación
remota desactivada:

```powershell
npm run release:dry-run
```

El orquestador exige una rama y un árbol Git limpios, conserva un reporte en
`.local/validation/release-orchestration/` y no crea tags, no publica artefactos
ni contacta servicios externos.

Cuando se ejecuta desde `release.ps1`, la captura reutiliza el ejecutable que ya
produjo el gate `Release`/`Package`; la opción `-SkipBuild` existe para evitar
que la evidencia se genere desde una reconstrucción distinta.

1. Ejecuta los contratos de documentación y versión:

   ```powershell
   npm run docs:check
   ```

2. Genera el binario Tauri sin instalador y captura la evidencia:

   ```powershell
   npm run accessibility:release
   ```

   El script usa el fixture sintético `fixtures/automation/input.csv`, conserva
   solo su SHA-256 y escribe capturas/sumario en `.local/validation/`.

3. Comprueba el sumario contra el baseline versionado:

   ```powershell
   npm run accessibility:release:check
   ```

4. Si necesitas la evidencia web del preview, independiente del binario Tauri,
   ejecuta:

   ```powershell
   npm run accessibility:visual
   npm run accessibility:check
   ```

## Ventanas y contrato

La captura de release usa exactamente estos casos: desktop `1280×900`, móvil
`390×844`, zoom CSS `125%`, zoom CSS `200%` y `forced-colors`. Cada caso
comprueba un `main`, una `nav`, un `aside`, foco visible, targets de al menos
24 px y ausencia de overflow horizontal. El escenario de zoom `200%` activa el
reflow compacto del shell para que la escala no convierta la tarjeta en una
columna ilegible.

El baseline exige además que el origen sea `tauri-release-binary`, que la
versión coincida con npm/Cargo/Tauri y que el hash de cada screenshot se
mantenga. La captura debe corresponder al `HEAD` limpio; al versionar el
baseline se permite únicamente un commit posterior que modifique ese archivo.
Una diferencia falla el gate y deja el nuevo sumario para revisión; no
actualices el baseline para ocultar una regresión.

## Verificación

La salida esperada termina con mensajes similares a:

```text
Evidencia release aprobada: .local/validation/release-evidence/<timestamp>
Baseline release aprobado: .local/validation/release-evidence-check/<timestamp>
```

El árbol de trabajo no debe recibir capturas ni datasets temporales. `.local/`
está ignorado por Git.

## Perfil de memoria del ejecutable release

`npm run smoke:cdp` inicia `tauri dev`; sirve para validar mutaciones sintéticas
que solo están disponibles en compilaciones debug. Para medir la interfaz real
del ejecutable release contra los límites V1 de 512 MiB de working set y 256
MiB privados, compila primero y luego ejecuta el recorrido de solo lectura:

```powershell
npm run tauri build -- --no-bundle
npm run smoke:cdp:release
```

El segundo comando requiere `src-tauri/target/release/columnia.exe`, abre
ProjectsPanel sin crear ni modificar proyectos y guarda el perfil de procesos
en `.local/validation/webview2-cdp-release/`. Así se conserva separado del
historial de mutaciones sintéticas. El probe restaura la variable de depuración
de WebView2 y cierra los procesos que inició. La memoria suma Columnia y sus
procesos WebView2 descendientes para reflejar el consumo completo del runtime.
Las mutaciones sintéticas y las pruebas de reinicio siguen reservadas al
ejecutable debug.

## Release firmado y updater

Para validar también el canal de actualizaciones, usa una clave privada ubicada
fuera del repositorio y define un endpoint HTTPS más la base HTTPS donde se
servirán los assets:

```powershell
$env:COLUMNIA_UPDATER_ENDPOINT = "https://updates.example/columnia.json"
$env:COLUMNIA_UPDATER_ASSET_BASE_URL = "https://downloads.example/columnia/0.95.0/"
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\ruta-privada\columnia-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run release:updater:dry-run
```

El flujo produce firmas `.sig` de Tauri, un manifiesto estático y un inventario
de SHA-256 bajo `.local/validation/release-orchestration/`. El gate falla si
falta el instalador, la firma, la URL HTTPS o cualquier hash; no publica, crea
tags ni sube archivos. `npm run updater:check` puede volver a verificar un par
manifiesto/inventario ya generado pasando sus rutas con `--manifest` y
`--inventory`. Para comprobar el contrato de forma aislada, ejecuta
`npm run updater:contract:test`: genera un fixture temporal y verifica el caso
válido, truncado de artefacto, firma alterada, manifiesto incompleto/corrupto y
URL HTTP. Es una prueba estructural local; la aceptación de I6 todavía requiere
ejercitar un canal real, pérdida de red, recuperación y rotación de claves.

## Troubleshooting

- **El puerto CDP está ocupado:** invoca directamente el wrapper PowerShell con
  otro puerto, por ejemplo `powershell -NoProfile -ExecutionPolicy Bypass -File
  tools/capture-release-evidence.ps1 -Port 9223`, y vuelve a ejecutar el gate.
- **No aparece el binario:** confirma que `npm run tauri build -- --no-bundle`
  termina correctamente y que existe `src-tauri/target/release/columnia.exe`.
- **El shell no monta:** revisa el sumario y los logs sanitizados del directorio
  de evidencia; no conectes el script a un puerto que ya tenga otro proceso.
- **Cambió un hash:** inspecciona la imagen nueva y el sumario antes de decidir
  si el cambio es intencional. El baseline solo se actualiza junto con una
  explicación y una nueva versión de evidencia.

## Relacionado

- [Referencia de evidencia](../reference/release-evidence.md)
- [Referencia de la CLI](../reference/cli.md)
- [Checklist de accesibilidad manual](../../ACCESSIBILITY_MANUAL_CHECKLIST.md)
