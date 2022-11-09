package com.toeverything.jwst

import com.toeverything.jwst.lib.WorkspaceTransaction as JwstWorkspaceTransaction;
import com.toeverything.jwst.lib.Block as JwstBlock;
import java.util.*
import com.toeverything.jwst.lib.Workspace as JwstWorkspace;

class Workspace(id: String) {

    /**
     * A native method that is implemented by the 'jwst' native library,
     * which is packaged with this application.
     */
    external fun stringFromJNI(): String

    internal var workspace: JwstWorkspace

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
        this.workspace.withTrx { trx -> callback(WorkspaceTransaction(trx)) }
    }
}

class WorkspaceTransaction constructor(internal var trx: JwstWorkspaceTransaction) {
    companion object {
        // Used to load the 'jwst' library on application startup.
        init {
            System.loadLibrary("jwst")
        }
    }

    public fun create(id: String, flavor: String): Block {
        return Block(this.trx.create(id, flavor));
    }

    public fun remove(block_id: String): Boolean {
        return this.trx.remove(block_id)
    }
}


class Block constructor(internal var block: JwstBlock) {
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

    public fun <T> set(trx: WorkspaceTransaction, key: String, value: T?) {
        value?.let {
            if (it is Boolean) {
                this.block.setBool(trx.trx, key, it)
            } else if (it is String) {
                this.block.setString(trx.trx, key, it)
            } else if (it is Int) {
                this.block.setInteger(trx.trx, key, it.toLong())
            } else if (it is Long) {
                this.block.setInteger(trx.trx, key, it)
            } else if (it is Float) {
                this.block.setFloat(trx.trx, key, it.toDouble())
            } else if (it is Double) {
                this.block.setFloat(trx.trx, key, it)
            } else {
                throw Exception("Unsupported type");
            }
        } ?: run {
            this.block.setNull(trx.trx, key)
        }
    }


    public fun get(key: String): Optional<Any> {
        return when {
            this.block.isBool(key) -> Optional.of(this.block.getBool(key))
                .filter(OptionalLong::isPresent)
                .map(OptionalLong::getAsLong).map { it == 1L }
            this.block.isString(key) -> Optional.of(this.block.getString(key))
                .filter(Optional<String>::isPresent)
                .map(Optional<String>::get)
            this.block.isInteger(key) -> Optional.of(this.block.getInteger(key))
                .filter(OptionalLong::isPresent)
                .map(OptionalLong::getAsLong)
            this.block.isFloat(key) -> Optional.of(this.block.getFloat(key))
                .filter(OptionalDouble::isPresent)
                .map(OptionalDouble::getAsDouble)
            else -> Optional.empty();
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

    public fun pushChildren(trx: WorkspaceTransaction, block: Block) {
        this.block.pushChildren(trx.trx, block.block)
    }

    public fun insertChildrenAt(trx: WorkspaceTransaction, block: Block, index: Long) {
        this.block.insertChildrenAt(trx.trx, block.block, index)
    }

    public fun insertChildrenBefore(trx: WorkspaceTransaction, block: Block, before: String) {
        this.block.insertChildrenBefore(trx.trx, block.block, before)
    }

    public fun insertChildrenAfter(trx: WorkspaceTransaction, block: Block, after: String) {
        this.block.insertChildrenAfter(trx.trx, block.block, after)
    }

    public fun removeChildren(trx: WorkspaceTransaction, block: Block) {
        this.block.removeChildren(trx.trx, block.block)
    }

    public fun existsChildren(block_id: String): Int {
        return this.block.existsChildren(block_id)
    }
}