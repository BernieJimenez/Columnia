# Columnia

Columnia será una estación local multiplataforma para revisar, limpiar,
transformar y entregar datasets confiables.

El proyecto está en su primer hito técnico. Actualmente contiene el shell Tauri
2, una interfaz React/TypeScript y un contrato mínimo entre ambos lados. El motor
de datos todavía no está implementado.

## Plataformas objetivo

- Windows
- macOS
- Linux

El diseño evita APIs exclusivas de un sistema operativo. Sin embargo, cada
plataforma se considerará soportada únicamente después de compilar, instalar y
probar Columnia localmente en ese sistema.

## Requisitos de desarrollo

- Node.js LTS.
- Rust estable instalado mediante `rustup`.
- Dependencias nativas de Tauri para el sistema operativo correspondiente.

La máquina actual todavía no tiene Rust/Cargo instalado. Por eso el frontend se
puede validar, pero el shell nativo aún no se ha compilado.

## Desarrollo local

```powershell
npm install
npm run tauri dev
```

Validación rápida y completamente local:

```powershell
.\tools\check.ps1 -Profile Fast
```

La validación completa añade el build nativo sin empaquetar:

```powershell
.\tools\check.ps1 -Profile Full
```

No se utilizan CI, GitHub Actions ni workflows. Consulta [ROADMAP.md](ROADMAP.md)
para conocer las decisiones y fases previstas.

