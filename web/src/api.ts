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
  properties: Record<string, unknown>;
}

export interface DiffResult {
  added: RailNode[];
  removed: RailNode[];
}

export interface Graph {
  nodes: Record<string, RailNode>;
  edges: unknown[];
}

export interface Commit {
  id: string;
  parents: string[];
  author: string;
  message: string;
  created_at: string;
}

export type ConflictSide = "source" | "target";

/**
 * Un conflit de fusion détecté par l'API — trois formes distinctes
 * (discriminées par `kind`), voir `graph-engine::MergeConflict` :
 * - `modification` : même id, modifié différemment des deux côtés.
 * - `deletion_vs_modification` : un côté a supprimé l'objet, l'autre l'a
 *   modifié.
 * - `spatial` : deux ids *différents*, positionnés sur la même voie à
 *   moins de `distance_meters` l'un de l'autre — un conflit propre au
 *   ferroviaire qu'un diff par id ne détecterait jamais.
 */
export type MergeConflict =
  | { kind: "modification"; node_id: string; ancestor: RailNode | null; source: RailNode; target: RailNode }
  | {
      kind: "deletion_vs_modification";
      node_id: string;
      ancestor: RailNode;
      modified: RailNode;
      deleted_in: ConflictSide;
    }
  | {
      kind: "spatial";
      track: string;
      source_node: RailNode;
      source_position: number;
      target_node: RailNode;
      target_position: number;
      distance_meters: number;
    };

/** La résolution choisie par l'utilisateur pour un conflit donné. */
export type ConflictResolution =
  | { kind: "modification"; node_id: string; keep: ConflictSide }
  | { kind: "deletion_vs_modification"; node_id: string; keep: ConflictSide }
  | { kind: "spatial"; source_node_id: string; target_node_id: string; keep: ConflictSide };

export type MergeResult =
  | { status: "merged"; commitId: string }
  | { status: "conflicts"; conflicts: MergeConflict[] }
  | { status: "error"; message: string };

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`, init);
  if (!response.ok) {
    throw new Error(`Erreur API (${response.status}) sur ${path}`);
  }
  return response.json() as Promise<T>;
}

function postJson<T>(path: string, body: unknown): Promise<T> {
  return request<T>(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

export interface Bbox {
  min_lon: number;
  min_lat: number;
  max_lon: number;
  max_lat: number;
}

export interface ExtractedTrackSegment {
  track_id: string;
  start_position: number;
  end_position: number;
  coordinates: [number, number][]; // [lon, lat]
  is_track_start: boolean;
  is_track_end: boolean;
}

export interface SwitchPort {
  endpoint: string;
  track: string;
}

export interface RailSwitch {
  id: string;
  switch_type: string;
  ports: Record<string, SwitchPort>;
}

export interface BufferStop {
  id: string;
  track: string;
  position: number;
}

export interface Detector {
  id: string;
  track: string;
  position: number;
}

export interface ExtractedSchema {
  tracks: ExtractedTrackSegment[];
  switches: RailSwitch[];
  buffer_stops: BufferStop[];
  detectors: Detector[];
}

export interface LayoutNode {
  id: string;
  x: number;
  y: number;
  kind: "switch" | "buffer_stop" | "cut";
}

export interface LayoutEdge {
  from: string;
  to: string;
  track_id: string;
  lane: number;
}

export interface SchematicLayout {
  nodes: LayoutNode[];
  edges: LayoutEdge[];
}

export const continuumApi = {
  health: () => request<{ status: string }>("/health"),
  listBranches: () => request<string[]>("/branches"),
  getBranch: (name: string) => request<Graph>(`/branches/${encodeURIComponent(name)}`),
  diff: (base: string, compare: string) =>
    request<DiffResult>(
      `/diff?base=${encodeURIComponent(base)}&compare=${encodeURIComponent(compare)}`
    ),
  createBranch: (name: string, fromCommit?: string) =>
    postJson<{ name: string; commit_id: string }>("/branches", {
      name,
      from_commit: fromCommit,
    }),
  commitOnBranch: (
    branch: string,
    params: { author: string; message: string; addNodes?: RailNode[]; removeNodes?: string[] }
  ) =>
    postJson<{ commit_id: string }>(`/branches/${encodeURIComponent(branch)}/commits`, {
      author: params.author,
      message: params.message,
      add_nodes: params.addNodes ?? [],
      remove_nodes: params.removeNodes ?? [],
    }),
  getHistory: (branch: string) =>
    request<Commit[]>(`/branches/${encodeURIComponent(branch)}/history`),
  getCommitGraph: (commitId: string) => request<Graph>(`/commits/${encodeURIComponent(commitId)}/graph`),

  /**
   * Contrairement à `request`, ne lève pas d'exception sur un 409 : un
   * conflit de fusion est un résultat attendu à afficher, pas une erreur
   * réseau/serveur — seuls les statuts vraiment inattendus sont levés.
   */
  merge: async (source: string, target: string, author: string, message: string): Promise<MergeResult> => {
    const response = await fetch(`${API_BASE_URL}/merge`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source, target, author, message }),
    });
    const body = await response.json();
    if (response.status === 201) {
      return { status: "merged", commitId: body.commit_id as string };
    }
    if (response.status === 409 && Array.isArray(body.conflicts)) {
      return { status: "conflicts", conflicts: body.conflicts as MergeConflict[] };
    }
    if (response.status === 404 || response.status === 409) {
      return { status: "error", message: (body.error as string) ?? "fusion impossible" };
    }
    throw new Error(`Erreur API (${response.status}) sur /merge`);
  },

  /**
   * Applique les résolutions choisies par l'utilisateur pour chaque
   * conflit, et committe le résultat. Même logique de statuts que
   * `merge` : un 409 avec des conflits restants (résolutions
   * incomplètes, ou branches ayant bougé) n'est pas une exception.
   */
  mergeResolve: async (
    source: string,
    target: string,
    author: string,
    message: string,
    resolutions: ConflictResolution[]
  ): Promise<MergeResult> => {
    const response = await fetch(`${API_BASE_URL}/merge/resolve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source, target, author, message, resolutions }),
    });
    const body = await response.json();
    if (response.status === 201) {
      return { status: "merged", commitId: body.commit_id as string };
    }
    if (response.status === 409 && Array.isArray(body.conflicts)) {
      return { status: "conflicts", conflicts: body.conflicts as MergeConflict[] };
    }
    if (response.status === 404 || response.status === 409) {
      return { status: "error", message: (body.error as string) ?? "fusion impossible" };
    }
    throw new Error(`Erreur API (${response.status}) sur /merge/resolve`);
  },

  /**
   * Endpoint de développement (`POST /debug/seed-conflict`), pas destiné
   * à un usage en production : génère en une fois un conflit spatial de
   * démonstration réaliste, pour tester la détection sans construire le
   * scénario à la main. Renvoie les deux branches à comparer.
   */
  seedDemoConflict: () =>
    postJson<{ branch_a: string; branch_b: string }>("/debug/seed-conflict", {}),

  extractSchema: (bbox: Bbox) => postJson<ExtractedSchema>("/schema/extract", bbox),
  layoutSchema: (bbox: Bbox) => postJson<SchematicLayout>("/schema/layout", bbox),
};
