use jwst::{Block as JwstBlock, Workspace as JwstWorkspace};
use pyo3::prelude::*;
use pyo3::types::PyList;
use yrs::{Doc as YrsDoc, Map as YrsMap, Subscription as YrsSubscription, UpdateEvent};

#[pyclass(subclass)]
pub struct Doc(YrsDoc);

#[pyclass(subclass, unsendable)]
pub struct Map(YrsMap);

#[pyclass(subclass, unsendable)]
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
    fn id(&self) -> String {
        self.0.id().clone()
    }

    #[getter]
    fn client_id(&self) -> f64 {
        self.0.client_id() as f64
    }

    pub fn create(&self, block_id: String, flavour: String) -> Block {
        Block(self.0.with_trx(|mut t| t.create(block_id, &flavour)))
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        self.0.get(block_id).map(|b| Block(b))
    }

    pub fn block_count(&self) -> u32 {
        self.0.block_count()
    }

    pub fn block_iter(&self) -> Vec<Block> {
        self.0.block_iter().map(|b| Block(b)).collect()
    }

    pub fn exists(&self, block_id: String) -> bool {
        self.0.exists(&block_id)
    }

    pub fn observe(&mut self, callback: PyObject) -> Subscription {
        Subscription(self.0.observe(move |_, u| {
            Python::with_gil(|py| {
                let update = PyList::new(py, u.update.as_slice());
                callback.call1(py, (update,)).unwrap();
            })
        }))
    }
}

#[pyclass(subclass)]
pub struct Block(JwstBlock);

#[pymethods]
impl Block {
    pub fn id(&self) -> String {
        self.0.id().to_string()
    }

    pub fn client_id(&self) -> Vec<String> {
        self.0.children().into_iter().collect::<Vec<String>>()
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn jwst_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Workspace>()?;
    m.add_class::<Block>()?;
    Ok(())
}
