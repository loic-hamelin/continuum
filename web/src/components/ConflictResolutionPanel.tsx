import { useState } from "react";
import type { ConflictSide, MergeConflict } from "../api";
import {
  conflictDescription,
  conflictEditorIds,
  conflictKey,
  toResolution,
} from "../conflictUtils";

const OSRD_EDITOR_URL = import.meta.env.VITE_OSRD_EDITOR_URL ?? "http://localhost:4000";

interface ConflictResolutionPanelProps {
  conflicts: MergeConflict[];
  sourceLabel: string;
  targetLabel: string;
  onValidate: (choices: Map<string, ConflictSide>) => void;
  submitting: boolean;
  onError: (message: string) => void;
}

export function ConflictResolutionPanel({
  conflicts,
  sourceLabel,
  targetLabel,
  onValidate,
  submitting,
  onError,
}: ConflictResolutionPanelProps) {
  const [choices, setChoices] = useState<Map<string, ConflictSide>>(new Map());

  const chooseSide = (conflict: MergeConflict, side: ConflictSide) => {
    setChoices((previous) => {
      const next = new Map(previous);
      next.set(conflictKey(conflict), side);
      return next;
    });
  };

  const openInEditor = async (conflict: MergeConflict) => {
    const ids = conflictEditorIds(conflict);
    try {
      await navigator.clipboard.writeText(ids.join(", "));
    } catch (e) {
      onError(`Impossible de copier dans le presse-papier : ${String(e)}`);
    }
    window.open(OSRD_EDITOR_URL, "_blank");
  };

  const allResolved = conflicts.every((conflict) => choices.has(conflictKey(conflict)));

  return (
    <div className="board">
      <div className="board-header">
        <h2>Résolution des conflits</h2>
        <span className="board-subtitle">{conflicts.length} conflit(s)</span>
      </div>
      <ul className="conflict-resolution-list">
        {conflicts.map((conflict) => {
          const key = conflictKey(conflict);
          const chosen = choices.get(key);
          return (
            <li key={key} className="conflict-resolution-item">
              <p className="conflict-resolution-description">{conflictDescription(conflict)}</p>
              <div className="conflict-resolution-actions">
                <button
                  type="button"
                  className={`btn btn-secondary${chosen === "source" ? " btn-selected" : ""}`}
                  onClick={() => chooseSide(conflict, "source")}
                >
                  Garder « {sourceLabel} »
                </button>
                <button
                  type="button"
                  className={`btn btn-secondary${chosen === "target" ? " btn-selected" : ""}`}
                  onClick={() => chooseSide(conflict, "target")}
                >
                  Garder « {targetLabel} »
                </button>
                <button type="button" className="btn btn-secondary" onClick={() => openInEditor(conflict)}>
                  Ouvrir dans l'éditeur OSRD
                </button>
              </div>
            </li>
          );
        })}
      </ul>
      <div className="board-footer">
        <button
          type="button"
          className="btn"
          disabled={!allResolved || submitting}
          onClick={() => onValidate(choices)}
        >
          {submitting ? "Fusion…" : "Valider la fusion"}
        </button>
        {!allResolved && (
          <span className="conflict-resolution-hint">
            Choisissez une résolution pour chaque conflit avant de valider.
          </span>
        )}
      </div>
    </div>
  );
}

export function buildResolutions(
  conflicts: MergeConflict[],
  choices: Map<string, ConflictSide>
) {
  return conflicts.map((conflict) => toResolution(conflict, choices.get(conflictKey(conflict))!));
}
