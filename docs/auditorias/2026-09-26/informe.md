# Re-auditoría de Columnia — 2026-09-26

Commit `5a35244` (`master`, árbol limpio) · Columnia 1.26.0 · Windows 11 x64.
Primera revisión desde el cierre del Tier 10 (2026-09-25). Parte del código y de la app corriendo, no de las auditorías archivadas. Hallazgos detallados en [`hallazgos.md`](hallazgos.md), medidas en [`linea-base.md`](linea-base.md), evidencias en [`capturas/`](capturas/).

## 0. Alcance acordado

| | |
|---|---|
| **Áreas incluidas** | 1 Corrección funcional · 2 Código · 3 Seguridad · 4 Datos y persistencia · 8 UI/UX · 10 QA y testing |
| **Excluidas** | 5 Rendimiento, 7 Accesibilidad (RV09 exige NVDA con una persona), 9 Arquitectura (monolito por decisión), 11 Refactorización, 12 Redacción, 13 Documentación, 14 DevOps (distribución pendiente de RV11), 15 Legal (solo se publica el código). No aplican: 6 SEO (escritorio) y 16 IA (no integra modelos) |
| **Profundidad** | Exhaustiva en los flujos críticos; el resto por muestreo |
| **Flujos críticos** | F1 cargar CSV/Excel · F2 Preparar (propuesta de un clic) · F3 deshacer/rehacer · F4 guardar, reabrir y recuperar · F5 Entregar (archivo y ODBC) |
| **Contexto** | Prototipo local, una persona, sin red salvo ODBC opcional, República Dominicana. Decisiones cerradas respetadas: sin CI, sin modularizar `dataset.rs`, sin telemetría, recuperación explícita, UTF-8 estricto (no se propone conversión automática) |
| **Muestreo** | Leído entero: la propuesta de Preparar, historial, exportación (CSV/XLSX/fórmulas), entrega ODBC, persistencia de proyectos (orden transacción/archivos), informe de fallos, configuración Tauri/CSP/capacidades, inventario de comandos IPC. Por muestra: resto de `dataset.rs` y del frontend. No abierto: CLI, tareas reutilizables, comparación, DuckDB, updater |
| **Datos** | Copias locales no versionadas de datasets de trabajo (OnlineRetail, dataset público de UCI: 541.909 filas, vmCloud 2.000.000 filas, HR), más sintéticos generados: hostil (20.000 filas), CR-only, Windows-1252, caracteres de control, 1,1 M filas |

## 1. Resumen ejecutivo

Columnia está técnicamente muy sólido: el perfil Full pasa entero (1.041 tests, Clippy y lint sin avisos), la superficie de seguridad está bien cerrada (CSP estricta, permisos mínimos, ninguna ruta por IPC, secretos enmascarados, nada en el historial de git) y la persistencia está bien diseñada (WAL, transacciones, publicación atómica, recuperación). La entrega ODBC funciona de punta a punta contra SQL Server real con unicode, emojis y textos largos.

Los problemas están en otra capa, la que ningún test mira: **lo que el producto hace con datos reales en el camino recomendado de un clic**. Con archivos del propio escritorio del dueño, la propuesta por defecto inventa datos, los recuentos no cuadran con lo que se aplica, la exportación estropea números y produce Excel que no abre, y dos formatos muy comunes ni siquiera cargan.

**Los 5 problemas más graves:**

1. **[FUN-03] Crítico** — La propuesta por defecto rellena identificadores, nombres y fechas con la moda: 135.037 compras pasan a un solo cliente (del 0,4 % al 15,3 % de los ingresos); 2.222 personas pasan a llamarse «José Ñúñez».
2. **[FUN-04] Alto** — Exportar a CSV pone un apóstrofo a todos los números negativos guardados como texto (10.587 cantidades en OnlineRetail).
3. **[FUN-05] Alto** — La exportación a Excel genera archivos que Excel no abre (caracteres de control, más de 1.048.576 filas) y la app los da por buenos.
4. **[FUN-06] Alto** — Lo que anuncia la propuesta de Preparar no es lo que aplica (7.500 rellenos donde se anunciaban 2.500; una columna anunciada no se toca).
5. **[FUN-02] Alto** — Un CSV guardado por Excel en español (Windows-1252) no se puede abrir y la app no ofrece salida.

Hay un patrón de fondo, **[QA-02]**: la suite verifica piezas, no invariantes de producto, y por eso los 1.041 tests pasan sobre estos fallos. Es la misma lección que dejó el pánico de Polars.

## 2. Línea base

| Medida | 2026-09-26 |
|---|---|
| Perfil Full | Aprobado (491 s) |
| Tests | 493 frontend · 521 Rust (+7 ignorados) · 27 E2E — 0 fallidos |
| Cobertura | Frontend 86,9 % sentencias / 82,1 % ramas · Rust 70,5 % líneas (`remote_databases` 52,7 %) |
| Lint / Clippy | 0 / 0 |
| Vulnerabilidades | npm 0 · cargo 2 con excepción verificada no alcanzable |
| Secretos | 0 en árbol (641 archivos) y en historial (652 commits) |
| Bundle | 803.409 B raw (98,1 % del límite) · 196.897 B gzip (80,1 %) |
| Tests ODBC ignorados ejecutados contra SQL Server local | 3 de 3 aprobados (ida y vuelta, timeout, lotes 21.316 filas/s) |

La auditoría anterior no guardó una línea base comparable (solo cifras sueltas en `CONTEXTO.md`); esta es la primera.

## 3. Puntuación

Rúbrica de la skill: un Crítico abierto limita la nota a 3; un Alto, a 6; solo Medios (< 4), a 8; solo Bajos, a 9. Cuenta el área principal de cada hallazgo.

| Área | Nota | Cómo se comprobó | Justificación |
|---|---|---|---|
| 1 Corrección funcional | **3** | App corriendo + código | FUN-03 crítico en el flujo de un clic; FUN-01/02/04/05/06 altos; FUN-07 medio |
| 2 Código | **8** | Código (muestreo) + app | Calidad alta (Clippy `-D warnings`, lint limpio); CODE-01 medio (informe de fallos inservible) |
| 3 Seguridad | **10** | Código + app + ODBC real | Superficie mínima y correcta; ningún hallazgo propio (FUN-04 toca la neutralización pero es de corrección) |
| 4 Datos y persistencia | **8** | App corriendo (cierre forzado, recuperación) + código | Diseño transaccional correcto; DAT-01 medio (pérdida silenciosa tras cierre) |
| 8 UI/UX | **9** | App corriendo (1280×800, 900×600, 1366×768) | La dirección «una propuesta, una acción» está bien lograda; UX-01 agrupa 10 detalles bajos. Los mensajes engañosos de carga están en FUN-01/02 |
| 10 QA y testing | **6** | Suite ejecutada + comparación con fallos reales | QA-02 alto; QA-01 bajo |

## 4. Hallazgos

Todos son **nuevos**: no aparecen como tareas en el roadmap archivado ni en el vigente. Ninguno es regresión de una tarea cerrada, salvo la **conexión de FUN-03 y FUN-06 con T10 «Preparar de un clic»** (`54f8bf6`, 2026-09-24), que introdujo la propuesta combinada. Detalle completo, con evidencia, reproducción y criterio de aceptación, en [`hallazgos.md`](hallazgos.md).

| ID | Severidad | Título |
|---|---|---|
| FUN-03 | Crítico | La propuesta por defecto rellena identificadores con la moda y falsea el dataset |
| FUN-01 | Alto | Un CSV con fin de línea solo CR no se puede cargar y el error culpa al tamaño |
| FUN-02 | Alto | Un CSV de Excel en español (Windows-1252) no se puede abrir y no hay salida |
| FUN-04 | Alto | Exportar a CSV antepone un apóstrofo a todos los negativos guardados como texto |
| FUN-05 | Alto | La exportación a Excel produce archivos que Excel no abre, sin avisar |
| FUN-06 | Alto | Lo que anuncia la propuesta no es lo que aplica |
| QA-02 | Alto | 1.041 tests en verde y ninguno detecta los fallos funcionales |
| FUN-07 | Medio | El flujo por defecto nunca tipa columnas numéricas (causa raíz de FUN-04) |
| DAT-01 | Medio | Tras un cierre inesperado se pierden en silencio los cambios no guardados |
| CODE-01 | Medio | El informe de fallo guarda solo `mod.rs:2136` |
| UX-01 | Bajo | Diez detalles de Preparar/Entregar que contradicen lo que muestran |
| QA-01 | Bajo | Los comandos manuales de tests Rust fallan sin una variable no documentada |

## 5. Fuera de alcance, pero relevante

- **Rendimiento** (excluido): `vmCloud_data.csv` (2.000.000 filas, 354 MB) tarda **313 s** en cargar y diagnosticar, con un pico de **1,36 GB** de memoria. Pandas lee el mismo archivo en 10 s. Hay progreso y cancelación visibles. Vale una medición dedicada si la beta usa archivos de ese tamaño.
- **Bundle al 98,1 % del límite raw**: el siguiente cambio de UI probablemente obligue a volver a subirlo (ya se subió el 2026-09-23).

## 6. Descartados en la refutación

- `quick-xml` vulnerable en el lector de Excel: `calamine` usa 0.41.0 (parcheado); la excepción de `deny.toml` es correcta.
- Escrituras Parquet con chunks desalineados fuera del arreglo `a1df4ad`: las no batched pasan por `ParquetWriter::finish`, que alinea (`polars-core/src/frame/chunks.rs:107`).
- Deshacer en datasets grandes que exigiera Parquet en la revisión original (`history.rs:603`): todas las entradas son copias Parquet; confirmado en vivo con 541.909 filas.
- Transacción ODBC abierta tras un error a mitad: `odbc-api` hace rollback al desconectar; «Reemplazar» solo se permite en motores con DDL transaccional.
- Contraseña ODBC en disco o en mensajes: no se persiste en presets ni proyectos; los errores la enmascaran.
- Borrado de generaciones antes de confirmar el catálogo: el orden es transacción → commit → borrado de archivos no referenciados.
- Los 3 informes de pánico existentes: anteriores (6 min) al commit que arregló los chunks.

## 7. Plan de acción

**Quick wins (< 1 día)**
- FUN-04: no neutralizar números válidos (una expresión regular y un caso de test).
- FUN-03 (contención): desmarcar por defecto la imputación y mostrar en el antes/después el valor y el recuento por columna.
- CODE-01: conservar la ruta desde el crate en el informe de fallo.
- QA-01: documentar la variable del manifiesto de tests.

**Corto plazo (1-2 semanas), antes de RV07 Beta**
- FUN-03 (completo): no imputar identificadores ni texto libre; usar «Desconocido» + columna de auditoría, que ya existe en el motor.
- FUN-06: recuentos calculados sobre la cadena completa.
- FUN-05: saneamiento XML y límites de Excel en la comprobación previa.
- FUN-01 y FUN-02: detectar CR-only y Windows-1252 y **proponer** la lectura, sin convertir automáticamente.
- QA-02: batería de invariantes de ida y vuelta con fixtures realistas.

**Medio plazo (1-3 meses)**
- FUN-07: propuesta de tipado de columnas numéricas y de fecha.
- DAT-01: aviso de cierre inesperado y autoguardado tras el primer guardado.
- UX-01: los diez detalles.

**Recomendación sobre la beta:** RV07 exige «sin P0/P1». FUN-03 y FUN-06 afectan al primer clic que hará cada participante; conviene cerrarlos antes de invitar a nadie, o la beta medirá estos fallos en lugar del producto.

## 8. Puntos fuertes (conservar)

- **Seguridad por diseño**: React sin rutas ni autoridad sobre el filesystem; CSP y capacidades mínimas con gates que lo comprueban; confirmación nativa antes de cualquier conexión remota, recordada solo durante la sesión y solo como huella.
- **Persistencia**: SQLite en WAL con `synchronous=FULL`, publicación con `persist_noclobber`, reconciliación de huérfanos y recuperación que abrió un proyecto de 541.909 filas en 4 s tras un `kill`.
- **Neutralización de fórmulas en CSV** (salvo el caso de los números) y XLSX con `inlineStr`: `=HYPERLINK`, `@SUM`, `+1+1` y `-2+3` quedaron inertes.
- **Entrega ODBC**: `NVARCHAR(MAX)`, lotes de 500 filas, timeouts, diario anti-duplicados; unicode y 5.000 caracteres intactos en SQL Server real.
- **Diagnóstico exacto**: los recuentos de vacíos, duplicados y tipos incompatibles coinciden al dígito con un cálculo independiente en los tres datasets (541.909 y 2.000.000 filas).
- **Deshacer** devuelve exactamente el estado previo; la aplicación parcial respeta la selección.
- **Detección de datos personales** antes de exportar (`nombre`).
- **Dirección de UI**: Revisar y Preparar con una propuesta y una acción, detalle plegado, antes/después visible; coherente con lo que se pidió.
- **Gates locales** reproducibles y con evidencia sanitizada, en lugar de CI.

## 9. Zonas no cubiertas

- **Excel como entrada** (`.xlsx`/`.ods`): no se cargó ninguno en vivo por tiempo; solo se revisó la ruta de `quick-xml`.
- **Datasets de 5,6 y 9 GB** (`2019-Oct.csv`, `2019-Nov.csv`): fuera del alcance (rendimiento); «sin límite fijo de tamaño» sigue sin verificar a esa escala.
- **PostgreSQL y MySQL**: sin servidores locales; solo SQL Server se probó de punta a punta.
- **NVDA, temas de contraste de Windows y escalado de texto**: fuera del alcance (RV09).
- **Build release e instalador**: se auditó `npm run tauri dev` (debug); el comportamiento del ejecutable optimizado no se ejerció.
- **CLI de proyectos, tareas reutilizables, comparación y DuckDB**: no abiertos.

## Limpieza

Base `columnia_auditoria` eliminada de SQL Server; `%APPDATA%\app.columnia.desktop` y el `localStorage` de WebView2 restaurados desde la copia previa; proceso de la app cerrado. Ver el estado final en el mensaje de cierre de la sesión.
