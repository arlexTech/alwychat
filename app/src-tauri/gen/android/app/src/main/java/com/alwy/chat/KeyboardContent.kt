package com.alwy.chat

import android.os.Build
import android.provider.OpenableColumns
import android.webkit.WebView
import androidx.core.view.inputmethod.InputConnectionCompat
import androidx.core.view.inputmethod.InputContentInfoCompat
import app.tauri.plugin.JSObject
import java.io.File

/**
 * Images, GIFs and stickers the keyboard inserts (Gboard's clipboard, GIF and sticker panels).
 * The WebView accepts them (see .cargo/config.toml, which extends wry's RustWebView); each one
 * is copied right away to the picked-files folder (the keyboard's read permission is temporary),
 * then the page is told, and the open chat takes it like a picked file (GalleryPlugin.takeKeyboard).
 */
object KeyboardContent {
  private val pending = mutableListOf<JSObject>()

  fun receive(view: WebView, info: InputContentInfoCompat, flags: Int): Boolean {
    if (Build.VERSION.SDK_INT >= 25 && flags and InputConnectionCompat.INPUT_CONTENT_GRANT_READ_URI_PERMISSION != 0) {
      try {
        info.requestPermission()
      } catch (e: Exception) {
        return false
      }
    }
    val context = view.context
    val mime = if (info.description.mimeTypeCount > 0) info.description.getMimeType(0) else "image/png"
    Thread {
      try {
        val resolver = context.contentResolver
        var name = "keyboard.${mime.substringAfter('/').substringBefore('+').ifEmpty { "png" }}"
        resolver.query(info.contentUri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
          if (c.moveToFirst() && !c.isNull(0)) name = c.getString(0)
        }
        val dir = File(context.cacheDir, "picked").apply { mkdirs() }
        val target = File(dir, "${System.nanoTime()}-kb-${name.replace('/', '_')}")
        resolver.openInputStream(info.contentUri)!!.use { input -> target.outputStream().use { input.copyTo(it) } }
        synchronized(pending) {
          pending.add(
            JSObject().put("path", target.absolutePath).put("name", name).put("mime", mime).put("size", target.length())
          )
        }
        view.post { view.evaluateJavascript("window.dispatchEvent(new Event('alwychat-keyboard-content'))", null) }
      } catch (e: Exception) {
        android.util.Log.w("AlwyChat", "keyboard content: $e")
      } finally {
        info.releasePermission()
      }
    }.start()
    return true
  }

  /** What arrived since the last call. */
  fun take(): List<JSObject> = synchronized(pending) { pending.toList().also { pending.clear() } }
}
