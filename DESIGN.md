# Diseño de Columnia

Esta guía documenta el lenguaje visual que ya existe en la aplicación: sirve
para que cambios y contribuciones nuevas conserven la misma jerarquía, claridad
y accesibilidad. El shell actual (barra lateral oscura en todos los temas, barra
superior con el dataset y trabajo a todo el ancho) es del 2026-10-30; sus
estilos viven en `src/shell.css`.

## Personalidad

Columnia es una estación de trabajo local para datos: sobria, precisa y calmada.
La interfaz prioriza orientación, estado y acción. Evita decoración que compita
con tablas, formularios o evidencia de calidad.

## Jerarquía del shell

```text
Producto
└── Espacio de trabajo
    └── Etapa o herramienta actual
        └── Estado del dataset
            └── Acción principal
```

- La marca orienta, pero no domina el trabajo.
- El espacio responde “¿en qué área estoy?”.
- La etapa responde “¿qué estoy haciendo ahora y qué sigue?”.
- Cada vista de contenido tiene una sola acción primaria visible.
- Estado del runtime, dataset, preferencias y asuntos legales son contexto
  secundario y no compiten con la etapa.

```text
┌──────────────┬──────────────────────────────────────────────┐
│ Columnia     │ archivo · filas · columnas · tamaño   estado │  barra superior
│ Analizar     ├──────────────────────────────────────────────┤
│ ✓ Cargar     │ Título de la etapa                           │
│ ▌2 Revisar   │ Una frase de qué se hace aquí                │
│  3 Preparar  │                                              │
│  4 Explorar  │ Trabajo a todo el ancho (máx. 1360 px),      │
│  5 Entregar  │ sobre la superficie, sin tarjeta alrededor   │
│ ──────────── │                                              │
│ Preferencias │                                              │
│ Licencia     │                                              │
└──────────────┴──────────────────────────────────────────────┘
```

- Las herramientas que sirven en cualquier etapa (hoy, «Consulta SQL») viven en
  la barra superior y se abren como panel lateral modal (`ModalDialog` con
  `variant="drawer"`): Esc cierra y el foco vuelve al botón.
- La barra superior es la única que nombra el dataset activo; las etapas no
  repiten el archivo en su cabecera, salvo Cargar, donde es lo que se elige.
- Los estados de la barra superior (motor, guardado) son texto con un punto de
  color al lado; el punto nunca es la única señal y el texto nunca usa
  `--warning` como color, porque no llega a 4,5:1 en el tema claro.

## Color y superficies

Los tokens de `src/styles.css` son la fuente de verdad:

| Uso | Token principal |
| --- | --- |
| Fondo de aplicación | `--canvas` |
| Barra lateral (oscura en todos los temas) | `--sidebar-surface` y los tokens de `.app-sidebar` |
| Contenido | `--surface`, `--surface-subtle`, `--surface-raised` |
| Texto | `--text-primary`, `--text-secondary` |
| Separación | `--border-subtle` |
| Estado actual y acción | `--accent`, `--action-fill`, `--action-hover` |
| Teclado | `--focus-ring` |
| Estados | `--success`, `--warning`, `--danger`, `--info` |

No se crean colores por funcionalidad ni por espacio de trabajo. El acento se
reserva para selección actual, foco y acción principal. El color nunca es la
única señal de estado.

### Barra lateral

La barra lateral es oscura en todos los temas, también en el claro:
`.app-sidebar` redefine sus propios tokens y el resto de la
aplicación no los ve. Los valores salen de dos sitios:

- La regla base `.app-sidebar` de `src/workflow-styles.css` sirve a los temas
  claro y oscuro (la barra es la misma en los dos): `--sidebar-surface` #142d33,
  `--surface` #20383e (etapa actual), `--surface-subtle` #243e44,
  `--surface-raised` #29474c (hover), `--border-subtle` #3a5157,
  `--text-primary` #e8f0ed, `--text-secondary` #b7c8c5, `--accent` #9cdebd,
  `--danger` #f4a39b, `--disabled-text` #a5b8b5 y `--focus-ring` #efba70.
- Los bloques `:root[data-theme="…"] .app-sidebar` de `src/styles.css` tiñen la
  barra en Papel, Océano y Slate: cambian `--sidebar-surface`, `--surface`,
  `--surface-subtle`, `--surface-raised`, `--border-subtle`, `--control-border` y
  `--accent`; los textos, el rojo y el foco siguen los de la regla base. Los
  fondos son #332a25 (Papel), #102f3b (Océano) y #242c34 (Slate).
- Los `--sidebar-surface` de `:root` en el tema claro (#ecf0ec) y en el oscuro
  (#101a1d) no llegan a pintarse: la regla base de la barra los sustituye. El
  fondo real de la barra en esos dos temas es #142d33.
- Con `forced-colors` la barra usa colores del sistema (`Canvas`, `CanvasText`,
  `Highlight`; `src/workflow-styles.css`, bloque `forced-colors`).

La tabla de contraste incluye, por tema, texto principal y secundario sobre la
barra, el texto sobre la etapa actual (`--surface` dentro de la barra), el
acento y el anillo de foco.

## Tipografía

- Títulos y rótulos de navegación: `--font-display`, actualmente Bahnschrift con
  fallbacks Aptos Display y Segoe UI.
- Cuerpo, formularios y tablas: `--font-body`, actualmente Aptos con fallbacks
  Segoe UI Variable Text y Segoe UI.
- El texto base es de 16 CSS px. Las notas, tablas, rótulos y metadatos usan
  una escala secundaria de 12 a 15 CSS px; ningún texto baja de 12 CSS px
  (ACC-04).
- Los eyebrow labels son secundarios; nunca sustituyen un encabezado claro.

## Componentes y superficies

- Usa layout, proximidad y encabezados antes de añadir contenedores.
- Una tarjeta solo se justifica cuando representa una unidad seleccionable,
  repetible o con estado propio.
- Evita mosaicos de tarjetas, iconos ornamentales, pills decorativas, gradientes,
  sombras expresivas y radios grandes uniformes.
- Los botones primarios representan la siguiente acción segura; las acciones
  destructivas conservan texto explícito y confirmación.
- Los controles deshabilitados explican el requisito faltante de forma visible,
  no solo mediante tooltip. Un botón deshabilitado del área de trabajo se
  dibuja con borde discontinuo y fondo transparente, nunca como bloque gris.
- Una elección que cambia todo lo que sigue (de dónde vienen los datos en
  Cargar, a dónde va la copia en Entregar) se hace con tarjetas de opción
  (`.choice-card`: radio, nombre y una línea de descripción) y debajo solo
  aparece lo de la opción elegida. Los formatos de esa opción van en un
  selector con la descripción del elegido debajo.
- Antes de una acción que saca datos del equipo, un resumen en pocas líneas
  (`.delivery-summary`) dice qué va a pasar junto al botón. En Entregar el
  resumen completo («Vas a exportar», `.delivery-summary-block`) va en flujo e
  incluye destino, servidor, base de datos y autenticación (nunca la
  contraseña), tabla y política; solo `.delivery-commit` (botón, una línea
  «destino · filas» y el motivo del bloqueo) queda fijo, con ventana de al
  menos 1180 × 700. Su altura real se publica en `--delivery-commit-height` y
  `scroll-padding-bottom` la usa para que ningún control con foco quede tapado
  (WCAG 2.4.11).
- Una acción destructiva lleva texto explícito, una confirmación propia ligada
  al objeto exacto («Entiendo que se borrará el contenido actual de <tabla>»)
  y un botón con verbo y objeto («Reemplazar <tabla> en <servidor>»). Una
  configuración guardada nunca reaplica «Reemplazar».
- El resultado de una acción larga (exportar, aplicar cambios) recibe el foco
  en su encabezado (`tabIndex=-1`) y se desplaza a la vista; se anuncia con una
  sola frase, no con una sección `aria-live` entera, y se presenta como una
  lista `dl` de pocas líneas, nunca como un mosaico de tarjetas de cifras.
  Mientras se muestra, el botón que lo produjo pasa a secundario.
- Un error de conexión, de campo o de análisis aparece cuando la persona
  intenta usar el formulario o la acción, no en un formulario sin tocar, y
  lleva dentro el botón que permite reintentar.
- Una conexión a SQL Server se pide con campos (servidor, base de datos,
  autenticación), no con una cadena ODBC: la interfaz manda campos y Rust arma la cadena; no hay cadena libre ni DSN.
  Al cambiar de motor en Cargar se conservan servidor, base de datos y usuario
  (y el puerto si no era el predeterminado); la contraseña no se traslada y una
  línea de estado lo dice. Los motivos de campo («Indica el servidor.») salen
  al salir del campo o cambiarlo, o cuando Entregar intenta usar la conexión;
  un formulario sin tocar solo muestra la guía general.
- Con el foco de teclado, el anillo `--focus-ring` rodea la `.choice-card`
  entera (`:has(input:focus-visible)`), no solo el radio; en forced-colors es
  `Highlight`.
- Las etiquetas y leyendas de los diálogos van en escala secundaria (.875rem,
  peso 650, sin mayúsculas forzadas) para no pesar más que el contenido.
- Una lista de problemas se ordena por lo que le hace al análisis (altera
  resultados, afecta a coincidencias, solo formato, opcional) y cada elemento
  lo dice con texto; la forma del marcador repite el nivel, no lo sustituye.

## Densidad

- `html[data-density="compact"]` (preferencia en Apariencia, guardada en
  `columnia.density` y aplicada en `main.tsx` antes del primer pintado) reduce
  márgenes, celdas de tabla y tamaños de título. Nunca reduce objetivos
  táctiles por debajo de 44 CSS px ni texto por debajo de 12 CSS px.
- La densidad no se aplica desde el script en línea de `index.html`: su hash
  forma parte de la CSP.

## Tablas de datos

- Una tabla del dataset activo resume cada columna en su cabecera cuando el
  perfil de calidad está listo (`ColumnProfileHeader`, estilos en
  `src/data-views.css`): completitud, valores distintos, rango, mini
  distribución y señales de calidad. Las cifras son de todas las filas y la
  tabla lo dice encima.
- La barra de completitud y el histograma son `aria-hidden`: la línea de texto
  ya dice cada cifra y un texto oculto (`visually-hidden`) describe la forma
  («Distribución: de x a y, la mayoría entre a y b»). El nombre accesible de la
  cabecera sigue siendo nombre y tipo, corto, porque los lectores lo repiten en
  las celdas; el perfil entero es su descripción (`aria-describedby` hacia el
  bloque `.col-profile`, con un id por posición de columna), que se oye una vez
  al situarse en la cabecera. El relleno del histograma usa el acento al 80 %
  (3,3-3,6:1 sobre la cabecera en los temas claros, 5,7:1 en el oscuro).
- Un identificador (todos los valores distintos) no muestra histograma: siempre
  sería plano.
- Mientras el perfil llega, cada cabecera reserva su hueco con un esqueleto
  `aria-hidden` (`.col-profile--pending`, sin animación) y la nota de encima
  conserva su sitio invisible, para que la tabla no salte al llegar el perfil.

## Navegación y espacios de trabajo

La barra lateral separa dos conceptos:

1. **Espacio de trabajo:** el área mayor del producto, hoy siempre Analizar.
2. **Flujo:** etapas secuenciales dentro del espacio actual.

Mientras solo Analizar sea accionable, el espacio es un rótulo sobre el flujo
(«Analizar», leído como «Espacio de trabajo: Analizar»), no un `nav` ni un
conjunto de botones. Automatizar y Preparar para BI no aparecen hasta que tengan
interfaz.

Cada etapa del flujo muestra número y nombre. Solo la actual añade su
descripción y una marca de acento a la izquierda; una etapa terminada cambia el
número por ✓ y se anuncia como «Completada»; «Revisar de nuevo» es la única
etiqueta de estado visible, porque pide una acción. En la vista compacta la
etiqueta se sustituye por un punto en la esquina y un subrayado discontinuo bajo
el nombre (ambos en `--accent`, sin añadir anchura); bajo 700 CSS px los números
desaparecen pero el ✓ de las completadas queda como marca en la esquina, también
a 320 px. El estado lo anuncia `aria-description`, no el texto visible. Los espacios no usan
números; la numeración pertenece al flujo Cargar → Revisar → Preparar →
Explorar → Entregar. Cuando exista un segundo espacio accionable se debe revisar
el modelo de foco, selección, URL o persistencia antes de convertir el rótulo en
navegación.

## Pantallas del flujo

**Cargar** (`src/features/load/LoadPhase.tsx`) pregunta de dónde vienen los
datos con tarjetas de opción (`.choice-card` dentro de `.load-sources`): Archivo,
Base de datos y Proyecto, más Ejemplo cuando hay datasets de muestra. Una
tarjeta solo aparece si su panel existe y, con una sola opción, no se muestran.
Debajo va lo de la opción elegida: para Archivo, la zona de selección
(`.load-brief`, con el botón «Elegir archivo», el aviso de que también se
puede arrastrar un archivo y los formatos admitidos) y, en un plegable
`.load-secondary`, los archivos recientes; «Elegir archivo» se abre también con Ctrl+O (`aria-keyshortcuts`) mientras la
tarjeta Archivo está activa, salvo con el foco en un campo de texto o en un
diálogo. `.load-brief` lleva borde sólido `--border-subtle`, no discontinuo:
el archivo se puede soltar en toda la ventana. Con un dataset ya cargado, la
cabecera lleva el nombre, debajo sus cifras (`DatasetMetrics`) y a la derecha
la única acción primaria, «Continuar a Revisar»; el pie de navegación no se
dibuja en Cargar para no repetirla. Debajo, un h3 «Cambiar de datos» precede a
las tarjetas y, con Archivo elegido, aparecen «Elegir otro archivo»
(secundaria) y la pista de arrastrar como texto plano. Los ejemplos se listan
como filas iguales a las de los recientes («Cargar el ejemplo …»). Los paneles
perezosos (Proyecto, Base de datos) muestran mientras cargan un texto
`role="status"` («Cargando proyectos guardados…»). «Reutilizar una
tarea» (`ReusableTaskPanel.tsx`) es otro plegable `.load-secondary` bajo la
opción elegida; su resumen dice cuántas tareas hay y, sin tareas ni dataset,
solo explica qué guarda una tarea, sin formulario deshabilitado. Borrar una
tarea se confirma en línea (`role="group"` etiquetado por la pregunta): el foco
va a «Conservar», Esc cancela y el foco vuelve a «Eliminar tarea». Los recientes solo recuerdan nombre y formato: cada uno ofrece
«Buscar en el equipo», que abre el selector, y nunca promete reabrir el archivo.
El error de elegir un archivo va junto al botón (dentro de `.load-brief`, o en
`.load-change` si ya hay dataset), dice el siguiente paso, se retira al cambiar
de fuente y recibe el foco solo si este estaba en el botón o en `<body>`. El
diálogo de revisión mide `min(960px, 100%)` y deja sus acciones fijas al pie
(`.sheet-dialog__actions--sticky`); dentro no hay una región viva dentro de
otra. Si las fechas están en «Sin definir» y una columna de texto tiene nombre
de fecha (`dateLikeTextColumns` en `loadModel.ts`), una nota avisa de que se
cargará como texto y dónde elegir su formato. Una tabla de base de datos se revisa por su nombre y su origen, no como
el archivo Parquet en que se guarda. Los avisos son bloques `.notice` encima de las tarjetas: motor
sin respuesta, cierre inesperado de la sesión anterior, informes de fallo y
catálogos apartados, estos tres con «Entendido». Las decisiones de importación
(hoja y encabezados, perfil guardado, coste estimado, esquema distinto) se
toman en un diálogo modal (`ModalDialog`) que no cambia el dataset hasta
confirmar.

**Revisar** (`src/features/review/`) solo mira: nada cambia hasta aprobar una
corrección en Preparar. El Diagnóstico abre con una lista (la misma que propone
Preparar, ordenada por consecuencia) y una sola acción primaria, «Ver cambios
propuestos»; el titular no promete que sea todo («N cosas se pueden corregir
con la propuesta»). Debajo, el bloque «No se corrige solo» recoge nulos,
valores incompatibles con el tipo sugerido, columnas con datos personales, coma
decimal y valores no finitos, cada uno con su cifra de todas las filas y una
acción secundaria («Ver … en Preparar» o «Volver a cargar con coma decimal»).
La completitud global es texto con cifra, no una barra de progreso. Un error
del perfil lleva dentro «Reintentar análisis» y una causa probable, y el pie no
repite el botón. El resultado de la Consulta SQL anuncia solo «N filas».
«Comparar con otro archivo» (`DatasetComparisonSection.tsx`) es un plegable en
dos pasos: la clave, qué cuenta como igual y una sola primaria; después el
resultado (filas nuevas, filas que faltan, conflictos, coinciden, claves
repetidas) en solo lectura, con «Combinar en Preparar» y «Descartar
comparación». Es el único nombre de la comparación en toda la aplicación. Las
regiones de tabla (`.table-region`, `.profile-region`, `.quality-chart__table`)
llevan `position: relative` para que sus textos ocultos no ensanchen la página.

**Preparar** (`src/features/prepare/`) reúne el trabajo a la izquierda y el
historial a la derecha (`.prepare-layout`). Arriba va la propuesta
(`PrepareProposal.tsx`): una lista de cambios con casilla y una frase cada uno,
«Ver antes y después», una sola acción primaria («Aplicar…») y «Personalizar
paso a paso», que pregunta de uno en uno con respuesta Sí o No; tras aplicar, la
propuesta se sustituye por el resultado con solo las cifras que cambiaron y
«Deshacer». Debajo, el encabezado «Más cambios» (`.prepare-more`) y un solo
nivel de plegables hermanos (`.prepare-tool`, una fila separada por una línea,
sin cajas): «Corregir columna por columna» (señales por columna y duplicados
parecidos; sin análisis el plegable sigue y dice por qué está vacío), «Limpiar
texto y filas» (normalizar texto, filas vacías y `_cambios`), «Transformar
columnas» (`TransformRecipeEditor.tsx`, el editor de receta: varias operaciones
que se aplican juntas y se pueden guardar y cargar como archivo; se monta al
abrirlo y la edición a medias se conserva al cerrarlo), «Combinar con otro
archivo» (solo con comparación) y «Medir el efecto de los cambios», al final
porque comprueba, no cambia. Dentro solo se pliegan la ayuda («Cómo decide
Columnia») y los grupos de operaciones del formulario de receta. El historial
(`HistoryBar.tsx`, `<aside>` «Historial de cambios») lleva «Deshacer» y
«Rehacer» bajo el título y la lista de versiones. A partir de 1180 CSS px es un
panel de 300 px pegajoso bajo la barra superior (`position: sticky` a
`calc(var(--topbar-height) + 8px)`, con scroll propio si es más alto que la
ventana, `src/data-views.css`); por debajo pasa a una columna, tras el trabajo.
El resultado de un cambio con «Deshacer» (`.change-result`) queda fijo arriba
mientras se usan las herramientas. «Combinar con otro archivo» (`CombineWithFileSection.tsx`) reutiliza la comparación de
Revisar, o permite hacerla allí, y ofrece unir por clave, añadir las filas
nuevas y resolver conflictos. Los tipos de unión van en español con el término
SQL entre paréntesis («Solo coincidencias (inner)», «Todas las filas del activo
(left)», «Todas las filas de ambos (full)»). Cada acción dice antes qué quedará
(filas y columnas), qué se descarta y si «Deshacer» lo devuelve; los conflictos
se deciden con una fila por celda y el botón al pie, nunca deshabilitado. El
resultado es un paso del historial y se anuncia en `.change-result`; «Deshacer»
devuelve las filas y lo que la combinación había descartado (contrato,
receta, perfil de importación, origen, historial SQL y vínculo con el
proyecto) si no se ha vuelto a configurar, y «Rehacer» lo vuelve a apartar. Una
corrección o un «Deshacer» conservan la comparación, recalculada, mientras
existan sus columnas clave; si no, se dice qué columna cambió.

La columna de trabajo es un contenedor (`container: prepare-main /
inline-size`): sus filas responden a su propio ancho con `@container`, porque
con el historial al lado mide unos 620 px. Tras aplicar, el resultado («Listo:
cambios aplicados») toma el foco y «Continuar a Explorar» es la primaria; tras
«Deshacer», el foco va a «Rehacer». Los botones ocupados usan `aria-disabled`
con su motivo, nunca `disabled`, para no perder el foco. Cada corrección
destructiva dice cuántas y cuáles en el botón o junto a él («Eliminar 2
columnas vacías», «Rellenar hasta 812 nulos en 3 columnas»).

**Explorar** (`src/features/explore/ExplorePhase.tsx`, `explore.css`) muestra un
panel que Columnia arma con todas las filas: una franja de cifras
(`.explore__kpis`) y debajo una cuadrícula de gráficos (`.explore__grid`) de dos
columnas. Hay barras por categoría («Filas por valor», cada barra es un botón
que filtra todo el panel), un histograma («Distribución») y una tendencia
(«Filas en el tiempo»). El histograma y la tendencia ocupan la fila entera
(`.explore__panel--wide`), y un gráfico de barras que queda solo en su fila
también. Los filtros activos son botones junto a un resumen («N filas de T
cumplen los filtros»), que es la única región `role="status"` y anuncia también
«Filtro quitado», «Filtros quitados» y el filtro que se pierde al cambiar de
medida, fecha o gráfico. Al quitar un filtro el foco va al siguiente, al
anterior o al gráfico que lo creó, y tras «Quitar filtros» a «Personalizar». Con
filtros, la cabecera dice «sobre N de M filas (filtradas)», el pie «todas las
filas que cumplen los filtros» y el filtro de rango se escribe como el tramo
(«100 a 149»). Las barras se recorren con flechas como el histograma y la
tendencia (una parada de Tab por gráfico) y se nombran «Norte: 18.200 filas».
La columna seleccionada se marca con un contorno de 2 px alrededor del relleno
(`CanvasText` en colores forzados), no solo con la intensidad. Las columnas
miden al menos 24 px; con muchas, columnas y eje se desplazan juntos y aparece
«Desliza para ver todos los tramos». Sin gráficos hay un único aviso neutro con
el motivo y, si una fecha está guardada como texto, «Interpretar fechas en
Preparar»; se ocultan «Personalizar», la pista y el pie. El valor nulo se llama
«nulo». «Ver como tabla» tiene tope de alto con scroll. Por debajo de 760 CSS px
la cuadrícula pasa a una columna.

**Entregar** (`src/features/delivery/`) pregunta primero «¿A dónde va la
copia?», después cómo validar y después el formato. «Conectar y escribir la
tabla» propone el esquema del motor (`dbo`, `public` o la base en MySQL) y una
tabla con el nombre del archivo saneado, como el script T-SQL; la política de
tabla son opciones con descripción. Un contrato fallido no se vuelve a validar
sin cambios: el motivo dice «corrige los datos en Preparar o ajusta la regla
N», la regla queda marcada con texto y borde completo y «Qué se exige» va
numerado. El término es «incumplimientos»; también «comprobación previa»,
«configuraciones guardadas» y «Paquete Columnia».

## Estados y copy

- Usa lenguaje de utilidad: qué ocurrió, qué está disponible y qué acción sigue.
- Evita promesas, slogans y texto sobre el propio diseño.
- Loading, vacío, error, éxito y parcial deben describir lo que la persona ve y
  cómo puede continuar.
- Los estados vacíos ofrecen contexto y una acción primaria; “No hay elementos”
  por sí solo no es suficiente.
- Los errores aparecen cerca de la acción y no eliminan el último estado válido.

## Responsive

- Escritorio: barra lateral vertical de 248 CSS px, utilidades al final de la
  barra y barra superior fija mientras la ventana mida más de 700 CSS px de alto.
- Vista compacta (900 CSS px o menos, o zoom 200 %): marca y utilidades forman
  la primera franja, las cinco etapas la segunda y la barra superior va debajo,
  sin quedar fija.
- A 320 CSS px y zoom 200 %, el contenido puede envolver texto pero nunca crear
  scroll horizontal global.
- No escondas acciones esenciales detrás de hover ni reduzcas objetivos táctiles
  por debajo de 44 CSS px de alto. Excepción: filas de gráficos y opciones de
  listas densas (barras de Explorar, casillas de columnas), que miden al menos
  24 CSS px (WCAG 2.5.8) y tienen alternativa por teclado.
- En la vista compacta la barra lateral ocupa como mucho el 40 % de la altura,
  también en la ventana mínima de 900 × 640 (UX-02).
- Un panel que crece debe poder desplazarse sin volver inaccesibles dataset,
  preferencias o contenido principal.

Puntos de corte del shell (`src/shell.css`, salvo que se indique otro archivo):

- 1180 CSS px o más: Preparar pone el historial en columna aparte
  (`src/data-views.css`), Entregar coloca contrato y exportación lado a lado y
  el resumen `.delivery-commit` queda fijo abajo.
- 901 a 1100: `.load-brief` reduce la ilustración a 100 px
  (`src/workflow-styles.css`).
- 901 o más de ancho y 700 de alto o menos: barra lateral más ajustada y sin la
  descripción de las etapas. 901 de ancho y 701 de alto o más: barra superior
  fija.
- 900 o menos: vista compacta, barra lateral convertida en franjas sobre el
  trabajo (ver Navegación).
- 700 o menos: desaparecen los números de las etapas y queda el ✓ como marca en
  la esquina; la exportación de Entregar pasa a una columna y el estado de la
  barra superior se alinea a la izquierda.
- 480 o menos: la fila servidor y puerto de la conexión a SQL Server se apila.
- 420 o menos: la barra lateral pasa a una columna (marca, utilidades, etapas),
  las utilidades van una por fila, «Motor local listo» se oculta a la vista y
  el margen lateral del trabajo baja a 12 px.
- 400 o menos: las etiquetas de las etapas bajan a .6875rem y su relleno se
  reduce para no partir ninguna palabra (AUD-055).
- Zoom al 200 % (`html[data-columnia-zoom="2"]`): imita la vista compacta con
  la barra lateral en una sola columna, sin números en las etapas, con la barra
  superior sin fijar y márgenes de 12 px. Es el modo que usa la evidencia de
  zoom, porque emula un viewport estrecho en una ventana ancha.
- Otros puntos de corte son de un componente, no del shell: 760 (cuadrícula de
  Explorar, `src/features/explore/explore.css`) y 640 (selectores de la
  comparación de revisiones, `src/styles.css`).

## Accesibilidad

- Conserva skip link, landmarks, encabezados y orden de tabulación coherentes.
- Solo los elementos accionables reciben foco.
- `aria-current` identifica la etapa actual; el texto visible también comunica
  el estado.
- Todo texto normal cumple WCAG AA, al menos 4.5:1; controles y texto grande,
  al menos 3:1 cuando corresponda.
- La tabla siguiente se genera desde los tokens de `src/styles.css` con
  `node tools/contrast-table.mjs`; no se edita a mano.

<!-- contrast-table:start -->
| Tema | Color | Fondo | Contraste | Mínimo |
| --- | --- | --- | ---: | ---: |
| `:root` | `--text-primary` #202e31 | `--canvas` #f5f5f1 | 12.84:1 | 4.5:1 |
| `:root` | `--text-primary` #202e31 | `--surface` #ffffff | 14.03:1 | 4.5:1 |
| `:root` | `--text-secondary` #536469 | `--surface` #ffffff | 6.19:1 | 4.5:1 |
| `:root` | `--accent` #176d62 | `--surface` #ffffff | 6.18:1 | 4.5:1 |
| `:root` | `--danger` #b5463c | `--surface` #ffffff | 5.38:1 | 4.5:1 |
| `:root` | `--control-border` #7d8b8e | `--surface` #ffffff | 3.53:1 | 3:1 |
| `:root` | `--control-border` #7d8b8e | `--canvas` #f5f5f1 | 3.23:1 | 3:1 |
| `:root[data-resolved-theme="dark"]` | `--text-primary` #e5edeb | `--canvas` #141c1f | 14.52:1 | 4.5:1 |
| `:root[data-resolved-theme="dark"]` | `--text-primary` #e5edeb | `--surface` #1b262a | 13.00:1 | 4.5:1 |
| `:root[data-resolved-theme="dark"]` | `--text-secondary` #b3c3c1 | `--surface` #1b262a | 8.47:1 | 4.5:1 |
| `:root[data-resolved-theme="dark"]` | `--accent` #84d7bd | `--surface` #1b262a | 9.16:1 | 4.5:1 |
| `:root[data-resolved-theme="dark"]` | `--danger` #f08a82 | `--surface` #1b262a | 6.38:1 | 4.5:1 |
| `:root[data-resolved-theme="dark"]` | `--control-border` #718385 | `--surface` #1b262a | 3.90:1 | 3:1 |
| `:root[data-resolved-theme="dark"]` | `--control-border` #718385 | `--canvas` #141c1f | 4.35:1 | 3:1 |
| `:root[data-theme="paper"]` | `--text-primary` #342b24 | `--canvas` #f3ecdf | 11.78:1 | 4.5:1 |
| `:root[data-theme="paper"]` | `--text-primary` #342b24 | `--surface` #fffaf0 | 13.31:1 | 4.5:1 |
| `:root[data-theme="paper"]` | `--text-secondary` #67574a | `--surface` #fffaf0 | 6.64:1 | 4.5:1 |
| `:root[data-theme="paper"]` | `--accent` #9a4f2e | `--surface` #fffaf0 | 5.71:1 | 4.5:1 |
| `:root[data-theme="paper"]` | `--danger` #ad3f38 | `--surface` #fffaf0 | 5.69:1 | 4.5:1 |
| `:root[data-theme="paper"]` | `--control-border` #8f7f6e | `--surface` #fffaf0 | 3.72:1 | 3:1 |
| `:root[data-theme="paper"]` | `--control-border` #8f7f6e | `--canvas` #f3ecdf | 3.29:1 | 3:1 |
| `:root[data-theme="ocean"]` | `--text-primary` #17313a | `--canvas` #edf4f6 | 12.28:1 | 4.5:1 |
| `:root[data-theme="ocean"]` | `--text-primary` #17313a | `--surface` #fbfeff | 13.48:1 | 4.5:1 |
| `:root[data-theme="ocean"]` | `--text-secondary` #496772 | `--surface` #fbfeff | 5.98:1 | 4.5:1 |
| `:root[data-theme="ocean"]` | `--accent` #076f85 | `--surface` #fbfeff | 5.72:1 | 4.5:1 |
| `:root[data-theme="ocean"]` | `--danger` #b74343 | `--surface` #fbfeff | 5.32:1 | 4.5:1 |
| `:root[data-theme="ocean"]` | `--control-border` #708a94 | `--surface` #fbfeff | 3.60:1 | 3:1 |
| `:root[data-theme="ocean"]` | `--control-border` #708a94 | `--canvas` #edf4f6 | 3.28:1 | 3:1 |
| `:root[data-theme="slate"]` | `--text-primary` #202a33 | `--canvas` #eef0f2 | 12.76:1 | 4.5:1 |
| `:root[data-theme="slate"]` | `--text-primary` #202a33 | `--surface` #fcfdfe | 14.32:1 | 4.5:1 |
| `:root[data-theme="slate"]` | `--text-secondary` #56636e | `--surface` #fcfdfe | 6.05:1 | 4.5:1 |
| `:root[data-theme="slate"]` | `--accent` #426c8a | `--surface` #fcfdfe | 5.51:1 | 4.5:1 |
| `:root[data-theme="slate"]` | `--danger` #aa4646 | `--surface` #fcfdfe | 5.61:1 | 4.5:1 |
| `:root[data-theme="slate"]` | `--control-border` #7c8790 | `--surface` #fcfdfe | 3.60:1 | 3:1 |
| `:root[data-theme="slate"]` | `--control-border` #7c8790 | `--canvas` #eef0f2 | 3.21:1 | 3:1 |
| `:root[data-theme="paper"] .app-sidebar` | `--accent` #f1b38e | `--surface` #4a3b32 | 5.90:1 | 4.5:1 |
| `:root[data-theme="paper"] .app-sidebar` | `--control-border` #9c8f86 | `--surface` #4a3b32 | 3.41:1 | 3:1 |
| `:root[data-theme="ocean"] .app-sidebar` | `--accent` #8edced | `--surface` #1b4352 | 6.90:1 | 4.5:1 |
| `:root[data-theme="ocean"] .app-sidebar` | `--control-border` #839da5 | `--surface` #1b4352 | 3.72:1 | 3:1 |
| `:root[data-theme="slate"] .app-sidebar` | `--accent` #acd2eb | `--surface` #35424d | 6.46:1 | 4.5:1 |
| `:root[data-theme="slate"] .app-sidebar` | `--control-border` #8d979f | `--surface` #35424d | 3.46:1 | 3:1 |
| `.app-sidebar (light)` | `--text-primary` #e8f0ed | `--sidebar-surface` #142d33 | 12.47:1 | 4.5:1 |
| `.app-sidebar (light)` | `--text-secondary` #b7c8c5 | `--sidebar-surface` #142d33 | 8.32:1 | 4.5:1 |
| `.app-sidebar (light)` | `--accent` #9cdebd | `--sidebar-surface` #142d33 | 9.36:1 | 4.5:1 |
| `.app-sidebar (light)` | `--text-primary` #e8f0ed | `--surface` #20383e | 10.68:1 | 4.5:1 |
| `.app-sidebar (light)` | `--text-secondary` #b7c8c5 | `--surface` #20383e | 7.12:1 | 4.5:1 |
| `.app-sidebar (light)` | `--focus-ring` #efba70 | `--sidebar-surface` #142d33 | 8.21:1 | 3:1 |
| `.app-sidebar (dark)` | `--text-primary` #e8f0ed | `--sidebar-surface` #142d33 | 12.47:1 | 4.5:1 |
| `.app-sidebar (dark)` | `--text-secondary` #b7c8c5 | `--sidebar-surface` #142d33 | 8.32:1 | 4.5:1 |
| `.app-sidebar (dark)` | `--accent` #9cdebd | `--sidebar-surface` #142d33 | 9.36:1 | 4.5:1 |
| `.app-sidebar (dark)` | `--text-primary` #e8f0ed | `--surface` #20383e | 10.68:1 | 4.5:1 |
| `.app-sidebar (dark)` | `--text-secondary` #b7c8c5 | `--surface` #20383e | 7.12:1 | 4.5:1 |
| `.app-sidebar (dark)` | `--focus-ring` #efba70 | `--sidebar-surface` #142d33 | 8.21:1 | 3:1 |
| `.app-sidebar (paper)` | `--text-primary` #e8f0ed | `--sidebar-surface` #332a25 | 12.09:1 | 4.5:1 |
| `.app-sidebar (paper)` | `--text-secondary` #b7c8c5 | `--sidebar-surface` #332a25 | 8.06:1 | 4.5:1 |
| `.app-sidebar (paper)` | `--accent` #f1b38e | `--sidebar-surface` #332a25 | 7.73:1 | 4.5:1 |
| `.app-sidebar (paper)` | `--text-primary` #e8f0ed | `--surface` #4a3b32 | 9.23:1 | 4.5:1 |
| `.app-sidebar (paper)` | `--text-secondary` #b7c8c5 | `--surface` #4a3b32 | 6.16:1 | 4.5:1 |
| `.app-sidebar (paper)` | `--focus-ring` #efba70 | `--sidebar-surface` #332a25 | 7.96:1 | 3:1 |
| `.app-sidebar (ocean)` | `--text-primary` #e8f0ed | `--sidebar-surface` #102f3b | 12.15:1 | 4.5:1 |
| `.app-sidebar (ocean)` | `--text-secondary` #b7c8c5 | `--sidebar-surface` #102f3b | 8.10:1 | 4.5:1 |
| `.app-sidebar (ocean)` | `--accent` #8edced | `--sidebar-surface` #102f3b | 9.12:1 | 4.5:1 |
| `.app-sidebar (ocean)` | `--text-primary` #e8f0ed | `--surface` #1b4352 | 9.19:1 | 4.5:1 |
| `.app-sidebar (ocean)` | `--text-secondary` #b7c8c5 | `--surface` #1b4352 | 6.13:1 | 4.5:1 |
| `.app-sidebar (ocean)` | `--focus-ring` #efba70 | `--sidebar-surface` #102f3b | 8.00:1 | 3:1 |
| `.app-sidebar (slate)` | `--text-primary` #e8f0ed | `--sidebar-surface` #242c34 | 12.20:1 | 4.5:1 |
| `.app-sidebar (slate)` | `--text-secondary` #b7c8c5 | `--sidebar-surface` #242c34 | 8.14:1 | 4.5:1 |
| `.app-sidebar (slate)` | `--accent` #acd2eb | `--sidebar-surface` #242c34 | 8.87:1 | 4.5:1 |
| `.app-sidebar (slate)` | `--text-primary` #e8f0ed | `--surface` #35424d | 8.89:1 | 4.5:1 |
| `.app-sidebar (slate)` | `--text-secondary` #b7c8c5 | `--surface` #35424d | 5.93:1 | 4.5:1 |
| `.app-sidebar (slate)` | `--focus-ring` #efba70 | `--sidebar-surface` #242c34 | 8.03:1 | 3:1 |
<!-- contrast-table:end -->

- `forced-colors` usa colores del sistema y mantiene bordes, foco y estado.
- El movimiento respeta `prefers-reduced-motion` y solo se añade cuando explica
  una transición o progreso.

## Lista de revisión

Antes de integrar una interfaz nueva, confirma:

- La primera acción y el estado actual se entienden en tres segundos.
- Cada sección tiene un solo trabajo.
- Los controles parecen controles y el contenido informativo no parece clicable.
- Teclado, lector de pantalla, 320 CSS px, zoom 200 % y forced-colors funcionan.
- No se duplicaron tokens ni patrones que ya existen.
- El cambio sigue siendo claro al retirar sombras, iconos y color decorativo.
