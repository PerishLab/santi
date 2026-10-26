use super::support::*;
use santi_core::object;
use santi_core::service::{self, Service};

#[tokio::test]
async fn scopes() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = open(&temp).await;
    let strand = service.weave().await.expect("create strand").strand;
    let bucket = object::Bucket::new("soul_default", strand.id.as_str()).expect("bucket");
    let uri = object::Uri::new(bucket.clone(), "avatars/santi.svg").expect("uri");

    let meta = service
        .stash(&uri, b"<svg>avatar</svg>")
        .await
        .expect("put object");
    assert_eq!(meta.uri.to_string(), uri.to_string());
    assert_eq!(meta.len, 17);
    assert_eq!(
        service
            .renderable(&uri.to_string())
            .expect("renderable ref"),
        format!(
            "/api/v1/bucket/soul_default/{}/avatars/santi.svg",
            strand.id
        )
    );

    let object = service
        .fetch("soul_default", &strand.id, "avatars/santi.svg")
        .await
        .expect("get object")
        .expect("object exists");
    assert_eq!(object.bytes, b"<svg>avatar</svg>");
    let objects = service
        .shelve(&bucket, Some("avatars"))
        .await
        .expect("list objects");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].uri.key, "avatars/santi.svg");
    let objects = service
        .shelve(&bucket, Some("avatars/santi"))
        .await
        .expect("list object prefix");
    assert_eq!(objects.len(), 1);
    assert!(service.erase(&uri).await.expect("delete object"));
    assert!(
        service
            .fetch("soul_default", &strand.id, "avatars/santi.svg")
            .await
            .expect("get deleted object")
            .is_none()
    );
}

#[tokio::test]
async fn rejects() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = open(&temp).await;
    let strand = service.weave().await.expect("create strand").strand;

    assert!(
        service
            .fetch("soul_default", &strand.id, "../escape.txt")
            .await
            .expect_err("unsafe key")
            .contains("object key")
    );
    assert!(
        service
            .fetch("soul_default", &strand.id, "bad//key.txt")
            .await
            .expect_err("empty segment")
            .contains("object key")
    );
    assert!(
        service
            .fetch("unknown_soul", &strand.id, "safe.txt")
            .await
            .expect_err("unknown soul")
            .contains("soul not found")
    );
}

#[tokio::test]
async fn removal() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = open(&temp).await;
    let strand = service.weave().await.expect("create strand").strand;
    let bucket = object::Bucket::new("soul_default", strand.id.as_str()).expect("bucket");
    let uri = object::Uri::new(bucket, "avatars/santi.svg").expect("uri");
    service
        .stash(&uri, b"<svg>avatar</svg>")
        .await
        .expect("put object");
    let runtime = temp.path().join("runtime");
    let home = runtime.join("strands").join(&strand.id);
    std::fs::create_dir_all(home.join("memory")).expect("strand home");
    std::fs::write(home.join("memory/MEMORY.md"), "notes").expect("memory");

    service.end_strand(&strand.id).await.expect("end strand");

    assert!(!home.exists());
    assert!(
        !runtime
            .join("buckets/soul_default")
            .join(&strand.id)
            .exists()
    );
    assert!(
        service
            .fetch("soul_default", &strand.id, "avatars/santi.svg")
            .await
            .expect_err("ended strand")
            .contains("strand not found")
    );
}

#[tokio::test]
async fn tidy() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = open(&temp).await;
    let strand = service.weave().await.expect("create strand").strand;
    let bucket = object::Bucket::new("soul_default", strand.id.as_str()).expect("bucket");
    let uri = object::Uri::new(bucket, "kept.txt").expect("uri");
    service.stash(&uri, b"kept").await.expect("put object");
    drop(service);
    let runtime = temp.path().join("runtime");
    let strands = runtime.join("strands");
    let buckets = runtime.join("buckets/soul_default");
    let orphans = [
        strands.join("strand_ghost/memory/MEMORY.md"),
        buckets.join("strand_ghost/left.txt"),
        buckets.join(&strand.id).join("stray/unrecorded.txt"),
        runtime.join("jobs/job_ghost/output.log"),
    ];
    let trash = runtime.join("jobs/.gc/job_archived/output.log");
    let memory = strands.join(&strand.id).join("memory/MEMORY.md");
    for path in orphans.iter().chain([&trash, &memory]) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, "bytes").expect("write");
    }

    let service = open(&temp).await;

    for path in &orphans {
        assert!(!path.exists(), "{} survived", path.display());
    }
    assert!(!strands.join("strand_ghost").exists());
    assert!(!runtime.join("jobs/job_ghost").exists());
    assert!(trash.exists());
    assert!(memory.exists());
    let object = service
        .fetch("soul_default", &strand.id, "kept.txt")
        .await
        .expect("get object")
        .expect("recorded object");
    assert_eq!(object.bytes, b"kept");
}

async fn open(temp: &tempfile::TempDir) -> Service {
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(FakeProvider::default()),
    )
    .await
    .expect("open service")
}
