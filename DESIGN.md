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
  (`.delivery-summary`) dice qué va a pasar junto al botón. Si queda fijo en
  pantalla, la página reserva su alto con `scroll-padding-bottom` para que no
  tape el foco.
- Una conexión a SQL Server se pide con campos (servidor, base de datos,
  autenticación), no con una cadena ODBC: la interfaz manda campos y Rust arma la cadena; no hay cadena libre ni DSN.
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
(`.load-brief`, con el botón «Seleccionar dataset», el aviso de que también se
puede arrastrar un archivo y los formatos admitidos) y, en un plegable
`.load-secondary`, los archivos recientes; con un dataset ya cargado la zona se
reduce a «Seleccionar otro dataset» y la pista de arrastrar. «Reutilizar una
tarea» (`ReusableTaskPanel.tsx`) es otro plegable `.load-secondary` bajo la
opción elegida. Los avisos son bloques `.notice` encima de las tarjetas: motor
sin respuesta, cierre inesperado de la sesión anterior, informes de fallo y
catálogos apartados, estos tres con «Entendido». Las decisiones de importación
(hoja y encabezados, perfil guardado, coste estimado, esquema distinto) se
toman en un diálogo modal (`ModalDialog`) que no cambia el dataset hasta
confirmar.

**Preparar** (`src/features/prepare/`) reúne el trabajo a la izquierda y el
historial a la derecha (`.prepare-layout`). Arriba va la propuesta
(`PrepareProposal.tsx`): una lista de cambios con casilla y una frase cada uno,
«Ver antes y después», una sola acción primaria («Aplicar…») y «Personalizar
paso a paso», que pregunta de uno en uno con respuesta Sí o No; tras aplicar, la
propuesta se sustituye por el resultado con solo las cifras que cambiaron y
«Deshacer». Lo demás está en el plegable «Diagnóstico y herramientas avanzadas»
(`.prepare-advanced`): medir el efecto de los cambios y dos pestañas,
Correcciones (señales y acciones sueltas) y Transformaciones
(`TransformRecipeEditor.tsx`, el editor de receta: varias operaciones que se
aplican juntas y se pueden guardar y cargar como archivo). El historial
(`HistoryBar.tsx`, `<aside>` «Historial de cambios») lleva «Deshacer» y
«Rehacer» bajo el título y la lista de versiones. A partir de 1180 CSS px es un
panel de 300 px pegajoso bajo la barra superior (`position: sticky` a
`calc(var(--topbar-height) + 8px)`, con scroll propio si es más alto que la
ventana, `src/data-views.css`); por debajo pasa a una columna, tras el trabajo.
El resultado de un cambio con «Deshacer» (`.change-result`) queda fijo arriba
mientras se usan las herramientas.

**Explorar** (`src/features/explore/ExplorePhase.tsx`, `explore.css`) muestra un
panel que Columnia arma con todas las filas: una franja de cifras
(`.explore__kpis`) y debajo una cuadrícula de gráficos (`.explore__grid`) de dos
columnas. Hay barras por categoría («Filas por valor», cada barra es un botón
que filtra todo el panel), un histograma («Distribución») y una tendencia
(«Filas en el tiempo»). El histograma y la tendencia ocupan la fila entera
(`.explore__panel--wide`), y un gráfico de barras que queda solo en su fila
también. Los filtros
activos son botones sobre la franja, con «Quitar filtros», y «Personalizar»
permite elegir las columnas de los gráficos. El histograma y la tendencia traen
su cifra como tabla («Ver como tabla»), y cada barra o columna seleccionada se
marca con algo más que el color. Por debajo de 760 CSS px la cuadrícula pasa a
una columna.

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
