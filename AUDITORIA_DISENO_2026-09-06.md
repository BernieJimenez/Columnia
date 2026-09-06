# Revisión de diseño de Columnia — 2026-09-06

Estado: **DONE_WITH_CONCERNS**. Rediseño aplicado y verificado en navegador;
aceptación nativa pendiente y gate Fast detenido por formato Rust preexistente.

## Alcance y criterio

Aplicación de escritorio para preparar datos: navegación, Cargar, Revisar,
Preparar, Entregar, vista previa, consulta SQL, transformaciones, reglas y
preferencias. Revisión inicial con capturas antes de modificar la interfaz.
Se usó Playwright instalado, con fixtures sintéticas del bridge Tauri existente.
No se instalaron dependencias, fuentes ni servicios externos.

Primera impresión: la aplicación comunica una utilidad técnica, pero la
navegación, las superficies y los paneles comparten demasiado peso visual.
La mirada va al nombre, al título y a los separadores; la selección de archivo
queda lejos de su explicación. Las fases son reconocibles, aunque la repetición
de texto y contornos dificulta escanear el área de trabajo.

Dirección aplicada: navegación petróleo, espacio de trabajo neutro, acento teal
para acciones, tipografía local Aptos/Bahnschrift y escalas compartidas. La
ilustración de carga representa una tabla y no solicita recursos de red.

## Hallazgos y resolución

| Hallazgo | Impacto | Resolución | Estado |
| --- | --- | --- | --- |
| Navegación y contenido sin separación suficiente | Alto | Rail oscuro, marca de columnas, estados activos distinguibles | Verificado |
| Selección alejada de las instrucciones de carga | Alto | Botón, instrucciones, formatos y capacidad en una sola zona | Verificado |
| Colores literales y reglas repetidas entre temas | Medio | Variables compartidas, retiro de 122 declaraciones redundantes y estilos de carga sin uso | Verificado |
| Jerarquía tipográfica y contornos compiten con los datos | Medio | Títulos, métricas, tablas y controles con escala común; retiro de sombras de paneles | Verificado |
| Sistema no aplica toda la paleta oscura | Alto | Selectores cubren el atributo `system` y ausencia de atributo | Verificado |
| Preferencias alterna dos veces con un clic | Alto | Se cancela la acción nativa porque React controla la apertura | Verificado |
| Contraste de controles y etiquetas entre temas | Alto | Botones primarios, selector de tema y etiquetas con colores semánticos correctos | Verificado |

Evaluación visual orientativa, no métrica automatizada: diseño 6/10 → 8/10;
patrones genéricos 3/10 → 2/10 (menor es mejor). Las puntuaciones resumen el
juicio visual; las pruebas y capturas son la evidencia verificable.

## Verificación

- 284 pruebas frontend aprobadas en 34 archivos.
- 10 pruebas E2E aprobadas, incluyendo regresión de apertura por clic/teclado y
  cambios Claro/Oscuro/Sistema con cambio de preferencia del sistema operativo.
- Build TypeScript/Vite y `cargo check` aprobados.
- Presupuesto frontend aprobado: 586.492 bytes raw y 146.847 bytes gzip en
  cinco archivos; se conservaron los límites existentes.
- 48 estados: cuatro fases × 1440/1024/768/390 px × Claro/Oscuro/Sistema.
  Ningún overflow horizontal del documento ni error JavaScript observado.
- 18 inspecciones adicionales de vista previa, SQL, transformaciones, reglas,
  preferencias y preferencias móviles. Sin overflow; el comprobador limitado
  de contraste sobre texto/controles visibles no detecta ratios insuficientes.
  Esto no equivale a una certificación completa de WCAG.
- Evidencia visual estándar aprobada para escritorio, móvil, zoom 125/200 % y
  colores forzados; foco, landmarks y tamaño mínimo de targets comprobados.

## Evidencia local y reproducción

Commits de implementación: `aeb8347` (sistema visual y carga) y `72bc681`
(preferencias y prueba de regresión).

Directorio `.local/design-review/`:

- `before-load.png`, `before-{load,review,prepare,deliver}-loaded.png`:
  capturas iniciales; `after-load.png` y `{light,dark,system}-{1440,390}-{fase}.png`:
  resultado visual.
- `matrix.mjs` y `verification.json`: matriz de 48 estados.
- `details.mjs` y `details.json`: recorridos ampliados y contraste limitado.
- `unit-tests.log`, `e2e.log`, `cargo-check.log`, `bundle-budget.json`.
- `fast-gate.log`: bloqueo de formato Rust anterior a los cambios de interfaz.

Los scripts de inspección esperan Vite en `http://127.0.0.1:5173` y reutilizan
el mock de `e2e/accessibility.spec.ts`. Se ejecutan con `node` desde la raíz.
La evidencia de accesibilidad estándar está en
`.local/validation/accessibility-visual/20260906T220235Z/summary.json`.

## Límites y pendientes

El gate Fast se detiene en `cargo fmt -- --check` por dos expresiones de una
prueba preexistente de JOIN/trazabilidad en `src-tauri/src/dataset.rs`.
No se modificó ese archivo. Los demás checks ejecutados se registran por
separado; no se presenta el gate completo como aprobado.

Los datos de prueba y el motor simulado sirven para verificar la interfaz.
Esta revisión no vuelve a certificar procesamiento nativo, selectores de
Windows, rendimiento de datasets grandes ni instalador. No se publicó release.

Resumen para revisión: rediseño de las cuatro fases, con siete hallazgos
resueltos, preferencias corregidas y validación visual/funcional en navegador.
