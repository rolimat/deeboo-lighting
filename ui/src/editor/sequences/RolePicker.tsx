import type { Hsv, Palette } from "../api";

export const ROLES = ["principal", "contraste", "suave", "white", "off"] as const;
export type Role = (typeof ROLES)[number];

export const ROLE_LABEL: Record<Role, string> = {
  principal: "Principal",
  contraste: "Contraste",
  suave: "Suave",
  white: "White",
  off: "Off",
};

/** HSV (H 0-360, S/V 0-1000) to a CSS colour. */
export function css({ h, s, v }: Hsv): string {
  const sat = s / 1000;
  const val = v / 1000;
  const l = val * (1 - sat / 2);
  const sl = l === 0 || l === 1 ? 0 : (val - l) / Math.min(l, 1 - l);
  return `hsl(${h} ${Math.round(sl * 100)}% ${Math.round(l * 100)}%)`;
}

export function roleColour(role: string, p: Palette): string {
  switch (role) {
    case "principal":
    case "contraste":
    case "suave":
      return css(p[role]);
    case "white":
      return "#fff";
    default:
      return "var(--color-bg)";
  }
}

/** Chooses the role the grid paints with. */
export function RolePicker({
  palette,
  value,
  onChange,
}: {
  palette: Palette;
  value: Role;
  onChange: (r: Role) => void;
}) {
  return (
    <div class="flex gap-2" role="group" aria-label="Role">
      {ROLES.map((r) => (
        <button
          type="button"
          key={r}
          aria-pressed={r === value}
          onClick={() => onChange(r)}
          class={`flex items-center gap-2 rounded-card border px-3 py-1 ${r === value ? "border-text" : "border-border"}`}
        >
          <span
            aria-hidden="true"
            class="inline-block h-4 w-4 rounded-full border border-border"
            style={{ background: roleColour(r, palette) }}
          />
          {ROLE_LABEL[r]}
        </button>
      ))}
    </div>
  );
}
