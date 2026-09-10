# Documentación de Columnia

Esta carpeta contiene las decisiones y contratos de trabajo que no caben en la
guía funcional. El código y sus pruebas siguen siendo la fuente de verdad del
comportamiento implementado.

La [guía de diseño](../DESIGN.md) documenta el lenguaje visual, la jerarquía del
shell y los contratos responsive y de accesibilidad para contribuciones UI.

## Índice Diátaxis

### Tutorial

- [Primer dataset confiable](tutorials/first-dataset.md): recorrido desde la
  instalación hasta Cargar → Revisar con una fixture sintética.

### How-to

- [Ejecutar una sesión beta](how-to/run-beta-validation.md): validar el recorrido
  completo con datos reales sin conservar información sensible.
- [Validar evidencia del release](how-to/validate-release-evidence.md): compilar
  el binario, capturar ventanas y comprobar el baseline.
- [Publicar y verificar un release](how-to/publish-release.md): preparar un
  release firmado, descargar los assets publicados y reanudar con seguridad.

La [plantilla de sesión beta](templates/beta-session.md) normaliza tareas,
hallazgos y veredictos; las copias completadas pertenecen a `.local/beta/`.
La [plantilla de resumen beta](templates/beta-summary.md) define la evidencia
agregada y sanitizada que se versiona al cerrar los gates.

### Referencia

- [Alcance de Columnia V1](reference/v1-scope.md): modelo local sin cuentas,
  formatos obligatorios, persistencia, presupuestos y límites explícitos.
- [Referencia de la CLI](reference/cli.md): comandos, opciones, contratos JSON y
  códigos de salida.
- [Paridad funcional](reference/feature-parity.md): capacidades disponibles,
  contratos de calidad y brechas pendientes.
- [Monitor de recursos](reference/resource-monitor.md): contrato de CPU,
  memoria disponible y estado explícito de la capacidad GPU.
- [Evidencia visual del release](reference/release-evidence.md): sumario,
  baseline, ownership y procedimiento para aceptar cambios.
- [Revisión legal y distribución](reference/legal-distribution-review.md): MIT,
  notices, privacidad local, rotación de claves y flujo de updater firmado.
- [Gobierno del repositorio](reference/repository-governance.md) y
  [auditoría de dependencias](reference/dependency-audit.md).
- [Inventario IPC](reference/ipc-inventory.json) y [revisión legal de distribución](reference/legal-distribution-review.md).
- [Política de fixtures](reference/fixtures-policy.md).
- [Red, privacidad y telemetría](reference/network-privacy.md).

### Explicación

- [Arquitectura local-first](explanation/local-first-architecture.md): por qué
  Rust conserva autoridad sobre datos y filesystem.

## Decisiones y operación

- [ADR-0001: contratos del repositorio](adr/0001-contratos-del-repositorio.md)
  explica licencia, plataforma inicial y fronteras técnicas.
- [Índice de ADRs](adr/README.md) para registrar decisiones duraderas.
- [CHANGELOG](../CHANGELOG.md) para cambios visibles por versión.
- [Gobierno del repositorio](reference/repository-governance.md),
  [auditoría de dependencias](reference/dependency-audit.md) y
  [política de fixtures](reference/fixtures-policy.md) reúnen las reglas
  operativas y sus evidencias.

Para empezar a trabajar, lee primero el [README](../README.md), luego
[`CONTEXTO.md`](../CONTEXTO.md) y finalmente
[`CONTRIBUTING.md`](../CONTRIBUTING.md).
