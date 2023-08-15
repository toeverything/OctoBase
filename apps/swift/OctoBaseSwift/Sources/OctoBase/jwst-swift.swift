import RustXcframework

public class Block: BlockRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$Block$_free(ptr)
        }
    }
}
public class BlockRefMut: BlockRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class BlockRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension BlockRef {
    public func get<GenericIntoRustString: IntoRustString>(_ block_id: GenericIntoRustString) -> Optional<DynamicValue> {
        { let val = __swift_bridge__$Block$get(ptr, { let rustString = block_id.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val != nil { return DynamicValue(ptr: val!) } else { return nil } }()
    }

    public func children() -> RustVec<RustString> {
        RustVec(ptr: __swift_bridge__$Block$children(ptr))
    }

    public func push_children(_ block: BlockRef) {
        __swift_bridge__$Block$push_children(ptr, block.ptr)
    }

    public func insert_children_at(_ block: BlockRef, _ pos: UInt32) {
        __swift_bridge__$Block$insert_children_at(ptr, block.ptr, pos)
    }

    public func insert_children_before<GenericToRustStr: ToRustStr>(_ block: BlockRef, _ reference: GenericToRustStr) {
        reference.toRustStr({ referenceAsRustStr in
            __swift_bridge__$Block$insert_children_before(ptr, block.ptr, referenceAsRustStr)
        })
    }

    public func insert_children_after<GenericToRustStr: ToRustStr>(_ block: BlockRef, _ reference: GenericToRustStr) {
        reference.toRustStr({ referenceAsRustStr in
            __swift_bridge__$Block$insert_children_after(ptr, block.ptr, referenceAsRustStr)
        })
    }

    public func remove_children(_ block: BlockRef) {
        __swift_bridge__$Block$remove_children(ptr, block.ptr)
    }

    public func exists_children<GenericToRustStr: ToRustStr>(_ block_id: GenericToRustStr) -> Int32 {
        return block_id.toRustStr({ block_idAsRustStr in
            __swift_bridge__$Block$exists_children(ptr, block_idAsRustStr)
        })
    }

    public func parent() -> RustString {
        RustString(ptr: __swift_bridge__$Block$parent(ptr))
    }

    public func updated() -> UInt64 {
        __swift_bridge__$Block$updated(ptr)
    }

    public func id() -> RustString {
        RustString(ptr: __swift_bridge__$Block$id(ptr))
    }

    public func flavour() -> RustString {
        RustString(ptr: __swift_bridge__$Block$flavour(ptr))
    }

    public func created() -> UInt64 {
        __swift_bridge__$Block$created(ptr)
    }

    public func set_bool<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString, _ value: Bool) {
        __swift_bridge__$Block$set_bool(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), value)
    }

    public func set_string<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString, _ value: GenericIntoRustString) {
        __swift_bridge__$Block$set_string(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = value.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func set_float<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString, _ value: Double) {
        __swift_bridge__$Block$set_float(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), value)
    }

    public func set_integer<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString, _ value: Int64) {
        __swift_bridge__$Block$set_integer(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), value)
    }

    public func set_null<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) {
        __swift_bridge__$Block$set_null(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func is_bool<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Bool {
        __swift_bridge__$Block$is_bool(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func is_string<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Bool {
        __swift_bridge__$Block$is_string(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func is_float<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Bool {
        __swift_bridge__$Block$is_float(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func is_integer<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Bool {
        __swift_bridge__$Block$is_integer(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
    }

    public func get_bool<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Optional<Int64> {
        { let val = __swift_bridge__$Block$get_bool(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val.is_some { return val.val } else { return nil } }()
    }

    public func get_string<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Optional<RustString> {
        { let val = __swift_bridge__$Block$get_string(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    }

    public func get_float<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Optional<Double> {
        { let val = __swift_bridge__$Block$get_float(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val.is_some { return val.val } else { return nil } }()
    }

    public func get_integer<GenericIntoRustString: IntoRustString>(_ key: GenericIntoRustString) -> Optional<Int64> {
        { let val = __swift_bridge__$Block$get_integer(ptr, { let rustString = key.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val.is_some { return val.val } else { return nil } }()
    }
}
extension Block: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_Block$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_Block$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Block) {
        __swift_bridge__$Vec_Block$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_Block$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (Block(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<BlockRef> {
        let pointer = __swift_bridge__$Vec_Block$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return BlockRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<BlockRefMut> {
        let pointer = __swift_bridge__$Vec_Block$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return BlockRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_Block$len(vecPtr)
    }
}


public class DynamicValueMap: DynamicValueMapRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$DynamicValueMap$_free(ptr)
        }
    }
}
public class DynamicValueMapRefMut: DynamicValueMapRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class DynamicValueMapRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension DynamicValueMap: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_DynamicValueMap$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_DynamicValueMap$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: DynamicValueMap) {
        __swift_bridge__$Vec_DynamicValueMap$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_DynamicValueMap$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (DynamicValueMap(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DynamicValueMapRef> {
        let pointer = __swift_bridge__$Vec_DynamicValueMap$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DynamicValueMapRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DynamicValueMapRefMut> {
        let pointer = __swift_bridge__$Vec_DynamicValueMap$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DynamicValueMapRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_DynamicValueMap$len(vecPtr)
    }
}


public class DynamicValue: DynamicValueRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$DynamicValue$_free(ptr)
        }
    }
}
public class DynamicValueRefMut: DynamicValueRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class DynamicValueRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension DynamicValueRef {
    public func as_bool() -> Optional<Bool> {
        { let val = __swift_bridge__$DynamicValue$as_bool(ptr); if val.is_some { return val.val } else { return nil } }()
    }

    public func as_number() -> Optional<Double> {
        { let val = __swift_bridge__$DynamicValue$as_number(ptr); if val.is_some { return val.val } else { return nil } }()
    }

    public func as_int() -> Optional<Int64> {
        { let val = __swift_bridge__$DynamicValue$as_int(ptr); if val.is_some { return val.val } else { return nil } }()
    }

    public func as_string() -> Optional<RustString> {
        { let val = __swift_bridge__$DynamicValue$as_string(ptr); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    }

    public func as_map() -> Optional<DynamicValueMap> {
        { let val = __swift_bridge__$DynamicValue$as_map(ptr); if val != nil { return DynamicValueMap(ptr: val!) } else { return nil } }()
    }

    public func as_array() -> Optional<RustVec<DynamicValue>> {
        { let val = __swift_bridge__$DynamicValue$as_array(ptr); if val != nil { return RustVec(ptr: val!) } else { return nil } }()
    }

    public func as_buffer() -> Optional<RustVec<UInt8>> {
        { let val = __swift_bridge__$DynamicValue$as_buffer(ptr); if val != nil { return RustVec(ptr: val!) } else { return nil } }()
    }
}
extension DynamicValue: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_DynamicValue$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_DynamicValue$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: DynamicValue) {
        __swift_bridge__$Vec_DynamicValue$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_DynamicValue$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (DynamicValue(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DynamicValueRef> {
        let pointer = __swift_bridge__$Vec_DynamicValue$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DynamicValueRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DynamicValueRefMut> {
        let pointer = __swift_bridge__$Vec_DynamicValue$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DynamicValueRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_DynamicValue$len(vecPtr)
    }
}


public class Workspace: WorkspaceRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$Workspace$_free(ptr)
        }
    }
}
public class WorkspaceRefMut: WorkspaceRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
extension WorkspaceRefMut {
    public func compare() -> Optional<RustString> {
        { let val = __swift_bridge__$Workspace$compare(ptr); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    }
}
public class WorkspaceRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension WorkspaceRef {
    public func id() -> RustString {
        RustString(ptr: __swift_bridge__$Workspace$id(ptr))
    }

    public func client_id() -> UInt64 {
        __swift_bridge__$Workspace$client_id(ptr)
    }

    public func get<GenericIntoRustString: IntoRustString>(_ block_id: GenericIntoRustString) -> Optional<Block> {
        { let val = __swift_bridge__$Workspace$get(ptr, { let rustString = block_id.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val != nil { return Block(ptr: val!) } else { return nil } }()
    }

    public func create<GenericIntoRustString: IntoRustString>(_ block_id: GenericIntoRustString, _ flavour: GenericIntoRustString) -> Block {
        Block(ptr: __swift_bridge__$Workspace$create(ptr, { let rustString = block_id.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = flavour.intoRustString(); rustString.isOwned = false; return rustString.ptr }()))
    }

    public func search<GenericIntoRustString: IntoRustString>(_ query: GenericIntoRustString) -> RustString {
        RustString(ptr: __swift_bridge__$Workspace$search(ptr, { let rustString = query.intoRustString(); rustString.isOwned = false; return rustString.ptr }()))
    }

    public func get_blocks_by_flavour<GenericToRustStr: ToRustStr>(_ flavour: GenericToRustStr) -> RustVec<Block> {
        return flavour.toRustStr({ flavourAsRustStr in
            RustVec(ptr: __swift_bridge__$Workspace$get_blocks_by_flavour(ptr, flavourAsRustStr))
        })
    }

    public func get_search_index() -> RustVec<RustString> {
        RustVec(ptr: __swift_bridge__$Workspace$get_search_index(ptr))
    }

    public func set_search_index<GenericIntoRustString: IntoRustString>(_ fields: RustVec<GenericIntoRustString>) -> Bool {
        __swift_bridge__$Workspace$set_search_index(ptr, { let val = fields; val.isOwned = false; return val.ptr }())
    }
}
extension Workspace: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_Workspace$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_Workspace$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Workspace) {
        __swift_bridge__$Vec_Workspace$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_Workspace$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (Workspace(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<WorkspaceRef> {
        let pointer = __swift_bridge__$Vec_Workspace$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return WorkspaceRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<WorkspaceRefMut> {
        let pointer = __swift_bridge__$Vec_Workspace$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return WorkspaceRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_Workspace$len(vecPtr)
    }
}


public class JwstWorkSpaceResult: JwstWorkSpaceResultRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$JwstWorkSpaceResult$_free(ptr)
        }
    }
}
public class JwstWorkSpaceResultRefMut: JwstWorkSpaceResultRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class JwstWorkSpaceResultRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension JwstWorkSpaceResult: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_JwstWorkSpaceResult$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_JwstWorkSpaceResult$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: JwstWorkSpaceResult) {
        __swift_bridge__$Vec_JwstWorkSpaceResult$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_JwstWorkSpaceResult$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (JwstWorkSpaceResult(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<JwstWorkSpaceResultRef> {
        let pointer = __swift_bridge__$Vec_JwstWorkSpaceResult$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return JwstWorkSpaceResultRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<JwstWorkSpaceResultRefMut> {
        let pointer = __swift_bridge__$Vec_JwstWorkSpaceResult$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return JwstWorkSpaceResultRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_JwstWorkSpaceResult$len(vecPtr)
    }
}


public class Storage: StorageRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$Storage$_free(ptr)
        }
    }
}
extension Storage {
    public convenience init<GenericIntoRustString: IntoRustString>(_ path: GenericIntoRustString) {
        self.init(ptr: __swift_bridge__$Storage$new({ let rustString = path.intoRustString(); rustString.isOwned = false; return rustString.ptr }()))
    }

    public convenience init<GenericIntoRustString: IntoRustString>(_ path: GenericIntoRustString, _ level: GenericIntoRustString) {
        self.init(ptr: __swift_bridge__$Storage$new_with_log_level({ let rustString = path.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = level.intoRustString(); rustString.isOwned = false; return rustString.ptr }()))
    }
}
public class StorageRefMut: StorageRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
extension StorageRefMut {
    public func connect<GenericIntoRustString: IntoRustString>(_ workspace_id: GenericIntoRustString, _ remote: GenericIntoRustString) -> Optional<Workspace> {
        { let val = __swift_bridge__$Storage$connect(ptr, { let rustString = workspace_id.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = remote.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val != nil { return Workspace(ptr: val!) } else { return nil } }()
    }
}
public class StorageRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension StorageRef {
    public func error() -> Optional<RustString> {
        { let val = __swift_bridge__$Storage$error(ptr); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    }

    public func is_offline() -> Bool {
        __swift_bridge__$Storage$is_offline(ptr)
    }

    public func is_connected() -> Bool {
        __swift_bridge__$Storage$is_connected(ptr)
    }

    public func is_finished() -> Bool {
        __swift_bridge__$Storage$is_finished(ptr)
    }

    public func is_error() -> Bool {
        __swift_bridge__$Storage$is_error(ptr)
    }

    public func get_sync_state() -> RustString {
        RustString(ptr: __swift_bridge__$Storage$get_sync_state(ptr))
    }

    public func get_last_synced() -> RustVec<Int64> {
        RustVec(ptr: __swift_bridge__$Storage$get_last_synced(ptr))
    }
}
extension Storage: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_Storage$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_Storage$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Storage) {
        __swift_bridge__$Vec_Storage$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_Storage$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (Storage(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<StorageRef> {
        let pointer = __swift_bridge__$Vec_Storage$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return StorageRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<StorageRefMut> {
        let pointer = __swift_bridge__$Vec_Storage$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return StorageRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_Storage$len(vecPtr)
    }
}



