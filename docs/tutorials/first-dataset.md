# Primer dataset confiable con Columnia

En este tutorial abrirás una fixture sintética, revisarás su preview y dejarás
el proyecto listo para continuar con limpieza y entrega. Al final tendrás una
ejecución local reproducible, sin subir datos a un servidor.

## Qué necesitas

- Windows x64 con WebView2.
- Node.js LTS, Rust estable y las Build Tools de Visual Studio.
- Una copia local de Columnia en la raíz del repositorio.

## Paso 1: instala las dependencias

Desde PowerShell, ejecuta:

```powershell
npm install
```

El comando instala las dependencias fijadas en `package-lock.json`. No crea
servicios ni modifica datasets.

## Paso 2: abre la aplicación

Ejecuta:

```powershell
npm run tauri dev
```

En menos de tres pasos ya verás la ventana de Columnia. La fase **Cargar** debe
estar activa y el botón **Seleccionar dataset** disponible.

## Paso 3: carga la fixture sintética

Pulsa **Seleccionar dataset** y elige:

```text
fixtures\automation\input.csv
```

Columnia lee el archivo dentro del equipo. La interfaz recibe el nombre visible,
el esquema, metadatos y una preview paginada; no recibe la ruta local.

Cuando termine la carga, verifica que la preview muestra las filas sintéticas y
que puedes entrar en **Revisar**. Desde allí puedes analizar calidad y pasar a
**Preparar** sin modificar el CSV original.

## Qué construiste

Completaste el flujo inicial **Cargar → Revisar** usando una entrada
determinista. Para recetas, proyectos, exportación y contratos JSON consulta la
[referencia de la CLI](../reference/cli.md) y la
[explicación de la arquitectura](../explanation/local-first-architecture.md).

Si quieres comprobar el binario y sus capturas reproducibles, sigue el
[how-to de evidencia de release](../how-to/validate-release-evidence.md).
