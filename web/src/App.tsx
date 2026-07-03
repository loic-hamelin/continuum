import { useEffect, useState } from "react";
import "./theme.css";
import "./App.css";
import { continuumApi } from "./api";
import { ComparisonTab } from "./components/ComparisonTab";
import { SchemaTab } from "./components/SchemaTab";

type ApiStatus = "loading" | "connected" | "offline";
type TabId = "comparaison" | "schema";

const TABS: { id: TabId; label: string }[] = [
  { id: "comparaison", label: "Comparaison" },
  { id: "schema", label: "Schéma" },
];

function App() {
  const [status, setStatus] = useState<ApiStatus>("loading");
  const [activeTab, setActiveTab] = useState<TabId>("comparaison");

  useEffect(() => {
    continuumApi
      .health()
      .then(() => setStatus("connected"))
      .catch(() => setStatus("offline"));
  }, []);

  const apiConnected = status === "connected";

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark">C</span>
          <div>
            <h1>CONTINUUM</h1>
            <p>Gestion de versions du jumeau numérique ferroviaire — fondé sur OSRD</p>
          </div>
        </div>
      </header>

      {status === "loading" && (
        <div className="banner banner--info">Connexion à l'API…</div>
      )}
      {status === "offline" && (
        <div className="banner banner--error">
          API indisponible — lancez <code>cargo run -p continuum-api</code> dans un terminal, puis
          rechargez la page.
        </div>
      )}

      {apiConnected && (
        <>
          <nav className="tab-bar" role="tablist">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                role="tab"
                aria-selected={activeTab === tab.id}
                className={`tab-button ${activeTab === tab.id ? "tab-button--active" : ""}`}
                onClick={() => setActiveTab(tab.id)}
              >
                {tab.label}
              </button>
            ))}
          </nav>

          {activeTab === "comparaison" && <ComparisonTab apiConnected={apiConnected} />}
          {activeTab === "schema" && <SchemaTab />}
        </>
      )}
    </div>
  );
}

export default App;
