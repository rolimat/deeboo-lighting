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
| Calm to Build-up: three rising bars | `buildup_rise_db` 4 | Risers climb for several bars; a fill rises for one. |
| Rhythm to Build-up: kick ducked, highs up, 2 beats | `groove_buildup_kick_db` 6, `groove_buildup_high_db` 8 | Build-ups that start inside the groove: kick pulls back while the hats open. |
| Build-up to Calm: highs fall back without a drop | `collapse_db` 8 | The riser was abandoned. |
| Build-up to Rhythm: same drop rule, exempt from `min_state_beats` | `buildup_max_beats` off (`u32::MAX`) | A drop must be immediate; timeouts cost more than they saved. |
| No change within 4 beats of the last one | `min_state_beats` 4 | Avoids flicker. |

Plan 3 maps a sensitivity setting onto `calm_kick_db` and `calm_high_db`; the rest are fixed.

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
- Fitted on one session of one genre at about 125 BPM.
