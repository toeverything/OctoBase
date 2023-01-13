//! Linkable shared documentation of different OctoBase concepts.
//!
//! Progress 1/10:
//!
//!  * I like that we can have a single place to define shared concepts
//!  * This might be bad if people searching the `cargo doc` find these
//!    pages and fail to find the actual source code of JWST.
//!  * Consider moving concepts to their own `octo_concept`s crate to avoid that issue.

/// When you have a reference to a YJS block, you are usually going to be
/// interacting with a set of [SysAttr]s or [UserPropAttr]s.
/// ```json
/// { "sys:flavor": "affine:text",
///   "sys:created": 946684800000,
///   "sys:children": ["block1", "block2"],
///   "prop:text": "123",
///   "prop:color": "#ff0000" }
/// ```
///
/// Also see [YJSBlockHistory].
pub struct YJSBlockAttrMap;

/// The built-in properties OctoBase manages on the [YJSBlockAttrMap]. For example: `"sys:flavor"` ([SysFlavorAttr]), `"sys:created"`, `"sys:children"`.
///
/// ```json
/// "sys:flavor": "affine:text",
/// "sys:created": 946684800000,
/// "sys:children": ["block1", "block2"],
/// ```
pub struct SysAttr;

/// "Flavors" are specified as a [SysAttr] named `"sys:flavor"` of our [YJSBlockAttrMap].
///
/// Flavor is the type of `Block`, which is derived from [particle physics],
/// which means that all blocks have the same basic properties, when the flavor of
/// `Block` changing, the basic attributes will not change, but the interpretation
/// of user-defined attributes will be different.
///
/// [particle physics]: https://en.wikipedia.org/wiki/Flavour_(particle_physics)
pub struct SysFlavorAttr;

/// User defined attributes on the [YJSBlockAttrMap]. For example: `"prop:xywh"`, `"prop:text"`, etc.
///
/// ### Common props
///
/// These props are understood by the workspace search plugin, for example.
///
/// ```json
/// "prop:text": "123",
/// "prop:title": "123"
/// ```
///
/// ## AFFiNE examples
///
/// Some examples for different AFFiNE [SysFlavorAttr]s are as follows:
///
/// ### `"sys:flavor": "affine:shape"`
/// ```json
/// "prop:xywh": "[0,0,720,480]",
/// "prop:type": "rectangle",
/// "prop:color": "black",
/// ```
///
/// ### `"sys:flavor": "affine:list"`
/// ```json
///  "prop:checked": false,
///  "prop:text": "List item text",
///  "prop:type": "bulleted",
/// ```
pub struct UserPropAttr;

/// Each time an edit or update happens to a Block, an event is inserted into the
/// YJS Array this points to.
///
/// The values in this array each look like an array of three items: `[operator, utc_timestamp_ms, "action"]`
/// ```json
/// [ [35, 1672374542865, "add"],
///   [35, 1672374543214, "update"],
///   [35, 1672378928393, "update"] ]
/// ```
///
/// Also see [YJSBlockAttrMap] and [crate::HistoryOperation].
pub struct YJSBlockHistory;
