# Revisión legal y de distribución

Estado: preparación técnica completada; revisión jurídica y decisión de
publicación pendientes.

La ficha estructurada [`legal-distribution-decision.json`](./legal-distribution-decision.json)
es la fuente de estado para el gate. Mientras conserve
`status: "pending-legal-review"`, `npm run legal:check` valida los artefactos
técnicos, pero cualquier perfil `Release` o `Package` se detiene antes de crear
instaladores publicables. El estado solo puede pasar a `approved` después de que
la persona responsable complete y revise todos los campos de la ficha.

## Qué se entrega

- `LICENSE` contiene la licencia MIT del producto.
- `THIRD_PARTY_NOTICES.md` se genera desde `package-lock.json` y
  `src-tauri/Cargo.lock`, sin licencias `UNKNOWN` y sin consultar la red durante
  el gate.
- El instalador incluye ambos archivos como recursos de Tauri.
- La aplicación muestra un panel accesible de “Licencia y privacidad” con el
  alcance local-first, retención y borrado de proyectos.
- El updater usa `tauri-plugin-updater`: la aplicación embebe solo la clave
  pública, exige firma minisign de Tauri y muestra versión, notas, tamaño,
  progreso y cancelación después de una acción explícita.

## Campos que requieren aprobación

La ficha no acepta valores implícitos. Deben completarse responsable, jurisdicción,
contacto, mercados, canal, política de privacidad, retención, revisión de marca,
revisión de avisos de terceros y distribución del updater. El gate técnico
comprueba que existan los archivos, que el JSON sea válido y que no haya estados
contradictorios; el gate de release exige además `status: "approved"` y valores
no provisionales.

## Decisiones que debe cerrar la persona responsable

Antes de publicar hay que definir la entidad responsable, jurisdicción, canal de
contacto, mercados, política de privacidad final, retención aplicable, revisión
de marcas de “Columnia” y la forma de distribuir hashes, instaladores y futuros
updaters. Este documento no constituye asesoría legal ni reemplaza los textos
completos de copyright/licencia exigidos por cada dependencia.

## Release firmado del updater

La configuración versionada conserva la clave pública, pero deja vacíos los
endpoints y `createUpdaterArtifacts` desactivado para que una compilación local
normal nunca publique ni firme por accidente. El flujo de distribución exige
estas variables en el proceso de PowerShell; la clave privada debe permanecer
fuera del repositorio:

```powershell
$env:COLUMNIA_UPDATER_ENDPOINT = "https://updates.example/columnia.json"
$env:COLUMNIA_UPDATER_ASSET_BASE_URL = "https://downloads.example/columnia/0.91.0/"
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\ruta-privada\columnia-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run release:updater:dry-run
```

`release:updater:dry-run` activa temporalmente `createUpdaterArtifacts`, ejecuta
todos los gates de Package, produce MSI/NSIS y sus `.sig`, genera el manifiesto
estático y un inventario con SHA-256, y verifica el par instalador/firma. El
reporte queda bajo `.local/validation/release-orchestration/`; no crea tags, no
sube archivos y no contacta servicios de publicación. Para una actualización
estática, el endpoint debe servir el JSON generado y la base de assets debe
servir el instalador con el mismo nombre.

La firma del updater autentica el artefacto ante Columnia, pero no es
Authenticode: Windows puede seguir mostrando “Editor desconocido” y SmartScreen.
La política ejecutable de rotación vive en
`fixtures/updater/key-policy-v1.json` y se comprueba con
`npm run updater:key:check`. Como la compilación actual confía en una sola
clave pública, una rotación segura requiere una release puente firmada por la
clave anterior que incruste la clave nueva. La clave privada histórica se
conserva offline hasta publicar y volver a descargar/verificar la release
puente, verificar la primera release firmada con la clave nueva y cerrar la
ventana de recuperación. Si la clave se compromete, se detiene el canal, se
prohíbe una release puente firmada con esa clave, se conserva la versión
instalada y se recupera mediante un instalador fuera de banda autenticado por
separado que incruste la nueva clave. Nunca se sustituye la clave confiable en
el canal sin una transición firmada; la respuesta a compromiso requiere además
la verificación independiente del instalador de recuperación.

La verificación posterior a publicación se ejecuta con
`npm run updater:verify-published`. Descarga el manifiesto y el instalador del
canal HTTPS y comprueba tamaño, SHA-256 y firma minisign con la clave pública
embebida. Un fallo no instala ni elimina la versión local y deja evidencia
sanitizada en el directorio indicado.

## Evidencia técnica

Ejecuta `npm run legal:check`, `npm run notices:check`, `npm run network:check`,
`npm run installer:check`, `npm run updater:check` y revisa
`docs/reference/ipc-inventory.json` antes de crear una release. `npm run
release:dry-run` orquesta los gates sin firma; `npm run
release:updater:dry-run` añade la compilación firmada cuando existen las cuatro
variables anteriores. Ambos dejan reportes locales sin publicar ni etiquetar.
La evidencia de release debe corresponder a un commit limpio.
