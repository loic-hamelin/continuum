import type { Commit } from "../api";

/**
 * Formatte une date ISO en durée relative ("il y a 3 minutes"), comme
 * demandé pour l'historique — pas de dépendance externe, juste
 * `Intl.RelativeTimeFormat` déjà disponible dans les navigateurs.
 */
function formatRelativeTime(iso: string): string {
  const diffMs = new Date(iso).getTime() - Date.now();
  const diffMinutes = Math.round(diffMs / 60000);
  const formatter = new Intl.RelativeTimeFormat("fr", { numeric: "auto" });

  if (Math.abs(diffMinutes) < 60) {
    return formatter.format(diffMinutes, "minute");
  }
  const diffHours = Math.round(diffMinutes / 60);
  if (Math.abs(diffHours) < 24) {
    return formatter.format(diffHours, "hour");
  }
  const diffDays = Math.round(diffHours / 24);
  return formatter.format(diffDays, "day");
}

interface HistoryBoardProps {
  branchName: string;
  commits: Commit[];
  viewingCommitId: string | null;
  onSelectCommit: (commitId: string) => void;
}

export function HistoryBoard({
  branchName,
  commits,
  viewingCommitId,
  onSelectCommit,
}: HistoryBoardProps) {
  return (
    <div className="board">
      <div className="board-header">
        <h2>Historique — branche « {branchName} »</h2>
        <span className="board-subtitle">{commits.length} commit(s)</span>
      </div>
      <ul className="commit-list">
        {commits.map((commit) => (
          <li key={commit.id}>
            <button
              type="button"
              className={`commit-entry${commit.id === viewingCommitId ? " commit-entry--active" : ""}`}
              onClick={() => onSelectCommit(commit.id)}
            >
              <span className="commit-message">{commit.message}</span>
              <span className="commit-meta">
                {commit.author} · {formatRelativeTime(commit.created_at)}
              </span>
            </button>
          </li>
        ))}
        {commits.length === 0 && <li className="node-empty">Aucun commit.</li>}
      </ul>
    </div>
  );
}
