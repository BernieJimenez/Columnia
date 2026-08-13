# Columnia

Columnia será una estación local multiplataforma para revisar, limpiar,
transformar y entregar datasets confiables.

El proyecto está en su primer hito técnico. Actualmente contiene el shell Tauri
2, una interfaz React/TypeScript y el primer corte vertical del motor Polars:
selección nativa, carga local y vista previa de CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS de hasta
500 MB. Los libros con varias hojas muestran un selector antes de cargar y React
solo recibe un identificador opaco, nunca la ruta local. Este límite es provisional:
el dataset aún se materializa en memoria y un archivo
grande puede requerir bastante más RAM durante perfiles y transformaciones.
Parquet conserva su esquema nativo, incluidos tipos temporales compatibles,
nulos y texto Unicode, y se lee con una configuración conservadora de memoria.

## Plataformas objetivo

- Windows
- macOS
- Linux

El diseño evita APIs exclusivas de un sistema operativo. Sin embargo, cada
plataforma se considerará soportada únicamente después de compilar, instalar y
probar Columnia localmente en ese sistema.

## Requisitos de desarrollo

- Node.js LTS.
- Rust estable instalado mediante `rustup`.
- Dependencias nativas de Tauri para el sistema operativo correspondiente.

La máquina de desarrollo actual ya dispone de Rust/Cargo, WebView2 y las Build
Tools de Visual Studio. El shell nativo se compila localmente en Windows.

## Desarrollo local

```powershell
npm install
npm run tauri dev
```

La validación permanece completamente local. Desde PowerShell:

```powershell
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
```

`Fast` comprueba formato, compilación Rust, pruebas frontend y build web. `Full`
añade Clippy con warnings como errores y las pruebas Rust. `Release` agrega el
binario Tauri optimizado sin crear instaladores ni usar servicios externos.

El flujo principal replica el orden de `dataprepv1.1`: **Cargar → Revisar →
Preparar → Entregar**. Cada acción aparece únicamente en la etapa que le
corresponde.

En **Cargar**, usa **Seleccionar dataset**. Rust abre el diálogo nativo para CSV,
TSV, TXT delimitado, JSON/JSON Lines, Parquet, XLSX, XLS, XLSB u ODS. En cada libro permite
elegir la hoja y decidir si la primera fila contiene encabezados o debe conservarse
como datos generando `column_1`, `column_2`, etc. Rust valida y
conserva el dataset en la sesión; React recibe solamente el esquema,
los metadatos y las primeras 50 filas.
Durante la carga se muestra el avance por fases. El perfil de calidad informa el
porcentaje conforme termina cada columna; ambos canales permanecen dentro del
equipo mediante IPC de Tauri.
Las operaciones activas se pueden cancelar. El perfil se detiene entre columnas;
la lectura del dataset se descarta después de terminar la fase que Polars tenga en curso.
Cancelar una selección de hoja o una sustitución conserva el dataset que ya estaba activo.
TSV usa tabuladores estrictos y conserva valores léxicos como ceros iniciales y
decimales formateados. Excel/ODS conserva booleanos, enteros, decimales, fechas y
duraciones cuando una columna es compatible; las mezclas inseguras quedan como texto.
CSV también conserva todas sus columnas físicamente como texto porque el formato
no contiene un esquema confiable. Calidad calcula estadísticas numéricas semánticas
cuando todos los valores no vacíos son números seguros; identificadores con ceros
iniciales y enteros que perderían precisión quedan excluidos de esa interpretación.
CSV y TXT detectan de forma conservadora coma, punto y coma, tabulador o `|`,
respetando delimitadores dentro de campos entrecomillados. TSV fuerza tabulador.
Se acepta UTF-8 con o sin BOM; bytes inválidos se rechazan sin sustituir caracteres.
XLSX, XLSB y ODS advierten que su tamaño comprimido puede requerir bastante más RAM.
JSON admite un arreglo de objetos o un objeto por línea (`.jsonl`/`.ndjson`).
Los campos ausentes quedan como nulos y los objetos o arreglos anidados se conservan
como texto JSON, sin aplanarlos ni descartar su contenido silenciosamente.

Desde **Entregar**, el dataset activo puede exportarse a CSV o Parquet. Rust abre
el selector nativo y escribe primero un archivo temporal en la carpeta elegida.
El destino se reemplaza únicamente después de completar y sincronizar la
escritura; cancelar o fallar conserva cualquier archivo anterior.
En **Revisar**, la vista previa permite recorrer el dataset en páginas de 50 filas sin volver a abrir
el archivo ni enviar su ruta al frontend.
El botón **Analizar calidad** calcula en Rust los nulos, la completitud y los
valores únicos no nulos de cada columna. Para columnas numéricas también muestra
mínimo, máximo y promedio; el resultado se reutiliza durante la sesión.
El mismo análisis cuenta filas duplicadas adicionales y, para texto, cadenas
vacías y longitudes mínima, máxima y promedio. Las cadenas compuestas solo por
espacios se consideran vacías.
Para columnas de texto con al menos tres valores, Columnia sugiere tipos
booleano, entero, decimal o fecha cuando al menos el 90% coincide. La sugerencia
es informativa: en este hito no transforma el dataset.
Para columnas numéricas, el perfil añade desviación estándar muestral, Q1,
mediana, Q3 y posibles outliers mediante la regla IQR de 1.5. Los nulos y
valores no finitos se excluyen; no se señalan outliers con menos de cuatro datos.

En **Preparar**, cuando el perfil encuentra duplicados exactos, **Eliminar duplicados** conserva
la primera aparición y elimina las repeticiones posteriores de la sesión activa.
El CSV original no se modifica. La operación ofrece un nivel de **Deshacer** y
el perfil debe recalcularse sobre el resultado.

La corrección **Normalizar nombres de columnas** sigue las reglas del proyecto
de referencia: minúsculas, eliminación de acentos, `_` para espacios y guiones,
prefijo `col_` cuando el encabezado comienza con un número y sufijos estables
cuando dos encabezados producen el mismo nombre. También puede deshacerse.

Columnia también puede **Recortar espacios** exteriores en todas las columnas
de texto sin modificar su contenido interno. Para una limpieza más profunda,
**Normalizar texto** permite elegir columnas concretas, convertir a minúsculas,
compactar espacios y decidir si se eliminan acentos. La interfaz informa cuántas
celdas y filas cambiaron, conserva los nulos y permite deshacer el resultado.

La barra **Continuidad de trabajo** permite deshacer y rehacer la revisión más
reciente. **Aplicar recomendadas** agrupa el recorte exterior y la normalización
de encabezados en una sola operación atómica: ambos cambios se publican juntos
o el dataset permanece intacto. El historial está limitado honestamente a una
revisión mientras se diseña almacenamiento temporal con presupuesto de disco
para datasets grandes.

La pestaña **Transformaciones** permite construir una receta estructural con
varios renombres, conversiones de tipo y parseos de fecha. La receta se aplica
una sola vez y en orden determinista: renombres, tipos y fechas. Las conversiones
son estrictas —un valor inválido cancela el lote completo—, los nulos se preservan
y un resultado correcto ocupa una única revisión de Deshacer/Rehacer. Esta receta
es todavía una operación inmediata de sesión; no se guarda ni se ejecuta de forma
lazy.

La receta también admite hasta tres **filtros AND** y una **columna calculada**.
Como los filtros pueden eliminar filas, Columnia muestra una confirmación antes
de ejecutar. Los cálculos aceptan otra columna o un valor fijo de forma
explícita, con suma, resta, multiplicación, división, concatenación y extracción
de año, mes o día. El motor preserva nulos y cancela el lote completo ante
conversiones imprecisas, división por cero, infinitos o fechas no representables.

Validación rápida y completamente local:

```powershell
.\tools\check.ps1 -Profile Fast
```

La validación completa añade el build nativo sin empaquetar:

```powershell
.\tools\check.ps1 -Profile Full
```

No se utilizan CI, GitHub Actions ni workflows. Consulta [ROADMAP.md](ROADMAP.md)
para conocer las decisiones y fases previstas.
