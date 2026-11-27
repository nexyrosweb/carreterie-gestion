//! Vérifie qu'un scénario type « 2 opérateurs » reste cohérent sur SQLite local
//! (même logique métier que le miroir MySQL o2switch).
//! Test manuel distant : voir `deploy/o2switch/test-parallel.md`.

#[cfg(test)]
mod tests {
    use crate::db::{run_migrations, DbPool, SqlVal};
    use crate::models::contact::{ContactInput, ContactKind};
    use crate::services::contacts::ContactsService;

    #[test]
    fn two_sequential_clients_see_each_others_contacts() {
        let db = DbPool::open_in_memory().expect("mem");
        db.with_conn_mut(|conn| run_migrations(conn)).expect("migrate");

        let a = ContactsService::create(
            &db,
            "user-a",
            ContactInput {
                kind: ContactKind::Client,
                name: "Client Parallele A".into(),
                email: None,
                phone: None,
                address_line1: None,
                address_line2: None,
                postal_code: None,
                city: None,
                country: Some("France".into()),
                siret: None,
                vat_number: None,
                notes: None,
            },
        )
        .expect("create A");

        let list_b = ContactsService::list(&db, None).expect("list");
        assert!(list_b.iter().any(|c| c.id == a.id));

        let b = ContactsService::create(
            &db,
            "user-b",
            ContactInput {
                kind: ContactKind::Client,
                name: "Client Parallele B".into(),
                email: None,
                phone: None,
                address_line1: None,
                address_line2: None,
                postal_code: None,
                city: None,
                country: Some("France".into()),
                siret: None,
                vat_number: None,
                notes: None,
            },
        )
        .expect("create B");

        let list_a = ContactsService::list(&db, None).expect("list2");
        assert!(list_a.iter().any(|c| c.id == a.id));
        assert!(list_a.iter().any(|c| c.id == b.id));

        let n = db
            .query("SELECT COUNT(*) FROM contacts", &[])
            .unwrap()
            .first()
            .and_then(|r| r.first())
            .map(|c| c.as_i64().unwrap_or(0))
            .unwrap_or(0);
        assert_eq!(n, 2);

        let _ = SqlVal::Int(n);
    }
}
