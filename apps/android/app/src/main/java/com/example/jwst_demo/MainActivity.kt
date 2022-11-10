package com.example.jwst_demo

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import com.toeverything.jwst.Workspace

class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        val workspace = Workspace("asd");
        workspace.withTrx { trx ->
            val block = trx.create("a", "b");
            block.set(trx, "a key", "a value");
        };
        val block = workspace.get("a").get();
        val content = block.get("a key").get();
        this.title = content as String;

    }
}