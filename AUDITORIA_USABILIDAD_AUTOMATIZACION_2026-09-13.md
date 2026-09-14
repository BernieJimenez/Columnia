# Auditoría de usabilidad y automatización de Columnia

Fecha: 2026-09-13. Base revisada: commit `84abb16`.

## Diagnóstico

La interfaz está organizada alrededor de las operaciones disponibles del motor, y exige que la persona decida qué ejecutar, en qué orden y cuándo volver a analizar. Para un producto que pretende automatizar la preparación de datos, esa responsabilidad es excesiva.

El cambio de mayor valor es pasar de un catálogo de herramientas a un proceso dirigido por un resultado: **cargar un archivo, obtener un plan, resolver las excepciones necesarias y guardar una copia validada**. Reducir botones ayuda, pero necesita cambios en la coordinación del motor, los estados de ejecución y la persistencia.

Las mejoras anteriores añadieron capacidades útiles; algunas también aumentaron las decisiones visibles: perfiles, estimaciones, comparación de revisiones y acciones por señal. Ahora hace falta integrarlas en una experiencia común.

## Alcance y límites de la evidencia

Revisión estática de navegación, carga, diagnóstico, preparación, recetas, entrega, proyectos, historial y documentación de automatización. Se consultó CodeGraph antes de localizar código. Se revisaron estados y conexiones entre componentes, además de sus controles declarados.

No se realizó en esta revisión un recorrido nuevo del escritorio nativo ni una sesión con usuarios. Por tanto, no se atribuyen tiempos observados, tasas de éxito, conteos de botones simultáneamente visibles ni una evaluación visual nueva. La auditoría de diseño de 2026-09-06 contiene evidencia histórica, que no valida por sí sola esta versión.

Inventario reproducible mediante búsqueda de `<button` en los siguientes archivos:

| Archivo | Declaraciones de botón | Elementos `details` |
| --- | ---: | ---: |
| `src/App.tsx` | 4 | 2 |
| `src/features/load/LoadPhase.tsx` | 14 | 2 |
| `src/features/review/ReviewPhase.tsx` | 16 | 6 |
| `src/features/prepare/PreparePhase.tsx` | 41 | 3 |
| `src/features/prepare/TransformRecipeEditor.tsx` | 23 | 3 |
| `src/features/delivery/DeliveryPhase.tsx` | 9 | 0 |
| `src/features/projects/ProjectsPanel.tsx` | 7 | 0 |
| **Total de estos siete archivos** | **114** | **16** |

Son declaraciones en código, no botones visibles: incluyen ramas condicionales, diálogos y controles repetidos por fila. Tampoco incluyen todos los componentes hijos. Las 64 declaraciones de Preparar y su editor señalan concentración de complejidad; no significan 64 botones en una pantalla.

## Hallazgos confirmados

| ID | Evidencia actual | Consecuencia para la persona | Cambio recomendado |
| --- | --- | --- | --- |
| H01 | `App.loadSelection` activa Revisar con `profileStatus` en `idle`; `ReviewPhase` solicita «Analizar calidad». | Cargar no produce por sí mismo el diagnóstico que permite avanzar con criterio. | Iniciar análisis admitido por recursos al finalizar la carga; mostrar progreso y cancelación. |
| H02 | El pie global de `App` marca la fase como completada al pulsar «Continuar», condicionado a dataset disponible y ausencia de operación. | Avanzar puede parecer equivalente a haber completado una tarea. | Separar navegación de estados: pendiente, ejecutado, revisado, omitido, fallido. |
| H03 | El pie mantiene una acción primaria mientras Revisar y Preparar tienen acciones primarias propias. `DESIGN.md` pide una sola acción primaria por vista. | Compiten «hacer el trabajo» y «pasar de pantalla». | Una acción principal contextual por estado del flujo. |
| H04 | «Aplicar recomendadas» describe recortar espacios y normalizar encabezados; las demás correcciones tienen rutas separadas. | El nombre sugiere una preparación más completa que su alcance real. | Explicar el alcance ahora; sustituir después por un plan con operaciones seleccionables y efecto estimado. |
| H05 | `usePrepareController` invalida el perfil después de las correcciones; `App` lo vuelve a `idle`. | Se repite el ciclo de corregir, analizar y decidir qué hacer después. | Recalcular una vez al terminar un lote y actualizar el resumen por revisión. |
| H06 | Carga tiene estados distintos para hojas, reutilización de perfil, costo de recursos y diferencias de esquema. | Dependiendo de la ruta, se piden decisiones sucesivas sobre una sola importación. | Una revisión de importación que reúna las decisiones aplicables, conservando bloqueos reales. |
| H07 | Revisar mezcla diagnóstico, vista previa, comparación/unión, SQL y análisis detallados; varias herramientas ya están plegadas. | La persona debe conocer qué herramienta resuelve su objetivo. | Resumen con excepciones; exploración y comparación como herramientas explícitamente secundarias. |
| H08 | Las recomendaciones por señal ahora llevan a una corrección concreta, pero no forman un plan ejecutable común. | Se mejora la localización sin reducir la cantidad de decisiones operativas. | Cada señal aporta una propuesta al mismo plan; un solo punto de ejecución. |
| H09 | El editor de recetas organiza renombres, tipos, fechas, filtros, cálculos y más como formularios de operaciones. | Reutilizar una preparación exige conocer el vocabulario técnico. | Tareas guardadas con nombre de resultado; editor de operaciones bajo «Personalizar». |
| H10 | Entregar expone construcción de reglas, importación/guardado de contrato, validación y exportación. | La salida requiere aprender un subsistema de calidad. | Resumen del contrato y una acción «Validar y guardar copia»; edición de reglas aparte. |
| H11 | `ProjectsPanel` presenta recuperación, formulario de guardado y lista de proyectos. | Se mezcla preparar un archivo con administrar trabajo persistente. | Inicio centrado en abrir/continuar; guardado contextual dentro del proyecto. |
| H12 | La navegación informa «Automatizar — CLI disponible · Interfaz en preparación». | La promesa principal no tiene un recorrido gráfico equivalente. | Integrar primero «Repetir preparación» y luego lotes sobre el motor existente. |

## Experiencia propuesta

### Flujo habitual

```text
Seleccionar archivo
        ↓
Inspeccionar + analizar automáticamente, con límites de recursos
        ↓
Plan propuesto: cambios, motivos y asuntos que requieren decisión
        ↓
Aplicar plan → actualizar diagnóstico → validar
        ↓
Guardar copia → resultado + opción de repetir con otro archivo
```

La selección nativa del destino sigue formando parte del guardado. El análisis automático no implica modificar el dataset sin autorización. Una tarea previamente aprobada puede omitir una nueva aprobación de cada operación cuando coincidan su versión, esquema, alcance y política de salida; las diferencias relevantes vuelven a revisión.

### Estructura visible

- **Inicio:** seleccionar archivo o continuar un proyecto; ejemplos y administración en segundo nivel.
- **Preparación:** estado del proceso, resumen del archivo, plan y excepciones. La vista previa está a un clic.
- **Resultado:** cambios aplicados, validación, advertencias pendientes y destino de la copia.
- **Herramientas avanzadas:** SQL, uniones, editor de recetas, contratos detallados, comparación histórica y configuración del motor.

Las cuatro fases actuales pueden mantenerse internamente durante la transición. La persona no necesita visitar cuatro formularios para una preparación común.

### Ejemplo de contenido del plan

```text
ventas_septiembre.csv
Se encontraron espacios exteriores y fechas con más de una interpretación.

Cambios propuestos
[x] Recortar espacios exteriores en las columnas seleccionadas
[ ] Cambiar nombres de columnas — puede afectar informes existentes

Necesita tu decisión
    La fecha 03/04/2026 puede representar dos días distintos.
    Elegir interpretación o conservar el texto.

Acción principal: Aplicar plan
Acciones secundarias: Ver datos · Personalizar
```

Ejemplo conceptual, no resultado medido de un archivo real. Las casillas anteriores ilustran el plan; las casillas de implementación aparecen más abajo.

## Qué automatizar y qué debe seguir siendo una decisión

| Acción | Política propuesta | Condición |
| --- | --- | --- |
| Inspeccionar formato, tamaño y esquema | Automática | Sin sustituir el dataset activo hasta completar la carga correctamente. |
| Analizar calidad | Automática y cancelable | Admisión por recursos; declarar alcance exacto o muestreado de cada métrica. |
| Generar recomendaciones y priorizarlas | Automática | Cada recomendación debe explicar evidencia, alcance y consecuencia. |
| Recalcular calidad tras un lote | Automática | Una revisión estable; cancelar o descartar resultados obsoletos. |
| Recortar espacios | Propuesta seleccionable o política guardada | Incluso los espacios pueden ser significativos; restringir columnas y mostrar alcance. |
| Renombrar encabezados | Aprobación del plan o política explícita | Puede romper consultas, recetas e integraciones aunque sea reversible. |
| Fechas, números, booleanos y codificación | Automática solo con regla aprobada y válida | Conservar ceros iniciales, códigos y valores ambiguos. No inferir autorización de una puntuación de confianza. |
| Eliminar duplicados exactos | Decisión de negocio o tarea guardada | Filas iguales pueden representar eventos distintos. |
| Imputar nulos, eliminar columnas o tratar outliers | Revisión explícita | Cambian la interpretación y los resultados. |
| Fusionar registros parecidos o resolver conflictos | Revisión explícita | Mostrar criterio, alcance y alternativas; no decidir silenciosamente qué valor gana. |
| Ejecutar reglas de calidad ya aprobadas | Automática antes de exportar | El resultado debe corresponder a la misma revisión, reglas y opciones de salida. |
| Exportar, sobrescribir o escribir en una base remota | Acción explícita o política previa específica | Destino y alcance claros; no convertir una preferencia de formato en autorización para reemplazar datos. |
| Guardar un proyecto automáticamente | Opt-in por proyecto | Estado de guardado visible, cuota de disco, errores y recuperación comprobada. |

## Casillas de implementación priorizadas

Todas están pendientes: esta revisión produce el diseño y los criterios, no implementa el rediseño. P1 = desbloquea el flujo; P2 = mejora productividad; P3 = ampliación condicionada por uso. S/M/L son tamaños relativos, no estimaciones de calendario.

### Primera entrega: menos decisiones y estados honestos

- [ ] **UX01 · P1 · M — Acción principal contextual.** Reemplazar el avance genérico por la acción útil del estado: analizar, revisar plan, aplicar o guardar. **Cierre:** una acción primaria visible por estado normal y sin duplicado en el pie.
- [ ] **UX02 · P1 · M — Progreso basado en resultados.** Separar pantallas visitadas de operaciones completadas. **Cierre:** navegar no marca análisis/validación como realizados; omisiones y resultados obsoletos se distinguen.
- [x] **UX03 · P1 · M — Diagnóstico al cargar.** Coordinar carga y perfil automáticamente. **Cierre:** archivo pequeño válido llega al resumen sin pulsar «Analizar»; carga nueva y cancelación descartan resultados anteriores.
- [x] **UX04 · P1 · S — Alcance claro de «recomendadas».** Mientras no exista un plan, nombrar las dos acciones reales y sus consecuencias. **Cierre:** no se presenta la normalización de nombres como inocua para integraciones.
- [ ] **UX05 · P1 · M — Importación unificada.** Agrupar hoja, encabezados, perfil y recursos relevantes. **Cierre:** no repetir confirmaciones por información ya aceptada; las ambigüedades y bloqueos siguen identificados.
- [ ] **UX06 · P1 · S — Navegación enfocada.** Trasladar áreas futuras a información del producto y simplificar utilidades. **Cierre:** la navegación habitual contiene destinos accionables; funciones disponibles siguen localizables.
- [x] **UX07 · P1 · M — Inicio por intención.** Priorizar archivo nuevo y continuar trabajo; guardar y administrar proyectos en contexto. **Cierre:** no hay formulario de proyecto vacío compitiendo con abrir un archivo.

### Segunda entrega: automatización del trabajo común

- [ ] **AU01 · P1 · L — Plan de preparación ejecutable.** Modelo con revisión fuente, operaciones ordenadas, columnas, motivos, impacto y decisiones. **Cierre:** toda acción ejecutada coincide con el plan aprobado; un cambio de dataset invalida el plan.
- [ ] **AU02 · P1 · L — Ejecución coordinada del plan.** Reutilizar operaciones nativas; validar compatibilidad y recursos antes de publicar. **Cierre:** una unidad reversible cuando el motor lo permita; para operaciones no atómicas, estado parcial y recuperación explícitos. No prometer atomicidad por encadenar llamadas IPC.
- [ ] **AU03 · P1 · M — Calidad actualizada tras ejecución.** Recalcular al finalizar el lote, no tras cada clic. **Cierre:** antes/después identifican revisión y alcance; una tarea atrasada no sobrescribe resultados nuevos.
- [ ] **AU04 · P1 · M — Bandeja de excepciones.** Reunir fechas ambiguas, tipos, conflictos y cambios de esquema. **Cierre:** cada excepción permite resolver, conservar o excluir; no hay aprobaciones duplicadas para el mismo alcance.
- [ ] **AU05 · P1 · M — Cancelación y recuperación comunes.** Progreso por etapa y cancelación con estado real. **Cierre:** cancelar no publica una versión parcial como terminada; se explica si una operación nativa necesita terminar un bloque.
- [ ] **AU06 · P2 · M — Comparación simple de resultados.** Mostrar delta del último plan automáticamente cuando ya existe evidencia calculada. **Cierre:** no volver a cargar dos snapshots grandes solo para dibujar un resumen; comparación histórica completa sigue disponible bajo demanda.
- [ ] **AU07 · P2 · M — Política de recursos integrada.** Estimaciones discretas, avisos solo cuando cambian decisiones y bloqueo real desde el motor. **Cierre:** no confundir estimación con memoria/disco reservado; evitar análisis automáticos concurrentes del mismo dataset.

### Tercera entrega: salida y reutilización

- [ ] **OUT01 · P1 · M — Validar y guardar copia.** Coordinar contrato existente y exportación desde una acción. **Cierre:** contrato fallido detiene la escritura; exportación sin reglas sigue siendo una elección explícita y comprensible.
- [ ] **OUT02 · P1 · M — Reglas resumidas.** Mostrar «qué se exige» y los incumplimientos; mover el constructor a edición. **Cierre:** un usuario puede exportar con reglas guardadas sin manejar tipos técnicos de reglas.
- [ ] **OUT03 · P2 · M — Preferencias de entrega reutilizables.** Recordar formato y selección compatibles con la tarea. **Cierre:** cambios de esquema invalidan selecciones incompatibles; no persistir credenciales ni asumir permiso de sobrescritura.
- [ ] **OUT04 · P2 · M — Resultado accionable.** Mostrar archivo generado, cambios, calidad y limitaciones; abrir archivo o carpeta cuando exista soporte. **Cierre:** «terminado» aparece solo tras publicación exitosa; fallo/cancelación preserva el trabajo recuperable.
- [ ] **REP01 · P1 · L — Repetir preparación.** Guardar importación, receta, reglas y política de salida como tarea con nombre del resultado. **Cierre:** otro archivo compatible puede recorrer el flujo sin reconstruir formularios; esquema distinto exige revisión.
- [ ] **REP02 · P2 · M — Editor avanzado a demanda.** Mantener el editor de recetas como personalización del plan. **Cierre:** no presentar todas las familias de transformaciones a quien solo desea repetir una tarea.
- [ ] **REP03 · P2 · L — Lotes gráficos.** Usar el contrato batch existente, sus límites y validaciones. **Cierre:** preflight conjunto, progreso por trabajo, salida parcial honesta y ninguna sustitución implícita de archivos.
- [ ] **REP04 · P2 · L — Autoguardado opcional de proyectos.** Guardar revisiones estables con debounce y cuota. **Cierre:** indicadores guardando/guardado/error, recuperación tras reinicio y prueba de falta de espacio; no copiar snapshots por cada tecla del editor.
- [ ] **REP05 · P3 · L — Programación o vigilancia de carpetas.** Solo tras validar demanda y estabilizar tareas/lotes. **Cierre:** identidad de entradas, archivos aún en escritura, colisiones, reintentos y duplicación resueltos; acción externa con política específica.

### Coherencia, accesibilidad y mantenimiento

- [x] **CO01 · P1 · M — Herramientas contextuales.** Mostrar primero señales presentes y acciones aplicables. **Cierre:** herramientas no aplicables salen del recorrido principal y siguen disponibles en el catálogo avanzado cuando corresponde.
- [ ] **CO02 · P1 · M — Lenguaje orientado a datos.** Sustituir jerga en el flujo habitual: «reglas de calidad», «versión guardada», «columna identificadora», «copia de salida». **Cierre:** detalles como IPC, motor SQL, manifiesto o source-backed quedan en información técnica pertinente.
- [ ] **CO03 · P1 · M — Menos repetición visual.** Unificar título, archivo, estado y siguiente paso; reducir tarjetas, introducciones y avisos reiterados. **Cierre:** la acción y el estado actual se localizan sin recorrer bloques de explicación duplicados.
- [ ] **CO04 · P1 · M — Accesibilidad del flujo automático.** Foco estable, progreso anunciado con moderación, teclado y errores asociados a su decisión. **Cierre:** análisis en segundo plano no cambia foco; teclado permite revisar, cancelar, deshacer y guardar; verificar zoom 200 %.
- [ ] **CO05 · P1 · L — Estado central de ejecución.** Coordinar carga, perfil, plan, aplicación, validación y entrega con transiciones explícitas. **Cierre:** no hay doble ejecución, exportación con validación obsoleta ni resultados de un archivo anterior.
- [ ] **CO06 · P2 · M — Historial compacto.** Deshacer/rehacer cerca del resultado; revisiones y uso de disco en detalle. **Cierre:** siempre se sabe qué acción se deshará y si el historial está degradado.
- [ ] **CO07 · P1 · M — Validación con tareas reales.** Medir decisiones, retrocesos, errores y necesidad de ayuda antes y después. **Cierre:** evidencia de usuarios representativos; pruebas automáticas y fixtures no se presentan como validación de usabilidad humana.

## Orden y dependencias

1. UX01–UX07, CO01–CO04: hacer comprensible el flujo actual; corregir estados y nombres.
2. CO05 + AU01: contrato del plan y coordinación; después AU02–AU07.
3. OUT01–OUT04: cerrar el recorrido hasta una copia verificable.
4. REP01–REP02: reutilización en un archivo; después REP03–REP04.
5. REP05 solo con necesidad validada. CO06 y CO07 acompañan las entregas.

No comenzar por un rediseño cromático, una nueva barra de botones, más acordeones o un chat que sustituya formularios. Ninguno elimina por sí mismo la obligación de coordinar manualmente las operaciones. Tampoco hace falta introducir un modelo generativo para automatizar reglas deterministas existentes.

## Verificación del rediseño propuesto

| Escenario | Resultado esperado |
| --- | --- |
| CSV pequeño compatible, primera preparación | Cargar inicia diagnóstico; un plan reúne decisiones; una acción aplica y otra valida/guarda. |
| Mismo trabajo del mes siguiente | Se reutiliza la tarea aprobada; solo se revisan diferencias relevantes y el destino. |
| Identificadores con ceros iniciales | No se convierten silenciosamente; plan y salida conservan la política aprobada. |
| Fechas ambiguas o esquema cambiado | Se conserva el trabajo anterior y se muestra la decisión concreta pendiente. |
| Dataset grande | Admisión real, progreso, cancelación y alcance del análisis explícitos. |
| Edición durante un cálculo | Resultado anterior descartado; plan y validación se vinculan a la revisión correcta. |
| Fallo a mitad de preparación | Ningún estado parcial se rotula como éxito; recuperación y cambios publicados identificados. |
| Contrato reprobado | Se señalan las reglas incumplidas; no se crea una salida presentada como validada. |
| Lote con fallo tardío | Se conservan e identifican las salidas exitosas; fallos y pendientes son distintos. |
| Proyecto con autoguardado y disco lleno | Error visible; última versión válida recuperable; no se afirma que todo está guardado. |
| Teclado, zoom 200 %, pantalla reducida | Acción principal, progreso y recuperación accesibles sin depender de hover. |

Objetivos propuestos, pendientes de medir: una acción primaria visible por estado normal; cero clics para iniciar un diagnóstico admisible tras cargar; cero reanálisis manuales tras un lote normal; reducir al menos a la mitad las decisiones de una tarea repetida compatible frente a su línea base. Contar decisiones y clics de forma separada, incluyendo selectores nativos; no prometer un número universal para archivos ambiguos.

## Avance de implementación · 2026-09-14

Quedaron cerradas UX03, UX04, UX07 y CO01. La carga inicia el diagnóstico en segundo plano; el pie central permite reintentar y no avanzar mientras el perfil de la revisión actual no esté listo. Preparar prioriza señales presentes, agrupa las dos correcciones generales con su alcance real y deja las herramientas menos frecuentes bajo «Más herramientas». Guardar y administrar proyectos quedó en contexto y el panel lateral ya no muestra destinos futuros sin acción disponible.

UX01 y UX02 avanzaron con acciones contextuales y estados de análisis, pero su cierre requiere coordinar el plan, la aplicación, la validación y la entrega; esas acciones no se presentan como completadas por esta entrega. CO02 cubre lenguaje del editor de recetas, no toda la interfaz. UX06 retira destinos no navegables del flujo, todavía falta ubicarlos en información del producto. AU03 se inicia automáticamente tras cambios y ahora descarta perfiles atrasados, pero falta identificar revisiones y alcance en la comparación antes/después.

### Avance de implementación · plan acotado · 2026-09-14

Preparar ahora propone un plan ligado a la revisión actual: recortar espacios exteriores (seleccionado si hay columnas de texto) y normalizar encabezados (opcional y desmarcado porque puede afectar consultas). Una sola acción envía las opciones al motor. El backend ejecuta la combinación elegida sobre una candidata y la publica como una revisión reversible; el plan se oculta mientras el diagnóstico de la nueva revisión está pendiente. Los resultados de un análisis que termine tarde ya no pueden reemplazar el perfil de una revisión posterior.

Esto es avance parcial de AU01, AU02, AU03 y CO05. El plan aún no reúne las señales de limpieza, decisiones sobre datos ambiguos, validación y salida en un contrato común; el estado común para todas las operaciones también queda pendiente. No se promete que esta unidad cubra las transformaciones que se ejecutan por otros comandos. La próxima etapa debe ampliar el plan sin encadenar llamadas que publiquen cada una un estado parcial. Los cambios que alteran significado o eliminan datos conservan aprobación explícita. La revisión estática y las pruebas automatizadas no sustituyen pruebas con usuarios; CO07 sigue pendiente.

## Fuentes locales

- [Shell y coordinación](src/App.tsx): `loadSelection`, `analyzeQuality`, invalidación y pie de navegación.
- [Carga](src/features/load/LoadPhase.tsx) y [modelo de carga](src/features/load/loadModel.ts): perfiles, hojas y preflight.
- [Revisar](src/features/review/ReviewPhase.tsx) y [recomendaciones](src/features/review/qualityActionPlan.ts).
- [Preparar](src/features/prepare/PreparePhase.tsx) y [controlador de operaciones](src/features/prepare/usePrepareController.ts).
- [Editor de recetas](src/features/prepare/TransformRecipeEditor.tsx) y [comparación histórica](src/features/prepare/RevisionComparison.tsx).
- [Entrega](src/features/delivery/DeliveryPhase.tsx), [proyectos](src/features/projects/ProjectsPanel.tsx) y [historial](src/features/prepare/HistoryBar.tsx).
- [Contrato de diseño](DESIGN.md), [CLI y lotes](docs/reference/cli.md), [auditoría general](AUDITORIA_GENERAL_2026-09-13.md) y [auditoría visual histórica](AUDITORIA_DISENO_2026-09-06.md).

Esta auditoría complementa la general: sus casillas evalúan automatización y facilidad de uso, aunque una capacidad técnica equivalente ya esté implementada. Las casillas sin marcar permanecen como trabajo pendiente; se ordenan por prioridad y sus criterios de cierre evitan presentar mejoras parciales como terminadas.
