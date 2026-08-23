# Checklist manual de accesibilidad

Esta lista complementa `npm run accessibility:visual` y `npm run accessibility:check`.
Las capturas automatizadas verifican estructura, foco, targets y `forced-colors`, pero
no sustituyen la prueba con una tecnología de asistencia real.

## Preparación

- [ ] Ejecutar `npm run tauri dev` desde una compilación limpia.
- [ ] Registrar versión de Columnia, Windows, navegador/WebView2 y lector usado.
- [ ] Abrir un dataset pequeño y conservar una evidencia sin datos sensibles.
- [ ] Repetir la sesión con zoom del sistema al 125% y con Windows High Contrast.

## Teclado y foco

- [ ] `Tab` llega al enlace «Saltar al contenido» y el foco siempre es visible.
- [ ] El enlace de salto mueve el foco al contenido principal.
- [ ] Las pestañas exponen nombre, selección y navegación coherentes.
- [ ] Los formularios y acciones de Preparar, Historial y Proyectos tienen una
      etiqueta anunciable y un orden de foco lógico.
- [ ] El diálogo destructivo atrapa el foco, anuncia su título y devuelve el foco
      al control que lo abrió después de cancelar o confirmar.
- [ ] `Escape` cierra diálogos no destructivos sin perder el estado de la pantalla.

## Lector de pantalla

Elegir NVDA o Narrador y activar la navegación por elementos/encabezados.

- [ ] Se anuncian los landmarks `main`, `nav` y `aside` con nombres útiles.
- [ ] El estado de carga/progreso se anuncia una sola vez y cambia a completado.
- [ ] Los errores de validación se anuncian y quedan asociados al campo afectado.
- [ ] Los botones de abrir, guardar, exportar y eliminar anuncian su acción y estado.
- [ ] La tabla/preview anuncia encabezados, fila/columna y mensajes de ausencia de datos.
- [ ] No se anuncian rutas locales, identificadores internos ni datos fuera de la vista.

## Contraste, zoom y movimiento

- [ ] Con High Contrast/`forced-colors: active`, texto, bordes, foco y estados
      seleccionados siguen distinguiéndose.
- [ ] A 125% no aparece scroll horizontal accidental ni se pierden acciones.
- [ ] Con `prefers-reduced-motion`, no hay animación imprescindible ni parpadeo.
- [ ] Los targets interactivos principales conservan al menos 24×24 CSS px.

## Evidencia y resultado

| Fecha/hora | Plataforma | Lector | Escenario | Resultado | Evidencia |
| --- | --- | --- | --- | --- | --- |
|  |  |  | teclado | ☐ pasa ☐ falla |  |
|  |  |  | lector de pantalla | ☐ pasa ☐ falla |  |
|  |  |  | High Contrast/125% | ☐ pasa ☐ falla |  |

Para cada fallo anotar pasos de reproducción, anuncio esperado/observado, captura
sin datos sensibles y prioridad. Esta checklist no marca la auditoría como completa
hasta que una persona haya ejecutado las casillas con hardware y asistencia reales.
