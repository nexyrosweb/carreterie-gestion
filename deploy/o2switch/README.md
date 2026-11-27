# Base partagée sur o2switch (multi-PC, hors site)

L’app reste installée sur chaque PC. La **base MySQL/MariaDB o2switch** devient la source unique (accès simultané).  
Pas de Docker / VPS : uniquement le cPanel o2switch.

## 1. Créer la base (cPanel)

1. Ouvre **Bases de données MySQL®** (ou l’**Assistant**).
2. Crée une base, ex. `compte_carreterie`.
3. Crée un utilisateur MySQL dédié + mot de passe fort.
4. **Ajoute l’utilisateur à la base** avec **tous les privilèges**.

## 2. Autoriser les PC distants

1. Ouvre **MySQL distant®**.
2. Ajoute l’hôte de chaque PC :
   - soit l’**IP publique** de chaque site (recommandé),
   - soit `%` en test (moins sûr : ouvre à tout Internet).
3. Note le **serveur MySQL** affiché par o2switch (souvent `mysql.ton-domaine.fr` ou un hôte du type `*.o2switch.net`).

PostgreSQL (phpPgAdmin) sur o2switch est surtout pensé pour les sites **locaux** au serveur. Pour des PC Windows ailleurs, **MySQL distant** est la voie prévue.

## 3. Importer le schéma

1. Ouvre **phpMyAdmin**.
2. Sélectionne la base créée.
3. Onglet **Importer** → fichier [`schema.sql`](schema.sql) (ou colle le SQL).

## 4. Brancher l’application

Dans **Paramètres → Base partagée (o2switch)** :

| Champ        | Exemple                          |
|-------------|-----------------------------------|
| Activer     | oui                               |
| Hôte        | `mysql.exemple.o2switch.net`      |
| Port        | `3306`                            |
| Base        | `compte_carreterie`               |
| Utilisateur | `compte_carreuser`                |
| Mot de passe| (celui du cPanel)                 |
| TLS/SSL     | recommandé si proposé             |

Enregistre, redémarre l’app si demandé, reconnecte-toi.

## 5. Migrer les données déjà sur un PC

Sur le PC admin (qui a déjà `carreterie.db`) :

1. Paramètres → Base partagée → **Migrer SQLite → MySQL**.
2. Vérifie contacts / factures sur le 2ᵉ PC.

## 6. Pièces jointes (PDF)

En mode partagé, les fichiers sont stockés dans la table MySQL `file_blobs` (plus de dossier local comme source de vérité).  
Chaque PC télécharge au besoin dans un cache local.

## Sécurité

- N’utilise `%` en hôte distant qu’en test.
- Mot de passe MySQL ≠ mot de passe admin de l’app.
- Sauvegarde : Export SQL via phpMyAdmin, ou outil Sauvegardes de l’app si disponible en mode distant.

## Dépannage

| Symptôme | Piste |
|----------|--------|
| Connexion refusée | MySQL distant : IP du PC absente / mauvais hôte |
| Access denied | User pas lié à la base / mauvais mot de passe |
| Timeout | Pare-feu box / IP qui a changé (réajouter l’hôte) |
| SSL error | Désactive TLS à l’essai, ou active « accepter certificat serveur » |
