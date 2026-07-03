import { useEffect, useMemo, useState } from "react";
import "./theme.css";
import "./App.css";
import {
  continuumApi,
  type RailNode,
  type Commit,
  type Graph,
  type MergeConflict,
  type ConflictSide,
} from "./api";
import { conflictSourceNodeIds, conflictTargetNodeIds } from "./conflictUtils";
import { HistoryBoard } from "./components/HistoryBoard";
import { CreateBranchForm } from "./components/CreateBranchForm";
import { CommitForm } from "./components/CommitForm";
import { MergePanel } from "./components/MergePanel";
import { MapPair } from "./components/MapPair";
import { ConflictResolutionPanel, buildResolutions } from "./components/ConflictResolutionPanel";
import { NodeGroupList, type NodeRow } from "./components/NodeGroupList";

type ApiStatus = "loading" | "connected" | "offline";

function App() {
  const [status, setStatus] = useState<ApiStatus>("loading");
  const [branches, setBranches] = useState<string[]>([]);
  const [reference, setReference] = useState<string>("référence");
  const [selectedBranch, setSelectedBranch] = useState<string>("etude-capacite");
  const [author, setAuthor] = useState<string>("");
  const [referenceNodes, setReferenceNodes] = useState<RailNode[]>([]);
  const [added, setAdded] = useState<RailNode[]>([]);
  const [removed, setRemoved] = useState<RailNode[]>([]);
  const [history, setHistory] = useState<Commit[]>([]);
  const [viewingCommitId, setViewingCommitId] = useState<string | null>(null);
  const [viewingNodes, setViewingNodes] = useState<RailNode[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [mergeConflicts, setMergeConflicts] = useState<MergeConflict[] | null>(null);
  const [mergeMessage, setMergeMessage] = useState("");
  const [conflictSourceGraph, setConflictSourceGraph] = useState<Graph | null>(null);
  const [conflictTargetGraph, setConflictTargetGraph] = useState<Graph | null>(null);
  const [resolvingMerge, setResolvingMerge] = useState(false);
  const [generatingDemoConflict, setGeneratingDemoConflict] = useState(false);

  useEffect(() => {
    continuumApi
      .health()
      .then(() => setStatus("connected"))
      .catch(() => setStatus("offline"));
  }, []);

  const loadBranches = () => {
    continuumApi
      .listBranches()
      .then((names) => setBranches(names))
      .catch((e) => setError(String(e)));
  };

  const loadReferenceNodes = () => {
    continuumApi
      .getBranch(reference)
      .then((graph) => setReferenceNodes(Object.values(graph.nodes)))
      .catch((e) => setError(String(e)));
  };

  const loadDiff = () => {
    if (!selectedBranch || selectedBranch === reference) {
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
  };

  const loadHistory = () => {
    if (!selectedBranch) {
      setHistory([]);
      return;
    }
    continuumApi
      .getHistory(selectedBranch)
      .then(setHistory)
      .catch((e) => setError(String(e)));
  };

  const clearConflicts = () => {
    setMergeConflicts(null);
    setConflictSourceGraph(null);
    setConflictTargetGraph(null);
  };

  const handleConflicts = (conflicts: MergeConflict[], message: string) => {
    setMergeConflicts(conflicts);
    setMergeMessage(message);
    Promise.all([continuumApi.getBranch(selectedBranch), continuumApi.getBranch(reference)])
      .then(([sourceGraph, targetGraph]) => {
        setConflictSourceGraph(sourceGraph);
        setConflictTargetGraph(targetGraph);
      })
      .catch((e) => setError(String(e)));
  };

  const handleValidateResolutions = async (choices: Map<string, ConflictSide>) => {
    if (!mergeConflicts) return;
    setResolvingMerge(true);
    try {
      const resolutions = buildResolutions(mergeConflicts, choices);
      const result = await continuumApi.mergeResolve(
        selectedBranch,
        reference,
        author.trim() || "anonyme",
        mergeMessage,
        resolutions
      );
      if (result.status === "merged") {
        clearConflicts();
        setViewingCommitId(null);
        loadReferenceNodes();
        loadDiff();
      } else if (result.status === "conflicts") {
        // Les branches ont changé entre la prévisualisation et la
        // validation, ou une résolution était incomplète : on réaffiche
        // ce qui reste à traiter plutôt que d'échouer silencieusement.
        setMergeConflicts(result.conflicts);
        setError("Certains conflits restent non résolus — les branches ont peut-être changé entre-temps.");
      } else {
        setError(result.message);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setResolvingMerge(false);
    }
  };

  /**
   * Raccourci de développement : génère un conflit spatial de
   * démonstration côté API (`POST /debug/seed-conflict`), sélectionne les
   * deux branches renvoyées, et déclenche immédiatement la tentative de
   * fusion pour afficher les cartes + le panneau de résolution sans autre
   * manipulation.
   */
  const handleGenerateDemoConflict = async () => {
    setGeneratingDemoConflict(true);
    try {
      const { branch_a, branch_b } = await continuumApi.seedDemoConflict();
      const names = await continuumApi.listBranches();
      setBranches(names);
      setReference(branch_a);
      setSelectedBranch(branch_b);

      const message = `Fusion de démonstration « ${branch_b} » vers « ${branch_a} »`;
      const result = await continuumApi.merge(branch_b, branch_a, author.trim() || "anonyme", message);
      if (result.status === "conflicts") {
        setMergeMessage(message);
        setMergeConflicts(result.conflicts);
        const [sourceGraph, targetGraph] = await Promise.all([
          continuumApi.getBranch(branch_b),
          continuumApi.getBranch(branch_a),
        ]);
        setConflictSourceGraph(sourceGraph);
        setConflictTargetGraph(targetGraph);
      } else if (result.status === "merged") {
        setError("Le scénario de démonstration n'a pas produit de conflit cette fois — réessayez.");
      } else {
        setError(result.message);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setGeneratingDemoConflict(false);
    }
  };

  useEffect(() => {
    if (status !== "connected") return;
    loadBranches();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]);

  useEffect(() => {
    if (status !== "connected") return;
    loadReferenceNodes();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, reference]);

  useEffect(() => {
    if (status !== "connected") return;
    loadDiff();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, reference, selectedBranch]);

  useEffect(() => {
    if (status !== "connected") return;
    loadHistory();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, selectedBranch]);

  useEffect(() => {
    if (!viewingCommitId) {
      setViewingNodes(null);
      return;
    }
    continuumApi
      .getCommitGraph(viewingCommitId)
      .then((graph) => setViewingNodes(Object.values(graph.nodes)))
      .catch((e) => setError(String(e)));
  }, [viewingCommitId]);

  const otherBranches = useMemo(
    () => branches.filter((b) => b !== reference),
    [branches, reference]
  );

  const displayedNodes = viewingCommitId ? viewingNodes ?? [] : referenceNodes;

  const diffRows: NodeRow[] = useMemo(
    () => [
      ...added.map((node) => ({ node, variant: "added" as const })),
      ...removed.map((node) => ({ node, variant: "removed" as const })),
    ],
    [added, removed]
  );

  const sourceConflictIds = useMemo(
    () => (mergeConflicts ?? []).flatMap(conflictSourceNodeIds),
    [mergeConflicts]
  );
  const targetConflictIds = useMemo(
    () => (mergeConflicts ?? []).flatMap(conflictTargetNodeIds),
    [mergeConflicts]
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
      {viewingCommitId && (
        <div className="banner banner--info">
          Vous consultez l'état au commit <code>#{viewingCommitId.slice(0, 8)}</code> —{" "}
          <button type="button" className="banner-link" onClick={() => setViewingCommitId(null)}>
            revenir à l'état courant
          </button>
        </div>
      )}

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
            <label>
              Auteur
              <input
                type="text"
                value={author}
                placeholder="votre nom"
                onChange={(e) => setAuthor(e.target.value)}
              />
            </label>
            <CreateBranchForm onCreated={loadBranches} onError={setError} />
            <button
              type="button"
              className="btn btn-secondary"
              disabled={generatingDemoConflict}
              onClick={handleGenerateDemoConflict}
            >
              {generatingDemoConflict ? "Génération…" : "Générer un conflit de démonstration"}
            </button>
          </section>

          <section className="columns">
            <div className="board">
              <div className="board-header">
                <h2>État de référence</h2>
                <span className="board-subtitle">{displayedNodes.length} nœud(s)</span>
              </div>
              <NodeGroupList
                rows={displayedNodes.map((node) => ({ node }))}
                emptyMessage="Aucun nœud."
              />
            </div>

            <div className="board">
              <div className="board-header">
                <h2>Diff — branche « {selectedBranch} »</h2>
                <span className="board-subtitle">
                  {added.length} ajout(s) · {removed.length} suppression(s)
                </span>
              </div>
              <NodeGroupList
                rows={diffRows}
                emptyMessage="Aucune différence avec la référence."
              />
              {selectedBranch && selectedBranch !== reference && (
                <div className="board-footer">
                  <MergePanel
                    source={selectedBranch}
                    target={reference}
                    author={author}
                    addedCount={added.length}
                    removedCount={removed.length}
                    onMerged={() => {
                      setViewingCommitId(null);
                      loadReferenceNodes();
                      loadDiff();
                    }}
                    onConflicts={handleConflicts}
                    onError={setError}
                  />
                </div>
              )}
            </div>

            <HistoryBoard
              branchName={selectedBranch}
              commits={history}
              viewingCommitId={viewingCommitId}
              onSelectCommit={setViewingCommitId}
            />

            {selectedBranch && (
              <CommitForm
                branch={selectedBranch}
                author={author}
                onCommitted={() => {
                  loadDiff();
                  loadHistory();
                }}
                onError={setError}
              />
            )}
          </section>

          {mergeConflicts && mergeConflicts.length > 0 && (
            <section className="conflict-resolution">
              <MapPair
                sourceLabel={selectedBranch}
                targetLabel={reference}
                sourceGraph={conflictSourceGraph}
                targetGraph={conflictTargetGraph}
                sourceConflictIds={sourceConflictIds}
                targetConflictIds={targetConflictIds}
              />
              <ConflictResolutionPanel
                conflicts={mergeConflicts}
                sourceLabel={selectedBranch}
                targetLabel={reference}
                onValidate={handleValidateResolutions}
                submitting={resolvingMerge}
                onError={setError}
              />
            </section>
          )}
        </>
      )}
    </div>
  );
}

export default App;
