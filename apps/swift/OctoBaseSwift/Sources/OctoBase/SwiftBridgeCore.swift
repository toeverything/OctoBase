import RustXcframework
import Foundation

extension RustString {
    public func toString() -> String {
        let str = self.as_str()
        let string = str.toString()

        return string
    }
}

extension RustStr {
    func toBufferPointer() -> UnsafeBufferPointer<UInt8> {
        let bytes = UnsafeBufferPointer(start: self.start, count: Int(self.len))
        return bytes
    }

    public func toString() -> String {
        let bytes = self.toBufferPointer()
        return String(bytes: bytes, encoding: .utf8)!
    }
}
extension RustStr: Identifiable {
    public var id: String {
        self.toString()
    }
}
extension RustStr: Equatable {
    public static func == (lhs: RustStr, rhs: RustStr) -> Bool {
        return __swift_bridge__$RustStr$partial_eq(lhs, rhs);
    }
}

public protocol IntoRustString {
    func intoRustString() -> RustString;
}

public protocol ToRustStr {
    func toRustStr<T> (_ withUnsafeRustStr: (RustStr) -> T) -> T;
}

extension String: IntoRustString {
    public func intoRustString() -> RustString {
        // TODO: When passing an owned Swift std String to Rust we've being wasteful here in that
        //  we're creating a RustString (which involves Boxing a Rust std::string::String)
        //  only to unbox it back into a String once it gets to the Rust side.
        //
        //  A better approach would be to pass a RustStr to the Rust side and then have Rust
        //  call `.to_string()` on the RustStr.
        RustString(self)
    }
}

extension RustString: IntoRustString {
    public func intoRustString() -> RustString {
        self
    }
}

/// If the String is Some:
///   Safely get a scoped pointer to the String and then call the callback with a RustStr
///   that uses that pointer.
///
/// If the String is None:
///   Call the callback with a RustStr that has a null pointer.
///   The Rust side will know to treat this as `None`.
func optionalStringIntoRustString<S: IntoRustString>(_ string: Optional<S>) -> RustString? {
    if let val = string {
        return val.intoRustString()
    } else {
        return nil
    }
}

extension String: ToRustStr {
    /// Safely get a scoped pointer to the String and then call the callback with a RustStr
    /// that uses that pointer.
    public func toRustStr<T> (_ withUnsafeRustStr: (RustStr) -> T) -> T {
        return self.utf8CString.withUnsafeBufferPointer({ bufferPtr in
            let rustStr = RustStr(
                start: UnsafeMutableRawPointer(mutating: bufferPtr.baseAddress!).assumingMemoryBound(to: UInt8.self),
                // Subtract 1 because of the null termination character at the end
                len: UInt(bufferPtr.count - 1)
            )
            return withUnsafeRustStr(rustStr)
        })
    }
}

extension RustStr: ToRustStr {
    public func toRustStr<T> (_ withUnsafeRustStr: (RustStr) -> T) -> T {
        return withUnsafeRustStr(self)
    }
}

func optionalRustStrToRustStr<S: ToRustStr, T>(_ str: Optional<S>, _ withUnsafeRustStr: (RustStr) -> T) -> T {
    if let val = str {
        return val.toRustStr(withUnsafeRustStr)
    } else {
        return withUnsafeRustStr(RustStr(start: nil, len: 0))
    }
}
// TODO:
//  Implement iterator https://developer.apple.com/documentation/swift/iteratorprotocol

public class RustVec<T: Vectorizable> {
    var ptr: UnsafeMutableRawPointer
    var isOwned: Bool = true

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }

    public init() {
        ptr = T.vecOfSelfNew()
        isOwned = true
    }

    public func push (value: T) {
        T.vecOfSelfPush(vecPtr: ptr, value: value)
    }

    public func pop () -> Optional<T> {
        T.vecOfSelfPop(vecPtr: ptr)
    }

    public func get(index: UInt) -> Optional<T.SelfRef> {
         T.vecOfSelfGet(vecPtr: ptr, index: index)
    }

    /// Rust returns a UInt, but we cast to an Int because many Swift APIs such as
    /// `ForEach(0..rustVec.len())` expect Int.
    public func len() -> Int {
        Int(T.vecOfSelfLen(vecPtr: ptr))
    }

    deinit {
        if isOwned {
            T.vecOfSelfFree(vecPtr: ptr)
        }
    }
}

extension RustVec: Sequence {
    public func makeIterator() -> RustVecIterator<T> {
        return RustVecIterator(self)
    }
}

public struct RustVecIterator<T: Vectorizable>: IteratorProtocol {
    var rustVec: RustVec<T>
    var index: UInt = 0

    init (_ rustVec: RustVec<T>) {
        self.rustVec = rustVec
    }

    public mutating func next() -> T.SelfRef? {
        let val = rustVec.get(index: index)
        index += 1
        return val
    }
}

extension RustVec: Collection {
    public typealias Index = Int

    public func index(after i: Int) -> Int {
        i + 1
    }

    public subscript(position: Int) -> T.SelfRef {
        self.get(index: UInt(position))!
    }

    public var startIndex: Int {
        0
    }

    public var endIndex: Int {
        self.len()
    }
}

extension RustVec: RandomAccessCollection {
}

extension UnsafeBufferPointer {
    func toFfiSlice () -> __private__FfiSlice {
        __private__FfiSlice(start: UnsafeMutablePointer(mutating: self.baseAddress), len: UInt(self.count))
    }
}

extension Array {
    /// Get an UnsafeBufferPointer to the array's content's first byte with the array's length.
    ///
    /// ```
    /// // BAD! Swift will immediately free the arrays memory and so your pointer is invalid.
    /// let pointer = useMyPointer([1, 2, 3].toUnsafeBufferPointer())
    ///
    /// // GOOD! The array will outlive the buffer pointer.
    /// let array = [1, 2, 3]
    /// useMyPointer(array.toUnsafeBufferPointer())
    /// ```
    func toUnsafeBufferPointer() -> UnsafeBufferPointer<Element> {
        UnsafeBufferPointer(start: UnsafePointer(self), count: self.count)
    }
}

public protocol Vectorizable {
    associatedtype SelfRef
    associatedtype SelfRefMut

    static func vecOfSelfNew() -> UnsafeMutableRawPointer;

    static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer)

    static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self)

    static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self>

    static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<SelfRef>

    static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<SelfRefMut>

    static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt
}

extension UInt8: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_u8$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_u8$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_u8$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u8$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u8$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u8$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_u8$len(vecPtr)
    }
}
    
extension UInt16: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_u16$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_u16$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_u16$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u16$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u16$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u16$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_u16$len(vecPtr)
    }
}
    
extension UInt32: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_u32$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_u32$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_u32$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u32$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u32$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u32$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_u32$len(vecPtr)
    }
}
    
extension UInt64: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_u64$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_u64$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_u64$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u64$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u64$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_u64$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_u64$len(vecPtr)
    }
}
    
extension UInt: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_usize$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_usize$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_usize$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_usize$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_usize$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_usize$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_usize$len(vecPtr)
    }
}
    
extension Int8: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_i8$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_i8$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_i8$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i8$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i8$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i8$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_i8$len(vecPtr)
    }
}
    
extension Int16: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_i16$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_i16$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_i16$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i16$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i16$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i16$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_i16$len(vecPtr)
    }
}
    
extension Int32: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_i32$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_i32$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_i32$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i32$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i32$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i32$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_i32$len(vecPtr)
    }
}
    
extension Int64: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_i64$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_i64$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_i64$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i64$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i64$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_i64$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_i64$len(vecPtr)
    }
}
    
extension Int: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_isize$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_isize$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_isize$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_isize$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_isize$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_isize$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_isize$len(vecPtr)
    }
}
    
extension Bool: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_bool$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_bool$_free(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Self) {
        __swift_bridge__$Vec_bool$push(vecPtr, value)
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let val = __swift_bridge__$Vec_bool$pop(vecPtr)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_bool$get(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<Self> {
        let val = __swift_bridge__$Vec_bool$get_mut(vecPtr, index)
        if val.is_some {
            return val.val
        } else {
            return nil
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_bool$len(vecPtr)
    }
}
    
protocol SwiftBridgeGenericFreer {
    func rust_free();
}
    
protocol SwiftBridgeGenericCopyTypeFfiRepr {}

public class RustString: RustStringRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RustString$_free(ptr)
        }
    }
}
extension RustString {
    public convenience init() {
        self.init(ptr: __swift_bridge__$RustString$new())
    }

    public convenience init<GenericToRustStr: ToRustStr>(_ str: GenericToRustStr) {
        self.init(ptr: str.toRustStr({ strAsRustStr in
            __swift_bridge__$RustString$new_with_str(strAsRustStr)
        }))
    }
}
public class RustStringRefMut: RustStringRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RustStringRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RustStringRef {
    public func len() -> UInt {
        __swift_bridge__$RustString$len(ptr)
    }

    public func as_str() -> RustStr {
        __swift_bridge__$RustString$as_str(ptr)
    }

    public func trim() -> RustStr {
        __swift_bridge__$RustString$trim(ptr)
    }
}
extension RustString: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RustString$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RustString$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RustString) {
        __swift_bridge__$Vec_RustString$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RustString$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RustString(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RustStringRef> {
        let pointer = __swift_bridge__$Vec_RustString$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RustStringRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RustStringRefMut> {
        let pointer = __swift_bridge__$Vec_RustString$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RustStringRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RustString$len(vecPtr)
    }
}

public class __private__RustFnOnceCallbackNoArgsNoRet {
    var ptr: UnsafeMutableRawPointer
    var called = false

    init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }

    deinit {
        if !called {
            __swift_bridge__$free_boxed_fn_once_no_args_no_return(ptr)
        }
    }

    func call() {
        if called {
            fatalError("Cannot call a Rust FnOnce function twice")
        }
        called = true
        return __swift_bridge__$call_boxed_fn_once_no_args_no_return(ptr)
    }
}


public enum RustResult<T, E> {
    case Ok(T)
    case Err(E)
}

extension RustResult {
    func ok() -> T? {
        switch self {
        case .Ok(let ok):
            return ok
        case .Err(_):
            return nil
        }
    }

    func err() -> E? {
        switch self {
        case .Ok(_):
            return nil
        case .Err(let err):
            return err
        }
    }
    
    func toResult() -> Result<T, E>
    where E: Error {
        switch self {
        case .Ok(let ok):
            return .success(ok)
        case .Err(let err):
            return .failure(err)
        }
    }
}
