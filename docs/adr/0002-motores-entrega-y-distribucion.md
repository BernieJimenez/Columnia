# ADR-0002: motores, entrega, actualización y distribución de la versión 1.x

- Estado: Aceptada
- Fecha: 2026-10-04
- Alcance: Columnia `1.26.0`; reemplaza las decisiones 1 (binarios), 4 y 5 de
  [ADR-0001](0001-contratos-del-repositorio.md)

## Contexto

ADR-0001 describía el prototipo `0.49.0`: solo Polars, DuckDB como decisión
futura, distribución abierta de código y binarios, y una versión fija. Desde
entonces el producto incorporó varios componentes cuyas decisiones solo
estaban en el CHANGELOG y en CONTEXTO (DOC-06). Este ADR las registra tal como
están aplicadas en el código; no introduce cambios.

## Decisiones

1. **Dos motores de datos.** Polars procesa en memoria los datasets que caben
   en ella (camino eager y lazy de recetas). DuckDB (`duckdb` con el feature
   `bundled`, sin extensiones remotas) lee los archivos grandes sin cargarlos
   (perfil, consultas SQL, JOIN, comparación y exportación «source-backed»),
   con memoria y disco temporal acotados por sesión. Los tres caminos de
   receta deben dar el mismo resultado; lo vigila un test de paridad.
2. **Entrega ODBC.** La exportación a SQL Server, PostgreSQL y MySQL usa
   `odbc-api` con los controladores del sistema. Es una acción explícita de la
   persona hacia un motor que elige; las credenciales no se guardan y cada
   entrega «añadir filas» deja un registro local para avisar antes de repetirla.
3. **Actualizaciones.** `tauri-plugin-updater` está compilado pero solo se
   activa si la compilación tiene `COLUMNIA_UPDATER_ENDPOINT`; valida la firma
   con la clave pública embebida. Hoy no hay endpoint: la publicación de
   actualizaciones no está autorizada (ver distribución).
4. **Persistencia local.** Proyectos, tareas reutilizables y presets de entrega
   viven en catálogos SQLite separados en la carpeta de datos de la app, con
   migraciones versionadas; los datos de cada proyecto se guardan como
   generaciones Parquet. Un catálogo auxiliar ilegible se aparta sin bloquear
   el arranque (ARQ-02).
5. **Informes de fallo locales.** Un pánico escribe un informe mínimo en
   `crash-reports/` (versión, hora, archivo y línea; nunca el mensaje ni datos).
   No se envía a ningún sitio.
6. **Distribución y uso.** El canal aprobado es el código fuente en GitHub, sin
   instaladores ni actualizaciones publicados
   ([`legal-distribution-decision.json`](../reference/legal-distribution-decision.json)).
   El uso real es personal, del propio autor; beta pública, distribución de
   binarios y lectores de pantalla externos quedan aparcados hasta una nueva
   decisión.
7. **Versionado.** SemVer; la versión debe coincidir en `package.json`,
   `package-lock.json`, `src-tauri/Cargo.toml`, `Cargo.lock` y
   `tauri.conf.json` (lo comprueban el test de sincronía y `tools/release.ps1`).
   El salto de `0.167.0` a `1.25.0` marcó el cierre del alcance v1, no una
   serie de versiones intermedias publicadas.

## Consecuencias

- Dos motores duplican parte de la lógica (recetas, perfil); cada cambio de
  receta exige el test de paridad y cada cambio del perfil, comparar ambos
  caminos.
- ODBC y el updater amplían la superficie de confianza solo cuando la persona
  los usa o cuando se configure un endpoint; la política de red lo vigila
  (`npm run network:check`).
- Publicar binarios o actualizaciones requiere antes firma Authenticode y una
  revisión legal separada (ADR nuevo).

## Alternativas consideradas

- **Solo Polars:** se descartó porque los archivos de varios GB no caben en
  memoria en equipos normales.
- **Solo DuckDB:** se descartó porque las transformaciones interactivas y el
  historial de deshacer sobre datos en memoria son más simples y rápidos con
  Polars.
- **Exportación a bases de datos por drivers nativos de cada motor:** se
  descartó a favor de ODBC para no incorporar tres clientes de red distintos.
