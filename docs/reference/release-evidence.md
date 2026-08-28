# Evidencia visual del release

La evidencia visual de release es un sumario JSON más cinco capturas tomadas
desde el binario Tauri optimizado. Sirve para detectar cambios del shell antes de
publicar un artefacto, no para sustituir una auditoría manual con tecnologías de
asistencia.

## Contrato del sumario

El script escribe un documento `schemaVersion: 1` con estos campos:

| Campo | Tipo | Regla |
| --- | --- | --- |
| `source` | string | Debe ser `tauri-release-binary`. |
| `projectVersion` | string | Coincide con npm, Cargo, Cargo.lock y Tauri. |
| `binary` | object | Ruta relativa, tamaño y SHA-256 del ejecutable. |
| `fixture` | object | Ruta relativa y SHA-256 de la fixture sintética estable. |
| `cases` | array | Exactamente desktop, mobile, zoom-125, zoom-200 y forced-colors. |
| `status` | string | Debe ser `passed` para entrar al baseline. |

Cada caso conserva viewport, escala, modo forced-colors, inspección de
landmarks/foco/overflow y SHA-256 de la imagen. No guarda filas ni rutas
absolutas de datasets.

## Baseline

`fixtures/accessibility/release-evidence-baseline-v1.json` fija las ventanas,
el contrato accesible y, después de la primera aprobación, los hashes de las
capturas canónicas. `tools/check-release-evidence.mjs` compara el último
sumario con ese archivo y vuelve a calcular cada hash desde el PNG.

Una diferencia falla el gate. Para aceptar un cambio visual intencional:

1. inspecciona las cinco imágenes nuevas;
2. explica el cambio en `CHANGELOG.md` o en un ADR;
3. regenera el baseline con el comando explícito documentado y guarda ese
   archivo en un commit posterior exclusivo del baseline;
4. ejecuta otra vez `npm run accessibility:release:check`.

No se versionan capturas dentro de `docs/` manualmente. Las capturas canónicas
son artefactos generados y su ownership, fecha, propósito y hash viven en el
manifest del baseline.

## Relacionado

- [How to validar evidencia](../how-to/validate-release-evidence.md)
- [Baseline visual del preview](../../fixtures/accessibility/visual-baseline-v1.json)
- [Política de fixtures](fixtures-policy.md)
