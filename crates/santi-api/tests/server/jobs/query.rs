use super::*;
use santi_core::job::{Query, QueryCount, QueryError, QueryRequest};

fn structured(strand: Option<&str>, all_strands: bool, count: bool) -> QueryRequest {
    QueryRequest {
        q: None,
        strand: strand.map(str::to_string),
        all_strands,
        state: Vec::new(),
        limit: None,
        count,
    }
}

#[tokio::test]
async fn scoped() {
    let temp = tempfile::tempdir().expect("temp dir");
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let (service, strand, _store) = fixture::prepare(&temp, supervisor).await;

    match service
        .job_query(&strand.soul, structured(None, true, true))
        .await
        .expect("count")
    {
        Query::Count(QueryCount { count }) => assert_eq!(count, 0),
        Query::Rows(_) => panic!("count request returned rows"),
    }

    let mut rows = structured(Some(&strand.id), false, false);
    rows.limit = Some(5);
    match service.job_query(&strand.soul, rows).await.expect("rows") {
        Query::Rows(rows) => assert!(rows.rows.is_empty()),
        Query::Count(_) => panic!("rows request returned count"),
    }
}

#[tokio::test]
async fn rejects() {
    let temp = tempfile::tempdir().expect("temp dir");
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let (service, strand, _store) = fixture::prepare(&temp, supervisor).await;

    let mut native = structured(None, true, false);
    native.q = Some("from Job where supervisor_ref = \"x\" limit 5".to_string());
    let error = service
        .job_query(&strand.soul, native)
        .await
        .expect_err("private field must be rejected");
    assert!(matches!(error, QueryError::Invalid(_)));

    let mut mixed = structured(Some(&strand.id), false, true);
    mixed.q = Some("from Job count".to_string());
    let error = service
        .job_query(&strand.soul, mixed)
        .await
        .expect_err("native and structured fields must not mix");
    assert!(matches!(error, QueryError::Invalid(_)));

    let unknown = serde_json::json!({ "limit": 5, "after": 1 });
    assert!(serde_json::from_value::<QueryRequest>(unknown).is_err());
}
