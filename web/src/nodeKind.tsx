import type { NodeKind } from "./api";

export const KIND_LABELS: Record<NodeKind, string> = {
  Voie: "Voie",
  AppareilDeVoie: "Appareil de voie",
  Signal: "Signal",
  Sillon: "Sillon",
  Horaire: "Horaire",
  ProjetInvestissement: "Projet d'investissement",
};

// Chaque type d'objet du graphe hétérogène a sa propre couleur, à la façon
// dont OSRD distingue ses catégories de trains — voir theme.css.
export const KIND_CLASS: Record<NodeKind, string> = {
  Voie: "kind-voie",
  AppareilDeVoie: "kind-appareil",
  Signal: "kind-signal",
  Sillon: "kind-sillon",
  Horaire: "kind-horaire",
  ProjetInvestissement: "kind-projet",
};

/** Ordre d'affichage des groupes dans `NodeGroupList` — l'infrastructure
 * physique d'abord (voies, appareils, signaux), puis le reste. */
export const KIND_ORDER: NodeKind[] = [
  "Voie",
  "AppareilDeVoie",
  "Signal",
  "Sillon",
  "Horaire",
  "ProjetInvestissement",
];

export function KindBadge({ kind }: { kind: NodeKind }) {
  return <span className={`kind-badge ${KIND_CLASS[kind]}`}>{KIND_LABELS[kind] ?? kind}</span>;
}
