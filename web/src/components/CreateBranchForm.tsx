import { useState } from "react";
import { continuumApi } from "../api";

interface CreateBranchFormProps {
  onCreated: () => void;
  onError: (message: string) => void;
}

export function CreateBranchForm({ onCreated, onError }: CreateBranchFormProps) {
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim()) return;

    setSubmitting(true);
    try {
      await continuumApi.createBranch(name.trim());
      setName("");
      onCreated();
    } catch (e) {
      onError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form className="inline-form" onSubmit={handleSubmit}>
      <label>
        Nouvelle branche
        <input
          type="text"
          value={name}
          placeholder="ex: etude-signalisation"
          onChange={(e) => setName(e.target.value)}
        />
      </label>
      <button type="submit" className="btn" disabled={submitting || !name.trim()}>
        {submitting ? "Création…" : "Créer"}
      </button>
    </form>
  );
}
