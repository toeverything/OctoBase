package com.toeverything.jwst

import com.toeverything.jwst.lib.JwstStorage
import java.util.*
import com.toeverything.jwst.lib.Block as JwstBlock
import com.toeverything.jwst.lib.Workspace as JwstWorkspace
import com.toeverything.jwst.lib.WorkspaceTransaction as JwstWorkspaceTransaction

class Workspace(workspace: JwstWorkspace) {
    private var workspace: JwstWorkspace

    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    init {
        this.workspace = workspace
    }

    fun id(): String {
        return this.workspace.id()
    }

    fun client_id(): Long {
        return this.workspace.clientId()
    }

    fun get(trx: WorkspaceTransaction, block_id: String): Optional<Block> {
        return this.workspace.get(trx.trx, block_id).map { block -> Block(block) }
    }

    fun exists(trx: WorkspaceTransaction, block_id: String): Boolean {
        return this.workspace.exists(trx.trx, block_id)
    }

    fun <T> withTrx(callback: (trx: WorkspaceTransaction) -> T): T? {
        var ret: T? = null
        for (i in 0..5) {
            val success = this.workspace.withTrx { trx ->
                run {
                    ret = callback(WorkspaceTransaction(trx))
                    this.workspace.dropTrx(trx)
                }
            }
            if (success) {
                return ret
            }
            Thread.sleep(50)
        }

        return ret
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
        return Block(this.trx.create(id, flavor))
    }

    fun remove(block_id: String): Boolean {
        return this.trx.remove(block_id)
    }

    fun commit() {
        this.trx.commit()
    }
}


class Block constructor(private var block: JwstBlock) {
    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    fun <T> set(trx: WorkspaceTransaction, key: String, value: T?) {
        value?.let {
            when (it) {
                is Boolean -> this.block.setBool(trx.trx, key, it)
                is String -> this.block.setString(trx.trx, key, it)
                is Int -> this.block.setInteger(trx.trx, key, it.toLong())
                is Long -> this.block.setInteger(trx.trx, key, it)
                is Float -> this.block.setFloat(trx.trx, key, it.toDouble())
                is Double -> this.block.setFloat(trx.trx, key, it)
                else -> throw Exception("Unsupported type")
            }
        } ?: run {
            this.block.setNull(trx.trx, key)
        }
    }

    fun get(trx: WorkspaceTransaction, key: String): Optional<Any> {
        return when {
            this.block.isBool(trx.trx, key) -> Optional.of(this.block.getBool(trx.trx, key))
                .filter(OptionalLong::isPresent).map(OptionalLong::getAsLong).map { it == 1L }
            this.block.isString(trx.trx, key) -> Optional.of(this.block.getString(trx.trx, key))
                .filter(Optional<String>::isPresent).map(Optional<String>::get)
            this.block.isInteger(trx.trx, key) -> Optional.of(this.block.getInteger(trx.trx, key))
                .filter(OptionalLong::isPresent).map(OptionalLong::getAsLong)
            this.block.isFloat(trx.trx, key) -> Optional.of(this.block.getFloat(trx.trx, key))
                .filter(OptionalDouble::isPresent).map(OptionalDouble::getAsDouble)
            else -> Optional.empty()
        }
    }

    fun id(): String {
        return this.block.id()
    }

    fun flavor(trx: WorkspaceTransaction): String {
        return this.block.flavor(trx.trx)
    }

    fun created(trx: WorkspaceTransaction): Long {
        return this.block.created(trx.trx)
    }

    fun updated(trx: WorkspaceTransaction): Long {
        return this.block.updated(trx.trx)
    }

    fun parent(trx: WorkspaceTransaction): Optional<String> {
        return this.block.parent(trx.trx)
    }

    fun children(trx: WorkspaceTransaction): Array<String> {
        return this.block.children(trx.trx)
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

    fun existsChildren(trx: WorkspaceTransaction, block_id: String): Int {
        return this.block.existsChildren(trx.trx, block_id)
    }
}

class Storage constructor(path: String, private val remote: String = "") {
    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    private var storage = JwstStorage(path)

    val failed get() = this.storage.error().isPresent

    val error get() = this.storage.error()

    fun getWorkspace(id: String): Optional<Workspace> {
        return  this.storage.connect(id, this.remote + "/" + id).map { Workspace(it) }
    }
}