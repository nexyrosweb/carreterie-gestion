# Sauvegarde et restauration

## Créer une sauvegarde

1. Ouvrir **Sauvegardes**.
2. Optionnel : saisir un mot de passe pour chiffrer l’archive (AES-256-GCM).
3. Cliquer sur **Créer une sauvegarde**.

La sauvegarde contient :

- une copie cohérente de la base SQLite (`VACUUM INTO`) ;
- les pièces jointes ;
- un manifeste SHA-256 pour contrôle d’intégrité.

Fichiers générés dans :

`%APPDATA%\fr.nexyros.carreterie-gestion\backups\`

Formats :

- `.zip` — non chiffré
- `.zip.enc` — chiffré

Vous pouvez copier ces fichiers vers un disque externe ou un NAS.

## Restaurer une sauvegarde

1. Ouvrir **Sauvegardes**.
2. Choisir le fichier `.zip` ou `.zip.enc`.
3. Si l’archive est chiffrée, saisir le mot de passe.
4. Confirmer la restauration.
5. **Redémarrer l’application** après restauration.

La restauration vérifie l’empreinte SHA-256 de la base avant remplacement.  
Une copie de secours `carreterie.db.bak` est créée avant écrasement.

## Bonnes pratiques

- Sauvegarder avant chaque clôture de période.
- Conserver au moins une copie hors du poste (disque externe).
- Activer BitLocker sur le disque Windows.
- Ne pas supprimer manuellement le dossier AppData sans sauvegarde préalable.
