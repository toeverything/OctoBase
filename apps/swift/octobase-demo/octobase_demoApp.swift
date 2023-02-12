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
    
    func get() -> Optional<Block> {
        return self.workspace.get("a")
    }
}
