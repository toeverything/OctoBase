package com.example.jwst_demo

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import com.toeverything.jwst.Workspace

class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        var workspace = Workspace("asd");

    }
}