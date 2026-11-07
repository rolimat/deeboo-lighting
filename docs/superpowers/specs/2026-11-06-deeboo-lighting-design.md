# Deeboo Lighting — Documento de diseño

Fecha: 2026-11-06 · Estado: aprobado por el usuario (2026-11-07), con la sección de seguridad (§12) añadida a petición suya

Convención de este documento: lo que no lleva marca lo decidió el usuario.
*(mi decisión)* = lo decidió el diseñador y el usuario puede revisarlo.
*(constructor)* = queda a criterio de quien lo construya.

## 1. Intención y porqué

DJs que pinchan en casa, no profesionales, con tiras LED en la habitación donde tienen el equipo.
Quieren que las luces vayan **sincronizadas con la música, incluidas las frases**, como en una
discoteca, sin tener que tocarlas durante la sesión.

Deeboo Lighting es una **app de barra de menú para macOS**, hermana de Deeboo MIDI Sync y con el mismo stack.
Funciona así:

```
Fuente de clock (Traktor, Deeboo MIDI Sync, DAW…) ──MIDI clock──▶ Deeboo Lighting ──wifi (Tuya local)──▶ tira LED
Micrófono del Mac o tarjeta de sonido ──audio──▶ Deeboo Lighting (decide el "momento" de la música)
```

- El **MIDI clock** da el tempo y la posición: beat, compás y frase.
- El **audio** dice en qué momento de la canción estamos: Ritmo, Calma o Subidón.
- La tira se divide en **segmentos** que actúan como bloques. Las **secuencias** dicen qué segmento se enciende en cada paso,
  usando **papeles de color**. Las **paletas** dan los colores concretos.
- Los **sets** combinan secuencias y paletas por momento. La app también **genera secuencias nuevas** en tiempo real.

Caso del usuario: Traktor → Deeboo MIDI Sync → Deeboo Lighting, con una mesa de mezclas externa. Por eso el audio
master no sale de Traktor y la app escucha el micro del Mac o una tarjeta con la salida de la mesa.
La app debe ser **genérica**: el clock puede venir de cualquier sitio.

## 2. Objetivos (v1)

1. Escuchar MIDI clock de cualquier fuente y seguir beat (1–4), compás y frase.
2. Conectar una tira Tuya por wifi en local con un **asistente sencillo**: login con Smart Life por QR, búsqueda
   automática en la red y detección del número de segmentos.
3. Dibujar la **forma física** de la tira con piezas libres.
4. Diseñar **secuencias** en una rejilla tipo caja de ritmos, con **simulación** sobre la forma real.
5. **Paletas** de colores que combinan y **sets** por momento, con rotación configurable.
6. **Generación** de secuencias en tiempo real a partir de patrones.
7. **Detección del momento** (Ritmo / Calma / Subidón) con el audio de una entrada elegible, más override manual.
8. Barra de menú con estado, BPM y momento; popover con los controles de directo; ventana de editor.
9. Icono de la app, de la familia visual de Deeboo MIDI Sync.
10. `.dmg` y desarrollo en Docker con TDD, linters, hooks y CI, igual que en Deeboo MIDI Sync.
11. **Seguridad estricta desde el primer día**: seguro por diseño, sin brechas (§12). Requisito explícito del usuario.
12. **Eficiencia**: rendimiento excelente con el mínimo de recursos (§13). Requisito explícito del usuario.

## 3. No-objetivos (v1)

- Tiras **solo Bluetooth**. El usuario lo descartó por ahora.
- **Varias tiras** a la vez. Descartado por ahora.
- Control por la nube de Tuya. Solo control local por wifi; la nube se usa únicamente para obtener la key.
- Tiras sin segmentos pintables: las que no aceptan el formato de §5.3 se muestran como "no compatibles".
  *(mi decisión)*
- Otras marcas (Hue, Govee, WLED…).
- Windows y Linux como plataformas de uso.
- Firma de código y notarización de Apple.
- **Recolocar la frase** con el audio (por ejemplo, en el drop) o con un botón "Mark 1". El usuario eligió que la
  posición de frase salga **solo** del arranque del clock (§6.1).

**Fase 2 (acordado, fuera de la v1):**
- introducir la key a mano, como plan B si el login por QR deja de funcionar;
- exportar e importar sets.

## 4. Anti-objetivos (sería un fracaso aunque "funcione")

- Luces que **parpadean a lo loco** o a destiempo. En Ritmo, los movimientos son **por beat**, en paralelo y sin
  parpadeo rápido.
- Que la app **acumule retraso**: si la tira no da abasto, se descartan pasos atrasados, nunca se encolan.
- Que el cambio de momento llegue **tan tarde** que ya estemos en el siguiente: entrar en Calma cuando ya vuelve el bombo,
  o seguir en Subidón después del drop.
- Colores que no combinan: las secuencias nunca llevan colores concretos, solo papeles de color.
- Que la app aparezca en el Dock o pida atención: es una utilidad de fondo.
- Textos de la app en un idioma que no sea **inglés**.
- **Cualquier brecha de seguridad**:
  - credenciales filtradas (en logs, ficheros, exportaciones o el repo);
  - la app hablando con hosts no previstos;
  - entrada no validada que provoque un fallo o un comportamiento inesperado;
  - dependencias con vulnerabilidades conocidas.

## 5. Hardware y protocolo (verificado en el spike del 2026-11-06)

### 5.1 Dispositivo de referencia

Tira del usuario:
- Smart Life, modelo **JSK-P22**, categoría Tuya `dd` ("light strip"), módulo Tuya;
- protocolo local **3.3**;
- **10 segmentos** (el usuario ve 9 trozos físicos);
- controlador wifi + Bluetooth.

**Ojo:** estaba emparejada **solo por Bluetooth** y no aparecía en la red. Al volver a añadirla en Smart Life entró en la
wifi de 2,4 GHz, y en ese momento **cambiaron su ID y su key**.

### 5.2 Conexión local

- Librería **`rustuya` 0.4.x** (MIT, feature `tokio`). Probada en el spike:
  - `Discovery` encontró la tira con ID, IP, versión y `productKey`;
  - `Device::builder(id, key).address(ip).version(V3_3).connect()` conecta;
  - `set_value(dp, valor)` envía;
  - `listener()` recibe los mensajes que manda la tira.
- La **nube no anuncia** el DP de segmentos (solo `switch_led`, `work_mode`, `colour_data`, `countdown`), pero la tira lo
  usa en local.
- Los DPs `raw` viajan como **base64** en el JSON.

### 5.3 Formato para pintar segmentos (DP **61**, `paint_colour_data`, raw)

Capturado mientras el usuario pintaba en Smart Life, y reproducido con éxito desde el Mac:

```
Cabecera:  00  MM  00  NN
           │   │   │   └ número de segmentos de la tira (0x0a = 10)
           │   │   └ efecto (0x00 ninguno)
           │   └ modo (0x01 color; Smart Life usa 0x02 "tarjeta de color"; los dos funcionan)
           └ versión 0x00
Orden:     AA  HHHH SSSS VVVV  [SEL seg…]
           │   └ H 0–360, S 0–1000, V 0–1000, enteros de 16 bits big-endian
           └ acción: 0x00 bote (toda la tira, sin SEL) · 0x01 lápiz · 0x02 goma (apaga; HSV a 0)
           SEL = 0x80 | k, seguido de k números de segmento (empiezan en 1)
```

Ejemplos reales, que deben usarse como datos fijos de los tests:

| Acción | Hex |
|---|---|
| Bote rojo | `0001000a 00 0000 03e8 03e8` |
| Bote azul | `0001000a 00 00f0 03e8 03e8` |
| Lápiz rojo en 1,3,5,7,9 | `0001000a 01 0000 03e8 03e8 85 0103050709` |
| Lápiz blanco en 2,4,6,8,10 | `0001000a 01 0000 0000 03e8 85 020406080a` |
| Goma en el segmento 8 (Smart Life) | `0002000a 02 0000 0000 0000 81 08` |

El blanco es `S=0, V=1000` y se ve blanco.

### 5.4 Límites medidos

- **Solo se aplica la PRIMERA orden de cada mensaje.** Si concatenas dos órdenes, la segunda se ignora.
  Consecuencia: un cambio de escena con N colores distintos necesita **N mensajes**.
- Con **un mensaje por cambio**, la tira siguió bien a 2, 4 y 8 cambios por segundo. Confirmó 160 de 162 a 8/s.
- Con **dos mensajes seguidos sin pausa** por cambio, a 2 cambios por segundo, se perdieron algunos y el usuario vio
  **estados intermedios y orden alterado**. Por tanto, hay que espaciar los mensajes.
- Latencia: la tira acusa recibo en ~100 ms y devuelve el nuevo estado en ~400 ms. **No se ha medido** cuándo se ve el
  cambio.
- El ping ICMP tarda 1–2 s por el ahorro de energía del wifi, pero no afecta a la conexión TCP.

### 5.5 Obtención de la key: login con Smart Life por QR

Lo mismo que hace la integración oficial de Tuya en Home Assistant, a través de `tuya-device-sharing-sdk`
(Python, **MIT**, versión 0.2.15). Se porta a Rust.

1. `POST https://apigw.iotbing.com/v1.0/m/life/home-assistant/qrcode/tokens?clientid=HA_3y9q4ak7g4ephrvke&usercode=<code>&schema=haauthorize`
   → `result.qrcode` (token).
2. Mostrar un QR con el texto `tuyaSmart--qrLogin?token=<token>`. El usuario lo escanea en Smart Life y confirma.
3. Consultar `GET …/qrcode/tokens/<token>?clientid=…&usercode=…` hasta que llegue `success`. Devuelve `access_token`,
   `refresh_token`, `uid`, `expire_time`, `terminal_id` y `endpoint`.
4. Con esa sesión: `GET /v1.0/m/life/users/homes` y `GET /v1.0/m/life/ha/home/devices?homeId=…`. Cada aparato trae
   `id`, `name`, `category`, `product_name`, `local_key` y `online`.
   - Estas peticiones van **cifradas y firmadas** (AES-GCM + HMAC-SHA256 con una clave derivada de un `requestId` y del
     `refresh_token`): `_secret_generating`, `_restful_sign`, `_aes_gcm_encrypt` y `_aex_gcm_decrypt` en
     `tuya_sharing/customerapi.py`. Se portan tal cual.

**Verificado en el spike con el SDK de Python:** el flujo completo (pasos 1–4) se ejecutó dos veces con la cuenta del
usuario y devolvió sus 3 aparatos con `local_key`. Con esa key, `rustuya` controló la tira. **El port a Rust no está
probado todavía:** es la primera tarea del plan *(mi decisión)*. Si el port no funciona, se para y se consulta al usuario.

**Durante el desarrollo** se puede cargar una key conocida con una variable de entorno, solo en builds de desarrollo y sin
UI *(mi decisión)*. Así se puede trabajar en la tira antes de tener el asistente.

**Riesgo aceptado por el usuario:** se usa el registro de app de Home Assistant (`client_id`). Tuya podría cortarlo. El plan
B (key manual) queda para la fase 2.

## 6. Comportamiento

### 6.1 Reloj, beat y frase

- **Entrada MIDI:**
  - el usuario elige una fuente de la lista de puertos (por ejemplo, `Deeboo Clock Sync Out`);
  - la app además crea su propio destino virtual **`Deeboo Lighting In`**, para que cualquier software pueda enviarle el
    clock sin un bus IAC. *(mi decisión)*
- Clock: 24 pulsos por negra (`0xF8`). BPM medido como en Deeboo MIDI Sync.
- **Beat 1 = el primer mensaje de clock que llega tras una parada**, sea `Start` (`0xFA`), `Continue` (`0xFB`) o un pulso
  `0xF8`.
  - Además, un `Start` recibido en marcha también reinicia al beat 1. *(mi decisión)*
  - SPP se ignora.
  - La posición de beat, compás y frase sale **solo** de ahí: no se recoloca con el audio ni a mano (decisión del
    usuario). Si el clock no arrancó en el 1 de una frase, el usuario lo recoloca parando y arrancando el clock en su
    origen, como hace con Deeboo MIDI Sync.
- **Parada:** `Stop` (`0xFC`) o **500 ms sin clock** (mismo umbral que MIDI Sync). La tira pasa a la escena de **reposo**:
  todos los segmentos quietos en el color **Suave** de la paleta actual. *(mi decisión)*
- Compás de **4 beats**. **Longitud de frase configurable: 4, 8 o 16 compases**, 8 por defecto *(el valor por defecto es
  mi decisión)*.

### 6.2 Momentos y detección por audio

Tres momentos:
- **Ritmo:** con bombo.
- **Calma:** break sin graves.
- **Subidón:** subida hacia el drop.

**Entrada de audio:** cualquier dispositivo de entrada de macOS (micro del Mac o tarjeta USB con la salida de la mesa),
elegible en ajustes. Es una sola feature.

**Detección**, punto de partida aprobado por el usuario y que se afinará con pruebas reales:
- **Por beat, sincronizada con el clock:** en una ventana alrededor de cada beat esperado se mide la energía de graves
  (bombo, aprox. 40–120 Hz).
- **→ Calma:** 2 beats seguidos sin bombo. Se aplica en el beat siguiente.
- **→ Subidón:** tendencia creciente durante 1–2 compases de energía aguda y densidad de golpes (redoble, risers). Puede
  venir de Calma o de Ritmo. Se aplica al empezar el compás siguiente.
- **→ Ritmo (drop):** vuelve el bombo **y** hay **bajo grave sostenido** *(mi decisión, por la duda del usuario: los
  redobles de bombo de un subidón no suelen llevar sub-bajo)*.
  - **Predicción:** tras un Subidón, la escena de Ritmo se envía con antelación para que llegue al **beat 1 de la frase
    siguiente**. Si el drop no se confirma en ese beat, se corrige al momento anterior.
  - Si el drop cae fuera del inicio de frase, se aplica en el beat en que se detecta.
- **Override manual:** al tocar un momento en el popover se fija y se desactiva Auto, hasta que el usuario reactiva Auto.
- **Sin audio** (sin permiso o sin dispositivo): se queda en Ritmo y el estado de audio se marca en rojo.
- **Ajustes:** sensibilidad general en el popover; umbrales y medidores en vivo en el editor (§7.3).

### 6.3 Modelo

- **Tira:** ID, IP y versión, número de segmentos (N) y **forma**. La forma es una lista de N piezas, cada una con
  posición, ángulo y longitud en una cuadrícula.
- **Papeles de color:** `Principal`, `Contraste`, `Suave`, más `White` y `Off`, que son fijos.
- **Paleta:** nombre + colores para Principal, Contraste y Suave.
- **Brillo general:** escala la V de todos los colores.
- **Secuencia:**
  - nombre;
  - momento al que pertenece;
  - lista de **pasos**: cada paso tiene una duración (½, 1, 2 o 4 beats) y el papel de color de cada segmento.
  - Un paso describe el estado completo de la tira; el motor calcula qué cambia.
- **Set:** para cada momento:
  - lista de secuencias, que puede incluir la entrada especial **Generated**;
  - lista de paletas;
  - **orden**: `seguido`, `aleatorio` o `fijo` (fijo = una sola secuencia, sin rotar);
  - **intervalo de cambio** en beats, compases o frases.
  - Por defecto *(mi decisión)*: Ritmo cada 1 frase; Calma, una secuencia por break; Subidón cada 2 compases.
  - **Paleta nueva en cada drop** *(mi decisión)*.
- **Set de serie "Auto":** todo Generated y todas las paletas.
- La app trae **~8 paletas y ~10 secuencias** de ejemplo *(mi decisión)*. Las paletas tienen que combinar, y para Calma,
  Suave es un color claro.
- Los cambios de secuencia o paleta caen siempre en un **límite de compás** (de frase, si el intervalo es en frases).

### 6.4 Generación en tiempo real

Biblioteca de patrones por momento. Ejemplos dados por el usuario:

- **Ritmo:**
  - mitades;
  - extremos ↔ centro (1-3 y 8-10 en el beat 1, 4-7 en el beat 2);
  - pares ↔ impares;
  - cuartos, encendiendo 2 grupos y luego los otros 2;
  - serpiente (1, 2, 3…);
  - espejo.
- **Calma:** solo algunos segmentos encendidos, que cambian o se desplazan despacio (cada 4–8 beats), con colores Suave.
- **Subidón:** destellos de colores por grupos, juegos de segmentos y serpiente rápida, cada beat o medio beat.

Reglas:
- Los grupos espaciales (izquierda/derecha, arriba/abajo, centro/extremos) se calculan a partir de la **forma** dibujada.
- Los patrones priorizan **pasos de un solo mensaje** (§6.5).
- Generated produce una secuencia nueva en cada intervalo de cambio y alterna patrones y parámetros. *(constructor:
  algoritmo de variación)*

### 6.5 Motor de envío (estado → mensajes)

1. En cada paso se compara el estado deseado con el **último estado enviado**. Los segmentos que cambian se agrupan por
   color destino, y cada grupo es un mensaje.
   - Si todos los segmentos acaban del mismo color, se usa el **bote**.
   - `Off` usa la **goma**.
2. **Orden:** primero lo que se apaga y luego lo que se enciende *(mi decisión)*, para que el estado intermedio sea
   "oscuro" y no "todo encendido".
3. **Espaciado mínimo entre mensajes** *(valor inicial mío, a calibrar con la tira real)*: 125 ms, que corresponde a los
   8 por segundo medidos.
4. **Sin retrasos acumulados:** si los mensajes de un paso no caben antes del siguiente, se descarta lo pendiente y se
   salta al estado del paso nuevo.
5. **Compensación de latencia:** los envíos se adelantan respecto al beat, porque el clock permite saber cuándo cae el
   siguiente. Ajuste avanzado en ms, con un valor inicial a calibrar en la prueba de aceptación.
6. El editor **avisa** en las columnas de la rejilla cuyo paso necesita más de un mensaje ("se verá en dos tiempos").

### 6.6 Luces / Apagar y reposo

- **Lights ▶:** arranca el show, que sigue al clock.
- **Off:** deja la tira apagada (goma o bote a negro; *constructor*).
- Sin clock: escena de reposo (§6.1).

### 6.7 Conexión de la tira (asistente)

1. **Connect with Smart Life:** instrucciones de dónde está el User Code en Smart Life (Me → Settings → Account and
   Security → User Code); se pega el código; se muestra el QR; se escanea y confirma (§5.5).
2. **Elegir la tira:** se listan los aparatos de categoría de tira de luz (`dd`). El resto aparece en gris como "not
   supported".
3. **Encontrarla en la wifi:** discovery UDP, emparejando por ID → IP y versión; luego se conecta con la key.
   - Si no aparece, se explica en inglés que puede estar conectada solo por Bluetooth o en otra red, y cómo volver a
     añadirla en Smart Life en una wifi de 2,4 GHz.
   - El asistente sigue buscando y avanza solo cuando la encuentra.
4. **Detectar segmentos:** "Open Smart Life, go to the paint screen and tap any segment". La app escucha el DP 61:
   - comprueba que el formato es el de §5.3;
   - toma **N del byte de cabecera**.
   - Siempre se usa ese N. *Suposición no verificada: la tira no anuncia N por sí misma, y mandarle otro N tiene un
     efecto desconocido.*
   - Si no llega un DP 61 en un tiempo razonable, la tira se marca como no compatible y se ofrece reintentar.
5. **Forma de la tira:** se crean N piezas en fila. **Al pulsar una pieza**, ese segmento se enciende en blanco en la tira
   real y los demás se apagan, para identificarlo.

**Re-emparejado:** si la key deja de valer (`watch_error` de rustuya) o el ID ya no aparece, la app avisa y pide repetir
los pasos 1–3. Se conservan la forma, las secuencias y los sets.

**Credenciales:**
- La `local_key` se guarda en un **fichero aparte con permisos `0600`** en la carpeta de configuración de la app, separado
  de la biblioteca (secuencias, paletas, sets) para que nunca acabe en una exportación *(mi decisión)*.
- No se usa el Llavero porque la app va con firma ad-hoc, y cada versión nueva haría que macOS volviera a pedir permiso
  (anti-objetivo de §4). La key solo permite controlar la luz desde la red local.
- La sesión de la nube no se guarda: si hace falta, se repite el QR *(mi decisión)*.

## 7. Interfaz (textos en inglés)

Aprobado con bocetos en el navegador. La app solo nombra a terceros ("Smart Life") en el asistente de conexión, porque el
usuario tiene que abrir esa app. *(mi decisión: excepción a la regla de MIDI Sync de no nombrar software)*

### 7.1 Barra de menú (cerrada)

`[icono] ● 125.0 [onda]`

- **Icono:** versión monocroma (plantilla) del icono de la app.
- **Punto de estado**, como en MIDI Sync:

| Punto | Significa |
|---|---|
| **Rojo fijo** | no conectado: falta el clock o la tira |
| **Verde fijo** | todo conectado, luces paradas |
| **Verde parpadeando con el beat** | en play |

- **BPM** con un decimal.
- **Onda animada** (opción B elegida), monocroma:
  - **Rhythm:** onda cuadrada al tempo;
  - **Calm:** onda suave y lenta;
  - **Build-up:** zigzag rápido que crece.
  - Sin onda cuando está parado o desconectado.
  - Momento forzado a mano: un puntito bajo la onda.
  - *(constructor: fotogramas pre-renderizados, ≥ 8 fps, legible en barra clara y oscura)*

### 7.2 Popover (aprobado, como el boceto `menubar-2.html`)

Mismo estilo visual que Deeboo MIDI Sync: los mismos tokens de color y tarjetas.

- Cabecera "Deeboo Lighting" + ⚙.
- **Tempo:** BPM grande, 4 barras de beat, "bar 3 / 8" (compás dentro de la frase).
- **Lights ▶ / Off.**
- **Moment:** Rhythm · Calm · Build-up, con "Auto ✓". Se ilumina el detectado; si tocas uno, lo fuerzas.
- **Set ◀ ▶**, **Palette ◀ ▶** (con sus 3 colores) y **Brightness** (deslizador).
- **Estado:** MIDI (fuente), Strip (nombre y N segmentos), Audio (dispositivo + medidor de nivel).
- **Open editor…**
- **⚙ Settings** (vista dentro del popover, como en MIDI Sync):
  - MIDI input;
  - audio input;
  - strip;
  - detection sensitivity;
  - **phrase length (4 / 8 / 16 bars)**;
  - launch at login.

### 7.3 Ventana del editor

Secciones *(mi decisión en la agrupación)*:

- **Strip:** asistente de conexión (§6.7) y **forma**. Piezas libres que se arrastran y giran sobre una cuadrícula, cada
  una con su número; al pulsar una, se identifica en la tira real (§6.7.5). Opción A elegida.
- **Sequences:**
  - **rejilla tipo caja de ritmos**: filas = segmentos, columnas = pasos (opción A elegida);
  - selector de papel de color para pintar casillas; duración de cada paso; momento de la secuencia;
  - aviso por columna si el paso necesita más de un mensaje;
  - **simulación** en vivo sobre la forma dibujada, con BPM y paleta elegibles, sin necesitar clock ni tira.
- **Palettes:** crear y editar los colores de Principal, Contraste y Suave.
- **Sets:** por momento, secuencias (incluida Generated), paletas, orden e intervalo.
- **Detection:** medidores en vivo (bombo, bajo grave, agudos/densidad), el momento detectado y 2–3 umbrales ajustables,
  para afinar con pruebas reales *(mi decisión)*.

### 7.4 Icono de la app

- Familia de Deeboo MIDI Sync (ver `deeboo-midi-sync/src-tauri/icons/app-icon.svg`): baldosa redondeada oscura con un
  disco de vinilo.
- **En lugar del conector MIDI naranja, un anillo de segmentos LED de colores** alrededor del vinilo. Idea aprobada por
  el usuario.
- Entregables: SVG fuente + PNG/ICNS/ICO para el bundle + variante monocroma para la barra.
- *(constructor: composición exacta; el usuario revisa el resultado)*

## 8. Stack técnico (el mismo que Deeboo MIDI Sync)

- **Tauri 2** (Rust, workspace edition 2024, `clippy::pedantic`, toolchain fijada en `rust-toolchain.toml`).
- **UI:** TypeScript + Vite sin framework, CSS propio con variables, mismos tokens que MIDI Sync. Vitest.
- **MIDI:** `midir`.
- **Tuya local:** `rustuya` (feature `tokio`). Tauri ya usa Tokio.
- **Login nube:** cliente HTTP + AES-GCM + HMAC-SHA256 + MD5 portados del SDK MIT. Generación del QR en local.
  *(constructor: crates concretos; `cargo-deny` debe aceptar sus licencias con GPL-3.0)*
- **Audio:** captura de la entrada (por ejemplo, `cpal`). El análisis se hace con **filtros IIR por bandas y envolventes**,
  no con FFT (§13) *(mi decisión)*.
- **Credenciales:** fichero `0600` (ver §6.7), sin Llavero.
- **Skill de Rust para agentes:** `rust-engineer` de Jeffallan
  (https://github.com/Jeffallan/claude-skills/tree/main/skills/rust-engineer), **la original**, no la adaptada de
  MIDI Sync.
  - Las demás skills del repo (TDD, UI, flujo de desarrollo) se adaptan de las de MIDI Sync a este dominio.
  - Se añade una skill de dominio para Tuya y el formato de segmentos.
- **Licencia:** GPL-3.0, como MIDI Sync *(mi decisión)*.

### 8.1 Estructura propuesta

```
crates/light-core/   # Rust puro, determinista, sin I/O: reloj→beat/compás/frase, máquina de momentos,
                     # modelo (secuencias, paletas, sets), generador, diff de escena → órdenes,
                     # planificador con espaciado/descartes, codificación del DP 61,
                     # clasificador de momentos a partir de rasgos de audio por beat.
src-tauri/           # midir, captura de audio + FFT → rasgos, rustuya, login nube, credenciales,
                     # persistencia, tray, comandos/eventos Tauri.
ui/                  # popover + ventana editor (forma, rejilla, simulación, paletas, sets, detección).
tools/detect-probe/  # binario de desarrollo: decodifica una grabación (MP3 o WAV), estima la rejilla de
                     # beats a partir del audio (no hay clock), ejecuta el MISMO clasificador de light-core
                     # e imprime las transiciones de momento con su tiempo (mm:ss).
```

- `light-core` se prueba con tiempo simulado. Ahí vive todo lo crítico.
- Los payloads de §5.3 son datos fijos de los tests.
- Persistencia en JSON en la carpeta de configuración de la app *(constructor: formato)*.
  - La key **nunca** va al JSON.

## 9. Distribución

Igual que Deeboo MIDI Sync:
- `.dmg` con firma ad-hoc y sin notarizar;
- release por tag `vX.Y.Z` con `SHA256SUMS` y atestación de procedencia;
- README con instrucciones de instalación y verificación;
- CSP estricta.

Además, en `Info.plist`:
- **`NSMicrophoneUsageDescription`**;
- **`NSLocalNetworkUsageDescription`**, porque hay discovery UDP y TCP a la red local.

## 10. Entorno de desarrollo y calidad

Igual que Deeboo MIDI Sync:
- Docker / devcontainer para todo lo que no requiera macOS;
- `just` (`fmt`, `lint`, `test`, `precommit`, `dev`, `dmg`);
- `lefthook` (pre-commit = formato + lint + tests);
- `cargo-deny`, `hadolint`, ESLint, Prettier, `tsc --strict`;
- CI en GitHub Actions (Linux en la misma imagen Docker + macOS) y Dependabot;
- `AGENTS.md` (con `CLAUDE.md` como enlace) y mapa de skills;
- **TDD obligatorio.**

Fuera de Docker (macOS): app real con MIDI, audio y tira; DMG.

El repositorio remoto se crea igual que el de MIDI Sync *(constructor: confirmar nombre con el usuario al publicar)*.

**Orden recomendado del plan** *(mi decisión)*:
- el **detector** (`light-core` + `tools/detect-probe`) se construye y se afina pronto, porque es la parte con más
  riesgo;
- el resto se construye encima.

**Afinado del detector** (acordado con el usuario):
1. El usuario pasa **un MP3 de una sesión larga suya**.
2. `detect-probe` propone las transiciones (break, subidón, drop) con su tiempo.
3. El usuario las escucha y corrige (añade, quita o mueve), sin tener que etiquetar desde cero.
4. El resultado corregido queda como **fichero de etiquetas** junto a la grabación. Es el criterio de aprobado o
   suspenso del detector: transiciones acertadas y retraso de cada una en beats.
- La grabación y sus etiquetas **no se suben al repo** (derechos de la música). Viven en una carpeta local ignorada por
  git, y los tests de CI usan señales sintéticas.
- **Límite conocido:** un MP3 es la señal limpia del master. El micro de la habitación, que *(mi decisión)* es el
  objetivo principal de la v1 porque es lo que tiene hoy el usuario, añade ruido y reverberación. Se valida en la prueba
  de aceptación 7, con música real en la habitación.

## 11. Pruebas de aceptación (manuales, en macOS, con la tira real)

1. Asistente: con el User Code y el QR aparece "dj lights"; las bombillas salen como no compatibles. La app encuentra la
   tira en la wifi. Tras pintar un segmento en Smart Life, detecta 10 segmentos.
2. Tira solo por Bluetooth (o apagada): el asistente lo explica y avanza solo cuando la tira entra en la wifi.
3. Forma: al pulsar cada pieza se enciende el segmento correcto en la tira.
4. Simulación: una secuencia se ve en el editor igual que luego en la tira, con el mismo orden de segmentos y los mismos
   colores de paleta.
5. Clock desde Deeboo MIDI Sync: al pulsar Play allí, el primer beat que recibe Deeboo Lighting es el beat 1. El popover
   muestra BPM, beat y compás de la frase, y la tira sigue los beats.
6. Stop del clock o 500 ms sin clock: la tira pasa a reposo y el punto se pone en verde fijo (o rojo si se pierde la
   conexión).
7. Detección con música real: break → Calma en ≤ 2–3 beats; subida → Subidón; drop → Ritmo en el beat del drop, sin
   quedarse en Calma ni en Subidón.
8. Override manual: forzar un momento lo mantiene (puntito en la barra) hasta reactivar Auto.
9. Sin retraso acumulado: con un set de pasos de medio beat durante 5 minutos, la tira sigue el tempo sin irse quedando
   atrás.
10. Re-emparejar la tira en Smart Life: la app lo detecta y pide reconectar; la forma y los sets se conservan.
11. Instalación desde el `.dmg` en un Mac limpio siguiendo el README, aceptando los permisos de micrófono y red local.
12. Rendimiento: `just perf` durante 10 minutos de sesión real (play + detección) cumple los objetivos de §13.1.
13. Seguridad: `/security-review` sin hallazgos abiertos, `gitleaks` limpio, y los logs de una sesión completa (asistente incluido) sin key, tokens ni User Code.

## 12. Seguridad desde el diseño (requisito del usuario)

El usuario pide seguridad estricta desde el primer día, sin brechas. Las medidas concretas son *(mi decisión)* salvo que
se indique otra cosa. Son obligatorias desde la primera tarea del plan, no un añadido al final.

### 12.1 Qué se protege y de quién

| Activo | Por qué importa | Tratamiento |
|---|---|---|
| Sesión de la nube Tuya (`access_token`, `refresh_token`) | Da acceso a **toda la cuenta** Smart Life: todos los aparatos y sus keys | Solo en memoria; se **revoca** al acabar el asistente |
| `local_key` de la tira | Permite controlar la tira desde la red local | Fichero `0600` aparte; nunca en logs, UI, exportaciones ni repo |
| User Code y token del QR | Identifican la cuenta y la sesión de login | No se guardan; nunca en logs |
| Audio del micrófono | Privacidad de la habitación | Solo en memoria; nunca se graba ni se envía |
| La app distribuida | Integridad de lo que instala el usuario | Release verificable (§9) |

Atacantes considerados:
- otro aparato de la red local (anuncios de discovery falsos, mensajes manipulados);
- respuestas de red manipuladas;
- ficheros de configuración alterados;
- dependencias comprometidas;
- contenido malicioso dentro de la webview.

Fuera de alcance: malware que ya corre con el usuario en el Mac y acceso físico al equipo.

### 12.2 Medidas

**Nube y credenciales**
- Solo HTTPS, con verificación TLS del sistema.
- **Lista blanca de hosts:**
  - `apigw.iotbing.com` para el login;
  - el `endpoint` que devuelve el login solo se acepta si es `https` y pertenece a la lista blanca de dominios de Tuya
    (fijada en código con los dominios que se observen al portar el login a Rust);
  - cualquier otro host se rechaza.
- Timeouts y límite de tamaño en todas las respuestas.
- **Revocación:** tras leer la lista de aparatos se revoca la sesión (`POST /v1.0/m/token/terminal/expire`, como `unload`
  en el SDK) y se borra de memoria. Lo mismo si se cancela el asistente.
- Los secretos se envuelven en un tipo que **no se puede imprimir** (Debug/Display redactados) y se borran de memoria al
  soltarse (*constructor:* `secrecy` + `zeroize` o equivalente). La UI muestra la key enmascarada.
- **Logs sin secretos:** ningún log incluye key, tokens, User Code ni token del QR. Un test lo comprueba.

**Red local**
- Solo se conecta a **IPv4 privadas** (RFC 1918) de la **misma subred** que el Mac.
- Los anuncios de discovery solo cuentan si el ID coincide con la tira elegida.
- Todo lo que llega de la red se trata como no fiable: límites de tamaño, errores sin pánico.
- **Riesgo aceptado e impuesto por el aparato:** el protocolo Tuya 3.3 cifra con AES-ECB y una clave fija. Es débil, pero
  sin la key no se puede controlar la tira. Queda documentado.

**App (Tauri y webview)**
- **Sin contenido remoto:** la webview solo carga ficheros del bundle. CSP estricta como MIDI Sync (sin inline ni
  orígenes externos).
- El QR se genera en local; nunca con un servicio externo.
- **Capabilities de Tauri mínimas** por ventana: solo los comandos propios, sin plugins de shell, fs ni http expuestos a
  JavaScript. Toda la red se hace desde Rust.
- **Validación en Rust de toda entrada de IPC:** tipos, rangos (segmentos ≤ lo que permite el DP 61 (255 bytes),
  pasos, H/S/V, BPM) y longitudes de texto.
- **Configuración y biblioteca validadas al cargar:**
  - versión de esquema y límites;
  - si un fichero es inválido, se guarda una copia y se arranca con valores por defecto, sin perder datos y sin
    fallar.
- MIDI: el parser solo acepta los bytes de tiempo real del clock e ignora el resto.
- **Rust:**
  - `#![forbid(unsafe_code)]` en los crates propios;
  - `clippy::unwrap_used` y `clippy::expect_used` denegados fuera de los tests;
  - tests de propiedades (por ejemplo, `proptest`) para todos los parsers: DP 61, MIDI, configuración y respuestas de la
    nube.
- **macOS:**
  - **Hardened Runtime + App Sandbox** con los permisos mínimos: cliente de red, servidor de red (escucha del discovery
    UDP) y entrada de audio;
  - sin auto-updater.
  - **Verificar pronto** que el puerto MIDI virtual y "abrir al iniciar sesión" funcionan dentro del sandbox. Si algo no
    funciona, se para y se consulta al usuario antes de relajar nada.

**Cadena de suministro y repo**
- **Dependencias:**
  - `cargo-deny` con advisories, licencias, solo crates.io y bans;
  - `npm audit --audit-level=high` en CI;
  - lockfiles en el repo e instalaciones con `npm ci`;
  - el mínimo de dependencias (la UI, sin dependencias de runtime aparte de `@tauri-apps/api`).
- **GitHub Actions:**
  - acciones fijadas por **SHA de commit**;
  - `permissions` de solo lectura por defecto;
  - publicar separado de compilar (como en MIDI Sync);
  - sin `pull_request_target`;
  - Dependabot.
- **Escaneo de secretos** (`gitleaks`) en el hook de pre-commit y en CI.
- Grabaciones, etiquetas y cualquier key de desarrollo quedan fuera del repo (`.gitignore`).
- **Proceso:**
  - reglas de seguridad en `AGENTS.md` y una skill `security` para agentes;
  - cada PR pasa una checklist de seguridad;
  - **revisión de seguridad antes de cada release** (`/security-review`);
  - el modelo de amenazas se actualiza cuando aparece una superficie nueva.

**Higiene del spike:** la sesión de la nube que se usó en el spike se revocó y las keys guardadas se borraron el
2026-11-07.

## 13. Rendimiento y consumo (requisito del usuario)

La app procesa audio en tiempo real y debe ser **súper eficiente**: rendimiento excelente con el mínimo de recursos.
Los objetivos son *(mi decisión)* y se miden en un Mac Apple Silicon con la build de release.

### 13.1 Objetivos medibles

| Situación | CPU (proceso de la app, sin contar WebKit) | Memoria del proceso Rust |
|---|---|---|
| Parada, popover cerrado | ~0 % (sin temporizadores ni sondeos) | < 30 MB |
| En play con detección de audio | < 2 % de un núcleo de media | < 40 MB |
| Popover abierto en play | < 5 % entre app y WebKit | — |

Además:
- **Análisis de audio:** procesar 10 minutos de audio sintético a 48 kHz en **< 1 s** en un solo hilo. Es un test de
  rendimiento en `light-core` que se ejecuta en CI con un margen generoso (×3) para no dar falsos fallos.
- **Callback de audio:** sin asignaciones de memoria, sin bloqueos y sin logs. Un test o lint lo vigila *(constructor:
  cómo)*.

### 13.2 Cómo se consigue

- **Audio:**
  - el callback solo copia muestras a un **ring buffer SPSC sin bloqueos** (por ejemplo, `rtrb`);
  - un hilo de análisis las procesa en bloques con **biquads por banda** (bombo 40–120 Hz, sub-bajo, agudos) y
    seguidores de envolvente, con mezcla a mono;
  - el clasificador corre **una vez por beat**;
  - sin FFT, salvo que el afinado demuestre que hace falta (y entonces se mide).
- **Solo trabaja lo necesario:**
  - la captura de audio se abre solo en play con Auto y se cierra al parar;
  - sin clock no hay motor;
  - la conexión con la tira se mantiene con el heartbeat de `rustuya` y nada más.
- **Dirigido por eventos:** el clock MIDI empuja el motor; nada de bucles de sondeo ni de espera activa. Los envíos a la
  tira se programan con temporizadores de Tokio.
- **UI:**
  - los eventos de estado hacia la webview solo se emiten **con el popover visible**, como máximo a 30 Hz;
  - con el popover oculto, cero;
  - la ventana del editor se **crea al abrirla y se destruye al cerrarla**;
  - la simulación solo anima con la ventana visible (`requestAnimationFrame`).
- **Barra de menú:**
  - punto y onda con **fotogramas pre-renderizados** (sin dibujar en tiempo de ejecución);
  - la onda solo se anima en play;
  - actualizaciones a ≤ 12 fps.
- **Build de release:** `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, `strip = true`.
- **Medición:** `just perf` (macOS) ejecuta la app de release y muestrea CPU y memoria del proceso durante N segundos.
  Forma parte de las pruebas de aceptación.

## 14. Riesgos conocidos

- **Login por QR de terceros:** depende del `client_id` de Home Assistant. Mitigación en la fase 2: key manual.
- **Precisión de la detección** con el micro de una habitación (voz, ruido, reverberación). Mitigaciones: tarjeta de
  sonido, umbrales ajustables, override manual, afinado con grabaciones.
- **Escenas de varios mensajes:** no son atómicas. Mitigación: patrones de un solo mensaje, orden "apagar antes",
  espaciado y aviso en el editor.
- **Variación entre tiras Tuya:** otros modelos pueden usar otro DP u otro formato. La detección por escucha (§6.7.4) lo
  comprueba; si no coincide, la tira se marca como no compatible.

## 15. Decisiones dejadas al constructor

- Algoritmo exacto del detector y sus umbrales iniciales, siguiendo §6.2 y el afinado con grabaciones.
- Algoritmo de variación de Generated y contenido de las paletas y secuencias de ejemplo.
- Crates concretos (HTTP, cripto, QR, audio, ring buffer) dentro de licencias compatibles.
- Formato de persistencia y de logs.
- Composición exacta del icono y de los fotogramas de la onda.
- Cómo apagar la tira en "Off" (goma, bote a negro o `switch_led`).
- Build universal o solo Apple Silicon.
