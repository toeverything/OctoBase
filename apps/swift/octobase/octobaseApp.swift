//
//  octobaseApp.swift
//  octobase
//
//  Created by ds on 2023/2/9.
//

import SwiftUI



@main
struct octobaseApp: App {
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
