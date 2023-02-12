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
extension Workspace {
    public convenience init<GenericIntoRustString: IntoRustString>(_ id: GenericIntoRustString) {
        self.init(ptr: __swift_bridge__$Workspace$new({ let rustString = id.intoRustString(); rustString.isOwned = false; return rustString.ptr }()))
    }
}
public class WorkspaceRefMut: WorkspaceRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class WorkspaceRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension WorkspaceRef {
    public func get<GenericIntoRustString: IntoRustString>(_ block_id: GenericIntoRustString) -> Optional<Block> {
        { let val = __swift_bridge__$Workspace$get(ptr, { let rustString = block_id.intoRustString(); rustString.isOwned = false; return rustString.ptr }()); if val != nil { return Block(ptr: val!) } else { return nil } }()
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



