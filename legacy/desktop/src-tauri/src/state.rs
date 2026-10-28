pub use explorer_app::state::{application_state_dir, AppState};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn corrupt_durable_state_never_creates_an_available_executor() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "startup fixture",
        )
        .unwrap();
        let lease = Arc::new(
            explorer_win::instance::InstanceLease::acquire(root.path())
                .unwrap()
                .unwrap(),
        );
        let db = root.path().join("state.sqlite3");
        std::fs::write(&db, "corrupt SQLite fixture").unwrap();
        assert!(AppState::new(root.path(), lease).is_err());
        assert_eq!(
            std::fs::read_to_string(db).unwrap(),
            "corrupt SQLite fixture"
        );
    }
}
