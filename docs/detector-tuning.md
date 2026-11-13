# Moment detector: tuning notes

`crates/light-core/src/detect/moment.rs` decides Rhythm / Calm / Build-up once per beat. The rules and parameters
below were fitted on one labelled DJ session (2026-11-07). They are a starting point for one style of set, not
a universal truth: re-run the probe on new material before trusting them.

## Method

1. **Labels v1.** The user marked, by ear, where Calm, Build-up and Rhythm begin in nine stretches of the session
   (`tools/detect-probe --labels`, JSON `[{"t", "moment"}]`, git-ignored with the recording).
2. **Corrections, labels v2.** Parameters alone could not pass the gate on v1, and several labels turned out
   to be off by a bar or mark the wrong moment. The user corrected them; v2 is the reference. Recordings and
   labels are never committed.
3. **Replica.** The per-beat features (`kick_db`, `high_db`) of the whole session were dumped once and the rules were
   iterated in a Python replica (hundreds of variants plus a +-25 % sensitivity sweep per parameter), scored exactly
   like the probe. The chosen rule set ("A2") was then ported to Rust.
4. **Parity.** On identical features the Rust detector and the replica produce the same 219 transitions. On the
   replica's 0.1 dB-rounded features, 25 of 26 label offsets matched within 0.6 s. The 26th is a rounding effect of the replica's input.
5. **Scoring.** A hit is a transition to the labelled moment within +-4 s (Calm: +-6 s, because the cue is the lights
   settling and about 4 s late is acceptable). Labels closer than 120 s form an episode; any other transition inside an
   episode window is a false alarm, except a Calm of at most 8 beats followed by Rhythm.
   `detect-probe` prints one `label ...` line per label with the offset of the nearest detection.

## Rules (A2)

References are the medians of kick and highs over the last 64 groove beats recorded in Rhythm (`learn_db`).
A *deficit* is a beat's level minus its reference.

| rule | parameters | musical reason |
|---|---|---|
| Rhythm to Calm: kick gone or highs gone, 2 beats in a row | `calm_kick_db` 16, `calm_high_db` 12, `calm_after_quiet` 2 | A break takes the kick 19 to 29 dB down; fades and filtered grooves stop short of that. Some breaks keep a bassline, so the highs also count. |
| ... or thin: 4-beat mean kick 5 dB and highs 8 dB below | `thin_kick_db` 5, `thin_high_db` 8 | A groove that loses both bands a little at once is a break even though neither is gone. |
| References learn only groove beats | `learn_db` 16 | A slow fade must not drag the reference down with it. |
| Calm to Rhythm, drop: a band jumps and stays | `jump_db` 10, `back_db` 4, `near_db` 8, `rhythm_after_full` 2 | A drop slams in: the kick rises 10 dB or more within 4 beats and then stays at full level. Fills never do both. |
| ... highs-only return needs a whole bar | `hats_after_full` 4 | Hats back for 4 beats is a groove; a fill in a break lasts 2 or 3. |
| ... slow return | `slow_return_beats` 32, `near_db` 8 | A groove that fades in, or a quieter track, never jumps. Eight bars near the references is a groove. |
| Calm to Build-up: three rising bars (A2; replaced by B1, see below) | `buildup_rise_db` 4 (removed) | Risers climb for several bars; a fill rises for one. |
| Rhythm to Build-up: kick ducked, highs up, 2 beats | `groove_buildup_kick_db` 6, `groove_buildup_high_db` 8 | Build-ups that start inside the groove: kick pulls back while the hats open. |
| Build-up to Calm: highs fall back without a drop | `collapse_db` 8 | The riser was abandoned. |
| Build-up to Rhythm: same drop rule, exempt from `min_state_beats` | `buildup_max_beats` off (removed in B1) | A drop must be immediate; timeouts cost more than they saved. |
| No change within 4 beats of the last one (a quick move on a phrase's beat 1 and its beat-2 confirm/undo are exempt, see Clock positions) | `min_state_beats` 4 | Avoids flicker. |

The editor's Detection panel stores `calm_kick_db` (8-30), `calm_high_db` (6-24) and `riser_db` (3-15) in
settings (defaults 16 / 12 / 7, whole dB); the popover's sensitivity then shifts the two calm ones by `5 - s` dB
(`settings::detector_params`); the rest are fixed. `MomentDetector::set_thresholds` applies all three live and keeps
the learned references and the riser's bar history. While the panel is open the engine emits the `detector` event
(per beat, at most 15 Hz) to the editor window; otherwise nothing is emitted.

## Scores (labels v2, Rust release probe)

| | result |
|---|---|
| Calm (+-6 s) | 8 of 9 (replica on rounded features: 9 of 9) |
| Rhythm (+-4 s) | 9 of 9 |
| Build-up (+-4 s) | 2 of 8 |
| False alarms | 0.67 per episode (6 in 9 episodes) |

The Calm miss is a detection about 7 s before the label. Build-up is weak by design: most labelled build-ups
are short fills or risers inside a break that the three-bar rule does not accept; they are not worth false alarms.

## Known limits

- **Kick versus bass.** The kick band (about 80 Hz) cannot tell a kick from a bass note. A break that keeps a
  bassline looks like groove to the kick path; only the highs path (or the thin rule) catches it.
- **Stale references.** References learn only in Rhythm. After a track whose kick is much louder, a quieter
  next track can sit more than `near_db` below the old reference and stay in Calm until a jump reaches it.
  The slow return (8 bars) is the safety valve; re-seeding the references there is future work.
- **Cold start.** A beatless intro (no kick yet) shows Rhythm, the initial state. The first 8 beats that carry
  features seed the references, so the detector needs about 8 beats of real music before its judgements mean
  anything. The app should not reset the detector on Stop: it would throw away the learned references.
- Fitted on one session of one genre at about 125 BPM.

## Single-song validation (rule set B1)

2026-11-07. Five more songs ran through the probe (BPM from the file tags: 125, 122, 124, 125, 128); the user
reviewed the A2 timelines of three of them by ear and corrected what they heard. The two others (songs 4 and 5) ran
but were not reviewed. The corrections became rule set B1 in the replica, then were ported to Rust and checked
transition by transition against the replica (all six recordings: same transitions, same count, within 0.6 s).

| change | parameters | musical reason |
|---|---|---|
| Highs-only quiet needs a weak kick | `calm_kick_db` 16, `calm_high_db` 12, `back_db` 4 | A drop without hats is still a groove while the kick pumps; a break where the bass fills the kick band still has a weaker kick band. |
| Sub reference; a return needs the bass back | `sub_back_db` 10 | A drop brings the bass back; a filtered or half-time kick inside a break without bass is not the groove. |
| Teaser: after a return, kick deficit mean of 4 beats back to Calm | `teaser_beats` 16, `teaser_kick_db` 6 | A one-bar kick tease inside a break is not the drop. The start of the track in Rhythm is not a return. |
| Build-up from Calm: riser | `riser_bars` 4, `riser_db` 7, `riser_level_db` 6 | Bar means of the highs rise monotonically (0.5 dB tolerance) and bring the highs back towards the groove level. A riser far below the groove is not tension. |
| Build-up from Calm: approach | `approach_calm_beats` 32, `approach_above_db` 2, `approach_rise_db` 2, `approach_bars` 8 | Late in a break the highs are already above the groove level and still climbing: subtle tension towards the drop. |
| Collapse needs 4 beats, then a refractory | `collapse_beats` 4, `refractory_beats` 16 | The one- or two-beat silence before a drop is not a collapse; a failed riser does not flap. |

The 3-rising-bars rule (`buildup_rise_db`) and `buildup_max_beats` are gone.

The probe now scores a Build-up label as a hit when a Build-up is detected from 8 s before it until the next label
(the drop), and a Build-up that leads into Rhythm is never a false alarm. A Build-up is tension towards the drop:
what matters is that it is seen before the drop, not that it starts within a few seconds of the label.
Calm and Rhythm scoring is unchanged.

| | A2 (old +-4 s Build-up metric) | B1 (new Build-up metric) |
|---|---|---|
| Session Calm (+-6 s) | 8 of 9 | 7 of 9 |
| Session Rhythm (+-4 s) | 9 of 9 | 9 of 9 |
| Session Build-up | 2 of 8 | 8 of 8 |
| Session false alarms | 6 | 9 |
| Songs 1 to 3 Build-up | 0 of 3 | 3 of 3 |

Trade-off: the session's Calm at 24:44 moves from +2.9 s (hit) to +6.3 s (miss by 0.3 s), a price of the stricter
quiet rule; 15:51 stays 7.0 s early as in A2. The new Build-up metric is not comparable to the old one, so the
Build-up row also reflects the metric.

What B1 still cannot do:

- The kick-vs-bass limit remains: bass-only breaks where the kick band drops less than 8 dB are not seen, and a
  16-second break after a slow kick fade in song 1 is missed.
- It can show a short Build-up inside a long break that collapses back to Calm after 2 to 4 s (a riser or crash
  that does not lead to the drop).
- Fitted on the session plus three reviewed songs, all at 122 to 128 BPM.

## Second song review (rule set B2)

2026-11-08. The user reviewed all five songs again on the B1 timelines. Three changes fix the build-ups that B1 cut
short, without changing any session label result, and make the detector robust to a stale groove reference.

| change | parameters | musical reason |
|---|---|---|
| Slower collapse | `collapse_beats` 4 to 16 | A riser that pauses or changes texture for a bar or two keeps the tension: only four bars with the highs well under their entry level end a Build-up. |
| Faster slow return | `slow_return_beats` 32 to 24 | After a long break the groove reference can be stale (the next track is a few dB quieter): six bars of a steady near groove are enough to return. |
| Build-up cap | `buildup_max_beats` 96 (back, new meaning) | A Build-up is the last bars before a drop; one that runs 24 bars without a drop was the break itself. It ends in Calm and starts the refractory like a collapse. |

Beat agreement with the user's labels on songs 1 to 5 (replica): mean 90.2 % (B1) to 92.6 % (B2); per song
91/81/100/97/82 to 90/87/100/98/88 %. Session unchanged: Calm 7 of 9, Rhythm 9 of 9, Build-up 8 of 8.

What B2 still cannot do:

- A Calm where only the kick leaves and the bass stays (two songs) is not seen.
- A Build-up made by fading the kick out over 8 bars (one song) is not seen: in the session the same fade leads into a Calm.
- A two-beat kick-and-bass hit at the end of a Build-up shows Rhythm for about 2 s.

Tried and rejected:

- A sudden sub-and-kick drop as Calm: fixes one song, makes the session miss a drop.
- A kick-fade Build-up rule: turns three session breaks into Build-ups.
- A relative drop on stale references: R/C flicker in long session breaks.
- Kick attack and on-beat pulse features: they do not separate kick from bass.

## Clock positions (plan 10)

With a clock position (spec §6.2.1) the rules above change in five places:

- Riser bars close on the clock's beat 4, not every 4 beats; a partial bar at start or after a gap never counts.
- `min_state_beats` does not bind a quick move on a phrase's beat 1; beat 2 confirms it or undoes it.
- A Build-up drops to Rhythm on a full beat 1 of a bar with a kick jump.
- Soft break: a phrase's bar 1 (four clocked beats) whose mean kick is 5 dB or more down, sub 3 dB or more down and highs within 2 dB of their reference is Calm from Rhythm (`soft_kick_db`, `soft_sub_db`, `soft_high_db`); that Calm has no slow return and Rhythm comes back with the kick jump or 2 full beats.
- In a phrase's second half 3 rising bars are enough. Measured alone (`--rules riser`) this depends on the phase of the 4-beat counter, because without `aligned_bars` the bars are not clock-aligned.
