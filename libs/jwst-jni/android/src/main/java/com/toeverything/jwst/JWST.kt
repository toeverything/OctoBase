package com.toeverything.jwst

import com.toeverything.jwst.lib.OnWorkspaceTransaction
import com.toeverything.jwst.lib.WorkspaceTransaction
import com.toeverything.jwst.lib.Block as JwstBlock;
import java.util.*
import com.toeverything.jwst.lib.Workspace as JwstWorkspace;

class Workspace(id: String) {

    /**
     * A native method that is implemented by the 'jwst' native library,
     * which is packaged with this application.
     */
    external fun stringFromJNI(): String

    private var workspace: JwstWorkspace

    companion object {
        // Used to load the 'jwst' library on application startup.
        init {
            System.loadLibrary("jwst")
        }
    }

    init {
        this.workspace = JwstWorkspace(id)
    }

    public fun id(): String {
        return this.workspace.id();
    }

    public fun client_id(): Long {
        return this.workspace.clientId();
    }

    public fun get(block_id: String): Optional<Block> {
        return this.workspace.get(block_id).map { block -> Block(block) };
    }

    public fun exists(block_id: String): Boolean {
        return this.workspace.exists(block_id);
    }

    public fun withTrx(callback: (trx: WorkspaceTransaction) -> Unit) {
        this.workspace.withTrx { trx -> callback(trx) }
    }
}

class Block constructor(private var block: JwstBlock) {
    /**
     * A native method that is implemented by the 'jwst' native library,
     * which is packaged with this application.
     */
    external fun stringFromJNI(): String

    companion object {
        // Used to load the 'jwst' library on application startup.
        init {
            System.loadLibrary("jwst")
        }
    }

    public fun id(): String {
        return this.block.id()
    }

    public fun flavor(): String {
        return this.block.flavor()
    }

    public fun created(): Long {
        return this.block.created()
    }

    public fun updated(): Long {
        return this.block.updated()
    }

    public fun parent(): Optional<String> {
        return this.block.parent()
    }

    public fun children(): Array<String> {
        return this.block.children()
    }
}