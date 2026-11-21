# Build-up antes y vista de depuración del detector — diseño

Fecha: 2026-11-21. Cambia en lo que toca a §6.2 de la spec principal (reglas de Calma → Subidón) y a Detection
(`2026-11-15-detection-zoom-knobs-design.md`, `2026-11-20-e2e-feedback-buildup-zoom-sidebar-design.md` §2).

## 1. Por qué

En el e2e el detector se quedó en Calm durante 7 compases en los que el usuario esperaba Build-up: los agudos
estaban altos (~5 dB sobre su referencia) pero planos. Hoy, desde Calm, el Build-up necesita:

- **riser:** las medias por compás de los agudos de los últimos 4 compases (3 en la segunda mitad de la frase) suben
  sin bajar más de 0.5 dB, en total al menos el umbral del knob (`riser_db`), acabando como mucho 6 dB bajo la
  referencia;
- **approach:** tras 8 compases de Calm, la media de 4 beats de los agudos está al menos 2 dB sobre la referencia y
  ha subido al menos 2 dB respecto a 8 compases antes.

El usuario quiere detectar el Build-up antes y poder ver por qué el detector decide lo que decide.

## 2. Reglas nuevas (decisión del usuario: las tres, medidas con las canciones de prueba)

1. **Approach a 4 compases:** se puede disparar tras **4** compases de Calm (antes 8) y compara con **4** compases
   antes (antes 8). La subida necesaria (hoy 2 dB fijos) se elige midiendo (§3). *Aclaración:* esta subida **no** es el
   knob de Detection; el knob es la subida del riser. Si la medición muestra que conviene, se puede atar al knob.
2. **Riser más tolerante:**
   - una media de compás puede quedar hasta **`riser_tol_db`** por debajo del máximo anterior (hoy 0.5 dB fijos; se
     mide 1.0 y 1.5);
   - **3 compases** que suben bastan en cualquier punto de la frase (hoy solo en la segunda mitad);
   - el knob del riser va de 2 a 15 dB (§8).
3. Regla de nivel: descartada por el usuario tras la medición (2026-11-21); no se implementa.

## 3. Medición

Con `just detect-probe` sobre `recordings/songs/song-1..5` (git-ignored, con etiquetas), igual que en
`.superpowers/plan9-measure.md`. Para medir sin recompilar, `detect-probe` acepta `--set clave=valor` para los
parámetros nuevos (`approach_bars`, `approach_calm_beats`, `approach_rise_db`, `riser_tol_db`, `riser_any_three`).
Se comparan: la línea base, cada regla sola y combinaciones; aciertos, fallos, falsas alarmas y
retraso medio del Build-up. Los valores por defecto se eligen con el usuario a la vista de la tabla; la tabla se guarda
en `.superpowers/plan16-measure.md`.

## 4. Vista de depuración (solo para el desarrollador)

- **Interruptor de compilación:** `VITE_DETECTION_DEBUG=true` en `ui/.env`. El fichero está en `.gitignore`: en el Mac
  del usuario existe (build local con la vista), en GitHub no (build sin la vista). Sin la variable, o con otro valor,
  no se ve nada nuevo. No es un ajuste de la app ni aparece en Setup.
- **Datos:** cada evento del detector lleva además, siempre (son valores que ya calcula; solo viajan con Detection
  abierto):
  - la última media de compás de los agudos;
  - progreso del riser: compases que suben de los necesarios, subida actual y necesaria;
  - progreso del approach: compases en Calm de los necesarios, agudos sobre la referencia, subida y necesaria;
  - la **causa** del último cambio de momento (riser, approach, silencio, teaser, groove, drop por bombo,
    drop por hats, vuelta lenta, colapso, tiempo máximo, break suave, inicio de frase, deshacer).
- **En pantalla (solo con el interruptor):**
  - en el carril Moment, cada tramo dice su causa («Build-up · riser 3.4 dB / 4 bars»);
  - un panel «Next moment» bajo la línea de tiempo con el progreso de cada regla hacia el siguiente momento, en vivo
    («Riser 2/4 bars, +1.2 of 3.0 dB · Approach 3/4 bars, +5.1 dB over ref ✓, rise 0.4 of 2.0 ✗»);
  - en Highs, las medias por compás dibujadas como escalones.

## 5. Fuera de alcance

Cambiar otras reglas (Calm, drop), mostrar la vista al usuario final, guardar las explicaciones.

## 6. Comprobaciones

- light-core: tests de cada regla (approach a 4 compases, tolerancia, 3 compases en cualquier punto), progreso y causa expuestos.
- detect-probe: `--set` acepta las claves y rechaza las desconocidas.
- UI: sin la variable no aparece nada; con ella (en test se fuerza), panel, causas y escalones.
- Medición (§3) y e2e del usuario con música real.

## 7. Valores elegidos (medición 2026-11-21)

- **Approach:** 8 compases y 2 dB (a 4 compases añadía 3 Build-up falsos y retrasaba uno real 9 s).
- **Riser de 3 compases en cualquier punto:** activo.
- **Guarda del riser (descartada):** Se probó exigir que la subida no venga solo del último compás: en las canciones de prueba empeora (song-5 vuelve a +20.6 s y aparece un falso), así que no se aplica. Riesgo conocido: un redoble de 2 beats en un break puede pasar el riser de 3 compases a 5 dB.
- **Tolerancia del riser:** 0.5 dB (1.0 y 1.5 sin efecto).
- **Umbral del riser:** 5 dB por defecto (el más bajo sin falsas alarmas extra).
- **Regla de nivel:** medida y descartada (añadía 1 falso en song-2 y no acertaba ninguno); se eliminó del código.

## 8. La línea de Build-up mide una subida; knobs con solo su nombre (maqueta aprobada 2026-11-21)

Maqueta: `.superpowers/brainstorm/detection-proposals-4.html`.

- **Línea de Build-up (Highs):** ya no es «referencia + X dB». Se dibuja solo sobre los **últimos 4 compases**, a
  **inicio de la subida + X dB**. El inicio de la subida es el primer compás de la racha que no baja (lo que el
  detector ya envía: `highBarDb − riserRiseDb`); se marca con una línea punteada «climb start» y una pequeña línea
  vertical a la derecha une ambas. Si los agudos llegan a la línea discontinua mientras suben, entra Build-up. Se
  mueve con la música y se arrastra para cambiar X (el valor es la distancia al inicio de la subida). Sin compás
  cerrado todavía, la línea y su etiqueta no se dibujan (el knob sigue funcionando).
- **Etiqueta:** «Build-up when the highs climb 5.0 dB in 3–4 bars».
- **Knob «Highs climb»:** rango **2–15 dB** (antes 1–15), por defecto 5. Validación en Rust igual.
- **Knobs:** bajo cada knob solo su nombre: «Kick drop», «Highs climb», «Highs drop». El valor va en la etiqueta de su
  línea y en el tooltip.
- **Tooltips** (en el knob y en su línea/etiqueta), con el valor actual en la primera línea:
  - Kick drop: «A beat counts as quiet when the kick falls more than X dB below its usual level. Two quiet beats in a
    row during Rhythm switch to Calm. Lower: Calm comes in more easily.»
  - Highs drop: «A beat also counts as quiet when the hi-hats fall more than X dB below their usual level while the
    kick is already weaker. It covers breaks where the hats go but some kick remains. Lower: Calm comes in more
    easily.»
  - Highs climb: «From Calm, Build-up comes in when the hi-hats climb at least X dB over 3–4 bars, bar after bar. It
    is a climb, not a level. Lower: earlier Build-ups, but more false ones.»
  - Los dos de Calm añaden: «The Moment changes slider shifts this value: each step towards Reactive lowers it by
    1 dB.»
  - En el knob, al final, cómo se usa (arrastrar, rueda, flechas, Shift 0.1 dB, doble clic al valor por defecto); en
    la línea, «Drag to change. Hold Shift for 0.1 dB steps.»
