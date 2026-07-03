import { useEffect, useMemo, useState } from "react";
import "./theme.css";
import "./App.css";
import { continuumApi, type RailNode, type NodeKind } from "./api";

const KIND_LABELS: Record<NodeKind, string> = {
  Voie: "Voie",
  AppareilDeVoie: "Appareil de voie",
  Signal: "Signal",
  Sillon: "Sillon",
  Horaire: "Horaire",
  ProjetInvestissement: "Projet d'investissement",
};

// Chaque type d'objet du graphe hétérogène a sa propre couleur, à la façon
// dont OSRD distingue ses catégories de trains — voir theme.css.
const KIND_CLASS: Record<NodeKind, string> = {
  Voie: "kind-voie",
  AppareilDeVoie: "kind-appareil",
  Signal: "kind-signal",
  Sillon: "kind-sillon",
  Horaire: "kind-horaire",
  ProjetInvestissement: "kind-projet",
};

type ApiStatus = "loading" | "connected" | "offline";

function KindBadge({ kind }: { kind: NodeKind }) {
  return <span className={`kind-badge ${KIND_CLASS[kind]}`}>{KIND_LABELS[kind] ?? kind}</span>;
}

function App() {
  const [status, setStatus] = useState<ApiStatus>("loading");
  const [branches, setBranches] = useState<string[]>([]);
  const [reference, setReference] = useState<string>("référence");
  const [selectedBranch, setSelectedBranch] = useState<string>("etude-capacite");
  const [referenceNodes, setReferenceNodes] = useState<RailNode[]>([]);
  const [added, setAdded] = useState<RailNode[]>([]);
  const [removed, setRemoved] = useState<RailNode[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    continuumApi
      .health()
      .then(() => setStatus("connected"))
      .catch(() => setStatus("offline"));
  }, []);

  useEffect(() => {
    if (status !== "connected") return;
    continuumApi
      .listBranches()
      .then((names) => setBranches(names))
      .catch((e) => setError(String(e)));
  }, [status]);

  useEffect(() => {
    if (status !== "connected") return;
    continuumApi
      .getBranch(reference)
      .then((graph) => setReferenceNodes(Object.values(graph.nodes)))
      .catch((e) => setError(String(e)));
  }, [status, reference]);

  useEffect(() => {
    if (status !== "connected" || !selectedBranch || selectedBranch === reference) {
      setAdded([]);
      setRemoved([]);
      return;
    }
    continuumApi
      .diff(reference, selectedBranch)
      .then((result) => {
        setAdded(result.added);
        setRemoved(result.removed);
      })
      .catch((e) => setError(String(e)));
  }, [status, reference, selectedBranch]);

  const otherBranches = useMemo(
    () => branches.filter((b) => b !== reference),
    [branches, reference]
  );

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
      {error && <div className="banner banner--warning">{error}</div>}

      {status === "connected" && (
        <>
          <section className="toolbar">
            <label>
              Branche de référence
              <select value={reference} onChange={(e) => setReference(e.target.value)}>
                {branches.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Comparer avec
              <select value={selectedBranch} onChange={(e) => setSelectedBranch(e.target.value)}>
                {otherBranches.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="columns">
            <div className="board">
              <div className="board-header">
                <h2>État de référence</h2>
                <span className="board-subtitle">{referenceNodes.length} nœud(s)</span>
              </div>
              <ul className="node-list">
                {referenceNodes.map((n) => (
                  <li key={n.id}>
                    <KindBadge kind={n.kind} />
                    {n.label}
                  </li>
                ))}
              </ul>
            </div>

            <div className="board">
              <div className="board-header">
                <h2>Diff — branche « {selectedBranch} »</h2>
                <span className="board-subtitle">
                  {added.length} ajout(s) · {removed.length} suppression(s)
                </span>
              </div>
              <ul className="node-list">
                {added.map((n) => (
                  <li key={n.id} className="node-added">
                    <span className="diff-symbol">+</span>
                    <KindBadge kind={n.kind} />
                    {n.label}
                  </li>
                ))}
                {removed.map((n) => (
                  <li key={n.id} className="node-removed">
                    <span className="diff-symbol">−</span>
                    <KindBadge kind={n.kind} />
                    {n.label}
                  </li>
                ))}
                {added.length === 0 && removed.length === 0 && (
                  <li className="node-empty">Aucune différence avec la référence.</li>
                )}
              </ul>
            </div>
          </section>
        </>
      )}
    </div>
  );
}

export default App;
