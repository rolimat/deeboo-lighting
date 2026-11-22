# Bandas de medios y densidad de golpes para el Build-up — diseño

Fecha: 2026-11-22. Amplía §6.2 de la spec principal (Calma → Subidón) y
`2026-11-21-earlier-buildup-and-debug-view-design.md` (riser y approach).

## 1. Por qué

Muchos build-ups son sobre todo un redoble de caja que acelera: negras, corcheas, semicorcheas. Ese redoble vive
en gran parte en los medios (150 Hz–4 kHz) y su número de golpes se duplica aunque el nivel apenas suba. Hoy el
detector solo mira bombo, sub y agudos (> 4 kHz), y su única prueba de Build-up por nivel es el riser de los agudos.
Con las canciones de prueba quedan:

- un Build-up sin detectar (song-1, 01:32);
- dos detectados tarde (song-1, 05:08, +7.4 s; song-2, 04:58, +7.9 s).

La idea viene de comparar con LedFx (que separa medios de agudos), pero el código es nuestro: LedFx y aubio son GPL-3.
El usuario quiere **probar** las dos formas de medir la densidad (§3) y decidir con la medición (§5).

## 2. Objetivos y límites

- **Objetivos:** tres bandas de medios; dos reglas nuevas de prueba de Build-up, cada una con su interruptor;
  `detect-probe --set` para medirlas sin recompilar; una tabla de medición; valores por defecto que elige el usuario.
- **Fuera de alcance:**
  - el estado de silencio: se queda como está (reloj parado y sin audio, lo de hoy; reloj en marcha y sin audio, Calm);
  - carriles de medios en la pantalla Detection;
  - el panel de depuración, salvo las palabras de las causas nuevas (si una regla se adopta, su progreso en el panel
    es una tarea posterior);
  - FFT.
- **Anti-objetivos (sería un fracaso aunque funcione):**
  - más falsas alarmas de Build-up que hoy (2 en songs 1–5);
  - con las dos reglas apagadas, cualquier cambio en lo que detecta hoy el detector;
  - asignaciones, locks o logs nuevos en el callback de audio.

## 3. Enfoques (decisión del usuario: 2 y 3)

1. **Riser de nivel en los medios** (la regla del riser aplicada a cada banda): descartado; no mide densidad, un
   redoble que acelera sin subir de nivel no se ve y una voz que crece sí.
2. **Energía de ataques por beat:** la suma de los saltos positivos de la envolvente por beat y banda, en dB contra
   su referencia. Sin umbral de golpe. Elegido.
3. **Contar golpes:** cada ataque se detecta con un umbral adaptativo y una separación mínima; se cuentan golpes por
   beat. Elegido.

Las dos reglas se implementan apagadas y se miden solas y juntas.

## 4. Diseño

### 4.1 Bandas

Filtros IIR como las bandas actuales (`filter.rs`): paso alto + paso bajo Butterworth de 4.º orden (2 + 2 biquads, como
`kick_band`).

| Banda | Rango | Qué capta |
| --- | --- | --- |
| Medios bajos | 150–400 Hz | cuerpo de caja y toms |
| Medios | 400–1500 Hz | voces, sintes |
| Medios altos | 1500–4000 Hz | chasquido de caja, palmas |

Por hop (≈ 5.8 ms, como hoy) y banda, `BandAnalyzer` da el RMS y el **ataque**: `max(0, ln(1 + 100·rms) − anterior)`,
la misma fórmula que el onset del bombo. `HopFrame` lleva los tres RMS y los tres ataques.

### 4.2 Regla A: energía de ataques (`mid_flux`)

- Por beat y banda: media de los ataques de sus hops (suma / hops, para que no dependa del tempo), en dB (`db()`,
  que ya pone un suelo a una banda en silencio). *Decisión del constructor.*
- Referencia por banda: mediana de los beats de groove, aprendida como las de hoy (en Rhythm, con las mismas
  condiciones de aprendizaje y el mismo tamaño de historial).
- Por compás: media de los 4 beats. Los compases se cierran exactamente como los del riser de agudos (alineados al
  reloj, mismo reinicio tras un hueco).
- **Prueba:** en alguna banda, los últimos 4 compases (3 con `riser_any_three`, como el riser) suben sin bajar más de
  `flux_tol_db` bajo el máximo anterior, en total al menos `flux_rise_db`, y el último compás está como mucho
  `flux_level_db` bajo su referencia.
- Causa: `Cause::Attacks`.

### 4.3 Regla B: densidad de golpes (`mid_hits`)

- **Golpe**, por hop y banda: el ataque supera `max(hit_k × media móvil, 0.05)` y han pasado al menos `hit_gap_ms`
  desde el golpe anterior de esa banda. La media móvil es exponencial (constante de ~1 s), sin asignar memoria,
  corre en todos los estados y se pone a cero con `reset()`. Un hop con el RMS de la banda por debajo de −60 dBFS no
  cuenta. *El suelo de 0.05 y que la media corra siempre son decisión del constructor.*
- Por beat y banda: número de golpes. Por compás: golpes por beat (media de los 4 beats).
- **Prueba:** en alguna banda, sobre los últimos `hit_bars` compases cerrados (el "primero" es el más antiguo de
  ellos; hacen falta `hit_bars` compases):
  - la densidad no baja de un compás al siguiente más de `hit_tol` golpes por beat;
  - el último compás tiene al menos `hit_ratio` veces la densidad del primero;
  - el último compás tiene al menos `hit_min_per_beat` golpes por beat.
  *La tolerancia `hit_tol` es decisión del constructor: los recuentos son ruidosos y sin ella 3–4 compases casi
  nunca valdrían.*
- Causa: `Cause::Hits`.

### 4.4 Cómo entran en el detector

Como el riser y el approach: son prueba de Build-up **solo desde Calm**, con la misma puerta (`min_state_beats` y el
refractario tras un colapso). Si en un beat hay varias pruebas, la causa sigue este orden: riser, approach, ataques,
golpes. Con `mid_flux` y `mid_hits` apagados, el detector se comporta exactamente como hoy.

### 4.5 Parámetros

Los interruptores `mid_flux` y `mid_hits` van en `PositionRules`, como `riser_any_three`: apagados en `ALL` y en
`NONE` hasta decidir, y con palabras nuevas en `--rules` (`flux`, `hits`). Los umbrales van en `DetectorParams`.
Todos se cambian con `--set` en `detect-probe`. *Decisión del constructor.* Valores iniciales para medir, no
definitivos:

| Clave | Inicial | Se mide |
| --- | --- | --- |
| `mid_flux` | `false` | `true` |
| `flux_rise_db` | 4.0 | 2, 3, 4, 6 |
| `flux_tol_db` | 1.0 | 0.5, 1.0 |
| `flux_level_db` | 6.0 | — |
| `mid_hits` | `false` | `true` |
| `hit_k` | 2.0 | 1.5, 2, 3 |
| `hit_gap_ms` | 40 | 40, 60 |
| `hit_bars` | 2 | 2, 3, 4 |
| `hit_ratio` | 2.0 | 1.5, 2 |
| `hit_min_per_beat` | 2.0 | 2, 3 |
| `hit_tol` | 0.5 | 0.5, 1.0 |

`MomentDetector::new` ya recibe la duración del hop: `hit_gap_ms` se pasa a hops ahí.

### 4.6 App y UI

- `src-tauri`: el audio pasa ya el `HopFrame` completo; nada más cambia. Los ajustes guardados no cambian.
- **Referencias aprendidas:** las de los medios no se exportan ni importan (`LearnedRefs` no cambia) mientras las
  reglas sean de prueba; el probe no restaura referencias, así que la medición no se ve afectada. Si una regla se
  adopta, se añaden en esa tarea. *Decisión del constructor.*
- **`BeatFeatures`** no cambia (va a la UI). Para afinar, el detector expone los valores de medios del último beat
  (ataques en dB y su referencia, y golpes por banda) y `detect-probe --features` los imprime. *Decisión del
  constructor.*
- UI: `api.ts` y `causes.ts` añaden las dos causas con palabras: `Attacks` → "attacks rose", `Hits` → "hits doubled".

## 5. Medición

Con `just detect-probe` sobre `recordings/songs/song-1..5` (git-ignored, con etiquetas), `--phrase 8 --downbeat 0`,
como en `.superpowers/plan16-measure.md`:

1. Línea base con los valores de hoy. Debe dar lo mismo que el final del plan 16 (Build-up: 1 fallo, 2 falsas
   alarmas, retraso medio +2.2 s); si no, se para y se investiga.
2. `mid_flux` solo, barriendo sus valores de §4.5.
3. `mid_hits` solo, barriendo los suyos.
4. Las dos juntas, con los mejores valores de cada una.

Tabla de aciertos, fallos, falsas alarmas y retraso medio del Build-up por canción en `.superpowers/plan17-measure.md`.
El usuario elige a la vista de la tabla; ninguna regla se enciende por defecto sin su decisión. Criterio de partida
para recomendar: menos fallos o menos retraso, sin más falsas alarmas.

## 6. Lo que decide quien construye

- La estructura interna (dónde viven los anillos y los contadores nuevos), siempre sin asignar memoria en `push`.
- Los nombres de los tests.
- Cómo se ve la tabla de medición.
