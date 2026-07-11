import { lazy, Suspense, useEffect } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { useAuth } from "./hooks/useAuth";
import AppLayout from "./components/AppLayout";
import ErrorBoundary from "./components/ErrorBoundary";
import { Spin } from "antd";

const LoginPage = lazy(() => import("./pages/LoginPage"));
const HomePage = lazy(() => import("./pages/HomePage"));
const PromptSettingsPage = lazy(() => import("./pages/PromptSettingsPage"));

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}>
        <Spin size="large" />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

function SuspenseWrapper({ children }: { children: React.ReactNode }) {
  return (
    <Suspense fallback={
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "60vh" }}>
        <Spin />
      </div>
    }>
      <div className="fade-in-up">{children}</div>
    </Suspense>
  );
}

export default function App() {
  // Escape key: cancel voice recording
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && (e.target as HTMLElement)?.tagName !== "INPUT" && (e.target as HTMLElement)?.tagName !== "TEXTAREA") {
        const pulseBtn = document.querySelector(".pulse-recording") as HTMLButtonElement;
        if (pulseBtn) { pulseBtn.click(); }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <ErrorBoundary>
      <Routes>
        <Route path="/login" element={
          <SuspenseWrapper><LoginPage /></SuspenseWrapper>
        } />
        <Route
          path="/"
          element={
            <ProtectedRoute>
              <AppLayout />
            </ProtectedRoute>
          }
        >
          <Route index element={<SuspenseWrapper><HomePage /></SuspenseWrapper>} />
          <Route path="settings/prompts" element={<SuspenseWrapper><PromptSettingsPage /></SuspenseWrapper>} />
        </Route>
    </ErrorBoundary>
  );
}