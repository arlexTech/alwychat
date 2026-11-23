package com.alwy.chat

import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * One chat inside a notification bubble (see [Bubbles]). Android starts it in the bubble's
 * window; it gets its own webview, which shows only that chat (`bubbles.rs`, the UI's bubble
 * mode). MainActivity can't be used: bubbles can't embed a singleTask activity.
 */
class BubbleActivity : TauriActivity() {
  companion object {
    const val ACTION = "com.alwy.chat.BUBBLE"
    const val EXTRA_ROOM = "room_id"

    /** The chat of the bubble open on screen, if any (its messages then come without sound). */
    @Volatile var onScreen: String? = null

    fun intent(context: Context, roomId: String): Intent =
      Intent(context, BubbleActivity::class.java).setAction(ACTION).putExtra(EXTRA_ROOM, roomId)
  }

  private var webView: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    Crashes.watch(this)
    super.onCreate(savedInstanceState)
    window.decorView.setBackgroundColor(Color.parseColor("#0e1014"))
    // The bubble's window is resized for the keyboard by Android; keep clear of anything else.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
    // Back closes what's open in the chat (viewer, sheet, menu), then folds the bubble.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val view = webView ?: return collapse()
        view.evaluateJavascript("window.__alwyBack ? window.__alwyBack() : false") { handled ->
          if (handled != "true") collapse()
        }
      }
    })
    val room = intent.getStringExtra(EXTRA_ROOM)
    if (room.isNullOrEmpty()) {
      finish()
      return
    }
    NativeBubble.window("BubbleActivity", id, room)
  }

  override fun onResume() {
    super.onResume()
    Front.resumed(this)
    onScreen = intent.getStringExtra(EXTRA_ROOM)
  }

  override fun onPause() {
    if (onScreen == intent.getStringExtra(EXTRA_ROOM)) onScreen = null
    super.onPause()
  }

  private fun collapse() = MinimizeBubbleActivity.start(this)
}
