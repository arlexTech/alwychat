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
    /** A live location share is running: the page keeps following the position. */
    @Volatile var sharingLocation = false
    /** The mic state for the picture-in-picture button (null: no button, push to talk). */
    @Volatile var muted: Boolean? = null
    @Volatile var muteLabel = "Mute"
    @Volatile var unmuteLabel = "Unmute"
    @Volatile var hangUpLabel = "Hang up"
    /** The picture-in-picture window is wide (someone's screen) instead of tall (cameras). */
    @Volatile var pipWide = false
    /** Going home during a call shrinks it into picture-in-picture (a setting, off by default:
     * the camera keeps running in the background without it). */
    @Volatile var pipEnabled = false
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
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) builder.setAutoEnterEnabled(inCall && pipEnabled).setSeamlessResizeEnabled(false)
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
    if (inCall && pipEnabled && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
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
    forCall(intent)
    keepShare(intent)
    // Already on screen (a floating window, split screen): the page never becomes "visible"
    // again, so tell it a share is waiting.
    if (share != null) webView(window.decorView)?.let { view ->
      view.post { view.evaluateJavascript("window.dispatchEvent(new Event('alwychat-share'))", null) }
    }
  }

  // Out of sight in a call, Android stops our camera: the page pauses it (the others then see
  // our photo, not a frozen picture) and starts it again here. A picture-in-picture window
  // keeps the activity started; closing that window stops it (still in PiP mode then).
  private var hiddenInCall = false

  override fun onStop() {
    super.onStop()
    if (inCall) {
      hiddenInCall = true
      try { NativeCall.action("app_hidden") } catch (_: Throwable) {}
    }
  }

  override fun onStart() {
    super.onStart()
    // Back on screen: the WebView's own camera takes over again.
    BackgroundCamera.stop()
    if (hiddenInCall) {
      hiddenInCall = false
      try { NativeCall.action("app_shown") } catch (_: Throwable) {}
    }
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
    // Same while sharing a live location (the page follows the position).
    if (inCall || sharingLocation) webView(window.decorView)?.onResume()
  }

  /** The page's WebView. */
  fun pageView(): android.webkit.WebView? = webView(window.decorView)

  /** The screen's rotation in degrees (the background camera turns its picture upright). */
  fun screenRotation(): Int {
    @Suppress("DEPRECATION")
    val r = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) display?.rotation else windowManager.defaultDisplay.rotation
    return when (r) { android.view.Surface.ROTATION_90 -> 90; android.view.Surface.ROTATION_180 -> 180; android.view.Surface.ROTATION_270 -> 270; else -> 0 }
  }

  private fun webView(view: View): android.webkit.WebView? {
    if (view is android.webkit.WebView) return view
    if (view is android.view.ViewGroup) {
      for (i in 0 until view.childCount) webView(view.getChildAt(i))?.let { return it }
    }
    return null
  }

  /** Opened by an incoming call (its full-screen notification, Answer): show over the lock
   * screen and turn the screen on, like a phone call. Only for the call: dropped again when the
   * ring ends unanswered or the call ends, so the chats never stay reachable without unlocking. */
  fun overLockScreen(on: Boolean) {
    // While it rings, the screen stays on (on the lock screen Android turns it off after ~10 s,
    // and the first tap on Answer then only woke it up). Dropped once answered or ended.
    keepScreenOn(on)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
      setShowWhenLocked(on)
      setTurnScreenOn(on)
    } else {
      @Suppress("DEPRECATION")
      val flags = android.view.WindowManager.LayoutParams.FLAG_SHOW_WHEN_LOCKED or
        android.view.WindowManager.LayoutParams.FLAG_TURN_SCREEN_ON
      if (on) window.addFlags(flags) else window.clearFlags(flags)
    }
  }

  fun keepScreenOn(on: Boolean) {
    val flag = android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
    if (on) window.addFlags(flag) else window.clearFlags(flag)
  }

  private fun forCall(intent: Intent?) {
    if (intent?.data?.path == "/call") overLockScreen(true)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    Crashes.watch(this)
    forCall(intent)
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
