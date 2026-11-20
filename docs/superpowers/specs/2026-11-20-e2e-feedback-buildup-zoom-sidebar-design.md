# Feedback del e2e: Build-up a medio tiempo, zoom de Highs, umbral riser y scroll de la barra lateral — diseño

Fecha: 2026-11-20. Cambia en lo que toca a §3 (tabla Build-up) de
`2026-11-14-generated-sequences-palettes-fades-design.md` y a §2-§4 de `2026-11-15-detection-zoom-knobs-design.md`.

## 1. Build-up: un cambio cada medio tiempo como mínimo

El usuario quiere Build-ups más rápidos: **todo paso de una secuencia generada de Build-up dura ½ tiempo**. No se baja
de ½ (a ¼ de tiempo, a 140 BPM serían 107 ms y la tira necesita ~125 ms entre mensajes). Las secuencias que solo eran
«accelerating» y a ½ quedan iguales a otra se quitan (decisión del usuario). Quedan **8**:

| Id | Nombre | Pasos (todo a ½) |
|---|---|---|
| B1 | Strobe | todo W, – |
| B5 | Converge | E W, Cn P, todo C, – |
| B8 | Colour strobe | todo W, todo P |
| B9 | Odd / even strobe | impares W, pares W (el resto apagado) |
| B10 | Converge, then strobe | B5 dos veces, luego (todo W, –) ×4 |
| B11 | **Halves** (antes «Halves, accelerating») | H1 W + H2 P, H1 P + H2 W |
| B14 | **Odd / even, then strobe** (antes «Odd / even, accelerating») | (impares W + pares P, al revés) ×2, luego (todo W, –) ×2 |
| B19 | **Four-part chase** (antes «Four-part chase, accelerating») | p1, p2, p3, p4 en P, luego p1..p4 en W |

- Se quitan B15 «Strobe, accelerating» (= B1) y B16 «Converge, accelerating» (= B5).
- Bibliotecas guardadas: un set que usaba una secuencia quitada pasa a usar su igual (B15 → Strobe, B16 → Converge);
  las renombradas se leen con su nombre antiguo. Sin migración de fichero: alias al leer.
- Las secuencias de Build-up que crea el usuario no cambian.

## 2. Detection

- **Riser:** el mínimo baja de 3 a **1 dB** (rango 1–15 dB) en Rust (validación) y en la UI (línea y knob).
- **Centrado de la vista con zoom:** la ventana se centra en el **punto medio entre la referencia y los umbrales de la
  banda** (el centro del intervalo que va del más alto al más bajo de: referencia, cada línea de umbral), recortada a
  −60…0 dB. Así, con riser 3 dB y calm 10 dB, a ×4 se ven a la vez la señal, la línea Build-up y la línea Calm.
  Sub (sin umbrales) sigue centrado en la referencia. Fit usa el mismo centro.
- **La vista no se mueve mientras se arrastra** una línea o su etiqueta (si no, la línea se alejaría del puntero):
  el centro se congela al empezar el arrastre y se recoloca al soltar. Con el knob o las flechas sí se recoloca.
- **Zoom inicial:** Highs abre en **×4**; Kick y Sub en ×1. Sigue sin guardarse.

## 3. Barra lateral de Sets, Sequences y Palettes

La lista de la izquierda tiene **su propio scroll**: al bajar por la lista, el contenido de la derecha (por ejemplo la
simulación de la secuencia) se queda a la vista, y el contenido tiene su propio scroll si no cabe. Igual en las tres
pestañas.

## 4. Comprobaciones

- light-core: todo paso de toda secuencia generada de Build-up es `Half`; las tablas nuevas; los nombres antiguos se
  leen como los nuevos (`strobeAccelerating` → Strobe, `convergeAccelerating` → Converge, `halvesAccelerating` →
  Halves, …).
- Rust: riser 1.0 se acepta, 0.9 no.
- UI: centro con umbrales; la vista no se mueve durante un arrastre; Highs empieza en ×4; knob riser llega a 1.0;
  la barra lateral y el contenido tienen scroll propio (clases) y se comprueba con captura en modo mock.
- e2e del usuario: Build-ups en la tira a ~140 BPM; Highs con zoom ×4 en música real.
