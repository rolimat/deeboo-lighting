---
name: writing-tests
description: Use when adding a feature or fixing a bug in any language: TDD loop, where each kind of test belongs, naming, simulated time and fixtures for light-core, property tests, Vitest with a mocked api for the UI.
---

# Writing tests

## Loop
1. Write the failing test; run `just test`; see it fail for the right reason.
2. Implement the minimum; run `just test`; see it pass.
3. Refactor, then `just precommit`.
Bug fix: first a test that reproduces it.

## Where tests go
| Kind of logic | Where | How |
|---|---|---|
| Beat/bar/phrase, moments, generator, scheduler, DP 61, classifier | `crates/light-core/tests/*.rs` | Simulated time, fixed fixtures, no I/O |
| Parsers (DP 61, MIDI, config, cloud responses) | same | `proptest`: never panic, round-trip |
| Secret handling | the module owning the secret | Assert logs and `Debug` output contain no secret |
| App glue (settings, DTO, credentials file) | unit tests in the module | pure helpers, temp dirs, check 0600 |
| UI views | `ui/src/*.test.ts` | Vitest + jsdom, mocked `api` |
| Audio analysis | `crates/light-core` | Synthetic signals only |

Real MIDI, audio and the strip are never used in automated tests; they are the manual acceptance tests
(spec §11, macOS host). Recordings and labels never enter the repo (music rights): CI uses synthetic signals.

## Performance test
Analysing 10 min of synthetic 48 kHz audio must take < 1 s on one thread (spec §13). Run it in CI with a x3 margin.

## Generated fixtures
`ui/src/editor/sequences/player-fixture.json` is what the real engine shows, including each step's time `t` (UI simulation must match). The Rust test
`crates/light-core/tests/player_fixture.rs` fails on drift; regenerate with
`UPDATE_FIXTURES=1 cargo test -p light-core --test player_fixture` (in Docker) and commit the JSON.

## Naming
`snake_case_behaviour` stating the outcome, e.g. `pencil_red_on_odd_segments_matches_fixture`,
`second_command_is_never_concatenated`. One behaviour per test.

## Details
- Rust helpers and timing: [references/rust-simulated-time.md](references/rust-simulated-time.md) (read when writing `light-core` tests).
- UI mocking and events: [references/ui-vitest.md](references/ui-vitest.md) (read when writing UI tests).
- Detector position rules: pass `Position::at(beat, false, PhraseBars::Eight)` to `on_beat` and set `DetectorParams::rules` explicitly (the default turns all four rules on; tests that need a rule off say so).

## Checklist
- [ ] Test failed before the code existed.
- [ ] Deterministic: no sleeps, no wall clock, no real ports, no network.
- [ ] `just precommit` green.
