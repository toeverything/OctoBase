package com.toeverything.jwst

import java.util.*
import com.toeverything.jwst.lib.WorkspaceTransaction as JwstWorkspaceTransaction
import com.toeverything.jwst.lib.Block as JwstBlock
import com.toeverything.jwst.lib.Workspace as JwstWorkspace

class Workspace(id: String) {
    private var workspace: JwstWorkspace

    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    init {
        this.workspace = JwstWorkspace(id)
    }

    fun id(): String {
        return this.workspace.id();
    }

    fun client_id(): Long {
        return this.workspace.clientId();
    }

    fun get(block_id: String): Optional<Block> {
        return this.workspace.get(block_id).map { block -> Block(block) };
    }

    fun exists(block_id: String): Boolean {
        return this.workspace.exists(block_id);
    }

    fun withTrx(callback: (trx: WorkspaceTransaction) -> Unit) {
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

    fun create(id: String, flavor: String): Block {
        return Block(this.trx.create(id, flavor));
    }

    fun remove(block_id: String): Boolean {
        return this.trx.remove(block_id)
    }
}


class Block constructor(private var block: JwstBlock) {
    companion object {
        // Used to load the 'jwst' library on application startup.
        init {
            System.loadLibrary("jwst")
        }
    }

    fun <T> set(trx: WorkspaceTransaction, key: String, value: T?) {
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

    fun get(key: String): Optional<Any> {
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

    fun id(): String {
        return this.block.id()
    }

    fun flavor(): String {
        return this.block.flavor()
    }

    fun created(): Long {
        return this.block.created()
    }

    fun updated(): Long {
        return this.block.updated()
    }

    fun parent(): Optional<String> {
        return this.block.parent()
    }

    fun children(): Array<String> {
        return this.block.children()
    }

    fun pushChildren(trx: WorkspaceTransaction, block: Block) {
        this.block.pushChildren(trx.trx, block.block)
    }

    fun insertChildrenAt(trx: WorkspaceTransaction, block: Block, index: Long) {
        this.block.insertChildrenAt(trx.trx, block.block, index)
    }

    fun insertChildrenBefore(trx: WorkspaceTransaction, block: Block, before: String) {
        this.block.insertChildrenBefore(trx.trx, block.block, before)
    }

    fun insertChildrenAfter(trx: WorkspaceTransaction, block: Block, after: String) {
        this.block.insertChildrenAfter(trx.trx, block.block, after)
    }

    fun removeChildren(trx: WorkspaceTransaction, block: Block) {
        this.block.removeChildren(trx.trx, block.block)
    }

    fun existsChildren(block_id: String): Int {
        return this.block.existsChildren(block_id)
    }
}