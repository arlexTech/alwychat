package com.alwy.chat

import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  companion object {
    /** On screen: pushes don't notify then (the app shows the message itself). */
    @Volatile var visible = false
  }

  override fun onResume() {
    super.onResume()
    visible = true
  }

  override fun onPause() {
    visible = false
    super.onPause()
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    // The UI is dark: light status/navigation bar icons on a transparent bar.
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)
    // Same colour as the app background (--bg in app.css), seen behind the system bars.
    window.decorView.setBackgroundColor(Color.parseColor("#0e1014"))

    // Keep the web UI out from under the status bar, gesture bar, camera cutout and keyboard
    // (edge-to-edge windows aren't resized for the keyboard on their own).
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
