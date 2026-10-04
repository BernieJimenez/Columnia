# Red, privacidad y telemetría

Columnia es local-first. En esta versión el flujo local de la aplicación no
inicia conexiones de red, no envía datasets y no incorpora telemetría, analytics
ni envío de informes de fallo: el único informe de fallo es un archivo local
que nunca sale del equipo (ver abajo). La entrega ODBC es una acción explícita del usuario hacia el
motor remoto que elija; no es un servicio obligatorio ni se ejecuta durante la
carga, revisión o preparación local.

## Inventario permitido

- El `ipc:` de Tauri y `http://ipc.localhost` son transporte interno del shell;
  no son destinos de Internet.
- Las herramientas de desarrollo pueden usar loopback para Vite/CDP. Esas
  reglas viven en el CSP de desarrollo y no se habilitan en la política de
  producción.
- La instalación de WebView2 puede descargar el bootstrapper si el equipo no
  tiene el runtime. Es una decisión del instalador, no una petición de datos
  de Columnia.
- La comprobación y descarga de actualizaciones solo aparece cuando la
  compilación tiene `COLUMNIA_UPDATER_ENDPOINT`; requiere una acción explícita,
  muestra versión/notas/tamaño y valida firma y tamaño antes de instalar. Una
  compilación sin endpoint mantiene esa capacidad desactivada.

## Archivos que Columnia escribe fuera de tus exportaciones

Todo se guarda en el equipo; nada se envía. En la carpeta de datos de la app
(`%APPDATA%pp.columnia.desktop`):

- `projects.sqlite3` y `project-snapshots/`: el catálogo de proyectos y las
  copias de los datos de cada proyecto guardado.
- `reusable-tasks.sqlite3` y `delivery-presets.sqlite3`: tareas reutilizables y
  presets de entrega. Si uno no se puede abrir al arrancar, se aparta como
  `<nombre>.unreadable-<segundos>` (no se borra) y Cargar avisa.
- `remote-deliveries.json`: registro de entregas «añadir filas» a bases de datos
  (destino y huella del contenido, sin filas), para avisar antes de repetir una.
- `session.active`: marca de sesión abierta; si sigue ahí al arrancar, Cargar
  avisa de que la sesión anterior se cerró de forma inesperada.
- `crash-reports/`: hasta 20 informes mínimos de fallo, solo locales, con
  versión, hora y archivo/línea del código; nunca el mensaje del fallo ni datos.

En la carpeta temporal del sistema (`%TEMP%`), borrados al terminar o en el
siguiente arranque:

- `columnia-history-*`: historial de deshacer de la sesión (copias de los datos).
- `columnia-utf8-*`: copia UTF-8 de un archivo Windows-1252 que aceptaste leer.
- `columnia-query-*` y `duckdb-spill/`: archivos de trabajo de consultas grandes.

Junto a una exportación, solo si lo pides, un archivo `.pbids` para abrirla en
Power BI. Las preferencias de la interfaz (tema, archivos recientes,
autoguardado) viven en el almacenamiento local del WebView.

## Controles

`npm run network:check` inspecciona `src/` y `src-tauri/src/` buscando APIs de
red, clientes HTTP y proveedores de telemetría, y verifica que el CSP de
producción solo permita el transporte IPC interno. `supply-chain:check` lo
ejecuta junto con npm audit, cargo-audit, cargo-deny, el escaneo de secretos y
el inventario de avisos de terceros.

El informe de diagnóstico v1 es una exportación local iniciada por la persona:
primero muestra una vista previa y solo escribe JSON después de confirmar en el
selector nativo de archivos. Su contrato Rust/TypeScript permite exclusivamente
la versión de Columnia, una etapa, un estado, hasta cinco códigos enumerados y,
si se eligen, rangos agregados de filas, columnas y tamaño de origen. Rechaza
campos desconocidos y no acepta mensajes de error, rutas, consultas, nombres,
valores, trazas, credenciales, logs ni identificadores del dispositivo. La
acción no tiene destino de red ni activa captura en segundo plano; cancelar el
selector no crea un archivo.

No se guardan URLs de datasets, contenido de filas, nombres de usuario ni
telemetría en servicios externos. Cualquier futura excepción requiere:

1. consentimiento explícito del usuario;
2. redacción y minimización de PII;
3. un ADR que delimite destino, datos, retención y apagado;
4. pruebas que verifiquen que la función está desactivada por defecto.

## Artefactos y conectores futuros

Los documentos ejecutables mantienen sus referencias locales únicamente en el
archivo que necesita la operación. Cuando una receta, un manifiesto o un
reporte se serializa para stdout, evidencia o un conector futuro, la CLI usa la
política común de `src-tauri/src/privacy.rs`: elimina rutas absolutas,
referencias `file://`/`fixture://`, nombres de archivo, valores, emails,
secretos y los campos de entrada, receta, salida y almacén. Los identificadores,
estados y conteos agregados se conservan para que el resultado siga siendo útil
sin revelar datos del usuario ni su carpeta.

Esta sanitización es una frontera adicional y no sustituye la validación de
recetas o manifiestos. Un conector remoto futuro debe consumir el JSON
sanitizado, pedir consentimiento explícito y declarar su retención antes de
habilitar cualquier red.
