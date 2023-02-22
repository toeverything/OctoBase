package com.example.jwst_demo

import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.appcompat.app.AppCompatActivity
import com.toeverything.jwst.Storage
import java.io.File
import java.util.*

fun <T> Optional<T>.unwrap(): T? = orElse(null)

class MainActivity : AppCompatActivity() {

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val database = File(filesDir, "jwst.db")
        val storage = Storage(database.absolutePath, "ws://10.0.2.2:3000/collaboration/test")
        storage.getWorkspace("test").unwrap()?.let { workspace ->
            workspace.withTrx { trx -> workspace.get(trx, "a").unwrap() }?.let { block ->
                // load the existing block on the second startup program.
                val content = workspace.withTrx { trx -> block.get(trx, "a key") }?.get()
                this.title = (content as String) + " exists"
                workspace.withTrx { trx -> workspace.get(trx, "root").get().children(trx) }
                    ?.joinToString { it }
                    ?.let { Log.i("jwst", it) }
                workspace.withTrx { trx ->
                    Thread.sleep(1000)
                    trx.create("child11", "child");
                }
            } ?: run {
                // create a new block on the first startup program.
                workspace.withTrx { trx ->
                    val block = trx.create("a", "b")
                    block.set(trx, "a key", "a value")
                }

                val content = workspace.withTrx { trx ->
                    val block = workspace.get(trx, "a").get()
                    block.get(trx, "a key").get()
                }
                this.title = content as String

                // new lot of block and insert into children
                workspace.withTrx { trx ->
                    val block1 = trx.create("root", "root")
                    val block2 = trx.create("child1", "child")
                    val block3 = trx.create("child2", "child")
                    val block4 = trx.create("child3", "child")
                    val block5 = trx.create("child4", "child")
                    val block6 = trx.create("child5", "child")
                    val block7 = trx.create("child6", "child")
                    val block8 = trx.create("child7", "child")
                    val block9 = trx.create("child8", "child")
                    val block10 = trx.create("child9", "child")
                    val block11 = trx.create("child10", "child")

                    block1.insertChildrenAt(trx, block2, 0)
                    block1.insertChildrenAt(trx, block3, 1)
                    block1.insertChildrenAt(trx, block4, 2)
                    block1.insertChildrenAt(trx, block5, 3)
                    block1.insertChildrenAt(trx, block6, 4)
                    block1.insertChildrenAt(trx, block7, 5)
                    block1.insertChildrenAt(trx, block8, 6)
                    block1.insertChildrenAt(trx, block9, 7)
                    block1.insertChildrenAt(trx, block10, 8)
                    block1.insertChildrenAt(trx, block11, 9)
                }
            }
            while (true) {
                workspace.withTrx { trx ->
                    Log.i("jwst", " getting root")
                    workspace.get(trx, "root").unwrap()?.let { block ->
                        block.get(trx, "test").ifPresent { value ->
                            Log.i("jwst", "test: $value")
                        }
                    }
                }
                Thread.sleep(1000)
            }
        }
    }
}