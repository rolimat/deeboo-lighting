# Deeboo Lighting — rediseño de la interfaz y arreglos del e2e v0.1.0

Fecha: 2026-11-09. Estado: diseño aprobado con bocetos en el navegador. Amplía y, donde choca, **sustituye** las
secciones §6.3, §7 y §11 de `2026-11-06-deeboo-lighting-design.md` (la spec principal).

Bocetos aprobados (no se commitean): `.superpowers/brainstorm/92603-1794249826/content/` — `sequences-v3b.html`,
`palettes-v2.html`, `sets-v5.html`, `strip-v2.html`, `detection-v4.html`, `setup-v2.html`, `menubar-v4.html`.

Las marcas *(mi decisión)* son llamadas del diseñador que el usuario no ha pedido explícitamente; puede cambiarlas.

## 1. Intención y porqué

La prueba e2e con la tira real (2026-11-09) mostró que el motor funciona bien en vivo, pero el editor es difícil de usar:
nombres sin etiqueta, demasiados botones de texto, guardado manual, scroll lateral que no se ve, sets incomprensibles,
un editor de forma que solo parece arrastrar hacia abajo y sin deshacer. La barra de menú parecía dos apps y se confundía
con Deeboo MIDI Sync. Además salieron fallos reales: la lista MIDI vacía si la fuente arranca después, el número de
segmentos que informa el controlador no es el cableado, la paleta no rota en Auto y la memoria supera el objetivo.

El usuario quiere una interfaz **inspirada en la usabilidad de Ableton** (no en sus colores): ver de un vistazo qué suena
en cada momento, rejillas claras, añadir cosas con facilidad y **mucho arrastrar y soltar**.

**Éxito:** el usuario prepara secuencias, paletas y sets sin dudar de qué hace cada control, ve en vivo lo que suena,
afina la detección viéndola, y todo el feedback del e2e queda resuelto. Después se retoma el e2e (bloques 5–8) y se
etiqueta v0.1.0.

## 2. Objetivos

1. Editor rediseñado con un lenguaje común (§4) y seis pestañas: **Sets · Sequences · Palettes · Strip · Detection ·
   Setup**.
2. Guardado automático + deshacer/rehacer en todo el editor; sin botones Save.
3. Ayuda al pasar el ratón, activable desde la barra superior.
4. Modelo: paletas por set con su propia rotación; intervalos solo en compases; secuencias siempre de compases enteros;
   patrones generados con nombre que se pueden arrastrar.
5. Barra de menú de un solo elemento y popover simplificado.
6. Arreglos del e2e: MIDI en caliente, número de segmentos editable, asistente coherente, permisos de micrófono, ruido
   en el log, memoria.

## 3. No-objetivos y anti-objetivos

- **No:** vista de línea de tiempo de un show entero (Arrangement de sets); temas claros/oscuros nuevos; deshacer
  entre sesiones; editar patrones generados en sitio (se duplican para editar); nombrar Deeboo MIDI Sync en la app.
- **Anti-objetivos (fracaso aunque funcione):**
  - colores por momento o emojis: el usuario los rechazó; solo un acento (turquesa) e iconos de línea SVG;
  - un control cuyo efecto no se entiende sin leer documentación;
  - poder soltar una secuencia en la pista de otro momento;
  - perder cambios por el guardado automático (un fallo de validación nunca descarta lo que el usuario hizo);
  - romper la biblioteca existente del usuario al migrar.

## 4. Lenguaje visual común (todas las pestañas)

- **Barra superior** (siempre igual, de izquierda a derecha): pestañas · deshacer · rehacer · separador · duplicar ·
  borrar · «✓ Saved» · ayuda (?) · lectura en vivo.
  - Deshacer/rehacer con las flechas curvas habituales; ⌘Z / ⇧⌘Z.
  - Duplicar (dos cuadrados) y borrar (papelera, en rojo) actúan sobre lo seleccionado en la pestaña. Borrar siempre
    pide confirmación («Delete "Strobe"? You can undo this with ⌘Z.»). En pestañas sin nada que duplicar (Strip,
    Detection, Setup) no aparecen.
  - «✓ Saved» refleja el guardado automático: «Saving…» mientras escribe, «✓ Saved» al terminar, «Not saved — <motivo>»
    en rojo si Rust rechaza el cambio.
  - **Ayuda (?):** turquesa = encendida. Encendida, cada control muestra al pasar el ratón una explicación corta de qué
    hace. El estado se recuerda entre sesiones (ajuste `help_tooltips`, por defecto encendido *(mi decisión)*).
  - **Lectura en vivo:** `● 122.0 BPM · beat 2 · phrase: bar 3 of 8 · [icono] Rhythm`. Sin reloj: `○ No clock`.
- **Etiqueta a la izquierda de su campo, en la misma fila**, en todas las pantallas (`NAME [Pulse]`).
- **Navegador a la izquierda** (Sets, Sequences, Palettes): buscador, listas, elementos arrastrables con asa ⋮⋮.
- **Iconos:** SVG de línea empaquetados en el repo (sin CDN; CSP estricta), con `aria-label` y ayuda. Cero emojis, también
  en las pestañas.
- **Color:** fondo y paneles grises; **un acento turquesa** para lo que suena, la selección y la zona donde se puede
  soltar. El rojo solo para borrar y errores. Sin colores por momento.
- **Iconos de momento** (los mismos en editor, popover y barra de menú): Rhythm = pulso, Calm = onda suave,
  Build-up = flecha que sube.
- **Arrastrar y soltar:** al coger algo, solo se iluminan en turquesa los destinos válidos; los demás se atenúan y dicen
  por qué («Rhythm only»). Soltar en un destino inválido no hace nada.

## 5. Pestañas

### 5.1 Sets (vista Sesión)

- **Navegador:**
  - **Sets:** cada uno con ▷ para hacerlo sonar (como lanzar una escena) y ▶ turquesa en el que suena. Clic en el nombre
    = abrirlo para editar, sin cambiar lo que suena. «+ New set».
  - **Sequences:** agrupadas por momento con su icono de grupo y el icono del momento en cada elemento. Los patrones
    generados (§6.3) van mezclados, con un icono turquesa de «generado».
  - **Palettes:** con sus tres colores.
- **Cabecera:** `SET NAME [Club night]` y a la derecha el estado: «▶ Playing live» o «Not playing · ▷ Play live».
- **Tres pistas** (Rhythm, Calm, Build-up) con cabecera neutra + icono. En una fila: `Order [in order | random | fixed]` (iconos de línea) y
  `Change every [8 bars ▾]` (Calm: además «once per break»).
  - Clips = secuencias de la pista, con miniatura de segmentos y longitud. Se reordenan arrastrando; ✕ los quita;
    ◉ mantenido = probar en la tira; doble clic = abrir en Sequences. El clip que suena brilla y lleva barra de progreso.
  - Zona «Drop sequences here» al final de cada pista.
- **Paletas del set:** una sola fila a todo el ancho, **alineada abajo**. Chips en línea con la zona de soltar a la
  derecha; si no caben, más filas. Cabecera: `Palettes · change [at each drop ▾] [→ | ⇄]`. ▶ en la que suena.
- Se acaba el «Preview» que no se podía cerrar y las casillas por momento.

### 5.2 Sequences (Arrangement de una secuencia)

- Navegador igual que en Sets (sin la lista de sets); la seleccionada en turquesa; «+ New sequence»; paletas para
  arrastrar sobre la rejilla y cambiar la vista previa.
- Cabecera: `NAME [Pulse]` · `MOMENT [icono Rhythm | icono Calm | icono Build-up]` · `PREVIEW PALETTE [Neon ▾]`.
- **Simulación arriba:** la forma real de la tira, ▶ para reproducir, sin BPM; sigue el reloj MIDI si suena, si no 120
  *(mi decisión)*.
- **Regla:** «Bar 1, 1.2, 1.3, 1.4, Bar 2…».
- **Clip:** una barra a todo el ancho con «Pulse · 2 bars · loops» y un asa en el borde derecho: arrastrarla añade o
  quita **compases enteros**. Al añadir, el compás nuevo copia el último *(mi decisión)*.
- **Filas S1…SN:** una por segmento; cada celda = ½ beat; líneas finas por beat y gruesas por compás. Pintar arrastrando
  en cualquier dirección con el papel elegido (Principal · Contrast · Soft · Off). Cursor de reproducción.
- **Sin scroll lateral:** la rejilla cabe en la ventana; zoom ⊖ ⊕ si se quiere más detalle.
- Desaparecen la cabecera de pasos, «Add step», el selector de duración por paso y el BPM.
- Aviso de mensajes múltiples (spec §6.5) sobre la columna afectada, como hoy.
- **Generadas:** se abren en solo lectura con «Duplicate to edit» (crea una secuencia normal con lo que se ve).
- Cambiar el momento de una secuencia que está en pistas de otro momento: aviso «Used by Club night (Calm). It will be
  removed there.» y, al aceptar, se quita de esas pistas *(mi decisión)*.

### 5.3 Palettes

- Navegador con las paletas; `NAME [Neon]`.
- Tres colores grandes: **Principal** («Main colour · most segments»), **Contrast** («Accents and hits»), **Soft**
  («Background, calm moments»). Clic = editar ese; arrastrar uno sobre otro = intercambiar; arrastrar un color desde
  otra paleta del navegador = copiarlo.
- Editor: **rueda de color** (tono + saturación), **un único deslizador de brillo**, muestra grande + hex editable,
  colores rápidos. Sin sliders H/S/V. Se guarda en HSV como hoy.
- Vista previa con la forma de la tira y una secuencia a elegir; **«Show on strip»** la muestra en la tira real mientras
  se edita y se apaga al salir de la pestaña *(mi decisión)*.
- «Used by: [Club night] [Auto]» (sets); clic = ir al set.

### 5.4 Strip

- Tarjeta de la tira: estado (`dj lights · Connected · 192.168.1.42`), **`SEGMENTS [− 8 +]`** con los números 1…M
  (M = lo que informa el controlador; los que sobran en gris discontinuo), texto «The controller reports 10; only 8 are
  wired. Click a number to light that segment and count.», y **Reconnect…** / **Change strip…** en la misma fila.
- **Formas de partida:** Line, Two rows, L shape, U shape, Arch, Zig-zag, Circle.
- **Lienzo libre** con rejilla de puntos: arrastrar piezas en cualquier dirección (el lienzo crece), imán a la rejilla y a
  la pieza vecina; flechas mueven la pieza; ⇧+arrastre mueve toda la forma; ⌘Z.
- Pieza seleccionada: se enciende en la tira real; asa redonda en la punta para girar; barrita flotante con ⟲/⟳ de 15° y
  «Segment 6 · 0°».
- **Guía de 3 pasos** la primera vez (y con la ayuda encendida): elegir forma de partida, arrastrar, pulsar para
  comprobar. Sin vídeo ni demo.
- Cambiar el número de segmentos rehace la forma y adapta secuencias como el camino de re-emparejar.
- **Primer arranque sin tira:** se abre el editor en Strip con el asistente *(mi decisión; resuelve que el icono pasa
  desapercibido)*.

### 5.5 Detection

- **Barra de estado en vivo** encima: `● MIDI clock playing  from <fuente>` · `● Listening  <dispositivo> [nivel]` ·
  «Live view: it moves in real time while the clock plays and the audio input hears the music.» Lo que falta se pone gris
  y dice qué hacer.
- **`MOMENT CHANGES`:** slider de **5 pasos** con «Steady · Balanced · Reactive» debajo (el nombre del paso actual
  resaltado) y una frase que explica el actual. Mismo ajuste que en Setup. «Reset to defaults».
- **Línea de tiempo** «Last 8 bars … now ▸» (lo nuevo entra por la derecha):
  - pista **Moment**: lo que usó el show en cada beat, con los iconos;
  - pistas **Kick**, **Sub**, **Highs** con su descripción, señal (blanco) y referencia (gris);
  - **umbrales arrastrables** sobre la gráfica (línea turquesa discontinua con etiqueta en palabras: «Calm when 14 dB
    below», «Build-up when 6 dB above»). Sustituyen a los tres sliders.
  - Los cambios se aplican desde el siguiente beat.
- **Sin reloj:** en lugar de las gráficas, «Waiting for MIDI clock — Start playback in your clock source (DJ software,
  drum machine…). Detection follows the beats it sends.» + entrada MIDI, audio, nivel y «Change in Setup». Con momento
  forzado: «Moment forced — switch to Auto to see detection».

### 5.6 Setup (nueva)

Bloques con título, icono y una explicación corta a la izquierda:

- **MIDI clock:** `INPUT [fuente ▾]` + «● Receiving · 122.0 BPM». «Any MIDI clock works. The last input used is
  remembered; new sources appear on their own, and if the source goes away it reconnects when it comes back.»
- **Audio input:** `DEVICE [▾]` + medidor de nivel + estado del permiso del micrófono con enlace a Ajustes del Sistema.
- **Light latency:** `SEND EARLY BY [− 100 ms +]` (10 ms por clic) + **Calibrate…**: la tira destella en cada beat
  mientras suena el reloj y se ajusta hasta que coincide con el bombo.
- **Phrase:** `BARS PER PHRASE [4 | 8 | 16]`.
- **Moment changes:** el slider de 5 pasos (§5.5).
- Al final: «Quit Deeboo Lighting».

## 6. Modelo y motor (cambios en `light-core`)

### 6.1 Paletas por set

- `Set` gana `palettes: Vec<u32>`, `palette_order: Order` (InOrder | Random; Fixed = una sola) y
  `palette_change: PaletteChange` = `AtDrop` | `Bars(n)`.
- `MomentSet` pierde `palettes`.
- Un rotador de paletas del set, independiente de los de secuencias: cambia en cada drop o cada `n` compases (en límite
  de compás), en el orden elegido; Random nunca repite la anterior.
- **Popover «Palette: Auto»** = seguir la rotación del set (por defecto). ‹ › fija una paleta a mano hasta volver a Auto
  (hoy el pin dura hasta el siguiente drop: cambia). `active_palette: None` significa Auto.

### 6.2 Intervalos solo en compases

- `Interval` = `Bars(1 | 2 | 4 | 8 | 16)` | `OncePerBreak`. Las frases quedan solo para el contador «phrase: bar x of N»;
  `Beats` y `Phrases` desaparecen.
- `OncePerBreak` solo se ofrece en Calm: avanza una vez cada vez que se entra en el momento (lo que hoy hace el motor
  fijo para Calm, `engine.rs`). Calm también admite `Bars(n)`: entonces rota cada n compases dentro de un break largo.
  Por defecto Calm = `OncePerBreak` *(mi decisión)*.

### 6.3 Patrones generados con nombre

- `SeqRef` gana `Pattern(Pattern)`: ese patrón con parámetros nuevos (SplitMix64) cada vez que entra.
- El navegador lista, por momento, un elemento por patrón de `patterns::for_moment` con nombre en inglés (Halves,
  Ends ↔ centre, Odd / even, Quarters, Snake, Mirror, Drift, Sparse, Group flash, Fast snake, Strobe).
- `SeqRef::Generated` (cualquier patrón) se mantiene para el set «Auto» y aparece como «Any pattern» *(mi decisión)*.
- «Duplicate to edit» pide a Rust la secuencia que se ve (patrón + parámetros + N) y la añade como secuencia normal.

### 6.4 Secuencias de compases enteros

- Validación en Rust: la suma de los pasos es múltiplo de 8 medios beats (un compás) y como mucho 16 compases
  *(mi decisión)*. El límite de pasos sube lo necesario (16 compases en celdas de ½ beat = 128 pasos).
- Los patrones generados también producen compases enteros, para que «Duplicate to edit» dé una secuencia válida.
- El editor trabaja en celdas de ½ beat y guarda como hoy: columnas iguales consecutivas se juntan en pasos de ½, 1, 2 o
  4 beats. El motor y el formato de los pasos no cambian.

### 6.5 Migración de la biblioteca (schema 1 → 2)

- Calm migra siempre a `OncePerBreak`, que es como se comporta hoy, sea cual sea su intervalo guardado.
- `active_palette` migra a `None` (Auto), para que la paleta rote en Auto desde el primer arranque *(mi decisión)*.
- Paletas del set = unión ordenada de las paletas de sus tres momentos; `palette_change: AtDrop`, `palette_order` = el
  orden de Rhythm (Fixed → InOrder si hay más de una).
- Rhythm y Build-up: `Phrases(n)` → `Bars(n × phrase_bars del ajuste)`, `Beats(n)` → `Bars(⌈n/4⌉)`; si no cae en 1/2/4/8/16, el siguiente
  valor de la lista.
- Secuencias que no son de compases enteros: se completa hasta el siguiente compás repitiendo sus pasos desde el principio
  (se ve igual en bucle) *(mi decisión)*.
- La migración se hace al cargar, en Rust, con copia de seguridad del archivo v1 (`library.json.v1`) antes de escribir el
  v2. Si falla, el archivo original queda intacto y la app usa la biblioteca de serie, como hoy con un archivo inválido.

### 6.6 Guardado automático y deshacer

- **Qué cubre deshacer** *(mi decisión)*: lo que vive en la biblioteca y se edita en Sets, Sequences, Palettes y la forma
  de Strip. Los ajustes (Setup, Detection, número de segmentos) se aplican al momento, sin deshacer: cada uno tiene su
  valor por defecto o «Reset to defaults», y en esas pestañas los botones de deshacer/rehacer se ven desactivados con una
  ayuda que lo explica.
- **La selección en vivo queda fuera**: `active_set` y `active_palette` no entran en la pila de deshacer ni en el guardado
  del editor. El editor guarda sin tocarlas (Rust conserva las vigentes al guardar), y ▷ en Sets y el popover las cambian
  por su propio comando. Así deshacer o un guardado del editor nunca cambian el set o la paleta en mitad del show.
- El editor guarda cada cambio con antirrebote (~400 ms *(mi decisión)*) por `save_library`, validado en Rust como hoy.
- Deshacer/rehacer: pila en el editor (UI) de bibliotecas completas, hasta 100 pasos *(mi decisión)*; un deshacer es un
  cambio más que se guarda. Arrastres y pintados cuentan como un paso al soltar.
- Si Rust rechaza un guardado, el estado del editor se mantiene, «Not saved — <motivo>» en rojo y el siguiente cambio
  válido lo guarda todo. Nada se descarta.
- El motor aplica los cambios desde el siguiente compás, como hoy.

### 6.7 Ajustes

- `sensitivity` sigue en 1..=10; el slider de 5 pasos usa 1, 3, 5, 7, 9 (Balanced = 5) *(mi decisión)*.
- Nuevo `help_tooltips: bool` (por defecto `true`).
- `phrase_bars` y la sensibilidad salen del popover; viven en Setup (y la sensibilidad también en Detection).

## 7. Barra de menú y popover

### 7.1 Barra de menú (sustituye spec §7.1)

- **Un solo elemento**: imagen = **glifo de seis segmentos LED** (2×3, fijo, nunca animado) + **icono del momento**;
  título = BPM con un decimal.
- El icono del momento parpadea con el beat. Forzado: insignia «F» junto al icono. Parado: glifo + «—». Tira perdida:
  punto rojo sobre el glifo. Sin reloj ni tira: glifo + «—» y punto rojo.
- Se elimina el segundo elemento de la onda y su animación a 12 fps.
- Legible en barra clara y oscura (imágenes plantilla; el punto rojo, como hoy).

### 7.2 Popover (sustituye spec §7.2)

- Cabecera: BPM grande · «BPM · beat 2» · «phrase: bar 3 of 8» · chip con icono y momento.
- **Lights:** botón redondo ▶; turquesa con brillo suave = encendido («On · following the music»), gris = apagado (tira
  en reposo). Nombre de la tira y estado a la derecha.
- `BRIGHTNESS` deslizador · `MOMENT [Auto | 3 iconos]` · `SET ‹ Club night ›` · `PALETTE ‹ Auto ›` (Auto + paletas del
  set activo).
- Pie: puntos **Clock · Audio · Strip** (gris si falta, rojo si falla; ayuda dice qué hacer; clic = Setup del editor),
  «Open editor» y botón de salir.
- Sin vista de ajustes en el popover y sin slider de sensibilidad.

## 8. Arreglos del e2e

1. **MIDI en caliente:** las fuentes que aparecen después del arranque se ven sin reiniciar (cliente de notificaciones
   CoreMIDI en un hilo con run loop, o equivalente) y la fuente elegida se reconecta sola al volver. Se recuerda la última
   fuente; si no hay ninguna elegida y existe una sola fuente con reloj, se elige *(mi decisión)*. El puerto virtual
   «Deeboo Lighting In» sigue existiendo como alternativa.
2. **Textos:** ningún texto de la app nombra Deeboo MIDI Sync. README, spec §11.5 y el documento e2e dejan de decir
   «apunta MIDI Sync a Deeboo Lighting In».
3. **Segmentos:** `strip.json` guarda los dos números: `reported` (M, lo que informa el controlador) y `segments`
   (N ≤ M, los cableados). El encabezado de DP 61 sigue usando M, como hace Smart Life, y solo se pintan los segmentos
   1…N; N+1…M van siempre apagados *(mi decisión: no se manda al controlador un número que no ha informado)*. Archivos
   viejos: `reported` = `segments`. Cambiar N añade o quita piezas al final de la forma y conserva las demás.
   El asistente muestra lo detectado en un campo editable («Detected 10. How many light up? [8]») con
   identificar al lado; también editable en Strip (§5.4). Spec §11.1 y el doc e2e esperan «el número real».
4. **Asistente:** mientras corre, la cabecera muestra su estado («Connected to dj lights — waiting for paint data»), no el
   «not connected» del runtime. Identificar lleva el texto «Click a number to light that segment on the strip.».
5. **Micrófono:** no enumerar dispositivos abriendo streams; solo se pide permiso cuando empieza la captura. Verificar con
   la app lanzada desde Finder que el permiso se queda.
6. **Log:** sin `strip.json` no se registra «strip file ignored…» (como mucho, «no strip saved yet» a nivel debug).
7. **Memoria:** el proceso Rust superó 131 MB en play (objetivo spec §13.1: < 40 MB). Medir con el editor cerrado y
   abierto, encontrar la causa (webview en proceso, buffers de audio, runtime tokio, rustuya…) y arreglarla. Si el
   objetivo no es alcanzable con WebKit en el mismo proceso, proponer al usuario un objetivo nuevo con datos; no se
   cambia la spec sin su visto bueno.
8. **Paleta en Auto:** cubierto por §6.1.

## 9. Restricciones

- **Todo texto de la app en inglés, siempre**: etiquetas, ayudas, avisos, errores mostrados, confirmaciones, estados
  vacíos y la barra de menú. Esta spec está en español, pero los textos citados entre «» son los textos en inglés
  exactos; cualquier texto nuevo que el constructor añada va en inglés.
- Todo lo de `CLAUDE.md` y la skill `security`: CSP estricta, IPC validado en Rust, capacidades mínimas, secretos solo en
  `SecretString`, sin red desde el webview, sin polling, sin asignaciones ni logs en el callback de audio.
- Iconos SVG copiados al repo (licencia compatible anotada); sin dependencias nuevas de UI salvo que el plan lo
  justifique.
- TDD; `just precommit` verde; cada cambio de convención actualiza su skill (`ui-code` para el lenguaje visual).
- Rendimiento: el editor sin animaciones continuas cuando la ventana no está visible; la lectura en vivo y la línea de
  tiempo de Detection se alimentan de los eventos que ya existen (por beat), sin sondeos.

## 10. Pruebas de aceptación (se añaden al e2e)

- Ningún texto ni icono es emoji; ningún texto nombra MIDI Sync.
- Arrastrar una secuencia de Calm a Rhythm o Build-up no hace nada; a Calm la añade.
- Editar, deshacer y rehacer en cada pestaña; cerrar y abrir el editor conserva el último estado guardado.
- Una secuencia no puede quedar en 3½ beats.
- La paleta rota en Auto según el set; ‹ › la fija hasta volver a Auto.
- Arrancar la fuente MIDI después de la app: aparece y engancha sin reiniciar.
- Cambiar los segmentos a 8 en Strip: forma y secuencias se adaptan.
- La barra de menú es un solo elemento, legible en barra clara y oscura.
- Biblioteca v1 del usuario migrada sin perder secuencias, paletas ni sets.
- `just perf 600`: los objetivos de spec §13.1 (o los nuevos aprobados por el usuario).

## 11. Lo que queda al constructor

- Cómo componer las imágenes de la barra de menú (pre-renderizadas o compuestas en Rust).
- Cómo detectar fuentes CoreMIDI en caliente con lo que ofrece `midir` o `coremidi`.
- Estructura de componentes del editor y de la pila de deshacer.
- Qué SVG concretos usar, con licencia compatible.
- Cómo pintar las gráficas de Detection (SVG o canvas) con un búfer fijo de 32 beats.
