# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Aplicación de escritorio con Tauri 2: la interfaz es React y TypeScript dentro
de WebView2. Solo Windows 11 x64 está validado; Windows 10, ARM64, macOS y
Linux no.

## Users

Analistas de datos con oficio (SQL, Excel, BI) que reciben archivos
desordenados y tienen que dejarlos listos para un análisis, para Power BI o
para una base de datos. Hoy la usa solo su autor, pero se diseña para alguien
como él que no lo conoce: la interfaz tiene que explicarse sola, sin dar por
sabido nada de Columnia y sin rebajar el vocabulario del oficio (nulo, clave,
tipo, contrato).

## Product Purpose

Columnia convierte archivos desordenados en datasets confiables sin sacar los
datos del equipo, salvo cuando la persona los entrega a una base de datos. Es
una estación de trabajo local para revisar, limpiar, transformar, validar y
exportar datos. Funciona bien cuando la persona sabe en cada momento qué le
pasa a sus datos, qué cambió y cómo deshacerlo, y entrega una copia que cumple
las reglas que ella misma fijó.

## Positioning

Todo ocurre en el equipo: no hay cuenta, inicio de sesión, telemetría ni
sincronización remota. El trabajo sigue un flujo fijo de cinco etapas, Cargar,
Revisar, Preparar, Explorar y Entregar, y la entrega pasa por un contrato de
calidad antes de exportar.

## Operating Context

- Entradas: CSV, TSV, JSON, Parquet, Excel y ODS del equipo, o una tabla o
  vista de SQL Server, PostgreSQL o MySQL, siempre en solo lectura.
- Motor: Rust con Polars y DuckDB; la consulta SQL es local.
- Salidas: archivo, Power BI, SQL Server (por conexión, script T-SQL o paquete
  BULK INSERT) u otra base de datos.
- Los proyectos se guardan en local para continuar después.
- Archivos grandes: Columnia estima la RAM antes de cargar y lee por bloques
  los CSV, TSV, TXT, JSON, Parquet y Excel (.xlsx, .xlsb) de 512 MiB o más.
- Existe también una CLI (`docs/reference/cli.md`).

## Capabilities and Constraints

Innegociables, confirmados por el autor el 2026-11-12:

- **Los datos no salen solos.** Sin cuenta, telemetría ni red; solo salen del
  equipo cuando la persona los entrega a una base de datos.
- **El original no se toca.** Se trabaja sobre una copia, todo cambio es
  reversible y las fuentes se leen en solo lectura.
- **Cifras de todas las filas.** Lo que se muestra se calcula sobre el dataset
  entero, nunca sobre una muestra sin decirlo.
- **Accesibilidad WCAG AA** en cada cambio (ver Accessibility & Inclusion).

Otros hechos:

- La interfaz está solo en español.
- La CSP incluye el hash del script en línea de `index.html`; no se añade
  lógica ahí.
- Proyecto personal: no hay instaladores oficiales ni updater, y no se aceptan
  pull requests externos.
- Aparcado, sin decidir: beta con participantes, prueba con lector de pantalla
  y distribución (ver `ROADMAP.md`).
- Los espacios Automatizar y Preparar para BI no tienen interfaz todavía; hoy
  solo existe Analizar.

Terminología fija: Cargar, Revisar, Preparar, Explorar, Entregar; «nulo» para
la celda sin valor; «Consulta SQL»; «contrato» para las reglas de aceptación.
El glosario está en `docs/reference/glosario.md`.

## Brand Commitments

- Nombre y logotipo «Columnia» (`app-icon.svg`) son fijos.
- Voz sobria y de utilidad: qué ocurrió, qué está disponible y qué acción
  sigue. Sin promesas, eslóganes ni texto sobre el propio diseño.
- El lenguaje visual vigente está en `DESIGN.md`.

## Evidence on Hand

- Capturas y recorrido animado con datos sintéticos en `docs/images/gallery/`.
- Tutorial y guías en `docs/`, modelo de amenazas en `THREAT_MODEL.md` y
  privacidad de red en `docs/reference/network-privacy.md`.
- No hay testimonios, clientes, cifras de uso ni comparativas de rendimiento:
  no se inventan.

## Product Principles

1. La persona siempre sabe qué le pasa a sus datos: cada acción dice su efecto
   antes de ejecutarse y deja cómo deshacerla.
2. Nada sale del equipo sin una decisión explícita y un resumen previo de lo
   que va a salir.
3. Una cifra en pantalla es de todas las filas, o dice claramente que no lo es.
4. Se habla el idioma del oficio, sin jerga interna de Columnia y sin
   condescendencia.
5. La accesibilidad es parte de terminar un cambio, no un repaso posterior.

## Accessibility & Inclusion

WCAG AA en todos los temas: teclado, lector de pantalla, zoom al 200 %, 320 CSS
px sin scroll horizontal global, `forced-colors` y `prefers-reduced-motion`. El
color nunca es la única señal de estado. La prueba con un lector de pantalla
real está aparcada.
