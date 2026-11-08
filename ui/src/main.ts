import "./styles.css";
import { fitWindow } from "./fit-window";
import { syncStatus } from "./status-sync";
import { mountPopover } from "./view-popover";

const root = document.getElementById("app");
if (root) {
  const view = mountPopover(root);
  fitWindow(root);
  void syncStatus(view.update);
}
