# How to validar la evidencia del release

Este flujo compila el binario Tauri de release, captura cuatro ventanas
definidas sobre el WebView2 del binario y comprueba que no cambien los contratos
ni los hashes visuales aprobados.

## Prerequisitos

- Windows x64, WebView2 y las Build Tools de Visual Studio.
- Node.js LTS, Rust estable y dependencias instaladas con `npm install`.
- Una estación interactiva de Windows. La captura abre procesos locales y usa
  CDP solo en loopback.

## Pasos

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

La captura de release usa exactamente estas ventanas: desktop `1280×900`, móvil
`390×844`, escala `125%` y `forced-colors`. Cada caso comprueba un `main`, una
`nav`, un `aside`, foco visible, targets de al menos 24 px y ausencia de
overflow horizontal.

El baseline exige además que el origen sea `tauri-release-binary`, que la
versión coincida con npm/Cargo/Tauri y que el hash de cada screenshot se
mantenga. Una diferencia falla el gate y deja el nuevo sumario para revisión;
no actualices el baseline para ocultar una regresión.

## Verificación

La salida esperada termina con mensajes similares a:

```text
Evidencia release aprobada: .local/validation/release-evidence/<timestamp>
Baseline release aprobado: .local/validation/release-evidence-check/<timestamp>
```

El árbol de trabajo no debe recibir capturas ni datasets temporales. `.local/`
está ignorado por Git.

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
