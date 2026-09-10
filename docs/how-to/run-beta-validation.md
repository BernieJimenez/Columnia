# Cómo ejecutar una sesión beta de Columnia

Este recorrido comprueba si una persona puede convertir un archivo problemático
en una entrega confiable sin ayuda del equipo. No sustituye los gates automáticos:
busca fricción, resultados inesperados y problemas que solo aparecen con datos de
trabajo reales.

## Resultado esperado

Cada sesión debe terminar con una entrega local comprobada, un proyecto que se
puede cerrar y reabrir, y un reporte sin filas, rutas, consultas, credenciales ni
información personal. Usa la [plantilla de sesión](../templates/beta-session.md)
y guarda la copia completada bajo `.local/beta/`, que Git ignora. Las tres
sesiones aceptadas de un gate deben probar exactamente el mismo release candidate
y commit.

## Preparación

1. Usa Windows x64 con WebView2 y ejecuta desde la raíz del repositorio:

   ```powershell
   npm install
   .\tools\check.ps1 -Profile Fast
   npm run tauri dev
   ```

2. Copia la plantilla sin agregarla al repositorio:

   ```powershell
   $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
   $session = ".local\beta\$stamp"
   New-Item -ItemType Directory -Force -Path $session | Out-Null
   Copy-Item docs\templates\beta-session.md "$session\session.md"
   ```

3. Asigna un alias distinto a cada participante y confirma que no participó en
   otra de las tres sesiones aceptadas. No registres su identidad.

4. Elige tres datasets. Ningún dataset real se copia a `fixtures/`, `.local/` ni
   al repositorio.

   - **Baseline sintética:** `fixtures\automation\quality-input.csv`.
   - **Tabla real problemática:** CSV, TSV, JSON o Parquet con un problema que la
     persona necesite resolver de verdad.
   - **Caso estructural:** un libro con varias hojas o un dataset suficientemente
     grande para observar paginación, progreso y cancelación.

La fixture sintética es un calentamiento reproducible: no cuenta como dataset
real ni crea una sesión adicional. Cada sesión usa dos casos reales; entre las
tres sesiones deben aparecer al menos tres datasets reales distintos. Reutilizar
un caso entre participantes está permitido cuando ayuda a comparar recorridos.

Antes de comenzar, la persona facilitadora explica únicamente el objetivo y cómo
detener la prueba. No demuestra el flujo ni sugiere dónde hacer clic.

## Tareas de la persona participante

Cronometra cada tarea y registra si se completó sin ayuda, con ayuda o no se pudo
completar. Registra también acciones de navegación, datos reintroducidos,
retrocesos, eventos de ayuda y la causa sanitizada de cada duda, usando las
definiciones de la plantilla. No grabes la pantalla cuando pueda mostrar datos
sensibles.

### 1. Cargar y comprender

- Abrir el dataset correcto.
- Explicar qué entiende sobre filas, columnas y problemas detectados.
- Encontrar una columna o señal que merezca revisión.

### 2. Revisar

- Ejecutar el análisis de calidad.
- Inspeccionar al menos una señal de nulos, duplicados, tipos, distribución o
  privacidad que aplique al dataset.
- Determinar si el resultado representa el archivo completo o una muestra.

### 3. Preparar y recuperar

- Aplicar una transformación útil y comprobar su efecto.
- Deshacerla y rehacerla.
- Guardar el proyecto, cerrar la aplicación, abrirla otra vez y continuar desde
  el proyecto guardado.

### 4. Validar y entregar

- Definir al menos una regla de calidad relevante.
- Observar una validación aprobada o bloqueada y explicar por qué ocurrió.
- Exportar una copia local en un formato útil.
- Abrir la carpeta del resultado y comprobar el archivo con una herramienta
  independiente. Confirmar que el original no cambió.

### 5. Recuperarse de un problema

Ejecuta uno de estos casos que sea seguro para el dataset: cancelar una operación,
intentar una regla que falla, elegir un destino existente o cerrar y reabrir el
proyecto. La persona debe entender el estado final y una acción posible para
continuar.

## Preguntas finales

Hazlas después de completar las tareas, sin defender el diseño:

1. ¿Qué pensabas que ocurriría en cada paso que te sorprendió?
2. ¿En qué momento dudaste o necesitaste ayuda?
3. ¿Confiarías en la entrega producida? ¿Qué evidencia te faltó?
4. ¿Qué reemplaza Columnia hoy en tu trabajo?
5. Si pudieras corregir una sola cosa antes de volver a usarla, ¿cuál sería?

## Privacidad del reporte

Registra solo formato, rango aproximado de tamaño y filas, sistema operativo,
resultado de las tareas y observaciones redactadas. No registres:

- nombres o rutas de archivos reales;
- nombres o valores de columnas sensibles;
- filas, capturas, consultas SQL o reglas que revelen datos;
- nombres, correos o identidad de participantes;
- cadenas de conexión o credenciales.

Un fallo que dependa de los datos debe convertirse primero en una fixture mínima,
sintética y determinista. Solo esa reproducción sanitizada puede entrar en
`fixtures/`, siguiendo la [política de fixtures](../reference/fixtures-policy.md).

## Severidad de hallazgos

- **P0 — Bloqueante:** pérdida/corrupción de datos, exposición de información,
  ejecución insegura o resultado silenciosamente incorrecto. Detén la beta.
- **P1 — Alta:** impide completar una tarea principal y no tiene recuperación
  razonable.
- **P2 — Media:** la tarea se completa con ayuda, reintentos o una ruta confusa.
- **P3 — Baja:** texto, consistencia o fricción menor que no cambia el resultado.

Cada hallazgo debe incluir tarea, expectativa, resultado, severidad y una
reproducción sanitizada. Agrupa observaciones repetidas por causa, no por persona.

## Cuándo una sesión cuenta

Una sesión es válida cuando completa las diez tareas sobre el release candidate
declarado y conserva un veredicto íntegro. Aplica estas reglas:

- un fallo de instalación, preparación o entorno ocurrido antes de observar el
  recorrido no cuenta como sesión;
- un P0/P1 detiene el gate e invalida el conjunto aceptado para ese release
  candidate;
- después de corregir un P0/P1, crea un release candidate nuevo, repite el camino
  afectado y reúne tres sesiones válidas sobre ese mismo commit;
- una sesión con P2/P3 puede contar si termina el recorrido y cada hallazgo tiene
  decisión y responsable;
- no combines porcentajes ni fricción de commits distintos.

## Criterios para cerrar la beta V1

La beta queda aceptada cuando se cumplen todos estos puntos:

- tres sesiones válidas, tres participantes distintos y el mismo release
  candidate;
- dos casos reales por sesión y al menos tres datasets reales distintos en total;
- todas las personas completan Cargar → Revisar → Preparar → Entregar;
- al menos 24 de las 30 tareas agregadas se completan sin ayuda;
- no quedan hallazgos P0 o P1 abiertos;
- guardado/reapertura y verificación independiente de la entrega pasan en todas
  las sesiones;
- cualquier fallo basado en datos tiene una reproducción sintética o una razón
  documentada de por qué no puede conservarse;
- los P2/P3 aceptados tienen decisión y responsable, aunque se difieran.

Al terminar, copia la [plantilla de resumen](../templates/beta-summary.md) a
`docs/reference/beta-v1-summary.md`, completa únicamente métricas agregadas y
sométela a revisión de privacidad. Los reportes de sesión permanecen en
`.local/beta/`; solo el resumen sanitizado se versiona. Tier 8 no se marca cerrado
en `ROADMAP.md` hasta que ese archivo exista y todos los criterios anteriores
estén aprobados.

## Gate 2: validar el shell de espacios

Después de cerrar Tier 8 y añadir el shell, ejecuta una segunda ronda enfocada
sobre otro release candidate estable. Usa la misma definición de tareas y
métricas para comparar contra Gate 1. La consolidación de Analizar se acepta
cuando:

- no reaparecen P0/P1 y siguen pasando guardado, reapertura y entrega;
- las tareas principales conservan su capacidad;
- las acciones de navegación o los datos reintroducidos bajan al menos 20 %;
- Automatizar y Preparar para BI se entienden como interfaces futuras, sin
  confundirse con errores, permisos o funciones activas.

Si el baseline de una métrica es cero, regístrala como `N/A` y usa la otra
métrica para demostrar la mejora; nunca inventes un porcentaje a partir de cero.

La beta no autoriza publicar instaladores ni activar el updater. Esas decisiones
siguen el [flujo de publicación](publish-release.md) y la
[revisión legal](../reference/legal-distribution-review.md).
