use cloud_database::{CloudDatabase, CreateUser, PermissionType};
use jwst::Workspace;
use jwst_logger::info;
use jwst_storage::JwstStorage;
use tokio::signal;
use yrs::{ReadTxn, StateVector, Transact};

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

pub async fn create_debug_collaboration_workspace(db: &CloudDatabase, storage: &JwstStorage) {
    if db
        .get_user_by_email("debug1@toeverything.info")
        .await
        .unwrap()
        .is_some()
    {
        // already created
        return;
    }

    let user1 = CreateUser {
        name: "debug1".into(),
        email: "debug1@toeverything.info".into(),
        avatar_url: None,
        password: "debug1".into(),
    };

    let user2 = CreateUser {
        name: "debug1".into(),
        email: "debug2@toeverything.info".into(),
        avatar_url: None,
        password: "debug2".into(),
    };

    let user_model1 = db
        .create_user(user1.clone())
        .await
        .expect("failed to create user1");
    let user_model2 = db
        .create_user(user2.clone())
        .await
        .expect("failed to create user2");

    let ws = db
        .create_normal_workspace(user_model1.id.clone())
        .await
        .expect("failed to create workspace");

    let update = Workspace::new(ws.id.clone())
        .doc()
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
        .ok();

    storage.full_migrate(ws.id.clone(), update, true).await;

    let (permission_id, _) = db
        .create_permission(&user_model2.email, ws.id.clone(), PermissionType::Write)
        .await
        .expect("failed to create permission")
        .expect("test workspace not exists");

    db.accept_permission(permission_id)
        .await
        .expect("failed to accept permission");
}
