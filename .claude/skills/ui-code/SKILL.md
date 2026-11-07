---
name: ui-code
description: Use when editing anything in ui/: TypeScript views, api.ts, styles.css, index.html, or any user-visible text in the popover and editor window.
---

# UI code

The UI does not exist yet (arrives in a later plan). These rules apply from its first file.

## Rules
- TypeScript `strict: true`; no `any`. ESLint, Prettier and `tsc` are gates in `just lint` / `fmt-check`.
- No frameworks: plain TS modules and DOM APIs. Plain CSS with custom properties (tokens, same as Deeboo MIDI Sync) in `ui/src/styles.css`; no hard-coded colours or radii outside the token block.
- All text is English. "Smart Life" appears only in the connection flows; never name other third-party software.
- Icon-only buttons need an `aria-label`.
- The UI only talks to Rust through `ui/src/api.ts`; views never import `@tauri-apps/api` directly.
- Webview loads only bundled files: strict CSP, no inline scripts or styles, no remote origins. The QR is rendered locally.
- Never show a secret. The key is shown masked; nothing secret enters the DOM, console or state objects.
- Status events arrive only while the popover is visible (max 30 Hz). Write to the DOM only when a value changed (never rewrite text under the pointer; WebKit drops the click).
- The editor window is created on open and destroyed on close. Its simulation animates with `requestAnimationFrame` only while visible; no timers or polling.
- Tests are Vitest + jsdom with `api` mocked; see the `writing-tests` skill.

## Details
Tokens, component classes and UI strings: [references/styles-and-copy.md](references/styles-and-copy.md) (read before changing styles or copy).

## Checklist
- [ ] New text is in English and listed in the reference.
- [ ] New colour or size is a token.
- [ ] Interactive control has an accessible name and a test.
- [ ] A new command or state field is added to `api.ts` types and to the Rust DTO together.
