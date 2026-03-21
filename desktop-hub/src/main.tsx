import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import App from "./App";
import { SplashScreen } from "./components/SplashScreen";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/splash" element={<SplashScreen />} />
        <Route path="/app" element={<App />} />
        <Route path="/" element={<Navigate to="/splash" replace />} />
      </Routes>
    </BrowserRouter>
  </React.StrictMode>
);
