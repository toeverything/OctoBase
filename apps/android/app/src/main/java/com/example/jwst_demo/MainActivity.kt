package com.example.jwst_demo

import android.os.Build
import android.os.Bundle
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
        val storage = Storage(database.absolutePath);
        val workspace = storage.getWorkspace("test")

        workspace.get("a").ifPresentOrElse(
            { block ->
                // load the existing block on the second startup program.
                val content = block.get("a key").get();
                this.title = (content as String) + " exists";
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
            }
        )
    }
}