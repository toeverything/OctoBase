#![deny(clippy::all)]

use jwst::{BlobStorage, JwstError, Workspace as JwstWorkspace};
use jwst_storage::{JwstStorage, JwstStorageResult};
use yrs::Doc as YDoc;

use napi::{bindgen_prelude::Buffer, Error, Result, Status};

#[macro_use]
extern crate napi_derive;

fn map_err_inner<T, E: ToString>(v: std::result::Result<T, E>, status: Status) -> Result<T> {
    match v {
        Ok(val) => Ok(val),
        Err(e) => Err(Error::new(status, e)),
    }
}

macro_rules! map_err {
    ($val: expr) => {
        map_err_inner($val, Status::GenericFailure)
    };
    ($val: expr, $stauts: ident) => {
        map_err_inner($val, $stauts)
    };
}

#[napi]
pub struct Storage {
    inner: JwstStorage,
}

#[napi]
pub struct Workspace {
    inner: JwstWorkspace,
}

#[napi]
pub struct Doc {
    inner: YDoc,
}

#[napi]
impl Storage {
    #[napi]
    pub async fn connect(database: String) -> Result<Storage> {
        let inner = map_err!(JwstStorage::new(&database).await)?;

        Ok(Storage { inner })
    }

    #[napi]
    pub async fn get_or_create_workspace(&self, workspace_id: String) -> Result<Workspace> {
        let inner = map_err!(self.inner.create_workspace(workspace_id).await)?;

        Ok(Workspace { inner })
    }

    #[napi]
    pub async fn delete_workspace(&self, _workspace_id: String) -> Result<()> {
        todo!()
    }

    #[napi]
    pub async fn blob(&self, _workspace_id: String, _name: String) -> Result<Vec<u8>> {
        todo!()
    }

    #[napi]
    pub async fn upload_blob(&self, _workspace_id: String, _blob: Buffer) -> Result<String> {
        todo!()
    }
}

#[napi]
impl Workspace {
    #[napi(getter)]
    pub fn doc(&self) -> Doc {
        Doc {
            inner: self.inner.doc(),
        }
    }

    #[napi]
    pub fn to_update_v1(&self) -> Result<Buffer> {
        map_err!(self.inner.sync_migration()).map(|update| update.into())
    }

    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.id()
    }

    #[napi(getter)]
    pub fn client_id(&self) -> u64 {
        self.inner.client_id()
    }

    #[napi]
    pub fn subdoc(&self, _name: String) -> Result<Doc> {
        todo!()
    }

    #[napi]
    pub fn search(&self, _query: String) -> Result<()> {
        todo!()
    }
}

#[napi]
pub fn sum(a: i32, b: i32) -> i32 {
    a + b
}
