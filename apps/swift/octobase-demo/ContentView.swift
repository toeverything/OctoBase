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
                    let block = workspace.create(block_id: "test", flavor: "test")
                    
                    print(block.get("test").map({
                        $0.as_array()
                    }) as Any)
                    
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
