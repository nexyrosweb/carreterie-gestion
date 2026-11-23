# Publication Carreterie Gestion (Nexyros)

## Dépôts

| Rôle | Usage |
|------|--------|
| Code source (privé) | Développement, CI, secrets |
| Distribution (`Nexyros/carreterie-gestion-releases`) | GitHub Releases + `latest.json` public pour l’updater |

L’endpoint updater dans `src-tauri/tauri.conf.json` pointe vers le dépôt de distribution.

## Données utilisateur (conservées après mise à jour)

Sous `%APPDATA%\fr.nexyros.carreterie-gestion\` :

- `carreterie.db` — production
- `demo\demo.db` — démonstration isolée
- `attachments\`, `backups\` (et équivalents sous `demo\`)

L’installateur NSIS n’écrit pas dans AppData ; les migrations SQLite sont versionnées et une copie `backups\pre-migrate\` est créée au démarrage.

## Signature des mises à jour

1. Générer une paire (une seule fois) :  
   `npx tauri signer generate -w .keys/carreterie.key -p "<mot-de-passe>"`
2. Copier la **clé publique** dans `tauri.conf.json` → `plugins.updater.pubkey`
3. Stocker en **Repository secrets** GitHub (Settings → Secrets → Actions), **pas** seulement dans un Environment :

```powershell
# Depuis la racine du projet (PowerShell)
gh secret set TAURI_SIGNING_PRIVATE_KEY -R nexyrosweb/carreterie-gestion < .keys/carreterie.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD -R nexyrosweb/carreterie-gestion -b "VOTRE_MOT_DE_PASSE"
```

Erreur CI `Missing comment in secret key` / `incorrect updater private key password` :
- secret clé vide, tronqué, ou collé avec des retours à la ligne en trop ;
- **mot de passe absent ou incorrect** (la clé générée localement est chiffrée) ;
- secrets placés dans un Environment GitHub alors que le job n’y a pas accès.

TLS et vérification de signature restent actifs en production.

## Processus de release

1. Bump SemVer dans `package.json` et `src-tauri/tauri.conf.json` / `Cargo.toml`
2. Créer un tag `vX.Y.Z` **ou** lancer le workflow `Release Windows` (dispatch)
3. Valider le draft release GitHub (artefacts `.exe`, `.sig`, `latest.json`)
4. Publier la release après contrôle

## Tests manuels recommandés

- Séparation `carreterie.db` / `demo.db`
- Réinitialisation démo
- PDF filigrane en démo
- Mise à jour signée 1.0.0 → 1.0.1
- Hors ligne : pas de blocage facturation
- Signature incorrecte refusée
- Interruption téléchargement
- Conservation AppData après update
- Échec migration : restauration depuis `backups\pre-migrate`
- Windows 10 et 11

## Icônes

Régénérer depuis le logo TLC :

```bash
npx tauri icon src/assets/brand/icon-tlc.png -o src-tauri/icons
```

Puis reconstruire l’installateur pour que l’EXE / le setup affichent l’icône correcte.
