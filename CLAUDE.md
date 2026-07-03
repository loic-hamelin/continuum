# Contexte pour Claude Code

Ce fichier est lu automatiquement par Claude Code au début de chaque session dans ce dépôt. Il sert à donner le contexte du projet sans avoir à le réexpliquer à chaque fois.

## Le projet

CONTINUUM est un système de gestion de versions (façon Git) appliqué au jumeau numérique ferroviaire. Voir `README.md` pour la vue d'ensemble, et `docs/theorie.md` pour le cadre conceptuel complet.

## Concepts clés à connaître

- **Graphe versionné hétérogène** : le système ferroviaire est représenté comme un graphe d'objets de types différents (voie, appareil de voie, sillon, horaire, projet d'investissement...) reliés par des relations typées (dépendance, contrainte capacitaire, appartenance...).
- **Commit** : une modification élémentaire et tracée du graphe (auteur, justification, date, contexte).
- **Branche** : une exploration d'hypothèse concurrente, isolée de l'état de référence validé.
- **Merge** : fusion d'une branche dans l'état de référence, après revue.
- **Diff** : comparaison quantifiée entre deux versions du graphe.
- **Time machine** : navigation dans l'historique des décisions.
- **OSRD / RailJSON** : moteur de simulation ferroviaire open-source et son format de données, utilisé comme moteur de calcul pour évaluer les conséquences (capacité, robustesse) de chaque branche — CONTINUUM ne réimplémente pas la simulation, il l'orchestre.

## Pile technique

- **Rust** pour le moteur de graphe (`graph-engine`), le pont OSRD (`osrd-bridge`), l'API HTTP (`api`, sur le modèle d'editoast dans OSRD) et la ligne de commande (`cli`) — workspace Cargo unique à la racine.
- **React + TypeScript (Vite)** pour l'interface graphique (`web/`), qui consomme l'API via `web/src/api.ts`.
- Design tokens dans `web/src/theme.css`, inspirés de l'esthétique OSRD — à ajuster une fois comparé à l'interface réelle.

## Point de vigilance pour la première session

Le crate `api` (`graph-engine`, `osrd-bridge`, `cli` compilent tous et ont été vérifiés) a été écrit avec soin mais **n'a pas pu être compilé de bout en bout** dans l'environnement qui a généré ce squelette (Rust 1.75 trop ancien pour résoudre les dernières versions d'`actix-web` sur crates.io). Sur une machine avec un Rust récent (via rustup), ça devrait fonctionner directement — mais la toute première chose à faire est de lancer `cargo build -p continuum-api` et de corriger les éventuelles erreurs de compilation avant d'aller plus loin.

## Préférences de travail

- L'utilisateur est débutant en développement logiciel moderne. Expliquer les commandes avant de les exécuter, éviter le jargon non expliqué, aller étape par étape.
- Toujours demander confirmation avant un `git push`.
- Privilégier des étapes petites et vérifiables plutôt que de gros blocs de code d'un coup.
- Langue de travail : français.

## Pour démarrer une session de développement

Décrire ici la prochaine tâche concrète (ex: "modéliser les nœuds de base du graphe en Python", "écrire un premier script qui lit un fichier RailJSON d'exemple"), et laisser Claude Code proposer un plan avant d'écrire du code.
