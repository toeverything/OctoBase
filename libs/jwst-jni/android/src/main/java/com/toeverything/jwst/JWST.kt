package com.toeverything.jwst

import com.toeverything.jwst.lib.Workspace as JwstWorkspace;

class Workspace {

    /**
     * A native method that is implemented by the 'jwst' native library,
     * which is packaged with this application.
     */
    external fun stringFromJNI(): String

    var workspace: JwstWorkspace

    companion object {
        // Used to load the 'jwst' library on application startup.
        init {
            System.loadLibrary("jwst")
        }
    }

    constructor(id: String) {
        this.workspace = JwstWorkspace(id)
    }
}