import { useState } from "react";
import { continuumApi, type NodeKind } from "../api";

const KIND_OPTIONS: { value: NodeKind; label: string }[] = [
  { value: "Voie", label: "Voie" },
  { value: "AppareilDeVoie", label: "Appareil de voie" },
  { value: "Signal", label: "Signal" },
  { value: "Sillon", label: "Sillon" },
  { value: "Horaire", label: "Horaire" },
  { value: "ProjetInvestissement", label: "Projet d'investissement" },
];

interface CommitFormProps {
  branch: string;
  author: string;
  onCommitted: () => void;
  onError: (message: string) => void;
}

export function CommitForm({ branch, author, onCommitted, onError }: CommitFormProps) {
  const [kind, setKind] = useState<NodeKind>("Voie");
  const [id, setId] = useState("");
  const [label, setLabel] = useState("");
  const [message, setMessage] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const canSubmit = id.trim() && label.trim() && message.trim() && !submitting;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!canSubmit) return;

    setSubmitting(true);
    try {
      await continuumApi.commitOnBranch(branch, {
        author: author.trim() || "anonyme",
        message: message.trim(),
        addNodes: [{ id: id.trim(), kind, label: label.trim(), properties: {} }],
      });
      setId("");
      setLabel("");
      setMessage("");
      onCommitted();
    } catch (e) {
      onError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="board">
      <div className="board-header">
        <h2>Committer sur « {branch} »</h2>
      </div>
      <form className="board-form" onSubmit={handleSubmit}>
        <label>
          Type
          <select value={kind} onChange={(e) => setKind(e.target.value as NodeKind)}>
            {KIND_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          Id
          <input type="text" value={id} placeholder="ex: voie-13" onChange={(e) => setId(e.target.value)} />
        </label>
        <label>
          Label
          <input
            type="text"
            value={label}
            placeholder="ex: Voie 13 - gare de Lens"
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <label>
          Message de commit
          <input
            type="text"
            value={message}
            placeholder="ex: Ajout de la voie 13"
            onChange={(e) => setMessage(e.target.value)}
          />
        </label>
        <button type="submit" className="btn" disabled={!canSubmit}>
          {submitting ? "Envoi…" : "Committer"}
        </button>
      </form>
    </div>
  );
}
