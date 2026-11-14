# Secuencias autogeneradas, paletas, fundidos en Calma y niveles aprendidos persistentes

Fecha: 2026-11-14. Complementa la spec principal (`2026-11-06-deeboo-lighting-design.md`, §6.4 patrones y §6.2 detector).
Las decisiones salen de la sesión de diseño con páginas de propuestas (`.superpowers/brainstorm/generated-proposals-3.html`,
`palette-final.html`). El texto de la app sigue en inglés.

## 1. Intención

Los patrones actuales (Halves, Ends↔centre, Odd/even, Quarters, Snake, Mirror, Drift, Sparse, Group flash, Fast snake,
Strobe) no gustan: Snake y Mirror son malos, ninguno de Calma sirve y en Subidón solo vale Strobe. Se sustituyen por 35
secuencias elegidas una a una por el usuario, con reglas claras por momento:

- **Rhythm**: movimiento en el beat jugando con la oscuridad, como en una discoteca; blancos en los beats pares. Cambia
  solo en beats enteros, nunca en medios beats ni más rápido.
- **Calm**: mucha luz, casi toda la tira encendida, cambios lentos (cada cambio dura 2 compases o más) y fundidos suaves.
- **Build-up**: cambios rápidos, muchos segmentos y blancos; se prefieren las que aceleran.

No-objetivos: efectos nuevos en el editor aparte del interruptor de fundido; patrones configurables por parámetros
(desaparecen los parámetros aleatorios); fundidos en Rhythm o Build-up (allí el corte seco con la oscuridad es la idea).

## 2. Partes de la tira

Todas las secuencias se definen sobre grupos contiguos **a lo largo de la tira** (orden de segmentos 1..N), que es lo que
el usuario vio en la vista previa:

- **4 partes p1..p4**: si N es múltiplo de 4, cuatro partes iguales. Si N no es múltiplo de 4 pero sí de 3 (6, 9): tres
  secciones iguales s1, s2, s3 con p1 = s1, p2 = s2, p3 = s3, p4 = s2 (la sección central hace de partes 2 y 4). En otro
  caso, partes simétricas `[b, b+e, b+e, b]` con `b = ⌊m/4⌋`, `e = (m − 4b)/2`, `m` = N si es par o N−1 si es impar; con N
  impar el segmento central se comparte entre p2 y p3. Orden: primero múltiplo de 4 (12 → cuatro partes de 3), luego múltiplo de 3. Ejemplos: 8 → 2+2+2+2; 6 → 2/2/2 (tres secciones); 10 → 2+3+3+2;
  7 → 1+2(+c)+(c+)2+1.
- **Extremos E** = p1 ∪ p4 (con tres secciones: s1 ∪ s3). **Centro Cn** = p2 ∪ p3 (con tres secciones: s2).
- **Mitades H1/H2**: primera y segunda mitad de la tira; con N impar el central se comparte.
- **Pares/impares**: segmentos impares (1, 3, …) y pares.

Decisión mía: "a lo largo de la tira" en vez de los grupos espaciales actuales (`Groups::left_right`, `centre_ends`
calculados con la forma): con una tira recta dan lo mismo y es lo que se aprobó; los grupos espaciales quedan para el
editor y los demás usos que ya tienen. Con N < 4 las partes vacías simplemente no pintan; una secuencia nunca falla.

## 3. Las 35 secuencias

Notación: roles **P** principal, **C** contraste, **S** suave, **W** blanco, **–** apagado. "E P" = extremos en P y el
resto apagado; "E W/S" = extremos en W sobre fondo S; "todo P" = toda la tira. Los ids (R2, C1, B1…) son los de las páginas
de propuestas y no se muestran; el nombre inglés sí.

### Rhythm (cada paso dura 1 beat)
| Id | Nombre | Pasos |
|---|---|---|
| R2 | Cross pairs | p1+p3 P, p2+p4 P, p1+p3 P, p2+p4 P, p1+p3 C, p2+p4 C, p1+p3 C, p2+p4 C |
| R3 | Colour and dark | todo P, –, todo C, – |
| R4 | White backbeat | p1+p3 P, todo W, p2+p4 C, todo W |
| R6 | Outside in | E P, Cn P, E C, Cn W |
| R7 | Halves blink | H1 P, H2 W, H1 P, H2 W (los beats 2 y 4 siempre en blanco) |
| R10 | Cross pairs + white backbeat | p1+p3 P, p2+p4 W, p1+p3 C, p2+p4 W |
| R11 | Colour, dark, white | todo P, –, todo C, todo W |
| R12 | Outside in, with dark | E P, –, Cn P, –, E C, –, todo W, – |
| R15 | Mirror with white | E P, Cn C, E W, – |
| R16 | Colour and dark, two bars | todo P, –, todo C, –, todo S, –, todo W, – |
| R19 | Every other beat | p1+p3 P, –, p2+p4 C, – |
| R21 | Halves + white backbeat | H1 P, todo W, H2 C, todo W |
| R23 | Outside in, two bars | E P, Cn P, E C, Cn C, E W, –, Cn W, – |
| R24 | Cross pairs, white turnaround | p1+p3 P, p2+p4 P, p1+p3 C, todo W |
| R25 | Ends, centre, all, dark | E P, Cn C, todo W, – |

### Calm (duraciones en compases de 4 beats; todos los cambios con fundido, §4)
| Id | Nombre | Pasos |
|---|---|---|
| C1 | Wash and soften | todo P 2, todo S 2 |
| C2 | Ends white, then wash | E W 2, todo S 2, Cn P 2, todo S 2 |
| C6 | Slow fade to dark | todo P 2, todo S 2, H1 S 2, – 2 |
| C7 | Colour and white | todo P 2, todo C 2, E W/S 2, todo S 2 |
| C9 | Centre to ends | Cn W/S 2, todo P 2, E W/S 2, todo S 2 |
| C10 | Colour cycle | todo P 2, todo C 2, todo S 2, Cn W/P 2 |
| C11 | Fade and return | todo P 2, todo S 2, Cn S 2, todo P 2 |
| C13 | Slow white breath | todo S 4, todo W 2, todo P 2 |
| C14 | Long wash | todo P 4, todo S 4 |
| C16 | White ends, slowly | E W/S 4, todo P 4 |

Pasos de más de 4 beats se representan como pasos de 4 beats repetidos (`StepLen` no cambia; el máximo de 16 compases y
128 pasos de `library.rs` se respeta: la más larga, C13/C14/C16, son 8 compases).

### Build-up (½ = medio beat)
| Id | Nombre | Pasos |
|---|---|---|
| B1 | Strobe | todo W ½, – ½ |
| B5 | Converge | E W ½, Cn P ½, todo C ½, – ½ |
| B8 | Colour strobe | todo W ½, todo P ½ |
| B9 | Odd / even strobe | impares W ½, pares W ½ (el resto apagado) |
| B10 | Converge, then strobe | B5 dos veces, luego (todo W ½, – ½) ×4 |
| B11 | Halves, accelerating | (H1 W + H2 P 1, H1 P + H2 W 1) ×2, luego lo mismo a ½ ×4 |
| B14 | Odd / even, accelerating | (impares W + pares P 1, al revés 1) ×2, luego a ½ ×2, luego (todo W ½, – ½) ×2 |
| B15 | Strobe, accelerating | (todo W 1, – 1) ×2, luego (todo W ½, – ½) ×4 |
| B16 | Converge, accelerating | (E W, Cn P, todo C, –) a 1 beat, luego lo mismo a ½ dos veces |
| B19 | Four-part chase, accelerating | p1, p2, p3, p4 en P a 1 beat, luego p1..p4 dos veces en W a ½ |

Nota: B15 se define según su descripción. En la página de propuestas su animación tenía un fallo y se veía igual que B1.

### Uso
- `SeqRef::Pattern` pasa a referirse a una de estas 35 (enum nuevo, nombres ingleses de las tablas). La lista de patrones
  del editor (Sets) muestra estas, agrupadas por momento.
- **"Any pattern"** (`SeqRef::Generated`) elige una de las de su momento al azar en cada rotación, nunca la misma dos
  veces seguidas. Desaparecen los parámetros aleatorios (`Params`).
- Las secuencias se calculan con el número de segmentos actual, como hoy: se adaptan solas al cambiar la tira.
- `SeqRef::Pattern(p)` es determinista (sin semilla ni azar). "Any pattern" recuerda la última elegida por momento.
- Toda secuencia se rellena a compases enteros repitiendo sus pasos (`pad_to_bars`, que sale de `migrate.rs` a un sitio
  común), para que una copia guardada pase `validate`.
- Las bibliotecas nuevas no traen secuencias de usuario sembradas: los 35 patrones ya están disponibles.
- **Migración a esquema v3** (con copia de seguridad previa, como en las migraciones existentes):
  - Un patrón antiguo (`halves`, `snake`, …) pasa a "Any pattern" del mismo momento.
  - `fade` ausente se lee como activado.
  - Se añaden las 9 paletas nuevas que falten (§5).
  - Se quitan las paletas de serie antiguas (Sunset, Ocean, Club, Fire and ice, Forest, Aurora, …) que ningún set ni
    secuencia usa (decisión del usuario).
  - Las usadas y las creadas por el usuario se quedan. Si se pasa de `MAX_PALETTES`, no se añaden más.
  - Un patrón desconocido nunca hace fallar la carga.

## 4. Fundidos en Calma

- Todo cambio entre pasos de una secuencia de Calma es un fundido: de color a color, un fundido cruzado; hacia o desde
  apagado, un fundido de brillo. Se aplica siempre a las secuencias autogeneradas de Calma.
- Las secuencias de Calma del usuario también funden por defecto. El editor muestra un interruptor **Fade** solo en
  secuencias de Calma, activado por defecto; al desactivarlo los cambios son secos. Se guarda como `fade: bool` en la
  secuencia (ausente en archivos antiguos = activado).
- No hay más ajustes: ni duración ni curva en la app.
- Decisiones mías:
  - Duración: 2 beats desde el inicio del paso nuevo (4 fotogramas, uno cada medio beat; el último es el color final y
    se mantiene el resto del paso). Si el paso dura menos de 4 beats, el fundido ocupa la mitad del paso.
  - También funde la vuelta del último paso al primero y el cambio a otra secuencia de Calma por rotación.
  - Color intermedio: interpolación lineal en RGB por segmento, convertida a HSV para el DP 61. Así no se pasa por tonos
    que no están en la paleta, como pasaría interpolando el tono.
  - Los segmentos que no cambian no se reenvían, y cada fotograma pasa por `scene::diff` y el planificador como
    cualquier cambio. Sin bucles de sondeo: los fotogramas se programan como los pasos.
  - Entrar o salir de Calma y el cambio de paleta siguen siendo cortes secos.
  - Los fotogramas pendientes se descartan si llega el siguiente (`Scheduler::plan` ya lo hace): es lo buscado.
  - La simulación del editor (`Simulation`, `player.ts`) también dibuja el fundido, para que el interruptor se vea.
  - Diseño del interruptor: en la fila de cabecera de la secuencia, tras "Preview palette": etiqueta **Fade**, un switch
    y el texto "Smooth changes between steps". Solo aparece si el momento es Calm. Maqueta:
    `.superpowers/brainstorm/calm-fade-toggle.png`.
- Riesgo a comprobar en la e2e: con brillo muy bajo algunas tiras cambian de tono o parpadean. Si pasa, el fundido para en
  un mínimo y luego apaga.

## 5. Paletas

Las paletas de serie pasan a ser estas 9 (HSV con H 0-360, S y V 0-1000; principal / contraste / suave):

| Nombre | Principal | Contraste | Suave |
|---|---|---|---|
| Ocean | 190,1000,1000 | 320,900,1000 | 190,300,1000 |
| Laser | 125,1000,1000 | 325,1000,1000 | 160,600,1000 |
| UV | 257,1000,1000 | 177,1000,1000 | 282,500,1000 |
| Deep sea | 197,1000,1000 | 157,1000,1000 | 182,400,1000 |
| Ember | 28,1000,1000 | 53,1000,1000 | 43,500,1000 |
| Green | 130,1000,550 | 130,1000,1000 | 130,450,1000 |
| Lavender | 270,550,1000 | 220,450,1000 | 220,250,1000 |
| Sorbet | 45,650,1000 | 350,650,1000 | 350,300,1000 |
| Peach and mint | 20,550,1000 | 150,550,1000 | 40,250,1000 |

**New palette** en el editor crea una paleta con la regla "análoga pastel": tono al azar h, principal (h, 600, 1000),
contraste (h+40, 600, 1000), suave (h+20, 250, 1000). El usuario la edita después como hoy.

## 6. Niveles aprendidos del detector, persistentes por entrada de audio

- Las referencias son hoy los tres historiales del detector (`kick_hist`, `sub_hist`, `high_hist`, 64 valores como
  mucho cada uno); la referencia es su mediana. Se guardan esos tres historiales (nuevo `export`/`import` del detector).
  Al restaurar, el estado vuelve a Rhythm con las referencias ya calientes.
- Las referencias que aprende el detector se guardan por entrada de audio, con la misma clave que la configuración por
  entrada (`detectorByInput`). Hoy se guardan solo en memoria durante la sesión; ahora sobreviven a reinicios.
- Al arrancar o al volver a una entrada se restauran sus referencias. Una entrada nueva empieza sin referencias, como hoy.
- Archivo propio y pequeño junto a `settings.json`, con versión, validado al leer, como mucho 16 entradas (sale la escrita hace más
  tiempo; "System default" nunca sale) y valores acotados. Un archivo inválido se aparta (`.bad`, como `settings.rs`) y se sigue sin referencias, sin perder datos.
- Se guarda como mucho una vez cada 30 s mientras cambia y siempre al salir, desde el hilo del motor. Nada en el
  callback de audio.

## 7. Comprobaciones

- Tests de `parts` para N = 1..16: partición, simetría y regla de tres secciones.
- Una prueba por secuencia contra su tabla, a N = 6, 8 y 10.
- Rhythm sin pasos de medio beat. Calm sin cambios de menos de 2 compases.
- Fundidos con tiempo simulado: fotogramas cada medio beat durante 4 beats, el último igual al paso, cortes secos al
  cambiar de momento. Interruptor Fade en el editor (Vitest).
- Ritmo de mensajes: a 140 BPM las de Build-up con dos colores por cambio (B9, B11, B14) necesitan 2 mensajes cada medio
  beat, por encima de los 125 ms de separación. Se acepta la regla de descarte actual del planificador. Un test fija que
  no se acumula retraso.
- Persistencia de referencias: guardar, reiniciar, restaurar; archivo corrupto apartado; límite de 16 entradas.
- La spec principal (§6.4) y las skills afectadas (`tuya-protocol` si cambia algo del envío, `ui-code` por el
  interruptor) se actualizan en el mismo cambio.
