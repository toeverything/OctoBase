use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyList, PyFunction};
use jwst::{Block as JwstBlock, Workspace as JwstWorkspace};
use yrs::{Subscription as YrsSubscription, UpdateEvent};


// #[pyclass(subclass)]
pub struct Subscription(YrsSubscription<UpdateEvent>);

#[pyclass(subclass)]
struct Workspace(JwstWorkspace);

#[pymethods]
impl Workspace {
    #[new]
    fn new(id: String) -> Self {
        Self(JwstWorkspace::new(id))
    }

    #[getter]
    fn client_id(&self) -> f64 {
        self.0.client_id() as f64
    }

    pub fn create(&self, block_id: String, flavor: String) -> Block {
        Block(self.0.with_trx(|mut t| t.create(block_id, &flavor)))
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        self.0.get(block_id).map(|b| Block(b))
    }

    pub fn exists(&self, block_id: String) -> bool {
        self.0.exists(&block_id)
    }
}

#[pyclass(subclass)]
pub struct Block(JwstBlock);

#[pymethods]
impl Block {
    pub fn id(&self) -> String {
        self.0.id().to_string()
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn jwst_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Workspace>()?;
    m.add_class::<Block>()?;
    Ok(())
}