# Roadmap — Columnia

> Documento vivo de planificación del nuevo proyecto.
> La versión actual se define únicamente por el código y los contratos de este
> repositorio.

## Estado general

- Etapa actual: Fase I1 completa como prototipo vertical verificable, con las
  Fases I0, I4 e I8 cerradas, I3/I5 avanzadas y la Fase P1 de paridad funcional
  con JSON, SQL, comparación/consolidación por clave, joins multidataset y
  visualizaciones accesibles, resolución por columna/valor, privacidad visible
  en los destinos locales, correlaciones numéricas acotadas y contratos de calidad v3 con documento canónico
  Columnia v1; conserva opciones de entrega, cobertura temporal con calendario
  diario accesible, retiro confirmado de identificadores y proyectos locales;
  la superficie de compatibilidad externa fue retirada para mantener un contrato
  nativo y acotado;
  I3/I5 conservan validaciones externas de plataforma. En v0.153.0, la apertura
  de proyectos durables comprueba la guardia de RAM antes de leer `current.parquet`
  completo y conserva la sesión activa si la admisión falla. En v0.152.0, los fallbacks
  eager que necesitan materializar fuentes source-backed grandes pasan por una
  guardia de RAM disponible con reserva de seguridad y error accionable. En
  v0.151.0, la corrección
  source-backed de codificación repara secuencias mojibake inequívocas sin
  materializar el frame y conserva fallback eager ante valores ambiguos. En
  v0.150.0, la limpieza
  source-backed de tipos incompatibles infiere booleanos, enteros, decimales y
  fechas sobre el cursor Parquet efectivo y conserva fallback materializado
  cuando DuckDB no puede garantizar la paridad. En v0.149.0, los parseos
  ISO source-backed validan el snapshot Parquet efectivo del cursor después de
  etapas previas, evitando fallback eager por valores que ya fueron filtrados.
  En v0.148.0, la evidencia
  fresca de `perf:benchmark`, `perf:webview2` y `perf:check` aprueba el escenario
  de 100 MiB, el ciclo durable, cleanup y los presupuestos actuales. En v0.147.0,
  la entrega
  ODBC también transmite por bloques las fuentes source-backed compatibles y
  conserva fallback materializado explícito para las combinaciones no
  compatibles. En v0.146.0, la entrega
  remota compatible con PostgreSQL, MySQL y SQL Server usa ODBC con prueba de
  conexión y políticas explícitas de tabla. En v0.145.0, el gate
  de rendimiento valida también el intervalo entre proceso nativo listo y
  ventana visible desktop (1.000 ms), separándolo de la compilación debug.
  En v0.144.1, la
  plantilla de notices y la documentación del flujo de release quedan
  normalizadas en español, con el inventario regenerado desde los lockfiles.
  En v0.144.0, el
  benchmark reproducible de JOIN source-backed procesa 512 MiB desde DuckDB
  con `INNER`, `LEFT` y `FULL`, conserva conteo/paginación, deja el frame vacío
  y confirma cleanup bajo el presupuesto de working set. En v0.143.0, la
  validación de decisiones source-backed recorre conflictos por bloques y
  conserva solo índices y columnas divergentes, sin materializar los valores
  de todos los conflictos; el límite explícito sube a 8.192 y los casos fuera
  de presupuesto mantienen fallback eager. En v0.142.0, la
  resolución source-backed acotada de conflictos por clave aplica decisiones
  por fila o columna directamente en DuckDB, publica un snapshot Parquet
  reversible y mantiene el frame esquema-only; los casos fuera de 2.048
  conflictos o incompatibles conservan fallback eager. En v0.141.0, la página
  de conflictos por clave evita materializar el activo source-backed, recorre
  snapshots Parquet por bloques y conserva el frame esquema-only. En v0.140.0, la
  consolidación por claves entre un activo source-backed y el snapshot Parquet
  comparado valida duplicados/conflictos y publica solo las claves nuevas desde
  DuckDB, con orden e historial reversibles; los formatos incompatibles
  mantienen fallback eager. En v0.139.0, los
  JOINs source-backed `INNER`, `LEFT` y `FULL` entre un activo y una fuente
  CSV/TSV/TXT delimitada o Parquet ejecutan directamente en DuckDB, publican
  solo el resultado Parquet con límite de cardinalidad, orden estable e
  historial reversible, y conservan fallback eager para formatos incompatibles.
  En v0.138.0, los
  Bundles source-backed con receta activa mantienen la ejecución incremental:
  DuckDB transfiere el dataset y agrega `recipe.json` validado, su referencia y
  hash en `manifest.json` sin materializar el frame activo. En v0.137.0, la
  exportación source-backed también mantiene la ejecución incremental cuando
  se solicita protección `mask`/`hash`, generando un snapshot Parquet privado
  antes de transferir el resultado. En v0.136.0, la
  eliminación de duplicados parecidos y las correcciones recomendadas también
  usan DuckDB source-backed con snapshots reversibles. En v0.135.0, las
  acciones directas IQR de outliers (`cap`, `impute`, `drop`) también usan
  DuckDB source-backed con conteos exactos y snapshots reversibles. En v0.134.0, las
  imputaciones conservadora y categórica también usan DuckDB source-backed
  con conteos exactos y snapshots reversibles. En v0.133.0, la
  conversión numérica y la interpretación de fechas detectadas también usan
  DuckDB source-backed con las reglas eager de seguridad, conteos exactos y
  snapshots reversibles. En v0.132.0, la
  normalización de booleanos también usa DuckDB source-backed con el umbral
  eager, conteos exactos y snapshots reversibles. En v0.131.0, el
  recorte, la normalización de texto y los valores centinela también usan
  DuckDB source-backed con conteos exactos y snapshots reversibles. En
  v0.130.0, la
  normalización de nombres y la activación de `_cambios` también usan DuckDB
  source-backed con snapshots reversibles. En v0.129.0, la máscara
  de valores personales source-backed también usa DuckDB, conserva conteos
  agregados y snapshots reversibles. En v0.128.0, el retiro
  de columnas identificadoras y personales detectadas también usa DuckDB
  source-backed, snapshots Parquet reversibles y fallback eager seguro. En v0.127.0, las
  limpiezas de duplicados y columnas constantes, vacías o con alta nulidad
  también tienen ejecución source-backed con DuckDB, snapshots Parquet
  reversibles y fallback eager seguro.
- Versión actual del prototipo: `0.153.0`.
- Implementación: iniciada el 2026-08-12.
- Nombre: `Columnia`, aprobado.
- Carpeta del proyecto nuevo: `Columnia/`, creada.
- Política de validación: todo se ejecutará localmente; no habrá CI, GitHub
  Actions ni workflows automáticos.
- Política de costos: no se adoptarán certificados, servicios ni herramientas
  de pago obligatorias.
- Plataformas objetivo de diseño: Windows, macOS y Linux.
- Plataforma inicial de soporte y verificación: Windows x64; macOS y Linux
  deberán verificarse localmente en sus respectivos sistemas antes de declarar
  soporte público.
- Reauditoría profesional del 2026-08-28: no se detectaron hallazgos críticos;
  se abrió Tier 5 con 20 tareas. La implementación técnica de la mayoría de
  los gates ya está en el árbol de trabajo. El benchmark corto posterior a la
  optimización registra `project-save` en 59.75 s para 100 MiB; la corrida
  formal de tres actualizaciones ya cumple el presupuesto; la memoria nativa
  WebView2, la evidencia release desde un commit limpio y las decisiones
  legales/operativas siguen
  bloqueando la publicación. Informe: `AUDITORIA_PROFESIONAL_2026-08-28.md`.

### Índice operativo de la reauditoría

El roadmap histórico anterior a esta revisión usa fases y no IDs por Tier. Se
conserva sin reescribirlo; las tareas nuevas empiezan en Tier 5 y enlazan los
pendientes existentes cuando corresponde.

| Tanda | Estado | Tareas abiertas | Severidad | Esfuerzo agregado |
| --- | --- | ---: | --- | --- |
| Tier 5 — Integridad de gates y preparación de distribución | Implementación técnica mayormente cerrada 2026-08-28 | 20 | validaciones nativas y bloqueos legales/operativos | ver progreso detallado abajo |

Las Fases I5, I6 e I7 siguen abiertas y forman dependencias obligatorias del
release público; no se duplican como tareas nuevas.

## 1. Producto de referencia

La aplicación actual es una estación de trabajo local para preparar datos antes
de consumirlos en Power BI u otras herramientas de BI. Sus capacidades que deben
servir como referencia para el rediseño son:

- Importar CSV, Excel y Parquet.
- Perfilar esquemas, tipos, nulos, duplicados, outliers, formatos y PII.
- Limpiar, transformar y comparar datasets.
- Trabajar con archivos mayores que la memoria mediante ejecución incremental.
- Validar la calidad antes de entregar resultados.
- Exportar CSV, Excel, Parquet, JSON, scripts SQL y destinos de base de datos.
- Guardar recetas, sesiones, historial y evidencias de ejecución.
- Funcionar localmente, con privacidad y sin depender de un servidor web.
- Ofrecer interfaz de escritorio Windows y una CLI para automatización.

La nueva versión debe conservar el valor del producto, pero no copiar
automáticamente las decisiones técnicas o la estructura interna de la versión
anterior.

## 2. Nombre del producto

### Recomendación principal: Columnia

**Motivo:** combina la idea de “columnas” con un nombre de producto corto y
memorable. Describe el terreno de trabajo —datos tabulares— sin limitar el
producto a limpieza, Excel o Power BI. Es pronunciable tanto en español como en
inglés y permite una identidad propia.

- Nombre visible: `Columnia`
- Nombre técnico de la carpeta del proyecto: `Columnia`
- Posicionamiento provisional: “Prepara datos confiables, localmente”.
- Estado: **aprobado el 2026-08-12**.

### Alternativas consideradas

- `Tabularia`: expresa bien el dominio, pero es más largo y ya se utiliza en
  otros contextos editoriales.
- `Tamiz`: comunica selección y limpieza, pero limita la percepción del producto
  y ya aparece asociado a otros servicios digitales.
- `sistema anterior`: es descriptivo, pero demasiado genérico y se confunde con
  bibliotecas y productos existentes.
- `PrepDesk`: describe una herramienta de escritorio, pero ya existe como nombre
  de software comercial.

### Validación pendiente antes de publicar

La búsqueda web realizada es solamente un filtro preliminar, no una autorización
legal. Antes de distribuir el producto se debe revisar:

- Marcas registradas en los mercados donde se publicará.
- Dominio web y nombres en GitHub/redes.
- Nombre de paquete en los registros técnicos que se vayan a utilizar.

## 3. Arquitectura de escritorio recomendada

### Decisión propuesta

| Capa | Tecnología recomendada | Responsabilidad |
| --- | --- | --- |
| Shell de escritorio | Tauri 2 | Ventanas, instalador, actualizaciones, diálogo de archivos y permisos del sistema |
| Backend nativo | Rust | Casos de uso, trabajos en segundo plano, cancelación, persistencia, seguridad y coordinación |
| Motor tabular | Polars para Rust | Perfiles, limpieza y transformaciones mediante planes lazy |
| Motor analítico | DuckDB embebido | SQL local, joins/agregaciones, consultas sobre CSV/Parquet y datasets que exceden la RAM |
| Frontend | React + TypeScript + Vite | Interfaz de usuario, flujos, tablas virtualizadas, gráficos y accesibilidad |
| Contrato UI/backend | Comandos y eventos tipados de Tauri | Solicitudes, progreso, cancelación y resultados sin servidor HTTP local |
| Persistencia local | SQLite para metadatos + archivos Parquet | Proyectos, recetas e historial; caché/resultados tabulares portables |

### Backend: Rust dentro de Tauri 2

Se recomienda reemplazar Python + pywebview + PyInstaller por un backend nativo
en Rust integrado con Tauri 2.

Razones:

- Produce una aplicación de escritorio nativa sin levantar un servidor HTTP.
- Ofrece un límite explícito entre la interfaz web y las capacidades locales.
- Reduce la complejidad de distribuir un intérprete de Python y sus dependencias.
- Es apropiado para trabajos largos, paralelismo controlado, cancelación y uso de
  memoria predecible.
- Polars nació en Rust, por lo que el motor tabular puede usarse directamente sin
  cruzar un puente Python.
- DuckDB se puede ejecutar embebido en el mismo proceso para consultas analíticas
  y procesamiento fuera de memoria.

El costo principal es una curva de aprendizaje mayor que Python y un ecosistema
menos cómodo para algunas funciones estadísticas. Por eso se debe construir un
prototipo técnico antes de comprometer toda la migración.

### Frontend: React + TypeScript + Vite

Se recomienda mantener el paradigma React/TypeScript, pero crear la interfaz
nuevamente sobre contratos y componentes nuevos.

Razones:

- La aplicación tiene muchos estados, formularios, tablas virtualizadas, tareas
  largas y vistas coordinadas; React encaja bien con esa complejidad.
- La versión actual ya aporta experiencia y pruebas reutilizables a nivel de
  comportamiento, aunque no se copie su implementación.
- TypeScript permite que los mensajes enviados al backend sean verificables.
- Vite genera recursos estáticos adecuados para empaquetar dentro de Tauri; no
  hace falta Next.js ni renderizado en servidor para una aplicación local.
- El ecosistema actual cubre tablas grandes, accesibilidad, gráficos y pruebas de
  componentes.

### Regla de arquitectura

La interfaz nunca debe conocer rutas arbitrarias ni ejecutar SQL o comandos del
sistema directamente. El frontend solicita casos de uso tipados; Rust valida la
entrada, ejecuta el trabajo y emite progreso. Los permisos de Tauri deben ser
mínimos y específicos para la ventana principal.

### Qué no se recomienda como base principal

- **Electron:** sería viable, pero añade un runtime Chromium/Node completo sin
  aportar una ventaja decisiva para este producto local y orientado a datos.
- **C# + WPF/WinUI:** excelente si Windows fuera el único destino permanente,
  pero dificulta reaprovechar el conocimiento y ecosistema de la interfaz web.
- **Python como backend principal:** sigue siendo excelente para prototipar
  análisis, pero conserva el mayor dolor actual: empaquetado pesado y una frontera
  UI/backend menos fuerte. Podría existir más adelante como motor auxiliar
  opcional, no como requisito del núcleo.
- **Flutter:** ofrece UI multiplataforma sólida, pero obligaría a rehacer también
  la capa de presentación en Dart sin mejorar el motor de datos.

## 4. Referencia de infraestructura: ProcessDevKill

La segunda referencia es
[`xfiberex/ProcessDevKill`](https://github.com/xfiberex/ProcessDevKill), revisada
el 2026-08-12 sobre `main` en el commit
`e6ad1b23a6c7cb2dc6d887ad5e7cdc59992a24d6`.

ProcessDevKill es útil como referencia porque ya distribuye una aplicación
Windows con la combinación elegida para Columnia: Tauri 2, Rust, React,
TypeScript y Vite. No se toma como plantilla completa: su producto y su carga de
datos son mucho menores, y sistema anterior ya posee controles de ingeniería que allí no
existen.

### Comparación de infraestructura

| Área | `sistema anterior` | ProcessDevKill | Decisión para Columnia |
| --- | --- | --- | --- |
| Shell de escritorio | pywebview + WebView2 | Tauri 2 + WebView2 | Adoptar Tauri 2 |
| Backend | Python empaquetado con PyInstaller | Rust compilado nativamente | Adoptar Rust |
| Límite UI/backend | Fachada allowlisted y políticas de rutas | Comandos Tauri + capabilities declarativas | Combinar contratos tipados, comandos estrechos y capabilities mínimas |
| Seguridad del WebView | Assets offline y bridge restringido | CSP explícita y permisos por ventana | Añadir CSP estricta y permissions-as-code desde el primer prototipo |
| Instancia de la aplicación | No se identificó bloqueo de segunda instancia | Plugin `single-instance` | Implementar instancia única y reactivar la ventana existente |
| Empaquetado Windows | EXE one-file; MSIX local experimental | Instaladores NSIS y MSI | Publicar NSIS por usuario; evaluar MSI para empresas |
| Instalación sin administrador | EXE portable | NSIS `currentUser` | Ofrecer instalación por usuario sin UAC |
| Actualizaciones desde la app | Hay ejemplos de manifiesto, pero no flujo runtime completo | Consulta releases, muestra notas, descarga con progreso y verifica SHA-256 | Implementado con `tauri-plugin-updater`, UI explícita, progreso/cancelación y SHA-256 adicional |
| Autenticidad de actualización | SHA-256 externo, sin firma | Instalador y SHA-256 en el mismo release; reconoce que no autentica al editor | Firma minisign de Tauri integrada; la firma no sustituye Authenticode ni elimina SmartScreen |
| Automatización de release | Build, verificadores, smoke y checklist separados | `release.ps1` prueba, sincroniza versiones, compila, genera hashes, etiqueta y crea GitHub Release; admite dry-run | Crear un orquestador reproducible con dry-run, pero sin permitir saltar gates en releases oficiales |
| Versionado | `pyproject.toml` y metadatos Windows | Versión sincronizada en npm, Cargo y Tauri | Definir una fuente de verdad y verificar que los manifiestos coincidan |
| Automatización de pruebas | Matriz completa en GitHub Actions y herramientas locales | Solo pruebas y release locales | Ejecutar todos los gates localmente mediante scripts reproducibles; sin CI ni workflows |
| Calidad backend | pytest, Ruff, mypy y cobertura mínima | `cargo test` local | Usar `cargo test`, `cargo fmt --check`, Clippy y cobertura con umbral acordado |
| Calidad frontend | Vitest, cobertura crítica, Storybook y Playwright | Vitest + Testing Library y mocks de Tauri | Conservar cobertura, Storybook/E2E y añadir mocks nativos de Tauri |
| Contrato Rust/TypeScript | Contrato Python/TS probado | Tests que comparan constantes/tipos espejo | Generar tipos desde Rust cuando sea posible y añadir pruebas de deriva |
| Seguridad de dependencias | Bandit, pip-audit, escaneo de secretos y SBOM CycloneDX | Sin pipeline equivalente | Sustituir por `cargo audit`/`cargo deny`, auditoría npm, secret scan y SBOM combinado |
| Supply chain del build | Acciones de CI fijadas por SHA y verificadores locales | Lockfiles y build local | Eliminar la dependencia de Actions; conservar lockfiles, auditorías y SBOM locales |
| Rendimiento | Presupuestos y benchmarks de datasets/startup | Sin suite de benchmark comparable | Mantener budgets de RAM, tiempo, startup y tamaño del instalador |
| Evidencia visual | Storybook, snapshots y E2E; capturas manuales acumuladas | Script que conduce la app real y regenera capturas del README | Automatizar capturas canónicas y no versionar capturas manuales sueltas |
| Iconografía | No se identificó una fuente única que regenere todos los formatos | SVG fuente + generación de iconos Tauri | Mantener un icono maestro vectorial y generar todos los tamaños |
| Documentación | Arquitectura, ADR, threat model, operaciones y auditorías extensas | README de producto, ROADMAP, CONTEXT y bitácora | Mantener Diátaxis/ADR; añadir contexto operativo breve sin duplicar decisiones |
| Privacidad | Aplicación local, assets offline, threat model y manejo de PII | Declara datos locales y única petición de red para updates | Crear inventario de datos/red verificable y telemetría desactivada por defecto |
| Firma de código Windows | No disponible | No disponible; SmartScreen advierte | No comprar Authenticode; publicar honestamente como editor desconocido |

### Lo que ProcessDevKill aporta y sistema anterior no tiene completo

Estas son las brechas que sí deben incorporarse a Columnia:

1. **Infraestructura Tauri de producción:** `src-tauri/`, `Cargo.toml`,
   `tauri.conf.json`, capabilities por ventana, CSP y plugins mínimos.
2. **Instalación real:** NSIS por usuario, desinstalador registrado, recursos
   legales incluidos e iconos generados desde una única fuente.
3. **Instancia única:** una segunda apertura comunica con la instancia existente
   y no crea sesiones competidoras sobre la misma caché.
4. **Centro de actualizaciones:** comprobación opcional, notas visibles, descarga
   iniciada por el usuario, progreso, cancelación, verificación y reinicio seguro.
5. **Release local de extremo a extremo:** preflight, árbol limpio, pruebas,
   sincronía de versión, build, firma gratuita del updater, hashes y smoke del
   instalador. Publicar será un paso manual separado.
6. **Dry-run del release:** validar todo lo que no requiere publicar antes de
   efectuar cambios irreversibles.
7. **Contrato de escritorio probado:** mocks del runtime Tauri en frontend y
   pruebas que detecten divergencias entre tipos/eventos de Rust y TypeScript.
8. **Capturas reproducibles del binario real:** documentación visual generada
   por una rutina estable, con datos sintéticos y sin PII.
9. **Configuración mínima del editor:** recomendar `rust-analyzer` y la extensión
   de Tauri sin imponer preferencias personales.

### Lo que no se copiará

- No se crearán CI, GitHub Actions ni archivos de workflow. Toda comprobación se
  podrá ejecutar localmente y devolverá un código de salida fiable.
- No se copiarán las skills o archivos de agentes del repositorio remoto; deben
  responder al flujo real de Columnia y tener procedencia/licencia verificable.
- No se usará SHA-256 como prueba de autoría de una actualización.
- No se expondrá al frontend una ruta arbitraria para ejecutar instaladores.
- No se mantendrán tres versiones editadas manualmente sin un verificador o una
  fuente de verdad.
- No se habilitarán todos los targets de Tauri mientras Columnia solo esté
  verificada en Windows.
- No se conservarán bitácoras gigantes dentro del ROADMAP. Las decisiones
  duraderas irán a ADR; el ROADMAP registrará estado y enlaces.
- No se permitirá `SkipTests` en el camino oficial de publicación local.
- No se comprará ni exigirá un certificado Authenticode, una cuenta de tienda o
  un servicio cloud de firma.

## 5. Arquitectura de infraestructura objetivo

```text
Cambios locales
      |
      v
tools/check.ps1: formato + lint + tests + seguridad + SBOM + E2E Windows
      |
      v
tools/release.ps1 -DryRun
      |
      v
Build local reproducible -> firma gratuita del updater -> hashes -> smoke
      |
      v
Revisión humana de los artefactos
      |
      v
Publicación manual en un canal HTTPS gratuito
      |
      v
Columnia consulta el manifiesto firmado -> usuario confirma -> descarga/verifica/instala
```

### Estructura inicial prevista

```text
Columnia/
├── .vscode/
│   └── extensions.json
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── operations/
│   ├── reference/
│   └── security/
├── src/                         # React + TypeScript
├── src-tauri/
│   ├── capabilities/
│   ├── icons/
│   ├── src/
│   │   ├── application/         # Casos de uso y jobs
│   │   ├── domain/              # Recetas, calidad y reglas puras
│   │   ├── infrastructure/      # Polars, DuckDB, SQLite, archivos
│   │   └── commands/            # Superficie Tauri estrecha
│   ├── Cargo.toml
│   └── tauri.conf.json
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── e2e/
├── tools/
│   ├── check.ps1
│   ├── release.ps1
│   ├── capture-screenshots.ps1
│   └── verify-release.ps1
├── CHANGELOG.md
├── CONTRIBUTING.md
├── DESIGN.md
├── LICENSE
├── README.md
├── ROADMAP.md
└── THIRD_PARTY_NOTICES.md
```

Esta estructura es una intención, no autorización para crear el scaffold. Se
ajustará después del prototipo y de decidir el alcance de la primera versión.

## 6. Roadmap de infraestructura

### Fase I0 — Contratos y gobierno del repositorio

- [x] Elegir licencia MIT y modelo de distribución abierta inicial, sin
  telemetría ni servicio remoto obligatorio.
- [x] Definir Windows x64 como objetivo inicial; macOS y Linux permanecen como
  objetivos de diseño hasta su validación local.
- [x] Adoptar SemVer para el prototipo y verificar localmente que npm, Cargo y
  Tauri mantengan la misma versión. Los tags se definirán antes del primer release.
- [x] Crear ADR para Tauri/Rust, motores de datos y frontera UI/backend.
- [x] Definir política local de ramas, revisión y commits.
- [x] Crear una lista local de dependencias desactualizadas y auditorías; no
  depender de bots para mantenerlas.
- [x] Definir y comprobar una política para fixtures: solo datos sintéticos,
  nunca datos reales o PII.

**Salida:** repositorio inicial gobernado y decisiones técnicas trazables en
`LICENSE`, `CONTRIBUTING.md`, `docs/adr/`, `docs/reference/` y los gates de
`governance:check`.

### Fase I1 — Prototipo vertical de Tauri y datos

- [x] Crear el scaffold Tauri 2 + React + TypeScript + Vite.
- [x] Crear y probar en frontend el primer comando Tauri tipado (`get_app_info`).
- [x] Compilar el shell Tauri localmente en Windows.
- [x] Abrir el shell Tauri en Windows y completar un smoke de arranque.
- [x] Completar la revisión visual sistemática del shell en Windows.
- [x] Crear el primer comando Tauri tipado de selección y carga CSV.
- [x] Añadir un canal Tauri tipado de progreso para carga CSV y perfilado. El
  progreso de lectura es por fases; el perfil avanza por columnas.
- [x] Leer y perfilar CSV, TSV, JSON, Excel/ODS y Parquet con fixtures sintéticos
  representativos, conservando tipos nativos donde el formato sí aporta esquema.
- [x] Implementar una vista previa paginada sin enviar el dataset completo a
  React: páginas de 50 filas obtenidas desde la sesión Rust.
- [x] Ejecutar una receta lazy con limpieza, tipos, filtro y columna calculada.
- [x] Implementar la primera transformación reversible: eliminar duplicados
  exactos preservando la primera aparición, sin modificar el archivo original.
- [x] Sustituir el historial provisional de un nivel por hasta doce revisiones
  locales con deshacer/rehacer, snapshots Parquet y presupuesto explícito.
- [x] Guardar y cargar recetas estructurales JSON versionadas entre sesiones,
  sin exponer rutas al frontend ni aplicarlas automáticamente.
- [x] Añadir biblioteca local de proyectos con catálogo SQLite versionado,
  migraciones v1/v2→v3, snapshots Parquet y ejecución CLI por lotes de recetas.
- [x] Implementar cancelación cooperativa para carga CSV y perfilado, conservando
  el dataset anterior cuando se cancela una sustitución.
- [x] Implementar exportación atómica y cancelable CSV/Parquet mediante selector
  nativo, sin modificar el archivo original ni exponer rutas a React.
- [x] Comparar tiempo y RAM con `sistema anterior` mediante un benchmark local
  reproducible de 100 MiB, con cleanup y evidencia sanitizada.

**Gate:** el benchmark cruzado de I1 está aprobado; la arquitectura de entrada
de libros sigue condicionada a validar los casos difíciles de Excel antes de
declararla definitiva.

**Cierre 2026-08-23:** I1 queda implementada de extremo a extremo. Las recetas
compatibles ejecutan un plan Polars lazy con renombres, tipos, filtros y
columnas calculadas; las operaciones no compatibles conservan el camino eager
para mantener el contrato estricto. El monitor nativo de CPU/RAM se muestra en
el lateral de la aplicación, y `perf:i1` compara la inspección de 100 MiB con
`sistema anterior` sin conservar datos de la fixture.

**Estado actual 2026-08-26:** Columnia ya no impone el límite provisional de
500 MiB a los datasets. La capacidad efectiva queda determinada por la RAM, el
espacio en disco y los demás recursos disponibles; la materialización y las
operaciones eager todavía pueden requerir varias veces el tamaño del archivo.
El conteo de duplicados normalizados usa bloques paralelos y derrame temporal de
huellas XXH3-128 en 256 cubetas: solo una cubeta se ordena en RAM y la cancelación
cooperativa limpia el directorio temporal. Esto reduce el pico adicional de memoria
sin guardar valores del dataset, aunque la materialización del `DataFrame` y otras
operaciones eager siguen siendo el límite principal; el runtime Tauri no usa GPU.

**Avance 2026-08-12:** `npm run build`, veinticuatro pruebas Vitest y dieciocho pruebas Rust
pasan. El comando Rust `pick_and_load_csv` abre el selector nativo sin aceptar
rutas desde React, valida un límite provisional de 500 MB, carga el CSV con
Polars, conserva la sesión en memoria y devuelve esquema, metadatos y un máximo
de 50 filas. `get_dataset_page` permite navegar en páginas de 50 filas, limita
cada solicitud a un máximo de 200 y rechaza accesos sin una sesión activa.
`get_dataset_profile` calcula en un hilo de trabajo nulos, completitud, valores
únicos no nulos y, para columnas numéricas, mínimo, máximo y promedio. El perfil
también identifica filas duplicadas adicionales y mide vacíos y longitudes en
columnas de texto usando caracteres Unicode. Se conserva en caché durante la
sesión para evitar trabajo repetido.
El perfil propone de forma informativa tipos booleano, entero, decimal o fecha
para columnas de texto con al menos tres valores y 90% de coincidencia, y expone
cuántos valores no cumplen; todavía no transforma los datos.
Para columnas numéricas calcula desviación estándar muestral, cuartiles,
mediana y posibles outliers con la regla IQR de 1.5; exige al menos cuatro
valores antes de reportar outliers.
Para columnas categóricas de texto no sensibles, el perfil calcula un resumen
acotado de los grupos más frecuentes: hasta cuatro columnas y ocho categorías
por columna, con categorías raras o excedentes reunidas en “Resto”. Los grupos
con menos de tres filas y las columnas marcadas como contacto, nombre o
identificador no se publican; la tabla visual tiene un equivalente accesible.
La primera transformación elimina duplicados exactos de la sesión, conserva el
orden y la primera aparición, actualiza la vista previa e invalida el perfil. El
archivo original permanece intacto y existe un único punto de deshacer; este
historial es provisional hasta introducir recetas reproducibles.
La interfaz usa un panel lateral y replica el flujo de producto verificado en
`sistema anterior`: Cargar, Revisar, Preparar y Entregar. Esto evita una única
pantalla creciente y mantiene cada responsabilidad en su etapa. El límite de
carga se elevó a 500 MB a petición del usuario,
con una advertencia visible: todavía se materializa el dataset completo y el uso
real de RAM puede ser bastante mayor hasta incorporar streaming.
La carga y sustitución del CSV pertenece exclusivamente a Cargar. Revisar aloja
el diagnóstico de calidad y la vista previa; Preparar aloja las operaciones que
modifican la sesión, empezando por eliminar duplicados; Entregar es la única
etapa que muestra la exportación. Al completar una carga, Columnia avanza a
Revisar como lo hace el proyecto de referencia.
La versión 0.3.0 incorpora un canal IPC tipado para informar el progreso de la
carga CSV y el análisis de calidad. La interfaz muestra la etapa y el porcentaje
solo dentro de Datos o Calidad, según la operación activa. El progreso de carga
es honesto por fases; el perfil informa avance real al completar cada columna.
La versión 0.4.0 añade cancelación cooperativa mediante generaciones atómicas
independientes para carga y perfilado. El perfil comprueba la cancelación entre
columnas. La lectura CSV de Polars se puede descartar al terminar la fase activa,
pero no interrumpir dentro del parser; la interfaz lo refleja como “Cancelando”.
Si se cancela la selección o sustitución, Columnia recupera el dataset anterior
en lugar de dejar la sesión vacía.
La versión 0.5.0 exporta el dataset activo a CSV o Parquet. Rust abre el selector
de destino y React nunca recibe ni propone rutas. La escritura ocurre en un
temporal dentro de la carpeta elegida, se sincroniza y después reemplaza
atómicamente el destino. Una cancelación o error elimina el temporal y conserva
intacto cualquier archivo anterior. La exportación informa progreso y puede
cancelarse con las mismas garantías cooperativas de la carga.
La versión 0.6.0 corrige la arquitectura de navegación según `sistema anterior` y
añade pruebas de regresión para impedir que la selección de archivos aparezca
fuera de Cargar o que la exportación aparezca fuera de Entregar.
La versión 0.7.0 incorpora en Preparar la primera corrección segura del proyecto
de referencia: normalizar nombres de columnas. Rust elimina acentos, convierte
a minúsculas, sustituye espacios y guiones por `_`, protege nombres que empiezan
con números y resuelve colisiones con sufijos deterministas. El cambio invalida
el perfil y se integra con el punto de deshacer provisional.
La versión 0.8.0 agrupa dos correcciones de valores textuales inspiradas en el
proyecto de referencia. Recortar espacios actúa sobre todas las columnas String
y conserva mayúsculas, acentos y espacios internos. Normalizar texto exige una
selección explícita de columnas, compacta espacios, convierte a minúsculas y
permite decidir si se eliminan acentos. Ambas preservan nulos, cuentan exactamente
celdas, filas y columnas modificadas, invalidan el perfil y pueden deshacerse.
La columna `_cambios` se pospone hasta disponer de recetas por lotes y auditoría
por fila; implementarla como acción independiente no representaría el historial
real de cambios.
La versión 0.9.0 añade continuidad explícita en Preparar: Deshacer y Rehacer
intercambian una única revisión reversible sin mantener una pila de copias de
hasta 500 MB en RAM. Una mutación nueva descarta la rama de rehacer; los no-op y
errores conservan el estado previo. También incorpora Aplicar recomendadas, que
calcula recorte exterior y normalización de encabezados sobre un candidato y
publica ambas correcciones atómicamente como una sola revisión. El historial
múltiple se diseñará con snapshots Parquet temporales, límites de entradas y
presupuesto explícito de disco antes de prometerlo en la interfaz.
La versión 0.10.0 generaliza Cargar a datasets CSV y Parquet. El selector nativo
solo anuncia formatos realmente implementados; Parquet conserva su esquema,
nulos y Unicode y se lee en modo de baja memoria sin paralelización para reducir
picos. Se compilan Date, Datetime y Duration para preservar tipos temporales del
formato. El límite eager de 500 MB se mantiene porque el peso comprimido de un
Parquet no representa su tamaño descomprimido en memoria. Excel se pospone hasta
separar inspección del libro, selección de hoja y carga, evitando elegir hojas
silenciosamente o degradar tipos.
La versión 0.11.0 añade TSV estricto y libros XLSX, XLS, XLSB y ODS. Rust conserva
la ruta detrás de un identificador opaco: React recibe únicamente el nombre del
archivo y las hojas disponibles. Una sola hoja se carga directamente; varias
abren un selector explícito. La hoja elegida se convierte con encabezados únicos,
nulos preservados y tipos seguros para booleanos, enteros, decimales, fechas y
duraciones; las mezclas incompatibles y los errores de celda se conservan como
texto. La sustitución sigue siendo transaccional y no borra el dataset activo si
se cancela o falla. TSV usa tabulador fijo y se lee como texto para conservar
ceros iniciales, enteros grandes y formatos como `1.00`.
La versión 0.12.0 completa los formatos de entrada del flujo de referencia con
JSON de registros y JSON Lines. Solo se aceptan objetos tabulares: los campos se
unen en orden determinista, las ausencias quedan como nulos y los valores
anidados se preservan serializados como JSON. Los escalares o arreglos con
registros no-objeto se rechazan con un error explícito. La carga mantiene el
token opaco, el límite provisional de 500 MB y la publicación transaccional del
dataset activo. El hito I1 de lectura y perfilado multiformato queda completado
con fixtures sintéticos locales.
La versión 0.13.0 elimina la inferencia destructiva de CSV: todas sus columnas se
leen como texto para conservar exactamente identificadores con ceros iniciales,
enteros grandes y literales decimales como `1.00`. El perfil recupera estadísticas
numéricas semánticas solo cuando todos los valores no vacíos pueden convertirse
sin confundir identificadores ni exceder la precisión entera exacta de `f64`.
Así, mínimo, máximo, promedio, dispersión, cuartiles y outliers siguen disponibles
para medidas textuales seguras, mientras códigos como `00123` permanecen texto y
no generan sugerencias numéricas engañosas.
La versión 0.14.0 completa tres frentes paralelos. Los libros permiten elegir
entre usar la primera fila como encabezado o conservarla como datos con nombres
generados; los contenedores comprimidos muestran una advertencia de memoria.
CSV/TXT detectan conservadoramente `,`, `;`, tabulador o `|` sobre una muestra
UTF-8 acotada y consciente de comillas; TSV fuerza tabulador, UTF-8 BOM se acepta
y bytes inválidos se rechazan sin sustitución. Además, `tools/check.ps1` aporta
perfiles Fast, Full y Release, y pruebas locales fijan la CSP y capability mínima.
No se añade CI, workflows ni servicios de pago.
La versión 0.15.0 abre `Transformaciones` dentro de Preparar con una receta
estructural por lote. El usuario puede definir varios renombres, conversiones
estrictas de tipo y parseos explícitos de fecha; Rust los valida y ejecuta en el
orden renombrar → tipos → fechas sobre un candidato aislado. La publicación es
atómica: un fallo conserva intactos dataset, perfil e historial, mientras un
éxito genera una sola revisión reversible. Los renombres simultáneos permiten
intercambios de nombres y las referencias posteriores se resuelven contra el
esquema anterior a la receta. Este hito aún no implica ejecución lazy, recetas
persistentes ni historial multinivel.
La versión 0.16.0 extiende esa misma transacción con hasta tres filtros unidos
por `AND` y una columna calculada. Los filtros cubren igualdad, comparación
numérica estricta, búsqueda literal sin distinguir mayúsculas y nulos; al poder
eliminar filas requieren confirmación explícita. La columna derivada distingue
sin ambigüedad entre un valor fijo y otra columna, y admite aritmética,
concatenación y extracción de año, mes o día. El motor rechaza pérdida de
precisión, división por cero, resultados no finitos y fechas fuera de rango.
Todo se ejecuta en el orden estructura → filtros → cálculo y ocupa una sola
revisión reversible. La ejecución continúa siendo eager; el objetivo lazy de
la Fase I1 permanece abierto.
La versión 0.17.0 incorpora búsqueda y reemplazo literal sobre una columna de
texto o todas las columnas físicas de texto, además de selección y orden de las
columnas finales. Los nulos y tipos no textuales se preservan, el conteo informa
celdas realmente modificadas y una sustitución sin cambios no consume historial.
Descartar columnas requiere confirmación y el motor impide eliminar una fuente
necesaria por una columna calculada posterior. Renombres, filtros, reemplazos,
selección y cálculo siguen formando una sola publicación atómica y reversible.
La versión 0.18.0 añade división y combinación deterministas de columnas de
texto. Dividir crea entre dos y dieciséis columnas con un separador literal: la
última conserva el resto, los campos ausentes quedan nulos y los vacíos siguen
siendo valores vacíos. Combinar respeta el orden elegido, omite únicamente los
nulos y mantiene nula una fila sin ningún valor. Eliminar las fuentes requiere
confirmación, y el motor valida tipos, renombres, conversiones, colisiones y
dependencias antes de publicar una sola revisión reversible.
La versión 0.19.0 incorpora tratamiento IQR de valores atípicos sobre hasta
dieciséis columnas numéricas: limitar valores a los umbrales o eliminar las
filas que los exceden. Todos los cuartiles se calculan sobre el mismo estado
previo para que el orden de las selecciones no altere el resultado. Los nulos y
límites exactos se conservan; NaN, infinitos, rangos no representables y enteros
fuera de la precisión exacta se rechazan. Cualquier tratamiento exige
confirmación y un resultado sin outliers no crea una revisión vacía.
La versión 0.20.0 añade agrupación y resumen como etapa final y exclusiva de
granularidad. Permite entre una y ocho claves y hasta treinta y dos agregaciones:
suma, promedio, mínimo, máximo, conteo de filas y valores únicos. Los grupos
mantienen el orden de primera aparición y admiten claves nulas. El motor preserva
tipos en mínimos y máximos, comprueba overflow y precisión, y define expresamente
los resultados para grupos completamente nulos. Como reemplaza las filas por un
resumen, siempre requiere confirmación y genera una sola revisión reversible.
La versión 0.21.0 incorpora normalización explícita de correos, teléfonos y
direcciones, además de extracción literal de tokens, dígitos, letras y segmentos
separados por delimitadores. Las reglas son Unicode y preservan nulos; los
teléfonos conservan únicamente un `+` inicial y dígitos ASCII, y las direcciones
solo compactan espacios sin cambiar arbitrariamente las mayúsculas. Se evita
regex libre en este hito. Contactos se procesan antes de agrupar; una expansión
posterior permite que las extracciones también alimenten las claves y
agregaciones del resumen.
La versión 0.22.0 sustituye el historial de una sola revisión por una línea local
de hasta doce estados. Cada estado se escribe primero como Parquet temporal,
se sincroniza y solo entonces se publica; Deshacer y Rehacer restauran también
tipos y nulos sin mover el cursor ante un archivo corrupto. Una mutación posterior
a Deshacer corta la rama futura, mientras los no-op la conservan. El presupuesto
de disco es 1 GiB: si un único snapshot lo supera, la operación puede continuar,
pero la interfaz desactiva la reversión y explica la degradación. Los temporales
pertenecen a la sesión y se limpian al reemplazarla o cerrar Columnia.
La versión 0.23.0 permite guardar y cargar el borrador completo de Transformaciones
como JSON v1. Los selectores y las rutas permanecen exclusivamente en Rust; el
archivo está limitado a 1 MiB, debe ser UTF-8, usa campos estrictos y rechaza
versiones futuras. El guardado se realiza mediante un temporal sincronizado y
reemplazo atómico. Cargar solo hidrata el editor —con confirmación si hay trabajo
sin guardar— y nunca modifica ni ejecuta el dataset hasta que el usuario pulse
Aplicar receta.
La versión 0.24.0 añade contratos de calidad a Entregar. Se pueden combinar hasta
dieciséis reglas exactas de no nulo, texto no vacío, unicidad y rango numérico,
con tolerancia por conteo o porcentaje. La interfaz invalida resultados anteriores
al cambiar reglas o dataset y no muestra valores sensibles. La protección también
vive en Rust: antes de abrir el selector, la exportación vuelve a validar sobre el
mismo snapshot que escribirá y bloquea cualquier regla fallida. Exportar sin una
política requiere confirmación explícita y la validación respeta la cancelación.
`tauri build --debug --no-bundle` genera correctamente
`src-tauri/target/debug/columnia.exe`, que permanece estable durante el smoke de
arranque. La primera compilación reveló que el
scaffold no contenía los iconos requeridos por Tauri; se añadió un SVG maestro,
se generaron los recursos multiplataforma y una prueba impide su regresión.

La versión 0.25.0 incorpora proyectos locales recuperables. El catálogo SQLite
guarda metadatos, reglas de calidad y un borrador opcional de receta, mientras
generaciones Parquet privadas permiten abrir el proyecto aunque desaparezca la
fuente original. Guardar, abrir, recuperar y borrar usan identificadores opacos;
React nunca recibe rutas administradas.

La versión 0.26.0 eleva el catálogo a SQLite v3 compatible con v1/v2 y hace
durables el perfil cacheado, las revisiones Parquet y el cursor de historial.
Cada apertura valida todos los artefactos y crea copias temporales de sesión, de
modo que Deshacer/Rehacer no modifica el proyecto persistido hasta guardarlo de
nuevo. Se conservan los límites de doce revisiones y 1 GiB por proyecto.

La versión 0.27.0 expone cinco comandos de proyectos en `columnia-cli`:
`project-list`, `project-save`, `project-inspect`, `project-export` y
`project-delete`. Todos exigen un `--store` explícito y seguro, emiten JSON v1
sin rutas, filas ni muestras y funcionan entre procesos independientes.
Exportar vuelve a evaluar las reglas guardadas, solo permite omitir un contrato
inexistente con `--allow-unvalidated` y publica de forma atómica; borrar exige
que `--confirm` coincida exactamente con el ID. El smoke real cubre receta,
reglas, perfil, reinicio, exportación y confirmación destructiva.

La versión 0.28.0 añade integración de la UI de proyectos: Vitest recorre el
guardado y borrado confirmado/cancelado desde `App`, verificando que el dataset
activo se conserva; `smoke-tauri.ps1` valida el contrato estático de
`ProjectsPanel`, comprueba la ventana debug/WebView2 y deja explícito que no
simula clics DOM mientras UI Automation no exponga el árbol React de forma
estable.

La versión 0.29.0 amplía la cobertura del ciclo de proyectos con una prueba de
guardar→catálogo→abrir→restaurar dataset, perfil y reglas. También añade
targets interactivos mínimos de 24 px y reducción global de movimiento para
`prefers-reduced-motion`, con regresión CSS automatizada. El baseline local deja
startup total de escritorio en 6.33–6.98 s, mediana aproximada de 6.71 s y un
objetivo provisional menor de 8 s; la automatización DOM real de WebView2 sigue
pendiente de un driver/CDP aprobado.

La versión 0.30.0 instrumenta el smoke Tauri con hitos monotónicos de Vite,
proceso debug y ventana visible, y confirma el cleanup con hasta dos intentos
acotados. También añade contratos automatizados de landmarks, skip link,
estados ARIA, acciones de proyectos y `alertdialog`; no amplía capabilities ni
activa depuración WebView2.

La versión 0.31.0 configura Playwright 1.62 contra un preview Vite local (Edge en
Windows y Chromium en otros sistemas) para probar el shell web de forma
reproducible. Los dos E2E cubren landmarks, estado del runtime, navegación
accesible y skip link por teclado, con servidor reutilizable y artefactos de diagnóstico solo en fallos. La interacción real de
la ventana Tauri, el IPC Rust y los flujos de proyectos siguen requiriendo un
driver/CDP de WebView2 aprobado.

La versión 0.32.0 añade un mock aislado de `__TAURI_INTERNALS__` para que
Playwright recorra el flujo de proyectos en el shell web: cargar dataset, guardar,
abrir y eliminar con confirmación. Verifica el contrato React↔bridge sin tocar
filesystem ni SQLite reales; la interacción contra la ventana WebView2 nativa
continúa pendiente.

La versión 0.33.0 añade la marca `columnia:app-render` y un E2E de rendimiento que
mide el primer render del shell en el preview local, con presupuesto de 3 segundos.
Esto aporta una señal independiente de la compilación y del smoke Tauri, pero no
se presenta todavía como medición del WebView2 nativo.

La versión 0.34.0 añade un probe Windows aislado para WebView2: publica un puerto
CDP de loopback solo durante `npm run smoke:cdp`, verifica los endpoints de
diagnóstico y conecta Playwright para leer el DOM del shell y sus landmarks. El
probe conserva Job Object, cleanup y evidencia JSON; no habilita depuración en el
arranque normal ni ejecuta todavía mutaciones IPC de proyectos.

La versión 0.35.0 amplía el probe nativo para medir `columnia:app-render` dentro
de WebView2 con un presupuesto de 3 segundos y verificar landmarks, skip link y
foco del contenido principal. También añade E2E responsive con `prefers-reduced-motion`,
viewport móvil/desktop, targets mínimos y ausencia de overflow horizontal.
La medición nativa queda registrada como señal (no como gate de compilación fría):
la última ejecución observó 26,157.9 ms durante el arranque debug, mientras los
contratos de landmarks y foco pasaron.

La versión 0.36.0 conserva el probe nativo aislado y añade una lectura DOM de
solo lectura específica de `ProjectsPanel`: región y heading accesibles, etiqueta
del nombre, guardado deshabilitado sin dataset, ausencia de rutas visibles y
nombres de acción. `npm run smoke:cdp` ejecuta ambos probes en la misma ventana
WebView2 y deja sus contratos en la evidencia JSON, sin hacer clics, escribir
datos ni abrir diálogos. También incorpora `npm run perf:summary`, que lee solo
las evidencias locales existentes, separa señales web/CDP/desktop y calcula
deltas sin convertir la compilación fría en un fallo de producto.

La versión 0.37.0 extiende el probe de proyectos hasta la frontera IPC nativa de
solo lectura: dentro del WebView2 invoca `list_projects` y
`get_recovery_candidate`, valida la forma de los resúmenes y comprueba que no
aparezcan campos de rutas. La evidencia conserva únicamente conteos y estados;
no hace clics, no escribe proyectos, no abre diálogos y no registra nombres ni
datos del catálogo.

La versión 0.38.0 añade `npm run perf:benchmark`, un benchmark local y
reproducible de la CLI que genera una entrada sintética cercana a 100 MiB y mide
`inspect`, `validate`, `transform` a CSV y `transform` a Parquet. Cada comando
registra duración y pico de working set; los archivos de trabajo se eliminan al
finalizar y la evidencia conserva solo tamaños, conteos y métricas. La primera
muestra (100.11 MiB, 876,544 filas) observó un pico de 394.6 MiB durante la
transformación CSV, por lo que el límite provisional de 500 MiB no se presenta
como presupuesto final ni sustituye el benchmark de la ventana Tauri.

La versión 0.39.0 añade perfilado acotado al probe WebView2/CDP: durante el
arranque real de `npm run tauri dev` toma muestras del working set y la memoria
privada del ejecutable debug, conserva inicial/máximo/final y expone el bloque
en `npm run perf:summary`. No cambia el arranque normal, no registra rutas ni
datos del usuario y todavía no es un presupuesto global de RAM ni una medición
de transformaciones dentro de la ventana.

La muestra v0.39.0 verificada registró 3 muestras y 7 procesos propios
(`columnia` + WebView2), con working set máximo de 366,428,160 bytes (~349.4 MiB)
y memoria privada máxima de 147,488,768 bytes. El primer render frío fue de
43,412 ms; se conserva como señal de arranque debug fuera del presupuesto de 3 s.

La versión 0.40.0 completa el primer recorrido de mutación nativa del probe:
`probe_seed_dataset` (solo debug) prepara dos filas en memoria, luego Playwright
invoca `save_project`, `list_projects`, `open_project`, `get_dataset_page` y
`delete_project`. El runner verifica que el catálogo vuelva a su conteo inicial,
que el dataset permanezca disponible durante la apertura y que no aparezcan rutas,
IDs ni filas en la evidencia; cualquier fallo intenta borrar el proyecto temporal.
La validación Windows v0.40.0 dejó el catálogo en 0→0, cleanup confirmado y un
working set nativo máximo de 365,252,608 bytes (~348.4 MiB); el primer render frío
sigue siendo una señal observacional fuera del presupuesto de 3 s.

La versión 0.41.0 extiende el mismo límite de debug hasta receta y entrega:
`probe_save_transform_recipe` escribe y vuelve a leer una receta JSON desde un
directorio temporal, `apply_transform_recipe` la publica sobre el dataset
sintético y `probe_export_dataset` ejecuta la compuerta de calidad y la
exportación CSV atómica sin abrir diálogos. El proyecto guarda y restaura el
borrador de receta y la regla; el destino temporal desaparece al terminar y la
evidencia solo conserva estados, conteos y nombres de comandos.

La versión 0.42.0 añade una reapertura durable antes de la apertura IPC:
`probe_reopen_project` crea un `ProjectStore` nuevo contra el mismo almacén de
la aplicación, vuelve a leer SQLite y el snapshot Parquet, valida recovery,
dimensiones, reglas y borrador y entrega únicamente un resumen sanitizado. El
runner continúa con `open_project` y `delete_project`, por lo que la evidencia
confirma persistencia y cleanup 0→0 sin retener el proyecto temporal.

La versión 0.43.0 completa el reinicio real del probe: `npm run smoke:restart`
ejecuta una fase que persiste el proyecto y termina su proceso Tauri/WebView2,
seguida por una segunda fase independiente que recupera el candidato desde
SQLite/snapshot, valida el workspace, abre, pagina y elimina solo el proyecto
del probe. Cada fase conserva su propio Job Object y cleanup.

La versión 0.44.0 añade una señal de rendimiento por operación al mismo ciclo:
el runner mide cada comando IPC nativo y el total de receta, exportación,
persistencia, reapertura y cleanup, conservando únicamente milisegundos y
conteos. El probe CDP toma muestras posteriores al runner y aplica por defecto
un presupuesto de 512 MiB de working set y 256 MiB de memoria privada al árbol
de procesos propio; una ejecución soportada que exceda esos límites falla y el
resumen de rendimiento conserva el detalle para comparar regresiones.

La validación Windows v0.44.0 pasó Vitest 129/129, Playwright 9/9, CDP normal,
reinicio real, desktop, CLI, benchmark y Package. El CDP normal observó 448.97
MiB de working set y 251.92 MiB de memoria privada, dentro de los límites; las
dos fases de reinicio también quedaron dentro de presupuesto y con cleanup.

La versión 0.45.0 completa la primera capa reproducible de evidencia visual:
`src/styles.css` añade un contrato `forced-colors: active` basado en colores del
sistema para conservar contraste, bordes y foco en Windows High Contrast.
`npm run accessibility:visual` construye el preview y captura desktop, móvil,
escala 125% y forced-colors, verificando landmarks, targets de 24 px, foco y
overflow horizontal. Las capturas quedan fuera de Git y el resumen solo conserva
metadatos sanitizados; lector de pantalla y hardware real siguen pendientes.

La validación Windows v0.45.0 pasó Vitest 130/130, Rust 127/127, Playwright
9/9, las cuatro capturas visuales, CDP normal, reinicio real, desktop, CLI,
benchmark y Package. El bundle final quedó en 315,829 bytes raw/90,324 gzip y
los límites de memoria CDP se mantuvieron dentro de presupuesto.

La versión 0.46.0 convierte esas señales en contratos comparables. Cada captura
visual conserva SHA-256 y `npm run accessibility:check` compara los cuatro casos
contra `fixtures/accessibility/visual-baseline-v1.json`, verificando escenarios,
landmarks, foco, targets y overflow sin versionar imágenes. `npm run perf:check`
cruza el último resumen CDP, el benchmark de 100 MiB y el reporte Package contra
`fixtures/performance/performance-baseline-v1.json`; `npm run verify:experience`
ejecuta ambos gates después de generar evidencias.

La validación Windows v0.46.0 pasó Vitest 130/130, Rust 127/127, Playwright
9/9, captura y baseline visual 4/4, benchmark de 100 MiB, CDP normal, reinicio
real, `perf:summary`, baseline de rendimiento y Package. El CDP observó 449.49
MiB de working set y 246.98 MiB privados; el benchmark alcanzó 104,963,092 bytes
con cleanup confirmado. La auditoría manual con lector de pantalla y hardware
High Contrast continúa pendiente.

La versión 0.47.0 amplía `npm run perf:benchmark` a tres iteraciones sostenidas
de transformaciones CSV/Parquet y añade un ciclo CLI durable de proyecto:
`project-save` con receta, reglas y perfil, seguido de `project-inspect`,
`project-export`, `project-list`, `project-delete` y un catálogo vacío final.
El benchmark elimina su almacén temporal y publica únicamente tiempos, conteos,
estados y picos de memoria; `perf:check` exige las operaciones y al menos tres
iteraciones dentro del presupuesto de 512 MiB. Esto mide el motor CLI y no cierra
todavía la medición nativa desde WebView2 ni la ejecución lazy/incremental.

La validación Windows v0.47.0 pasó Vitest 130/130, Rust 127/127, Playwright
9/9, evidencia visual, benchmark sostenido, ciclo durable de proyecto, CDP,
reinicio, desktop/CLI, `perf:summary`, baseline de rendimiento y Package. El
benchmark alcanzó 104,963,092 bytes y su pico de working set fue 476,659,712
bytes; el gate CDP quedó en 451,100,672 bytes de working set y 242,634,752 bytes
privados, con cleanup confirmado. Selector nativo, medición WebView2 sostenida,
lazy/incremental y lector de pantalla manual siguen pendientes.

La versión 0.48.0 endurece el contrato de rendimiento del benchmark CLI: además
de tres iteraciones sostenidas exige límites explícitos para transformaciones,
guardado, inspección y exportación. El ciclo de proyecto actualiza dos veces el
mismo ID, inspecciona una reapertura durable y exporta después de la actualización;
los logs siguen sanitizados y el almacén temporal se elimina. `npm run verify:tier`
orquesta la validación reproducible completa, y `ACCESSIBILITY_MANUAL_CHECKLIST.md`
deja una ruta operativa para teclado, lector de pantalla, zoom y High Contrast.
Esto no cierra la medición nativa sostenida desde WebView2, la auditoría manual real
ni la ejecución lazy/incremental.

La validación Windows v0.48.0 pasó Vitest 130/130, Rust 127/127, Playwright 9/9,
build, evidencia/baseline visual 4/4, benchmark, Package, smoke CLI, desktop, CDP,
reinicio y los gates finales. El benchmark alcanzó 104,963,092 bytes con pico CLI
de 467,546,112 bytes; sus máximos fueron 2.58 s/26.26 s/11.46 s/13.14 s para
transformación/guardado/inspección/exportación. El resumen final reunió 49 muestras
CDP y 25 desktop; el CDP observó 438,829,056 bytes de working set y 242,012,160
bytes privados, dentro del presupuesto. La auditoría manual real y lazy/incremental
siguen pendientes.

La versión 0.49.0 extiende el probe nativo de WebView2: cada smoke con mutaciones
ejecuta tres ciclos reales de transformación y exportación sobre el dataset de
prueba antes de guardar el proyecto. El resumen conserva únicamente duraciones y
conteos; `perf:check` exige esas tres iteraciones y verifica sus máximos junto al
presupuesto global de 512 MiB/256 MiB aplicado al árbol Tauri/WebView2. Esto cierra
la señal nativa sostenida del probe, pero no sustituye el benchmark de datasets
grandes ni la implementación lazy/incremental.

La validación Windows v0.49.0 pasó `npm run verify:tier` en 9.09 minutos: Vitest
130/130, Rust 127/127, Playwright 9/9, build, evidencia/baseline visual 4/4,
benchmark, Package, smoke CLI/desktop, CDP sostenido, reinicio y gates finales.
El resumen reunió 59 muestras CDP y 26 desktop; el gate observó 453,664,768 bytes
de working set y 249,978,880 bytes privados. La señal nativa ejecutó tres ciclos
con máximos de 9.8 ms de transformación y 4.7 ms de exportación. El benchmark
alcanzó 104,963,092 bytes, tuvo pico CLI de 492,957,696 bytes, máximos de
6,243.26/26,306.45/11,436.06/13,285.38 ms y cleanup confirmado. Evidencia final:
`.local/validation/webview2-cdp/20260823T055801Z`,
`.local/validation/webview2-restart/20260823T055558Z` y
`.local/validation/performance-baseline/20260823T055904Z`.

### Fase I2 — Frontera de seguridad del escritorio

- [x] Definir CSP estricta: sin CDN, `object-src 'none'`, sin navegación remota,
  cubierta por una prueba local de regresión.
- [x] Crear una capability exclusiva para la ventana principal y conceder solo
  `core:default`; filesystem, shell, HTTP y opener permanecen fuera del frontend.
- [x] Mantener red fuera del frontend; la CSP de producción no admite conexiones
  HTTP(S)/WebSocket externas y una prueba impide ampliarla accidentalmente.
- [x] Validar y canonicalizar las rutas actuales de datasets, recetas,
  exportaciones y almacenes de proyectos antes de leer o escribir.
- [x] Usar selectores nativos; el frontend recibe handles/identificadores, no
  autoridad global sobre el filesystem.
- [x] Implementar instancia única y recuperación explícita de proyectos sin
  abrir datos silenciosamente.
- [x] Mantener un threat model vivo para datasets, fórmulas, rutas, proyectos,
  red y updater; SQL y updater permanecen como fronteras futuras.
- [x] Añadir pruebas negativas de traversal, symlinks/reparse points, fórmulas y
  payloads semánticos grandes.

**Gate:** revisión de la superficie de comandos y de cada permiso Tauri.

### Fase I3 — Calidad local reproducible

- [x] Crear `tools/check.ps1` como entrada única para `cargo fmt --check`, Clippy
  con warnings como errores, `cargo test`, TypeScript, Vitest y build Tauri.
- [x] Añadir perfiles rápido, completo y release; el perfil release siempre
  ejecutará todos los gates.
- [x] Ejecutar las pruebas puras de forma local sin requerir abrir la ventana.
- [x] Mantener mocks controlados del runtime Tauri para pruebas del frontend.
- [x] Verificar localmente la deriva de comandos, argumentos, retornos, campos y
  tipos entre Rust y TypeScript; la generación automática sigue siendo opcional.
- [x] Mantener E2E, accesibilidad WCAG 2.2 AA, pruebas visuales y el recorrido
  nativo Win32. Vitest cubre guardar→catálogo→abrir→restaurar y contratos de
  landmarks/ARIA; CSS cubre targets/reduced motion; el gate WebView2 real
  verifica Abrir/Guardar como, recetas y exportación sin exponer rutas.
- [ ] Completar la auditoría manual de asistencia/visual con lector de pantalla
  y hardware Windows High Contrast.
- [x] Definir umbrales de cobertura por capa, no solo un porcentaje global.
  V8 cubre `src` con 80% statements/lines, 75% branches y 75% functions;
  `npm run test:coverage` los hace cumplir.
- [ ] Completar presupuestos medibles de RAM, datasets grandes y startup; el
  perfil contractual actual, el gate CDP, el benchmark CLI y el bundle pasan.
  La corrida CLI de 100 MiB del 2026-09-02 completó tres transformaciones
  sostenidas y dos actualizaciones durables, con guardado máximo de 50.287,90 ms
  y pico de 429.744.128 B. `perf:webview2` del mismo día completó un input de
  100 MiB/819.137 filas, carga 2,847 s, paginación 28 ms, transformación 1,611 s,
  exportación 1,326 s, pico de 671.612.928 B de working set y 456.892.416 B
  privados, dentro del presupuesto de dataset grande y con cleanup confirmado.
  Desde v0.145.0,
  `perf:check` también exige que el intervalo proceso nativo listo→ventana
  visible sea ≤1.000 ms con hitos y cleanup confirmados; falta decidir el
  presupuesto global final de la aplicación y cerrar la optimización
  lazy/incremental general fuera de RAM.
- [x] Guardar reportes locales con fecha, commit, versiones de herramientas y
  resultados para que una validación pueda auditarse después.
- [x] Reducir el trabajo crítico del arranque: el bundle inicial separa las
  etapas pesadas, la migración SQLite se difiere hasta la primera operación y
  el monitor de recursos queda fuera del primer paint. Evidencia: build inicial
  de 253.10 KB raw/77.31 KB gzip y smoke desktop con Vite listo en 2.31 s.

**Gate:** no se puede declarar una fase terminada ni preparar un release si el
script local completo rompe contratos, seguridad, accesibilidad o presupuestos.

### Fase I4 — Supply chain y privacidad

- [x] Ejecutar `cargo audit` y evaluar `cargo deny` para advisories, licencias,
  duplicados y fuentes no aprobadas; las excepciones transitivas están
  justificadas en `src-tauri/deny.toml`.
- [x] Añadir auditoría explícita de vulnerabilidades npm; los lockfiles npm y
  Cargo ya tienen gates locales de sincronía, integridad y procedencia.
- [x] Escanear secretos y bloquear artefactos/datasets sensibles en Git.
- [x] Generar offline un SBOM CycloneDX 1.6 reproducible que incluya Rust y npm.
- [x] Mantener `THIRD_PARTY_NOTICES` generado y verificable.
- [x] Documentar cada acceso de red y comprobar que no exista telemetría oculta.
- [x] Telemetría y reporte remoto de fallos: desactivados por defecto; cualquier
  cambio requerirá consentimiento explícito, redacción de PII y un ADR.

**Gate:** SBOM, licencias y auditorías sin hallazgos bloqueantes.

### Fase I5 — Empaquetado e instalación Windows

- [x] Generar iconos Tauri desde un único SVG maestro de Columnia.
- [x] Configurar NSIS `currentUser` como instalador recomendado.
- [x] Producir e inventariar MSI y NSIS en Windows como artefactos locales; la
  decisión de publicación empresarial del MSI sigue pendiente.
- [x] Incluir licencia y avisos de terceros como resources; no se requiere EULA
  adicional mientras la distribución conserve la licencia MIT.
- [ ] Validar instalación, primera apertura, segunda instancia, actualización,
  desinstalación y conservación/borrado opcional de datos.
- [x] Probar usuario sin privilegios administrativos y rutas con Unicode/espacios.
- [x] Definir política WebView2 bootstrapper/offline: el instalador descarga el
  bootstrapper oficial cuando hace falta; un instalador totalmente offline sigue
  pendiente de validación como variante separada.
- [x] Medir tamaño, tiempo de instalación y tiempo hasta ventana utilizable.

**Gate:** smoke desde una VM Windows limpia, no solo desde la máquina de desarrollo.

> Evidencia local 2026-08-28: `tools/smoke-installed-artifact.ps1` pasó con el
> NSIS `Columnia_0.57.0_x64-setup.exe` desde `IMPACTX\User` sin privilegios,
> una ruta temporal con espacios/Unicode, primera apertura en 711 ms,
> segunda invocación absorbida por instancia única, instalación en 5.3 s,
> desinstalación en 1.3 s y retención/borrado del sentinel de datos de usuario.
> Una ejecución adicional con `-PreviousInstallerPath` instaló `0.49.0`, actualizó
> en la misma ruta a `0.57.0`, conservó el sentinel durante el upgrade y la
> desinstalación, y confirmó cleanup. La VM limpia, el updater contra canal real
> y las decisiones opcionales de borrado siguen siendo necesarias para cerrar
> el gate.

### Fase I6 — Actualizaciones autenticadas

- [x] Usar `tauri-plugin-updater`, cuya firma criptográfica se genera localmente
  con Tauri CLI y no requiere comprar un certificado.
- [x] Generar y custodiar fuera del repositorio la clave privada del updater.
- [x] Incrustar únicamente la clave pública en la aplicación.
- [x] Generar manifiesto, firma y checksum; rechazar un release incompleto.
- [x] Mostrar versión, tamaño y notas antes de descargar.
- [x] Descargar solo tras acción del usuario, mostrando progreso y cancelación.
- [x] Verificar firma antes de instalar y limitar cualquier ruta ejecutable al
  directorio privado del updater.
- [ ] Probar downgrade, versión igual, prerelease, descarga parcial, firma inválida,
  manifiesto corrupto, falta de red y recuperación después de un cierre. La
  frontera Rust ya rechaza downgrade/igualdad y versiones inválidas, con pruebas
  semver para estable y prerelease; el contrato local también prueba artefacto
  truncado, firma alterada, manifiesto incompleto/corrupto y URL insegura. Falta
  ejercitar el canal y el instalador real.
- [x] Definir rotación y recuperación de claves antes del primer release público.
  La política versionada y su gate son `fixtures/updater/key-policy-v1.json` y
  `npm run updater:key:check`; la implementación actual exige una release
  puente firmada por la clave anterior.
- [x] Documentar expresamente que esta firma autentica actualizaciones de
  Columnia, pero no elimina el aviso de SmartScreen ni identifica al editor ante
  Windows.

**Gate:** una actualización manipulada debe fallar de forma cerrada y dejar la
versión instalada utilizable.

### Fase I7 — Release local reproducible

- [x] Crear `tools/release.ps1` con `-DryRun` y mensajes de recuperación claros.
- [x] Exigir árbol completamente limpio, incluyendo archivos no rastreados.
- [x] Ejecutar todos los gates; la publicación oficial no admite `SkipTests`.
- [x] Sincronizar/verificar versión en Tauri, Cargo, npm y metadatos Windows.
- [x] Construir NSIS y los artefactos del updater desde el commit etiquetado.
- [x] Firmar gratuitamente los artefactos del updater con la clave privada local.
- [x] No firmar con Authenticode; aceptar y documentar `Editor desconocido` y el
  posible aviso de SmartScreen.
- [x] Generar SHA-256, SBOM, manifiesto de procedencia local y notas de release.
- [x] Instalar y ejecutar el artefacto final antes de publicarlo.
  El smoke NSIS local está integrado en el perfil `Package`; la VM limpia sigue
  pendiente como validación de distribución.
- [ ] Crear el tag únicamente después de superar toda la validación local.
- [ ] Publicar manualmente en el canal gratuito elegido.
- [ ] Descargar otra vez los assets publicados y verificar localmente firma/hash.
- [x] Mantener una vía de reanudación segura si falla después de crear el tag o
  durante la publicación manual. `docs/how-to/publish-release.md` exige no
  mover tags, reanudar con `--expected-version` y volver a descargar/verificar
  los assets con `npm run updater:verify-published`.

**Gate:** publicación promovida manualmente solo después de verificar los assets
descargados, no los archivos locales previos a la subida.

### Fase I8 — Documentación y evidencia de producto

- [x] Mantener un README orientado al problema, privacidad, uso, automatización
  local y limitaciones honestas.
- [x] Documentación separada en tutorial, how-to, referencia y explicación.
- [x] ADR para decisiones duraderas; CHANGELOG para cambios publicados.
- [x] Script de capturas con dataset sintético estable y ventanas definidas.
- [x] Regenerar capturas desde el binario de release y detectar diferencias.
- [x] No acumular imágenes manuales sin dueño, fecha o propósito.
- [x] Checklist de enlaces, encoding UTF-8 y coherencia de versiones.

**Gate:** una persona nueva puede instalar, verificar, usar y diagnosticar
Columnia siguiendo solamente la documentación publicada; `docs:check` y el gate
de evidencia release verifican los enlaces, las versiones, el contrato visual,
el ownership y los hashes generados desde `columnia.exe`.

## 7. Objetivos cuantitativos provisionales

Se confirmarán con el prototipo; hasta entonces funcionan como hipótesis a medir.

| Métrica | Objetivo inicial |
| --- | --- |
| Arranque en caliente hasta UI utilizable | ≤ 2 segundos en equipo de referencia |
| Primera vista previa | ≤ 3 segundos para CSV de 100 MB |
| Memoria durante preview | Sin tope fijo de archivo; la capacidad depende de RAM, disco y recursos disponibles. Objetivo final: no materializar el dataset completo |
| Cancelación visible | Confirmación de cancelación ≤ 1 segundo |
| Operación larga | Progreso real o estado indeterminado honesto; nunca UI congelada |
| Cobertura | Umbral por decidir después de clasificar código crítico |
| Vulnerabilidades conocidas bloqueantes | 0 en dependencias distribuidas |
| Red sin acción del usuario | Solo comprobación de actualización si está habilitada |
| Datos del dataset enviados fuera del equipo | 0 |
| Artefactos oficiales | Firma gratuita válida del updater + SHA-256; sin Authenticode |

## 8. Próximas decisiones

- [x] Aprobar el nombre `Columnia`.
- [x] Crear la carpeta `Columnia/` y establecer este ROADMAP como documento vivo.
- [x] Comparar infraestructura de sistema anterior y ProcessDevKill.
- [x] Diseñar desde el inicio para Windows, macOS y Linux.
- [x] Aprobar la arquitectura Rust + Tauri + Polars + DuckDB.
- [x] Definir licencia MIT y modelo de distribución abierta inicial.
- [ ] Definir el usuario principal y el problema número uno de la primera versión.
- [ ] Clasificar funciones actuales en conservar, rediseñar o eliminar.
- [ ] Definir formatos y bases de datos obligatorios para la primera versión.
- [x] Establecer testing, seguridad y releases como procesos exclusivamente locales.
- [x] Descartar GitHub Actions, CI y archivos de workflow.
- [x] Establecer que Columnia no dependerá de certificados o servicios de pago.
- [ ] Aprobar el updater gratuito firmado de Tauri o decidir no incluir
  actualizaciones dentro de la app.
- [x] Ejecutar el primer corte vertical CSV del prototipo técnico de la Fase I1.
- [x] Completar paginación por sesión, progreso, cancelación, Excel/ODS y Parquet.
- [x] Completar streaming/lazy y benchmark contra `sistema anterior`; el trabajo
  posterior es ampliar la cobertura a datasets mayores, historial integral y
  casos difíciles de Excel.

## 8.1. Cola de ejecución recomendada desde v0.105.0

1. **Superficie de compatibilidad externa:** retirada en v0.90.0. El selector de
   recetas y el catálogo de proyectos exponen únicamente contratos nativos de
   Columnia; no se conserva una ruta de importación, replay o round-trip de otro
   producto. Las futuras recetas deben ampliar el contrato nativo y sus pruebas,
   sin reintroducir adaptadores externos.
2. **Sistema visual y temas:** completado en v0.52.0. La barra lateral ofrece
   un selector persistente de tema `Sistema`, `Claro` y `Oscuro`; la preferencia
   se aplica antes de montar React, se conserva en almacenamiento local y mantiene
   `forced-colors`, foco visible y movimiento reducido. La dirección visual ahora
   usa una jerarquía más clara de estación de trabajo, paneles con profundidad y
   estados de selección distinguibles.
3. **Selector nativo Win32:** completado. `npm run smoke:native-selectors`
   verifica abrir dataset, guardar/cargar receta y exportar en una sesión Windows
   interactiva; `verify:tier` lo ejecuta junto a los demás smokes cuando no se
   usa `-SkipNative`. La evidencia final está en
   `.local/validation/webview2-cdp/20260824T233922Z`.
4. **Accesibilidad y evidencia visual:** Playwright cubre landmarks, foco,
   targets mínimos, reduced-motion, viewport móvil/desktop, escala 125%,
   `forced-colors` y el ciclo de foco del `alertdialog`; `npm run
   accessibility:visual` genera capturas canónicas reproducibles y
   `accessibility:check` verifica su contrato/hash. I8 añade la misma evidencia
   desde el binario release mediante `accessibility:release` y
   `accessibility:release:check`. Falta ejecutar lector de pantalla y validar
   en hardware real de Windows High Contrast.
5. **Baseline de rendimiento:** conservar el objetivo de startup <8 s, mantener
   cleanup 100 % repetible y comparar contra `sistema anterior`; `npm run smoke:cdp`
   ya aplica 512 MiB de working set/256 MiB de memoria privada al árbol nativo y
   registra duraciones IPC por operación, mientras `npm run perf:benchmark` cubre
     una muestra CLI de 100 MiB. v0.47 añadió tres iteraciones sostenidas, v0.48
     añadió límites de duración y stress durable, y v0.49 mide tres ciclos nativos
     de transformación/exportación dentro de WebView2 junto al presupuesto global
     del árbol. Todavía falta repetirlo con datasets grandes y con historial que
     ejerza el límite integral de memoria. El perfilado de columnas ya distribuye
     el trabajo entre hasta cuatro trabajadores para frames materializados; desde
     v0.66 el perfil source-backed procesa duplicados, columnas y resúmenes por
     bloques, ordena estadísticas numéricas en disco y limita la correlación a
     su muestra. Queda ampliar la lectura lazy/incremental a transformaciones,
     validación/exportación y medirla dentro de WebView2 con datasets grandes.
6. **Ejecución lazy/incremental y arranque:** la receta compatible de I1 usa
   planes Polars lazy con fallback eager; en v0.54.0 las etapas Review,
   Preparar y Entregar también se cargan bajo demanda, la migración SQLite se
   difiere hasta la primera operación y el monitor de recursos espera al primer
   paint. CSV, TSV y TXT delimitado ya se leen con `LazyCsvReader`, y Parquet
   con `scan_parquet`; ambos usan el motor streaming de Polars, baja memoria y
   sin `rechunk` paralelo. Ampliar después a datasets mayores y a operaciones
   que todavía requieren el camino eager. Desde v0.65.0, CSV, TSV, TXT
   delimitado y Parquet de al menos 512 MiB abren source-backed con esquema,
   primera página y conteo desde disco, sin retener todas las filas en el
   `DataFrame`; desde v0.66 el perfilado también recorre snapshots Parquet por
   bloques y columnas, con derrame de firmas y corridas numéricas temporales;
   desde v0.67 la validación de reglas fila-a-fila y de esquema/conteo también
   recorre bloques sin materializar la fuente. Transformaciones, exportación y
   reglas globales todavía no compatibles materializan bajo demanda con
   validación de tamaño y conteo; desde v0.70 la validación source-backed de
   reglas globales de unicidad, monotonicidad, agregados y deriva también
   recorre la fuente por bloques con cubetas/acumuladores temporales; desde
   v0.80 las recetas source-backed compuestas únicamente por renombres y `keepColumns`
   proyectan directamente desde la fuente a un Parquet privado administrado,
   conservando esquema, conteo, orden y preview sin llenar el `DataFrame`; desde
   v0.92 esa ruta también combina hasta tres filtros con selección/renombrado,
   aplica el filtro en DuckDB sobre la fuente y conserva el conteo exacto de
   filas eliminadas y la semántica de nulos; desde v0.93 añade casts, parseo de
   fechas fijas `YMD`/`DMY`/`MDY` y cálculos de suma, resta, multiplicación o
   concatenación; desde v0.94 añade partes de fecha `Year`/`Month`/`Day` tras
   un parseo fijo sin filtros previos; desde v0.95 permite reemplazo literal
   después del filtro y antes de proyectar, con conteo exacto de celdas cambiadas;
   desde v0.96 también une de dos a dieciséis columnas de texto sobre la fuente,
   conserva nulos, cadenas vacías, separador y orden, y respeta
   `keepColumns`/`dropSources` con casts numérico→texto; desde v0.97 también
   divide una columna de texto en dos a dieciséis destinos, conserva el resto
   final, segmentos vacíos, delimitadores Unicode y `dropSource`, y puede
   encadenar una unión posterior.
   Desde v0.98 también extrae tokens, dígitos, letras Unicode y segmentos antes
   o después de delimitadores; desde v0.99 normaliza correo, teléfono y
   dirección; y desde v0.100 agrupa y resume la fuente con `sum`, `mean`,
   `min`, `max`, `count` y `count_unique` directamente en DuckDB; desde v0.101
   también aplica tratamientos IQR `cap`, `drop` e `impute` sobre columnas
   numéricas, con baseline común posterior a filtros; desde v0.102 interpreta
   fechas ISO sin offset o con sufijo UTC `Z` directamente sobre la fuente.
   Estas rutas conservan el orden de primer grupo, agrupan nulos, validan
   precisión, overflow, finitud y umbrales, y publican contadores separados de
   grupos, filas colapsadas, celdas ajustadas y filas retiradas. Los offsets
   distintos de UTC, fechas inválidas o combinaciones no seguras todavía
   materializan bajo demanda.
   Desde
   v0.68 la exportación Parquet sin receta
   ni privacidad adicional puede convertir la fuente directamente desde disco
   con publicación atómica; desde v0.69 JSON sin receta ni privacidad adicional
   también se convierte directamente desde la fuente con DuckDB y publicación
   atómica. Recetas y exportaciones con reglas globales siguen pendientes de su
   ruta incremental equivalente. Desde v0.117, las protecciones `mask` y `hash`
   para exportaciones source-backed se aplican como una proyección DuckDB sobre
   un snapshot Parquet privado y reutilizan la misma transmisión para CSV,
   JSON, Parquet, SQL, Excel, SQLite y Bundle; se preservan nulos, columnas
   protegidas, cancelación y validación final de la fuente original.
   Desde v0.118, los reemplazos regex con grupos numéricos `$1`–`$9` y
   sustitución global también se ejecutan source-backed en DuckDB; las formas
   de sustitución no compatibles conservan el fallback eager.
   Desde v0.119, las divisiones calculadas con operandos literales o columnas
   validan ceros antes de publicar el snapshot y también conservan el fallback
   eager para entradas no compatibles. Desde v0.120, guardar un proyecto con
   una fuente source-backed CSV/TSV/TXT delimitada o Parquet convierte la fuente
   directamente a `current.parquet` mediante DuckDB, verifica el conteo real y
   mantiene el `DataFrame` activo en modo esquema-only; los proyectos normales
   conservan la ruta eager y su historial.
   Desde v0.121, el parseo de fechas y la extracción de `year`, `month` o `day`
   también pueden seguir a filtros dentro de la misma receta source-backed,
   respetando el orden de etapas y la paridad con eager. Desde v0.122, los
   filtros ordenados de columnas `Date` y `Datetime` aceptan literales ISO 8601
   directamente en eager, Polars lazy y DuckDB source-backed, sin convertirlos
   a números ni materializar la fuente compatible. Desde v0.123, una receta
   lazy también puede combinar parseo de fecha, filtros ISO 8601 y extracción
   de `year`, `month` o `day` en ese orden, sin degradar a la ruta eager.
   Desde v0.124, `Eq` y `Neq` usan el valor temporal real para columnas `Date`
   y `Datetime` en las tres rutas, sin depender de la unidad interna o del
   texto formateado.
   Desde v0.125, la apertura diferida de `JSON`, `JSONL` y `NDJSON` grandes
   construye un snapshot Parquet privado con DuckDB, conserva esquema y
   preview sin llenar el `DataFrame` activo, y reutiliza ese snapshot para
   paginación, consultas, proyectos y recetas source-backed.
   Desde v0.126, `remove_empty_rows` puede filtrar directamente una fuente
   source-backed en DuckDB, publicar el resultado como snapshot Parquet,
   preservar `_cambios` y habilitar undo/redo por snapshots sin materializar
   el frame activo; un presupuesto insuficiente mantiene el fallback eager.
7. **DuckDB y operaciones multidataset:** la primera ruta opcional de DuckDB ya
   valida y ejecuta la consulta SQL local restringida sobre snapshots Parquet
   temporales, con paginación, orden estable, agregaciones y joins seguros.
   Desde v0.63 cada consulta configura 512 MB de memoria y hasta 8 GB de
   derrame temporal privado, y desde v0.64 la conversión de fuentes a snapshots
   comparte esa misma frontera; los planes source-backed pueden continuar
   procesando intermedios mayores que la memoria disponible. En v0.65, las
   fuentes grandes intactas también evitan la materialización inicial y pueden
   reutilizar la lectura directa de disco en paginación y consultas compatibles.
   Quedan el benchmark del motor, la validación con datasets que excedan la RAM,
   transformaciones/validación/exportación incrementales, consultas más amplias
   y destinos de base de datos.
8. **Migración M1 desde `sistema anterior`:** la vertical de contratos de calidad
   ya produce en v0.55.0 un informe con conteos, advertencias, acciones manuales
   y hash del artefacto; v0.56.0 conserva las opciones de entrega compatibles de
   pipelines y reporta sus omisiones; v0.57.0 añade fixtures sintéticas para
   pipelines, sesiones, calidad y legacy, y reporta metadatos de sesión que no
   se pueden aplicar automáticamente; quedan restauración de sesiones,
   mapeo a proyectos y round-trip hacia proyectos Columnia.
9. **Cierre de distribución:** auditorías de vulnerabilidades, secretos,
   licencias/avisos, smoke de instalador limpio, updater autenticado y validación
   real en macOS/Linux.

### Fase P1 — Paridad funcional con `sistema anterior`

- [x] Crear una matriz verificable de entradas, transformaciones, proyectos,
  visualizaciones, salidas, privacidad y escala.
- [x] Implementar exportación JSON atómica en UI, Rust, CLI, batch y proyectos.
- [x] Implementar exportación SQL atómica como script portable en UI, Rust, CLI,
  batch y proyectos.
- [x] Implementar comparación local de dos datasets, diferencias por filas y
  columnas, y consolidación opt-in con historial cuando el esquema coincide.
- [x] Incorporar exportación Excel `.xlsx` y destino local SQLite con publicación
  atómica, esquema/datos tipados, CLI, batch, proyectos, cancelación y pruebas
  de reapertura.
- [x] Añadir apertura segura de la carpeta del último output local desde Entregar:
  Rust conserva temporalmente el destino de la exportación exitosa, lo revalida
  antes de abrirlo y React no recibe la ruta.
- [x] Completar la entrega compatible con sistema anterior: PostgreSQL/MySQL/SQL Server,
  prueba de conexión y políticas de tabla mediante ODBC. La primera slice
  exige un controlador instalado, valida la cadena en memoria, prueba `SELECT 1`,
  crea/anexa/reemplaza la tabla con identificadores acotados e inserta por lotes
  dentro de una transacción. Las fuentes source-backed compatibles se transmiten
  por bloques sin materializar el `DataFrame` activo; privacidad con `mask`/`hash`
  usa un snapshot Parquet temporal cuando corresponde, mientras las combinaciones
  no compatibles mantienen fallback materializado explícito; la
  primera slice local ya publica un bundle ZIP atómico con
  dataset CSV protegido, diccionario tipado, reporte de calidad opcional y
  manifest con hashes; cuando existe una receta validada también incluye
  `recipe.json` y su hash. Excel y SQLite permanecen cubiertos por separado.
- [x] Incorporar claves explícitas, conflictos por clave y consolidación segura de
  claves nuevas.
- [x] Incorporar joins multidataset `Inner`, `Left` y `Full` por claves, con
  validación de tipos, sufijo determinista para columnas compartidas e historial.
- [x] Añadir resolución interactiva acotada de conflictos por clave: mostrar las
  celdas divergentes, exigir una decisión por conflicto y conservar la fila del
  dataset activo o la comparada con historial reversible.
- [x] Ampliar la resolución visible a una combinación independiente por
  columna/valor, manteniendo compatibilidad con decisiones legacy por fila,
  historial reversible y bloqueo cuando faltan celdas por decidir.
- [x] Resolver conflictos fuera del límite visible sin ocultar decisiones
  pendientes: Review pagina conflictos en bloques de 50, mantiene índices globales
  y bloquea la resolución hasta completar todas las páginas.
- [x] Añadir visualizaciones de análisis con tabla accesible equivalente para
  completitud y posibles outliers.
- [x] Añadir la primera lectura agregada del catálogo de limpieza: duplicados,
  columnas incompletas/constantes, desajustes de tipo y posibles nombres de PII,
  con acciones seguras existentes y sin mostrar celdas.
- [x] Añadir eliminación explícita de filas completamente vacías (nulos o texto
  en blanco), con impacto contado, orden estable, historial reversible y control
  accesible en Preparar.
- [x] Añadir eliminación explícita y reversible de columnas constantes detectadas
  en el perfil, con nombres e impacto reportados, exclusión de columnas totalmente
  nulas y conservación de al menos una columna utilizable.
- [x] Añadir eliminación explícita y reversible de columnas completamente vacías,
  diferenciada de las constantes, con reporte de nombres/impacto y conservación
  de al menos una columna utilizable.
- [x] Añadir tratamiento explícito de columnas con alta nulidad usando un umbral
  visible del 80%, excluyendo columnas 100% nulas, con impacto, historial reversible
  y conservación de al menos una columna utilizable.
- [x] Detectar valores centinela textuales conocidos en el perfil y permitir su
  conversión reversible a nulos, con conteo por columna, acción accesible y
  exclusión de tipos no textuales y de `_cambios`.
- [x] Normalizar alias booleanos textuales (`yes`/`no`, `sí`/`no`, `true`/`false`)
  de forma reversible, conservando tokens no reconocidos y mostrando el impacto.
- [x] Detectar duplicados parecidos de forma agregada, excluyendo los exactos,
  normalizando mayúsculas, espacios y acentos sin eliminar filas automáticamente.
- [x] Intentar imputación conservadora y reversible de nulos: moda textual con
  evidencia repetida y mediana numérica observada, preservando tipos y `_cambios`.
- [x] Clasificar señales agregadas de privacidad por nombre de columna (correo,
  teléfono, dirección, identificador y nombre) sin exponer valores en el perfil.
- [x] Activar una columna reservada `_cambios` para trazabilidad local por fila;
  las mutaciones posteriores conservan/añaden la etiqueta de operación y la
  columna queda protegida de la limpieza textual general.
- [x] Detectar y reparar doble codificación UTF-8 heredada (`Ã©`, `â€™`) con
  conteo agregado por columna, una acción reversible y una decodificación
  cerrada que no modifica números, `_cambios` ni valores ambiguos.
- [x] Apartar con confirmación los valores de texto incompatibles con una sugerencia
  semántica que alcance al menos 90% de coincidencia, convirtiéndolos en nulos sin
  mostrar celdas y conservando la reversión desde el historial.
- [x] Imputar outliers detectados por IQR con la mediana observada, preservando el
  tipo numérico y `_cambios`, con acción directa y tratamiento seleccionable en
  recetas, impacto agregado y reversión desde el historial.
- [x] Completar nulos textuales con `Desconocido` como acción explícita,
  reversible y separada de la imputación conservadora por moda; excluye números
  y `_cambios` y reporta solo impacto agregado.
- [x] Proteger con confirmación los valores no nulos de columnas personales
  detectadas (correo, teléfono, dirección y nombre), sustituyéndolos por
  `[REDACTED]` sin tocar identificadores, números, nulos ni `_cambios`; la
  operación conserva las columnas, reporta solo impacto agregado y puede
  revertirse desde el historial.
- [x] Interpretar como `Datetime` las columnas de texto con una fecha dominante
  en un formato cerrado y cobertura segura; omitir formatos ambiguos, preservar
  nulos y ofrecer una acción directa reversible con impacto agregado.
- [x] Convertir como números las columnas de texto con más de 90% de coincidencia
  numérica, rechazando pérdida de precisión y preservando identificadores o
  códigos con ceros iniciales mediante una acción directa reversible.
 - [x] Completar la migración del catálogo de limpieza sugerida: las 22
   operaciones registradas por sistema anterior tienen acción directa reversible o replay
   determinista seguro en Columnia. Los identificadores y el PII personal
   (correo, teléfono, dirección y nombre) tienen retiro/protección explícitos;
   la eliminación difusa confirma el impacto, conserva la primera fila/orden y
   las copias exactas, y mantiene el límite de 5.000 filas de sistema anterior. También
   están cubiertos vacíos, constantes, alta nulidad, centinelas, imputación
   conservadora, booleanos, fechas con formato dominante cerrado, auditoría
   `_cambios` y las tres estrategias IQR (`cap_outliers`, `impute_outliers`,
   `drop_outliers`), que se validan como mutuamente excluyentes. Las operaciones
   desconocidas o modos hash/clave explícita de PII siguen siendo advertencias
   específicas y no se aproximan silenciosamente.
- [x] Migrar `selected_cleaning_operations` para que los aliases de las limpiezas
  deterministas se normalicen, se conserven en el informe de sesión y se
  reproduzcan desde la fuente en el orden fijo, incluyendo las estrategias IQR
  `cap_outliers`, `impute_outliers`, `drop_outliers`, `mask_pii` y
  `drop_fuzzy_duplicates` en sus modos locales seguros; los modos hash/clave
  explícita y las operaciones sin equivalente reversible permanecen como
  advertencias específicas para revisión manual.
- [x] Añadir una visualización accesible de distribución numérica tipo boxplot
  usando mínimo, cuartiles, mediana y máximo, con tabla exacta equivalente.
- [x] Añadir una primera visualización de histogramas numéricos por intervalos,
  con límites estables al persistir perfiles y tabla de frecuencias equivalente
  para teclado y lector de pantalla.
- [x] Añadir una primera matriz de correlaciones numéricas de Pearson, acotada a
  doce columnas y una muestra de hasta 100.000 filas, con tabla accesible y
  cancelación cooperativa; las columnas sin variación muestran un valor no
  disponible en vez de inventar una relación.
- [x] Añadir una lectura agregada de cobertura temporal para columnas Date,
  Datetime, Timestamp y fechas detectadas, con rango mínimo/máximo, cobertura,
  filas con valor y tabla accesible equivalente sin exponer celdas.
- [x] Ampliar visualizaciones y análisis exploratorio: perfil de columnas,
  distribuciones, histogramas/boxplots, correlaciones, grupos, patrones de
  nulos, validación de formatos, centinelas, casi duplicados, completitud,
  calendario, tendencias y series temporales, siempre con tabla accesible equivalente. Las
  primeras ampliaciones ya incluyen ranking de patrones de nulos, validación
  visual de formatos, grupos categóricos, cobertura temporal, tendencias por
  día/mes/año y una matriz de correlaciones de Pearson acotada, todas con tabla
  equivalente; el calendario diario dedicado y la serie de línea con métrica de
  filas/porcentaje ya están disponibles.
- [x] Añadir tendencia temporal diaria para rangos de hasta 90 días, con días
  sin filas visibles, payload acotado, cancelación cooperativa y tabla accesible
  equivalente; los rangos mayores conservan la agregación mensual o anual.
- [x] Extender los contratos de calidad con la primera slice v3: `allowed_values`,
  `regex`, `dtype`, unicidad compuesta y `row_count`, con tolerancias, límites de
  payload, evaluación Rust, bridge tipado, editor accesible y pruebas.
- [x] Añadir `column_compare` a los contratos de calidad: operadores `eq`, `ne`,
  `lt`, `lte`, `gt` y `gte`, comparación local con nulos inválidos, tolerancias,
  migración segura desde sistema anterior, bridge tipado, editor accesible y pruebas.
- [x] Añadir `date_range` a los contratos de calidad: límites `minDate`/`maxDate`,
  parseo seguro de texto/Date/Datetime, valores nulos o ilegibles inválidos,
  migración sistema anterior, bridge tipado, editor accesible y evaluación compartida.
- [x] Añadir `conditional` a los contratos de calidad con condiciones `eq`, `ne`,
  `lt`, `lte`, `gt` y `gte`, subreglas fila-a-fila seguras, tolerancia exterior,
  migración sistema anterior, bridge tipado, editor accesible y evaluación compartida.
- [x] Añadir `schema_contract` con columnas requeridas, control de columnas
  adicionales y orden opcional, migración sistema anterior, bridge tipado, editor
  accesible y evaluación estructural compartida.
- [x] Añadir `referential_integrity` con referencias locales explícitas para
  claves simples y compuestas, migración segura desde sistema anterior, bridge tipado,
  editor accesible, tolerancias y evaluación compartida sin exponer valores.
- [x] Añadir `monotonic` con direcciones no decreciente/no creciente,
  tolerancias por inversión, migración segura desde sistema anterior, bridge tipado,
  editor accesible y evaluación compartida; los nulos reinician la cadena.
- [x] Añadir `aggregate_check` y `aggregate_reconciliation` para conteo, suma,
  mínimo, máximo y reconciliación de dos columnas, con `expected`/referencias,
  tolerancias absolutas/relativas, migración sistema anterior, bridge tipado, editor
  accesible y resultados privados basados en conteos.
- [x] Añadir `distribution_drift` con línea base numérica, comparación de medias,
  umbral/tolerancia absoluta, exclusión de nulos y textos no numéricos, migración
  sistema anterior, bridge tipado, editor accesible y resultados privados basados en
  conteos.
- [x] Añadir versionado/compatibilidad explícita del documento de reglas de
  calidad: formato canónico Columnia v1, guardado atómico, selector nativo,
  compatibilidad con sistema anterior v1–v3 y legado v1/sin versión, rechazo seguro de
  formatos/campos/versiones futuras y contrato IPC sin rutas.
- [x] Migrar la primera slice del optimizador de transformaciones: recomendaciones
  no destructivas, preview antes/después, riesgo/confianza y alternativas de
  recuperación visibles antes de aplicar una receta. Quedan automatización de
  calidad y optimización global del plan para una siguiente iteración.
- [x] Añadir consulta SQL local restringida de solo lectura sobre `dataset`, con
  proyección, `LIMIT`/`OFFSET`, presupuesto de caracteres, resultado paginado y
  tabla accesible; no permite escritura ni rutas desde React.
- [x] Ampliar la consulta local con filtros simples (`=`, desigualdad, orden
  numérico, `IS NULL`/`IS NOT NULL`) y agregaciones acotadas (`COUNT`, `SUM`,
  `AVG`, `MIN`, `MAX`), con presupuesto y resultados tabulares seguros.
- [x] Añadir `GROUP BY` de una columna con orden estable, grupos nulos y
  paginación segura sobre agregaciones.
- [x] Extender `GROUP BY` local a hasta ocho columnas compuestas, preservando
  orden estable, claves nulas, validación de duplicados y presupuesto de filas.
- [x] Extender los `JOIN` locales a hasta ocho pares de columnas clave, con
  validación de duplicados, tipos compatibles, cardinalidad y orden estable.
- [x] Procesar las agregaciones SQL locales por bloques, conservando solo estados
  de agregación y grupos en la segunda pasada, sin retener índices de filas
  coincidentes fuera del presupuesto explícito.
- [x] Particionar los índices exactos de comparación por claves y de firmas completas
  de filas en cubetas temporales, procesando resumen, multiconjuntos, nuevas claves
  y conflictos paginados por una cubeta a la vez sin retener mapas globales en memoria.
- [x] Particionar el preflight de cardinalidad de los `JOIN` locales, contando por
  cubeta los productos de duplicidad con cancelación cooperativa y límites explícitos.
- [x] Procesar `JOIN` locales `INNER`/`LEFT` sin agregación por bloques del lado
  `dataset`, conservando orden, filtros y paginación globales sin acumular el
  `DataFrame` unido completo.
- [x] Procesar agregaciones de `JOIN` locales `INNER`/`LEFT` por bloques,
  fusionando estados de `COUNT`/`SUM`/`AVG`/`MIN`/`MAX` y grupos en orden estable;
  `FULL` también recorre el lado activo por bloques y agrega después bloques de
  filas derechas no emparejadas mediante un anti-join estable, dentro de los
  límites explícitos.
- [x] Endurecer la consulta SQL local con cancelación cooperativa, límite de
  filas coincidentes para agregaciones y preflight de cardinalidad para evitar
  materializar joins many-to-many fuera de presupuesto.
- [x] Añadir una primera ruta DuckDB opcional para la consulta local restringida:
  el bridge selecciona el motor, Rust valida el contrato existente, reutiliza
  los snapshots Parquet administrados de la revisión actual y la comparación
  cuando existen y usa snapshots temporales como fallback, conservando conteo
  exacto, paginación, orden estable, agregaciones y JOIN `INNER`/`LEFT`/`FULL`
  con claves coalescidas.
- [x] Ampliar la familia de recetas lazy para combinar parseos de fecha y
  conversiones en columnas distintas, y para ejecutar `split` y `merge` en una
  misma receta cuando sus dependencias se conservan; los conflictos de fuentes
  siguen produciendo un rechazo explícito o fallback eager seguro.
- [x] Servir la paginación de la muestra activa desde el snapshot Parquet del
  cursor actual mediante `slice` y colección streaming cuando el historial está
  habilitado; cuando el historial está degradado pero la fuente original
  Parquet o CSV/TSV/TXT permanece intacta, leer solo la página solicitada desde
  disco y conservar el fallback al `DataFrame` ante cambios o formatos no
  compatibles.
- [x] Completar consulta con joins y DuckDB para datasets source-backed grandes,
  dentro de límites explícitos. La
  versión 0.105.2 corrige el orden global de `FULL JOIN` cuando el esquema
  source-backed activo está vacío en memoria: el plan recibe el conteo real de
  filas y mantiene las filas derechas no emparejadas después de las activas,
  sin materializar el dataset para preparar la consulta. El benchmark reproducible
  de `npm run perf:duckdb:join` procesa 537.286.551 bytes y 1.810.432 filas;
  `INNER`, `LEFT` y `FULL` conservan sus conteos/páginas, el frame activo queda
  vacío, el working set máximo observado es 233.816.064 bytes y el cleanup pasa.
  La versión 0.107.0 añade una ruta parcial `DataFrame` activo + snapshot
  comparado Parquet: evita materializar de nuevo todas las filas de la
  comparación cuando el activo ya está transformado en memoria, y cubre el
  camino explícito DuckDB y la promoción automática desde Polars. La
  preparación también lee solo el esquema del snapshot comparado y no hereda
  el límite de entradas del plan Polars; las consultas Polars simples sin comparación ya recorren por
  bloques el snapshot Parquet del cursor y conservan fallback ante snapshots
  degradados o inconsistentes. Los `JOIN` y el `DataFrame` activo todavía no
  constituyen una ejecución completa fuera de RAM y requieren esta expansión.
  Cuando el historial se degrada por presupuesto pero la fuente original CSV,
  TSV, TXT delimitada o Parquet sigue intacta, la selección Polars intenta
  primero el mismo contrato restringido mediante DuckDB desde disco, incluidos
  los JOINs con un snapshot comparado, y vuelve al `DataFrame` solo si la
  consulta o la fuente no son compatibles.
  Con snapshots administrados válidos, la misma promoción se aplica a todos los
  JOIN compatibles, no solo a los que superan el umbral de entradas; así la ruta
  Polars no materializa innecesariamente ambos datasets antes de consultar.
  La ruta DuckDB configura por operación un límite de 512 MB y un directorio
  privado de derrame temporal de hasta 8 GB, con cleanup al finalizar; el
  benchmark confirma esta ruta concreta bajo ese presupuesto. La ejecución
  incremental de todas las operaciones, el presupuesto global de la aplicación
  y los datasets no compatibles siguen abiertos.
  La versión 0.143.0 amplía la resolución source-backed de conflictos por clave:
  valida por bloques solo índices y columnas divergentes, sin materializar los
  valores de todos los conflictos, y eleva el límite explícito a 8.192. La
  versión 0.142.0 amplía la resolución para activos source-backed cuando la
  comparación completa cabe en 2.048 conflictos; DuckDB aplica decisiones por
  fila o por columna sobre snapshots Parquet, publica un resultado reversible y
  conserva el frame esquema-only. Los casos fuera de presupuesto o incompatibles
  mantienen fallback eager. La versión 0.141.0 evita materializar el dataset activo al abrir la página de
  conflictos: genera o reutiliza una fuente Parquet temporal, calcula los
  conflictos por bloques y conserva esquema-only. La versión 0.140.0 amplía la consolidación
  multidataset: cuando el activo
  conserva una fuente source-backed y la comparación está en su snapshot
  Parquet, DuckDB valida duplicados y conflictos por clave, calcula únicamente
  las claves nuevas, preserva el orden y publica un cursor Parquet reversible
  sin cargar ambos datasets completos en el `DataFrame`. Los formatos
  incompatibles mantienen el fallback eager. La versión 0.139.0 amplía la
  mutación multidataset: cuando el activo y el
  comparado son source-backed y el comparado es CSV/TSV/TXT delimitado o
  Parquet, `INNER`, `LEFT` y `FULL` se ejecutan sobre ambas fuentes, se cuenta
  el resultado antes de escribir, se publica únicamente el Parquet resultante
  y el cursor queda en historial reversible. Los formatos comparados
  incompatibles conservan el fallback eager. Esta entrega cierra la ruta
  concreta de JOIN source-backed local, pero el benchmark sostenido, la
  validación con datos reales y la ejecución fuera de RAM de todas las demás
  operaciones siguen abiertos.
  La versión 0.108.0 incorpora `npm run perf:duckdb:join`: una corrida
  reproducible de una fuente CSV temporal de 512 MiB y 1.810.432 filas ejecuta
  un `LEFT JOIN` directo desde DuckDB, conserva el frame activo vacío y valida
  conteo, paginación, working set de 512 MiB y cleanup. Esta evidencia cubre
  la ruta source-backed concreta, pero no cierra el presupuesto global de la
  aplicación ni la ejecución integral fuera de RAM.
  La versión 0.109.0 conserva el esquema y la primera página de apertura con
  Polars, pero mueve el conteo total de la fuente a `COUNT(*)` de DuckDB con
  cancelación cooperativa; el benchmark end-to-end confirma que el frame sigue
  vacío y que el conteo coincide con la fuente. La ejecución integral del
  `DataFrame` fuera de RAM todavía requiere una expansión separada.
  La versión 0.110.0 extiende la exportación source-backed sin receta ni
  privacidad adicional a CSV: DuckDB convierte CSV/TSV/TXT delimitado o Parquet
  a una salida temporal, neutraliza prefijos de fórmulas, comprueba cancelación
  y cambios de la fuente y publica atómicamente. Las reglas de calidad no
  incrementales y las demás salidas continúan usando el camino materializado.
  La versión 0.111.0 extiende la misma frontera a SQL: DuckDB lee la fuente
  directamente y escribe un script portable con esquema, literales escapados,
  transacción y publicación atómica, sin materializar el `DataFrame` activo.
  Las recetas, la privacidad adicional, las reglas no incrementales y los
  destinos de base de datos continúan en el camino materializado.
  La versión 0.112.0 endurece las consultas source-backed: `INNER`, `LEFT` y
  `FULL JOIN` se validan y ejecutan desde DuckDB con la fuente en disco, y si
  el plan no es compatible se rechaza la materialización implícita en vez de
  ocultar un pico de memoria. El benchmark de 512 MiB confirma la ruta, pero
  la ejecución integral fuera de RAM de todas las operaciones sigue pendiente.
  La versión 0.113.0 extiende la misma frontera a Bundle ZIP cuando no hay
  receta, privacidad adicional ni reglas de calidad no incrementales: DuckDB
  escribe `dataset.csv` desde la fuente, calcula los nulos del diccionario por
  agregación, y el paquete publica manifest y hashes junto con el reporte de
  calidad incremental opcional. La versión 0.138.0 elimina la excepción de la
  receta: un Bundle source-backed también conserva `recipe.json`, su referencia
  y hash sin materializar; Excel, SQLite, reglas no incrementales y la
  ejecución integral fuera de RAM siguen pendientes.
  La versión 0.114.0 añade un gate de marca retirada que inspecciona el árbol
  activo, incluyendo archivos no ignorados, y evita regresiones de nomenclatura
  en código y documentación; no reescribe el historial Git anterior.
  La versión 0.115.0 extiende la exportación source-backed a Excel `.xlsx` y
  SQLite para fuentes CSV/TSV/TXT delimitadas o Parquet: DuckDB transfiere las
  filas por streaming, conserva esquema y tipos, y publica ambos destinos con
  cancelación, validación de cambios y atomicidad. Las recetas, la privacidad
  adicional, las reglas no incrementales y la ejecución integral fuera de RAM
  siguen usando rutas materializadas o requieren trabajo específico.
  La versión 0.116.0 extiende la apertura source-backed a libros XLSX/XLSB
  grandes: Calamine detecta el esquema y escribe un snapshot Parquet temporal
  por bloques, manteniendo vacío el `DataFrame` activo. Paginación, perfilado,
  consultas DuckDB y exportaciones compatibles usan ese snapshot y validan que
  el libro original no haya cambiado; XLS/ODS conservan el fallback materializado.
  Desde v0.65.0, la apertura de fuentes CSV/TSV/TXT delimitadas y Parquet de al
  menos 512 MiB conserva solo esquema, primera página y conteo; las consultas
  y páginas compatibles pueden seguir en disco, mientras las operaciones que
  necesitan filas completas materializan con validación.
  La comparación inicial de fuentes Parquet, CSV/TSV/TXT delimitadas y JSON
  reutiliza el snapshot Parquet administrado del activo cuando existe; si no,
  convierte una fuente original intacta compatible a un snapshot temporal y
  compara ambos lados por bloques. Los libros XLSX/XLSB también generan
  snapshots Parquet por bloques mediante el lector secuencial de Calamine.
  XLS/ODS y las fuentes incompatibles conservan el fallback materializado.
  Cuando el historial está degradado, el dataset está intacto y la fuente original es CSV, TSV, TXT delimitado o Parquet, DuckDB ya
  puede leerla directamente desde disco; un `JOIN` puede combinarla con el
  snapshot Parquet de la comparación sin reserializar el activo. Si la fuente
  cambió, desapareció, usa JSON/XLS/ODS o el dataset ya fue transformado, se
  conserva el fallback materializado seguro.
  La paginación de conflictos sobre un snapshot Parquet comparado también
  recorre bloques de 16K, conserva un índice temporal global de claves para
  mantener la semántica de duplicados y retiene solo una página y un bloque de
  valores al construir la respuesta; XLS/ODS y las fuentes que no puedan
  validarse conservan el fallback materializado dentro de sus límites
  explícitos.
- [x] Añadir detección, enmascarado/hash SHA-256 y modos de privacidad visibles
  para columnas personales detectadas durante la exportación.
- [x] Extender detección, enmascarado/hash y modos de privacidad visibles a los
  seis destinos locales actuales (CSV, JSON, Parquet, SQL, Excel y SQLite),
  incluidos identificadores numéricos, devolviendo al usuario las columnas
  protegidas sin exponer sus valores.
- [x] Añadir una frontera común de sanitización para los JSON de la CLI que
  contienen reportes, recetas o manifiestos: redacta rutas y referencias de
  filesystem incrustadas y conserva únicamente identificadores visibles y
  conteos agregados.
- [x] Extender la sanitización común a recetas, reports y manifests locales,
  incluyendo valores, emails, referencias incrustadas y errores largos sin
  filtrar metadatos sensibles.
- [x] Extender esa frontera a conectores remotos con contratos de privacidad
  equivalentes. La entrega ODBC a PostgreSQL, MySQL y SQL Server aplica `none`,
  `mask` o `hash`, no persiste la cadena de conexión y protege mediante un
  snapshot Parquet temporal solo cuando la transmisión directa no puede aplicar
  la transformación de privacidad.
- [ ] Ampliar lazy/incremental a operaciones y datasets que exceden la memoria:
  Parquet cacheado, chunks, comparación/joins grandes, historial degradado y
  presupuestos explícitos sin materialización silenciosa. La carga de CSV/TSV/TXT,
  Parquet y la primera familia de recetas compatibles ya comparten colección
  Polars con motor `streaming`; la apertura y validación de snapshots Parquet
  durables también reutiliza esa frontera, y el historial copia sus snapshots
  entrada por entrada, valida solo footer/esquema para revisiones no cursor y
  materializa únicamente el cursor para comprobar consistencia, dejando la
  lectura de filas restante a undo/redo. La comparación completa de
filas y la comparación por claves particionan sus firmas exactas en 256 cubetas
temporales y procesan multiconjuntos, resumen, nuevas claves y conflictos
paginados por una cubeta a la vez, sin retener mapas globales en memoria. La
comparación inicial de fuentes Parquet, delimitadas, JSON y XLSX/XLSB también
puede construir sus índices y conflictos leyendo bloques de 16K desde el
snapshot; si el activo tiene un snapshot administrado o una fuente original
compatible intacta, reutiliza también ese lado sin clonar su `DataFrame`; XLS/ODS
y fuentes incompatibles mantienen materialización. Los JOIN locales por claves ya ejecutan el
plan Polars con motor `streaming` después de su preflight; ese preflight también derrama las
  claves y cuenta por cubeta los productos de duplicidad con cancelación, pero su resultado sigue dentro
  de los límites explícitos. Los `JOIN` `INNER`/`LEFT` sin agregación procesan el
  lado `dataset` por bloques y conservan solo la página global, su conteo y un
  bloque unido temporal; las agregaciones `INNER`/`LEFT` fusionan estados sin
  conservar el resultado unido completo. `FULL` recorre el lado activo por
  bloques y visita las filas derechas no emparejadas mediante un índice temporal
  de claves y bloques de 16K, sin materializar el anti-join derecho completo;
  conserva la semántica SQL de nulos, duplicados y orden de entrada.
  La paginación de conflictos sobre un snapshot Parquet comparado también recorre
  bloques de 16K, conserva un índice temporal global de claves para mantener la
  semántica de duplicados y retiene solo una página y un bloque de valores al
construir la respuesta; XLS/ODS y fuentes inconsistentes mantienen
materialización dentro de sus límites explícitos. Esta optimización de
comparación no equivale a ejecución fuera de memoria general. La
  consulta Polars simple sin comparación
  también lee el snapshot Parquet del cursor por bloques de 16K filas, cuenta
  coincidencias y conserva solo la página o los acumuladores; si el snapshot
  falla vuelve al frame activo. Quedan fuera de esta slice los `JOIN` sobre un
  dataset activo materializado, las fuentes comparadas incompatibles, las
  operaciones generales y el presupuesto integral fuera de RAM. Desde v0.80,
   las recetas source-backed compuestas únicamente por renombres y
   `keepColumns` proyectan directamente a un Parquet privado administrado,
   conservando el esquema, conteo, orden y preview sin llenar el `DataFrame`;
   desde v0.92 también pueden aplicar hasta tres filtros con esa misma frontera
   source-backed; desde v0.93 las recetas source-backed también aplican casts, parseo de fechas
   fijas `YMD`/`DMY`/`MDY` y cálculos de suma, resta, multiplicación o
   concatenación directamente con DuckDB; desde v0.94 también extraen partes
   de fecha `Year`/`Month`/`Day` después de un parseo fijo sin filtros previos;
   desde v0.95 también ejecutan reemplazo literal sobre la fuente después de
   filtros, con conteo exacto de celdas modificadas; desde v0.96 también pueden
   unir columnas de texto sobre la fuente, conservando nulos, cadenas vacías,
   orden, separador y `dropSources`; desde v0.97 la división literal también
   conserva delimitadores Unicode, segmentos vacíos, nulos, resto final y
   `dropSource`; desde v0.98 también ejecutan las seis extracciones textuales
   de tokens, dígitos, letras y segmentos antes/después de delimitadores
   literales, conservando Unicode, nulos, coincidencias ausentes y resultados
   vacíos; desde v0.99 también normalizan correo, teléfono y dirección en
   DuckDB, con conteo exacto de celdas y extracciones posteriores sobre los
   valores normalizados. Desde v0.102 los parseos ISO sin offset o con sufijo
   UTC `Z` también se ejecutan en DuckDB; desde v0.118 los reemplazos regex
   globales seguros con grupos `$1`–`$9` y desde v0.119 las divisiones
   calculadas con validación previa también conservan la ruta source-backed;
   desde v0.131 el recorte de espacios, la normalización de texto y los
   valores centinela también se proyectan sobre la fuente con conteos exactos
   y snapshots reversibles; desde v0.132 la normalización de booleanos
   clasifica candidatos en DuckDB con el umbral eager del 90% y transforma
   únicamente alias reconocidos; desde v0.133 la conversión numérica y la
   interpretación de fechas detectadas también calculan estadísticas y
   proyectan resultados en DuckDB, conservando códigos con ceros iniciales,
   precisión, tolerancia de nulos y rango de años. Desde v0.150, la separación
   de tipos incompatibles también infiere sobre la fuente efectiva y publica
   el snapshot solo cuando la validación conserva el umbral eager. Desde
   v0.151, la corrección de codificación repara secuencias mojibake
   inequívocas en DuckDB y deriva al fallback eager cuando aparece un valor
   ambiguo. Las operaciones no compatibles conservan materialización. Desde
   v0.134, las imputaciones conservadora y categórica
   también calculan sus reemplazos y proyectan nulos directamente en DuckDB,
   preservando el desempate de la moda, la mediana inferior, `Desconocido` y
   los tipos físicos compatibles. Desde v0.135, las acciones directas de
   outliers `cap`, `impute` y `drop` calculan cuantiles, límites y reemplazos
   en DuckDB, cuentan filas/celdas afectadas y publican snapshots reversibles
   sin llenar el frame activo; las fuentes incompatibles conservan fallback.
   Desde v0.136, la eliminación de duplicados parecidos conserva repeticiones
   exactas, la primera aparición de cada clave normalizada y el orden original
   directamente en DuckDB; las correcciones recomendadas combinan trim y
   renombres en una sola proyección source-backed.
   Desde v0.137, las exportaciones source-backed conservan esta frontera
   también con privacidad `mask`/`hash`: DuckDB genera un snapshot privado,
   valida la fuente original antes y después y transfiere todos los destinos
   locales sin llenar el frame activo.
   `keep_columns` también puede proyectar dentro de
  una receta lazy/streaming y comprueba dependencias calculadas antes de
  materializar. La búsqueda/reemplazo literal sobre texto también cuenta sus
  cambios con una agregación streaming separada y preserva nulos, renombres y
   casts a texto; el modo regex seguro conserva grupos de captura y valida el
   patrón antes de construir el plan. La división calculada source-backed
   rechaza ceros antes de escribir y mantiene nulos como nulos. La unión de
   columnas de texto conserva el orden de fuentes,
  omite nulos y mantiene nula la fila vacía, incluyendo casts numérico→texto
  validados antes de publicar. La división literal conserva el resto en el
  último destino y rellena destinos ausentes con nulos dentro del plan
  streaming. La agrupación y los resúmenes tipados también pueden ejecutarse
  dentro del plan lazy incluso con filtros previos; la búsqueda/reemplazo
  literal se aplica antes de agrupar dentro del mismo plan y el preflight
  proyecta solo las columnas necesarias:
  preservan orden estable, claves nulas, conteo de filas, `count_unique`, tipos
  y validaciones de finitos, precisión y desbordamiento. Los datasets normales
  siguen materializados para conservar la compatibilidad de transformaciones,
  perfil e historial; desde v0.65.0 las fuentes grandes soportadas abren con
  esquema/preview source-backed y materializan al entrar en estas operaciones.
  Las normalizaciones de contactos también pueden ejecutarse en el
  plan streaming, conservando nulos y espacios Unicode junto con conteos exactos; su combinación
  con agrupación también se ejecuta dentro del plan, validando el resumen sobre
  los valores normalizados.
  Las agregaciones SQL locales procesan su segunda pasada por bloques y conservan
  solo estados de agregación y grupos, sin retener índices de todas las filas
  coincidentes; el presupuesto explícito de coincidencias sigue vigente.
  Las extracciones textuales de tokens, runs Unicode y delimitadores literales
  también se ejecutan en streaming, conservando nulos y coincidencias ausentes;
  sus columnas derivadas pueden alimentar claves y agregaciones agrupadas.
  Las calculadas de suma, resta, multiplicación, división y concatenación también
  se ejecutan en streaming, con validación previa de operandos, nulos, división
  por cero e infinitos; sus columnas derivadas también pueden alimentar claves
  y agregaciones agrupadas. La extracción de año, mes y día también se ejecuta en
  streaming sobre `Date` y `Datetime` sin zona horaria cuando no hay filtros
  previos, con preflight de fechas no representables, y sus columnas derivadas
  también pueden alimentar claves de agrupación; las columnas derivadas por
  `split` y `merge` también pueden alimentar claves y agregaciones agrupadas,
  validando la proyección posterior a las etapas estructurales; las demás fechas
  mantienen fallback eager. Los parseos explícitos `Ymd`, `Dmy` y `Mdy` ya se
  ejecutan dentro del plan lazy/streaming, conservando trim, nulos y objetivos
  `Date`/`Datetime`; `Iso8601` sin offset o con sufijo UTC `Z` también se
  ejecuta en streaming, mientras offsets distintos de UTC, valores ISO
  inválidos y zonas horarias mantienen fallback eager.
  Los tratamientos IQR (`cap`, `impute`, `drop`) sobre columnas numéricas
  también se ejecutan en streaming con conteos exactos después de filtros y
  etapas source-backed compatibles, calculando los umbrales sobre las filas
  supervivientes; las etapas no compatibles o las dependencias eliminadas
  mantienen fallback eager.
  Las recetas pueden combinar parseos de fecha con conversiones en columnas
  distintas, y `split` con `merge` cuando ninguna etapa descarta una fuente aún
  necesaria; las dependencias incompatibles conservan rechazo o fallback eager.
  Los tratamientos IQR también pueden combinarse con `keep_columns` cuando la
  proyección conserva todas sus columnas fuente; si la proyección descarta una
  dependencia, la receta mantiene el fallback eager y su validación cerrada.
  La muestra paginada también puede leerse desde el snapshot Parquet del cursor
  actual con `slice`, sin volver a construirla desde todas las filas; si el
  historial está degradado, se conserva la ruta de `DataFrame`.
  La publicación durable de proyectos entrega al escritor Parquet la copia ya
  aislada del dataset y evita una segunda clonación completa durante el guardado;
  la interfaz sigue manteniendo un `DataFrame` activo como contrato. En el
  caso acotado de SQL DuckDB, si el historial se degrada por presupuesto y la
  fuente original sigue intacta, el lector registra directamente CSV/TSV/TXT
  delimitado o Parquet desde disco, incluso al unirlo con el snapshot comparado;
  una mutación invalida la referencia para no consultar el archivo obsoleto.
- [x] Exponer un presupuesto opt-in de concurrencia Rayon desde Preferencias y
  recursos: perfiles conservador/equilibrado/máximo, límite de 64 hilos,
  persistencia local y estado explícito cuando el pool ya no puede cambiarse.
- [ ] Completar la paridad de sesión operativa: muestras, preferencias, caché
  derivada e historial de ejecuciones. La primera
  slice de arrastre/soltar ya captura rutas en Rust y entrega a React únicamente
  la inspección validada. La primera slice de archivos recientes ya conserva solo nombre,
  formato, fecha e ID opaco, sin rutas, y Revisar ya muestra una actividad SQL
  acotada con estado, duración y filas. Al guardar un proyecto se conservan y
  restauran sus últimas cinco ejecuciones agregadas, sin guardar la consulta,
  rutas ni valores; la caché derivada del perfil queda ligada por SHA-256 al
  `current.parquet` durable y se invalida si cambia el snapshot. La vista activa
  de Revisar (`diagnosis`/`preview`) y el desplazamiento de la página visible de
  la muestra ya se conservan en el workspace durable con fallback seguro para
  catálogos anteriores. La etapa activa del flujo también se conserva con
  fallback a Revisar y validación cerrada; Revisar ahora comunica la cobertura
  agregada de muestreo importada desde sistema anterior, sin copiar filas, valores ni
  resultados originales. El motor SQL elegido (`polars`/`duckdb`) se persiste por
  proyecto en SQLite v10, con validación cerrada y fallback a la preferencia
  local para catálogos anteriores. La cobertura elegida para
  la matriz de correlaciones se persiste por proyecto con las opciones acotadas
  de 10.000, 50.000 o 100.000 filas; los catálogos anteriores vuelven al valor
  local seguro y fuerzan el recálculo si la caché no coincide. El perfil de
  rendimiento (`conservative`, `balanced` o `maximum`) también se persiste por
  proyecto en SQLite v11; los catálogos anteriores vuelven al perfil local y
  los cambios de dataset no arrastran el perfil de otro proyecto. La versión
  0.106.0 añade al workspace SQLite v12 el formato de exportación, la protección
  de datos, las columnas clave de comparación y el tipo de JOIN; sus valores se
  validan con listas cerradas y las claves se filtran contra el esquema
  restaurado. Las muestras originales y las preferencias que contienen datos
  o resultados derivados siguen fuera del contrato por privacidad. La importación M1 conserva además
  hasta cinco entradas de historial de ejecución cuando solo contienen estado,
  duración y filas; normaliza los estados reales `completed`/`failed` de
  sistema anterior y sus aliases, interpreta `rows_out` como filas de salida, asigna IDs
  locales y nunca copia consultas, rutas ni valores.
  La apertura segura del último output local ya está implementada desde Entregar
  con revalidación en Rust y sin transportar rutas por IPC.
  El modelo durable de proyectos de Columnia se conserva como reemplazo de la
  sesión persistente original.

**Gate:** cada capacidad marcada como implementada debe tener contrato, prueba
automatizada y una fila de paridad con evidencia del original.

## Tier 5 — Integridad de gates y preparación de distribución (abierto 2026-08-28)

Origen: reauditoría profesional exhaustiva sobre `137520b`. Las severidades de
esta tanda prevalecen sobre su número de Tier. Ninguna tarea está implementada
por el mero hecho de estar documentada aquí.

### Prioridad alta

- [x] **[T5-01] Aislar y corregir la regresión del ciclo durable del benchmark**
  - **Área:** Rendimiento / Persistencia
  - **Severidad:** Alta; regresión
  - **Ubicación:** `tools/benchmark-datasets.ps1:332`, `src-tauri/src/dataset.rs:16577`
  - **Qué hacer:** preservar una causa sanitizada por fase, perfilar
    `project-save`/`project-inspect` y corregir el fallo que interrumpe el ciclo
    tanto con 1 MiB como con 100 MiB.
  - **Criterio de aceptación:** tres corridas consecutivas de 100 MiB registran
    todos los comandos requeridos; `project-save` queda por debajo de 60 s y el
    cleanup es verdadero sin elevar el baseline.
  - **Esfuerzo:** alto
  - **Depende de:** ninguna

- [x] **[T5-02] Recuperar el presupuesto de memoria privada WebView2**
  - **Área:** Rendimiento
  - **Severidad:** Alta; regresión
  - **Ubicación:** `fixtures/performance/performance-baseline-v1.json:5`, `tools/probe-webview2-cdp.ps1:14`
  - **Qué hacer:** atribuir memoria por proceso/fase, eliminar retenciones o
    copias evitables y mantener el presupuesto contractual de 256 MiB privado.
  - **Criterio de aceptación:** tres recorridos funcionales y uno de selectores
    nativos consecutivos quedan dentro de 256 MiB privado y 512 MiB working set,
    con cleanup confirmado.
  - **Resultado:** el driver filtra por proceso/owner y el perfil limita tanto
    memoria como cleanup a procesos dentro del `Job Object`. La réplica que
    contaba un descendiente externo llegó a 332,472,320 bytes privados; la
    corrida posterior pasó con 503,644,160 bytes de working set y 267,456,512
    bytes privados, con cleanup confirmado. El driver espera el cierre del
    modal entre acciones y tiene timeout propio para no dejar colgado el gate;
    la variación del runtime WebView2 sigue bajo observación cerca del límite.
  - **Esfuerzo:** alto
  - **Depende de:** ninguna

- [x] **[T5-03] Hacer vinculante el presupuesto de primer render**
  - **Área:** QA / Rendimiento
  - **Severidad:** Media; verde falso
  - **Ubicación:** `tools/probe-webview2-playwright.mjs:55`, `tools/probe-webview2-playwright.mjs:172`
  - **Qué hacer:** separar estado funcional y de rendimiento, y propagar ambos al
    gate compuesto; medir arranque caliente del binario sin compilación fría.
  - **Criterio de aceptación:** una marca ausente o fuera de presupuesto produce
    un estado de rendimiento fallido inequívoco y una prueba del script lo cubre.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna

- [x] **[T5-04] Restablecer umbrales de cobertura por capa crítica**
  - **Área:** QA
  - **Severidad:** Alta; regresión de una tarea cerrada
  - **Ubicación:** `vitest.config.ts:18`, `ROADMAP.md:877`
  - **Qué hacer:** definir grupos/per-file para orquestación, controllers y fases;
    añadir tests conductuales antes de exigir los umbrales acordados.
  - **Criterio de aceptación:** bajar artificialmente una capa crítica bajo
    80/75/75/80 rompe `npm run test:coverage`; ninguna exclusión amplia la oculta.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna

- [x] **[T5-05] Limitar `SkipPackage` exclusivamente al bundling**
  - **Área:** QA / DevOps
  - **Severidad:** Alta; verde falso
  - **Ubicación:** `tools/verify-tier.ps1:47`, `tools/check.ps1:158`
  - **Qué hacer:** ejecutar siempre Full/Release según el tier y omitir solo la
    creación/inventario de MSI/NSIS; si se conserva otra semántica, renombrarla.
  - **Criterio de aceptación:** `verify:tier -SkipPackage` ejecuta Rust check,
    cobertura, Clippy, tests Rust y controles Release exigibles, pero no bundling.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna

- [x] **[T5-06] Ligar evidencia release a un commit limpio**
  - **Área:** QA / DevOps
  - **Severidad:** Alta
  - **Ubicación:** `tools/check-release-evidence.mjs:27`, `tools/capture-release-evidence.mjs:159`
  - **Qué hacer:** registrar `HEAD`, rama, dirty state y hashes de lockfiles; usar
    el mismo build para captura/paquete y revisar el baseline visual actual.
  - **Criterio de aceptación:** evidencia de otro commit, árbol sucio o baseline
    anterior falla; la evidencia fresca aprobada corresponde exactamente al
    commit empaquetado.
  - **Esfuerzo:** medio
  - **Depende de:** T5-05
  - **Resultado:** captura/check y el orquestador `tools/release.ps1` rechazan
    árboles sucios; el baseline versionado conserva el commit de evidencia, los
    hashes de ambos lockfiles y los cinco hashes visuales. La captura aprobada
    incluye desktop, móvil, zoom 125%, zoom 200% y forced-colors; el commit
    posterior que guarda exclusivamente el baseline es el único delta permitido
    por el checker. El orquestador reutiliza para la captura el mismo binario
    producido por el gate `Release/Package`.

- [ ] **[T5-20] Completar notices y atribuciones antes de publicar**
  - **Área:** Legal / Supply chain
  - **Severidad:** Alta para distribución pública; regresión de una tarea cerrada
  - **Ubicación:** `THIRD_PARTY_NOTICES.md:3`, `tools/generate-third-party-notices.ps1:94`
  - **Qué hacer:** mantener el inventario reproducible sin `UNKNOWN`, incorporar
    textos/copyrights requeridos cuando el canal los exija y anexar la revisión
    legal de las excepciones, atribuciones y jurisdicciones aplicables. El gate
    técnico y el inventario actual ya están implementados; queda la revisión
    legal del canal elegido.
  - **Criterio de aceptación:** cero `UNKNOWN` no exceptuados, cero duplicados y
    revisión legal documentada; `notices:check` falla ante incompletitud, no solo
    ante diferencia con lockfiles.
  - **Esfuerzo:** alto
  - **Depende de:** revisión legal y decisión del canal de distribución
  - **Resultado técnico 2026-08-31:** el generador consolida paquetes repetidos
    por identidad, conserva todas sus fuentes y rechaza filas incompletas,
    licencias `UNKNOWN` y licencias contradictorias. `npm run notices:check`
    valida 995 identidades; la revisión legal del canal y los textos completos
    que ese canal exija siguen pendientes.

### Prioridad media

- [x] **[T5-07] Sustituir escala de raster por zoom/reflow accesible real**
  - **Área:** Accesibilidad
  - **Severidad:** Media
  - **Ubicación:** `tools/capture-accessibility-evidence.mjs:17`, `tools/capture-release-evidence.mjs:113`
  - **Qué hacer:** ejercer zoom CSS/UI real a 125% y 200%, con aserciones de
    dimensiones, overflow, foco y contenido visible.
  - **Criterio de aceptación:** el caso cambia el viewport CSS efectivo, su
    evidencia difiere de desktop y una regresión de reflow rompe el gate.
  - **Esfuerzo:** bajo
  - **Depende de:** T5-06

- [x] **[T5-08] Confinar y confirmar salidas de manifiestos batch**
  - **Área:** Seguridad
  - **Severidad:** Media
  - **Ubicación:** `src-tauri/src/automation.rs:1689`, `src-tauri/src/automation.rs:1768`
  - **Qué hacer:** rechazar absolutas/`..` por defecto, usar un output root
    canonicalizado y exigir opt-in/`--force` para destino externo o existente.
  - **Criterio de aceptación:** tests prueban traversal, absoluta, symlink/reparse,
    colisión y archivo existente; ningún caso escribe fuera del root sin opt-in.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna

- [x] **[T5-09] Corregir la política de advisories de `quick-xml`**
  - **Área:** Seguridad / Supply chain
  - **Severidad:** Media
  - **Ubicación:** `src-tauri/deny.toml:5`, `docs/reference/dependency-audit.md:65`
  - **Qué hacer:** retirar el feature cloud, actualizar dependencias o demostrar
    no alcanzabilidad; sustituir la razón factual falsa de las excepciones.
  - **Criterio de aceptación:** `cargo tree -e features` y la justificación
    coinciden; `cargo audit`/`cargo deny` pasan con riesgo residual documentado.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna

- [x] **[T5-10] Generar el inventario y los contratos IPC exhaustivos**
  - **Área:** Arquitectura / Seguridad / QA
  - **Severidad:** Media
  - **Ubicación:** `src-tauri/src/lib.rs:154`, `src/ipc-contract.test.ts:481`, `THREAT_MODEL.md:71`
  - **Qué hacer:** detectar automáticamente comandos y estructuras compartidas,
    conservar literales/versiones y eliminar la cifra manual del threat model.
  - **Criterio de aceptación:** añadir un comando/tipo no clasificado rompe el
    gate; los 60 comandos de producción quedan inventariados desde código.
  - **Esfuerzo:** alto
  - **Depende de:** ninguna

- [x] **[T5-11] Permitir reintentar la inicialización del catálogo**
  - **Área:** Arquitectura / Fiabilidad
  - **Severidad:** Media
  - **Ubicación:** `src-tauri/src/projects.rs:159`, `src-tauri/src/projects.rs:174`
  - **Qué hacer:** cachear solo migraciones exitosas y serializar reintentos sin
    ejecutar dos migraciones concurrentes.
  - **Criterio de aceptación:** una prueba inyecta un primer fallo transitorio y
    la siguiente operación inicializa sin reiniciar el proceso.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna

- [x] **[T5-12] Reconciliar generaciones de proyecto huérfanas**
  - **Área:** Persistencia / Fiabilidad
  - **Severidad:** Media
  - **Ubicación:** `src-tauri/src/projects.rs:347`, `src-tauri/src/projects.rs:430`
  - **Qué hacer:** barrer bajo lock generaciones no referenciadas con margen de
    edad, registrar fallos sanitizados y no tocar artefactos activos.
  - **Criterio de aceptación:** una prueba simula crash antes del commit y la
    siguiente apertura elimina solo el huérfano, conservando el proyecto válido.
  - **Esfuerzo:** medio
  - **Depende de:** T5-11

- [x] **[T5-13] Abrir límites modulares en el motor de datos**
  - **Área:** Arquitectura / Refactorización
  - **Severidad:** Media; deuda conocida agravada
  - **Ubicación:** `src-tauri/src/dataset_fingerprints.rs`, `src-tauri/src/dataset.rs`, `src/App.tsx:100`
  - **Qué hacer:** extraer por etapas los límites de quality/recipe/export/persistence,
    dividir validadores grandes y reducir clones solo con perfiles y contratos verdes.
  - **Criterio de aceptación:** el primer módulo extraído tiene API interna acotada,
    sin ciclos, con tests equivalentes y una reducción medida de complejidad del
    motor; las siguientes fronteras se abren por etapas y no se hace una
    reescritura total.
  - **Resultado:** `dataset_fingerprints.rs` contiene 150 líneas y `dataset.rs`
    queda 119 líneas por debajo de `HEAD`; `cargo fmt`, clippy y los 244 tests
    Rust pasan.
  - **Esfuerzo:** alto
  - **Depende de:** T5-01

- [x] **[T5-14] Endurecer el gate Playwright local**
  - **Área:** QA
  - **Severidad:** Media
  - **Ubicación:** `playwright.config.ts:5`, `playwright.config.ts:20`
  - **Qué hacer:** activar `forbidOnly` para gates y servir cada corrida desde un
    puerto/proceso aislado ligado al bundle recién construido.
  - **Criterio de aceptación:** un `.only` falla localmente y un servidor previo
    no puede satisfacer el gate oficial.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna

- [x] **[T5-15] Reconstruir la trazabilidad de cambios y dependencias**
  - **Área:** Documentación
  - **Severidad:** Media; regresión
  - **Ubicación:** `CHANGELOG.md:3`, `docs/reference/dependency-audit.md:3`
  - **Qué hacer:** revisar los commits posteriores al último changelog, completar
    `[Unreleased]` orientado al usuario y refrescar el snapshot SCA de `0.57.0`.
  - **Criterio de aceptación:** cada cambio visible actual está representado y el
    inventario identifica versión, conteos y advisories derivados de los
    manifest/lockfiles vigentes, sin conservar cifras históricas.
  - **Esfuerzo:** medio
  - **Depende de:** T5-09

- [x] **[T5-16] Incluir las fuentes vivas en el gate documental**
  - **Área:** Documentación / QA
  - **Severidad:** Media
  - **Ubicación:** `tools/check-documentation.mjs:6`, `CONTEXTO.md:94`
  - **Qué hacer:** validar `ROADMAP.md`, `CONTEXTO.md` y el informe; comprobar
    métricas generadas y reducir la bitácora duplicada.
  - **Criterio de aceptación:** un enlace/UTF-8/versión inválido en cualquiera de
    las fuentes rompe `docs:check`; las líneas de App/dataset se derivan o prueban.
  - **Esfuerzo:** medio
  - **Depende de:** T5-15

- [x] **[T5-17] Fijar toolchains y alinear la configuración de distribución**
  - **Área:** DevOps / Configuración
  - **Severidad:** Media
  - **Ubicación:** `README.md:51`, `package.json:1`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json:49`
  - **Qué hacer:** declarar/comprobar Node/npm/Rust, documentar red de `npm audit`
    y WebView2, y seleccionar bundles explícitos por plataforma/canal.
  - **Criterio de aceptación:** una versión fuera de contrato falla con mensaje
    claro; modo offline no promete pasos de red; Package produce solo targets
    aprobados.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna

- [ ] **[T5-18] Hacer descubribles licencia, notices y privacidad local**
  - **Área:** Legal / Accesibilidad / UX
  - **Severidad:** Media; requiere revisión legal
  - **Ubicación:** `src/App.tsx:667`, `src-tauri/tauri.conf.json:52`, `THREAT_MODEL.md:75`
  - **Qué hacer:** crear una vista accesible Acerca de/Legal/Privacidad y definir
    persistencia, retención, borrado/desinstalación y responsable/contacto según
    jurisdicción/canal. La vista y el comportamiento local ya están
    implementados; queda la aceptación legal y la prueba en el canal elegido.
  - **Criterio de aceptación:** teclado y lector encuentran los textos desde la
    app instalada; revisión legal y pruebas de retención/borrado quedan anexadas.
  - **Esfuerzo:** medio
  - **Depende de:** T5-20 y definición de jurisdicción/canal
  - **Resultado técnico 2026-08-31:** el panel lateral expone una región
    etiquetada para licencia, avisos, privacidad, retención y borrado; la ficha
    `legal-distribution-decision.json` y el gate de distribución bloquean
    `Release`/`Package` mientras falten aprobación y canal.

### Prioridad baja

- [x] **[T5-19] Normalizar redacción y compactar el contexto histórico**
  - **Área:** Ortografía / Documentación
  - **Severidad:** Baja
  - **Ubicación:** `tools/generate-third-party-notices.ps1:96`, `docs/how-to/validate-release-evidence.md:1`, `CONTEXTO.md:563`, `CONTEXTO.md:584`
  - **Qué hacer:** corregir tildes/anglicismos desde las plantillas y trasladar
    bitácora redundante a CHANGELOG/ADR sin borrar decisiones vigentes.
  - **Criterio de aceptación:** regenerar notices conserva español correcto y
    `CONTEXTO.md` contiene estado/decisiones, no un historial de releases duplicado.
  - **Esfuerzo:** bajo
  - **Depende de:** T5-16

### Progreso de Tier 5

| Fecha | Estado | Evidencia |
| --- | --- | --- |
| 2026-09-02 | Versión 0.153.0 aplica la guardia de materialización a snapshots durables: la apertura verifica el tamaño de `current.parquet` antes de leerlo completo y rechaza de forma segura una expansión que no cabe en la RAM disponible, sin reemplazar la sesión activa. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.152.0 añade una guardia de admisión para la materialización eager de fuentes source-backed grandes: estima la expansión, conserva una reserva de seguridad y rechaza con mensaje accionable si la RAM disponible no alcanza; las operaciones compatibles siguen source-backed. | `src-tauri/src/resource.rs`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.151.0 extiende `Corregir codificación UTF-8` a fuentes source-backed: DuckDB repara secuencias mojibake inequívocas, publica snapshots reversibles, conserva el frame activo esquema-only y deriva al fallback eager cuando el valor no es demostrablemente seguro. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.150.0 extiende la limpieza source-backed a `Apartar tipos incompatibles`: DuckDB infiere booleanos, enteros, decimales y fechas sobre la fuente/snapshot efectivo, aplica el umbral eager del 90 %, conserva ceros iniciales como identificadores, publica snapshots reversibles y deja el frame activo esquema-only; los casos incompatibles mantienen fallback materializado. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.149.0 corrige recetas ISO source-backed encadenadas: la validación usa el snapshot Parquet efectivo tras filtros previos y conserva la ejecución incremental sin materializar el frame activo. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-02 | Versión 0.148.0 refresca `perf:benchmark`, `perf:webview2` y `perf:check`: 100 MiB, tres corridas sostenidas, dos actualizaciones durables, 819.137 filas WebView2, presupuestos actuales y cleanup confirmado. | `.local/validation/performance-benchmark/20260902T070437Z`, `.local/validation/performance-webview2/20260902T070748Z`, `.local/validation/performance-baseline/20260902T070920Z`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-02 | Versión 0.147.0 extiende la entrega ODBC a fuentes source-backed compatibles: DuckDB transmite por bloques sin materializar el `DataFrame` activo; calidad incremental, privacidad temporal, cancelación y validación de cambios conservan el contrato seguro, con fallback materializado explícito. | `src-tauri/src/remote_databases.rs`, `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-01 | Versión 0.146.0 añade entrega remota por ODBC a PostgreSQL, MySQL y SQL Server; exige `SELECT 1`, valida identificadores, aplica políticas `create_only`/`append`/`replace`, escapa valores, inserta por lotes y no persiste credenciales. | `src-tauri/src/remote_databases.rs`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-01 | Versión 0.145.0 conecta startup desktop con el gate de rendimiento: resume el intervalo proceso nativo listo→ventana visible y exige ≤1.000 ms, hitos completos, estado aprobado y cleanup. | `tools/summarize-performance.ps1`, `tools/check-performance-baseline.ps1`, `fixtures/performance/performance-baseline-v1.json`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-01 | Versión 0.144.1 normaliza la redacción de release y la plantilla de notices en español; el inventario regenerado conserva 995 identidades únicas y el gate de supply chain verifica el encabezado `Versión`. | `tools/generate-third-party-notices.ps1`, `THIRD_PARTY_NOTICES.md`, `src/supply-chain.test.ts`, `docs/how-to/validate-release-evidence.md` |
| 2026-09-01 | Versión 0.144.0 valida la ruta source-backed de JOIN con el benchmark reproducible de 512 MiB: `INNER`/`LEFT`/`FULL` conservan conteos y páginas, el frame activo permanece vacío, el working set máximo es 233.816.064 bytes y cleanup pasa. | `tools/benchmark-duckdb-join.ps1`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.143.0 valida decisiones source-backed recorriendo conflictos por bloques y reteniendo solo índices y columnas divergentes; eleva el límite explícito a 8.192, conserva el fallback eager para entradas mayores y mantiene orden, publicación reversible, integridad y cleanup. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.142.0 resuelve conflictos source-backed acotados por fila o columna directamente en DuckDB: publica un snapshot Parquet reversible, conserva orden y frame esquema-only, valida fuentes y limpia la comparación después de publicar; los casos fuera de 2.048 conflictos o incompatibles mantienen fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.141.0 evita la materialización del activo source-backed al paginar conflictos por clave: genera un snapshot Parquet temporal solo cuando hace falta, procesa bloques y conserva integridad, orden, valores agregados y frame esquema-only; la resolución completa queda cubierta por v0.142.0. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.140.0 ejecuta la consolidación por claves entre activos source-backed y snapshots Parquet comparados directamente en DuckDB; valida duplicados/conflictos antes de escribir, publica solo las claves nuevas, conserva orden, fuentes, nombre visible e historial reversible y mantiene fallback eager para formatos incompatibles. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.139.0 ejecuta mutaciones `INNER`/`LEFT`/`FULL` entre activos source-backed y fuentes locales CSV/TSV/TXT/Parquet en DuckDB; publica solo el resultado Parquet, comprueba el límite de 2.000.000 de filas, preserva orden y fuentes, activa historial reversible y mantiene fallback eager para formatos incompatibles. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.138.0 mantiene la exportación incremental de Bundles source-backed con receta activa; incorpora `recipe.json`, su referencia y hash en `manifest.json`, y conserva privacidad, calidad incremental, cancelación, atomicidad, validación de cambios y cleanup sin materializar el frame activo. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.137.0 mantiene la exportación source-backed con protección `mask`/`hash`: genera un snapshot privado en DuckDB y transfiere todos los destinos locales sin materializar el frame activo; conserva nulos, columnas no personales, validación de cambios, atomicidad y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.136.0 ejecuta la eliminación de duplicados parecidos y las correcciones recomendadas sobre fuentes source-backed con DuckDB; conserva claves normalizadas/exactas, repeticiones idénticas, trim, renombres, orden, conteos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.135.0 ejecuta las acciones directas IQR `cap`, `impute` y `drop` sobre fuentes source-backed con DuckDB; conserva cuantiles, límites, mediana, tipos, nulos, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.134.0 ejecuta las imputaciones conservadora y categórica sobre fuentes source-backed con DuckDB; conserva moda/mediana de la ruta eager, `Desconocido`, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.133.0 ejecuta la conversión numérica y la interpretación de fechas detectadas sobre fuentes source-backed con DuckDB; conserva las reglas eager de seguridad, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.132.0 ejecuta la normalización de booleanos sobre fuentes source-backed con DuckDB; conserva el umbral eager, alias reconocidos, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.131.0 ejecuta recorte de espacios, normalización de texto y valores centinela sobre fuentes source-backed con DuckDB; conserva conteos exactos, snapshots reversibles y fallback eager para modos no equivalentes. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.130.0 ejecuta la normalización de nombres y la activación de `_cambios` sobre fuentes source-backed con DuckDB; conserva colisiones, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.129.0 ejecuta la máscara de valores personales source-backed sobre DuckDB; conserva conteos agregados, `_cambios`, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.128.0 ejecuta el retiro de columnas identificadoras y personales detectadas sobre fuentes source-backed con DuckDB; conserva `_cambios`, snapshots reversibles, el contrato IPC agregado y fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.127.0 ejecuta duplicados y limpiezas de columnas constantes, vacías o con alta nulidad sobre fuentes source-backed con DuckDB; conserva orden, `_cambios`, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.126.0 ejecuta la eliminación de filas completamente vacías sobre fuentes source-backed con DuckDB, conserva `_cambios`, activa historial reversible por Parquet y mantiene fallback eager si el presupuesto de disco no alcanza. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.125.0 abre `JSON`, `JSONL` y `NDJSON` grandes mediante snapshots Parquet privados de DuckDB; conserva esquema, preview y conteo sin materializar el frame activo, valida cancelación e integridad de la fuente y reutiliza el snapshot en recetas source-backed. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.124.0 extiende los filtros temporales a `Eq`/`Neq` sobre `Date` y `Datetime`; eager, Polars lazy y DuckDB source-backed usan literales ISO 8601 y la regresión cubre unidad temporal, paridad y snapshot. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.123.0 mantiene parseo de fecha, filtros ISO 8601 y extracción de `year`, `month` o `day` en un único plan Polars lazy/streaming; la validación y la regresión confirman el orden de etapas, conteo y tipos sin materialización eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.122.0 amplía los filtros ordenados de recetas a `Date`/`Datetime` con literales ISO 8601 en eager, Polars lazy y DuckDB source-backed; la regresión compara límites, unidades, conteo, nulos y snapshot Parquet. | `src-tauri/src/dataset.rs`, `src/features/prepare/TransformRecipeEditor.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-28 | Implementación técnica mayormente cerrada: T5-01–T5-17 y T5-19; T5-18/T5-20 siguen pendientes de aceptación legal y pruebas del canal. T5-06 queda cerrado con evidencia release ligada a commit limpio y baseline visual actualizado. El orquestador local y el updater firmado ya están implementados. | Código, cobertura, IPC, notices, toolchains, benchmark formal, Package firmado, smoke nativo, suite Rust y `fixtures/accessibility/release-evidence-baseline-v1.json` |
| 2026-08-31 | Se cerró la protección técnica de T5-18/T5-20 sin falsear aprobación: la ficha jurídica es explícita y versionada, `legal:check` valida artefactos y `Release`/`Package` exigen sign-off; notices consolida 995 identidades sin `UNKNOWN` ni duplicados y el panel legal es una región accesible. Las decisiones de jurisdicción/canal y la prueba en VM/canal real siguen pendientes. | `docs/reference/legal-distribution-decision.json`, `tools/check-legal-distribution.mjs`, `tools/generate-third-party-notices.ps1`, `npm run notices:check`, `npm run legal:check` |
| 2026-08-31 | Versión 0.93.0 amplía las recetas source-backed: casts, fechas fijas `YMD`/`DMY`/`MDY` y cálculos de suma, resta, multiplicación o concatenación se ejecutan en DuckDB por etapas y publican un snapshot Parquet privado; división, partes de fecha, ISO y operaciones no compatibles mantienen el fallback eager. La paridad con la ruta eager queda cubierta por regresión Rust. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.94.0 añade partes de fecha `Year`/`Month`/`Day` a las recetas source-backed cuando la fuente se parsea con una fecha fija sin filtros previos; valida dependencias y publica el snapshot Parquet con paridad eager, manteniendo fallback para ISO, filtros previos y conflictos de conversión. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.95.0 añade reemplazo literal source-backed después de filtros y antes de la proyección: DuckDB publica el snapshot y una consulta escalar obtiene el conteo exacto de celdas cambiadas; regex y caracteres no válidos conservan fallback. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.96.0 añade unión de dos a dieciséis columnas de texto source-backed en DuckDB; conserva orden, nulos, cadenas vacías, separador, casts numérico→texto, `keepColumns` y `dropSources`, con una regresión de paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.97.0 añade división de columnas de texto source-backed en DuckDB; conserva delimitadores Unicode, segmentos vacíos, nulos, resto final, `keepColumns`, renombrados y `dropSource`, con paridad contra eager y unión posterior. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.98.0 añade extracciones textuales source-backed de tokens, dígitos, letras Unicode y segmentos antes/después de delimitadores literales en DuckDB; conserva nulos, coincidencias ausentes, resultados vacíos, renombrados y `keepColumns`, con paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.99.0 añade normalización de correo, teléfono y dirección source-backed en DuckDB; conserva nulos, espacios Unicode, prefijos telefónicos y conteos exactos, y permite extracciones posteriores sobre los valores normalizados con paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.100.0 añade resúmenes source-backed por grupo en DuckDB para `sum`, `mean`, `min`, `max`, `count` y `count_unique`; conserva orden estable de primer grupo, claves nulas, límites de tipo y validaciones de finitud/precisión/overflow, publica un snapshot Parquet sin materializar el `DataFrame` activo y compara salida y contadores con eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.101.0 añade tratamientos IQR source-backed `cap`, `drop` e `impute` en DuckDB; calcula un baseline común posterior a filtros, conserva nulos y tipos cuando corresponde, valida mínimo de valores/precisión/finitud/umbrales y compara los tres modos y sus contadores con eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.102.0 añade fechas ISO source-backed sin offset o con sufijo UTC `Z` en DuckDB; conserva la conversión estricta y mantiene fallback eager para offsets no UTC o valores inválidos, con regresiones de paridad. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.103.0 persiste por proyecto la cobertura de filas de correlaciones (`10.000`, `50.000` o `100.000`), migra el catálogo SQLite a v9 y mantiene fallback local seguro para catálogos anteriores, con regresiones de reapertura y validación cerrada. | `src-tauri/src/projects.rs`, `src/App.tsx`, `src/App.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.104.0 persiste por proyecto el motor SQL elegido (`polars`/`duckdb`), migra el catálogo SQLite a v10, rechaza valores desconocidos y conserva fallback local seguro para catálogos anteriores, con regresiones de reapertura y validación cerrada. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `src/App.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.109.0 mantiene el esquema y la primera página source-backed en Polars, pero calcula el conteo total de CSV/TSV/TXT delimitado o Parquet mediante DuckDB con cancelación cooperativa; la regresión y el benchmark end-to-end confirman conteo exacto, frame activo vacío y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.110.0 extiende la exportación source-backed a CSV mediante DuckDB para fuentes CSV/TSV/TXT delimitadas o Parquet; conserva atomicidad, cancelación, validación de cambios, neutralización de fórmulas y no materializa el `DataFrame` activo en la ruta compatible. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.121.0 amplía las recetas source-backed para parsear fechas y extraer `year`, `month` o `day` después de filtros mediante DuckDB; la regresión comprueba orden de etapas, conteo, paridad eager y snapshot Parquet sin materializar el frame. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.120.0 evita materializar fuentes source-backed al guardar proyectos: DuckDB crea `current.parquet` directamente desde CSV/TSV/TXT delimitado o Parquet, se verifica el conteo y la sesión conserva el esquema vacío; la regresión cubre snapshot, filas y estado activo. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.119.0 extiende las recetas source-backed con división calculada por operando literal o columna en DuckDB; valida división por cero antes de publicar, conserva nulos y paridad eager, y mantiene el frame activo vacío. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.118.0 extiende las recetas source-backed con reemplazos regex globales y grupos numéricos `$1`–`$9` mediante DuckDB; conserva paridad eager, nulos, conteo de celdas y frame activo vacío, mientras las sustituciones no compatibles mantienen fallback. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.117.0 extiende la protección source-backed: `mask` y `hash` se aplican mediante una proyección DuckDB a un snapshot Parquet privado y los siete destinos locales lo transmiten sin materializar el `DataFrame` activo; se preservan nulos, columnas protegidas, cancelación y validación final de la fuente original. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.116.0 extiende la apertura source-backed a libros XLSX/XLSB grandes: Calamine detecta esquema y tipos en streaming, escribe un snapshot Parquet temporal por bloques y deja el `DataFrame` activo vacío; paginación, perfilado, consultas DuckDB y exportaciones compatibles reutilizan el snapshot con validación del libro original, y XLS/ODS conservan fallback materializado. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.115.0 extiende la exportación source-backed a Excel `.xlsx` y SQLite para fuentes CSV/TSV/TXT delimitadas o Parquet: DuckDB transfiere las filas por streaming, conserva esquema y tipos, publica de forma atómica y comprueba cancelación, cambios de la fuente y cleanup; las regresiones reabren ambos destinos sin materializar el `DataFrame` activo. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.114.0 añade `npm run brand:check`, que inspecciona archivos activos rastreados y no ignorados para impedir el regreso de referencias a la marca retirada; el gate pasa y deja explícito que el historial Git anterior no se reescribe. | `tools/check-retired-brand.mjs`, `package.json`, `tools/check.ps1`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-01 | Versión 0.113.0 extiende la exportación source-backed a Bundle ZIP para fuentes CSV/TSV/TXT delimitadas o Parquet: DuckDB escribe `dataset.csv`, calcula nulos del diccionario sin materializar el `DataFrame` activo y publica calidad incremental, manifest con hashes, cancelación y atomicidad; la regresión confirma fuente intacta y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.112.0 endurece consultas source-backed: `INNER`, `LEFT` y `FULL JOIN` ejecutan directamente desde la fuente con DuckDB y el fallback que materializaría silenciosamente se convierte en error explícito; el benchmark de 512 MiB valida conteos, paginación, frame vacío, working set y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `tools/benchmark-duckdb-join.ps1`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.111.0 extiende la exportación source-backed a SQL mediante DuckDB para fuentes CSV/TSV/TXT delimitadas o Parquet; escribe esquema y literales escapados en una transacción, conserva cancelación, validación de cambios y publicación atómica sin materializar el `DataFrame` activo en la ruta compatible. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.108.0 añade `perf:duckdb:join`, un benchmark opt-in de una fuente CSV temporal de 512 MiB y 1.810.432 filas; ejecuta un LEFT JOIN source-backed desde DuckDB, conserva el frame activo vacío, valida conteo/paginación/working set de 512 MiB y confirma cleanup. La ejecución integral fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `tools/benchmark-duckdb-join.ps1`, `package.json`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.105.1 renombra el componente interno de vista previa a `DatasetPreviewPanel` para que el árbol activo no contenga coincidencias textuales con la marca retirada, sin alterar el contrato visible ni la funcionalidad. | `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-09-01 | Versión 0.107.0 añade la ruta DuckDB `DataFrame` activo + snapshot Parquet comparado para evitar materializar de nuevo el lado comparado en JOINs compatibles; Polars la promueve automáticamente, se conserva fallback seguro y una regresión comprueba paridad de filas, nulos y orden. La ejecución completa fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.106.0 persiste en SQLite v12 el formato de exportación, la protección de datos, las columnas clave de comparación y el tipo de JOIN; Rust valida listas cerradas, la reapertura filtra claves contra el snapshot restaurado y no se guardan muestras ni valores. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/features/delivery/DeliveryPhase.tsx`, `src/bridge.ts`, `src/App.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.105.2 corrige el orden global de `FULL JOIN` en consultas DuckDB source-backed: la preparación usa el conteo real del dataset activo aunque el esquema en memoria esté vacío, y una regresión ejecuta ambos snapshots Parquet sin materializar el activo. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.105.0 persiste por proyecto el perfil de rendimiento (`conservative`/`balanced`/`maximum`), migra el catálogo SQLite a v11, rechaza valores desconocidos y conserva fallback local seguro para catálogos anteriores; la UI restaura el perfil y lo desvincula al cambiar de dataset. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/components/ResourceMonitor.tsx`, `src/features/projects/useProjectsController.ts`, `src/App.test.tsx`, `src/components/ResourceMonitor.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-28 | Benchmark corto post-optimización aprobado: 100 MiB, 876,544 filas, `project-save` 59.75 s, actualización 58.84 s, reapertura/exportación y cleanup confirmados. | `.local/validation/performance-benchmark/20260828T180520Z/summary.json` |
| 2026-08-28 | Benchmark formal final aprobado: 100 MiB, 876,544 filas, `project-save` en 52.09 s y tres actualizaciones durables entre 56.37 y 57.31 s, reapertura/exportación y cleanup confirmados. | `.local/validation/performance-benchmark/20260828T184531Z/summary.json` |
| 2026-08-28 | CDP funcional de ProjectsPanel y perf gate aprobados: 3 ciclos sostenidos, 470.25 MiB working set, 253.48 MiB privados y cleanup; accesibilidad visual 125%/200% y forced-colors aprobada. | `.local/validation/webview2-cdp/20260828T185215Z/summary.json`, `.local/validation/accessibility-visual/20260828T185137Z` |
| 2026-08-28 | Package firmado del updater aprobado en Windows: Tauri produjo MSI/NSIS y `.sig`; el manifiesto estático para `windows-x86_64`, SHA-256 y verificación del par artefacto/firma pasan localmente. El canal real, la VM y la promoción siguen fuera de esta corrida. | `.local/validation/updater-build/updater-manifest.json`, `.local/validation/updater-build/updater-integrity.json`, `.local/validation/20260828T194329Z-6ec7bae-package.json` |
| 2026-08-28 | Contrato updater reproducible aprobado: el gate genera una clave Ed25519 efímera, valida criptográficamente el fixture válido y confirma fallo cerrado ante artefacto truncado, firma alterada, manifiesto incompleto/corrupto y URL HTTP; no sustituye la prueba del canal contra un servidor publicado. | `tools/test-updater-manifest.mjs`, `tools/updater-crypto.mjs`, `npm run updater:contract:test` |
| 2026-08-28 | Smoke del artefacto NSIS real aprobado desde usuario sin privilegios: instalación en ruta Unicode/con espacios, primera apertura, instancia única, desinstalación y retención controlada del dato de usuario; `Package` lo ejecuta automáticamente. | `tools/smoke-installed-artifact.ps1`, `.local/validation/installer-smoke/20260828T230154Z/summary.json`, `.local/validation/20260828T225503Z-ff90fb2-package.json` |
| 2026-08-28 | Smoke de upgrade local aprobado con `Columnia_0.49.0_x64-setup.exe` como versión previa: misma ruta de instalación, ejecutable actualizado a `0.57.0`, datos de usuario conservados y cleanup confirmado. | `tools/smoke-installed-artifact.ps1 -PreviousInstallerPath`, `.local/validation/installer-smoke/20260828T235459Z/summary.json` |
| 2026-08-28 | Política de rotación updater versionada y comprobada por fingerprint: rotación normal mediante release puente; compromiso de clave mediante congelación del canal y recuperación fuera de banda, sin firmar otra release con la clave comprometida. | `fixtures/updater/key-policy-v1.json`, `tools/check-updater-key-policy.mjs`, `npm run updater:key:check` |
| 2026-08-28 | Perfil `Release` completo aprobado después de integrar el contrato updater: documentación, IPC, toolchains, cobertura, build web, Clippy, 244 tests Rust, SBOM, supply chain, instalador, fixture updater y binario Tauri sin bundle. La evidencia release posterior fue capturada desde un commit limpio, revisada visualmente en los cinco escenarios y ligada al baseline; la ruta local se valida con `npm run accessibility:release:check`. | `.local/validation/20260828T214048Z-6ec7bae-release.json`, `fixtures/accessibility/release-evidence-baseline-v1.json` |
| 2026-08-28 | Smoke nativo aislado aprobado desde `npm run smoke:native-selectors`: los cuatro diálogos Win32 de abrir/guardar pasan con filtrado por proceso, outputs verificados, cleanup confirmado y presupuesto de 512 MiB working set / 256 MiB privado respetado. El smoke Playwright se mantiene separado para no mezclar su retención de WebView2 con la medición nativa. | `.local/validation/webview2-cdp/20260828T203917Z/summary.json` |
| 2026-08-29 | `perf:webview2` aprobado: 100 MiB/819,137 filas dentro de WebView2, con selector Win32, carga, paginación, transformación, exportación y cleanup; el recorrido grande queda separado del CDP normal y registra 686,817,280 bytes de working set y 456,114,176 bytes privados bajo su techo de medición 1.5 GiB/1 GiB. El presupuesto global 512/256 MiB continúa abierto. | `.local/validation/performance-webview2/20260829T010235Z/summary.json`, `.local/validation/webview2-cdp/20260829T010238Z/summary.json` |
| 2026-08-29 | M1 conserva en `migrationReport.session` los nombres estructurales acotados de operaciones aplicadas y comprobaciones de análisis cuando son tokens seguros; sus conteos, receta, reglas y round-trip durable siguen siendo verificables, mientras resultados, cachés y snapshots históricos de sistema anterior continúan pendientes. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | M1 prioriza el snapshot local compatible de sistema anterior para restaurar el estado materializado exacto; solo reaplica la receta sobre el origen cuando no hay snapshot, con regresión nativa para impedir una reproducción incompleta de `applied_ops`. | `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | P1 añade detección agregada y reparación reversible de doble codificación UTF-8 heredada; solo se aplican conversiones de texto inequívocas y se excluyen números y `_cambios`. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-29 | M1 regenera y persiste el perfil agregado al importar una sesión sistema anterior; el proyecto abre con caché de calidad válida, mientras resultados originales, cachés reanudables e historial que no estén en el artefacto siguen pendientes. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | Nueva corrida estricta `smoke:cdp` con Playwright, ProjectsPanel, mutaciones nativas y cleanup aprobados: el working set quedó en 501,563,392 bytes dentro de 512 MiB, pero la memoria privada alcanzó 272,379,904 bytes frente al límite de 256 MiB. Corridas diagnósticas previas quedaron en 256.06–258.06 MiB; no se atribuye todavía una fuga al código y el gate privado estable sigue abierto. | `.local/validation/webview2-cdp/20260829T023923Z/summary.json`, `tools/probe-webview2-cdp.ps1` |
| 2026-08-29 | El perfil de `smoke:cdp` ahora comprueba pertenencia al `Job Object` al medir y limpiar el árbol; una réplica explicó 63,680,512 bytes privados de un descendiente externo no reconocido. La corrida posterior quedó dentro del contrato con 503,644,160 bytes de working set, 267,456,512 bytes privados y cleanup confirmado. | `.local/validation/webview2-cdp/20260829T044214Z/summary.json`, `.local/validation/webview2-cdp/20260829T044540Z/summary.json`, `tools/probe-webview2-cdp.ps1` |
| 2026-08-29 | P1 amplía el catálogo de limpieza con una acción confirmada para apartar como nulos los valores de texto que contradicen una sugerencia semántica con al menos 90% de coincidencia; la operación es reversible, no muestra celdas y conserva pendientes las reglas avanzadas restantes. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-29 | P1 añade imputación reversible de outliers por mediana: el perfil muestra solo conteos agregados, Preparar ofrece la acción directa y las recetas admiten `impute`; Int64/Float64 conservan su tipo y `_cambios` queda protegido. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/TransformRecipeEditor.tsx` |
| 2026-08-29 | P1 añade imputación categórica explícita y reversible: completa solo nulos textuales como `Desconocido`, protege números y `_cambios`, publica impacto agregado y actualiza el inventario IPC a 61 comandos de producción. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts` |
| 2026-08-29 | P1 añade acciones IQR confirmables para limitar valores atípicos o eliminar las filas que excedan los límites, con historial reversible, impacto agregado y sin mostrar celdas; el inventario IPC pasa a 63 comandos de producción. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts` |
| 2026-08-29 | P1/M1 liga los perfiles cacheados del catálogo a la huella SHA-256 de `current.parquet`: al reabrir un proyecto se invalida solo la caché si cambia el snapshot, se conserva compatibilidad con catálogos anteriores y SQLite migra a v5 de forma transaccional. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `CONTEXTO.md` |
| 2026-08-29 | M1 reproduce durante el fallback a la fuente las operaciones sistema anterior deterministas `drop_duplicates`, `drop_fuzzy_duplicates`, `drop_high_null_cols`, `drop_id_cols`, `drop_empty_cols`, `drop_constant_cols`, `drop_empty_rows`, `normalize_sentinels`, `impute_numeric`, `impute_categorical`, `parse_dates`, `trim_text`, `normalize_text`, `fix_encoding`, `cast_numeric`, `cap_outliers`, `impute_outliers`, `drop_outliers`, `normalize_booleans`, `mask_pii`, `normalize_columns` y `add_cambios_col`, en el orden fijo del registro y sin inventar parámetros; `parse_dates` solo convierte texto con formatos cerrados, cobertura segura, años entre 1900 y 2100 y como máximo 1% de literales ilegibles; `drop_fuzzy_duplicates` conserva el guard de 5.000 filas y usa el fingerprint normalizado local, mientras `mask_pii` aplica solo la máscara predeterminada `[REDACTED]`; las eliminaciones de columnas conservan al menos una columna utilizable, `drop_empty_rows` conserva la semántica de filas completamente nulas, `impute_numeric` usa la mediana de columnas físicas después de normalizar centinelas y promueve a `Float64` si resulta fraccionaria, `trim_text` recorta espacios exteriores y protege `_cambios`, `normalize_text` colapsa espacios, pasa a minúsculas y retira acentos, `cast_numeric` exige más de 90% de valores numéricos y rechaza conversiones con pérdida de precisión, las estrategias IQR cap/impute/drop usan al menos cuatro valores numéricos válidos y `drop_outliers` elimina una fila si cualquier columna numérica excede sus límites, `normalize_booleans` convierte solo vocabularios cerrados, `normalize_columns` aplica la resolución Unicode de nombres, `add_cambios_col` recupera la estructura reservada sin inventar anotaciones históricas y los snapshots compatibles siguen teniendo prioridad y no se reaplican. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | M1/P1 migra `selected_cleaning_operations` mediante aliases canónicos: `normalize_text` y las demás limpiezas deterministas entran al replay desde la fuente y al informe de sesión; las operaciones avanzadas sin equivalente reversible conservan una advertencia específica y no se ejecutan silenciosamente. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | M1/P1 incorpora `mask_pii` y `drop_fuzzy_duplicates` al replay seguro de sesiones sistema anterior: sin snapshot compatible se aplica la máscara predeterminada y conservadora `[REDACTED]` o el fingerprint normalizado local (hasta 5.000 filas), preservando nulos, copias exactas, columnas no personales y `_cambios`; los snapshots mantienen prioridad y los modos hash/clave explícita no se inventan. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-29 | P1 cierra la brecha de acción directa para `parse_dates`: Preparar ofrece interpretar columnas de texto con un formato de fecha dominante cerrado, omite mezclas ambiguas, conserva nulos y publica el cambio en el historial reversible con impacto agregado; el inventario IPC pasa a 65 comandos de producción. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts`, `docs/reference/ipc-inventory.json` |
| 2026-08-29 | P1 cierra la brecha de acción directa para `cast_numeric`: Preparar convierte texto con más de 90% de coincidencia numérica, rechaza pérdida de precisión, conserva identificadores/códigos con ceros iniciales y publica una mutación reversible con impacto agregado; el inventario IPC pasa a 66 comandos de producción. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts`, `docs/reference/ipc-inventory.json` |
| 2026-08-29 | M1 amplía el round-trip de sesiones con un `.xlsx` real generado por el exportador nativo: la hoja registrada se valida, la receta se aplica y el proyecto se reabre comprobando esquema, conteos y etapa activa; la restauración completa de artefactos históricos sigue pendiente. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `fixtures/manifest.json` |
| 2026-08-29 | M1 importa hasta cinco entradas de historial de ejecución solo cuando sus metadatos agregados son seguros (estado, duración y filas), las persiste en la actividad SQL del proyecto con IDs locales y descarta consultas, rutas, valores y entradas inválidas. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-29 | M1 rechaza antes de publicar una sesión que combine estrategias IQR de outliers mutuamente excluyentes (`cap`, `impute` y `drop`), preservando la semántica de sistema anterior y el catálogo existente. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-29 | P1/M1 añade dos datasets de ejemplo locales para explorar calidad y series temporales desde Cargar: se crean bajo el almacenamiento de la aplicación, se inspeccionan con el flujo nativo existente y React recibe solo metadatos e identificadores opacos; el inventario IPC pasa a 68 comandos de producción y 59 estructuras. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/load/LoadPhase.tsx`, `src/styles.css`, `docs/reference/ipc-inventory.json` |
| 2026-08-29 | M1 conserva metadatos agregados de muestreo de análisis cuando una sesión sistema anterior los aporta: estado muestreado y conteos de filas validados, sin importar filas, valores ni resultados; el resumen queda visible en el informe de compatibilidad. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/prepare/TransformRecipeEditor.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-29 | P1 persiste por proyecto las últimas cinco ejecuciones SQL como actividad agregada (estado, duración y filas), las restaura al abrir y rechaza historiales corruptos o sobredimensionados; no guarda consultas, rutas ni valores. | `src-tauri/src/projects.rs`, `src/features/review/ReviewPhase.tsx`, `src/App.tsx` |
| 2026-08-29 | P1/M1 añade desde Entregar la apertura segura del último output local: Rust retiene solo durante la sesión el destino de una exportación exitosa, lo revalida como archivo regular y abre su carpeta mediante el explorador nativo, sin enviar rutas a React. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-30 | P1 amplía la consulta SQL local a `GROUP BY` compuesto de hasta ocho columnas, con orden de primera aparición, combinaciones nulas, rechazo de claves duplicadas y presupuesto de agregación conservado; DuckDB, joins más amplios y ejecución incremental siguen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 amplía los `JOIN` SQL locales a hasta ocho pares de claves entre `dataset` y `compared`, con validación de duplicados, tipos compatibles, preflight de cardinalidad y orden izquierdo; DuckDB y joins fuera de memoria siguen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 procesa la segunda pasada de agregaciones SQL locales por bloques: conserva estados de `COUNT`/`SUM`/`AVG`/`MIN`/`MAX` y grupos, no índices de todas las coincidencias, sin cambiar el presupuesto ni el resultado. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 particiona el índice exacto de comparación por claves en 256 cubetas temporales y procesa resumen, nuevas claves y conflictos paginados por cubeta, preservando orden y duplicados sin retener todos los índices en memoria. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 calcula la comparación completa de filas por multiconjuntos de firmas particionadas, conservando conteos exactos sin mapas globales de firmas. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 particiona el preflight de cardinalidad de los `JOIN` locales: derrama claves de ambos lados, calcula productos de duplicidad por cubeta y conserva cancelación y límites explícitos. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 procesa consultas SQL locales `INNER`/`LEFT` sin agregación por bloques del lado `dataset`: conserva el orden y la paginación globales, cuenta todas las coincidencias y evita acumular el `DataFrame` unido completo. `FULL` y agregaciones mantienen la ruta eager acotada mientras se define su estrategia incremental. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 extiende el procesamiento por bloques a las agregaciones SQL locales `INNER`/`LEFT`: fusiona estados de `COUNT`/`SUM`/`AVG`/`MIN`/`MAX` y grupos en orden estable, sin acumular el `DataFrame` unido completo. `FULL` y DuckDB permanecen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 extiende la ruta por bloques a consultas SQL locales `FULL`: procesa el lado `dataset` como `LEFT` y añade por bloques las filas derechas no emparejadas con anti-join estable; paginación y agregaciones incluyen ambos lados sin acumular el resultado unido completo, aunque el frame anti-join derecho sigue acotado por los límites actuales. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | M1 restaura el historial portable `history_snapshots` v1: hasta doce Parquet locales, etiquetas/cursor validados, presupuesto durable de 1 GiB y reanudación de Deshacer/Rehacer tras reabrir el proyecto; historiales ambiguos y artefactos de análisis siguen fuera de alcance. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/bridge.ts`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 incorpora la primera ruta DuckDB opcional para SQL local: valida el contrato restringido, reutiliza el snapshot Parquet administrado de la revisión actual cuando existe y usa snapshots temporales como fallback, conserva conteo/paginación/orden estable y reproduce el esquema coalescido de JOIN `INNER`/`LEFT`/`FULL`; el `DataFrame` activo y la ejecución incremental fuera de RAM permanecen como límites explícitos. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 hace que DuckDB reutilice el snapshot Parquet administrado de la revisión actual, añadiendo la columna de orden solo en la vista temporal y evitando serializar otra vez el `DataFrame`; la ruta conserva fallback para historiales degradados y no afirma todavía ejecución fuera de RAM. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 conserva la fuente comparada en un snapshot Parquet temporal mientras la comparación está activa: conflictos y consolidación leen bajo demanda, los JOIN DuckDB pueden registrar ambos snapshots sin reserializar el frame comparado y el dueño temporal garantiza cleanup al descartar la comparación. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 reduce la materialización de JOIN DuckDB: cuando existe snapshot administrado, la preparación lee solo el esquema Parquet de la comparación y evita volver a cargar sus filas; la preparación DuckDB tampoco aplica el límite de entradas Polars, mientras el `DataFrame` activo y la ejecución completa fuera de RAM siguen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 sirve la paginación de la muestra activa desde el snapshot Parquet del cursor actual con `slice` y colección streaming, conservando el fallback al `DataFrame` cuando el historial está degradado. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 evita una segunda clonación completa al guardar proyectos: la copia aislada del dataset se entrega directamente al escritor Parquet, conservando la publicación atómica y la recuperación ante fallos; el benchmark fija el perfil de compilación reproducible y restaura el entorno del proceso. | `src-tauri/src/projects.rs`, `tools/benchmark-datasets.ps1`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 añade consultas Polars simples respaldadas por el snapshot Parquet del cursor: valida el esquema sin filas, cuenta coincidencias por bloques de 16K y relee solo la ventana o los bloques necesarios para agregaciones; comprueba el conteo exacto, respeta cancelación y vuelve al `DataFrame` activo ante snapshot inválido. `JOIN`, comparación, historial degradado y ejecución integral fuera de RAM conservan sus límites explícitos. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | M1 expone en Revisar la cobertura agregada de análisis conservada en sesiones sistema anterior reabiertas: estado muestreado y conteos de filas, con un límite visible que distingue metadatos de la muestra original no portable. | `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 persiste localmente la preferencia del motor SQL de Revisar (`Polars`/`DuckDB`), rechaza valores desconocidos y conserva `Polars` como fallback seguro sin ampliar el workspace ni el IPC. | `src/features/review/reviewModel.ts`, `src/features/review/ReviewPhase.tsx`, `src/features/review/reviewModel.test.ts`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 permite elegir el límite de muestreo de la matriz de correlaciones numéricas entre 10.000, 50.000 y 100.000 filas; el bridge lo transporta como opción acotada, Rust invalida cachés con otra cobertura y conserva fallback seguro. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/review/reviewModel.ts`, `src/features/review/ReviewPhase.tsx`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 expande la receta lazy/streaming a parseos explícitos de fecha `Ymd`, `Dmy` y `Mdy`, y a `Iso8601` sin offset o con sufijo UTC `Z`: conserva espacios exteriores, nulos y objetivos `Date`/`Datetime`, mientras offsets distintos de UTC y zonas horarias mantienen fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 expande la receta lazy/streaming a tratamientos IQR aislados (`cap`, `impute`, `drop`) sobre columnas numéricas, calculando umbrales y conteos después de filtros compatibles; las etapas que alteran valores mantienen fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 amplía la receta lazy/streaming para combinar parseos de fecha con conversiones en columnas distintas y ejecutar `split` y `merge` en una misma receta cuando se conservan sus dependencias; los conflictos de fuentes mantienen rechazo o fallback eager explícito. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | M1/P1 reduce el pico temporal de restauración de `history_snapshots`: cada Parquet histórico se lee y publica secuencialmente, conserva validación de etiquetas/cursor/cancelación y no acumula todos los `DataFrame` antes del commit; la ejecución lazy del dataset activo y los presupuestos globales de datasets grandes continúan pendientes. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | M1 reconoce el campo `filename` que emite sistema anterior como referencia local reproducible cuando falta `source_path`: lo resuelve junto al manifiesto, valida que sea un archivo regular y conserva nombre/estado sin filtrar la ruta al bridge. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 amplía las recetas IQR lazy/streaming para convivir con `keep_columns` cuando se conservan todas las columnas tratadas; si una proyección elimina una dependencia, se mantiene el fallback eager y el rechazo explícito. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | M1 corrige la migración de pipelines sistema anterior persistidos: el campo real `selected` ahora se reconoce junto a sus aliases de sesión, normaliza las operaciones deterministas y conserva la selección al importar la receta. | `src-tauri/src/dataset.rs`, `docs/reference/feature-parity.md` |
| 2026-08-30 | M1 completa el mapeo de pipelines al catálogo mediante `project-save --recipe`: valida el `--input`, reproduce las limpiezas deterministas seleccionadas y aplica la receta estructural antes de publicar el snapshot; una regresión verifica el proyecto reabierto. | `src-tauri/src/automation.rs`, `src-tauri/src/projects.rs`, `ROADMAP.md` |
| 2026-08-30 | P1/M1 cierra el catálogo de limpieza sugerida: las 22 operaciones de `sistema anterior` comparten una lista canónica con el replay de sesiones, y una prueba evita que una operación registrada quede sin mapping migrable. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | M1 corrige la paridad de la actividad de sesiones con el `ExecutionHistory` real de sistema anterior: `completed`/`failed`, duraciones decimales y `rows_out`/`rowsOut` se normalizan a la actividad agregada segura de Columnia; consultas, rutas y valores siguen descartados. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-30 | M1 añade la fixture `sistema anterior-session-v3-real.json`, basada en la forma v3 de `SessionRecipe`, y verifica importar → reabrir con etapa, reglas, muestreo agregado y tres entradas de actividad normalizadas; resultados, cachés, consultas y rutas privadas no cruzan al workspace. | `fixtures/manifest.json`, `fixtures/manifest.json`, `src-tauri/src/projects.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 conecta el límite de entradas de Polars con la ruta DuckDB: un JOIN grande se promueve automáticamente a DuckDB cuando puede reutilizar los snapshots Parquet administrados del activo y la comparación, evitando recargar la segunda fuente completa; sin ambos snapshots se conserva el rechazo seguro, y la ejecución incremental general fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 elimina la materialización completa del anti-join derecho en `FULL JOIN` local: derrama el índice temporal de claves del activo, recorre la comparación por bloques de 16K, conserva `NULL` como no emparejado y mantiene duplicados/orden; los `DataFrame` fuente y la ejecución general fuera de RAM siguen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | I3 fija los perfiles Cargo `dev` y `test` sin símbolos de depuración para evitar `LNK1140` en el enlazado MSVC del binario Tauri; `npm run tauri dev` queda reproducible desde `Columnia` sin variables temporales y `release` mantiene su política independiente. | `src-tauri/Cargo.toml`, `README.md`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | I3 valida el benchmark WebView2 de dataset grande: un input sintético de 100 MiB y 819.137 filas completa carga, paginación, transformación y exportación; el pico observado queda en 691.789.824 B de working set y 460.587.008 B privados, con presupuesto y cleanup aprobados. La ejecución general fuera de RAM sigue pendiente. | `.local/validation/performance-webview2/20260831T031622Z`, `.local/validation/webview2-cdp/20260831T031624Z`, `.local/validation/performance-baseline/20260831T031959Z` |
| 2026-08-30 | P1 amplía DuckDB para registrar directamente la fuente original CSV/TSV/TXT delimitada o Parquet cuando el historial está degradado: las consultas explícitas y los JOINs grandes pueden combinarla con el snapshot comparado sin crear una copia Parquet del activo; cualquier mutación invalida la referencia y conserva el fallback materializado. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 procesa la comparación inicial de fuentes Parquet, delimitadas y JSON por bloques: copia o genera el snapshot temporal de forma secuencial, cuenta filas por streaming y calcula intersección, claves y conflictos desde índices temporales; Excel y el dataset activo conservan la materialización actual. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.58.0 procesa la comparación inicial de libros XLSX/XLSB con el lector secuencial de celdas de Calamine: detecta esquema en una pasada, escribe Parquet por bloques de 16K y compara desde snapshot; XLS/ODS conservan fallback compatible. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.59.0 permite que la consulta Polars use automáticamente DuckDB sobre la fuente original CSV/TSV/TXT delimitada o Parquet cuando el historial se degrada; las consultas compatibles, incluidos JOINs con snapshots comparados, evitan reconstruir la fuente desde el `DataFrame` y mantienen fallback seguro. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.60.0 promueve todos los JOIN compatibles elegidos por Polars a DuckDB cuando el activo y la comparación tienen snapshots o fuentes de disco válidas, no solo los JOIN grandes; la ruta evita materializar ambos datasets y conserva fallback seguro. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.61.0 extiende la paginación source-backed al historial degradado: un dataset intacto lee solo la ventana solicitada desde su fuente original Parquet o CSV/TSV/TXT, comprueba el tamaño de la fuente y la cantidad esperada de filas de la página, y vuelve al frame ante inconsistencias. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.62.0 extiende la comparación inicial source-backed al lado activo: reutiliza su snapshot Parquet o una fuente original Parquet/CSV/TSV/TXT intacta, compara ambos lados por bloques e índices temporales sin clonar el `DataFrame` y conserva fallback ante inconsistencias. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.80.0 extiende las recetas source-backed: renombres y selección/orden de columnas se convierten directamente desde CSV/TSV/TXT delimitado o Parquet a un snapshot Parquet privado, sin materializar las filas del `DataFrame` activo; se conserva fallback eager para transformaciones de valores y una regresión comprueba paridad. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.70.0 extiende la validación source-backed: unicidad simple/compuesta, monotonicidad, agregados y deriva de distribución recorren bloques y conservan conteos globales sin materializar la fuente; una regresión verifica duplicados que cruzan bloques. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.69.0 extiende la exportación source-backed: JSON sin receta ni privacidad adicional convierte directamente CSV/TSV/TXT delimitado o Parquet desde la fuente con límites de DuckDB, publicación atómica, validación de cambios y cleanup; la regresión confirma una salida JSON válida sin materializar el `DataFrame` activo. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.68.0 extiende la exportación source-backed: Parquet sin receta ni privacidad adicional se convierte directamente desde la fuente con límites de DuckDB, publicación atómica, validación de cambios y cleanup; la regresión confirma una salida legible sin materializar el `DataFrame` activo. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.67.0 extiende la validación source-backed: las reglas fila-a-fila y de esquema/conteo recorren bloques Parquet sin materializar la fuente completa, comprueban cambios de tamaño y conservan fallback para reglas globales; la paridad con la validación en memoria queda cubierta por regresión Rust. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.66.0 extiende el perfilado source-backed: derrama firmas exactas y normalizadas por cubetas, perfila columnas por bloques, ordena corridas numéricas en disco y limita categorías, tendencias y correlaciones a las columnas/muestras necesarias; la paridad con el perfil en memoria queda cubierta por regresión Rust. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.65.0 abre CSV/TSV/TXT delimitados y Parquet de al menos 512 MiB source-backed: conserva esquema, primera página y conteo desde disco; paginación y consultas compatibles evitan el `DataFrame` completo, y operaciones eager materializan bajo demanda con validación de cambios. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.64.0 configura la misma frontera de 512 MB de memoria y hasta 8 GB de derrame temporal privado para consultas DuckDB y conversión source-backed a snapshots Parquet; la regresión delimitada verifica legibilidad y cleanup. | `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |

### Decisiones cerradas que Tier 5 conserva

- SEO no se incorpora mientras Columnia siga sin superficie web indexable.
- No se añade CI/GitHub Actions: los gates seguirán siendo locales y deberán
  entregar códigos de salida fiables y evidencia ligada al commit.
- No se exige Authenticode de pago; sí se debe documentar SmartScreen y no se
  confunde checksum con autoría.
- macOS/Linux no se declaran soportados hasta verificarse localmente.
- No se eleva un presupuesto ni se aprueba un baseline solo para obtener verde.

## 9. Registro de decisiones

| Fecha | Decisión | Estado |
| --- | --- | --- |
| 2026-08-26 | Evaluar filtros SQL locales por bloques Rayon; las consultas paginadas cuentan en paralelo y solo reescanean los bloques que contienen la ventana solicitada, mientras los agregados conservan todos sus índices | Implementada en `src-tauri/src/dataset.rs`; joins grandes y ejecución DuckDB siguen en cola |
| 2026-08-26 | Hacer explícito el progreso de Cargar, Diagnóstico y Entrega con contexto, etapa actual, estado de cancelación y porcentaje acotado, manteniendo la región viva accesible | Implementada en `src/components/OperationProgressView.tsx` y `src/styles.css` |
| 2026-08-26 | Las consultas SQL locales no agregadas cuentan coincidencias en una pasada y conservan solo la ventana `LIMIT/OFFSET`; las agregaciones mantienen todas sus filas para calcular resultados exactos | Implementada en `src-tauri/src/dataset.rs`; joins grandes y ejecución DuckDB siguen en cola |
| 2026-08-26 | Contar duplicados exactos con `unique` lazy en motor streaming y proyectar solo el total, con fallback eager exacto si el backend no soporta el plan | Implementada en `src-tauri/src/dataset.rs`; la materialización del `DataFrame` activo y el perfil incremental completo siguen en cola |
| 2026-08-26 | Reutilizar una única conversión `Float64` durante el perfil numérico para compartirla entre histograma y atípicos y reducir picos de memoria | Implementada en `src-tauri/src/dataset.rs` |
| 2026-08-26 | Paralelizar la construcción de firmas para comparación y claves con reducciones Rayon, ordenando los índices por clave al final para conservar resultados deterministas | Implementada en `src-tauri/src/dataset.rs`; joins/comparación incremental de datasets que exceden memoria sigue en cola |
| 2026-08-26 | Publicar histogramas numéricos como dato derivado del perfil, con límites estables y tabla equivalente accesible; los análisis exploratorios amplios permanecen como siguiente expansión | Implementada como primera slice de visualización |
| 2026-08-26 | Mostrar antes de ejecutar una receta su impacto estimado, riesgo, confianza y alternativas de recuperación; las estimaciones basadas en la muestra se etiquetan explícitamente | Implementada como primera slice del asesor de transformaciones |
| 2026-08-27 | Sanitizar en una frontera común los JSON públicos de la CLI para eliminar rutas, nombres de archivo, valores, emails, secretos y referencias privadas en reportes, recetas y manifiestos, conservando identificadores, estados y conteos | Implementada; futuros conectores remotos y artefactos adicionales requieren ampliar el contrato |
| 2026-08-27 | Añadir cancelación cooperativa a SQL local, presupuesto de agregaciones y preflight de cardinalidad para rechazar joins many-to-many antes de materializar resultados fuera de límite | Implementada en `src-tauri/src/dataset.rs`; DuckDB y ejecución incremental completa siguen en cola |
| 2026-08-27 | Conservar hasta cinco archivos recientes sin rutas y reabrir siempre el selector nativo al elegir uno | Implementada en `src/features/load/recentFilesModel.ts` y `src/features/load/LoadPhase.tsx` |
| 2026-08-27 | Mostrar actividad SQL de la sesión actual con las últimas cinco ejecuciones, estado, duración y filas, sin persistir texto de consulta ni valores | Implementada en `src/features/review/ReviewPhase.tsx`; la actividad se conserva además en el workspace del proyecto al guardarlo |
| 2026-08-27 | Añadir arrastre nativo de datasets sin exponer rutas al frontend: Tauri guarda temporalmente el primer archivo soltado, notifica un evento opaco y reutiliza la inspección segura del selector | Implementada en `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts` y `src/App.tsx`; los conectores remotos siguen en cola |
| 2026-08-29 | Retener en Rust el último destino exportado durante la sesión y abrir únicamente su carpeta tras una nueva validación de archivo regular, sin aceptar rutas arbitrarias desde React | Implementada en `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts` y `src/features/delivery/DeliveryPhase.tsx`; la retención no es durable tras reiniciar |
| 2026-08-26 | Retirar el límite provisional de 500 MiB para datasets; la capacidad efectiva depende de la RAM, el espacio en disco y los demás recursos disponibles | Implementada |
| 2026-08-26 | Paralelizar el perfilado por columna con una cola acotada de hasta cuatro trabajadores, mantener el orden de resultados y publicar progreso ponderado por sub-etapa para datasets grandes | Implementada en `src-tauri/src/dataset.rs`; la lectura lazy/incremental completa sigue en cola |
| 2026-08-26 | Usar `LazyCsvReader` con motor streaming, baja memoria y `rechunk` desactivado para CSV, TSV y TXT delimitado; se conserva un `DataFrame` activo para mantener la compatibilidad actual | Implementada; extender el mismo límite a Parquet cacheado, joins, comparación e historial sigue en cola |
| 2026-08-26 | Extender la lectura streaming a Parquet mediante `scan_parquet`, conservando `parallel: None`, baja memoria y `rechunk` desactivado para evitar picos innecesarios | Implementada en la carga inicial; cacheado, joins, comparación e historial incremental siguen en cola |
| 2026-08-29 | Centralizar la colección Polars de lectores delimitados, Parquet y recetas lazy compatibles en el motor `streaming`, manteniendo fallback eager solo para operaciones no compatibles | Implementada como primera expansión de operaciones; el `DataFrame` activo, cacheado Parquet, joins/comparación e historial incremental siguen en cola |
| 2026-08-29 | Reutilizar la lectura `scan_parquet` con motor `streaming` al abrir y validar snapshots de proyectos e historial temporal, sin cambiar el contrato durable | Implementada como segunda expansión; el frame activo aún se materializa y joins/comparación, operaciones eager e historial degradado siguen en cola |
| 2026-08-29 | Restaurar el historial durable de forma incremental, reteniendo solo el frame del cursor para validar consistencia mientras las demás entradas se procesan una por una | Implementada como tercera expansión; el frame activo, joins/comparación, operaciones eager e historial degradado siguen en cola |
| 2026-08-29 | Fusionar las firmas de comparación por bloques de 16K filas y eliminar conjuntos de claves auxiliares redundantes, conservando conteos exactos y orden determinista | Implementada como cuarta expansión; el mapa global de comparación y los joins todavía requieren presupuesto incremental específico |
| 2026-08-29 | Ejecutar los JOIN locales por claves mediante un plan Polars con motor `streaming` después del preflight de cardinalidad, manteniendo límites y comprobación posterior | Implementada como quinta expansión; la entrada sigue siendo un `DataFrame` materializado y DuckDB/join fuera de memoria continúan en cola |
| 2026-08-29 | Incorporar `keep_columns` a la familia de recetas lazy, proyectando dentro del plan `streaming` y validando dependencias calculadas antes de materializar | Implementada como sexta expansión; la entrada y el candidato activo siguen siendo `DataFrame` y las operaciones eager restantes continúan en cola |
| 2026-08-29 | Incorporar búsqueda/reemplazo literal sobre texto a la familia lazy, con conteo streaming de celdas modificadas y semántica estable para nulos, renombres y casts a texto | Implementada como séptima expansión; el dataset activo y las operaciones estructurales restantes siguen requiriendo materialización |
| 2026-08-29 | Incorporar unión de columnas de texto a la familia lazy mediante `concat_str`, preservando orden, nulos y casts numérico→texto validados | Implementada como octava expansión; la entrada y el candidato activo siguen siendo `DataFrame` y `split_columns`/operaciones avanzadas continúan en cola |
| 2026-08-29 | Incorporar división literal de texto a la familia lazy mediante `splitn`, conservando el resto, nulos, validaciones y descarte de la fuente | Implementada como novena expansión; la entrada y el candidato activo siguen siendo `DataFrame` y las operaciones avanzadas continúan en cola |
| 2026-08-29 | Incorporar agrupación y resúmenes tipados al plan lazy con orden estable, claves nulas, conteo de filas, `count_unique` y preflight de finitos, precisión y desbordamiento | Implementada como décima expansión; la entrada y el candidato activo siguen limitando la ejecución fuera de memoria |
| 2026-08-29 | Incorporar normalización lazy de contactos para correo, teléfono y dirección, preservando nulos, espacios Unicode y conteos exactos de celdas modificadas | Implementada como undécima expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-29 | Incorporar extracción lazy de tokens, runs Unicode y segmentos antes/después de delimitadores literales, preservando nulos y coincidencias ausentes | Implementada como duodécima expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Incorporar cálculos lazy de suma, resta, multiplicación, división y concatenación, con validación previa de operandos, nulos, división por cero e infinitos | Implementada como decimotercera expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Ejecutar búsqueda/reemplazo literal antes de agrupación y resúmenes dentro del mismo plan lazy, preservando conteos y la semántica de claves transformadas | Implementada como decimocuarta expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Incorporar extracción lazy de año, mes y día sobre `Date`/`Datetime` sin zona horaria, con preflight de rango y fallback explícito para filtros o zonas horarias | Implementada como decimoquinta expansión; filtros y fechas con zona horaria mantienen fallback eager; la entrada/candidato siguen materializados |
| 2026-08-30 | Extender agrupación y resúmenes lazy a filtros previos, con preflight streaming de las columnas necesarias, conteo correcto de filas retiradas y validación exacta de sumas supervivientes | Implementada como decimosexta expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Permitir normalización lazy de contactos antes de agrupación, validando sobre la proyección posterior a la normalización y preservando los contadores de cambios | Implementada como decimoséptima expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Permitir extracciones textuales lazy antes de agrupación, usando columnas derivadas como claves o fuentes de agregación y conservando nulos, orden y preflight | Implementada como decimoctava expansión; la entrada y el candidato activo siguen materializados |
| 2026-08-30 | Permitir columnas calculadas lazy numéricas y concatenadas antes de agrupación, usando sus resultados como claves o fuentes de agregación con preflight posterior | Implementada como decimonovena expansión; partes temporales, split/merge derivados, la entrada y el candidato activo mantienen sus límites actuales |
| 2026-08-30 | Permitir columnas derivadas lazy de split/merge antes de agrupación, usando sus resultados como claves o fuentes de agregación con preflight posterior a las etapas estructurales | Implementada como vigésima expansión; partes temporales derivadas, la entrada y el candidato activo mantienen sus límites actuales |
| 2026-08-30 | Permitir partes temporales calculadas lazy de año/mes/día como claves de agrupación sobre `Date` y `Datetime` sin zona horaria, con preflight de rango | Implementada como vigésimo primera expansión; filtros previos y zonas horarias mantienen fallback eager, y la entrada/candidato activo siguen materializados |
| 2026-08-30 | Extender la consulta SQL local restringida a claves compuestas de hasta ocho columnas, formando grupos en orden estable y conservando claves nulas sin superar el presupuesto de filas | Implementada como vigésimo segunda expansión; la consulta sigue siendo solo lectura sobre el `DataFrame` activo y DuckDB/ejecución fuera de memoria continúan en cola |
| 2026-08-30 | Extender `find_replace` a expresiones regulares seguras en recetas eager/lazy y migración sistema anterior, conservando grupos de captura, nulos, conteos y validación previa del patrón | Implementada como slice de migración y ejecución; la entrada y el candidato activo siguen materializados y el catálogo avanzado restante continúa en cola |
| 2026-08-30 | Extender el parser SQL local para JOINs compuestos de hasta ocho pares, reutilizando el preflight existente de tipos/cardinalidad y el plan streaming con orden izquierdo | Implementada como vigésimo tercera expansión; no amplía aún el motor a DuckDB ni la ejecución fuera de memoria |
| 2026-08-30 | Procesar agregaciones SQL locales en una segunda pasada por bloques, con acumuladores por grupo y sin guardar índices de filas coincidentes | Implementada como vigésimo cuarta expansión; el `DataFrame` activo, el límite de coincidencias y los resultados paginados siguen siendo el contrato actual |
| 2026-08-30 | Particionar el índice exacto de comparación por claves en cubetas temporales y procesar cada cubeta de forma independiente | Implementada como vigésimo quinta expansión; la comparación de filas completas, el `DataFrame` activo y los joins fuera de memoria siguen pendientes |
| 2026-08-30 | Particionar también las firmas completas de filas y calcular la intersección de multiconjuntos por cubeta | Implementada como vigésimo sexta expansión; el `DataFrame` activo, las operaciones eager restantes y los joins fuera de memoria siguen pendientes |
| 2026-08-30 | Particionar el preflight de cardinalidad de JOIN y calcular duplicidades por cubeta antes de materializar el resultado | Implementada como vigésimo séptima expansión; la materialización final del JOIN, DuckDB y los joins fuera de memoria siguen pendientes |
| 2026-08-30 | Combinar parseos de fecha con conversiones en columnas distintas y ejecutar `split` con `merge` dentro de una misma receta lazy | Implementada como vigésimo octava expansión; las dependencias que descartan una fuente necesaria conservan rechazo o fallback eager, y la entrada/candidato activo siguen materializados |
| 2026-08-30 | Reducir el pico de RAM de historiales Parquet en importación de sesión, guardado y apertura: copiar snapshots byte a byte, validar footer/esquema para revisiones no cursor y materializar solo el cursor para comprobar consistencia | Implementada como vigésimo novena expansión; undo/redo conserva la lectura completa bajo demanda, mientras la ejecución incremental del dataset activo y los presupuestos globales siguen en cola |
| 2026-08-30 | Alinear y endurecer el contrato frontend de metadatos de sesión sistema anterior: declarar `nonPortableArtifacts` y rechazar contadores no enteros/no negativos, banderas inválidas y categorías mal formadas | Implementada como hardening de la vertical M1; los resultados, cachés y rutas no portables siguen requiriendo revisión manual |
| 2026-08-26 | Derramar fingerprints XXH3 de duplicados normalizados en 256 cubetas temporales y ordenar una cubeta a la vez; se conserva el conteo, el orden de las filas y la cancelación sin guardar valores del dataset | Implementada en `src-tauri/src/dataset.rs`; la materialización del `DataFrame`, transformaciones eager y joins fuera de memoria siguen en cola |
| 2026-08-27 | Añadir tendencia temporal diaria para rangos de hasta 90 días, con días vacíos, límite de periodos, cancelación cooperativa y tabla accesible equivalente; rangos mayores mantienen la agregación mensual/anual | Implementada en `src-tauri/src/dataset.rs`, `src/bridge.ts` y `src/features/review/ReviewPhase.tsx` |
| 2026-08-23 | Cerrar Fase I0: MIT, Windows x64 inicial, frontera Rust/UI, validación local y fixtures sintéticas | Aprobada; `docs/adr/0001-contratos-del-repositorio.md` |
| 2026-08-12 | Usar `../sistema anterior/` como referencia funcional, no como plantilla técnica automática | Aprobada |
| 2026-08-12 | Nombre del producto y del proyecto: `Columnia` | Aprobada |
| 2026-08-12 | Backend: Rust + Tauri 2 + Polars + DuckDB | Aprobada |
| 2026-08-12 | Frontend: React + TypeScript + Vite | Aprobada |
| 2026-08-12 | Conservar los gates de seguridad, E2E, SBOM y rendimiento de sistema anterior, ejecutados solo localmente | Aprobada |
| 2026-08-12 | Adoptar de ProcessDevKill los patrones de empaquetado, instancia única, actualización, release y evidencia visual | Propuesta |
| 2026-08-12 | No usar CI, GitHub Actions ni workflows; centralizar validación en scripts locales | Aprobada |
| 2026-08-12 | No usar componentes obligatorios de pago ni comprar firma Authenticode | Aprobada |
| 2026-08-12 | Evaluar el updater oficial de Tauri con firma local gratuita en vez del modelo SHA-256 del mismo release | Propuesta |
| 2026-08-12 | NSIS por usuario como instalador Windows principal; MSI condicionado a demanda empresarial | Propuesta |
| 2026-08-12 | Diseñar Columnia para Windows, macOS y Linux; verificar cada plataforma localmente | Aprobada |
| 2026-08-12 | Arquitectura base Rust + Tauri 2 + React + TypeScript + Vite; Polars y DuckDB se incorporarán tras el scaffold | Aprobada |
| 2026-08-12 | Scaffold inicial creado; frontend compilado y probado localmente | Implementada |
| 2026-08-12 | Icono maestro y recursos Tauri multiplataforma generados; build nativo Windows verificado | Implementada |
| 2026-08-12 | Primer corte vertical CSV: diálogo nativo en Rust, Polars, sesión local y preview de 50 filas con límite provisional de 100 MB | Implementada |
| 2026-08-12 | Paginación por sesión y perfil inicial de calidad por columna ejecutados localmente en Rust | Implementada |
| 2026-08-12 | Detección de filas duplicadas y métricas específicas de columnas textuales añadidas al perfil | Implementada |
| 2026-08-12 | Sugerencias conservadoras de tipos ocultos en columnas textuales, sin transformación automática | Implementada |
| 2026-08-12 | Perfil numérico avanzado con desviación muestral, cuartiles, mediana y outliers IQR | Implementada |
| 2026-08-12 | Primera transformación reversible: eliminar duplicados exactos en sesión con un nivel de deshacer | Implementada |
| 2026-08-12 | Versión 0.2.0 sincronizada entre npm, Cargo y Tauri mediante prueba local | Implementada |
| 2026-08-12 | Navegación lateral por vistas y límite provisional de CSV elevado a 500 MB con advertencia de RAM | Implementada |
| 2026-08-12 | Calidad reutiliza exclusivamente el dataset activo; la selección de CSV queda aislada en Datos | Implementada |
| 2026-08-12 | Versión 0.3.0: progreso tipado por canal Tauri para carga CSV y perfilado por columnas | Implementada |
| 2026-08-12 | Versión 0.4.0: cancelación cooperativa aislada por operación y recuperación del dataset previo | Implementada |
| 2026-08-12 | Versión 0.5.0: exportación atómica y cancelable CSV/Parquet mediante selector nativo | Implementada |
| 2026-08-12 | Versión 0.6.0: flujo Cargar → Revisar → Preparar → Entregar alineado con `sistema anterior`; exportación aislada en Entregar | Implementada |
| 2026-08-12 | Versión 0.7.0: normalización reversible y determinista de nombres de columnas en Preparar | Implementada |
| 2026-08-12 | Versión 0.8.0: recorte de espacios y normalización explícita de texto con métricas exactas | Implementada |
| 2026-08-13 | Versión 0.9.0: Deshacer/Rehacer de una revisión y aplicación atómica de correcciones recomendadas | Implementada |
| 2026-08-13 | Versión 0.10.0: carga local CSV/Parquet con preservación de esquema Parquet y lector de baja memoria | Implementada |
| 2026-08-13 | Versión 0.11.0: TSV fiel y carga Excel/ODS con selección de hoja mediante token opaco | Implementada |
| 2026-08-13 | Versión 0.12.0: carga JSON de registros/JSON Lines con esquema tabular conservador | Implementada |
| 2026-08-13 | Versión 0.13.0: fidelidad léxica CSV con perfil numérico semántico y protección de identificadores | Implementada |
| 2026-08-13 | Versión 0.14.0: opciones Excel, delimitadores/UTF-8 conservadores y gates locales de seguridad/calidad | Implementada |
| 2026-08-13 | Versión 0.15.0: receta estructural atómica para renombres, tipos y fechas en Preparar | Implementada |
| 2026-08-13 | Versión 0.16.0: filtros AND y columna calculada estricta dentro de la receta atómica | Implementada |
| 2026-08-13 | Versión 0.17.0: reemplazo literal y selección de columnas dentro de la receta atómica | Implementada |
| 2026-08-13 | Versión 0.18.0: división y combinación deterministas de columnas de texto | Implementada |
| 2026-08-13 | Versión 0.19.0: tratamiento IQR atómico para limitar o eliminar outliers | Implementada |
| 2026-08-13 | Versión 0.20.0: agrupación estable y resúmenes tipados como etapa final | Implementada |
| 2026-08-13 | Versión 0.21.0: normalización de contactos y extracción literal Unicode | Implementada |
| 2026-08-13 | Versión 0.22.0: historial local multinivel con snapshots Parquet y presupuesto explícito | Implementada |
| 2026-08-14 | Versión 0.23.0: recetas estructurales JSON v1 guardables y cargables localmente | Implementada |
| 2026-08-15 | Versión 0.24.0: contratos de calidad exactos y compuerta obligatoria antes de exportar | Implementada |
| 2026-08-21 | Versión 0.25.0: proyectos SQLite recuperables con generaciones Parquet, reglas y borrador de receta | Implementada |
| 2026-08-21 | Versión 0.26.0: SQLite v3 con perfil, historial y cursor durables, compatible con catálogos v1/v2 | Implementada |
| 2026-08-21 | Versión 0.27.0: cinco comandos CLI de proyectos con almacén explícito, JSON privado, quality gate y borrado confirmado | Implementada |
| 2026-08-22 | Versión 0.28.0: integración Vitest del ciclo UI guardar/borrar con conservación del dataset y preflight/runtime smoke de `ProjectsPanel` | Implementada |
| 2026-08-22 | Versión 0.29.0: cobertura UI guardar→abrir→restaurar, targets WCAG/reduced motion y baseline local de startup/bundle | Implementada |
| 2026-08-22 | Versión 0.30.0: hitos y reintento de cleanup en smoke Tauri, más contratos automatizados de landmarks/ARIA/dialog | Implementada |
| 2026-08-22 | Versión 0.31.0: Playwright 1.62 con Edge en Windows/Chromium en otros sistemas y E2E reproducible del shell web; IPC/Tauri nativo queda pendiente | Implementada |
| 2026-08-22 | Versión 0.32.0: E2E Playwright del ciclo de proyectos con `__TAURI_INTERNALS__` simulado; WebView2/IPC nativo queda pendiente | Implementada |
| 2026-08-22 | Versión 0.33.0: marca de primer render y presupuesto Playwright independiente de 3 s para el shell web; medición WebView2 nativa queda pendiente | Implementada |
| 2026-08-22 | Versión 0.34.0: probe Windows WebView2/CDP aislado con lectura DOM mediante Playwright y cleanup por Job Object; mutaciones IPC nativas quedan pendientes | Implementada |
| 2026-08-22 | Versión 0.35.0: primer render, landmarks y foco verificados dentro de WebView2 mediante CDP; E2E responsive/reduced-motion añadido; acciones IPC de proyectos siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.36.0: smoke CDP combinado con contrato accesible de solo lectura de `ProjectsPanel` y resumen local de rendimiento con deltas; acciones IPC que abren diálogos o escriben proyectos siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.37.0: el probe CDP de `ProjectsPanel` invoca `list_projects` y `get_recovery_candidate` de forma nativa, valida formas sin rutas y conserva evidencia sin datos; las mutaciones IPC siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.38.0: benchmark CLI reproducible de 100 MiB con inspect/validate/transform CSV+Parquet, duración, working set y cleanup sin conservar datos; RAM final y perfilado Tauri siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.39.0: el probe WebView2/CDP registra working set y memoria privada inicial/máxima/final del proceso debug y los conserva en el resumen sin rutas ni datos; presupuesto global y transformaciones nativas siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.40.0: recorrido IPC nativo temporal de dataset sintético y proyecto (guardar/listar/abrir/paginar/eliminar) bajo debug, con cleanup y evidencia privada; selector/exportación y continuidad entre procesos siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.41.0: receta JSON temporal, aplicación estructural, exportación CSV atómica con quality gate y workspace de proyecto restaurable verificados dentro de WebView2; selector nativo y continuidad entre procesos siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.42.0: reapertura durable desde una instancia fresca de `ProjectStore` con SQLite, snapshot, recovery y workspace verificados dentro del probe WebView2; selector nativo y reinicio de proceso real siguen pendientes | Implementada |
| 2026-08-22 | Versión 0.43.0: dos procesos Tauri/WebView2 independientes preparan, cierran, reinician, recuperan, abren y eliminan un proyecto sintético; selector nativo sigue pendiente | Implementada |
| 2026-08-22 | Versión 0.44.0: telemetría IPC sanitizada por operación y presupuesto CDP de memoria aplicado al árbol nativo, con resumen de rendimiento ampliado | Implementada |
| 2026-08-22 | Versión 0.45.0: estilos `forced-colors` y evidencia visual reproducible en desktop, móvil, escala 125% y alto contraste; lector de pantalla manual sigue pendiente | Implementada |
| 2026-08-23 | Versión 0.46.0: baseline visual con hashes, gate de rendimiento CDP/benchmark/bundle y `verify:experience` compuesto; lazy/incremental sigue pendiente | Implementada |
| 2026-08-23 | Versión 0.47.0: benchmark CLI sostenido con tres iteraciones y ciclo durable de proyecto con cleanup; medición WebView2 y lazy/incremental siguen pendientes | Implementada |
| 2026-08-23 | Versión 0.48.0: presupuestos de duración, stress de actualización/reapertura durable, `verify:tier` y checklist manual de accesibilidad; medición WebView2, lector real y lazy/incremental siguen pendientes | Implementada |
| 2026-08-23 | Versión 0.49.0: tres ciclos nativos de transformación/exportación dentro de WebView2, duraciones incorporadas al gate y presupuesto global del árbol; datasets grandes, lector real y lazy/incremental siguen pendientes | Implementada |
| 2026-08-23 | Fase I8: documentación Diátaxis, índice de ADR/CHANGELOG, validadores de enlaces/UTF-8/versiones/ownership y evidencia visual reproducible desde el binario release con baseline de hashes; lector de pantalla manual sigue en I3 | Implementada |
| 2026-08-23 | Fase I1 completa: receta Polars lazy con fallback eager seguro, monitor nativo compacto de CPU/RAM, benchmark cruzado de 100 MiB contra `sistema anterior` y revisión visual desktop/móvil/zoom/forced-colors | Implementada |
| 2026-08-23 | P1: contrato de calidad v3 ampliado con `allowed_values`, `regex`, `dtype`, unicidad compuesta y `row_count`; evaluación Rust/CLI/exportación, bridge/UI accesibles y tests end-to-end | Implementada |
| 2026-08-24 | P1: `column_compare` completa la comparación de dos columnas con seis operadores, nulos inválidos, tolerancias, migración sistema anterior, editor accesible y evaluación compartida por UI/CLI/exportación | Implementada |
| 2026-08-24 | Versión 0.50.0: resolución de conflictos por columna/valor dentro del preview visible, compatibilidad legacy por fila y privacidad aplicada a los seis destinos locales | Implementada |
| 2026-08-24 | Versión 0.51.0: conflictos por clave paginados en bloques de 50, índices globales para decisiones y resolución completa fuera del preview con historial reversible; privacidad de artefactos sigue pendiente | Implementada |
| 2026-08-24 | P1: `date_range` añade límites inclusivos de fecha, soporte texto/Date/Datetime, rechazo de nulos/fechas ilegibles, migración sistema anterior, editor accesible y evaluación compartida | Implementada |
| 2026-08-24 | P1: `conditional` añade condiciones eq/ne/lt/lte/gt/gte, subreglas then fila-a-fila seguras, tolerancia exterior, migración sistema anterior, editor accesible y evaluación compartida | Implementada |
| 2026-08-24 | P1: `schema_contract` añade columnas requeridas, control de adicionales y orden opcional, migración sistema anterior, editor accesible y evaluación estructural compartida | Implementada |
| 2026-08-24 | P1: documento de calidad `columnia-quality-rules` v1 con guardado atómico, compatibilidad explícita Columnia/sistema anterior v1–v3/legado, rechazo de versiones futuras, CLI retrocompatible y UI sin exposición de rutas | Implementada |
| 2026-08-24 | I3: selector nativo Win32 estabilizado y elevado a `verify:tier`; el smoke WebView2 verifica abrir dataset, guardar/cargar receta y exportar con variantes de editor Abrir/Guardar como, cleanup y evidencia sin rutas. Quedan lector de pantalla, High Contrast manual, datasets grandes y VM limpia | Parcial, con evidencia |
| 2026-08-24 | Versión 0.52.0: sistema visual explícito con selector persistente `Sistema`/`Claro`/`Oscuro`, aplicación temprana en el documento, capa visual refinada y estilos compatibles con foco, movimiento reducido y `forced-colors` | Implementada |
| 2026-08-24 | Versión 0.53.0: el selector de recetas importa el núcleo representable de pipelines sistema anterior v1–v3 a receta Columnia v1 y rechaza semánticas ambiguas antes de modificar el borrador | Implementada |
| 2026-08-24 | Versión 0.54.0: optimización de arranque con code-splitting de etapas pesadas, estado inicial local sin esperar `get_app_info`, migración SQLite diferida y monitor de recursos fuera del primer paint | Implementada |
| 2026-08-24 | Versión 0.55.0: la migración de contratos de calidad genera un informe auditable con conteos, omisiones, advertencias, acciones manuales y SHA-256 del artefacto sin exponer rutas ni valores | Implementada |
| 2026-08-24 | Versión 0.56.0: los pipelines sistema anterior conservan opciones de entrega compatibles, normalizan XLSX a Excel y publican informe con warnings para semánticas de exportación omitidas, sin exponer rutas | Implementada |
| 2026-08-24 | Versión 0.57.0: inventario y fixtures sintéticas de migración para pipelines, sesiones, calidad y legacy; los metadatos de sesión se reconocen y se omiten con warnings sanitizados | Implementada |
| 2026-08-26 | P1 añade cobertura y tendencia temporal agregadas en Review: rangos Date/Datetime/Timestamp, conteos por mes/año, filas interpretables y porcentaje con tabla equivalente; el calendario diario y las series temporales completas siguen pendientes | Implementada |
| 2026-08-26 | M1 añade una slice segura para mapear sesiones sistema anterior al catálogo de proyectos: selector nativo, validación de fuente/hoja/esquema/receta, restauración de `snapshot_path` compatible cuando falta la fuente y publicación transaccional sin tocar el dataset activo; una prueba nativa cubre importar → reabrir → validar → exportar, mientras la restauración histórica completa sigue pendiente | Implementada |
| 2026-08-27 | P1 incorpora un calendario diario accesible para rangos cortos, conserva días sin filas y mantiene la tabla exacta equivalente; las series temporales completas siguen pendientes | Implementada |
| 2026-08-27 | P1 incorpora retiro explícito y reversible de columnas identificadoras detectadas por encabezado, con confirmación, conservación de una columna y protección de valores personales | Implementada |
| 2026-08-27 | M1 incorpora `project-save --recipe` en la CLI: migra sesiones con validación previa, fallback a snapshot compatible, salida JSON sanitizada y creación de proyectos nuevos sin reemplazos | Implementada |

## 10. Fuentes de esta revisión

- [Repositorio ProcessDevKill](https://github.com/xfiberex/ProcessDevKill)
- [README de ProcessDevKill](https://github.com/xfiberex/ProcessDevKill/blob/main/README.md)
- [Configuración Tauri](https://github.com/xfiberex/ProcessDevKill/blob/main/src-tauri/tauri.conf.json)
- [Capabilities de ProcessDevKill](https://github.com/xfiberex/ProcessDevKill/blob/main/src-tauri/capabilities/default.json)
- [Automatización de release](https://github.com/xfiberex/ProcessDevKill/blob/main/release.ps1)
- [Actualizador de ProcessDevKill](https://github.com/xfiberex/ProcessDevKill/blob/main/src-tauri/src/update.rs)
- [Updater oficial de Tauri 2](https://v2.tauri.app/plugin/updater/)
- [CLI oficial para generar gratuitamente las claves del updater](https://v2.tauri.app/reference/cli/#signer-generate)
- [Firma de código Windows, diferente de la firma del updater](https://v2.tauri.app/distribute/sign/windows/)
- Infraestructura local de `../sistema anterior/`, especialmente
  `.github/workflows/ci.yml`, `tools/build_windows.ps1`, `docs/operations/` y
  `docs/reference/THREAT_MODEL.md`.
