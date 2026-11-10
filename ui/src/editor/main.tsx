import "./editor.css";
import { render } from "preact";
import { App } from "./App";
import { api } from "./api";

async function boot() {
  // Dev-only visual review harness; the DEV branch is removed from production builds.
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
    (await import("./mock")).installMock(api);
  }
  const root = document.getElementById("app");
  if (root) render(<App />, root);
}
void boot();
