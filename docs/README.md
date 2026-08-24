# Documentación de Columnia

Esta carpeta contiene las decisiones y contratos de trabajo que no caben en la
guía funcional. El código y sus pruebas siguen siendo la fuente de verdad del
comportamiento implementado.

## Índice Diátaxis

### Tutorial

- [Primer dataset confiable](tutorials/first-dataset.md): recorrido desde la
  instalación hasta Cargar → Revisar con una fixture sintética.

### How-to

- [Validar evidencia del release](how-to/validate-release-evidence.md): compilar
  el binario, capturar ventanas y comprobar el baseline.

### Referencia

- [Referencia de la CLI](reference/cli.md): comandos, opciones, contratos JSON y
  códigos de salida.
- [Paridad funcional](reference/feature-parity.md): capacidades migradas desde
  `dataprepv1.1`, compatibilidad de contratos de calidad y brechas pendientes.
- [Evidencia visual del release](reference/release-evidence.md): sumario,
  baseline, ownership y procedimiento para aceptar cambios.
- [Gobierno del repositorio](reference/repository-governance.md) y
  [auditoría de dependencias](reference/dependency-audit.md).
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
