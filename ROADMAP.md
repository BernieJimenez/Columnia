# Roadmap — Columnia

> Documento vivo de planificación del nuevo proyecto.
> La versión actual usada como referencia está en `../dataprepv1.1/`.

## Estado general

- Etapa actual: Fase I1 completa como prototipo vertical verificable, con las
  Fases I0, I4 e I8 cerradas, I3/I5 avanzadas y la Fase P1 de paridad funcional
  con JSON, SQL, comparación/consolidación por clave, joins multidataset y
  visualizaciones accesibles, resolución por columna/valor, privacidad visible
  en los destinos locales, correlaciones numéricas acotadas y contratos de calidad v3 con documento canónico
  Columnia v1; la Fase M1 importa reglas de DataPrep v1–v3 y legados, conserva
  opciones de entrega, reconoce metadatos de sesiones con fixtures sintéticas y
  ofrece cobertura temporal con calendario diario accesible, retiro confirmado de
  identificadores y migración de sesiones por CLI; mantiene pendientes la
  restauración completa de sesiones y la cobertura integral del round-trip hacia
  proyectos;
  I3/I5 conservan validaciones externas de plataforma.
- Versión actual del prototipo: `0.57.0`.
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
- `DataPrep`: es descriptivo, pero demasiado genérico y se confunde con
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
datos son mucho menores, y DataPrep ya posee controles de ingeniería que allí no
existen.

### Comparación de infraestructura

| Área | `dataprepv1.1` | ProcessDevKill | Decisión para Columnia |
| --- | --- | --- | --- |
| Shell de escritorio | pywebview + WebView2 | Tauri 2 + WebView2 | Adoptar Tauri 2 |
| Backend | Python empaquetado con PyInstaller | Rust compilado nativamente | Adoptar Rust |
| Límite UI/backend | Fachada allowlisted y políticas de rutas | Comandos Tauri + capabilities declarativas | Combinar contratos tipados, comandos estrechos y capabilities mínimas |
| Seguridad del WebView | Assets offline y bridge restringido | CSP explícita y permisos por ventana | Añadir CSP estricta y permissions-as-code desde el primer prototipo |
| Instancia de la aplicación | No se identificó bloqueo de segunda instancia | Plugin `single-instance` | Implementar instancia única y reactivar la ventana existente |
| Empaquetado Windows | EXE one-file; MSIX local experimental | Instaladores NSIS y MSI | Publicar NSIS por usuario; evaluar MSI para empresas |
| Instalación sin administrador | EXE portable | NSIS `currentUser` | Ofrecer instalación por usuario sin UAC |
| Actualizaciones desde la app | Hay ejemplos de manifiesto, pero no flujo runtime completo | Consulta releases, muestra notas, descarga con progreso y verifica SHA-256 | Implementar UX equivalente con el updater firmado de Tauri |
| Autenticidad de actualización | SHA-256 externo, sin firma | Instalador y SHA-256 en el mismo release; reconoce que no autentica al editor | No copiar este límite: exigir firma del updater; mantener SHA-256 como evidencia adicional |
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

### Lo que ProcessDevKill aporta y DataPrep no tiene completo

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
- [x] Comparar tiempo y RAM con `dataprepv1.1` mediante un benchmark local
  reproducible de 100 MiB, con cleanup y evidencia sanitizada.

**Gate:** el benchmark cruzado de I1 está aprobado; la arquitectura de entrada
de libros sigue condicionada a validar los casos difíciles de Excel antes de
declararla definitiva.

**Cierre 2026-08-23:** I1 queda implementada de extremo a extremo. Las recetas
compatibles ejecutan un plan Polars lazy con renombres, tipos, filtros y
columnas calculadas; las operaciones no compatibles conservan el camino eager
para mantener el contrato estricto. El monitor nativo de CPU/RAM se muestra en
el lateral de la aplicación, y `perf:i1` compara la inspección de 100 MiB con
`dataprepv1.1` sin conservar datos de la fixture.

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
`dataprepv1.1`: Cargar, Revisar, Preparar y Entregar. Esto evita una única
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
La versión 0.6.0 corrige la arquitectura de navegación según `dataprepv1.1` y
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
regex libre en este hito. Contactos se procesan antes de agrupar, mientras las
extracciones y el resumen agrupado son incompatibles dentro de una misma receta.
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
  perfil contractual de 100 MiB, el gate CDP y el bundle ya pasan. El escenario
  CLI de 256 MiB también completa transformaciones y ciclo durable, pero alcanza
  aproximadamente 1.12 GiB de working set y aún falta medir el mismo caso dentro
  de WebView2.
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
- [ ] Probar usuario sin privilegios administrativos y rutas con Unicode/espacios.
- [x] Definir política WebView2 bootstrapper/offline: el instalador descarga el
  bootstrapper oficial cuando hace falta; un instalador totalmente offline sigue
  pendiente de validación como variante separada.
- [ ] Medir tamaño, tiempo de instalación y tiempo hasta ventana utilizable.

**Gate:** smoke desde una VM Windows limpia, no solo desde la máquina de desarrollo.

### Fase I6 — Actualizaciones autenticadas

- [ ] Usar `tauri-plugin-updater`, cuya firma criptográfica se genera localmente
  con Tauri CLI y no requiere comprar un certificado.
- [ ] Generar y custodiar fuera del repositorio la clave privada del updater.
- [ ] Incrustar únicamente la clave pública en la aplicación.
- [ ] Publicar manifiesto, firma y checksum; rechazar un release incompleto.
- [ ] Mostrar versión, tamaño y notas antes de descargar.
- [ ] Descargar solo tras acción del usuario, mostrando progreso y cancelación.
- [ ] Verificar firma antes de instalar y limitar cualquier ruta ejecutable al
  directorio privado del updater.
- [ ] Probar downgrade, versión igual, prerelease, descarga parcial, firma inválida,
  manifiesto corrupto, falta de red y recuperación después de un cierre.
- [ ] Definir rotación y recuperación de claves antes del primer release público.
- [ ] Documentar expresamente que esta firma autentica actualizaciones de
  Columnia, pero no elimina el aviso de SmartScreen ni identifica al editor ante
  Windows.

**Gate:** una actualización manipulada debe fallar de forma cerrada y dejar la
versión instalada utilizable.

### Fase I7 — Release local reproducible

- [ ] Crear `tools/release.ps1` con `-DryRun` y mensajes de recuperación claros.
- [ ] Exigir árbol completamente limpio, incluyendo archivos no rastreados.
- [ ] Ejecutar todos los gates; la publicación oficial no admite `SkipTests`.
- [ ] Sincronizar/verificar versión en Tauri, Cargo, npm y metadatos Windows.
- [ ] Construir NSIS y los artefactos del updater desde el commit etiquetado.
- [ ] Firmar gratuitamente los artefactos del updater con la clave privada local.
- [ ] No firmar con Authenticode; aceptar y documentar `Editor desconocido` y el
  posible aviso de SmartScreen.
- [ ] Generar SHA-256, SBOM, manifiesto de procedencia local y notas de release.
- [ ] Instalar y ejecutar el artefacto final antes de publicarlo.
- [ ] Crear el tag únicamente después de superar toda la validación local.
- [ ] Publicar manualmente en el canal gratuito elegido.
- [ ] Descargar otra vez los assets publicados y verificar localmente firma/hash.
- [ ] Mantener una vía de reanudación segura si falla después de crear el tag o
  durante la publicación manual.

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
- [x] Comparar infraestructura de DataPrep y ProcessDevKill.
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
- [x] Completar streaming/lazy y benchmark contra `dataprepv1.1`; el trabajo
  posterior es ampliar la cobertura a datasets mayores, historial integral y
  casos difíciles de Excel.

## 8.1. Cola de ejecución recomendada desde v0.57.0

1. **Migración de recetas DataPrep:** completada en v0.53.0 para el núcleo
   representable. El selector importa pipelines v1–v3, normaliza operaciones
   compatibles a receta Columnia v1 y rechaza semánticas ambiguas como regex o
   booleanos personalizados. La migración de sesiones, reglas incrustadas,
   artefactos y round-trip completo permanece en M1.
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
   cleanup 100 % repetible y comparar contra `dataprepv1.1`; `npm run smoke:cdp`
   ya aplica 512 MiB de working set/256 MiB de memoria privada al árbol nativo y
   registra duraciones IPC por operación, mientras `npm run perf:benchmark` cubre
     una muestra CLI de 100 MiB. v0.47 añadió tres iteraciones sostenidas, v0.48
     añadió límites de duración y stress durable, y v0.49 mide tres ciclos nativos
     de transformación/exportación dentro de WebView2 junto al presupuesto global
     del árbol. Todavía falta repetirlo con datasets grandes y con historial que
     ejerza el límite integral de memoria. El perfilado de columnas ya distribuye
     el trabajo entre hasta cuatro trabajadores con progreso ponderado y evita
     materializar vectores numéricos gigantes; queda ampliar la lectura
     lazy/incremental y medirla dentro de WebView2 con datasets grandes.
6. **Ejecución lazy/incremental y arranque:** la receta compatible de I1 usa
   planes Polars lazy con fallback eager; en v0.54.0 las etapas Review,
   Preparar y Entregar también se cargan bajo demanda, la migración SQLite se
   difiere hasta la primera operación y el monitor de recursos espera al primer
   paint. CSV, TSV y TXT delimitado ya se leen con `LazyCsvReader`, y Parquet
   con `scan_parquet`; ambos usan el motor streaming de Polars, baja memoria y
   sin `rechunk` paralelo. Ampliar después a datasets mayores y a operaciones
   que todavía requieren el camino eager.
7. **DuckDB y operaciones multidataset:** incorporar DuckDB solo después del
   benchmark; los joins, la comparación y la consolidación local ya tienen una
   primera entrega; quedan destinos de base de datos y consultas más amplias.
8. **Migración M1 desde `dataprepv1.1`:** la vertical de contratos de calidad
   ya produce en v0.55.0 un informe con conteos, advertencias, acciones manuales
   y hash del artefacto; v0.56.0 conserva las opciones de entrega compatibles de
   pipelines y reporta sus omisiones; v0.57.0 añade fixtures sintéticas para
   pipelines, sesiones, calidad y legacy, y reporta metadatos de sesión que no
   se pueden aplicar automáticamente; quedan restauración de sesiones,
   mapeo a proyectos y round-trip hacia proyectos Columnia.
9. **Cierre de distribución:** auditorías de vulnerabilidades, secretos,
   licencias/avisos, smoke de instalador limpio, updater autenticado y validación
   real en macOS/Linux.

### Fase P1 — Paridad funcional con `dataprepv1.1`

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
- [ ] Completar la entrega compatible con DataPrep: PostgreSQL/MySQL/SQL Server,
  prueba de conexión, políticas de tabla y apertura segura de la carpeta. La
  primera slice ya publica un bundle ZIP atómico con
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
- [ ] Completar la migración del catálogo de limpieza sugerida: las reglas
  avanzadas que aún no tengan una acción reversible. Los identificadores y el
  PII personal (correo, teléfono, dirección y nombre) ya tienen acciones
  explícitas y confirmadas. La
  eliminación difusa ya tiene una primera acción implementada: confirma el
  impacto agregado, conserva la primera fila/orden y las copias exactas, y
  permite deshacer; siguen pendientes las reglas restantes.
  También están cubiertos como señal vacíos, constantes,
  alta nulidad, centinelas, imputación conservadora, booleanos y auditoría
  `_cambios`.
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
  migración segura desde DataPrep, bridge tipado, editor accesible y pruebas.
- [x] Añadir `date_range` a los contratos de calidad: límites `minDate`/`maxDate`,
  parseo seguro de texto/Date/Datetime, valores nulos o ilegibles inválidos,
  migración DataPrep, bridge tipado, editor accesible y evaluación compartida.
- [x] Añadir `conditional` a los contratos de calidad con condiciones `eq`, `ne`,
  `lt`, `lte`, `gt` y `gte`, subreglas fila-a-fila seguras, tolerancia exterior,
  migración DataPrep, bridge tipado, editor accesible y evaluación compartida.
- [x] Añadir `schema_contract` con columnas requeridas, control de columnas
  adicionales y orden opcional, migración DataPrep, bridge tipado, editor
  accesible y evaluación estructural compartida.
- [x] Añadir `referential_integrity` con referencias locales explícitas para
  claves simples y compuestas, migración segura desde DataPrep, bridge tipado,
  editor accesible, tolerancias y evaluación compartida sin exponer valores.
- [x] Añadir `monotonic` con direcciones no decreciente/no creciente,
  tolerancias por inversión, migración segura desde DataPrep, bridge tipado,
  editor accesible y evaluación compartida; los nulos reinician la cadena.
- [x] Añadir `aggregate_check` y `aggregate_reconciliation` para conteo, suma,
  mínimo, máximo y reconciliación de dos columnas, con `expected`/referencias,
  tolerancias absolutas/relativas, migración DataPrep, bridge tipado, editor
  accesible y resultados privados basados en conteos.
- [x] Añadir `distribution_drift` con línea base numérica, comparación de medias,
  umbral/tolerancia absoluta, exclusión de nulos y textos no numéricos, migración
  DataPrep, bridge tipado, editor accesible y resultados privados basados en
  conteos.
- [x] Añadir versionado/compatibilidad explícita del documento de reglas de
  calidad: formato canónico Columnia v1, guardado atómico, selector nativo,
  compatibilidad con DataPrep v1–v3 y legado v1/sin versión, rechazo seguro de
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
- [x] Endurecer la consulta SQL local con cancelación cooperativa, límite de
  filas coincidentes para agregaciones y preflight de cardinalidad para evitar
  materializar joins many-to-many fuera de presupuesto.
- [ ] Completar consulta con joins y DuckDB después de validar el benchmark y
  ampliar los límites de forma explícita.
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
- [ ] Extender esa frontera a futuros conectores remotos con contratos de
  privacidad equivalentes.
- [ ] Ampliar lazy/incremental a operaciones y datasets que exceden la memoria:
  Parquet cacheado, chunks, comparación/joins grandes, historial degradado y
  presupuestos explícitos sin materialización silenciosa.
- [x] Exponer un presupuesto opt-in de concurrencia Rayon desde Preferencias y
  recursos: perfiles conservador/equilibrado/máximo, límite de 64 hilos,
  persistencia local y estado explícito cuando el pool ya no puede cambiarse.
- [ ] Completar la paridad de sesión operativa: muestras, arrastrar/soltar,
  preferencias, caché derivada, historial de ejecuciones y apertura segura de
  outputs; la primera slice de archivos recientes ya conserva solo nombre,
  formato, fecha e ID opaco, sin rutas, y Revisar ya muestra una actividad SQL
  acotada a la sesión con estado, duración y filas, sin guardar la consulta.
  El modelo durable de proyectos de Columnia se conserva como reemplazo de la
  sesión persistente original.

**Gate:** cada capacidad marcada como implementada debe tener contrato, prueba
automatizada y una fila de paridad con evidencia del original.

### Fase M1 — Migración de compatibilidad desde `dataprepv1.1`

Esta fase no copia Python ni `pywebview`: convierte los artefactos que una
persona ya tiene en DataPrep para que puedan abrirse y continuar en Columnia.
La auditoría se hizo contra `src/dataprep/core/bridge_contract.py`,
`src/dataprep/core/recipes.py`, `src/dataprep/core/cleaner.py`,
`src/dataprep/capabilities/analysis.py`, `src/dataprep/api.py` y la UI React
del original.

- [x] Registrar la brecha funcional: Columnia ya cubre entradas, preview,
  perfil básico, recetas nativas, proyectos, JSON/SQL, comparación por clave,
  joins y visualizaciones accesibles básicas; no cubre todavía el catálogo
  completo de análisis/limpieza/calidad/entrega ni los artefactos legacy.
- [x] Definir un inventario de formatos de migración y fixtures sintéticas para
  pipelines, sesiones, reglas de calidad y recetas antiguas, sin incluir rutas
  reales ni celdas de usuario; el contrato vive en `docs/reference/migration-inventory.md`
  y `fixtures/manifest.json`.
- [x] Importar el núcleo representable de pipelines JSON DataPrep v1–v3:
  renombres, casts, fechas, filtros, reemplazos literales, columnas conservadas,
  cálculos, split/merge, outliers, grupos, contactos y extracciones; producir
  una receta Columnia v1. Regex, booleanos personalizados y operaciones sin
  equivalente se rechazan explícitamente.
- [x] Completar la importación de opciones de exportación representables de
  DataPrep: formatos CSV/JSON/Parquet/SQL/XLSX→Excel, columnas seleccionadas y
  privacidad; conservarlas en la receta y generar warnings estructurados para
  reportes, CSV/ZIP y parámetros SQL sin equivalente.
- [x] Reconocer manifiestos de sesión DataPrep durante la importación de recetas:
  registrar origen, snapshot, hoja, etapa, operaciones aplicadas, análisis y
  calidad como omisiones explícitas, sin copiar rutas ni afirmar una restauración
  de dataset que todavía no existe.
- [x] Conservar un resumen estructural sanitizado de la sesión importada en el
  informe de migración: referencias de origen/snapshot como booleanos, hoja y
  etapa visibles, y conteos de operaciones, reglas y comprobaciones de análisis;
  admite claves DataPrep `snake_case` y `camelCase`, acepta comprobaciones como
  objeto o arreglo, y no activa snapshots ni escribe proyectos automáticamente.
- [x] Implementar la primera vertical de migración de reglas de calidad: selector
  nativo JSON, conversión de reglas representables, tolerancias por conteo y
  porcentaje, warnings/omitidas para severidad o políticas no equivalentes,
  límites de archivo y sin exponer rutas al frontend.
- [x] Completar el informe estructurado de esta vertical: conteos de entradas,
  conversiones, omisiones y advertencias, acciones manuales, y SHA-256 del
  artefacto leído sin publicar rutas ni valores del dataset.
- [x] Versionar el artefacto de reglas con el formato canónico
  `columnia-quality-rules` v1, guardarlo atómicamente e importar de forma
  explícita Columnia v1, DataPrep v1–v3 y documentos legados compatibles; las
  versiones futuras y contratos ambiguos fallan antes de convertir reglas.
- [x] Completar la migración de semánticas de reglas antiguas y v3, conservando
  tolerancias, severidad,
  referencias, condiciones y reglas no soportadas como advertencias explícitas;
  nunca convertir una regla bloqueante en una entrega aprobada silenciosamente.
  Se aceptan aliases snake/camel, severidades históricas (`error`, `critical`,
  `fatal`), `blocking`, referencias escalares y condiciones heredadas; las
  políticas externas, `nullable` no equivalente y contradicciones se omiten con
  warning explícito.
- [x] Reforzar la migración de reglas representables con aliases de tipo, números
  serializados como texto y validación cerrada de políticas/tolerancias; los
  valores inválidos se omiten con warnings en vez de publicarse como reglas
  convertidas.
- [ ] Importar sesiones guardadas de DataPrep: dataset/origen, hoja, etapa,
  operaciones aplicadas, receta, reglas y análisis; cuando no sea seguro guardar
  un snapshot, conservar solo una referencia reproducible y explicarlo.
- [ ] Mapear sesiones/pipelines importados al catálogo de proyectos de Columnia,
  con validación de esquema, tipos, archivos ausentes, hojas inexistentes y
  colisiones de nombres antes de escribir cualquier snapshot.
- [x] Implementar la primera slice de mapeo de sesiones DataPrep al catálogo:
  selector nativo, validación de referencias, esquema, formato, hoja y receta
  en un estado temporal, publicación únicamente después de validar, fallback a
  `snapshot_path` compatible cuando falta la fuente y errores sin rutas
  administradas en el bridge.
- [x] Crear un informe de migración con operaciones convertidas, omitidas,
  advertencias, acciones manuales y hash de los artefactos; el preflight CLI
  session-migration-report lee manifiestos DataPrep v1–v3, resume origen,
  snapshot, hoja, etapa, receta, calidad y análisis, detecta referencias
  ausentes/colisiones y no publica secretos, rutas administradas ni valores de
  datasets. Devuelve código 2 cuando requiere revisión y nunca escribe
  proyectos.
- [x] Añadir un preflight CLI `quality-migration-report` para contratos Columnia,
  DataPrep v1–v3 y legacy: resume severidad/políticas, marca reglas omitidas,
  conserva el hash del artefacto y devuelve código 2 cuando requiere revisión,
  sin publicar rutas, columnas ni valores.
- [x] Añadir la primera compatibilidad de bridge necesaria para la migración:
  contrato versionado, selector nativo, lectura fuera del hilo de UI, errores
  sanitizados y compatibilidad con el informe de sesiones; no se expone la
  allowlist Python completa. La cancelación/progreso de importaciones largas
  sigue pendiente como ampliación separada.
- [x] Verificar una primera vertical de round-trip con una sesión sintética:
  importar → reabrir → validar → exportar, comparando conteos, columnas y tipos;
  los casos de importación parcial conservan la regresión que impide reemplazar un
  proyecto válido.
- [ ] Completar el round-trip con fixtures representativas de sesiones, hojas,
  historial y artefactos que todavía requieran restauración manual.

#### Límites de alcance de M1

- No se migran restos de Marimo, la implementación Python, `pywebview`,
  dependencias ECharts ni la estructura interna del bridge.
- El modelo multi-tabla/esquema estrella y los proyectos legacy se mantienen como
  capacidades opcionales: no se convierten en requisito de la primera migración
  sin una decisión de producto explícita.
- Los proyectos SQLite/Parquet, historial y reglas nativos de Columnia son la
  representación final; la compatibilidad se mide por comportamiento observable,
  no por igualdad de archivos internos.

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
| 2026-08-27 | Mostrar actividad SQL de la sesión actual con las últimas cinco ejecuciones, estado, duración y filas, sin persistir texto de consulta ni valores | Implementada en `src/features/review/ReviewPhase.tsx`; el historial durable de ejecuciones sigue en cola |
| 2026-08-26 | Retirar el límite provisional de 500 MiB para datasets; la capacidad efectiva depende de la RAM, el espacio en disco y los demás recursos disponibles | Implementada |
| 2026-08-26 | Paralelizar el perfilado por columna con una cola acotada de hasta cuatro trabajadores, mantener el orden de resultados y publicar progreso ponderado por sub-etapa para datasets grandes | Implementada en `src-tauri/src/dataset.rs`; la lectura lazy/incremental completa sigue en cola |
| 2026-08-26 | Usar `LazyCsvReader` con motor streaming, baja memoria y `rechunk` desactivado para CSV, TSV y TXT delimitado; se conserva un `DataFrame` activo para mantener la compatibilidad actual | Implementada; extender el mismo límite a Parquet cacheado, joins, comparación e historial sigue en cola |
| 2026-08-26 | Extender la lectura streaming a Parquet mediante `scan_parquet`, conservando `parallel: None`, baja memoria y `rechunk` desactivado para evitar picos innecesarios | Implementada en la carga inicial; cacheado, joins, comparación e historial incremental siguen en cola |
| 2026-08-26 | Derramar fingerprints XXH3 de duplicados normalizados en 256 cubetas temporales y ordenar una cubeta a la vez; se conserva el conteo, el orden de las filas y la cancelación sin guardar valores del dataset | Implementada en `src-tauri/src/dataset.rs`; la materialización del `DataFrame`, transformaciones eager y joins fuera de memoria siguen en cola |
| 2026-08-27 | Añadir tendencia temporal diaria para rangos de hasta 90 días, con días vacíos, límite de periodos, cancelación cooperativa y tabla accesible equivalente; rangos mayores mantienen la agregación mensual/anual | Implementada en `src-tauri/src/dataset.rs`, `src/bridge.ts` y `src/features/review/ReviewPhase.tsx` |
| 2026-08-23 | Cerrar Fase I0: MIT, Windows x64 inicial, frontera Rust/UI, validación local y fixtures sintéticas | Aprobada; `docs/adr/0001-contratos-del-repositorio.md` |
| 2026-08-12 | Usar `../dataprepv1.1/` como referencia funcional, no como plantilla técnica automática | Aprobada |
| 2026-08-12 | Nombre del producto y del proyecto: `Columnia` | Aprobada |
| 2026-08-12 | Backend: Rust + Tauri 2 + Polars + DuckDB | Aprobada |
| 2026-08-12 | Frontend: React + TypeScript + Vite | Aprobada |
| 2026-08-12 | Conservar los gates de seguridad, E2E, SBOM y rendimiento de DataPrep, ejecutados solo localmente | Aprobada |
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
| 2026-08-12 | Versión 0.6.0: flujo Cargar → Revisar → Preparar → Entregar alineado con `dataprepv1.1`; exportación aislada en Entregar | Implementada |
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
| 2026-08-23 | Fase I1 completa: receta Polars lazy con fallback eager seguro, monitor nativo compacto de CPU/RAM, benchmark cruzado de 100 MiB contra `dataprepv1.1` y revisión visual desktop/móvil/zoom/forced-colors | Implementada |
| 2026-08-23 | P1: contrato de calidad v3 ampliado con `allowed_values`, `regex`, `dtype`, unicidad compuesta y `row_count`; evaluación Rust/CLI/exportación, bridge/UI accesibles y tests end-to-end | Implementada |
| 2026-08-24 | P1: `column_compare` completa la comparación de dos columnas con seis operadores, nulos inválidos, tolerancias, migración DataPrep, editor accesible y evaluación compartida por UI/CLI/exportación | Implementada |
| 2026-08-24 | Versión 0.50.0: resolución de conflictos por columna/valor dentro del preview visible, compatibilidad legacy por fila y privacidad aplicada a los seis destinos locales | Implementada |
| 2026-08-24 | Versión 0.51.0: conflictos por clave paginados en bloques de 50, índices globales para decisiones y resolución completa fuera del preview con historial reversible; privacidad de artefactos sigue pendiente | Implementada |
| 2026-08-24 | P1: `date_range` añade límites inclusivos de fecha, soporte texto/Date/Datetime, rechazo de nulos/fechas ilegibles, migración DataPrep, editor accesible y evaluación compartida | Implementada |
| 2026-08-24 | P1: `conditional` añade condiciones eq/ne/lt/lte/gt/gte, subreglas then fila-a-fila seguras, tolerancia exterior, migración DataPrep, editor accesible y evaluación compartida | Implementada |
| 2026-08-24 | P1: `schema_contract` añade columnas requeridas, control de adicionales y orden opcional, migración DataPrep, editor accesible y evaluación estructural compartida | Implementada |
| 2026-08-24 | P1: documento de calidad `columnia-quality-rules` v1 con guardado atómico, compatibilidad explícita Columnia/DataPrep v1–v3/legado, rechazo de versiones futuras, CLI retrocompatible y UI sin exposición de rutas | Implementada |
| 2026-08-24 | I3: selector nativo Win32 estabilizado y elevado a `verify:tier`; el smoke WebView2 verifica abrir dataset, guardar/cargar receta y exportar con variantes de editor Abrir/Guardar como, cleanup y evidencia sin rutas. Quedan lector de pantalla, High Contrast manual, datasets grandes y VM limpia | Parcial, con evidencia |
| 2026-08-24 | Versión 0.52.0: sistema visual explícito con selector persistente `Sistema`/`Claro`/`Oscuro`, aplicación temprana en el documento, capa visual refinada y estilos compatibles con foco, movimiento reducido y `forced-colors` | Implementada |
| 2026-08-24 | Versión 0.53.0: el selector de recetas importa el núcleo representable de pipelines DataPrep v1–v3 a receta Columnia v1 y rechaza semánticas ambiguas antes de modificar el borrador | Implementada |
| 2026-08-24 | Versión 0.54.0: optimización de arranque con code-splitting de etapas pesadas, estado inicial local sin esperar `get_app_info`, migración SQLite diferida y monitor de recursos fuera del primer paint | Implementada |
| 2026-08-24 | Versión 0.55.0: la migración de contratos de calidad genera un informe auditable con conteos, omisiones, advertencias, acciones manuales y SHA-256 del artefacto sin exponer rutas ni valores | Implementada |
| 2026-08-24 | Versión 0.56.0: los pipelines DataPrep conservan opciones de entrega compatibles, normalizan XLSX a Excel y publican informe con warnings para semánticas de exportación omitidas, sin exponer rutas | Implementada |
| 2026-08-24 | Versión 0.57.0: inventario y fixtures sintéticas de migración para pipelines, sesiones, calidad y legacy; los metadatos de sesión se reconocen y se omiten con warnings sanitizados | Implementada |
| 2026-08-26 | P1 añade cobertura y tendencia temporal agregadas en Review: rangos Date/Datetime/Timestamp, conteos por mes/año, filas interpretables y porcentaje con tabla equivalente; el calendario diario y las series temporales completas siguen pendientes | Implementada |
| 2026-08-26 | M1 añade una slice segura para mapear sesiones DataPrep al catálogo de proyectos: selector nativo, validación de fuente/hoja/esquema/receta, restauración de `snapshot_path` compatible cuando falta la fuente y publicación transaccional sin tocar el dataset activo; una prueba nativa cubre importar → reabrir → validar → exportar, mientras la restauración histórica completa sigue pendiente | Implementada |
| 2026-08-27 | P1 incorpora un calendario diario accesible para rangos cortos, conserva días sin filas y mantiene la tabla exacta equivalente; las series temporales completas siguen pendientes | Implementada |
| 2026-08-27 | P1 incorpora retiro explícito y reversible de columnas identificadoras detectadas por encabezado, con confirmación, conservación de una columna y protección de valores personales | Implementada |
| 2026-08-27 | M1 incorpora `project-import-dataprep` en la CLI: migra sesiones con validación previa, fallback a snapshot compatible, salida JSON sanitizada y creación de proyectos nuevos sin reemplazos | Implementada |

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
- Infraestructura local de `../dataprepv1.1/`, especialmente
  `.github/workflows/ci.yml`, `tools/build_windows.ps1`, `docs/operations/` y
  `docs/reference/THREAT_MODEL.md`.
