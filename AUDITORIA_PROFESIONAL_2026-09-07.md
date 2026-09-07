# Reauditoría profesional incremental de Columnia — escritorio

**Fecha de cierre:** 2026-09-07.  
**Base:** `d0e00fd` (`master`), versión `0.167.0`.  
**Dictamen:** el producto no presenta un fallo crítico ni alto nuevo en las rutas
ejercidas. La compilación, las suites funcionales y el recorrido nativo pasan,
pero tres gates locales quedaron rojos tras el rediseño y la modularización. La
distribución pública sigue condicionada además por pendientes externos ya
conocidos.

## 0. Alcance acordado

Reauditoría incremental exhaustiva de las doce áreas aplicables: código,
seguridad, rendimiento, accesibilidad, UI/UX, arquitectura, QA, refactorización,
redacción, documentación, configuración de escritorio y legal/cumplimiento.
Se revisó el estado posterior a las auditorías del 2026-09-05 y 2026-09-06,
clasificando lo nuevo frente a lo conocido.

- **Excluidos por acuerdo:** CLI del producto, automatización batch, workflows,
  GitHub Actions, dependencias vendorizadas, generados, compilados y SEO.
- **Plataforma ejercida:** Windows x64 con WebView2. macOS y Linux continúan sin
  certificación.
- **Contexto pendiente por decisión del usuario:** usuario principal, escala,
  canal y países de distribución, jurisdicción y normativa aplicable.
- **Datos y servicios:** solo datos sintéticos y almacenamiento local. No se
  escribió en producción ni en servicios externos.
- **Estado inicial de Git:** limpio. No existe `.codegraph/`; se usaron búsquedas
  dirigidas, pruebas y herramientas propias del repositorio.

## 1. Resumen ejecutivo

Columnia conserva una base funcional sólida después del rediseño y la extracción
de módulos. Pasan 284 pruebas frontend, 399 pruebas Rust, 10 E2E y el smoke de la
aplicación Tauri real. El recorrido nativo quedó dentro de sus presupuestos de
memoria y limpió el proyecto sintético. No se encontró una vulnerabilidad
explotable nueva ni secretos activos.

La reauditoría sí reproduce tres regresiones de control: el CSS supera por 234 B
su límite por archivo; el inventario IPC no reconoce el nuevo propietario de dos
comandos; y el gate de red confunde `RowSetCursor::fetch()` dentro de pruebas ODBC
con la API web `fetch()`. Esta última coincidencia también impide completar el
gate agregado de cadena de suministro. La ficha de auditoría de dependencias
publica todavía conteos y resultados anteriores. Ninguno de estos cuatro
hallazgos demuestra corrupción de datos, fuga o ejecución remota, pero un release
local no puede considerarse verde mientras sus controles fallen.

## 2. Puntuación y cobertura por área

| Área | Nota / 10 | Cobertura y motivo |
| --- | ---: | --- |
| 1. Código | 8,5 | Build, clippy y suites pasan; revisión estática y aplicación real. Los fallos están en herramientas de control. |
| 2. Seguridad | 8,5 | Revisión estática y recorrido nativo; 0 secretos actuales, 0 advisories npm y 0 vulnerabilidades Cargo no exceptuadas. Sin hallazgo explotable nuevo. |
| 3. Rendimiento | 7,5 | Primer render y memoria nativa pasan; el CSS excede 234 B el presupuesto contractual. |
| 5. Accesibilidad | 8,5 | E2E, cinco capturas, zoom 125/200 %, móvil y colores forzados pasan; lector real sigue pendiente. |
| 6. UI/UX | 8,5 | Revisada en navegador y WebView2 real; 10 E2E y capturas sin overflow. No se recorrieron todos los estados con datasets grandes. |
| 7. Arquitectura | 7,5 | La extracción mantiene contratos y pruebas, pero dejó el inventario/gate desincronizado. |
| 8. QA y testing | 7,5 | 693 pruebas aprobadas en total, pero tres gates reproducibles fallan. |
| 9. Refactorización | 8 | Fachadas compatibles y tests separados; el seguimiento documental de módulos quedó incompleto. |
| 10. Redacción | 8 | Interfaz revisada sin problemas nuevos; la ficha técnica conserva resultados obsoletos. |
| 11. Documentación | 7 | Buen contexto vivo, con una tabla de dependencias y un inventario IPC desactualizados. |
| 12. Configuración de escritorio | 8 | CSP, capability, instalador y updater pasan; el pipeline local integral no queda verde por los gates señalados. |
| 13. Legal y cumplimiento | 5,5 | El gate técnico pasa; diez decisiones jurídicas y el canal/jurisdicción siguen pendientes por acuerdo. |

SEO no aplica a esta aplicación de escritorio.

## 3. Evidencias ejecutadas

| Comprobación | Resultado |
| --- | --- |
| `npm run test:coverage` | 284/284; cobertura global 93,34 % líneas y 83,57 % ramas; cinco umbrales críticos aprobados. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 399 aprobadas, 4 ignoradas explícitamente, 0 fallidas. |
| `npm run build` | 74 módulos; build aprobado. |
| `cargo clippy --all-targets -- -D warnings` | Aprobado. |
| `npm run test:e2e` | 10/10 en 14,8 s. |
| `npm run smoke:cdp` | WebView2/Tauri real aprobado; IPC nativo, proyectos y cleanup verificados. |
| Presupuesto nativo | 498.577.408 B working set y 256.425.984 B privados; ambos dentro del contrato. |
| Evidencia visual | Cinco casos aprobados en `.local/validation/accessibility-visual/20260907T223346Z`. |
| `npm audit --json` | 0 vulnerabilidades entre 287 dependencias. |
| `cargo audit` | 0 vulnerabilidades no exceptuadas; 17 avisos unmaintained, 1 unsound, 1 yanked y 2 excepciones conocidas. |
| `cargo deny ... check` | Aprobado con warnings ya documentados de duplicados/advisories. |
| `npm run secrets:check` | 475 archivos, 0 coincidencias. |
| Notices / toolchains / legal / instalador / updater | Aprobados. |
| `npm run network:check` | Falló por falso positivo en una prueba ODBC. |
| `npm run supply-chain:check` | Falló únicamente al propagar el gate de red anterior. |
| `npm run ipc:check` | Falló por propietario modular desactualizado. |
| Presupuesto frontend | Falló: CSS 131.306 B frente a 131.072 B. |

## 4. Hallazgos detallados

### R7-01 — El CSS vuelve a exceder el presupuesto contractual

- **Severidad:** Media · Regresión posterior a la revisión visual.
- **Ubicación:** `tools/check-bundle.mjs:8`, `src/styles.css:1`.
- **Problema:** el build produce `assets/index-P5S176k-.css` con 131.306 B raw;
  el límite vigente es 128 KiB (131.072 B). El gate falla por 234 B.
- **Impacto:** el release local no puede aprobar y se pierde la garantía de que
  el rediseño conserve el presupuesto acordado. No se observó una degradación de
  primer render en el recorrido medido.
- **Solución propuesta:** retirar reglas redundantes o dividir estilos por fase
  sin elevar el límite. Volver a ejecutar build y presupuesto.
- **Aceptación:** cada CSS queda en o bajo 131.072 B raw y 40 KiB gzip; build,
  E2E y matriz visual siguen pasando.
- **Esfuerzo:** bajo.

### R7-02 — El inventario IPC no siguió la extracción modular

- **Severidad:** Media · Nuevo.
- **Ubicación:** `src-tauri/src/lib.rs:154`,
  `docs/reference/ipc-inventory.json:34`, `tools/check-ipc-inventory.mjs:103`.
- **Problema:** `list_sample_datasets` e `inspect_sample_dataset` están registrados
  como `dataset::samples::*`, pero el inventario aún declara el módulo `dataset`.
  Además, `sourceFiles` y la lista generada siguen apuntando a `src/bridge.ts` y
  omiten los nuevos módulos `src/bridge/*` y `src-tauri/src/dataset/samples.rs`;
  el checker ni siquiera compara `sourceFiles`.
- **Impacto:** `npm run ipc:check` falla y el inventario publicado no describe la
  superficie real. Futuras extracciones podrían quedar fuera de la revisión
  documental aunque las pruebas de paridad funcional sigan verdes.
- **Solución propuesta:** actualizar el generador y el inventario con todos los
  propietarios reales; verificar también `sourceFiles` y evitar duplicar listas
  manuales entre el test y el checker.
- **Aceptación:** `ipc:check` pasa con 67 comandos de producción, 4 debug y 58
  estructuras; todo archivo fuente consumido por la paridad figura y se valida.
- **Esfuerzo:** bajo.

### R7-03 — El gate de red confunde un cursor ODBC de prueba con `fetch` web

- **Severidad:** Media · Regresión de gate.
- **Ubicación:** `src-tauri/src/remote_databases.rs:724`,
  `tools/check-network-policy.mjs:11`, `tools/check-network-policy.mjs:24`.
- **Problema:** el patrón `\bfetch\s*\(` escanea todo archivo Rust, incluida la
  sección `#[cfg(test)]`. Coincide con `RowSetCursor::fetch()` usado para releer
  filas en el round-trip ODBC, no con una API HTTP.
- **Impacto:** `network:check` y `supply-chain:check` quedan rojos aunque no exista
  una conexión web nueva. Un control con falsos positivos bloquea releases y
  reduce la confianza en sus alertas reales.
- **Solución propuesta:** distinguir invocaciones web por lenguaje o excluir de
  forma demostrable módulos Rust `#[cfg(test)]`; conservar detección de clientes
  HTTP reales en producción y añadir fixtures positiva/negativa del checker.
- **Aceptación:** el cursor ODBC de prueba no infringe la política; una llamada
  real a `fetch()` en TypeScript y un cliente HTTP Rust de producción sí fallan;
  `network:check` y `supply-chain:check` pasan.
- **Esfuerzo:** bajo.

### R7-04 — La ficha de dependencias publica evidencia anterior

- **Severidad:** Baja · Regresión documental de T6-11.
- **Ubicación:** `docs/reference/dependency-audit.md:69`.
- **Problema:** la tabla conserva 347 archivos escaneados, afirma que red está
  aprobada y registra 65 comandos IPC. La corrida actual inspecciona 475 archivos,
  red falla por R7-03 y el contrato vigente espera 67 comandos.
- **Impacto:** una persona que evalúe preparación de release obtiene un estado
  contradictorio con los gates reproducibles.
- **Solución propuesta:** actualizar la tabla desde evidencia actual después de
  R7-02/R7-03 y añadir una comprobación que enlace resultados versionados cuando
  sea viable.
- **Aceptación:** ficha, inventario, contexto y salidas actuales coinciden; el
  checker documental detecta conteos vigentes incoherentes.
- **Esfuerzo:** bajo.

## 5. Clasificación y trazabilidad

| Hallazgo | Clasificación | Tarea |
| --- | --- | --- |
| R7-01 | Regresión posterior al rediseño | T7-01 |
| R7-02 | Nuevo tras modularización | T7-02 |
| R7-03 | Regresión de gate introducida por cobertura ODBC | T7-03 |
| R7-04 | Regresión documental de T6-11 | T7-04 |

Pendientes ya conocidos, sin duplicar: T6-05 (round-trip SQL Server real),
T5-18/T5-20 (aceptación jurídica/canal), auditoría manual con lector y High
Contrast, VM limpia, updater real y validación macOS/Linux.

## 6. Plan de acción priorizado

- **Quick wins:** T7-03, T7-02 y T7-01; después T7-04.
- **Corto plazo:** ejecutar juntos build, presupuesto, IPC, red, supply chain,
  documentación, E2E y smoke nativo para cerrar la tanda sin falsos verdes.
- **Medio plazo:** completar los pendientes externos ya existentes; no se crean
  IDs duplicados.

## 7. Puntos fuertes que conservar

- Frontera IPC tipada con paridad de comandos, argumentos, retornos y estructuras.
- CSP y capability de Tauri estrechas; ODBC solo bajo acción explícita.
- Mutaciones, guardado/reapertura y cleanup pasan sobre la aplicación real.
- Cobertura frontend alta y 399 pruebas Rust aprobadas.
- Presupuestos de memoria nativa se respetan sin elevar límites.
- Capturas reproducibles para móvil, zoom y colores forzados.
- Publicación atómica, saneamiento de reportes y parámetros ODBC tipados se
  conservan en las rutas inspeccionadas.

## 8. Zonas no cubiertas y límites

- SQL Server real, updater con canal real, VM limpia, macOS y Linux.
- Lector NVDA/Narrador y hardware Windows High Contrast.
- Todos los formatos y operaciones sobre datasets mayores; se reutilizó la
  evidencia vigente y el smoke nativo, no se repitió `perf:benchmark` ni
  `perf:webview2` de 100 MiB.
- No se revisaron CLI, batch, workflows ni GitHub Actions por exclusión expresa.
- Sin usuario, escala, jurisdicción, canal ni normativa no se emite una opinión
  jurídica ni se certifica WCAG completo.

La revisión de seguridad es un escaneo asistido y no sustituye una auditoría
profesional independiente o una prueba de penetración para un producto que vaya
a manejar datos sensibles en producción.
