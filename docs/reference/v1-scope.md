# Alcance de Columnia V1

## Modelo de uso

Columnia V1 es una estación de trabajo local para una sola persona operadora en
su propio equipo. No incluye cuentas, registro, inicio de sesión, organizaciones,
roles, sincronización entre dispositivos ni colaboración remota.

Los proyectos se identifican mediante IDs locales opacos y se guardan en el
almacenamiento privado de la aplicación. La identidad de una persona no forma
parte del modelo de datos ni de los contratos IPC.

## Problema principal

Permitir que una persona revise, limpie, transforme, valide y entregue datasets
tabulares confiables sin subirlos a un servicio web y sin depender de una cuenta.

## Capacidades incluidas

- Cargar archivos locales y mantener sus rutas fuera de React.
- Perfilar esquema, tipos, nulos, duplicados, outliers, formatos y posibles datos
  personales.
- Preparar datos mediante transformaciones reversibles y recetas locales.
- Comparar, consultar, unir y consolidar datasets con límites explícitos.
- Aplicar reglas de calidad antes de entregar resultados.
- Guardar y recuperar proyectos, preferencias operativas e historial agregado.
- Automatizar operaciones con la CLI local sobre el mismo motor Rust.
- Consultar recursos del equipo y elegir un perfil local de concurrencia.

## Formatos obligatorios

### Entrada local

- Delimitados: CSV, TSV y TXT delimitado.
- Libros: XLSX, XLSB y ODS. XLS heredado conserva su fallback compatible.
- Semiestructurados: JSON, JSONL y NDJSON.
- Columnar: Parquet.

### Entrega local

- CSV, XLSX, Parquet y JSON.
- Script SQL y base SQLite.
- Bundle de Columnia con manifiesto, diccionario y receta cuando corresponda.

### Destinos opcionales

PostgreSQL, MySQL/MariaDB y SQL Server mediante ODBC instalado por la persona.
Esta salida es explícita, no es necesaria para trabajar localmente y no convierte
Columnia en un servicio remoto. Ninguna credencial se conserva en proyectos.

## Persistencia permitida

Un proyecto puede conservar el dataset durable, reglas de calidad, borrador de
receta, perfil cacheado ligado al snapshot, historial reversible, actividad SQL
agregada, vista y etapa activa, página visible, motor SQL, cobertura de análisis,
perfil de rendimiento y opciones de entrega.

No se guardan consultas SQL, credenciales ODBC, rutas internas expuestas a React,
muestras de datos ni valores derivados que no sean necesarios para restaurar el
trabajo. Estas exclusiones son parte del contrato de privacidad, no faltantes de
paridad.

## Rendimiento V1

Los presupuestos versionados de
`fixtures/performance/performance-baseline-v1.json` son el contrato de V1:

- Arranque desde proceso nativo listo hasta ventana visible: máximo 1.000 ms.
- Recorrido nativo normal: máximo 512 MiB de working set y 256 MiB privados.
- Dataset grande de referencia: mínimo 100 MiB, máximo 1,5 GiB de working set y
  1 GiB privado durante el recorrido WebView2.
- Benchmark local de 100 MiB: tres transformaciones sostenidas, dos
  actualizaciones durables y máximo 512 MiB de working set.
- JOIN source-backed: evidencia adicional de 512 MiB sin materialización completa.

Las operaciones compatibles se ejecutan source-backed. Las combinaciones que
necesitan materialización pasan por admisión de RAM y fallan de forma accionable
si el equipo no dispone del presupuesto necesario.

## Actualizaciones

V1 conserva el updater firmado de Tauri. La comprobación y descarga requieren
una acción explícita, muestran versión y tamaño, verifican firma y no transmiten
datasets. El trabajo local sigue disponible aunque no exista red o no se configure
un canal de actualizaciones.

## Fuera de alcance

- Cuentas, autenticación, perfiles remotos y permisos multiusuario.
- Nube obligatoria, telemetría y sincronización automática.
- Edición colaborativa o servidor de proyectos.
- GPU como requisito del motor.
- Compatibilidad de importación con contratos retirados de otros productos.
- Declarar soporte público de una plataforma o destino sin su validación real.
