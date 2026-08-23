# ADR-0001: contratos iniciales del repositorio

- Estado: Aprobada
- Fecha: 2026-08-23
- Alcance: Fase I0, prototipo `0.49.0`

## Contexto

Columnia es una aplicación de escritorio local para preparar datasets. El
prototipo combina Tauri 2, Rust, Polars, React, TypeScript y Vite. El código ya
protege las rutas y los datos en Rust, no necesita un servidor para funcionar y
se valida con comandos locales. Faltaban decisiones explícitas para que una
persona nueva supiera qué puede publicar, sobre qué plataforma se acepta el
primer soporte y qué cambios requieren revisión.

## Decisiones

1. **Licencia y distribución.** Columnia usa la licencia MIT. El modelo inicial
   es distribución abierta del código y de los binarios del prototipo, sin
   telemetría obligatoria ni servicio remoto requerido. La licencia se revisará
   antes del primer release público si cambia el titular o el modelo comercial.
2. **Plataforma inicial.** Windows x64 es el objetivo de soporte inicial. macOS
   y Linux siguen siendo objetivos de diseño, pero no se declararán soportados
   hasta compilar, instalar y ejecutar sus validaciones localmente.
3. **Frontera de la aplicación.** React presenta estado y solicita casos de uso
   mediante IPC tipado. Rust conserva la autoridad sobre rutas, archivos,
   datasets y almacenamiento. No se introduce un servidor HTTP de aplicación.
4. **Motor de datos.** Polars es el motor implementado en este prototipo.
   DuckDB queda como decisión futura y solo se incorporará después de medir el
   benchmark y justificar sus operaciones multidataset.
5. **Versionado.** Se mantiene SemVer y la versión `0.49.0` debe coincidir en
   `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `Cargo.lock` y
   `src-tauri/tauri.conf.json`; el test de sincronía lo verifica.
6. **Calidad.** La validación oficial es local. No se agregan GitHub Actions,
   CI, telemetría ni servicios de pago obligatorios como requisito del
   repositorio.
7. **Datos de prueba.** Las fixtures son sintéticas, deterministas y libres de
   PII. No se aceptan datasets reales, secretos, credenciales ni artefactos de
   usuario en el árbol versionado.

## Flujo de cambio

```text
rama corta -> diff revisado -> governance:check -> gate Fast/Full -> integración
```

Las reglas concretas de nombres de rama, commits y revisión están en
[`CONTRIBUTING.md`](../../CONTRIBUTING.md).

## Consecuencias

- MIT simplifica el uso y redistribución del prototipo, pero no sustituye una
  revisión legal si el producto cambia de titular, incorpora activos externos o
  adopta un modelo comercial.
- Windows x64 recibe primero la evidencia de instalación y ejecución. Los
  cambios multiplataforma deben conservar APIs portables y aportar evidencia de
  cada sistema antes de cambiar el estado de soporte.
- La frontera Rust/UI mantiene la superficie de privacidad pequeña, a cambio de
  que nuevos casos de uso necesiten contratos en ambos lados del IPC.
- La ausencia de CI reduce infraestructura y costo, pero cada contribuidor debe
  ejecutar y conservar los gates locales correspondientes.

## Alternativas consideradas

- **Licencia propietaria:** se descartó para el prototipo porque impediría la
  redistribución abierta prevista y no aporta una ventaja técnica ahora.
- **Soporte público simultáneo para Windows, macOS y Linux:** se descartó hasta
  contar con compilación, instalación y smoke local en cada sistema.
- **Servidor o telemetría desde el inicio:** se descartó porque no es necesario
  para el flujo local y ampliaría la superficie de confianza.
