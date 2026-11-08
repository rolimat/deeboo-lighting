import type { DeviceDto } from "../api";

export function StepChoose({
  devices,
  onChoose,
}: {
  devices: DeviceDto[];
  onChoose: (id: string) => void;
}) {
  return (
    <ul class="space-y-2">
      {devices.map((d) => (
        <li key={d.id}>
          <button
            type="button"
            disabled={!d.supported}
            onClick={() => onChoose(d.id)}
            class="w-full rounded-card bg-card px-3 py-2 text-left disabled:opacity-50"
          >
            {d.name}
            {!d.supported && <span class="text-dim"> · not supported</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}
