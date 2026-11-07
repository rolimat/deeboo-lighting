# UI tests (Vitest + jsdom)

- Mock the whole `api` module so no Tauri code runs: `vi.mock("./api")`, every command as `vi.fn()`, and the event subscription resolving to an unsubscribe function. Mock `@tauri-apps/api` the same way for `main.ts`.
- Mount into a fresh `<div id="app">` per test and call the returned `update(state)` with a full state object.
- Query by `aria-label` and by class, not by position.
- Errors from a command: `mockRejectedValue("message")`, await, assert the text is visible.
- Clean up listeners between tests (unmount, or `vi.restoreAllMocks()`).
- The editor simulation animates with `requestAnimationFrame`: fake it and step frames by hand.
