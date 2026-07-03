import type { ConflictResolution, ConflictSide, MergeConflict } from "./api";

/** Clé stable identifiant un conflit, pour les listes React et le suivi
 * des résolutions choisies (Record<conflictKey, side>). */
export function conflictKey(conflict: MergeConflict): string {
  switch (conflict.kind) {
    case "modification":
    case "deletion_vs_modification":
      return `${conflict.kind}:${conflict.node_id}`;
    case "spatial":
      return `spatial:${conflict.source_node.id}|${conflict.target_node.id}`;
  }
}

/** Description courte et lisible d'un conflit, pour le panneau de résolution. */
export function conflictDescription(conflict: MergeConflict): string {
  switch (conflict.kind) {
    case "modification":
      return `« ${conflict.source.label} » a été modifié différemment des deux côtés.`;
    case "deletion_vs_modification": {
      const deletedSide = conflict.deleted_in === "source" ? "la source" : "la cible";
      const modifiedSide = conflict.deleted_in === "source" ? "la cible" : "la source";
      return `« ${conflict.modified.label} » a été supprimé côté ${deletedSide}, mais modifié côté ${modifiedSide}.`;
    }
    case "spatial":
      return `« ${conflict.source_node.label} » et « ${conflict.target_node.label} » sont à ${conflict.distance_meters.toFixed(
        1
      )}m l'un de l'autre sur la voie ${conflict.track}.`;
  }
}

/** Les ids de nœuds à surligner sur la carte "source" pour ce conflit. */
export function conflictSourceNodeIds(conflict: MergeConflict): string[] {
  switch (conflict.kind) {
    case "modification":
    case "deletion_vs_modification":
      return [conflict.node_id];
    case "spatial":
      return [conflict.source_node.id];
  }
}

/** Les ids de nœuds à surligner sur la carte "cible" pour ce conflit. */
export function conflictTargetNodeIds(conflict: MergeConflict): string[] {
  switch (conflict.kind) {
    case "modification":
    case "deletion_vs_modification":
      return [conflict.node_id];
    case "spatial":
      return [conflict.target_node.id];
  }
}

/** Construit la résolution correspondant au choix "garder {side}" pour ce conflit. */
export function toResolution(conflict: MergeConflict, keep: ConflictSide): ConflictResolution {
  switch (conflict.kind) {
    case "modification":
      return { kind: "modification", node_id: conflict.node_id, keep };
    case "deletion_vs_modification":
      return { kind: "deletion_vs_modification", node_id: conflict.node_id, keep };
    case "spatial":
      return {
        kind: "spatial",
        source_node_id: conflict.source_node.id,
        target_node_id: conflict.target_node.id,
        keep,
      };
  }
}

/** Les ids d'objets à copier dans le presse-papier pour "Ouvrir dans
 * l'éditeur OSRD" — un seul id pour modification/suppression, les deux
 * pour un conflit spatial (deux objets distincts). */
export function conflictEditorIds(conflict: MergeConflict): string[] {
  switch (conflict.kind) {
    case "modification":
    case "deletion_vs_modification":
      return [conflict.node_id];
    case "spatial":
      return [conflict.source_node.id, conflict.target_node.id];
  }
}
