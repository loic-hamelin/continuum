import { useState } from "react";
import "./App.css";

/**
 * CONTINUUM — démonstrateur d'interface minimal.
 *
 * Ceci est un point de départ volontairement simple : deux versions
 * statiques du graphe (référence / branche) et un diff calculé côté
 * client. À faire évoluer avec Claude Code — en particulier pour brancher
 * cette interface sur le vrai moteur de graphe (graph-engine, en Rust)
 * une fois une API ou un pont WASM mis en place.
 */

type NodeKind =
  | "Voie"
  | "AppareilDeVoie"
  | "Signal"
  | "Sillon"
  | "Horaire"
  | "ProjetInvestissement";

interface RailNode {
  id: string;
  kind: NodeKind;
  label: string;
}

const reference: RailNode[] = [
  { id: "voie-12", kind: "Voie", label: "Voie 12 - gare de Lens" },
];

const brancheEtudeCapacite: RailNode[] = [
  ...reference,
  {
    id: "aiguille-45",
    kind: "AppareilDeVoie",
    label: "Aiguille 45 (hypothèse d'ajout)",
  },
];

const branches: Record<string, RailNode[]> = {
  référence: reference,
  "etude-capacite": brancheEtudeCapacite,
};

function diff(base: RailNode[], compare: RailNode[]) {
  const baseIds = new Set(base.map((n) => n.id));
  const compareIds = new Set(compare.map((n) => n.id));
  return {
    added: compare.filter((n) => !baseIds.has(n.id)),
    removed: base.filter((n) => !compareIds.has(n.id)),
  };
}

function App() {
  const [selectedBranch, setSelectedBranch] = useState("etude-capacite");
  const result = diff(reference, branches[selectedBranch]);

  return (
    <div className="app">
      <header>
        <h1>CONTINUUM</h1>
        <p className="subtitle">
          Système de gestion de versions et de cycle de vie temporel du
          jumeau numérique ferroviaire — démonstrateur d'interface
        </p>
      </header>

      <section>
        <label htmlFor="branch-select">Branche à comparer à la référence : </label>
        <select
          id="branch-select"
          value={selectedBranch}
          onChange={(e) => setSelectedBranch(e.target.value)}
        >
          {Object.keys(branches)
            .filter((b) => b !== "référence")
            .map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
        </select>
      </section>

      <section className="columns">
        <div className="column">
          <h2>État de référence ({reference.length} nœud(s))</h2>
          <ul>
            {reference.map((n) => (
              <li key={n.id}>
                <span className="kind">{n.kind}</span> {n.label}
              </li>
            ))}
          </ul>
        </div>

        <div className="column">
          <h2>
            Diff — branche « {selectedBranch} »
          </h2>
          <ul>
            {result.added.map((n) => (
              <li key={n.id} className="added">
                + <span className="kind">{n.kind}</span> {n.label}
              </li>
            ))}
            {result.removed.map((n) => (
              <li key={n.id} className="removed">
                − <span className="kind">{n.kind}</span> {n.label}
              </li>
            ))}
            {result.added.length === 0 && result.removed.length === 0 && (
              <li>Aucune différence.</li>
            )}
          </ul>
        </div>
      </section>
    </div>
  );
}

export default App;
