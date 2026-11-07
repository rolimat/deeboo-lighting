import "./styles.css";
import { syncStatus } from "./status-sync";
import { mountPopover } from "./view-popover";

const root = document.getElementById("app");
if (root) {
  const view = mountPopover(root);
  void syncStatus(view.update);
}
