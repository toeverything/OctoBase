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
    
    func create_block_set_prop_demo() {
        print("create_block_set_prop_demo")
        let block = self.create(block_id: "test", flavor: "test")
        
        print(self.get(blockId: "test"))
        print(block.id().toString());
        print(block.flavor().toString())
        print(block.version().toString())
        
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
}
