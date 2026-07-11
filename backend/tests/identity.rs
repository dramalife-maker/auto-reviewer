use std::process::Command;
use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::identity::{
    self, bind_identity, create_person, list_unmatched_authors, normalize_git_email,
    prepare_manifest_authors, resolve_person_by_email, KIND_GIT_EMAIL,
};
use reviewer_server::runs::{write_weekly_manifest, ProjectRow};
use reviewer_server::summary::ingest_project_summaries;
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

fn git_commit(path: &std::path::Path, email: &str, name: &str, message: &str) {
    let p = path.display().to_string();
    for args in [
        vec!["-C", &p, "config", "user.email", email],
        vec!["-C", &p, "config", "user.name", name],
        vec!["-C", &p, "commit", "--allow-empty", "-m", message],
    ] {
        let out = Command::new("git").args(&args).output().expect("git");
        assert!(out.status.success(), "git {args:?}");
    }
}

fn init_repo_with_commits(path: &std::path::Path, commits: &[(&str, &str, &str)]) {
    std::fs::create_dir_all(path).expect("source dir");
    let p = path.display().to_string();
    let out = Command::new("git")
        .args(["init", "-b", "main", &p])
        .output()
        .expect("git init");
    assert!(out.status.success());
    for (email, name, message) in commits {
        git_commit(path, email, name, message);
    }
}

#[tokio::test]
async fn git_author_email_is_normalized_for_identity_lookup() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let pool = init_pool(tempfile::tempdir().unwrap().path())
        .await
        .expect("init pool");

    let person_id = create_person(&pool, "Alice Chen")
        .await
        .expect("create person");
    bind_identity(
        &pool,
        person_id,
        KIND_GIT_EMAIL,
        "Alice@Company.COM",
        None,
    )
    .await
    .expect("bind identity");

    assert_eq!(normalize_git_email("  Alice@Company.COM  "), "alice@company.com");

    let resolved = resolve_person_by_email(&pool, "alice@company.com")
        .await
        .expect("resolve")
        .expect("found");
    assert_eq!(resolved.person_id, person_id);
    assert_eq!(resolved.display_name, "Alice Chen");
}

#[tokio::test]
async fn unmatched_authors_are_recorded_during_run_preparation() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");
    let repo = temp.path().join("repo");
    init_repo_with_commits(&repo, &[("unknown@example.com", "Ghost", "ghost work")]);

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(repo.display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let authors = prepare_manifest_authors(&pool, &repo, 1, "2000-01-01", "2099-12-31")
        .await
        .expect("prepare authors");
    assert!(authors.is_empty());

    let unmatched = list_unmatched_authors(&pool).await.expect("list unmatched");
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0].value, "unknown@example.com");
    assert_eq!(unmatched[0].commit_count, 1);
}

#[tokio::test]
async fn weekly_manifest_includes_resolved_authors() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");
    let repo = temp.path().join("repo");
    init_repo_with_commits(
        &repo,
        &[
            ("alice@co.com", "Alice", "alice work"),
            ("bob@other.com", "Bob", "bob work"),
        ],
    );

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(repo.display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let person_id = create_person(&pool, "Alice Chen")
        .await
        .expect("create person");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind alice");

    let project = ProjectRow {
        id: 1,
        name: "alpha".into(),
        repo_path: repo.display().to_string(),
    };
    let manifest_path =
        write_weekly_manifest(&pool, temp.path(), 42, &project, &project.repo_path)
            .await
            .expect("write manifest");

    let json: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    let authors = json["authors"].as_array().expect("authors array");
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0]["email"], "alice@co.com");
    assert_eq!(authors[0]["display_name"], "Alice Chen");

    let unmatched = list_unmatched_authors(&pool).await.expect("list unmatched");
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0].value, "bob@other.com");
}

#[tokio::test]
async fn manifest_includes_person_report_root() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");
    let repo = temp.path().join("repo");
    init_repo_with_commits(
        &repo,
        &[("alice@co.com", "Alice", "alice work")],
    );

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(repo.display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let person_id = create_person(&pool, "Alice Chen")
        .await
        .expect("create person");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind alice");

    let project = ProjectRow {
        id: 1,
        name: "alpha".into(),
        repo_path: repo.display().to_string(),
    };
    let manifest_path =
        write_weekly_manifest(&pool, temp.path(), 42, &project, &project.repo_path)
            .await
            .expect("write manifest");

    let json: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    let expected = temp
        .path()
        .join("reports")
        .join("_people")
        .display()
        .to_string()
        .replace('\\', "/");
    assert_eq!(json["person_report_root"], expected);
}

#[tokio::test]
async fn unmatched_authors_list_api() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    identity::record_unmatched_author(&pool, KIND_GIT_EMAIL, "ghost@example.com", 1, 3)
        .await
        .expect("record unmatched");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/unmatched-authors")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json.as_array().expect("array").len(), 1);
    assert_eq!(json[0]["value"], "ghost@example.com");
    assert_eq!(json[0]["project_name"], "game-backend");
}

#[tokio::test]
async fn create_person_api() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let app = build_app().await.expect("build app");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/people")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice Chen"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);

    let duplicate = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/people")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice Chen"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn administrator_can_pre_register_identities_before_review() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let app = build_app().await.expect("build app");

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/people")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice Chen"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(create.status(), StatusCode::CREATED);
    let body = create.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let person_id = json["id"].as_i64().expect("person id");

    let bind = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_id}/identities"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"kind":"git_email","value":"alice@co.com"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(bind.status(), StatusCode::NO_CONTENT);

    let list = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/identities"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(list.status(), StatusCode::OK);
    let body = list.into_body().collect().await.expect("body").to_bytes();
    let identities: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(identities[0]["value"], "alice@co.com");
}

#[tokio::test]
async fn bind_identity_to_person_api() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_a = create_person(&pool, "Alice Chen").await.expect("alice");
    let person_b = create_person(&pool, "Bob Lee").await.expect("bob");
    bind_identity(&pool, person_a, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind alice");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    identity::record_unmatched_author(&pool, KIND_GIT_EMAIL, "ghost@example.com", 1, 1)
        .await
        .expect("record unmatched");

    let app = build_app().await.expect("build app");
    let bind = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_b}/identities"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"kind":"git_email","value":"ghost@example.com"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(bind.status(), StatusCode::NO_CONTENT);

    let conflict = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_b}/identities"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"kind":"git_email","value":"alice@co.com"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let unmatched = list_unmatched_authors(&pool).await.expect("list unmatched");
    assert!(unmatched.is_empty());
}

#[tokio::test]
async fn list_identities_for_a_person_api() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice Chen").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", Some("work"))
        .await
        .expect("bind");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/identities"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json[0]["label"], "work");
}

#[tokio::test]
async fn cross_email_summaries_map_to_same_person_id() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let person_id = create_person(&pool, "Alice Chen").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind co");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@gmail.com", None)
        .await
        .expect("bind gmail");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    for (dir_name, date) in [("Alice Chen", "2026-07-05"), ("Alice Chen", "2026-07-06")] {
        let summary_path = temp
            .path()
            .join(format!("reports/game-backend/{dir_name}/{date}/summary.md"));
        std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(
            &summary_path,
            format!(
                r#"---
person: Alice Chen
project: game-backend
date: {date}
one_line: Stable week
commit_count: 1
---

## 待確認
- Question?
"#
            ),
        )
        .expect("write summary");
    }

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT person_id) FROM reports WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 1);

    let stored_person: i64 = sqlx::query_scalar("SELECT person_id FROM reports LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("person id");
    assert_eq!(stored_person, person_id);
}

#[tokio::test]
async fn summary_ingestion_skips_unknown_person() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Ghost/2026-07-05/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Ghost
project: game-backend
date: 2026-07-05
one_line: Should be skipped
---

## 待確認
- Question?
"#,
    )
    .expect("write summary");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let report_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(report_count, 0);

    let people_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM people")
        .fetch_one(&pool)
        .await
        .expect("people count");
    assert_eq!(people_count, 0);
}

#[tokio::test]
async fn person_detail_api_returns_identities_and_projects() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id = create_person(&pool, "Alice Chen").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", Some("work"))
        .await
        .expect("bind email");
    bind_identity(&pool, person_id, "gitlab_user", "alice.chen", None)
        .await
        .expect("bind gitlab");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0), ('web-portal', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .bind(temp.path().join("repos/web-portal").display().to_string())
    .execute(&pool)
    .await
    .expect("insert projects");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'success', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO reports (project_id, person_id, run_id, report_date, report_md_path, summary_md_path)
         VALUES (1, ?, ?, '2026-07-05', 'r.md', 's.md')",
    )
    .bind(person_id)
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("insert report");

    sqlx::query("INSERT INTO participation (project_id, person_id) VALUES (2, ?)")
        .bind(person_id)
        .execute(&pool)
        .await
        .expect("insert participation");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["id"], person_id);
    assert_eq!(json["display_name"], "Alice Chen");
    assert_eq!(json["identities"].as_array().expect("identities").len(), 2);
    let project_names: Vec<&str> = json["projects"]
        .as_array()
        .expect("projects")
        .iter()
        .map(|p| p["name"].as_str().expect("name"))
        .collect();
    assert!(project_names.contains(&"game-backend"));
    assert!(project_names.contains(&"web-portal"));
}

#[tokio::test]
async fn person_detail_api_empty_projects() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Solo").await.expect("create");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["projects"].as_array().expect("projects").len(), 0);
    assert_eq!(json["identities"].as_array().expect("identities").len(), 0);
}

#[tokio::test]
async fn person_detail_api_unknown_returns_404() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/people/99999")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rename_person_updates_database_and_people_directory() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice").await.expect("create");

    let old_dir = temp.path().join("reports").join("_people").join("Alice");
    std::fs::create_dir_all(&old_dir).expect("mkdir");
    std::fs::write(old_dir.join("index.md"), "notes").expect("write");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/people/{person_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice Chen"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["display_name"], "Alice Chen");

    let stored: String = sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_one(&pool)
        .await
        .expect("stored name");
    assert_eq!(stored, "Alice Chen");

    let new_dir = temp.path().join("reports").join("_people").join("Alice Chen");
    assert!(new_dir.is_dir(), "renamed directory should exist");
    assert!(!old_dir.exists(), "old directory should be gone");
    assert!(new_dir.join("index.md").is_file());
}

#[tokio::test]
async fn rename_person_rejects_colliding_destination_directory() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice").await.expect("create");

    let people_root = temp.path().join("reports").join("_people");
    std::fs::create_dir_all(people_root.join("Alice")).expect("mkdir alice");
    std::fs::create_dir_all(people_root.join("Alice Chen")).expect("mkdir collision");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/people/{person_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice Chen"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let stored: String = sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_one(&pool)
        .await
        .expect("stored name");
    assert_eq!(stored, "Alice");
}

#[tokio::test]
async fn rename_person_rejects_duplicate_display_name() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let _alice = create_person(&pool, "Alice").await.expect("alice");
    let bob = create_person(&pool, "Bob").await.expect("bob");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/people/{bob}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"Alice"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_identity_from_person_api() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice Chen").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind email");
    bind_identity(&pool, person_id, "gitlab_user", "alice.chen", None)
        .await
        .expect("bind gitlab");

    let identities = identity::list_identities_for_person(&pool, person_id)
        .await
        .expect("list");
    let identity_id = identities
        .iter()
        .find(|item| item.kind == "gitlab_user")
        .expect("gitlab identity")
        .id;

    let app = build_app().await.expect("build app");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/people/{person_id}/identities/{identity_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let list = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/identities"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(list.status(), StatusCode::OK);
    let body = list.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json.as_array().expect("array").len(), 1);
    assert_eq!(json[0]["kind"], "git_email");
}

#[tokio::test]
async fn delete_identity_wrong_person_returns_404() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_a = create_person(&pool, "Alice").await.expect("alice");
    let person_b = create_person(&pool, "Bob").await.expect("bob");
    bind_identity(&pool, person_a, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind");
    let identities = identity::list_identities_for_person(&pool, person_a)
        .await
        .expect("list");
    let identity_id = identities[0].id;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/people/{person_b}/identities/{identity_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let remaining = identity::list_identities_for_person(&pool, person_a)
        .await
        .expect("list");
    assert_eq!(remaining.len(), 1);
}

#[tokio::test]
async fn deleting_last_identity_is_allowed() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind");
    let identities = identity::list_identities_for_person(&pool, person_id)
        .await
        .expect("list");
    let identity_id = identities[0].id;

    let app = build_app().await.expect("build app");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/people/{person_id}/identities/{identity_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let list = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/identities"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = list.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json.as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn bind_gitlab_user_identity_preserves_case() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice Chen").await.expect("create");

    let app = build_app().await.expect("build app");
    let bind = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_id}/identities"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"kind":"gitlab_user","value":"  Alice.Chen  "}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(bind.status(), StatusCode::NO_CONTENT);

    let identities = identity::list_identities_for_person(&pool, person_id)
        .await
        .expect("list");
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].kind, "gitlab_user");
    assert_eq!(identities[0].value, "Alice.Chen");
}

#[tokio::test]
async fn same_person_rebind_is_noop() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = create_person(&pool, "Alice Chen").await.expect("create");
    bind_identity(&pool, person_id, KIND_GIT_EMAIL, "alice@co.com", None)
        .await
        .expect("bind");

    let app = build_app().await.expect("build app");
    let rebind = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_id}/identities"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"kind":"git_email","value":"alice@co.com"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(rebind.status(), StatusCode::NO_CONTENT);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM person_identities WHERE person_id = ? AND kind = ? AND value = ?",
    )
    .bind(person_id)
    .bind(KIND_GIT_EMAIL)
    .bind("alice@co.com")
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 1);
}

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}
