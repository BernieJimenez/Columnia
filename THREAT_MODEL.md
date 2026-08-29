# Threat model vivo de Columnia

> Estado verificado el 2026-08-28. Este documento describe el sistema implementado, no una garantía absoluta de seguridad. Debe actualizarse cuando cambien datos, IPC, permisos, red, parsers, persistencia o distribución.

## Alcance y supuestos

Columnia es una aplicación de escritorio Tauri y una CLI que procesan datasets locales con React, Rust y Polars. Este modelo cubre la ventana `main`, el puente IPC, `columnia-cli`, el motor de datos, archivos elegidos por la persona, proyectos SQLite, snapshots temporales o durables, recetas y exportaciones.

Se asume que el sistema operativo, Tauri/WebView y la cuenta local funcionan como fronteras externas. Un atacante con control de la cuenta, del proceso o del sistema operativo queda fuera de las garantías actuales. Tampoco se afirma resistencia criptográfica, aislamiento frente a malware local ni seguridad de formatos que aún no se hayan probado de forma adversarial.

## Activos

- Contenido y metadatos de datasets, incluidas vistas previas y perfiles cacheados.
- Rutas locales, nombres de archivos y estructura de directorios.
- Recetas de transformación y reglas de calidad.
- Integridad del dataset activo, historial y archivos exportados.
- Disponibilidad de la aplicación y recursos del equipo: memoria, CPU y disco.
- Integridad del binario, dependencias, lockfiles y artefactos distribuidos.

## Fronteras de confianza

```text
Archivo no confiable / persona
          |
          v
Diálogo nativo y validación Rust
          |
          v
Motor Rust + parsers ----------> filesystem local
          ^                           |
          | IPC tipado                v
React/WebView                  snapshots y exportaciones
          |
          v
CSP + capability de ventana

CLI + argumentos ------------> motor Rust compartido
```

1. **Archivo → Rust:** CSV, JSON, Parquet y hojas de cálculo son entrada no confiable aunque provengan del disco local.
2. **React/WebView → Rust:** la UI puede solicitar casos de uso registrados por IPC, pero Rust conserva la autoridad sobre rutas y estado.
3. **Rust → filesystem:** lecturas, temporales, snapshots y exportaciones cruzan una frontera con efectos persistentes.
4. **Aplicación → dependencias/plataforma:** Tauri, WebView, Polars, Calamine y demás crates o paquetes forman parte de la cadena de suministro.
5. **Desarrollo → producción:** desarrollo permite Vite y WebSocket locales; producción usa una CSP más cerrada.
6. **Shell local → CLI:** quien ejecuta la CLI proporciona rutas deliberadamente, incluido un `--store` obligatorio para proyectos; el proceso reutiliza las validaciones y operaciones del motor, sin la mediación de diálogos ni el sandbox de la WebView.

## Adversarios y no objetivos

Adversarios considerados:

- un archivo local malformado o construido para agotar recursos, confundir tipos o explotar un parser;
- contenido web inyectado que intente ampliar su acceso mediante IPC;
- una ruta manipulada, symlink o reparse point que intente escapar del archivo elegido;
- una dependencia comprometida o vulnerable;
- errores operativos que sobrescriban una exportación o entreguen datos que no cumplen las reglas esperadas.

No objetivos actuales:

- proteger datos frente a una cuenta local o sistema operativo ya comprometidos;
- cifrar datasets, recetas, snapshots o exportaciones en reposo;
- ofrecer separación multiusuario, control de acceso por rol o auditoría centralizada;
- garantizar recuperación después de cerrar la aplicación o fallar el equipo;
- consultar y descargar actualizaciones desde un endpoint HTTPS configurado para
  una compilación de distribución; el trabajo con datos sigue siendo local-first.

## Superficies, amenazas y estado

| Superficie | Amenazas principales | Controles implementados | Riesgo residual o pendiente |
| --- | --- | --- | --- |
| Selección y lectura de archivos | Traversal, symlinks, reparse points, directorios usados como archivos, formatos falsos | Rust mantiene las rutas privadas; canonicaliza lecturas; exige archivos regulares; rechaza symlinks y reparse points en entradas actuales | Un parser vulnerable sigue pudiendo fallar después de validar la ruta. La extensión no prueba que el contenido sea seguro. |
| Parsers de CSV, JSON, Parquet y hojas | Corrupción, payloads patológicos, descompresión o consumo excesivo | UTF-8 estricto para texto; validaciones de formato y pruebas funcionales; duplicados normalizados por bloques CPU con huellas XXH3-128 compactas y cancelación cooperativa | Los datasets no tienen un tope fijo de tamaño. No hay un presupuesto estricto de RAM/CPU ni análisis adversarial completo por formato; la capacidad depende de los recursos disponibles y de la expansión del formato. |
| IPC React ↔ Rust | Invocación inesperada, deriva de contratos, argumentos manipulados, fuga de rutas | Se registran 61 comandos de producción y 4 exclusivos de debug; [`ipc-inventory.json`](docs/reference/ipc-inventory.json) se compara con `generate_handler!`; la fachada TypeScript centralizada mantiene paridad de comandos, argumentos, retornos y 57 estructuras compartidas; IDs opacos evitan exponer rutas; recetas y reglas de calidad tienen presupuestos semánticos validados en Rust; `open_last_export` no acepta una ruta desde React | La UI comprometida puede intentar cualquier comando registrado. Los presupuestos actuales se aplican después de deserializar y no limitan el transporte bruto. Cada comando nuevo debe actualizar el inventario y validar sus argumentos y autorización de estado en Rust. |
| CLI local | Rutas o almacenes manipulados, hoja ambigua, receta/contrato/manifiesto inválido, colisiones, sobrescritura parcial o fuga de rutas/datos en resultados automatizados | Parser estricto; canonicalización; hoja exacta y encabezado obligatorio; contratos de calidad `columnia-quality-rules` v1 estrictos, con límite de 1 MiB, compatibilidad legada v1 y rechazo de formatos/versiones futuras; batch limita manifiestos a 1 MiB/64 trabajos, hace preflight y rechaza colisiones con outputs/fuentes; proyectos exigen un `--store` explícito y canonicalizado; cada exportación es atómica; JSON v1 sin rutas, filas ni muestras | La CLI hereda la autoridad de la cuenta local y no es un sandbox ni autenticación. Acepta datasets sin tope fijo, por lo que una entrada grande puede agotar RAM, CPU o disco. Un `--store` explícito puede apuntar a una ubicación sensible accesible por la cuenta. Batch no es una transacción global: un fallo de datos tardío conserva outputs anteriores y lo declara por ordinal. Los documentos JSON todavía se deserializan antes de aplicar sus presupuestos semánticos internos. |
| Capability y plugins Tauri | Ampliar acceso a filesystem, shell, HTTP o apertura externa | `main` tiene únicamente `core:default`; no existen permisos frontend de filesystem, shell, HTTP u opener; diálogo, archivos y updater se operan desde Rust; el updater valida firma con la clave pública embebida y rechaza versiones semver iguales o anteriores | `core:default` y cada plugin futuro deben revisarse al actualizar Tauri. Añadir una permission por comodidad rompería el principio de mínimo privilegio. |
| WebView y contenido frontend | XSS, navegación o conexión remota, carga de contenido externo | CSP de producción limitada a `self`; bloquea objetos y frames; `connect-src` solo admite `self` e IPC local. El updater nativo usa HTTPS solo cuando la compilación de distribución lo configura | `style-src` permite `unsafe-inline`. La CSP reduce impacto, pero no sustituye evitar inyección. La CSP de desarrollo admite Vite y WebSocket locales. |
| Transformaciones e historial | Resultado parcial, corrupción de estado, datos previos irrecuperables, crecimiento de disco | Las recetas compuestas publican un único candidato o revierten; las imputaciones directas, incluida la categórica explícita como `Desconocido`, publican impacto agregado y una revisión reversible; Deshacer/Rehacer usa snapshots Parquet; cada historial conserva como máximo 12 revisiones/1 GiB; los cambios invalidan perfil y validación previa | El límite es por proyecto, no global. Varios proyectos pueden acumular bastante disco y los snapshots no están cifrados. Un snapshot demasiado grande puede desactivar reversión. |
| Proyectos y recuperación | Traversal desde catálogo, symlink/reparse, snapshot/perfil corrupto, cursor inválido, desincronización SQLite↔Parquet, migración incompleta, borrado accidental o exposición persistente de datos | En escritorio, root canonicalizado bajo `app_data_dir`; en CLI, root obligatorio y canonicalizado desde `--store`; IDs opacos; nombres acotados; SQL parametrizado; esquema SQLite v4 compatible con catálogos v1/v2/v3; snapshots con nombre administrado, containment y rechazo de links/reparse; nueva generación atómica antes de transacción DB; apertura y exportación validan el estado durable antes de usarlo, mientras inspección limita su respuesta a metadatos seguros; la actividad SQL persistida está acotada a estado, duración y filas, sin consultas, rutas ni valores; `project-delete` exige que `--confirm` coincida con `--id`; contratos JSON no exponen rutas ni muestras | Snapshots, perfiles, reglas, borradores y actividad SQL agregada durables no están cifrados ni tienen borrado seguro. Una cuenta local comprometida puede leer o alterar app-data o el almacén CLI elegido. La validación fail-closed puede impedir recuperar un proyecto corrupto hasta repararlo o eliminarlo. La confirmación exacta reduce errores operativos, pero no sustituye autorización. Un fallo al limpiar después de borrar puede dejar archivos huérfanos. |
| Exportación | Sobrescritura parcial, enlace de destino, entrega inválida, fórmulas de hoja de cálculo, fuga a ruta equivocada | Canonicaliza la carpeta; valida el destino; rechaza destinos no regulares y enlaces; escribe y sincroniza un temporal antes de reemplazar; cancelación no destruye el archivo previo; en escritorio la calidad exige reglas o bypass explícito; en CLI de proyectos las reglas guardadas siempre deben aprobar y `--allow-unvalidated` solo admite cero reglas; CSV antepone apóstrofo a texto con prefijos de fórmula y Parquet conserva el frame original; tras una exportación exitosa, Rust revalida el último archivo retenido en sesión y abre su carpeta sin aceptar rutas arbitrarias desde React | La persona aún puede elegir un destino sensible o, donde corresponde, autorizar una entrega sin reglas. No existe clasificación de datos ni prevención de exfiltración local. La neutralización CSV cambia deliberadamente la representación exportada de esas celdas de texto. La retención del último destino no es durable y el explorador opera con la autoridad de la cuenta local. |
| Errores, resultados y evidencia local | Filtrar celdas, rutas o contenido en mensajes/reportes | Los resultados de calidad devuelven conteos sin muestras; los reportes de gates no incluyen rutas ni contenido y usan hashes de lockfiles | No existe una política global verificada para todo mensaje de error o futuro logging. Cada nuevo diagnóstico debe revisarse por fuga de datos. |
| Instancia única | Carreras de apertura, ventana inaccesible, abuso por proceso local | Plugin oficial de escritorio registrado primero; la segunda apertura muestra, desminimiza y enfoca `main`; los errores de ventana no causan panic | No protege frente a un proceso local hostil ni constituye autenticación. Los argumentos de la segunda instancia no se procesan actualmente. |
| Dependencias y distribución | Paquete comprometido, versión vulnerable, binario o instalador manipulado | npm y Cargo usan lockfiles; gates verifican coherencia, procedencia oficial, integridad criptográfica e identidades contradictorias; Release genera offline un SBOM CycloneDX reproducible; Package firmado produce MSI/NSIS y `.sig`, inventaría artefactos con SHA-256 y valida el manifiesto updater; toolchains de Node/npm/Rust están fijados | No hay CI ni firma Authenticode. El SBOM y los hashes no aportan confianza si no se revisan o publican mediante un canal confiable. La primera creación NSIS puede descargar herramientas oficiales, cuya integridad valida Tauri. La clave privada del updater y su rotación requieren custodia operativa; la política de avisos depende de los metadatos de lockfiles y requiere revisión legal. |

## Controles actuales que no deben degradarse

- Procesamiento de datos local sin servicios externos requeridos.
- Autoridad de rutas y archivos exclusivamente en Rust; la UI recibe identificadores opacos.
- Capability mínima para `main` y CSP separada para producción y desarrollo.
- Canonicalización, rechazo de enlaces y comprobación de archivos regulares en entradas actuales.
- Transformaciones compuestas y exportaciones con publicación atómica.
- Cancelación cooperativa para carga, perfil y exportación.
- Contratos de calidad sin muestras de celdas y reportes locales sin rutas ni contenido.
- Gate automático de paridad IPC y validación local Fast, Full y Release.
- Neutralización de fórmulas en texto CSV sin modificar columnas tipadas ni exportaciones Parquet.
- Presupuestos semánticos de recetas y reglas de calidad aplicados por Rust en todas sus entradas.
- Documentos de calidad con formato canónico, límite de archivo, guardado atómico
  y rechazo cerrado de versiones futuras o campos desconocidos.
- SBOM reproducible y gates offline que fijan registros oficiales, checksums y ausencia de fuentes Git.
- CLI con parser estricto, resultados JSON versionados sin rutas y reutilización de los controles de lectura, receta y exportación del motor.
- Proyectos confinados al app-data privado, con SQL parametrizado, generaciones atómicas, apertura fail-closed ante corrupción y respuestas IPC sin rutas.
- Proyectos CLI ligados a un `--store` explícito y canonicalizado; exportación sin bypass de reglas reprobadas y borrado con confirmación exacta del ID.
- Updater nativo limitado a HTTPS configurado, firma Tauri y versiones semver estrictamente posteriores; la consulta solo ocurre tras acción explícita.

## Riesgos residuales prioritarios

La prioridad es orientativa; no sustituye una evaluación formal de severidad y probabilidad.

1. **Agotamiento de recursos por entrada no confiable:** no existe un tope fijo para datasets, el contenido se materializa en memoria y algunas operaciones crean copias completas. La capacidad depende de la RAM, la CPU y el disco disponibles, y falta un presupuesto efectivo de recursos y expansión por formato.
2. **Cadena de suministro y distribución:** SBOM, lockfiles, instaladores firmados y updater tienen evidencia local; faltan escaneo periódico, publicación en un canal confiable, VM limpia y rotación operativa de claves.
3. **Datos en disco:** proyectos Parquet, perfiles, historiales, reglas de calidad y borradores de receta durables no tienen garantía de cifrado ni borrado seguro. El límite de 12 revisiones/1 GiB se aplica por proyecto, por lo que el conjunto puede consumir mucho más disco y aumenta deliberadamente el tiempo de exposición local.
4. **Cobertura adversarial de parsers:** falta verificar archivos comprimidos o patológicos y expansión de memoria por formato; los presupuestos semánticos actuales no limitan el JSON bruto antes de deserializar.
5. **Cobertura multiplataforma:** las garantías de rutas y empaquetado deben verificarse también en macOS y Linux.
6. **Monolitos de UI y motor:** el tamaño de `App.tsx` y `dataset.rs` aumenta el radio de impacto de cambios de seguridad.

## Checklist de seguridad para cambios

Antes de integrar un cambio, responde y verifica:

- [ ] ¿Introduce un formato, parser, dependencia, plugin, URL o servicio nuevo?
- [ ] ¿Amplía `capabilities/main.json`, la CSP o los comandos IPC? Si sí, ¿la ampliación es mínima y está justificada?
- [ ] ¿Rust valida tamaño, tipo, límites, nombres, índices y estado para toda entrada nueva del frontend?
- [ ] ¿Las rutas siguen ocultas a React, canonicalizadas y protegidas contra symlinks/reparse points?
- [ ] ¿Una escritura usa temporal y reemplazo seguro sin destruir el archivo anterior ante fallo o cancelación?
- [ ] ¿La operación tiene límites de memoria, CPU, disco y cardinalidad proporcionales al input?
- [ ] ¿Errores, eventos, resultados y reportes evitan rutas, credenciales y muestras de datos?
- [ ] Si cambia la CLI, ¿sus contratos siguen versionados, deterministas y libres de rutas o valores sensibles?
- [ ] ¿Una transformación completa sigue siendo atómica y conserva una política explícita de reversión?
- [ ] ¿Se probaron entradas malformadas, límites, cancelación, rollback y diferencias de plataforma aplicables?
- [ ] ¿El gate IPC y el perfil `Full` pasan? Para distribución, ¿pasa también `Release`?
- [ ] ¿Este documento y `CONTEXTO.md` reflejan cualquier cambio real de frontera, control o riesgo?

## Protocolo de actualización

Actualiza `THREAT_MODEL.md` en el mismo cambio cuando:

- se agregue o retire un formato, parser, comando IPC, plugin o permission;
- cambien CSP, red, rutas, temporales, persistencia, exportación o actualización;
- se introduzca telemetría, logging, servicios externos o distribución firmada;
- una prueba demuestre o invalide un control descrito aquí;
- aparezca una vulnerabilidad o incidente que cambie la evaluación de riesgo.

Procedimiento:

1. Verifica el comportamiento en código, configuración y pruebas; no promociones planes a controles implementados.
2. Actualiza la fecha, la superficie afectada, el control actual y el riesgo residual.
3. Añade una prueba o evidencia reproducible para la nueva garantía cuando sea viable.
4. Elimina afirmaciones obsoletas en vez de acumular excepciones contradictorias.
5. Registra decisiones duraderas en `CONTEXTO.md`; conserva este archivo centrado en amenazas y mitigaciones.

## Fuentes de verdad usadas

- `CONTEXTO.md`: arquitectura, flujos, controles y pendientes verificados.
- `src-tauri/tauri.conf.json`: ventana y CSP de producción/desarrollo.
- `src-tauri/capabilities/main.json`: alcance de permisos de la ventana principal.
- `src-tauri/Cargo.toml` y `package.json`: dependencias y scripts declarados.

Ante una contradicción, el código y las pruebas actuales prevalecen sobre este documento.
