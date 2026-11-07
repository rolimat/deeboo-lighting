import "./styles.css";
import { api } from "./api";
import { mountPopover } from "./view-popover";

const root = document.getElementById("app");
if (root) {
  const view = mountPopover(root);
  void api.getStatus().then(view.update);
}
