/**
 * Client minimal pour l'API CONTINUUM (crate `api`, sur le modèle
 * d'editoast dans OSRD). Pas de génération automatique depuis l'OpenAPI
 * pour l'instant — une bonne évolution une fois l'API stabilisée serait
 * d'utiliser un outil comme `openapi-typescript` pour générer ces types
 * et fonctions automatiquement à partir de `api/openapi/openapi.json`.
 */

const API_BASE_URL = "http://127.0.0.1:8000";

export type NodeKind =
  | "Voie"
  | "AppareilDeVoie"
  | "Signal"
  | "Sillon"
  | "Horaire"
  | "ProjetInvestissement";

export interface RailNode {
  id: string;
  kind: NodeKind;
  label: string;
  properties: Record<string, string>;
}

export interface DiffResult {
  added: RailNode[];
  removed: RailNode[];
}

async function request<T>(path: string): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`);
  if (!response.ok) {
    throw new Error(`Erreur API (${response.status}) sur ${path}`);
  }
  return response.json() as Promise<T>;
}

export const continuumApi = {
  health: () => request<{ status: string }>("/health"),
  listBranches: () => request<string[]>("/branches"),
  getBranch: (name: string) =>
    request<{ nodes: Record<string, RailNode>; edges: unknown[] }>(
      `/branches/${encodeURIComponent(name)}`
    ),
  diff: (base: string, compare: string) =>
    request<DiffResult>(
      `/diff?base=${encodeURIComponent(base)}&compare=${encodeURIComponent(compare)}`
    ),
};
