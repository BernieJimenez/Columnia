# Auditoría profesional de Columnia

**Fecha:** 2026-08-28  
**Base revisada:** `master` en `137520b28b96233a27a6152978a57ef2a2dda9bd`  
**Versión declarada:** `0.57.0`  
**Tipo:** aplicación de escritorio local-first, Tauri 2 + Rust/Polars + React 19/TypeScript/Vite  
**Profundidad:** exhaustiva  
**Veredicto:** apta como prototipo local verificable; **no apta todavía para publicación pública**.

No se detectó ningún hallazgo crítico ni evidencia de SQL injection, command
injection, XSS, SSRF, secretos reales expuestos o permisos Tauri excesivos. El
dictamen de seguridad de este documento no sustituye una auditoría profesional
independiente ni un análisis legal aplicable a la jurisdicción de publicación.

## 0. Alcance acordado

Se incluyeron las áreas recomendadas y aprobadas por el usuario: código,
seguridad, rendimiento, accesibilidad y semántica, diseño responsivo y UI/UX,
arquitectura, QA y testing, refactorización y limpieza, ortografía y redacción,
documentación, DevOps/configuración y legal/cumplimiento.

- **Excluida:** SEO. Columnia es una aplicación de escritorio sin superficie
  pública indexable ni adquisición orgánica web.
- **Excluidos como código bajo revisión:** `node_modules/`, `dist/`, `coverage/`,
  `test-results/`, `playwright-report/`, `.local/`, `.codegraph/` y artefactos
  compilados. Sus configuraciones y evidencias sí se usaron para verificar gates.
- **Plataforma verificada:** Windows x64. macOS y Linux se trataron como objetivos
  de diseño, no como plataformas soportadas.
- **Contexto desconocido:** persona principal, escala esperada, canal de
  distribución, jurisdicción y normativa. Las conclusiones legales dependientes
  de esos datos se marcan **requiere revisión legal**.
- **Reauditoría:** se leyeron completos `ROADMAP.md`, `CHANGELOG.md` y
  `CONTEXTO.md`. Se conserva `CONTEXTO.md` como equivalente español ya adoptado
  por el proyecto; crear además `CONTEXT.md` duplicaría la misma fuente viva.
- **Límite de esta fase:** informe y documentación de planificación. No se
  corrigió código de producto ni se aprobaron baselines nuevos.

### Aplicabilidad del anexo Windows

- FlaUI no se prescribe como suite principal: la UI es React dentro de
  WebView2/Tauri, no WinUI/WPF/WinForms. El shell real se conduce por CDP y los
  diálogos Win32 ya tienen automatización dedicada en
  `tools/probe-webview2-native-selectors.mjs` y
  `tools/automate-native-file-dialog.ps1`. Debe reevaluarse FlaUI si crece la
  UI nativa fuera del WebView.
- La función de `capture-screenshots.ps1` ya está cubierta por
  `tools/capture-accessibility-evidence.mjs` y
  `tools/capture-release-evidence.mjs`, con capturas web y del binario release.
- `tools/release.ps1` no existe; no se duplica como hallazgo nuevo porque la Fase
  I7 ya lo mantiene abierto y F-06 vuelve a verificar ese bloqueo.

## 1. Resumen ejecutivo

Columnia tiene una frontera local-first bien diseñada, una capability Tauri
reducida, escrituras atómicas, contratos sanitizados y una suite amplia: 248
tests frontend, 234 Rust y 9 E2E aprobaron. `Full`, `Release` y `Package` también
aprobaron; este último produjo MSI y NSIS reproducibles para el commit auditado.

La señal global, sin embargo, es más optimista que el estado real. Tres recorridos
WebView2 excedieron el presupuesto de memoria privada, el benchmark durable se
interrumpió después de `project-save`, y el gate de rendimiento falló. Hay además
tres verdes falsos importantes: la cobertura se exige globalmente y no por capa,
`verify:tier -SkipPackage` omite Rust y supply chain junto al paquete, y la
evidencia release no está ligada al commit. La captura denominada `zoom-125`
tampoco aumenta el zoom CSS/texto. La publicación pública sigue bloqueada por
licencias `UNKNOWN`, instalación real, updater/firma y proceso de release.

### Los cinco problemas de mayor prioridad

| Prioridad | Problema | Consecuencia |
| --- | --- | --- |
| 1 | Regresión de memoria y persistencia durable | El gate de rendimiento falla y `project-save` tarda 123 s en 100 MiB. |
| 2 | Cobertura “por capa” cerrada en falso | Rutas críticas quedan por debajo de los umbrales aunque el total apruebe. |
| 3 | `-SkipPackage` omite validaciones no relacionadas | Puede presentarse un tier como verificado sin Rust, Clippy, SBOM ni supply chain. |
| 4 | Evidencia release no ligada a `HEAD` y baseline desactualizado | Capturas/binarios antiguos pueden confundirse con evidencia del código actual. |
| 5 | Distribución legal/operativa incompleta | 349 notices `UNKNOWN` y faltan pruebas reales de instalación, updater y release. |

### Clasificación de la reauditoría

- **Nuevos:** F-02, F-04, F-05, F-07, F-08, F-09, F-12, F-13,
  F-16, F-17 y F-18.
- **Ya conocidos:** F-06 y F-14; se verificaron de nuevo y no se presentan como
  deuda recién creada.
- **Regresiones o controles derivados:** F-01, F-03, F-10, F-11 y F-15.
- **Críticos:** ninguno verificado.

## 2. Tabla de puntuación

Las notas describen el commit auditado y no son porcentajes de trabajo terminado.

| Área | Nota | Justificación |
| --- | ---: | --- |
| Código | 7.2/10 | Tipado estricto, formato y pruebas fuertes; funciones y archivos de dominio excesivamente grandes. |
| Seguridad | 7.4/10 | Frontera Tauri estrecha y entradas locales robustas; batch permite destinos externos y hay excepciones SCA mal justificadas. |
| Rendimiento | 5.3/10 | Bundle y transformaciones pasan; memoria CDP y ciclo durable incumplen el baseline. |
| Accesibilidad y semántica | 8.0/10 | Teclado, landmarks, foco, `forced-colors` y responsive pasan; el caso 125% no prueba zoom real y falta lector. |
| Diseño responsivo y UI/UX | 8.6/10 | Los cuatro viewports/casos inspeccionados son coherentes, sin overflow ni bloqueos visuales observados. |
| Arquitectura | 6.1/10 | Buen límite React↔Rust y persistencia atómica; `dataset.rs` concentra demasiado dominio y hay riesgos de recuperación. |
| QA y testing | 6.5/10 | Gran volumen y variedad; varios gates no hacen cumplir literalmente su contrato. |
| Refactorización y limpieza | 6.0/10 | Sin TODO/FIXME relevantes ni código muerto evidente; monolitos y clones completos elevan el radio de cambio. |
| Ortografía y redacción | 7.2/10 | Producto mayormente consistente en español; documentación generada conserva anglicismos y tildes ausentes. |
| Documentación | 5.8/10 | Amplia y útil, pero CHANGELOG, auditoría de dependencias, métricas y gate documental derivaron. |
| DevOps y configuración | 6.0/10 | Buenos perfiles locales, SBOM, hashes y paquetes; faltan toolchains fijadas y orquestador de release. |
| Legal y cumplimiento | 5.0/10 | MIT está clara; notices, privacidad para usuario y descubribilidad legal no están listos para publicación. |

**Valoración global orientativa: 6.7/10.** La calidad del prototipo es mayor que
su preparación de distribución.

## 3. Evidencia de ejecución

| Verificación | Resultado observado |
| --- | --- |
| `tools/check.ps1 -Profile Full` | Aprobó en 92.4 s; reporte `.local/validation/20260827T225705Z-137520b-full.json`. |
| Vitest + cobertura | 248/248; global 90.09% statements/lines, 80.38% branches y 79.91% functions. |
| Rust | 234/234; `cargo fmt`, `cargo check` y Clippy `-D warnings` aprobaron. |
| Playwright web | 9/9; desktop, móvil, foco, ARIA, proyectos y primer render web aprobaron. |
| Accesibilidad visual web | Cuatro casos aprobaron sin overflow, con landmarks, foco y targets válidos. |
| WebView2/CDP | Tres recorridos funcionales aprobaron, pero los tres fallaron el presupuesto de memoria privada. |
| Benchmark CLI | Transformaciones de 100 MiB aprobaron; `project-save` tardó 123,074.82 ms y el flujo falló antes de completar `project-inspect`. El caso de 1 MiB repitió la interrupción. |
| `perf:check` | Falló por `cdp-memory` y `dataset-benchmark`; evidencia `.local/validation/performance-baseline/20260828T053153Z/summary.json`. |
| `tools/check.ps1 -Profile Release` | Aprobó en 261.2 s; npm audit 0, SBOM de 944 componentes y binario release. |
| Evidencia release fresca | La captura aprobó, pero `accessibility:release:check` falló contra el baseline: `.local/validation/release-evidence-check/20260828T054044Z/summary.json`. |
| `tools/check.ps1 -Profile Package` | Aprobó en 340.8 s; MSI de 52,191,232 B y NSIS de 33,280,624 B, inventariados en `.local/validation/20260828T054125Z-137520b-package.json`. |

## 4. Hallazgos detallados

### Rendimiento

#### F-01 — El gate de rendimiento falla en memoria y persistencia durable

**Severidad:** Alta · **Clasificación:** regresión · **Esfuerzo:** alto  
**Ubicación:** `fixtures/performance/performance-baseline-v1.json:5-23`,
`src-tauri/src/dataset.rs:1258-1263`, `src-tauri/src/dataset.rs:16577-16592`

**Problema (hechos).** El baseline limita memoria privada CDP a 256 MiB y
`project-save` a 60 s:

```json
"privateMemoryBytes": 268435456,
"projectSave": 60000
```

Tres recorridos WebView2 funcionales terminaron fuera del presupuesto. El gate
consolidado observó 274,149,376 B de memoria privada; otras dos corridas llegaron
a 283,291,648 B y 259.37 MiB. En 100 MiB, las seis transformaciones pasaron, pero
`project-save` tardó 123,074.82 ms y el benchmark terminó antes de registrar
`project-inspect`. Una repetición de 1 MiB falló en el mismo punto, por lo que no
se explica solo por el volumen. Históricamente el escenario de 100 MiB completaba
en unos 26–27 s.

El camino de guardado clona el frame activo y el historial:

```rust
frame: dataset.frame.clone(),
history: capture_project_history(&dataset.history, &dataset.frame)?,
```

**Impacto.** Se incumple una puerta declarada bloqueante; guardar proyectos puede
congelar una estación modesta y el benchmark deja sin verificar actualización,
reapertura, exportación, listado y borrado.

**Hipótesis pendiente de verificación.** Los clones completos y la serialización
del historial son candidatos a explicar el pico/latencia, pero la causa exacta de
la salida en `project-inspect` no quedó aislada. No debe corregirse por intuición.

**Solución propuesta.** Añadir diagnóstico de fase/error preservando sanitización,
perfilar `active_project_snapshot`/`write_generation`, reducir copias y repetir
100 MiB en tres corridas limpias. No elevar el baseline sin una decisión medida.

**Trazabilidad:** `T5-01`, `T5-02`.

#### F-02 — El probe declara éxito aunque el primer render exceda su presupuesto

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `tools/probe-webview2-playwright.mjs:55-70`,
`tools/probe-webview2-playwright.mjs:172-176`,
`tools/probe-webview2-playwright.mjs:217-224`

**Problema (hecho).** El probe calcula `withinBudget`, pero el estado final solo
depende de landmarks y foco:

```js
const status = landmarks.valid && focus.valid ? "passed" : "failed";
```

Dos corridas registraron 88,775.9 ms y 84,566.3 ms con
`withinBudget: false`, mientras `playwright.status` quedó `passed`. El mensaje de
error sí afirma comprobar “primer render, landmarks o foco”.

**Impacto.** Una regresión de arranque puede ocultarse detrás de un contrato de
accesibilidad correcto; consumidores del JSON reciben una señal contradictoria.

**Solución propuesta.** Separar `functionalStatus` de `performanceStatus` o hacer
que el gate compuesto exija ambos; distinguir compilación fría de arranque del
binario ya construido.

**Trazabilidad:** `T5-03`.

### QA y testing

#### F-03 — La cobertura “por capa” está cerrada en falso

**Severidad:** Alta · **Clasificación:** regresión · **Esfuerzo:** medio  
**Ubicación:** `vitest.config.ts:18-28`, `ROADMAP.md:877-879`

**Problema (hecho).** El roadmap marca umbrales por capa como terminados, pero
Vitest solo declara umbrales globales:

```ts
thresholds: {
  statements: 80,
  branches: 75,
  functions: 75,
  lines: 80,
},
```

La corrida global aprobó, aunque `App.tsx` obtuvo 48.14% de functions,
`DeliveryPhase.tsx` 52.38%, `PreparePhase.tsx` 65.38% y
`usePrepareController.ts` 45.16% de branches.

**Impacto.** Las rutas orquestadoras más sensibles pueden perder cobertura sin
romper el gate, exactamente el riesgo que la tarea cerrada pretendía evitar.

**Solución propuesta.** Configurar umbrales por archivo/grupo o un verificador
post-cobertura para capas críticas; añadir primero tests conductuales y luego
activar el gate para no normalizar exclusiones.

**Trazabilidad:** `T5-04`.

#### F-04 — `verify:tier -SkipPackage` omite mucho más que empaquetado

**Severidad:** Alta · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `tools/verify-tier.ps1:47-56`, `tools/check.ps1:158-198`

**Problema (hecho).** Todo `check.ps1 -Profile Package` está detrás de
`if (-not $SkipPackage)`. Por tanto, el flag también omite `cargo check`,
cobertura, Clippy, tests Rust, SBOM, supply chain, contrato del instalador y build
release.

**Impacto.** El nombre del flag induce a creer que solo evita MSI/NSIS; una
validación puede reportarse como tier completo sin comprobar el motor nativo o
la cadena de suministro.

**Solución propuesta.** Ejecutar siempre `Full` o `Release` y aislar únicamente
la etapa de bundling; renombrar el flag si se desea conservar una omisión mayor.

**Trazabilidad:** `T5-05`.

#### F-05 — La evidencia release no identifica el commit y el baseline actual falla

**Severidad:** Alta · **Clasificación:** nuevo · **Esfuerzo:** medio  
**Ubicación:** `tools/check-release-evidence.mjs:27-42`,
`tools/check-release-evidence.mjs:50-65`,
`tools/capture-release-evidence.mjs:159-180`

**Problema (hechos).** El checker elige el sumario más reciente por `mtime` y
solo exige coincidencia de versión más hashes con forma SHA-256. La captura no
guarda commit, rama ni estado dirty:

```js
candidates.sort((left, right) => right.mtime - left.mtime);
if (summary.projectVersion !== packageManifest.version) { /* falla */ }
```

La evidencia aprobada del 2026-08-25 seguía en `0.57.0` mientras el repositorio
acumuló 32 commits sin bump. Al generar evidencia fresca del commit auditado, el
check falló por cambio del hash desktop
`aa2c8b... -> d62180...`.

**Impacto.** Una evidencia antigua puede atribuirse al código actual, o un build
correcto quedar sin baseline válido. Ninguna de las dos señales permite promover
un release con trazabilidad.

**Solución propuesta.** Capturar `HEAD`, dirty state, hashes de lockfiles y
configuración; exigir árbol limpio y el mismo commit entre build, captura,
baseline y paquete. Aprobar el baseline fresco solo tras revisión visual humana.

**Trazabilidad:** `T5-06`.

#### F-06 — La distribución pública conserva bloqueos legales y operativos conocidos

**Severidad:** Alta para publicación pública · **Clasificación:** conocido  
**Esfuerzo:** alto · **Requiere revisión legal:** sí

**Ubicación:** `THIRD_PARTY_NOTICES.md:3-6`,
`tools/generate-third-party-notices.ps1:94-118`, `ROADMAP.md:911-968`

**Problema (hechos).** El inventario contiene 1,009 filas, 349 licencias
`UNKNOWN` y 65 filas completas duplicadas. El generador llama `$UniqueRows` a un
simple `Sort-Object` y su `-Check` solo compara igualdad:

```powershell
$UniqueRows = @($Rows | Sort-Object ecosystem, name, version, source)
Write-Host "Third-party notices aprobados: $($UniqueRows.Count) dependencias."
```

El roadmap ya mantiene abiertas la prueba en VM/usuario no privilegiado, el
updater autenticado y el orquestador `tools/release.ps1`. El `Package` aprobado
solo demuestra que la máquina de desarrollo produjo dos instaladores, no que se
instalen, desinstalen, actualicen o superen SmartScreen correctamente.

**Impacto.** Publicar ahora expone al proyecto a atribuciones incompletas, una
experiencia de instalación no ejercida y un proceso de promoción no reproducible.

**Solución propuesta.** Resolver identidades/licencias y textos de atribución,
deduplicar, hacer fallar el gate ante `UNKNOWN` no exceptuado y cerrar en orden
I5 (VM), I6 (updater o decisión explícita de no tenerlo) e I7 (release limpio).

**Trazabilidad:** `T5-20`; pendientes existentes Fases I5, I6 e I7.

#### F-07 — El caso `zoom-125` no prueba zoom real de texto o reflow

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `tools/capture-accessibility-evidence.mjs:17-35`,
`tools/capture-accessibility-evidence.mjs:145-149`,
`tools/capture-release-evidence.mjs:113-128`

**Problema (hecho).** El caso solo cambia `deviceScaleFactor` a `1.25`; mantiene
el mismo viewport CSS. En la evidencia release fresca, desktop y `zoom-125`
produjeron exactamente el mismo hash `d6218080...`.

```js
viewport: { width: 1280, height: 900 },
deviceScaleFactor: 1.25,
```

**Impacto.** Se valida densidad de raster, no aumento de texto/UI ni reflow; una
barrera de accesibilidad a 125%/200% podría pasar inadvertida.

**Solución propuesta.** Aplicar zoom de página real o emular el viewport CSS
resultante, afirmar dimensiones/overflow y añadir un caso 200% conforme a la
estrategia WCAG. Conservar lector de pantalla y High Contrast manual como
pendientes explícitos.

**Trazabilidad:** `T5-07`.

#### F-08 — Playwright local permite `.only` y reutiliza servidores ajenos

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `playwright.config.ts:5-7`, `playwright.config.ts:20-24`

**Problema (hecho).** `forbidOnly` se activa únicamente si existe `CI`, pero el
proyecto decidió no usar CI; además `reuseExistingServer: true` puede probar un
preview iniciado con otro árbol. No hay `.only` actualmente.

**Impacto.** Un desarrollador puede ejecutar involuntariamente un subconjunto o
una build distinta y conservar un resultado local verde.

**Solución propuesta.** Prohibir `.only` siempre en scripts de gate, usar puerto
aislado/proceso propio y registrar el hash del bundle servido.

**Trazabilidad:** `T5-14`.

### Seguridad

#### F-09 — Un manifiesto batch puede sobrescribir destinos fuera de su carpeta

**Severidad:** Media · **Clasificación:** nuevo · **Confianza:** 9/10  
**Esfuerzo:** medio · **STRIDE:** Tampering

**Ubicación:** `src-tauri/src/automation.rs:1689-1696`,
`src-tauri/src/automation.rs:1768-1796`, `src-tauri/src/dataset.rs:11379-11387`

**Problema (hecho verificado).** Se aceptan rutas absolutas y rutas relativas con
`..`; la salida solo se restringe por extensión y luego se publica reemplazando
el destino:

```rust
if path.is_absolute() { path.to_owned() } else { base.join(path) }
// ...
temporary.persist(&destination)
```

Un manifiesto no confiable puede usar, por ejemplo, `../../package.json` con
formato JSON y reemplazar un archivo accesible por la cuenta. No permite escoger
bytes arbitrarios, pero sí alterar o destruir archivos de extensiones soportadas.

**Impacto.** Daño local al ejecutar un manifiesto descargado o compartido. La
cuenta local sigue siendo la autoridad; no es escalada de privilegios.

**Solución propuesta.** Rechazar absolutas y `..` por defecto, confinar outputs a
la carpeta del manifiesto o `--output-root`, y exigir opt-in/`--force` para un
destino externo o existente.

**Trazabilidad:** `T5-08`.

#### F-10 — Dos advisories se ignoran con una premisa de features falsa

**Severidad:** Media · **Clasificación:** regresión de control · **Confianza:** 10/10  
**Esfuerzo:** medio

**Ubicación:** `src-tauri/deny.toml:5-12`,
`docs/reference/dependency-audit.md:65-75`

**Problema (hecho).** Las excepciones RUSTSEC-2026-0194/0195 dicen que cloud no
está habilitado. `cargo tree -e features -i quick-xml@0.39.4` demostró que
`object_store/cloud` sí se compila a través de Polars.

```toml
# El feature cloud no está habilitado en Columnia
{ id = "RUSTSEC-2026-0194", reason = "... cloud feature disabled ..." },
```

**Pendiente de verificación.** No se encontró una ruta de aplicación que use
cloud/HTTP o procese ese XML; no se afirma explotabilidad alcanzable.

**Impacto.** El gate SCA aprueba una excepción cuya justificación factual es
incorrecta, debilitando futuras decisiones de riesgo.

**Solución propuesta.** Desactivar realmente el feature, actualizar el grafo o
documentar y probar la no alcanzabilidad; corregir la excepción mientras exista.

**Trazabilidad:** `T5-09`.

#### F-11 — Inventario y contrato IPC quedaron por detrás de la superficie real

**Severidad:** Media · **Clasificación:** regresión · **Esfuerzo:** alto  
**Ubicación:** `THREAT_MODEL.md:67-76`, `src-tauri/src/lib.rs:154-214`,
`src/ipc-contract.test.ts:481-536`, `src/ipc-contract.test.ts:278-305`,
`src/ipc-contract.test.ts:381-383`

**Problema (hechos).** El threat model afirma 25 comandos; `generate_handler!`
registra 52 de producción, excluidos cuatro probes debug. El gate de estructuras
usa una lista manual que omite comparaciones, conflictos, histogramas, temporal,
remoción de columnas y migraciones. Además normaliza enums a `string` y el
literal TypeScript `version: 1` a `number`, igualándolo con Rust `u32`.

**Impacto.** No se verifican exhaustivamente las formas que cruzan la principal
frontera privilegiada; una deriva compatible con la normalización puede pasar.

**Solución propuesta.** Generar inventario/tipos desde una fuente canónica o
fallar ante cualquier estructura compartida no clasificada; probar literales y
versiones discriminantes de forma exacta; derivar la cifra del threat model.

**Trazabilidad:** `T5-10`.

**Fortalezas de seguridad revisadas sin hallazgo:** `main` solo posee
`core:default`; no hay filesystem/shell/HTTP/opener en frontend, ni
`dangerouslySetInnerHTML`, `eval` o ejecución de procesos en producto. Las rutas
de proyecto usan contención/reparse checks, el SQL de catálogo está parametrizado,
la consulta de dataset es acotada, las exportaciones son atómicas y la CLI
sanitiza respuestas. `npm audit` reportó cero vulnerabilidades. No se encontraron
secretos reales actuales o históricos; como defensa adicional, conviene ignorar
`.env*` y adoptar un escáner de entropía, pero no se registró una exposición.

### Arquitectura, código y refactorización

#### F-12 — Un fallo transitorio de migración queda cacheado hasta reiniciar

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `src-tauri/src/projects.rs:159-175`,
`src-tauri/src/projects.rs:178-187`

**Problema (hecho).** `OnceLock` almacena el `Result` completo:

```rust
self.initialized.get_or_init(|| self.migrate()).clone()
```

Si la primera apertura falla temporalmente por lock, WAL, disco o antivirus, el
`Err` se reutiliza en todas las operaciones aunque la condición desaparezca.

**Impacto.** El catálogo permanece inutilizable hasta reiniciar la aplicación.

**Solución propuesta.** Cachear solo el éxito o usar una máquina de estados
reintentable y segura; añadir una prueba “falla una vez → corrige → reintenta”.

**Trazabilidad:** `T5-11`.

#### F-13 — Un cierre abrupto puede dejar generaciones de proyecto huérfanas

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** medio  
**Ubicación:** `src-tauri/src/projects.rs:347-365`,
`src-tauri/src/projects.rs:380-440`, `src-tauri/src/projects.rs:191-264`

**Problema (hecho).** La generación Parquet se escribe antes de la transacción
SQLite. Los errores controlados limpian, pero un crash entre ambas operaciones
deja un directorio no referenciado. La migración no reconcilia huérfanos y la
limpieza de la generación anterior ignora errores.

```rust
write_generation(&active.frame, &active.history, &generation_path)?;
let transaction = connection.transaction_with_behavior(...)?;
```

**Impacto.** Crecimiento silencioso de disco y posible confusión durante soporte
o recuperación; no se observó corrupción de un proyecto referenciado.

**Solución propuesta.** Reconciliar al iniciar bajo lock: catálogo como fuente de
verdad, edad mínima para evitar carreras, log sanitizado y prueba de crash
inyectado entre publicación y commit.

**Trazabilidad:** `T5-12`.

#### F-14 — El dominio sigue concentrado y conserva copias completas

**Severidad:** Media · **Clasificación:** conocido, agravado · **Esfuerzo:** alto  
**Ubicación:** `src-tauri/src/dataset.rs:1252-1263`,
`src-tauri/src/dataset.rs:15648-15653`,
`src-tauri/src/dataset.rs:16577-16592`, `src/App.tsx:100-104`

**Problema (hechos).** `dataset.rs` tiene 22,745 líneas, de las cuales unas
16,766 son producción antes de `mod tests`; `App.tsx` tiene 867. Entre los
puntos de mayor complejidad están `validate_quality_rule_definition` (~1,140
líneas), migración de calidad (~646) y el fallback eager (~321). Este último
empieza con `let mut candidate = source.clone()`; historial y guardado también
clonan frames.

**Impacto.** Alto radio de cambio, revisiones costosas y picos de memoria en
rutas que justamente fallan su presupuesto.

**Solución propuesta.** Extraer primero límites estables (quality, recipe,
export, persistence), dividir validadores por tipo de regla y perfilar antes de
eliminar clones. Mantener la fachada y tests de contrato para hacer el cambio
incremental.

**Trazabilidad:** `T5-13`.

### Accesibilidad, responsive y UI/UX

Además de F-07, la revisión visual no encontró defectos reproducibles de
overflow, jerarquía, targets, foco, landmarks, tema oscuro o `forced-colors` en
desktop 1280×900 y móvil 390×844. El skip link se verificó visible y enfocado;
los cuatro estados release se inspeccionaron manualmente. No se crea un hallazgo
sin evidencia. Siguen fuera de cobertura lector de pantalla real, hardware High
Contrast y zoom/reflow corregido.

### Documentación, DevOps y configuración

#### F-15 — La documentación viva y su gate han derivado

**Severidad:** Media · **Clasificación:** regresión · **Esfuerzo:** medio  
**Ubicación:** `tools/check-documentation.mjs:6-18`,
`docs/reference/dependency-audit.md:3-6`,
`docs/reference/dependency-audit.md:65-87`, `CONTEXTO.md:99`,
`CONTEXTO.md:114`,
`CHANGELOG.md:3-7`

**Problema (hechos).** El gate excluye `ROADMAP.md` y `CONTEXTO.md`, aunque
imprime que enlaces/versiones están aprobados. La auditoría de dependencias sigue
en `0.49.0` y 1,001 notices, frente a `0.57.0` y 1,009. Al iniciar esta auditoría,
el `CONTEXTO.md` de `137520b` describía `App.tsx` con ~504 líneas y `dataset.rs`
con ~7,983; la entrega requerida ya corrigió esas dos celdas a 867 y 22,745. El
CHANGELOG no recogía de forma completa 32 commits posteriores a su última
actualización y solo recibió aquí un resumen parcial.

```js
const markdownRoots = ["README.md", "CONTRIBUTING.md", "CHANGELOG.md", "docs"];
```

**Impacto.** Sin completar la tarea, una persona o agente todavía puede tomar
decisiones desde dependencias y notas de release antiguas mientras el gate
permanece verde. La métrica básica de tamaño ya quedó corregida documentalmente.

**Solución propuesta.** Incluir las fuentes vivas en el gate, generar métricas
estructurales, refrescar el snapshot SCA y reconstruir `[Unreleased]` desde los
commits/rutas verificadas. La entrega de esta auditoría corrige la ficha básica,
pero no sustituye la reconstrucción completa.

**Trazabilidad:** `T5-15`, `T5-16`.

#### F-16 — El entorno de release no está fijado y la configuración se contradice

**Severidad:** Media · **Clasificación:** nuevo/deriva · **Esfuerzo:** medio  
**Ubicación:** `README.md:51-55`, `README.md:287-289`, `package.json:1-7`,
`tools/check-supply-chain.ps1:49-55`, `src-tauri/tauri.conf.json:49-55`,
`ROADMAP.md:242-253`

**Problema (hechos).** Se pide Node LTS y Rust stable sin `engines`,
`packageManager` ni `rust-toolchain.toml`; los reportes registran versiones, pero
no las exigen. README afirma que Release no usa servicios externos, aunque ejecuta
`npm audit`. El roadmap dice no habilitar todos los targets mientras solo Windows
esté verificado, pero Tauri configura `"targets": "all"`.

**Matiz.** En Tauri, `all` produce los bundles aplicables a la plataforma de
build, no compila mágicamente macOS/Linux en Windows; aun así contradice la
decisión escrita y generó MSI además del NSIS recomendado.

**Impacto.** Builds diferentes bajo el mismo commit, expectativas offline
incorrectas y artefactos no alineados con la política de distribución.

**Solución propuesta.** Fijar y comprobar toolchains, documentar qué pasos
requieren red y seleccionar explícitamente targets por plataforma/canal.

**Trazabilidad:** `T5-17`.

Los perfiles locales, evidencia con hashes, SBOM CycloneDX, lockfiles y bundles
son puntos fuertes. `Release` y `Package` aprobaron en la estación auditada. Eso
no elimina los pendientes ya reconocidos de I5/I6/I7 ni autoriza publicación.

### Legal y cumplimiento

#### F-17 — Licencia, privacidad y retención no son descubribles para el usuario

**Severidad:** Media · **Clasificación:** nuevo · **Esfuerzo:** medio  
**Requiere revisión legal:** sí  
**Ubicación:** `src-tauri/tauri.conf.json:52-55`, `src/App.tsx:667-676`,
`README.md:31-33`, `README.md:62-81`, `THREAT_MODEL.md:75-76`

**Problema (hechos).** LICENSE y notices se empaquetan como resources, pero la UI
solo ofrece monitor y tema en “Preferencias y recursos”. No existe un aviso único
orientado al usuario que explique metadatos recientes, SQLite/Parquet, historial,
ubicación, retención, eliminación/desinstalación, cifrado o contacto responsable.

**Impacto.** Quien instala el binario no tiene una ruta accesible para comprender
atribuciones y tratamiento local. Las obligaciones exactas dependen del canal,
entidad y jurisdicción, hoy no declarados.

**Solución propuesta.** Añadir “Acerca de / Legal / Privacidad” accesible y una
política breve local-first; definir responsable/contacto, retención y borrado;
validarla jurídicamente antes de una distribución pública.

**Trazabilidad:** `T5-18`.

No aplican cookies, banner de consentimiento, cuentas, pagos ni transferencias de
datos a servicios de Columnia según el código actual. Eso deberá reauditarse si
se incorporan updater, telemetría, conectores o un sitio público.

### Ortografía y redacción

#### F-18 — Hay inconsistencias editoriales en documentación visible

**Severidad:** Baja · **Clasificación:** nuevo · **Esfuerzo:** bajo  
**Ubicación:** `docs/how-to/validate-release-evidence.md:1-10`,
`THIRD_PARTY_NOTICES.md:1-6`,
`tools/generate-third-party-notices.ps1:96-101`, `CONTEXTO.md:563-584`

**Problema (hechos).** Se mezclan “How to”, “Prerequisitos” y español; el notice
generado omite tildes en “índice”, “según” y “versión”. `CONTEXTO.md` pide evitar
un changelog y acto seguido mantiene un registro de más de cien entradas.

**Impacto.** Menor acabado profesional, especialmente en documentación legal, y
mayor costo para retomar el proyecto.

**Solución propuesta.** Corregir la plantilla generadora, normalizar títulos y
compactar el registro histórico hacia CHANGELOG/ADR conservando solo decisiones
que cambien trabajo futuro.

**Trazabilidad:** `T5-19`.

## 5. Plan de acción priorizado

### Quick wins — menos de un día por tarea

1. `T5-05`: separar `SkipPackage` de los gates Rust/Release.
2. `T5-03`: hacer explícito el fallo/estado del presupuesto de primer render.
3. `T5-07`: sustituir densidad de píxel por zoom/reflow real.
4. `T5-11`: cachear solo el éxito de la migración de proyectos.
5. `T5-14`: prohibir `.only` y aislar el servidor E2E.
6. `T5-19`: corregir redacción desde las plantillas generadoras.

### Corto plazo — 1 a 2 semanas

1. `T5-01` y `T5-02`: aislar la regresión durable/memoria y recuperar el gate.
2. `T5-04`: activar cobertura real por capas críticas.
3. `T5-06`: ligar evidencia de release a un `HEAD` limpio y aprobar baseline.
4. `T5-08` y `T5-09`: confinar batch y corregir excepciones SCA.
5. `T5-12`: reconciliar generaciones huérfanas.
6. `T5-15`–`T5-18`: reparar trazabilidad documental, toolchains y privacidad.

### Medio plazo — 1 a 3 meses

1. `T5-10` y `T5-13`: generar contratos IPC y modularizar el dominio sin romper
   la fachada.
2. `T5-20`: completar licencias/atribuciones con revisión legal.
3. Cerrar I5 con instalación/desinstalación en VM y usuario no privilegiado.
4. Decidir y cerrar I6; después implementar I7 como única ruta de publicación.
5. Validar lector de pantalla, High Contrast y plataformas no Windows antes de
   declarar soporte.

## 6. Puntos fuertes que deben conservarse

- Autoridad de filesystem/datos en Rust y frontend sin rutas locales.
- Capability Tauri mínima, CSP estrecha y ausencia de red/telemetría de producto.
- Validación fail-closed de formatos, hojas, contratos, proyectos y calidad.
- SQL parametrizado para catálogo y parser SQL de dataset restringido.
- Publicación atómica con temporales, `sync_all` y cleanup comprobado.
- Sanitización recursiva de JSON CLI y evidencias sin filas, muestras o secretos.
- Neutralización de fórmulas CSV y escape de formatos de hoja.
- Tests unitarios, integración, E2E, WebView2 nativo y empaquetado local reales.
- Bundle total de 548,225 B raw/139,605 B gzip, dentro del presupuesto.
- Tema claro/oscuro/sistema, foco visible, reduced motion, landmarks y
  `forced-colors` bien cubiertos.
- Decisiones honestas ya registradas sobre no CI, no Authenticode de pago y
  plataformas todavía no verificadas.

## 7. Zonas no cubiertas o pendientes de verificación

- Causa raíz exacta del fallo `project-inspect` dentro del benchmark.
- Datasets reales/adversariales y formatos comprimidos patológicos.
- Lector de pantalla real, Windows High Contrast en hardware y zoom 200%.
- Instalación/desinstalación en VM limpia, usuario sin privilegios, rutas Unicode,
  SmartScreen y tiempo hasta ventana utilizable.
- Updater, firma, downgrade, manipulación, publicación y descarga posterior.
- macOS y Linux.
- Jurisdicción, entidad responsable, mercados, marca “Columnia”, canal de
  distribución y obligaciones de consumidor/privacidad.
- Escáner completo de entropía/historia de secretos; se hicieron búsquedas de alta
  señal y no se encontraron secretos reales.
- Flakiness estadística: las suites se ejecutaron en esta estación, no en una
  matriz repetida de hardware.

## 8. Trazabilidad informe → roadmap

| Hallazgo | Estado | Tarea |
| --- | --- | --- |
| F-01 | Regresión | `T5-01`, `T5-02` |
| F-02 | Nuevo | `T5-03` |
| F-03 | Regresión | `T5-04` |
| F-04 | Nuevo | `T5-05` |
| F-05 | Nuevo | `T5-06` |
| F-06 | Conocido | `T5-20`, Fases I5/I6/I7 existentes |
| F-07 | Nuevo | `T5-07` |
| F-08 | Nuevo | `T5-14` |
| F-09 | Nuevo | `T5-08` |
| F-10 | Regresión | `T5-09` |
| F-11 | Regresión | `T5-10` |
| F-12 | Nuevo | `T5-11` |
| F-13 | Nuevo | `T5-12` |
| F-14 | Conocido, agravado | `T5-13` |
| F-15 | Regresión | `T5-15`, `T5-16` |
| F-16 | Nuevo/deriva | `T5-17` |
| F-17 | Nuevo | `T5-18` |
| F-18 | Nuevo | `T5-19` |

## 9. Addendum de implementación — 2026-08-28

Este addendum conserva el veredicto histórico sobre `137520b` y registra el
estado del trabajo realizado después de la auditoría. No convierte por sí solo
el prototipo en un release público.

### Hallazgos con control técnico implementado

- **F-01/T5-02:** el recorrido nativo aislado verifica los cuatro diálogos
  Win32 (abrir dataset, guardar/cargar receta y exportar), sin campos de ruta
  expuestos, con cleanup confirmado y dentro de los límites de 512 MiB working
  set / 256 MiB privados. La corrida oficial registra 521,785,344 bytes de
  working set y 265,981,952 bytes privados. El driver restringe ventanas al PID/owner de Columnia,
  espera el cierre modal entre comandos y termina de forma controlada un driver
  bloqueado. Evidencia: `.local/validation/webview2-cdp/20260828T203917Z/summary.json`.
- **F-02/T5-03:** el probe Playwright separa estado funcional y presupuesto de
  primer render; el estado compuesto falla si falta cualquiera de los dos. La
  métrica nativa usa el delta entre `columnia:app-bootstrap` y
  `columnia:app-render` para excluir compilación fría de Vite sin quitar el
  presupuesto de 3 s.
- **F-03/T5-04:** `npm run test:coverage` aplica umbrales por archivo para App,
  Entrega, Preparar y `usePrepareController`; la corrida actual tiene 267 tests
  y cumple los umbrales críticos.
- **F-04/T5-05:** `-SkipPackage` solo evita bundling MSI/NSIS; conserva los
  checks Rust, cobertura, supply chain y Release.
- **F-05/T5-06:** captura y validación release registran commit, rama, árbol
  limpio y hashes de `package-lock.json`/`Cargo.lock`; cualquier árbol sucio,
  commit distinto o baseline desactualizado falla.
- **F-07/T5-07:** 125% y 200% aplican zoom CSS real con contratos de overflow,
  foco y contenido visible; `deviceScaleFactor` ya no se usa como sustituto.
  La captura local renovada y su baseline pasan en
  `.local/validation/accessibility-visual/20260828T185137Z`.
- **F-09/T5-08:** batch rechaza por defecto traversal, absolutas, destinos fuera
  del root y archivos existentes; `--force` es opt-in y conserva comprobaciones
  contra colisiones con inputs, recetas y manifiesto.
- **F-10/T5-09:** la excepción `quick-xml` documenta su ruta transitiva real
  por `object_store` y la ausencia de features cloud en Columnia.
- **F-11/T5-10:** el inventario generado conserva 56 comandos de producción,
  4 debug y 56 estructuras; los tests de contrato usan ese inventario.
- **F-12/T5-11 y F-13/T5-12:** la inicialización solo cachea éxitos y la
  apertura reconcilia generaciones válidas antiguas no referenciadas con margen
  de una hora, sin tocar activas ni staging.
- **F-16/T5-17:** Node 24.14.0, npm 11.10.1 y Rust/Cargo 1.97.1 están fijados
  y los bundles Tauri se limitan a MSI y NSIS.
- **T5-15/T5-16/T5-19/T5-20 técnico:** notices y auditoría de dependencias se
  regeneran offline con 960 identidades sin `UNKNOWN`, el gate documental
  incluye las fuentes vivas y la app expone MIT, notices y privacidad local.
- **I6/I7 técnico:** `tauri-plugin-updater` ya está integrado con consulta
  explícita, progreso/cancelación, instalación verificada y clave pública
  embebida; Rust exige que la versión semver sea estrictamente posterior y
  rechaza entradas inválidas. Un Package firmado real produjo MSI/NSIS y `.sig`; el manifiesto
  estático y el inventario SHA-256 pasan sus gates. El contrato reproducible del
  gate también confirma fallo cerrado ante artefacto truncado, firma alterada,
  manifiesto incompleto/corrupto y URL HTTP. Esto no sustituye el canal real ni
  la verificación criptográfica contra assets publicados. La clave privada vive
  fuera del repositorio y el flujo se activa solo con variables de entorno.

### Validaciones todavía abiertas

- **F-01/T5-01:** la corrida formal final del binario modular registra
  `project-save` en 52.09 s y tres actualizaciones durables en 56.37–57.31 s
  sobre 100 MiB; reapertura, exportación y cleanup también fueron confirmados.
  Evidencia: `.local/validation/performance-benchmark/20260828T184531Z/summary.json`.
- **F-14/T5-13:** se extrajo `dataset_fingerprints.rs` como primer límite
  modular del motor de datos, con API `pub(crate)` acotada y los 244 tests Rust
  verdes. El módulo contiene 150 líneas y `dataset.rs` queda 119 líneas por
  debajo de `HEAD`; las siguientes fronteras de quality/recipe/export/persistence
  siguen siendo incrementales.
- **F-06/T5-18/T5-20:** el orquestador local `tools/release.ps1` ya existe y
  ejecuta los gates sin publicar, etiquetar ni contactar servicios remotos.
  Siguen pendientes la VM limpia, el canal real de updater, la rotación de
  claves y las decisiones externas sobre responsable, jurisdicción, contacto,
  mercados, retención y canal legal final.
