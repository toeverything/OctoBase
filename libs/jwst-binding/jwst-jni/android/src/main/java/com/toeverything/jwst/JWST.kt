package com.toeverything.jwst

import com.toeverything.jwst.lib.JwstStorage
import java.util.*
import com.toeverything.jwst.lib.Block as JwstBlock
import com.toeverything.jwst.lib.Workspace as JwstWorkspace
import com.toeverything.jwst.lib.VecOfStrings

typealias JwstVecOfStrings = VecOfStrings

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

    fun clientId(): Long {
        return this.workspace.clientId()
    }

    fun create(blockId: String, flavour: String): Block {
        return Block(this.workspace.create(blockId, flavour))
    }

    fun get(blockId: String): Optional<Block> {
        return this.workspace.get(blockId).map { block -> Block(block) }
    }

    fun getBlocksByFlavour(flavour: String): List<Block> {
        return this.workspace.getBlocksByFlavour(flavour).map { block -> Block(block) }
    }

    fun exists(blockId: String): Boolean {
        return this.workspace.exists(blockId)
    }

    fun remove(blockId: String): Boolean {
        return this.workspace.remove(blockId)
    }

    fun search(query: String): String {
        return this.workspace.search(query)
    }

    fun getSearchIndex(): Array<String> {
        return this.workspace.getSearchIndex()
    }

    fun setSearchIndex(fields: Array<String>): Boolean {
        val indexFields = JwstVecOfStrings()
        for (item in fields) {
            indexFields.push(item)
        }
        return this.workspace.setSearchIndex(indexFields)
    }

    fun setCallback(callback: (blockIds: Array<String>) -> Unit): Boolean {
//        return this.workspace.setCallback {
//            blockIds -> run {
//                var x = mutableListOf<String>()
//                for (i in 0 until block_ids.len()) {
//                    x.add(block_ids.at(i))
//                }
//                callback(x.toTypedArray())
//            }
//        }
        return false
    }
}

class Block constructor(private var block: JwstBlock) {
    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    fun <T> set(key: String, value: T?) {
        value?.let {
            when (it) {
                is Boolean -> this.block.setBool(key, it)
                is String -> this.block.setString(key, it)
                is Int -> this.block.setInteger(key, it.toLong())
                is Long -> this.block.setInteger(key, it)
                is Float -> this.block.setFloat(key, it.toDouble())
                is Double -> this.block.setFloat(key, it)
                else -> throw Exception("Unsupported type")
            }
        } ?: run {
            this.block.setNull(key)
        }
    }

    fun get(key: String): Optional<Any> {
        return when {
            this.block.isBool(key) -> Optional.of(this.block.getBool(key)).filter(OptionalLong::isPresent)
                .map(OptionalLong::getAsLong).map { it == 1L }

            this.block.isString(key) -> Optional.of(this.block.getString(key)).filter(Optional<String>::isPresent)
                .map(Optional<String>::get)

            this.block.isInteger(key) -> Optional.of(this.block.getInteger(key)).filter(OptionalLong::isPresent)
                .map(OptionalLong::getAsLong)

            this.block.isFloat(key) -> Optional.of(this.block.getFloat(key)).filter(OptionalDouble::isPresent)
                .map(OptionalDouble::getAsDouble)

            else -> Optional.empty()
        }
    }

    fun id(): String {
        return this.block.id()
    }

    fun flavour(): String {
        return this.block.flavour()
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

    fun pushChildren(block: Block) {
        this.block.pushChildren(block.block)
    }

    fun insertChildrenAt(block: Block, index: Long) {
        this.block.insertChildrenAt(block.block, index)
    }

    fun insertChildrenBefore(block: Block, before: String) {
        this.block.insertChildrenBefore(block.block, before)
    }

    fun insertChildrenAfter(block: Block, after: String) {
        this.block.insertChildrenAfter(block.block, after)
    }

    fun removeChildren(block: Block) {
        this.block.removeChildren(block.block)
    }

    fun existsChildren(block_id: String): Int {
        return this.block.existsChildren(block_id)
    }
}

class Storage constructor(path: String, private val remote: String = "", private val logLevel: String = "debug") {
    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    private var storage = JwstStorage(path, logLevel)

    val failed get() = this.storage.error().isPresent

    val error get() = this.storage.error()

    fun initWorkspace(id: String, data: ByteArray): Result<Unit> {
        val success = this.storage.init(id,data)
        return if (success) {
            Result.success(Unit)
        } else {
            val error = this.storage.error().orElse("Unknown error")
            Result.failure(Exception(error))
        }
    }

    fun exportWorkspace(id: String): Result<ByteArray> {
        val data = this.storage.export(id)
        return if (data.isNotEmpty()) {
            Result.success(data)
        } else {
            val error = this.storage.error().orElse("Unknown error")
            Result.failure(Exception(error))
        }
    }

    fun getWorkspace(id: String): Optional<Workspace> {
        return this.storage.connect(id, this.remote.takeIf { it.isNotEmpty() }?.let { "$it/$id" } ?: "").map { Workspace(it) }
    }

    fun isOffline(): Boolean {
        return this.storage.is_offline
    }

    fun isConnected(): Boolean {
        return this.storage.is_connected
    }

    fun isFinished(): Boolean {
        return this.storage.is_finished
    }

    fun isError(): Boolean {
        return this.storage.is_error
    }

    fun getSyncState(): String {
        return this.storage._sync_state
    }

    fun getLastSynced(): LongArray? {
        return this.storage._last_synced
    }
}
