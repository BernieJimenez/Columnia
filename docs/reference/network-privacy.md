# Red, privacidad y telemetría

Columnia es local-first. En la versión `0.55.0` el código de la aplicación no
inicia conexiones de red, no envía datasets y no incorpora telemetría,
analytics, crash reporting ni servicios remotos obligatorios.

## Inventario permitido

- El `ipc:` de Tauri y `http://ipc.localhost` son transporte interno del shell;
  no son destinos de Internet.
- Las herramientas de desarrollo pueden usar loopback para Vite/CDP. Esas
  reglas viven en el CSP de desarrollo y no se habilitan en la política de
  producción.
- La instalación de WebView2 puede descargar el bootstrapper si el equipo no
  tiene el runtime. Es una decisión del instalador, no una petición de datos
  de Columnia.
- La comprobación y descarga de actualizaciones será una capacidad futura de
  I6. Deberá ser opt-in, visible y documentada antes de incorporar un cliente
  de red.

## Controles

`npm run network:check` inspecciona `src/` y `src-tauri/src/` buscando APIs de
red, clientes HTTP y proveedores de telemetría, y verifica que el CSP de
producción solo permita el transporte IPC interno. `supply-chain:check` lo
ejecuta junto con npm audit, cargo-audit, cargo-deny, el escaneo de secretos y
el inventario de avisos de terceros.

No se guardan URLs de datasets, contenido de filas, nombres de usuario ni
telemetría en servicios externos. Cualquier futura excepción requiere:

1. consentimiento explícito del usuario;
2. redacción y minimización de PII;
3. un ADR que delimite destino, datos, retención y apagado;
4. pruebas que verifiquen que la función está desactivada por defecto.
