package com.example.jwst_demo

import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.appcompat.app.AppCompatActivity
import com.toeverything.jwst.Storage
import java.io.File

class MainActivity : AppCompatActivity() {

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val database = File(filesDir, "jwst.db");
        val storage = Storage(database.absolutePath, "ws://10.0.2.2:3000/collaboration/test");
        val workspace = storage.getWorkspace("test")

        workspace.get("a").ifPresentOrElse(
            { block ->
                // load the existing block on the second startup program.
                val content = block.get("a key").get();
                this.title = (content as String) + " exists";
                Log.i("jwst", workspace.get("root").get().children().joinToString { it });
                workspace.withTrx { trx ->
                    Thread.sleep(5000);
                    trx.create("child11", "child");
                };
            },
            {
                // create a new block on the first startup program.
                workspace.withTrx { trx ->
                    val block = trx.create("a", "b");
                    block.set(trx, "a key", "a value");
                };
                val block = workspace.get("a").get();
                val content = block.get("a key").get();
                this.title = content as String;

                // new lot of block and insert into children
                workspace.withTrx { trx ->
                    val block1 = trx.create("root", "root");
                    var block2 = trx.create("child1", "child");
                    var block3 = trx.create("child2", "child");
                    var block4 = trx.create("child3", "child");
                    var block5 = trx.create("child4", "child");
                    var block6 = trx.create("child5", "child");
                    var block7 = trx.create("child6", "child");
                    var block8 = trx.create("child7", "child");
                    var block9 = trx.create("child8", "child");
                    var block10 = trx.create("child9", "child");
                    var block11 = trx.create("child10", "child");

                    block1.insertChildrenAt(trx, block2, 0);
                    block1.insertChildrenAt(trx, block3, 1);
                    block1.insertChildrenAt(trx, block4, 2);
                    block1.insertChildrenAt(trx, block5, 3);
                    block1.insertChildrenAt(trx, block6, 4);
                    block1.insertChildrenAt(trx, block7, 5);
                    block1.insertChildrenAt(trx, block8, 6);
                    block1.insertChildrenAt(trx, block9, 7);
                    block1.insertChildrenAt(trx, block10, 8);
                    block1.insertChildrenAt(trx, block11, 9);
                };

            }
        )
    }
}