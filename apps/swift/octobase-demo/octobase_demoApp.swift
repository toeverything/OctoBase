//
//  octobase_demoApp.swift
//  octobase-demo
//
//  Created by ds on 2023/2/12.
//

import SwiftUI
import OctoBase

@main
struct octobase_demoApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView().environmentObject(
                JwstWorkspace(workspace: "test"))
            
        }
    }
}

class JwstWorkspace: ObservableObject {
    var workspace: Workspace
    
    init (workspace: String) {
        self.workspace = Workspace(workspace)
    }
    
    func create(block_id:String, flavor:String) -> Block {
        return self.workspace.create(block_id, flavor)
    }
    
    func get(blockId: String) -> Optional<Block> {
        return self.workspace.get(blockId)
    }
    
    func get_blocks_by_flavour(flavour: String) -> RustVec<OctoBase.Block> {
        return self.workspace.get_blocks_by_flavour(flavour)
    }
    
    func create_block_set_prop_demo() {
        print("create_block_set_prop_demo")
        let block = self.create(block_id: "test", flavor: "test")
        
        print(self.get(blockId: "test"))
        print(block.id().toString());
        print(block.flavor().toString())
        
        block.set_integer("integer", 1)
        block.set_float("float",  1.1)
        block.set_bool("bool", true)
        block.set_string("string", "blockValue")
        block.set_null("null")
        
        print(block.get_integer("integer"))
        print(block.get_float("float"))
        print(block.get_bool("bool"))
        print(block.get_string("string")?.toString())
    }
    
    func insert_remove_children_demo() {
        print("insert_remove_children_demo")
        let block = self.create(block_id: "test", flavor: "test")
        let b = self.create(block_id: "b", flavor: "test")
        let c = self.create(block_id: "c", flavor: "test")
        let d = self.create(block_id: "d", flavor: "test")
        let e = self.create(block_id: "e", flavor: "test")
        let f = self.create(block_id: "f", flavor: "test")
        
        block.push_children(b)
        block.insert_children_at(c, 0)
        block.insert_children_before(d, "b")
        block.insert_children_after(e, "b")
        block.insert_children_after(f, "c")
        for child in block.children() {
            print(child.as_str().toString());
        }
        
        block.remove_children(d);
        print("after remove:")
        for child in block.children() {
            print(child.as_str().toString());
        }
    }
    
    func search_demo() {
        print("search_demo")
        let block = self.create(block_id: "search_test", flavor: "search_test_flavor")
        block.set_string("title", "introduction")
        block.set_string("text", "hello every one")
        block.set_string("index", "this is demo")
        
        let index_fields1 = RustVec<RustString>()
        index_fields1.push(value: "title".intoRustString())
        index_fields1.push(value: "text".intoRustString())
        self.workspace.set_search_index(index_fields1)
        print("search_index: ", terminator: "")
        for field in self.workspace.get_search_index() {
            print(field.as_str().toString() + " ", terminator: "");
        }
        print("")
        
        let result1 = self.workspace.search("duc")
        print("search_result1: ", result1.toString())
        
        let result2 = self.workspace.search("this")
        print("search_result2: ", result2.toString())
        
        let index_fields2 = RustVec<RustString>()
        index_fields2.push(value: "index".intoRustString())
        self.workspace.set_search_index(index_fields2)
        print("search_index: ", terminator: "")
        for field in self.workspace.get_search_index() {
            print(field.as_str().toString() + " ", terminator: "");
        }
        print("")
        
        let result3 = self.workspace.search("this")
        print("search_result3: ", result3.toString())
    }
    
    func search_blocks_demo() {
        print("search_blocks_demo")
        let block = self.create(block_id: "test", flavor: "test")
        print(self.get_blocks_by_flavour(flavour: "test"))
    }
    
    func storage_demo() {
        let fileManager = FileManager.default

        if let documentDirectory = try? fileManager.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: false) {
            let fileURL = documentDirectory.appendingPathComponent("jwst.db")
            if !fileManager.fileExists(atPath: fileURL.path) {
                fileManager.createFile(atPath: fileURL.path, contents: nil, attributes: nil)
            }
            let storage = Storage(fileURL.description.intoRustString())
            print("is_offline", storage.is_offline())
            print("is_initialized", storage.is_initialized())
            print("is_finished", storage.is_finished())
            print("is_syncing", storage.is_syncing())
            print("is_error", storage.is_error())
            print("get_sync_state", storage.get_sync_state())
        }
    }
}

class JWSTStorage: ObservableObject {
    var storage: Storage
    
    var remote: String
    
    // path:    path of sqlite db
    // remote:  websocket server api, eg: ws://localhost:3000/collaboration
    init (path: String, remote: String) {
        self.storage = Storage(path)
        self.remote = remote
    }
    
    func get_workspace(id: String) -> Workspace {
        return self.storage.connect(id, self.remote + "/" + id)!
    }
}
