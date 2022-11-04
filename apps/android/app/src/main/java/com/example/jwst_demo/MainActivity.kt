package com.example.jwst_demo

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import com.example.jwst_demo.lib.Workspace

class MainActivity : AppCompatActivity() {
    companion object {
        init {
            System.loadLibrary("jwst")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
    }
}