import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./shared/styles/reset.css";
import "./shared/styles/tokens.css";
import "./app/frame/AppFrame.css";
import "./app/shell/layouts/classic-sidebar/ClassicSidebar.css";
import "./features/dashboard/Dashboard.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
