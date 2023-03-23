use chrono::Utc;
use cloud_database::{CloudDatabase, CreateUser, User};
use jwst::{JwstError, JwstResult};
use jwst_logger::info;
use nanoid::nanoid;
use tokio::signal;
use yrs::{Doc, ReadTxn, StateVector, Transact};

pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

pub async fn create_debug_collaboration_workspace(db: &CloudDatabase, name: String) -> Option<()> {
    let user1 = CreateUser {
        name: "debug1".into(),
        email: "debug2@toeverything.info".into(),
        avatar_url: None,
        password: nanoid!(),
    };

    let user2 = CreateUser {
        name: "debug1".into(),
        email: "debug2@toeverything.info".into(),
        avatar_url: None,
        password: nanoid!(),
    };

    let user_model1 = db.create_user(user1.clone()).await.ok()?;
    let user_model2 = db.create_user(user2.clone()).await.ok()?;

    db.create_normal_workspace(user_model1.id.clone())
        .await
        .ok()?;

    let doc = Doc::new();
    let doc_data = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());

    Some(())
}
