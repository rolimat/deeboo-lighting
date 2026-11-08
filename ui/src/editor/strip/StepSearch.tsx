import type { DeviceDto } from "../api";

export function StepSearch({ device, help }: { device: DeviceDto; help: boolean }) {
  return (
    <div class="space-y-3">
      <p>
        <span
          class="mr-2 inline-block h-3 w-3 animate-spin rounded-full border-2 border-dim border-t-transparent"
          aria-hidden="true"
        />
        Looking for {device.name} on your wifi…
      </p>
      {help && (
        <p class="text-dim">
          Not found yet. It may be connected only by Bluetooth or on another network. In Smart Life,
          remove it and add it again on a 2.4 GHz wifi. Keep this window open; it will continue when
          the strip appears.
        </p>
      )}
      <p class="text-dim">
        macOS may ask to allow Deeboo Lighting to find devices on your local network. Choose Allow.
      </p>
    </div>
  );
}
