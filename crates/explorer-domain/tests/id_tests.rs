use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{ItemToken, JobId, PlanId, SessionId};

#[test]
fn test_ids_uniqueness_and_serialization() {
    let s1 = SessionId::new();
    let s2 = SessionId::new();
    assert_ne!(s1, s2);

    let json = serde_json::to_string(&s1).expect("serialize");
    let deser: SessionId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(s1, deser);

    let t1 = ItemToken::new();
    let p1 = PlanId::new();
    let j1 = JobId::new();
    assert!(!t1.0.is_empty());
    assert!(!p1.0.is_empty());
    assert!(!j1.0.is_empty());
}

#[test]
fn test_structured_error() {
    let err = ExplorerError::new(ErrorCode::NotFound, "File not found", "open_item");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.user_message, "File not found");
    assert_eq!(err.operation, "open_item");
    assert!(!err.correlation_id.is_empty());

    let json = serde_json::to_string(&err).expect("serialize error");
    let deser: ExplorerError = serde_json::from_str(&json).expect("deserialize error");
    assert_eq!(deser.code, ErrorCode::NotFound);
}
