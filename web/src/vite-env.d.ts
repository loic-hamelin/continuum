/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** URL de l'éditeur OSRD, utilisée par l'action "Ouvrir dans l'éditeur
   * OSRD" du panneau de résolution de conflits. Défaut : http://localhost:4000. */
  readonly VITE_OSRD_EDITOR_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
