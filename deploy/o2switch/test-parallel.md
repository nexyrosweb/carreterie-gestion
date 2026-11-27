# Test multi-PC simultané (o2switch)

1. PC A et PC B : même version de l’app, base partagée activée, **même** config MySQL.
2. Sur les deux : redémarrer l’app après activation (session « MySQL distant »).
3. PC A : créer un contact « Test parallèle A ».
4. PC B : actualiser la liste contacts → le contact A apparaît.
5. PC B : créer une facture brouillon pour ce contact.
6. PC A : ouvrir la facture → visible.
7. PC A : enregistrer un paiement partiel ; PC B : recharger → statut / montant à jour.

Si une IP change (box), ré-ajouter l’hôte dans **MySQL distant** o2switch.
