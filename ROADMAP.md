# Roadmap — Columnia

> Documento vivo de planificación del nuevo proyecto.
> La versión actual usada como referencia está en `../dataprepv1.1/`.

## Estado general

- Etapa actual: Fase I0 y prototipo vertical de la Fase I1.
- Implementación: iniciada el 2026-08-12.
- Nombre: `Columnia`, aprobado.
- Carpeta del proyecto nuevo: `Columnia/`, creada.
- Política de validación: todo se ejecutará localmente; no habrá CI, GitHub
  Actions ni workflows automáticos.
- Política de costos: no se adoptarán certificados, servicios ni herramientas
  de pago obligatorias.
- Plataformas objetivo de diseño: Windows, macOS y Linux.
- Plataforma inicial de verificación: Windows; macOS y Linux deberán verificarse
  localmente en sus respectivos sistemas antes de declarar soporte público.

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

- [ ] Elegir licencia y modelo de distribución de Columnia.
- [ ] Definir Windows x64 como objetivo inicial o aprobar una matriz distinta.
- [ ] Definir versionado SemVer, formato de tags y fuente de verdad de versión.
- [ ] Crear ADR para Tauri/Rust, motores de datos y frontera UI/backend.
- [ ] Definir política local de ramas, revisión y commits.
- [ ] Crear una lista local de dependencias desactualizadas y auditorías; no
  depender de bots para mantenerlas.
- [ ] Definir política para fixtures: solo datos sintéticos, nunca datos reales o PII.

**Salida:** repositorio inicial gobernado y decisiones técnicas trazables.

### Fase I1 — Prototipo vertical de Tauri y datos

- [x] Crear el scaffold Tauri 2 + React + TypeScript + Vite.
- [x] Crear y probar en frontend el primer comando Tauri tipado (`get_app_info`).
- [x] Compilar el shell Tauri localmente en Windows.
- [x] Abrir el shell Tauri en Windows y completar un smoke de arranque.
- [ ] Completar la revisión visual sistemática del shell en Windows.
- [x] Crear el primer comando Tauri tipado de selección y carga CSV.
- [ ] Añadir un canal de eventos de progreso para operaciones largas.
- [ ] Leer y perfilar CSV, Excel y Parquet con datasets representativos. La carga
  y el perfil inicial de CSV ya funcionan; faltan perfiles avanzados, Excel y
  Parquet.
- [x] Implementar una vista previa paginada sin enviar el dataset completo a
  React: páginas de 50 filas obtenidas desde la sesión Rust.
- [ ] Ejecutar una receta lazy con limpieza, tipos, filtro y columna calculada.
- [x] Implementar la primera transformación reversible: eliminar duplicados
  exactos preservando la primera aparición, sin modificar el archivo original.
- [ ] Sustituir el historial provisional de un nivel por una receta reproducible
  con múltiples operaciones y deshacer/rehacer.
- [ ] Implementar cancelación cooperativa y exportación atómica CSV/Parquet.
- [ ] Comparar tiempo y RAM con `dataprepv1.1`.

**Gate:** ninguna arquitectura se declara definitiva hasta superar el benchmark
y validar los casos difíciles de Excel.

**Avance 2026-08-12:** `npm run build`, dieciocho pruebas Vitest y diez pruebas Rust
pasan. El comando Rust `pick_and_load_csv` abre el selector nativo sin aceptar
rutas desde React, valida un límite provisional de 100 MB, carga el CSV con
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
La primera transformación elimina duplicados exactos de la sesión, conserva el
orden y la primera aparición, actualiza la vista previa e invalida el perfil. El
archivo original permanece intacto y existe un único punto de deshacer; este
historial es provisional hasta introducir recetas reproducibles.
`tauri build --debug --no-bundle` genera correctamente
`src-tauri/target/debug/columnia.exe`, que permanece estable durante el smoke de
arranque. La primera compilación reveló que el
scaffold no contenía los iconos requeridos por Tauri; se añadió un SVG maestro,
se generaron los recursos multiplataforma y una prueba impide su regresión.

### Fase I2 — Frontera de seguridad del escritorio

- [ ] Definir CSP estricta: sin CDN, `object-src 'none'`, sin navegación remota.
- [ ] Crear capabilities separadas por ventana y conceder solo permisos usados.
- [ ] Mantener red en Rust; el frontend no realizará peticiones externas directas.
- [ ] Validar y canonicalizar toda ruta antes de leer, escribir, abrir o ejecutar.
- [ ] Usar selectores nativos; el frontend recibe handles/identificadores, no
  autoridad global sobre el filesystem.
- [ ] Implementar instancia única y política de recuperación de sesión.
- [ ] Actualizar threat model para datasets, SQL, fórmulas de Excel y updater.
- [ ] Añadir pruebas negativas de traversal, symlinks, fórmulas y payloads grandes.

**Gate:** revisión de la superficie de comandos y de cada permiso Tauri.

### Fase I3 — Calidad local reproducible

- [ ] Crear `tools/check.ps1` como entrada única para `cargo fmt --check`, Clippy
  con warnings como errores, `cargo test`, TypeScript, Vitest y build Tauri.
- [ ] Añadir perfiles rápido, completo y release; el perfil release siempre
  ejecutará todos los gates.
- [ ] Ejecutar las pruebas puras de forma local sin requerir abrir la ventana.
- [ ] Mocks oficiales/controlados de Tauri para pruebas del frontend.
- [ ] Generar tipos TypeScript desde contratos Rust o verificar su deriva dentro
  de `tools/check.ps1`.
- [ ] Mantener E2E, accesibilidad WCAG 2.2 AA y pruebas visuales.
- [ ] Definir umbrales de cobertura por capa, no solo un porcentaje global.
- [ ] Presupuestos medibles: RAM, datasets grandes, startup, bundle e instalador.
- [ ] Guardar reportes locales con fecha, commit, versiones de herramientas y
  resultados para que una validación pueda auditarse después.

**Gate:** no se puede declarar una fase terminada ni preparar un release si el
script local completo rompe contratos, seguridad, accesibilidad o presupuestos.

### Fase I4 — Supply chain y privacidad

- [ ] Ejecutar `cargo audit` y evaluar `cargo deny` para advisories, licencias,
  duplicados y fuentes no aprobadas.
- [ ] Auditar npm y usar lockfiles reproducibles (`npm ci`, Cargo.lock).
- [ ] Escanear secretos y bloquear artefactos/datasets sensibles en Git.
- [ ] Generar SBOM CycloneDX que incluya Rust y npm.
- [ ] Mantener `THIRD_PARTY_NOTICES` generado y verificable.
- [ ] Documentar cada acceso de red y comprobar que no exista telemetría oculta.
- [ ] Telemetría y reporte remoto de fallos: desactivados por defecto; cualquier
  cambio requerirá consentimiento explícito, redacción de PII y un ADR.

**Gate:** SBOM, licencias y auditorías sin hallazgos bloqueantes.

### Fase I5 — Empaquetado e instalación Windows

- [ ] Generar iconos Tauri desde un único SVG maestro de Columnia.
- [ ] Configurar NSIS `currentUser` como instalador recomendado.
- [ ] Evaluar MSI solo si hay necesidad real de despliegue empresarial.
- [ ] Incluir licencia, EULA si aplica y avisos de terceros como resources.
- [ ] Validar instalación, primera apertura, segunda instancia, actualización,
  desinstalación y conservación/borrado opcional de datos.
- [ ] Probar usuario sin privilegios administrativos y rutas con Unicode/espacios.
- [ ] Definir política WebView2 bootstrapper/offline.
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

- [ ] README orientado al problema, privacidad, descarga y limitaciones honestas.
- [ ] Documentación separada en tutorial, how-to, referencia y explicación.
- [ ] ADR para decisiones duraderas; CHANGELOG para cambios publicados.
- [ ] Script de capturas con dataset sintético estable y ventanas definidas.
- [ ] Regenerar capturas desde el binario de release y detectar diferencias.
- [ ] No acumular imágenes manuales sin dueño, fecha o propósito.
- [ ] Checklist de enlaces, encoding UTF-8 y coherencia de versiones.

**Gate:** una persona nueva puede instalar, verificar, usar y diagnosticar
Columnia siguiendo solamente la documentación publicada.

## 7. Objetivos cuantitativos provisionales

Se confirmarán con el prototipo; hasta entonces funcionan como hipótesis a medir.

| Métrica | Objetivo inicial |
| --- | --- |
| Arranque en caliente hasta UI utilizable | ≤ 2 segundos en equipo de referencia |
| Primera vista previa | ≤ 3 segundos para CSV de 100 MB |
| Memoria durante preview | Prototipo: límite de 100 MB; objetivo final: no materializar el dataset completo |
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
- [ ] Definir licencia y modelo de distribución.
- [ ] Definir el usuario principal y el problema número uno de la primera versión.
- [ ] Clasificar funciones actuales en conservar, rediseñar o eliminar.
- [ ] Definir formatos y bases de datos obligatorios para la primera versión.
- [x] Establecer testing, seguridad y releases como procesos exclusivamente locales.
- [x] Descartar GitHub Actions, CI y archivos de workflow.
- [x] Establecer que Columnia no dependerá de certificados o servicios de pago.
- [ ] Aprobar el updater gratuito firmado de Tauri o decidir no incluir
  actualizaciones dentro de la app.
- [x] Ejecutar el primer corte vertical CSV del prototipo técnico de la Fase I1.
- [ ] Completar el prototipo técnico con paginación/streaming, progreso,
  cancelación, Excel, Parquet y benchmark.

## 9. Registro de decisiones

| Fecha | Decisión | Estado |
| --- | --- | --- |
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
