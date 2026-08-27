# Monitor de recursos

El monitor lateral muestra una lectura local y efímera del proceso Columnia y
del equipo. No se envía telemetría ni se guardan muestras.

## Lecturas

- `CPU` es el uso del proceso y se expresa como hilos ocupados sobre los hilos
  lógicos detectados.
- `RAM` de Columnia es la memoria residente del proceso. Su barra usa esta
  misma magnitud, con la RAM total del equipo como referencia.
- `Equipo` es la memoria usada sobre la memoria total del sistema.
- `Disponible` es la memoria que `sysinfo` reporta como disponible. Windows
  puede exponer una semántica equivalente a memoria libre, por lo que debe
  interpretarse como una señal orientativa y no como un límite garantizado.
- `GPU` indica `No disponible` mientras Columnia no inicialice un backend o una
  sonda GPU. En ese estado el contrato devuelve `status: "unavailable"` y deja
  las métricas de uso y memoria en `null`; la interfaz nunca estima ese valor.

La lectura se solicita al backend de escritorio cada dos segundos. En el
navegador o cuando el backend falla, el panel conserva estados visibles de
`Solo escritorio` o `No disponible ahora`.

## Contrato IPC

`get_resource_usage` devuelve las métricas de CPU, memoria y el objeto `gpu`.
`systemMemoryAvailableBytes` y `gpu` son opcionales en TypeScript para que una
interfaz actualizada pueda convivir con un binario anterior durante una
actualización. Las futuras sondas GPU deben mantener el estado `unavailable`
cuando el sistema no exponga una lectura confiable.

