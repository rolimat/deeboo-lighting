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
   - el knob ya baja a 1 dB.
3. **Regla de nivel (experimental):** en la segunda mitad de la frase, un compás entero con la media de los agudos al
   menos `riser_db` sobre la referencia → Build-up. Riesgo conocido: falsos Build-up en breaks con hi-hats
   constantes. Va detrás de un interruptor (`PositionRules.level_buildup`) y **solo se activa por defecto si la
   medición no empeora las falsas alarmas**.

## 3. Medición

Con `just detect-probe` sobre `recordings/songs/song-1..5` (git-ignored, con etiquetas), igual que en
`.superpowers/plan9-measure.md`. Para medir sin recompilar, `detect-probe` acepta `--set clave=valor` para los
parámetros nuevos (`approach_bars`, `approach_calm_beats`, `approach_rise_db`, `riser_tol_db`, `riser_bars_any`,
`level_buildup`). Se comparan: la línea base, cada regla sola y combinaciones; aciertos, fallos, falsas alarmas y
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
  - progreso de la regla de nivel (si está activa);
  - la **causa** del último cambio de momento (riser, approach, nivel, silencio, teaser, groove, drop por bombo,
    drop por hats, vuelta lenta, colapso, tiempo máximo, break suave, inicio de frase, deshacer).
- **En pantalla (solo con el interruptor):**
  - en el carril Moment, cada tramo dice su causa («Build-up · riser 3.4 dB / 4 bars»);
  - un panel «Next moment» bajo la línea de tiempo con el progreso de cada regla hacia el siguiente momento, en vivo
    («Riser 2/4 bars, +1.2 of 3.0 dB · Approach 3/4 bars, +5.1 dB over ref ✓, rise 0.4 of 2.0 ✗ · Level …»);
  - en Highs, las medias por compás dibujadas como escalones.

## 5. Fuera de alcance

Cambiar otras reglas (Calm, drop), mostrar la vista al usuario final, guardar las explicaciones.

## 6. Comprobaciones

- light-core: tests de cada regla (approach a 4 compases, tolerancia, 3 compases en cualquier punto, regla de nivel
  con y sin interruptor, solo en la segunda mitad de la frase), progreso y causa expuestos.
- detect-probe: `--set` acepta las claves y rechaza las desconocidas.
- UI: sin la variable no aparece nada; con ella (en test se fuerza), panel, causas y escalones.
- Medición (§3) y e2e del usuario con música real.
