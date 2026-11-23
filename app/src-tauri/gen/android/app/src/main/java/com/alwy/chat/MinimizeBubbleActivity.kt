package com.alwy.chat

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log

/**
 * A see-through activity that lives a moment in its own task: taking the focus from an open
 * bubble makes Android fold it back into its chat head. A link tapped in a bubble is opened
 * once it's folded, so the browser comes to the front instead of hiding behind the bubble.
 */
class MinimizeBubbleActivity : Activity() {
  companion object {
    private const val EXTRA_URI = "uri"

    fun start(context: Context, uri: String? = null) {
      context.startActivity(
        Intent(context, MinimizeBubbleActivity::class.java)
          .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
          .apply { if (uri != null) putExtra(EXTRA_URI, uri) },
      )
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    val uri = intent.getStringExtra(EXTRA_URI)
    // A short wait so Android notices the focus moved before we go.
    Handler(Looper.getMainLooper()).postDelayed({
      if (uri != null) {
        try {
          startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(uri)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (e: Exception) {
          Log.w("AlwyChat", "bubble link: $e")
        }
      }
      finish()
      @Suppress("DEPRECATION")
      overridePendingTransition(0, 0)
    }, 100)
  }
}
