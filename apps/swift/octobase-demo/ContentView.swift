//
//  ContentView.swift
//  octobase-demo
//
//  Created by ds on 2023/2/12.
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject var workspace:JwstWorkspace
    var body: some View {
        VStack {
            Image(systemName: "globe")
                .imageScale(.large)
                .foregroundColor(.accentColor)
            Text("Hello, world!")
            Button(
                action: {
                    workspace.create_block_set_prop_demo();
                    workspace.insert_remove_children_demo();
                    
                },
                label: { Text("Click Me") }
            )
            .foregroundColor(Color.white)
            .padding()
            .background(Color.blue)
            .cornerRadius(5)
        }
        .padding()
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
