package com.example.jwst_demo

import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.appcompat.app.AppCompatActivity
import com.toeverything.jwst.Block
import com.toeverything.jwst.Storage
import java.io.File
import java.util.*
import com.toeverything.jwst.Workspace
import kotlin.jvm.optionals.getOrNull
import kotlin.random.Random

fun <T> Optional<T>.unwrap(): T? = orElse(null)

fun String.hexStringToByteArray(): ByteArray {
    return this.chunked(2)
        .map { it.toInt(16).toByte() }
        .toByteArray()
}

fun getStaticWorkspace(): String {
    return "010895E2C0E01D0027010A73706163653A6D6574610570616765730027010C73706163653A626C6F636B73047465737401280095E2C0E01D010B7379733A666C61766F757201770474657374270095E2C0E01D010C7379733A6368696C6472656E00280095E2C0E01D010B7379733A63726561746564017B4278B38B757B900028010D73706163653A757064617465640474657374017B4278B38B757B9000280095E2C0E01D010970726F703A74657374017703616263A895E2C0E01D05017B4278B38B757B90000195E2C0E01D010501"
}

fun getRandomId(): String {
    val chars = "abcdefghijklmnopqrstuvwxyz0123456789"
    return (1..8).map { chars[Random.nextInt(chars.length)] }.joinToString("")
}

class MainActivity : AppCompatActivity() {

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val database = File(filesDir, "jwst.db")
        val storage = Storage(database.absolutePath, "ws://10.0.2.2:3000/collaboration", "debug")

        storage.initWorkspace(getRandomId(), getStaticWorkspace().hexStringToByteArray())
        val text = storage.getWorkspace("test1").get().get("test").get().get("test").get()
        Log.i("jwst", "text: $text")

        storage.getWorkspace("test").unwrap()?.let { workspace ->
            setupWorkspace(workspace)

            workspace.create("test", "list")
            workspace.create("test2", "list")

            val blocks = workspace.getBlocksByFlavour("list")
            Log.i("jwst", "getBlocksByFlavour: $blocks")

            // search demo
            Log.i("jwst", "search demo")

            val block = workspace.create("search_test", "search_test_flavour")
            block.set("title", "introduction")
            block.set("text", "hello every one")
            block.set("index", "this is index")

            var indexFields = arrayOf("title", "text")
            workspace.setSearchIndex(indexFields)
            Log.i("jwst", "search index: " + workspace.getSearchIndex().joinToString(" "))

            val searchResult1 = "search result1: " + workspace.search("duc")
            Log.i("jwst", searchResult1)

            val searchResult2 = "search result2: " + workspace.search("this")
            Log.i("jwst", searchResult2)

            var indexFields2 = arrayOf("index")
            workspace.setSearchIndex(indexFields2)

            Log.i("jwst", "search index: " + workspace.getSearchIndex().joinToString(" "))

            val searchResult3 = "search result3: " + workspace.search("this")
            Log.i("jwst", searchResult3)

            while (true) {
                Log.i("jwst", " getting root")
                workspace.get("root").unwrap()?.let { block ->
                    block.get("test").ifPresent { value ->
                        Log.i("jwst", "test: $value")
                    }
                }

                Thread.sleep(1000)
            }
        }
    }

    private fun setupWorkspace(workspace: Workspace) {
        workspace.setCallback { blockIds ->
            Log.i("jwst", "change: $blockIds")
        }

        val existingBlock = workspace.get("a").getOrNull()

        if (existingBlock != null) {
            handleExistingBlock(existingBlock, workspace)
        } else {
            handleNewBlock(workspace)
        }

        while (true) {
            Log.i("jwst", " getting root")
            workspace.get("root").unwrap()?.let { block ->
                block.get("test").ifPresent { value ->
                    Log.i("jwst", "test: $value")
                }
            }
            Thread.sleep(1000)
        }
    }


    private fun handleExistingBlock(block: Block, workspace: Workspace) {
        val content = block.get("a key").get()
        this.title = "$content exists"

        val children = workspace.get("root").get().children().joinToString { it.toString() }
        Log.i("jwst", children)

        Thread.sleep(1000)
        workspace.create("child11", "child")
    }

    private fun handleNewBlock(workspace: Workspace) {
        val block = workspace.create("a", "b")
        block.set("a key", "a value")

        val content = workspace.get("a").get().get("a key").get()
        this.title = content as String

        // Create and insert blocks into children
        createAndInsertChildren(workspace)
    }

    private fun createAndInsertChildren(workspace: Workspace) {
        val root = workspace.create("root", "root")
        val children = (1..10).map { i ->
            workspace.create("child$i", "child")
        }

        children.forEachIndexed { index, child ->
            root.insertChildrenAt(child, index.toLong())
        }
    }

    private fun logWorkspaceStatus(storage: Storage) {
        Log.i("jwst", "getSyncState ${storage.getSyncState()}")
        Log.i("jwst", "isOffline ${storage.isOffline()}")
        Log.i("jwst", "isConnected ${storage.isConnected()}")
        Log.i("jwst", "isFinished ${storage.isFinished()}")
        Log.i("jwst", "isError ${storage.isError()}")
    }
}
