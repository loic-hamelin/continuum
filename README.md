# CONTINUUM

**Système de gestion de versions et de cycle de vie temporel du jumeau numérique ferroviaire, fondé sur OSRD.**

> Dépôt privé — développement en cours. Candidature déposée dans le cadre de l'Appel à Projets 2026 FIF / IRT Railenium.

## Idée en une phrase

Appliquer au système ferroviaire (voies, appareils de voie, sillons, horaires, projets d'investissement...) les principes qui ont transformé l'industrie du logiciel avec les systèmes de gestion de versions de type Git : un état de référence validé, des branches pour explorer des hypothèses concurrentes, des commits qui tracent chaque décision, un mécanisme de comparaison (diff), une revue avant fusion (merge), et une navigation dans l'historique (time machine).

Le moteur de simulation open-source **OSRD** (et son modèle de données **RailJSON**) sert de moteur de calcul : il évalue automatiquement les conséquences (capacité, robustesse, performance) de chaque branche du graphe.

## Statut du projet

- [x] Cadre théorique rédigé (graphe versionné hétérogène, typologie des conflits, fonctions commit/branche/fusion/time machine)
- [x] Dossier de candidature AAP 2026 FIF/Railenium
- [x] Squelette du projet : workspace Rust (graph-engine, osrd-bridge, cli) + interface React (web)
- [ ] Modélisation complète du graphe d'objets ferroviaires versionnés (branches, merge)
- [ ] Prototype d'intégration avec OSRD / RailJSON
- [ ] Mécanisme de détection de conflits
- [ ] Lien moteur Rust ↔ interface React (API ou WebAssembly)
- [ ] Interface de navigation temporelle (time machine)

## Structure du dépôt

```
continuum/
├── docs/            théorie, architecture, notes de conception
├── graph-engine/    cœur en Rust : modèle de graphe versionné (nœuds, arêtes, commits, diff)
├── osrd-bridge/     crate Rust : intégration avec OSRD / RailJSON (moteur de simulation)
├── cli/             interface en ligne de commande, en Rust
├── web/             interface graphique, en React + TypeScript (Vite)
└── examples/        jeux de données d'exemple pour les démonstrateurs
```

## Pile technique

- **Moteur de graphe et logique métier : Rust** (workspace Cargo à la racine, 3 crates : `graph-engine`, `osrd-bridge`, `cli`) — choisi pour la robustesse (typage fort, pas de null, gestion d'erreurs explicite) et la performance sur un modèle de graphe qui devra, à terme, orchestrer des simulations à grande échelle.
- **Interface graphique : React + TypeScript** (Vite), dans `web/` — pour l'exploration visuelle du graphe, la navigation entre branches et la « time machine ».
- **Interopérabilité** : format RailJSON (OSRD) en entrée/sortie, côté `osrd-bridge`.
- Le lien entre le moteur Rust et l'interface React reste à concevoir (API HTTP classique, ou compilation du moteur en WebAssembly pour l'exécuter directement dans le navigateur) — sujet à trancher lors d'une prochaine session avec Claude Code.

## Démarrer en local

**Rust (moteur + CLI)**
```
cargo build
cargo test
cargo run -p continuum-cli
```

**React (interface)**
```
cd web
npm install
npm run dev
```

## Licence

Non définie — dépôt privé pour l'instant. À décider avant toute ouverture publique du code (MIT, Apache 2.0, etc. sont les choix classiques pour un projet open-source appuyé sur un écosystème existant comme OSRD).
