package com.example.jwst_demo

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import com.toeverything.jwst.Storage
import java.io.File

class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val database = File(filesDir, "jwst.db");
        val storage = Storage(database.absolutePath);
        val workspace = storage.getWorkspace("test")

        workspace.withTrx { trx ->
            val block = trx.create("a", "b");
            block.set(trx, "a key", "a value");
        };
        val block = workspace.get("a").get();
        val content = block.get("a key").get();
        this.title = content as String;

    }
}