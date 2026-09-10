# Diseño de Columnia

Esta guía documenta el lenguaje visual que ya existe en la aplicación. No es una
propuesta de rediseño: sirve para que cambios y contribuciones nuevas conserven
la misma jerarquía, claridad y accesibilidad.

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

## Color y superficies

Los tokens de `src/styles.css` son la fuente de verdad:

| Uso | Token principal |
| --- | --- |
| Fondo de aplicación | `--canvas` |
| Barra lateral | `--sidebar-surface` |
| Contenido | `--surface`, `--surface-subtle`, `--surface-raised` |
| Texto | `--text-primary`, `--text-secondary` |
| Separación | `--border-subtle` |
| Estado actual y acción | `--accent`, `--action-fill`, `--action-hover` |
| Teclado | `--focus-ring` |
| Estados | `--success`, `--warning`, `--danger`, `--info` |

No se crean colores por funcionalidad ni por espacio de trabajo. El acento se
reserva para selección actual, foco y acción principal. El color nunca es la
única señal de estado.

## Tipografía

- Títulos y rótulos de navegación: `--font-display`, actualmente Bahnschrift con
  fallbacks Aptos Display y Segoe UI.
- Cuerpo, formularios y tablas: `--font-body`, actualmente Aptos con fallbacks
  Segoe UI Variable Text y Segoe UI.
- El texto de cuerpo no baja de 16 CSS px cuando contiene instrucciones o
  explicaciones esenciales.
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
  no solo mediante tooltip.

## Navegación y espacios de trabajo

La barra lateral separa dos conceptos:

1. **Espacios de trabajo:** áreas mayores del producto.
2. **Flujo:** etapas secuenciales dentro del espacio actual.

Mientras solo Analizar sea accionable, “Espacios de trabajo” es una región
informativa con lista de estados, no un `nav` ni un conjunto de botones. El copy
vigente es:

- `Analizar — Actual`.
- `Automatizar — CLI disponible · Interfaz en preparación`.
- `Preparar para BI — Interfaz planificada`.

Los espacios no usan números; la numeración pertenece al flujo Cargar → Revisar
→ Preparar → Entregar. Cuando exista un segundo espacio accionable se debe revisar
el modelo de foco, selección, URL o persistencia antes de convertir la región en
navegación.

## Estados y copy

- Usa lenguaje de utilidad: qué ocurrió, qué está disponible y qué acción sigue.
- Evita promesas, slogans y texto sobre el propio diseño.
- Loading, vacío, error, éxito y parcial deben describir lo que la persona ve y
  cómo puede continuar.
- Los estados vacíos ofrecen contexto y una acción primaria; “No hay elementos”
  por sí solo no es suficiente.
- Los errores aparecen cerca de la acción y no eliminan el último estado válido.

## Responsive

- Escritorio: barra lateral vertical, contenido principal amplio y utilidades al
  final de la barra.
- Vista compacta: marca y dataset forman la primera franja; los espacios se
  convierten en dos líneas informativas; las fases conservan la fila horizontal
  existente.
- A 320 CSS px y zoom 200 %, el contenido puede envolver texto pero nunca crear
  scroll horizontal global.
- No escondas acciones esenciales detrás de hover ni reduzcas objetivos táctiles
  por debajo de 44 × 44 CSS px.
- Un panel que crece debe poder desplazarse sin volver inaccesibles dataset,
  preferencias o contenido principal.

## Accesibilidad

- Conserva skip link, landmarks, encabezados y orden de tabulación coherentes.
- Solo los elementos accionables reciben foco.
- `aria-current` identifica la etapa actual; el texto visible también comunica
  el estado.
- Todo texto normal cumple WCAG AA, al menos 4.5:1; controles y texto grande,
  al menos 3:1 cuando corresponda.
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
