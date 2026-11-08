import { expect, it, vi } from "vitest";

vi.mock("./api", () => ({ api: { setWindowHeight: vi.fn(() => Promise.resolve()) } }));
import { api } from "./api";
import { fitWindow } from "./fit-window";

it("reports_height_changes", () => {
  let cb: ResizeObserverCallback = () => {};
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(c: ResizeObserverCallback) {
        cb = c;
      }
      observe() {}
    },
  );
  fitWindow(document.body);
  cb([{ borderBoxSize: [{ blockSize: 321.4 }] } as unknown as ResizeObserverEntry], {} as never);
  expect(api.setWindowHeight).toHaveBeenCalledWith(322);
});
