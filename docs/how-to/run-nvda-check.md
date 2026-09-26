# Cómo probar Columnia con NVDA

Este recorrido cubre la aceptación nativa de accesibilidad (RV09): completar
Cargar → Revisar → Preparar → Entregar solo con teclado y un lector de pantalla
real. Lleva unos 30 minutos. Anota los resultados en la tabla del
[checklist manual de accesibilidad](../../ACCESSIBILITY_MANUAL_CHECKLIST.md).

## Preparación (5 minutos)

1. Instala NVDA desde [nvaccess.org](https://www.nvaccess.org/download/). Es
   gratuito.
2. Abre Columnia con `npm run tauri dev` y ten a mano
   `fixtures\automation\quality-input.csv`, que no contiene datos reales.
3. Arranca NVDA (`Ctrl` + `Alt` + `N`). Para pararlo: `Insert` + `Q`.
4. Aparta el ratón. Toda la prueba se hace con el teclado.

Teclas que vas a usar (`NVDA` es la tecla `Insert`):

| Tecla | Qué hace |
| --- | --- |
| `Tab` / `Shift` + `Tab` | Siguiente / anterior control |
| `H` / `Shift` + `H` | Siguiente / anterior encabezado |
| `D` | Siguiente región (landmark) |
| `T` | Siguiente tabla; dentro, `Ctrl` + `Alt` + flechas recorre celdas |
| `NVDA` + `Espacio` | Alterna entre modo foco y modo exploración |
| `NVDA` + `F7` | Lista de encabezados, enlaces y regiones |
| `Espacio` / `Enter` | Activa el botón o casilla con foco |

## Recorrido

Para cada paso, anota **qué esperabas oír** y **qué oíste**. Un paso pasa si se
puede completar sin ratón y el anuncio permite entender qué ocurre.

### 1. Cargar

- [ ] `Tab` desde el inicio llega a «Saltar al contenido principal» y lo anuncia.
- [ ] `NVDA` + `F7` muestra regiones con nombre: navegación principal, contenido.
- [ ] «Seleccionar dataset» abre el selector de Windows; eliges el CSV.
- [ ] El diálogo de importación anuncia su título («Revisar encabezados de…»).
- [ ] Las dos opciones de encabezados se anuncian como botones de opción.
- [ ] «Ver columnas y tipos» se anuncia como contraída y se expande con `Enter`.
- [ ] `Escape` cierra el diálogo y el foco vuelve a «Seleccionar dataset».
- [ ] Vuelves a abrirlo y «Cargar archivo» lleva a Revisar.

### 2. Revisar

- [ ] Se anuncia el cambio de etapa («Etapa activa: Revisar»).
- [ ] El progreso del análisis se anuncia y después termina, sin repetirse sin
      parar.
- [ ] Con `H` llegas a «Encontramos N cosas para arreglar» y la lista se lee
      como pares nombre y cifra.
- [ ] «Ver cambios propuestos» lleva a Preparar.

### 3. Preparar

- [ ] «Columnia propone N cambios» se anuncia como encabezado.
- [ ] Cada cambio es una casilla con nombre y descripción.
- [ ] «Ver antes y después» se anuncia como contraído; al abrirlo, `T` llega a la
      tabla y se leen los encabezados Cambio, Columna, Antes y Después.
- [ ] «Aplicar N cambios» aplica y se anuncia el resultado («Listo: cambios
      aplicados»).
- [ ] «Deshacer» funciona con teclado y se anuncia.

### 4. Entregar

- [ ] «Validar calidad» y «Exportar sin validar» se anuncian como botones de
      opción de un mismo grupo.
- [ ] «Columnia propone N comprobaciones» se lee como grupo con casillas.
- [ ] «Usar estas N comprobaciones» cambia el botón a «Validar y exportar».
- [ ] Si hay datos personales, el aviso y su casilla se anuncian antes del botón.
- [ ] Un error (por ejemplo, un campo ODBC vacío) se anuncia y queda asociado a
      su campo.
- [ ] La exportación termina con el selector de Windows y se anuncia el
      resultado.

## Después

1. Repite Revisar y Entregar con **Windows en contraste alto** (`Alt` izquierdo +
   `Mayús` izquierdo + `Impr Pant`) y con zoom del sistema al 125 %.
2. Completa la tabla del checklist: fecha, Windows, versión de NVDA, escenario y
   resultado.
3. Por cada fallo, anota los pasos, el anuncio esperado y el oído, y una
   prioridad (P0–P3, como en la [guía de la beta](run-beta-validation.md)). No
   incluyas datos reales en capturas ni notas.
