package com.alwy.chat

import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.Icon
import android.os.Build
import android.os.Bundle
import android.util.Rational
import android.view.View
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  companion object {
    /** On screen: pushes don't notify then (the app shows the message itself). */
    @Volatile var visible = false

    /** The last "Share" sent to us from another app, until the UI takes it (GalleryPlugin.takeShare). */
    @Volatile var share: Intent? = null

    /** In a call (set with the call notification): leaving the app shrinks it to picture-in-picture. */
    @Volatile var inCall = false
    /** The mic state for the picture-in-picture button (null: no button, push to talk). */
    @Volatile var muted: Boolean? = null
    @Volatile var muteLabel = "Mute"
    @Volatile var unmuteLabel = "Unmute"
    @Volatile var hangUpLabel = "Hang up"
    /** The picture-in-picture window is wide (someone's screen) instead of tall (cameras). */
    @Volatile var pipWide = false
  }

  /** The picture-in-picture window of a call: tall (wide for a screen), with mute and hang-up buttons. */
  private fun pipParams(): PictureInPictureParams? {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
    val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    fun action(icon: Int, label: String, what: String, code: Int) = RemoteAction(
      Icon.createWithResource(this, icon), label, label,
      PendingIntent.getService(this, code, Intent(this, CallService::class.java).setAction(what), flags),
    )
    val actions = mutableListOf<RemoteAction>()
    muted?.let { m ->
      actions += action(
        if (m) R.drawable.ic_call_mic_off else R.drawable.ic_call_mic,
        if (m) unmuteLabel else muteLabel, CallService.TOGGLE_MIC, 12,
      )
    }
    actions += action(R.drawable.ic_call_phone_off, hangUpLabel, CallService.HANG_UP, 11)
    val builder = PictureInPictureParams.Builder()
      .setAspectRatio(if (pipWide) Rational(16, 9) else Rational(3, 4))
      .setActions(actions)
    // Android 12+: entered smoothly by the home gesture itself.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) builder.setAutoEnterEnabled(inCall).setSeamlessResizeEnabled(false)
    return builder.build()
  }

  /** Called when the call starts, ends or its mic changes. */
  fun updatePip() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
    runOnUiThread { pipParams()?.let { runCatching { setPictureInPictureParams(it) } } }
  }

  // An app, not a web page: no pinch zoom (the photo viewer zooms by itself).
  override fun onWebViewCreate(webView: android.webkit.WebView) {
    super.onWebViewCreate(webView)
    webView.settings.setSupportZoom(false)
    webView.settings.builtInZoomControls = false
    webView.settings.displayZoomControls = false
  }

  override fun onUserLeaveHint() {
    super.onUserLeaveHint()
    // Before Android 12 (no auto-enter): going home during a call shrinks the app.
    if (inCall && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
      pipParams()?.let { runCatching { enterPictureInPictureMode(it) } }
    }
  }

  override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
    super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
    // The UI shows only the call while it's small.
    NativeCall.action(if (isInPictureInPictureMode) "pip_on" else "pip_off")
  }

  private fun keepShare(intent: Intent?) {
    if (intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE) share = intent
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    keepShare(intent)
  }

  override fun onResume() {
    super.onResume()
    visible = true
    // On screen, the app shows incoming calls itself.
    IncomingCall.cancelAll(this)
  }

  override fun onPause() {
    visible = false
    super.onPause()
    // The generated activity pauses the WebView here, which hides the page: in a call (picture-
    // in-picture or in the background) it must keep drawing and running the call.
    if (inCall) webView(window.decorView)?.onResume()
  }

  private fun webView(view: View): android.webkit.WebView? {
    if (view is android.webkit.WebView) return view
    if (view is android.view.ViewGroup) {
      for (i in 0 until view.childCount) webView(view.getChildAt(i))?.let { return it }
    }
    return null
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    Crashes.watch(this)
    // The UI is dark: light status/navigation bar icons on a transparent bar.
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)
    keepShare(intent)
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
