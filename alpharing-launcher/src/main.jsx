import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <App buildInfo={"build v1.0.0"} modInfo={"AlphaRing v1.3.5"}/>
  </React.StrictMode>,
);
