# Rust simulated time and fixtures

`light-core` is pure and deterministic: tests pass time in explicitly (microseconds, monotonic) and nothing reads a clock.
The API does not exist yet; follow these conventions as the crate grows.

- One monotonic microsecond value feeds the core. Tests advance it by hand; never `sleep`.
- Shared helpers go in `crates/light-core/tests/common/mod.rs` (clock pulse feeders, scene builders).
- At 125 BPM a pulse is exactly 20 000 us (24 PPQN); 100 BPM is 25 000 us. Assert BPM with a tolerance.
- DP 61 payloads from the `tuya-protocol` skill are fixed hex fixtures: encode and compare bytes, decode and round-trip.
- The scheduler is tested on a simulated timeline: assert spacing between messages, one command per message, and which updates are dropped.
- `unwrap`/`expect` are allowed in tests only.
