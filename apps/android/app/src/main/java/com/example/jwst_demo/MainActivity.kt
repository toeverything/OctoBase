package com.example.jwst_demo

import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.appcompat.app.AppCompatActivity
import com.toeverything.jwst.Storage
import java.io.File
import java.util.*
import com.toeverything.jwst.JwstVecOfStrings
import kotlin.jvm.optionals.getOrNull

fun <T> Optional<T>.unwrap(): T? = orElse(null)

class MainActivity : AppCompatActivity() {

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val database = File(filesDir, "jwst.db")
        val storage = Storage(database.absolutePath, "ws://10.0.2.2:3000/collaboration", "debug")
        storage.getWorkspace("test").unwrap()?.let { workspace ->
            workspace.setCallback { block_ids -> Log.i("jwst", "change: $block_ids") }
            workspace.get("a").getOrNull()?.let { block ->
                // load the existing block on the second startup program.
                val content = block.get("a key").get()
                this.title = (content as String) + " exists"
                workspace.get("root").get().children().joinToString { it }.let { Log.i("jwst", it) }

                Thread.sleep(1000)
                workspace.create("child11", "child");

            } ?: run {
                // create a new block on the first startup program.

                val block = workspace.create("a", "b")
                block.set("a key", "a value")

                val content = {
                    val block = workspace.get("a").get()
                    block.get("a key").get()
                }
                this.title = content as String

                // new lot of block and insert into children
                {
                    val block1 = workspace.create("root", "root")
                    val block2 = workspace.create("child1", "child")
                    val block3 = workspace.create("child2", "child")
                    val block4 = workspace.create("child3", "child")
                    val block5 = workspace.create("child4", "child")
                    val block6 = workspace.create("child5", "child")
                    val block7 = workspace.create("child6", "child")
                    val block8 = workspace.create("child7", "child")
                    val block9 = workspace.create("child8", "child")
                    val block10 = workspace.create("child9", "child")
                    val block11 = workspace.create("child10", "child")

                    block1.insertChildrenAt(block2, 0)
                    block1.insertChildrenAt(block3, 1)
                    block1.insertChildrenAt(block4, 2)
                    block1.insertChildrenAt(block5, 3)
                    block1.insertChildrenAt(block6, 4)
                    block1.insertChildrenAt(block7, 5)
                    block1.insertChildrenAt(block8, 6)
                    block1.insertChildrenAt(block9, 7)
                    block1.insertChildrenAt(block10, 8)
                    block1.insertChildrenAt(block11, 9)
                }
            }

            Log.i("jwst", "getSyncState " + storage.getSyncState())
            Log.i("jwst", "isOffline " + storage.isOffline())
            Log.i("jwst", "isConnected " + storage.isConnected())
            Log.i("jwst", "isFinished " + storage.isFinished())
            Log.i("jwst", "isError " + storage.isError())

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
}
