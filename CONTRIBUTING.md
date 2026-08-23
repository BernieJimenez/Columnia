# Contribuir a Columnia

Columnia se mantiene como un proyecto local-first: el trabajo con datasets no
requiere red, el repositorio no usa CI y los gates se ejecutan en la máquina de
desarrollo. Esta política hace explícito cómo proponer y validar cambios.

## Ramas

- `master` es la rama de integración estable.
- Usa ramas cortas con uno de estos prefijos: `feat/`, `fix/`, `docs/` o
  `chore/`, seguido de un identificador breve en minúsculas y separado por
  guiones.
- No mezcles refactorizaciones no relacionadas con el cambio que estás
  validando.
- Antes de integrar, revisa el diff, confirma que el árbol no contiene archivos
  ajenos y ejecuta el gate apropiado.

## Commits

Usa Conventional Commits en modo imperativo:

```text
feat: conserva tipos nativos al leer ODS
fix: rechaza destinos que son reparse points
docs: documenta la política de fixtures
```

El cuerpo del commit debe explicar el motivo cuando no sea evidente y debe
indicar la validación ejecutada. No incluyas rutas privadas, datasets reales,
PII, secretos, credenciales ni artefactos generados bajo `.local/`.

## Revisión local

Desde la raíz del repositorio:

```powershell
npm run governance:check
.\tools\check.ps1 -Profile Fast
```

Para cambios Rust, IPC, persistencia o seguridad ejecuta también:

```powershell
.\tools\check.ps1 -Profile Full
```

Para cambios de release o empaquetado ejecuta `Release` o `Package` según
corresponda. Los smokes de WebView2 y los recorridos que requieren un escritorio
interactivo se documentan junto con su evidencia; no se sustituyen por una
afirmación de que el gate pasó.

## Límites de alcance

- React recibe metadatos, filas de preview e identificadores opacos, nunca
  rutas locales ni autoridad sobre el filesystem.
- Rust conserva la autoridad sobre rutas, archivos, datasets y almacenamiento
  durable.
- Las fixtures deben ser sintéticas y deterministas. Consulta la
  [política de fixtures](docs/reference/fixtures-policy.md).
- No agregues telemetría, servicios remotos, CI, certificados de pago ni nuevas
  capacidades Tauri sin registrar primero una decisión duradera.

## Documentación

Las decisiones duraderas viven en [ADRs](docs/adr/0001-contratos-del-repositorio.md).
El inventario de dependencias y la cadencia de auditoría están en
[dependency-audit.md](docs/reference/dependency-audit.md). Si un cambio altera
arquitectura, contratos, límites, comandos o validación, actualiza también
`CONTEXTO.md` y `ROADMAP.md` en el mismo cambio.
