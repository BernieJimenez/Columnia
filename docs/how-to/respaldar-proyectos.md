# Respaldar y restaurar proyectos

Columnia guarda los proyectos, las tareas y los presets en tu perfil de
Windows, en la carpeta de datos:

```text
%APPDATA%\app.columnia.desktop
```

Escribe esa ruta en la barra de direcciones del Explorador de archivos para
abrirla. La carpeta solo existe en este equipo: Columnia no sincroniza nada.

## Qué contiene

| Elemento | Qué es | ¿Respaldar? |
| --- | --- | --- |
| `projects.sqlite3` | Catálogo de proyectos: nombre, versiones, receta, reglas y ajustes. | Sí |
| `project-snapshots\` | Copia del dataset de cada versión de proyecto. Sin ella, el catálogo no puede abrir los datos. | Sí, junto con el catálogo |
| `reusable-tasks.sqlite3` | Tareas reutilizables. | Sí |
| `delivery-presets.sqlite3` | Presets de entrega. | Sí |
| `remote-deliveries.json` | Registro de entregas a bases remotas que evita duplicar filas al repetir. Solo guarda huellas, nunca datos ni credenciales. | Sí, si entregas a bases remotas |
| `projects.sqlite3.v<N>.bak` | Copia automática del catálogo antes de migrarlo a una versión nueva. | Opcional |
| `*.unreadable-<fecha>` | Catálogo de tareas o de presets que no se pudo abrir y se apartó al arrancar. | Consérvalo hasta saber qué pasó |
| `crash-reports\` | Informes de fallo sin datos. | No hace falta |
| `examples\` | Datasets de ejemplo; Columnia los vuelve a crear. | No |
| `session.active`, `session.started` | Marcas de la sesión en curso. | No |

## Hacer una copia

1. Cierra Columnia. Con la aplicación abierta, la copia de los catálogos
   SQLite puede quedar a medias.
2. Copia la carpeta `app.columnia.desktop` entera a otro disco, a una memoria
   USB o a una carpeta que respalde tu sistema. Es lo más sencillo y se lleva
   todo lo de la tabla.
3. Guarda también los archivos originales que cargaste si quieres repetir el
   trabajo desde cero: un proyecto conserva el dataset preparado, no el
   archivo de origen.

La copia contiene tus datos tal como están en los proyectos. Guárdala donde
guardarías los archivos originales.

## Restaurar

1. Cierra Columnia.
2. Si hay una carpeta `app.columnia.desktop` actual, renómbrala (por ejemplo,
   `app.columnia.desktop.antes`) en lugar de borrarla.
3. Copia la carpeta respaldada a `%APPDATA%` con el nombre
   `app.columnia.desktop`.
4. Abre Columnia y entra en **Continuar un proyecto**. Los proyectos deben
   aparecer con sus versiones. Abre uno para comprobarlo.
5. Cuando todo esté bien, borra la carpeta renombrada del paso 2.

Una copia de una versión anterior de Columnia también sirve: al abrirla, la
versión actual migra el catálogo y deja antes un `projects.sqlite3.v<N>.bak`.

## Después de un cierre inesperado

Si Windows se apagó o Columnia se cerró de golpe, al volver a abrirla Cargar
avisa: «La sesión anterior se cerró de forma inesperada».

- Los cambios guardados en un proyecto están a salvo. Ábrelo desde
  **Continuar un proyecto**.
- Los cambios posteriores al último guardado no se conservaron. Para no
  perderlos la próxima vez, guarda el proyecto al terminar cada etapa.
- Si Cargar avisa de que se apartó un catálogo que no se podía abrir (de
  tareas o de presets; los proyectos no se tocan), Columnia empezó con uno
  nuevo y vacío, y el anterior sigue en la ruta que indica el aviso. No lo
  borres: restaura una copia o guárdalo para revisarlo.
