use super::*;
use lib0::any::Any;
use serde::{Serialize, Serializer};
use types::{BlockContentValue, JsonValue};
use yrs::{Array, Map, PrelimArray, PrelimMap, Transaction};

struct BlockChildrenPosition {
    pos: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

impl From<&InsertChildren> for RemoveChildren {
    fn from(options: &InsertChildren) -> RemoveChildren {
        Self {
            block_id: options.block_id.clone(),
        }
    }
}

impl From<&InsertChildren> for BlockChildrenPosition {
    fn from(options: &InsertChildren) -> BlockChildrenPosition {
        Self {
            pos: options.pos,
            before: options.before.clone(),
            after: options.after.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Block {
    // block schema
    // for example: {
    //     title: {type: 'string', default: ''}
    //     description: {type: 'string', default: ''}
    // }
    // default_props: HashMap<String, BlockField>,
    id: String,
    operator: i64,
    block: Map,
    children: Array,
    content: Map,
    updated: Array,
}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F, O>(
        workspace: &Workspace,
        trx: &mut Transaction,
        block_id: B,
        flavor: F,
        operator: O,
    ) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
        O: TryInto<i64>,
    {
        let block_id = block_id.as_ref();
        let operator = operator.try_into().unwrap_or_default();
        if let Some(block) = Self::from(workspace, &block_id, operator) {
            block
        } else {
            // init base struct
            workspace
                .blocks()
                .insert(trx, block_id.clone(), PrelimMap::<Any>::new());
            let block = workspace
                .blocks()
                .get(&block_id)
                .and_then(|b| b.to_ymap())
                .unwrap();

            // init default schema
            block.insert(trx, "sys:flavor", flavor.as_ref());
            block.insert(trx, "sys:version", PrelimArray::from([1, 0]));
            block.insert(
                trx,
                "sys:children",
                PrelimArray::<Vec<String>, String>::from(vec![]),
            );
            block.insert(
                trx,
                "sys:created",
                chrono::Utc::now().timestamp_millis() as f64,
            );
            block.insert(trx, "content", PrelimMap::<Any>::new());

            workspace
                .updated()
                .insert(trx, block_id.clone(), PrelimArray::<_, Any>::from([]));

            trx.commit();

            let children = block
                .get("sys:children")
                .and_then(|c| c.to_yarray())
                .unwrap();
            let content = block.get("content").and_then(|c| c.to_ymap()).unwrap();
            let updated = workspace
                .updated()
                .get(&block_id)
                .and_then(|c| c.to_yarray())
                .unwrap();

            let block = Self {
                id: block_id.to_string(),
                operator,
                block,
                children,
                content,
                updated,
            };

            block.log_update(trx, HistoryOperation::Add);

            block
        }
    }

    pub fn from<B, O>(workspace: &Workspace, block_id: B, operator: O) -> Option<Block>
    where
        B: AsRef<str>,
        O: TryInto<i64>,
    {
        workspace
            .blocks()
            .get(block_id.as_ref())
            .and_then(|b| b.to_ymap())
            .and_then(|block| {
                workspace
                    .updated()
                    .get(block_id.as_ref())
                    .and_then(|u| u.to_yarray())
                    .map(|updated| (block, updated))
            })
            .and_then(|(block, updated)| {
                if let (Some(children), Some(content)) = (
                    block.get("sys:children").and_then(|c| c.to_yarray()),
                    block.get("content").and_then(|c| c.to_ymap()),
                ) {
                    Some(Self {
                        id: block_id.as_ref().to_string(),
                        operator: operator.try_into().unwrap_or_default(),
                        block,
                        children,
                        content,
                        updated,
                    })
                } else {
                    None
                }
            })
    }

    pub fn block(&self) -> &Map {
        &self.block
    }

    pub(crate) fn content(&mut self) -> &mut Map {
        &mut self.content
    }

    pub(crate) fn log_update(&self, trx: &mut Transaction, action: HistoryOperation) {
        let array = PrelimArray::from([
            Any::Number(self.operator as f64),
            Any::Number(chrono::Utc::now().timestamp_millis() as f64),
            Any::String(Box::from(action.to_string())),
        ]);

        self.updated.push_back(trx, array);
    }

    pub(self) fn set_value(
        block: &mut Map,
        trx: &mut Transaction,
        key: &str,
        value: JsonValue,
    ) -> bool {
        match value {
            JsonValue::Bool(v) => {
                block.insert(trx, key.clone(), v);
                true
            }
            JsonValue::Null => {
                block.remove(trx, key);
                true
            }
            JsonValue::Number(v) => {
                if let Some(v) = v.as_f64() {
                    block.insert(trx, key.clone(), v);
                    true
                } else if let Some(v) = v.as_i64() {
                    block.insert(trx, key.clone(), v);
                    true
                } else if let Some(v) = v.as_u64() {
                    block.insert(trx, key.clone(), i64::try_from(v).unwrap_or(0));
                    true
                } else {
                    false
                }
            }
            JsonValue::String(v) => {
                block.insert(trx, key.clone(), v.clone());
                true
            }
            _ => false,
        }
    }

    pub fn set<T>(&mut self, trx: &mut Transaction, key: &str, value: T)
    where
        T: Into<BlockContentValue>,
    {
        match value.into() {
            BlockContentValue::Json(json) => {
                if Self::set_value(self.content(), trx, key, json) {
                    self.log_update(trx, HistoryOperation::Update);
                }
            }
            BlockContentValue::Boolean(bool) => {
                self.content.insert(trx, key, bool);
                self.log_update(trx, HistoryOperation::Update);
            }
            BlockContentValue::Text(text) => {
                self.content.insert(trx, key, text);
                self.log_update(trx, HistoryOperation::Update);
            }
            BlockContentValue::Number(number) => {
                self.content.insert(trx, key, number);
                self.log_update(trx, HistoryOperation::Update);
            }
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    // start with a namespace
    // for example: affine:text
    pub fn flavor(&self) -> String {
        self.block.get("sys:flavor").unwrap_or_default().to_string()
    }

    // block schema version
    // for example: [1, 0]
    pub fn version(&self) -> [usize; 2] {
        self.block
            .get("sys:version")
            .and_then(|v| v.to_yarray())
            .map(|v| {
                v.iter()
                    .take(2)
                    .filter_map(|s| s.to_string().parse::<usize>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap()
            .try_into()
            .unwrap()
    }

    pub fn created(&self) -> i64 {
        self.block
            .get("sys:created")
            .and_then(|c| match c.to_json() {
                Any::Number(n) => Some(n as i64),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn updated(&self) -> i64 {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .last()
            .and_then(|a| {
                a.get(1).and_then(|i| match i.to_json() {
                    Any::Number(n) => Some(n as i64),
                    _ => None,
                })
            })
            .unwrap_or_else(|| self.created())
    }

    pub fn history(&self) -> Vec<BlockHistory> {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .map(|v| (v, self.id.clone()).into())
            .collect()
    }

    pub fn insert_children(&mut self, trx: &mut Transaction, options: InsertChildren) {
        self.remove_children(trx, (&options).into());

        let children = &self.children;

        if let Some(position) = self.position_calculator(children.len(), (&options).into()) {
            children.insert(trx, position, options.block_id);
        } else {
            children.push_back(trx, options.block_id);
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn remove_children(&mut self, trx: &mut Transaction, options: RemoveChildren) {
        let children = &self.children;

        if let Some(current_pos) = children
            .iter()
            .position(|c| c.to_string() == options.block_id)
        {
            children.remove(trx, current_pos as u32);
            self.log_update(trx, HistoryOperation::Delete);
        }
    }

    pub fn exists_children(&self, options: ExistsChildren) -> Option<usize> {
        self.children
            .iter()
            .position(|c| c.to_string() == options.block_id)
    }

    fn position_calculator(&self, max_pos: u32, position: BlockChildrenPosition) -> Option<u32> {
        let BlockChildrenPosition { pos, before, after } = position;
        let children = &self.children;
        if let Some(pos) = pos {
            if pos < max_pos {
                return Some(pos);
            }
        } else if let Some(before) = before {
            if let Some(current_pos) = children.iter().position(|c| c.to_string() == before) {
                let prev_pos = current_pos as u32;
                if prev_pos < max_pos {
                    return Some(prev_pos);
                }
            }
        } else if let Some(after) = after {
            if let Some(current_pos) = children.iter().position(|c| c.to_string() == after) {
                let next_pos = current_pos as u32 + 1;
                if next_pos < max_pos {
                    return Some(next_pos);
                }
            }
        }
        None
    }
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let any = self.block.to_json();
        any.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::Workspace;

    #[test]
    fn init_block() {
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "test");

        // new block
        let block = workspace.create(&mut trx, "test", "affine:text", 123);
        assert_eq!(block.id(), "test");
        assert_eq!(block.flavor(), "affine:text");
        assert_eq!(block.version(), [1, 0]);

        // get exist block
        let block = workspace.get("test", 123).unwrap();
        assert_eq!(block.flavor(), "affine:text");
        assert_eq!(block.version(), [1, 0]);
    }

    #[test]
    fn set_value() {
        use super::Any;
        use serde_json::{Number, Value};
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "test");

        let mut block = workspace.create(&mut trx, "test", "affine:text", doc.client_id);

        // normal type set
        block.set(&mut trx, "bool", true);
        block.set(&mut trx, "text", "hello world");
        block.set(&mut trx, "text_owned", "hello world".to_owned());
        block.set(&mut trx, "num", 123);
        assert_eq!(block.content().get("bool").unwrap().to_string(), "true");
        assert_eq!(
            block.content().get("text").unwrap().to_string(),
            "hello world"
        );
        assert_eq!(
            block.content().get("text_owned").unwrap().to_string(),
            "hello world"
        );
        assert_eq!(block.content().get("num").unwrap().to_string(), "123");

        // json type set
        block.set(&mut trx, "json_bool", Value::Bool(false));
        block.set(
            &mut trx,
            "json_f64",
            Value::Number(Number::from_f64(1.23).unwrap()),
        );
        block.set(&mut trx, "json_i64", Value::Number(i64::MAX.into()));
        block.set(&mut trx, "json_u64", Value::Number(u64::MAX.into()));
        block.set(&mut trx, "json_str", Value::String("test".into()));
        assert_eq!(
            block.content().get("json_bool").unwrap().to_json(),
            Any::Bool(false)
        );
        assert_eq!(
            block.content().get("json_f64").unwrap().to_json(),
            Any::Number(1.23)
        );
        assert_eq!(
            block.content().get("json_i64").unwrap().to_json(),
            Any::Number(i64::MAX as f64)
        );
        assert_eq!(
            block.content().get("json_u64").unwrap().to_json(),
            Any::Number(u64::MAX as f64)
        );
        assert_eq!(
            block.content().get("json_str").unwrap().to_json(),
            Any::String("test".into())
        );
    }

    #[test]
    fn insert_remove_children() {
        use super::{InsertChildren, RemoveChildren};
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "text");

        let mut block = workspace.create(&mut trx, "a", "affine:text", 123);

        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "b".to_owned(),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "c".to_owned(),
                pos: Some(0),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "d".to_owned(),
                before: Some("b".to_owned()),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "e".to_owned(),
                after: Some("b".to_owned()),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "f".to_owned(),
                after: Some("c".to_owned()),
                ..Default::default()
            },
        );

        assert_eq!(
            block
                .children
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>(),
            vec![
                "c".to_owned(),
                "f".to_owned(),
                "d".to_owned(),
                "b".to_owned(),
                "e".to_owned()
            ]
        );

        block.remove_children(
            &mut trx,
            RemoveChildren {
                block_id: "d".to_owned(),
            },
        );

        assert_eq!(
            block
                .children
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>(),
            vec![
                "c".to_owned(),
                "f".to_owned(),
                "b".to_owned(),
                "e".to_owned()
            ]
        );
    }

    #[test]
    fn updated() {
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "test");
        let mut block = workspace.create(&mut trx, "a", "affine:text", 123);

        block.set(&mut trx, "test", 1);

        assert!(block.created() <= block.updated())
    }

    #[test]
    fn history() {
        use super::{
            BlockHistory, ExistsChildren, HistoryOperation, InsertChildren, RemoveChildren,
        };
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "test");

        let mut block = workspace.create(&mut trx, "a", "affine:text", 123);

        block.set(&mut trx, "test", 1);

        let history = block.history();

        assert_eq!(history.len(), 2);

        // let history = history.last().unwrap();

        assert_eq!(
            history,
            vec![
                BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: history.get(0).unwrap().timestamp,
                    operation: HistoryOperation::Add,
                },
                BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: history.get(1).unwrap().timestamp,
                    operation: HistoryOperation::Update,
                }
            ]
        );

        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "b".to_owned(),
                ..Default::default()
            },
        );

        assert_eq!(
            block.exists_children(ExistsChildren {
                block_id: "b".to_owned(),
            }),
            Some(0)
        );

        block.remove_children(
            &mut trx,
            RemoveChildren {
                block_id: "b".to_owned(),
            },
        );

        assert_eq!(
            block.exists_children(ExistsChildren {
                block_id: "b".to_owned(),
            }),
            None
        );

        let history = block.history();
        assert_eq!(history.len(), 4);

        if let [.., insert, remove] = history.as_slice() {
            assert_eq!(
                insert,
                &BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: insert.timestamp,
                    operation: HistoryOperation::Add,
                }
            );
            assert_eq!(
                remove,
                &BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: remove.timestamp,
                    operation: HistoryOperation::Delete,
                }
            );
        } else {
            assert!(false)
        }
    }
}
