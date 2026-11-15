# Detection: zoom vertical, escala en dB y knobs — diseño

Fecha: 2026-11-15. Sustituye en lo que toca a §5.5 de `2026-11-09-redesign-and-e2e-fixes-design.md` y a la nota de
Detection de su §5.6 (los indicadores de dB en una columna a la derecha). Maqueta aprobada por el usuario:
`.superpowers/brainstorm/detection-proposals-3.html` (diseño B, con la columna de knobs estrecha).

## 1. Por qué

En el e2e los carriles se ven bien, pero ajustar es difícil: en Highs las variaciones son de pocos dB y el carril
muestra 60 dB de alto; mover la línea discontinua para cambiar un umbral es poco preciso. El usuario quiere zoom
vertical por banda, poder arrastrar también la etiqueta verde y knobs para ajustar con precisión, con un ajuste fino con
Shift como en Ableton.

## 2. Distribución de cada carril (Kick, Sub, Highs)

De izquierda a derecha:

1. **Cabecera** de la banda (nombre y descripción), como hoy.
2. **Columna de zoom**, fuera de la gráfica: una pila vertical `+` / `×N` / `−` / `Fit`. En la fila de cabecera de la
   línea de tiempo esta columna dice «Zoom».
3. **Forma de onda**, que gana el ancho que pierde la columna derecha:
   - **escala de dB dentro de la gráfica, a la izquierda**, con una marca cada 10, 5, 2 o 1 dB según el zoom (con
     el zoom actual: 60 → 10, 30 → 5, 15 → 2, 8 → 1). Marcas tenues que cruzan el carril;
   - señal (blanco) y referencia (gris), como hoy;
   - **líneas discontinuas** de los umbrales, como hoy;
   - **etiqueta verde** de cada umbral («Calm when 14.3 dB below») **sobre su línea, en el extremo izquierdo**, justo
     después de los números de la escala. Se puede arrastrar igual que la línea.
4. **Columna de knobs**, estrecha (unos 7.5rem): un knob por umbral (Highs tiene dos, uno al lado del otro), con el
   texto en tres líneas debajo: el momento, el valor y «above»/«below» (p. ej. «Build-up / 7.0 dB / above»).

Los carriles pasan de 80 px a **96 px** de alto para que quepan la pila de zoom y el knob con su texto.

## 3. Zoom vertical

- Por banda, independiente; **no se guarda** (cada vez que se abre la pestaña empieza en ×1).
- Niveles: **×1, ×2, ×4, ×8** = 60, 30, 15 y 7.5 dB de alto (×1 es la vista de hoy, de −60 a 0 dB).
- La ventana se **centra en la referencia** de la banda (la línea gris, la del último beat), recortada para no salirse
  de −60…0 dB. Así los umbrales, que cuelgan de la referencia, siguen a la vista.
- `+` y `−` se desactivan en los extremos. **Fit** elige el mayor zoom en el que se ven toda la señal de los últimos
  32 beats y todos los umbrales de la banda, con un pequeño margen.
- Un umbral que cae **fuera de la vista** no dibuja su línea; su etiqueta se queda pegada al borde con ▲ o ▼ y borde
  discontinuo. Al pulsarla se reduce el zoom hasta que el umbral se ve.

## 4. Formas de cambiar un umbral

Las tres cambian el mismo valor:

- **Línea discontinua** y **etiqueta verde:** arrastrar en vertical. El valor es la posición del puntero, redondeado a
  **1 dB**, o a **0.1 dB con Shift** pulsado.
- **Knob:** arrastrar arriba/abajo, rueda del ratón o flechas: **1 dB por paso**; **con Shift, 0.1 dB por paso**. Al
  pulsar o soltar Shift a mitad de arrastre, el knob sigue desde donde está, sin saltar. Doble clic vuelve al valor por
  defecto. Arrastrando, unos 6 px por paso. El arco del knob muestra el valor dentro de su rango.
- Los rangos no cambian: kick 8–30, highs 6–24, riser 3–15 dB (antes del desplazamiento por sensibilidad, como hoy:
  lo que se ve y se ajusta es el valor ya desplazado).
- Accesibilidad: el knob es un `role="slider"` con nombre, valor y texto en palabras; las líneas, igual que hoy.
- **Ayuda (globos «?»):** el knob dice «Drag up or down, scroll, or use the arrow keys: 1 dB per step. Hold Shift for
  0.1 dB steps. Double-click resets it to <default> dB.» precedido de lo que hace ese umbral (los textos actuales
  `thresholdKick`, `thresholdRiser`, `thresholdHigh`). Los botones de zoom tienen su ayuda: «Zoom in on this band»,
  «Zoom out», «Zoom to the closest view that shows the music and every threshold». La línea y la etiqueta mencionan
  Shift.
- Los valores se muestran **con un decimal** («14.3 dB») en etiquetas y knobs.

## 5. Umbrales con decimales

- El detector (`light-core`) ya trabaja con `f32`. Lo que pasa a admitir décimas es lo guardado y la IPC:
  `Settings`, `DetectorConfig` y el `Thresholds` de `set_detection_thresholds` pasan de `u8` a **`f32`**.
- Validación (en Rust, en la frontera): finito, dentro del rango, y se **redondea a 0.1 dB** al guardar. Fuera de
  rango → error, como hoy.
- Ficheros existentes: los enteros de hoy se leen tal cual como `f32`; no hace falta migración.
- `Settings` y `DetectorConfig` dejan de ser `Eq` (siguen siendo `PartialEq`).
- El envío al detector no cambia: sigue con el retardo de 300 ms y aplica en el siguiente beat.

## 6. Fuera de alcance

- Guardar el zoom, zoom horizontal, cambiar los rangos de los umbrales, cambiar el carril Moment.

## 7. Comprobaciones

- Rust: guardar y leer décimas; redondeo a 0.1; rechazo de fuera de rango y no finitos; un fichero con enteros se lee
  igual.
- UI (Vitest, api mockeada): zoom +/−/Fit cambia la escala y se desactiva en los extremos; un umbral fuera de vista
  pega su etiqueta al borde y al pulsarla se reduce el zoom; knob con flechas (1 dB) y Shift+flechas (0.1 dB), rueda,
  doble clic al valor por defecto; arrastrar la etiqueta cambia el valor; cada control tiene ayuda (test de
  cobertura de ayuda).
- e2e del usuario: ajustar Highs con zoom y Shift en música real.
