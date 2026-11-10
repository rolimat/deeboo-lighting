import "./styles.css";
import { api } from "./api";
import { fitWindow } from "./fit-window";
import { syncStatus } from "./status-sync";
import { mountPopover } from "./view-popover";

const root = document.getElementById("app");
if (root) {
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
    (await import("./mock")).installMock(api);
  }
  const view = mountPopover(root);
  fitWindow(root);
  void syncStatus(view.update);
}
