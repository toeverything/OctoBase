use super::{entities::prelude::*, *};
use crate::types::JwstStorageResult;
use jwst::{sync_encode_update, DocStorage, Workspace};
use jwst_codec::{CrdtReader, RawDecoder};
use sea_orm::Condition;
use std::collections::hash_map::Entry;
use yrs::{Doc, Options, ReadTxn, StateVector, Transact};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

type DocsModel = <Docs as EntityTrait>::Model;
type DocsActiveModel = super::entities::docs::ActiveModel;
type DocsColumn = <Docs as EntityTrait>::Column;

pub struct DocDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    workspaces: RwLock<HashMap<String, Workspace>>, // memory cache
    remote: RwLock<HashMap<String, Sender<Vec<u8>>>>,
}

impl DocDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        Ok(Self {
            bucket,
            pool,
            workspaces: RwLock::new(HashMap::new()),
            remote: RwLock::new(HashMap::new()),
        })
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    pub fn remote(&self) -> &RwLock<HashMap<String, Sender<Vec<u8>>>> {
        &self.remote
    }

    /// warn: records of the same workspace may belong to different doc
    #[allow(unused)]
    async fn workspace_all<C>(conn: &C, workspace: &str) -> JwstStorageResult<Vec<DocsModel>>
    where
        C: ConnectionTrait,
    {
        trace!("start scan all records of workspace: {workspace}");
        let models = Docs::find()
            .filter(DocsColumn::WorkspaceId.eq(workspace))
            .all(conn)
            .await?;
        trace!("end scan all records of workspace: {workspace}, {}", models.len());
        Ok(models)
    }

    async fn doc_all<C>(conn: &C, guid: &str) -> JwstStorageResult<Vec<DocsModel>>
        where
            C: ConnectionTrait,
    {
        trace!("start scan all records with guid: {guid}");
        let models = Docs::find()
            .filter(DocsColumn::Guid.eq(guid))
            .all(conn)
            .await?;
        trace!("end scan all: {guid}, {}", models.len());
        Ok(models)
    }

    async fn count<C>(conn: &C, guid: &str) -> JwstStorageResult<u64>
    where
        C: ConnectionTrait,
    {
        trace!("start count: {guid}");
        let count = Docs::find()
            .filter(DocsColumn::Guid.eq(guid))
            .count(conn)
            .await?;
        trace!("end count: {guid}, {count}");
        Ok(count)
    }

    async fn workspace_count<C>(conn: &C, workspace: &str) -> JwstStorageResult<u64>
    where
        C: ConnectionTrait,
    {
        trace!("start count: {workspace}");
        let count = Docs::find()
            .filter(DocsColumn::WorkspaceId.eq(workspace))
            .count(conn)
            .await?;
        trace!("end count: {workspace}, {count}");
        Ok(count)
    }

    async fn workspace_guid<C>(conn: &C, workspace: &str) -> JwstStorageResult<Option<String>>
    where
        C: ConnectionTrait,
    {
        let record = Docs::find()
            .filter(
                Condition::all()
                    .add(DocsColumn::WorkspaceId.eq(workspace))
                    .add(DocsColumn::IsWorkspace.eq(true)),
            )
            .one(conn)
            .await?;

        Ok(record.map(|r| r.guid))
    }

    async fn is_workspace<C>(conn: &C, guid: &str) -> JwstStorageResult<bool>
    where
        C: ConnectionTrait,
    {
        let record = Docs::find()
            .filter(DocsColumn::Guid.eq(guid))
            .one(conn)
            .await?;

        Ok(record.map_or(false, |r| r.is_workspace))
    }

    async fn insert<C>(conn: &C, workspace: &str, guid: &str, blob: &[u8]) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        let workspace_guid = Self::workspace_guid(conn, workspace).await?;
        trace!("start insert: {workspace}");
        Docs::insert(DocsActiveModel {
            workspace_id: Set(workspace.into()),
            guid: Set(guid.into()),
            blob: Set(blob.into()),
            created_at: Set(Utc::now().into()),
            // if not workspace guid found, the current insertion should be workspace init data.
            is_workspace: Set(workspace_guid.map_or(true, |g| g == guid)),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        trace!("end insert: {workspace}");
        Ok(())
    }

    async fn replace_with<C>(
        conn: &C,
        workspace: &str,
        guid: &str,
        blob: Vec<u8>,
    ) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start replace: {guid}");

        let is_workspace = Self::is_workspace(conn, guid).await?;

        Docs::delete_many()
            .filter(DocsColumn::Guid.eq(guid))
            .exec(conn)
            .await?;
        Docs::insert(DocsActiveModel {
            workspace_id: Set(workspace.into()),
            guid: Set(guid.into()),
            blob: Set(blob),
            is_workspace: Set(is_workspace),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        trace!("end replace: {workspace}");
        Ok(())
    }

    pub async fn delete<C>(conn: &C, guid: &str) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start drop: {guid}");
        Docs::delete_many()
            .filter(DocsColumn::Guid.eq(guid))
            .exec(conn)
            .await?;
        trace!("end drop: {guid}");
        Ok(())
    }

    pub async fn delete_workspace<C>(conn: &C, workspace_id: &str) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start drop workspace: {workspace_id}");
        Docs::delete_many()
            .filter(DocsColumn::WorkspaceId.eq(workspace_id))
            .exec(conn)
            .await?;
        trace!("end drop workspace: {workspace_id}");
        Ok(())
    }

    async fn update<C>(
        &self,
        conn: &C,
        workspace: &str,
        guid: &str,
        blob: Vec<u8>,
    ) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start update: {guid}");
        let update_size = Self::count(conn, guid).await?;
        if update_size > MAX_TRIM_UPDATE_LIMIT - 1 {
            trace!("full migrate update: {guid}, {update_size}");
            let doc_records = Self::doc_all(conn, guid).await?;

            let data = tokio::task::spawn_blocking(move || utils::merge_doc_records(doc_records))
                .await
                .map_err(JwstStorageError::DocMerge)??;

            Self::replace_with(conn, workspace, guid, data).await?;
        }

        trace!("insert update: {guid}, {update_size}");
        Self::insert(conn, workspace, guid, &blob).await?;
        trace!("end update: {guid}");

        trace!("update {}bytes to {}", blob.len(), guid);
        if let Entry::Occupied(remote) = self.remote.write().await.entry(guid.into()) {
            let broadcast = &remote.get();
            if broadcast.send(sync_encode_update(&blob)).is_err() {
                // broadcast failures are not fatal errors, only warnings are required
                warn!("send {guid} update to pipeline failed");
            }
        }
        trace!("end update broadcast: {guid}");

        Ok(())
    }

    async fn full_migrate(
        &self,
        workspace: &str,
        guid: &str,
        blob: Vec<u8>,
    ) -> JwstStorageResult<()> {
        trace!("start full migrate: {guid}");
        Self::replace_with(&self.pool, workspace, guid, blob).await?;
        trace!("end full migrate: {guid}");
        Ok(())
    }

    async fn init_workspace<C>(conn: &C, workspace: &str) -> JwstStorageResult<Workspace>
    where
        C: ConnectionTrait,
    {
        trace!("start create doc in workspace: {workspace}");
        let all_data = Docs::find()
            .filter(
                Condition::all()
                    .add(DocsColumn::WorkspaceId.eq(workspace))
                    .add(DocsColumn::IsWorkspace.eq(true)),
            )
            .all(conn)
            .await?;

        let ws = if all_data.is_empty() {
            // keep workspace root doc's guid the same as workspaceId
            let doc = Doc::with_options(Options {
                guid: yrs::Uuid::from(workspace),
                ..Default::default()
            });
            let ws = Workspace::from_doc(doc.clone(), workspace);

            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default())?;

            Self::insert(conn, workspace, doc.guid(), &update).await?;
            ws
        } else {
            let doc = Doc::with_options(Options {
                guid: all_data.first().unwrap().guid.clone().into(),
                ..Default::default()
            });
            let doc = utils::migrate_update(all_data, doc)?;
            Workspace::from_doc(doc, workspace)
        };

        trace!("end create doc in workspace: {workspace}");

        Ok(ws)
    }
}

#[async_trait]
impl DocStorage<JwstStorageError> for DocDBStorage {
    async fn detect_workspace(&self, workspace_id: &str) -> JwstStorageResult<bool> {
        trace!("check workspace exists: get lock");
        let _lock = self.bucket.read().await;

        Ok(self.workspaces.read().await.contains_key(workspace_id)
            || Self::workspace_count(&self.pool, workspace_id)
                .await
                .map(|c| c > 0)?)
    }

    async fn get_or_create_workspace(&self, workspace_id: String) -> JwstStorageResult<Workspace> {
        trace!("get workspace enter: {workspace_id}");
        match self.workspaces.write().await.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => {
                trace!("get workspace cache: {workspace_id}");
                Ok(ws.get().clone())
            }
            Entry::Vacant(v) => {
                debug!("init workspace cache: get lock");
                let _lock = self.bucket.write().await;
                info!("init workspace cache: {workspace_id}");
                let ws = Self::init_workspace(&self.pool, &workspace_id).await?;

                Ok(v.insert(ws).clone())
            }
        }
    }

    async fn flush_workspace(
        &self,
        workspace_id: String,
        data: Vec<u8>,
    ) -> JwstStorageResult<Workspace> {
        trace!("create workspace: get lock");
        let _lock = self.bucket.write().await;

        let ws = Self::init_workspace(&self.pool, &workspace_id).await?;

        self.full_migrate(&workspace_id, ws.doc_guid(), data)
            .await?;

        debug_assert_eq!(
            Self::workspace_count(&self.pool, &workspace_id).await?,
            1u64
        );

        Ok(ws)
    }

    async fn delete_workspace(&self, workspace_id: &str) -> JwstStorageResult<()> {
        debug!("delete workspace: get lock");
        let _lock = self.bucket.write().await;

        debug!("delete workspace cache: {workspace_id}");
        self.workspaces.write().await.remove(workspace_id);
        DocDBStorage::delete_workspace(&self.pool, workspace_id).await?;

        Ok(())
    }

    async fn detect_doc(&self, guid: &str) -> JwstStorageResult<bool> {
        trace!("check doc exists: get lock");
        let _lock = self.bucket.read().await;

        Ok(Self::count(&self.pool, guid).await? > 0)
    }

    async fn get_doc(&self, guid: String) -> JwstStorageResult<Option<Doc>> {
        let records = Docs::find()
            .filter(DocsColumn::Guid.eq(guid))
            .all(&self.pool)
            .await?;

        if records.is_empty() {
            return Ok(None);
        }

        let doc = utils::migrate_update(records, Doc::new())?;

        Ok(Some(doc))
    }

    async fn update_doc(
        &self,
        workspace_id: String,
        guid: String,
        data: &[u8],
    ) -> JwstStorageResult<()> {
        debug!("write_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_update: {:?}", data);
        self.update(&self.pool, &workspace_id, &guid, data.into())
            .await?;

        Ok(())
    }

    async fn update_doc_with_guid(
        &self,
        workspace_id: String,
        data: &[u8],
    ) -> JwstStorageResult<()> {
        debug!("write_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_update: {:?}", data);
        let mut decoder = RawDecoder::new(data.to_vec());
        let guid = decoder.read_var_string()?;

        self.update(&self.pool, &workspace_id, &guid, decoder.drain())
            .await?;

        Ok(())
    }

    async fn delete_doc(&self, guid: &str) -> JwstStorageResult<()> {
        trace!("start drop doc: {guid}");

        Docs::delete_many()
            .filter(DocsColumn::Guid.eq(guid))
            .exec(&self.pool)
            .await?;

        trace!("end drop doc: {guid}");
        Ok(())
    }
}

#[cfg(test)]
pub async fn docs_storage_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    let conn = &pool.pool;

    DocDBStorage::delete_workspace(conn, "basic").await?;

    // empty table
    assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 0);

    // first insert
    DocDBStorage::insert(conn, "basic", "1", &[1, 2, 3, 4]).await?;
    DocDBStorage::insert(conn, "basic", "1", &[2, 2, 3, 4]).await?;
    assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 2);

    // second insert
    DocDBStorage::replace_with(conn, "basic", "1", vec![3, 2, 3, 4]).await?;

    let all = DocDBStorage::workspace_all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            guid: all.get(0).unwrap().guid.clone(),
            workspace_id: "basic".into(),
            created_at: all.get(0).unwrap().created_at,
            is_workspace: true,
            blob: vec![3, 2, 3, 4]
        }]
    );
    assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 1);

    DocDBStorage::delete_workspace(conn, "basic").await?;

    DocDBStorage::insert(conn, "basic", "1", &[1, 2, 3, 4]).await?;

    let all = DocDBStorage::workspace_all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            workspace_id: "basic".into(),
            guid: all.get(0).unwrap().guid.clone(),
            created_at: all.get(0).unwrap().created_at,
            is_workspace: true,
            blob: vec![1, 2, 3, 4]
        }]
    );
    assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 1);

    // no cache
    {
        pool.workspaces.write().await.clear();
        let workspace = DocDBStorage::init_workspace(conn, "basic").await?;
        assert_eq!(workspace.doc_guid(), "1");
    }

    Ok(())
}

#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn full_migration_stress_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    let final_bytes: Vec<u8> = (0..1024 * 100).map(|_| rand::random::<u8>()).collect();
    for i in 0..=50 {
        let random_bytes: Vec<u8> = if i == 50 {
            final_bytes.clone()
        } else {
            (0..1024 * 100).map(|_| rand::random::<u8>()).collect()
        };
        let (r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15) = tokio::join!(
            pool.flush_workspace("full_migration_1".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_2".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_3".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_4".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_5".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_6".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_7".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_8".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_9".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_10".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_11".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_12".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_13".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_14".to_owned(), random_bytes.clone()),
            pool.flush_workspace("full_migration_15".to_owned(), random_bytes.clone())
        );
        r1?;
        r2?;
        r3?;
        r4?;
        r5?;
        r6?;
        r7?;
        r8?;
        r9?;
        r10?;
        r11?;
        r12?;
        r13?;
        r14?;
        r15?;
    }

    assert_eq!(
        DocDBStorage::workspace_all(&pool.pool, "full_migration_1")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes.clone()]
    );

    assert_eq!(
        DocDBStorage::workspace_all(&pool.pool, "full_migration_2")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes]
    );

    Ok(())
}

#[cfg(test)]
pub async fn docs_storage_partial_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    use tokio::sync::mpsc::channel;

    let conn = &pool.pool;

    DocDBStorage::delete_workspace(conn, "basic").await?;

    // empty table
    assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 0);

    {
        let ws = pool.get_or_create_workspace("basic".into()).await.unwrap();
        let guid = ws.doc_guid().to_string();
        let (tx, mut rx) = channel(100);

        let sub = ws
            .doc()
            .observe_update_v1(move |_, e| {
                futures::executor::block_on(async {
                    tx.send(e.update.clone()).await.unwrap();
                });
            })
            .unwrap();

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.create(&mut t.trx, "block1", "text").unwrap();
            block.set(&mut t.trx, "test1", "value1").unwrap();
        });

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.get(&mut t.trx, "block1").unwrap();
            block.set(&mut t.trx, "test2", "value2").unwrap();
        });

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.create(&mut t.trx, "block2", "block2").unwrap();
            block.set(&mut t.trx, "test3", "value3").unwrap();
        });

        drop(sub);

        while let Some(update) = rx.recv().await {
            info!("recv: {}", update.len());
            pool.update_doc("basic".into(), guid.clone(), &update)
                .await
                .unwrap();
        }

        assert_eq!(DocDBStorage::workspace_count(conn, "basic").await?, 4);
    }

    // clear memory cache
    pool.workspaces.write().await.clear();

    {
        // memory cache empty, retrieve data from db
        let ws = pool.get_or_create_workspace("basic".into()).await.unwrap();
        ws.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.get(&mut t.trx, "block1").unwrap();
            assert_eq!(block.flavour(&t.trx), "text");
            assert_eq!(block.get(&t.trx, "test1"), Some("value1".into()));
            assert_eq!(block.get(&t.trx, "test2"), Some("value2".into()));

            let block = space.get(&mut t.trx, "block2").unwrap();
            assert_eq!(block.flavour(&t.trx), "block2");
            assert_eq!(block.get(&t.trx, "test3"), Some("value3".into()));
        });
    }

    Ok(())
}
