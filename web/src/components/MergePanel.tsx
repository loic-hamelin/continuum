import { useState } from "react";
import { continuumApi, type MergeConflict } from "../api";

interface MergePanelProps {
  source: string;
  target: string;
  author: string;
  addedCount: number;
  removedCount: number;
  onMerged: () => void;
  onConflicts: (conflicts: MergeConflict[], message: string) => void;
  onError: (message: string) => void;
}

export function MergePanel({
  source,
  target,
  author,
  addedCount,
  removedCount,
  onMerged,
  onConflicts,
  onError,
}: MergePanelProps) {
  const [message, setMessage] = useState(`Fusion de « ${source} » vers « ${target} »`);
  const [submitting, setSubmitting] = useState(false);

  const handleMerge = async () => {
    const confirmed = window.confirm(
      `Fusionner « ${source} » vers « ${target} » ? (${addedCount} ajout(s), ${removedCount} suppression(s) prévisualisés dans le diff ci-dessus)`
    );
    if (!confirmed) return;

    setSubmitting(true);
    try {
      const trimmedMessage = message.trim();
      const result = await continuumApi.merge(source, target, author.trim() || "anonyme", trimmedMessage);
      if (result.status === "merged") {
        onMerged();
      } else if (result.status === "conflicts") {
        onConflicts(result.conflicts, trimmedMessage);
      } else {
        onError(result.message);
      }
    } catch (e) {
      onError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="merge-panel">
      <label>
        Message de fusion
        <input type="text" value={message} onChange={(e) => setMessage(e.target.value)} />
      </label>
      <button type="button" className="btn" disabled={submitting} onClick={handleMerge}>
        {submitting ? "Fusion…" : `Fusionner « ${source} » vers « ${target} »`}
      </button>
    </div>
  );
}
