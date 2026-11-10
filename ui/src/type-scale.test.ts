import { expect, it } from "vitest";

// Every file that can set a text size, as raw text (test files and the token file excluded).
const files = Object.entries(
  import.meta.glob(["./**/*.{ts,tsx,css}", "!./**/*.test.*", "!./tokens.css"], {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>,
);

const RAW_UTILITY = /\btext-(xs|sm|base|lg|[2-9]?xl|\[[\d.]+(px|rem|em)\])(?![\w-])/;
const RAW_CSS_SIZE = /font-size:\s*[\d.]+(px|rem|em)\b|font:\s*[\d.]+(px|rem|em)\b/;

it("text_sizes_come_only_from_the_role_tokens", () => {
  expect(files.length).toBeGreaterThan(20);
  const bad = files.flatMap(([f, src]) =>
    src
      .split("\n")
      .filter((l) => RAW_UTILITY.test(l) || RAW_CSS_SIZE.test(l))
      .map((l) => `${f}: ${l.trim()}`),
  );
  expect(bad).toEqual([]);
});
