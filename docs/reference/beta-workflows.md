# Recorridos candidatos para la beta

Estos dos casos sintéticos hacen reproducibles las hipótesis de uso de la auditoría. No demuestran que esos flujos sean prioritarios: esa decisión requiere observar si las personas beta los reconocen como trabajo real y qué tareas actuales reemplazan. Las fixtures sirven de calentamiento y no cuentan como datasets reales para Gate 1. Ejecuta `npm run beta:workflows:check` para comprobar que sus resultados esperados sigan alineados con los archivos de ejemplo.

## Preparar un reporte periódico para BI

Entrada: [`monthly-sales.csv`](../../fixtures/beta/monthly-sales.csv).

Objetivo para la persona: excluir ventas canceladas, crear `revenue = units × unit_price`, conservar el código de cliente y exportar el resultado.

Resultado esperado: 3 filas, suma de `revenue` igual a 15.60 y códigos `00124`, `00007` y `00042` intactos. El archivo original debe seguir igual. La tarea cuenta como completada si la persona llega al resultado sin ayuda, valida las tres condiciones y abre la exportación fuera de Columnia.

Métricas: sin/con ayuda, tiempo, retrocesos, valores reintroducidos, observación de ceros iniciales y verificación externa de la entrega.

## Comparar dos versiones de inventario

Entradas: [`inventory-before.csv`](../../fixtures/beta/inventory-before.csv) y [`inventory-after.csv`](../../fixtures/beta/inventory-after.csv).

Objetivo para la persona: comparar por `sku` y explicar qué permanece igual, cambió, se agregó y se retiró.

Resultado esperado: `B-02` permanece igual; `A-01` cambió; `D-04` se agregó; `C-03` se retiró. Hay 1 clave coincidente, 1 conflicto, 1 clave solo actual y 1 clave solo anterior. No debe modificarse ninguno de los dos archivos fuente.

Métricas: éxito sin ayuda, tiempo para identificar las cuatro categorías, errores de interpretación y si la persona sabe continuar hacia la consolidación o exportación prevista.

## Cómo incorporarlos en sesiones reales

La persona facilitadora no demuestra los pasos ni entrega las respuestas esperadas. Primero pide a la persona que describa un caso real equivalente. Si no existe, registra que el recorrido no aplica; no lo cuentes como evidencia de demanda. Si existe, usa sus datos localmente y conserva en el reporte solo el alias del caso, el formato, rangos de tamaño y métricas sanitizadas. Nunca copies esos datos reales al repositorio.
