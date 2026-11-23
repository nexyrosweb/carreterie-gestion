# Manuel utilisateur — Carreterie Gestion

## Connexion

- Première ouverture (installation) : création obligatoire du compte administrateur.
- Un mot de passe sécurisé de **12 caractères** est généré automatiquement ; il est modifiable et régénérable.
- Conservez ce mot de passe dans un endroit sûr (bouton Copier disponible).
- Ensuite : identifiant + mot de passe.
- Après inactivité : écran de verrouillage (ressaisir le mot de passe).
- Changement / régénération possible dans **Paramètres**.

## Tableau de bord

Indicateurs : recettes (factures émises TTC), dépenses fournisseurs, factures impayées, brouillons, contacts.

## Contacts

Gérer clients et fournisseurs : raison sociale, adresse, e-mail, téléphone, SIRET.

## Factures clients

1. Créer un **brouillon** (client, lignes, TVA, échéance).
2. Vérifier les totaux HT / TVA / TTC.
3. **Émettre** : attribution d’un numéro unique ; la facture devient non modifiable.
4. Générer le **PDF** et l’imprimer / archiver.
5. Enregistrer les **règlements**.
6. Pour corriger une facture émise : créer un **avoir** (jamais supprimer la facture émise).

## Factures fournisseurs

Enregistrer la facture reçue, le montant, la référence, et joindre le PDF ou l’image justificative.

## Import PDF

Importer d’anciennes factures (clients, acomptes, avoirs, fournisseurs) :

1. Ouvrir **Import PDF** et sélectionner les fichiers.
2. Vérifier l’aperçu et corriger les champs extraits.
3. Associer ou créer un contact (sans fusion automatique).
4. Valider pour archiver — aucune numérotation officielle n’est générée.

Détails : `docs/IMPORT_PDF.md`.

## Archives

Consulter l’historique, le journal d’audit, et exporter un CSV des factures.

## Sauvegardes

Voir `docs/SAUVEGARDE_RESTAURATION.md`.

## Paramètres

Coordonnées du théâtre, logo, préfixes de numérotation, mentions légales, taux de TVA par défaut, dossier d’export, changement de mot de passe, section « À propos et mises à jour », réinitialisation démo (en mode démonstration).

Sur l’écran de connexion : **Essayer la démonstration** ouvre une base `demo.db` isolée (données fictives, bandeau, filigrane PDF). La production n’est jamais modifiée.

Mention discrète : **Développé par Nexyros**.

## À propos

Version du logiciel, éditeur Nexyros, et rappel : non plateforme agréée de facturation électronique.
