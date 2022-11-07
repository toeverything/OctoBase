package com.toeverything.jwst

import com.toeverything.jwst.lib.Block as JwstBlock;
import java.util.*
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

    public fun id(): String {
        return this.workspace.id();
    }

    public fun client_id(): Long {
        return this.workspace.clientId();
    }

    public fun get(block_id: String): Optional<JwstBlock> {
        return this.workspace.get(block_id);
    }

    public fun exists(block_id: String): Boolean {
        return this.workspace.exists(block_id);
    }
}