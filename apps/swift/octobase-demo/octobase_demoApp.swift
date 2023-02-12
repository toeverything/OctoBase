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
                JwstWorkspace(workspace: Workspace("test")))
        }
    }
}

class JwstWorkspace: ObservableObject {
    var workspace: Workspace

    init (workspace: Workspace) {
        self.workspace = workspace
    }
}
