# Política de seguridad

## Versiones con soporte

Hasta que exista una primera distribución binaria pública, solo el código de la
rama predeterminada recibe correcciones de seguridad. No hay instaladores ni un
canal de actualizaciones oficialmente soportados.

## Reportar una vulnerabilidad

No publiques información sensible en un issue. Envía el reporte de forma privada
a [berniejimenez493@gmail.com](mailto:berniejimenez493@gmail.com) e incluye:

- una descripción del problema y su posible impacto;
- la versión o commit afectado;
- pasos mínimos para reproducirlo;
- cualquier mitigación temporal conocida.

Se acusará recibo cuando sea posible y se coordinará la divulgación después de
evaluar y corregir el problema. No incluyas datasets reales, credenciales, rutas
privadas ni otra información personal en el reporte.

## Alcance

Son especialmente relevantes los reportes sobre exposición de rutas o valores,
escape del contrato SQL de solo lectura, validación de archivos, exportaciones,
persistencia local, IPC de Tauri, dependencias y el futuro canal de actualización.

La investigación de buena fe debe usar datos sintéticos y evitar acceder, alterar
o destruir información de terceros.
