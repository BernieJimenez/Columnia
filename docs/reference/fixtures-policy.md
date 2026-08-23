# Política de fixtures

Las fixtures son entradas y salidas pequeñas, deterministas y sintéticas para
probar contratos. No son un almacén de ejemplos reales ni un lugar para copiar
datasets de clientes.

## Permitido

- Filas inventadas sin relación con personas, empresas o expedientes reales.
- Valores obvios de prueba como `Alice`, `Bob`, `Ventas`, `10` o fechas creadas
  para el caso.
- Casos extremos reproducibles: nulos, Unicode, fórmulas como texto, errores de
  validación, colisiones y estructuras incompletas.
- Archivos JSON y CSV versionados; otros formatos solo cuando sean necesarios
  para cubrir una ruta del parser y su origen sintético quede documentado.

## Prohibido

- PII, datos financieros o de salud, credenciales, tokens, cookies y secretos.
- Copias descargadas de clientes, proveedores, sistemas internos o internet.
- Rutas absolutas, nombres de usuario, dumps de bases de datos y artefactos de
  ejecución local.
- Fixtures no deterministas que dependan de la hora, locale, red o una carpeta
  privada.

## Contrato mecánico

[`fixtures/manifest.json`](../../fixtures/manifest.json) enumera los archivos
versionados y declara que cada uno es sintético. `src/governance.test.ts`
comprueba que la lista coincide con el árbol, rechaza extensiones típicas de
secretos y busca marcadores de claves privadas. El manifest debe actualizarse
en el mismo cambio que agrega, elimina o renombra una fixture.

La salida temporal de smokes, benchmarks y validaciones va en `.local/`, que
está ignorado por Git y se elimina al terminar el flujo correspondiente.
