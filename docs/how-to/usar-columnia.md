# Usar Columnia de principio a fin

Esta guía recorre las cinco etapas con un archivo propio: Cargar, Revisar,
Preparar, Explorar y Entregar. Para un primer contacto con datos de ejemplo,
empieza por el [tutorial](../tutorials/first-dataset.md).

Todo ocurre en tu equipo: Columnia no envía los datos a ningún servicio, salvo
cuando tú eliges entregar a una base de datos remota.

## 1. Cargar

1. Pulsa **Seleccionar dataset** y elige un CSV, TSV, JSON, Parquet, Excel u
   ODS. También puedes arrastrar el archivo a la ventana o abrir uno de
   **Explora con un dataset de ejemplo**.
2. Revisa el diálogo antes de cargar:
   - En un CSV o TXT, comprueba los encabezados. Si las columnas o los acentos
     salen mal, abre «¿Columnas o acentos mal leídos?» y elige otro separador o
     codificación.
   - En un libro de Excel, elige la hoja. Las hojas ocultas aparecen como
     «(oculta)» y el diálogo avisa de las celdas combinadas. Si hay filas de
     título encima de la tabla, Columnia las salta y dice en qué fila encontró
     los encabezados; «Generar encabezados» lee todas las filas como datos.
3. Confirma la carga. El archivo original nunca se modifica.

Para seguir con un trabajo guardado, abre **Continuar un proyecto**.

## 2. Revisar

Columnia analiza la calidad al llegar: nulos, duplicados, tipos que no
encajan, valores atípicos, coma decimal y columnas con datos personales. Lee
el diagnóstico y pulsa **Ver cambios propuestos** (o **Continuar a Preparar** si
no hay nada que corregir).

La consola SQL de Revisar consulta el dataset sin cambiarlo.

## 3. Preparar

Preparar propone los cambios seguros que el diagnóstico encontró (quitar
duplicados, convertir tipos, recortar espacios…). Elige los que quieras y
aplícalos. Cada cambio:

- muestra cuántas celdas quedaron vacías, cuántas filas se quitaron y si se
  puede deshacer;
- queda en el historial, desde donde **Deshacer** vuelve al estado anterior.

Las transformaciones a medida (renombrar, convertir con separador decimal,
filtrar, columnas calculadas) se guardan como una receta reutilizable.

Si los datos cambian después de Revisar, la barra lateral marca Revisar como
«Revisar de nuevo».

## 4. Explorar

Explorar dibuja un panel automático con indicadores, barras por categoría, un
histograma y una tendencia por fecha. Deja fuera las columnas que parecen
claves o identificadores. Con **Personalizar** eliges qué columnas usa cada
gráfico, y **Ver como tabla** muestra los números de un gráfico.

## 5. Entregar

1. Añade reglas de calidad si quieres que la copia cumpla un contrato (por
   ejemplo, ninguna fila sin importe). Columnia valida antes de exportar.
2. Elige el formato:
   - **CSV para Excel** si lo abrirás con doble clic en Excel en español:
     punto y coma, coma decimal y tildes correctas.
   - **Excel**, **Parquet**, **JSON**, **SQL**, **SQLite** o un **Paquete**
     con la receta y las reglas.
   - Una base de datos PostgreSQL, MySQL o SQL Server mediante ODBC. Windows
     pide confirmar el destino en un diálogo propio.
3. Exporta. «Copia lista» resume qué se entregó, con **Abrir carpeta** y la
   opción de abrir la copia en Power BI.

## Guardar el trabajo

Abre **Proyectos** y pulsa **Guardar proyecto nuevo** (o **Actualizar
proyecto**). Un proyecto conserva el dataset preparado, el historial, la receta
y las reglas. Para protegerlo fuera de Columnia, sigue
[Respaldar y restaurar proyectos](respaldar-proyectos.md).

## Problemas frecuentes

| Síntoma | Qué hacer |
| --- | --- |
| La aplicación no abre o se queda en blanco | Columnia necesita Microsoft Edge WebView2. Windows 11 ya lo incluye; en Windows 10, instala «WebView2 Runtime» desde la web de Microsoft y vuelve a abrirla. |
| Un CSV muestra «Ã©» en lugar de «é» | Es un archivo de Excel para Windows (Windows-1252). Cargar propone **Leer como Excel para Windows**: pulsa **Convertir y continuar**. Columnia lee una copia en UTF-8 y el original no cambia. |
| Todo el CSV aparece en una sola columna | El separador no es la coma. En el diálogo de carga, abre «¿Columnas o acentos mal leídos?» y elige punto y coma, tabulador o barra vertical. |
| Excel no deja exportar | Una hoja de Excel admite como máximo 1.048.575 filas de datos y 32.767 caracteres por celda. Exporta a CSV o Parquet para conservarlo todo. |
| Cargar avisa de que Columnia tuvo un fallo interno | Los informes están en la carpeta de datos, dentro de `crash-reports`. No incluyen tus datos. Si el fallo se repite, guárdalos para revisarlo. |
| La sesión anterior se cerró de forma inesperada | Los cambios que no estaban en un proyecto no se conservaron. Abre **Continuar un proyecto** para seguir desde el último guardado. |
| Al compilar, las pruebas Rust terminan con `STATUS_ENTRYPOINT_NOT_FOUND` | Define `COLUMNIA_TEST_HARNESS_MANIFEST=1` antes de `cargo test` (ver [CONTRIBUTING.md](../../CONTRIBUTING.md)). |
