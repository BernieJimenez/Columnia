# Glosario

Términos internos que aparecen en la documentación, el roadmap y el código.

| Término | Significado |
| --- | --- |
| Cargar, Revisar, Preparar, Explorar, Entregar | Las cinco etapas de la aplicación, en ese orden. Se escriben con mayúscula cuando nombran la etapa. |
| Dataset activo | El conjunto de datos abierto en la sesión. Columnia trabaja sobre una copia: el archivo original nunca cambia. |
| Proyecto | Dataset preparado, con su historial, receta y reglas, guardado en la carpeta de datos (ver [respaldar proyectos](../how-to/respaldar-proyectos.md)). |
| Receta | Lista ordenada de transformaciones de Preparar que se puede guardar y volver a aplicar a otro archivo con el mismo esquema. |
| Tarea reutilizable | Receta, reglas de calidad y formato de entrega guardados juntos para repetir el mismo trabajo. |
| Reglas de calidad / contrato | Condiciones que la copia entregada debe cumplir (por ejemplo, ninguna fila sin importe). Entregar valida antes de exportar. |
| Source-backed | Modo para CSV, TSV, TXT delimitado y Parquet de al menos 512 MiB: el dataset activo guarda solo el esquema y la primera página, y las operaciones leen el archivo de origen desde disco. |
| Snapshot | Copia Parquet de un estado del dataset. El historial de Preparar y cada versión de un proyecto guardan una. |
| Gate | Comprobación automática que debe pasar antes de integrar o publicar (por ejemplo, `npm run docs:check` o `npm run ipc:check`). `tools/check.ps1` los agrupa por perfiles. |
| Tier | Una verificación completa y reproducible: tests, build, accesibilidad, benchmark, smokes y gates finales (`npm run verify:tier`). «Tier 10» nombra también la reauditoría del 2026-09-23, que se cerró con esa verificación. |
| Smoke | Prueba corta de extremo a extremo con la aplicación real (escritorio, instalador, CLI). |
| Probe / sonda | Script que conduce la aplicación real mediante WebView2 para medir o capturar evidencia. Usa la carpeta de datos real, protegida con una copia de seguridad. |
| Evidencia | Resultados de una verificación guardados en `.local/validation/` (fuera de git), cada uno con una cabecera común: commit, árbol, versión y horas. |
| RVnn | Objetivo del roadmap («revisión», por ejemplo RV113). Agrupa hallazgos y tiene un criterio de cierre. RV07 (beta), RV09 (accesibilidad nativa) y RV11 (distribución) están aparcados mientras Columnia sea de uso personal. |
| Códigos de hallazgo | Identificadores de una auditoría por área: `FUN` funcional, `PROD` producto, `COD` código, `SEG` seguridad, `DAT` datos, `REN` rendimiento, `ACC` accesibilidad, `UX` interfaz, `ARQ` arquitectura, `QA` pruebas, `DOC` documentación, `OPS` operación, `LEG` legal, `LIM` limpieza. Un mismo código puede repetirse en auditorías de fechas distintas. |
| T10-xx | Hallazgo de la reauditoría Tier 10 (2026-09-23). Se cita en el modelo de amenazas para explicar de dónde viene cada control. |
| Fase I0, I1… | Fases del plan inicial del proyecto (agosto de 2026): I0 fijó los contratos del repositorio e I1 los planes lazy de las recetas. Hoy solo aparecen en documentos históricos y en ADR. |
| «sistema anterior» | Nombre retirado del proyecto antes de llamarse Columnia (ver la versión 0.114.0 del CHANGELOG). Se conserva así en las entradas antiguas. |
| ADR | Registro de una decisión de arquitectura, en `docs/adr/`. |
| ODBC | Interfaz de Windows con la que Columnia entrega a PostgreSQL, MySQL y SQL Server mediante el controlador instalado. |
