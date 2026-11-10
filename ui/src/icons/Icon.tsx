import { icons, type IconName } from "./index";

/** Decorative inline SVG from the bundled set (static markup, never user data). */
export function Icon({ name }: { name: IconName }) {
  return (
    <span
      aria-hidden="true"
      class="inline-flex shrink-0 [&>svg]:h-4 [&>svg]:w-4"
      dangerouslySetInnerHTML={{ __html: icons[name].trim() }}
    />
  );
}
