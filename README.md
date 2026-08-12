# Columnia

Columnia será una estación local multiplataforma para revisar, limpiar,
transformar y entregar datasets confiables.

El proyecto está en su primer hito técnico. Actualmente contiene el shell Tauri
2, una interfaz React/TypeScript y el primer corte vertical del motor Polars:
selección nativa, carga local y vista previa de archivos CSV de hasta 100 MB.

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

En la ventana de Columnia, usa **Seleccionar CSV**. Rust abre el diálogo nativo,
valida y conserva el dataset en la sesión; React recibe solamente el esquema,
los metadatos y las primeras 50 filas.
La vista permite recorrer el dataset en páginas de 50 filas sin volver a abrir
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
