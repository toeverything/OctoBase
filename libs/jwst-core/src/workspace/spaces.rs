use super::*;
use jwst_codec::Any;

const RESERVE_SPACE: [&str; 2] = [constants::space::META, constants::space::UPDATED];

impl Workspace {
    pub fn get_space<S: AsRef<str>>(&mut self, space_id: S) -> JwstResult<Space> {
        let pages = self.pages()?;
        Space::new(self.doc, Pages::new(pages), self.id(), space_id)
    }

    pub fn get_exists_space<S: AsRef<str>>(&self, space_id: S) -> Option<Space> {
        Space::from_exists(self.doc, self.id(), space_id)
    }

    /// The compatibility interface for keck/jni/swift, this api was outdated.
    pub fn get_blocks(&mut self) -> JwstResult<Space> {
        self.get_space("blocks")
    }

    #[inline]
    pub fn spaces<R>(&self, cb: impl FnOnce(Box<dyn Iterator<Item = Space> + '_>) -> R) -> R {
        let keys = self.doc.keys();
        let iterator = keys.iter().filter_map(|key| {
            if key.starts_with("space:") && !RESERVE_SPACE.contains(&key.as_str()) {
                Space::from_exists(self.doc, self.id(), &key[6..])
            } else {
                None
            }
        });

        cb(Box::new(iterator))
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) -> JwstResult<()> {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Null | Any::Undefined => {
                self.metadata.remove(&key);
            }
            value => {
                self.metadata.insert(key, value);
            }
        }

        Ok(())
    }
}
