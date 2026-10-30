package com.alwy.chat

import android.app.Activity
import android.content.ContentValues
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class SaveArgs {
  /** File written by the Rust core in the app's cache; deleted once copied. */
  lateinit var path: String
  /** Photos/videos go to the gallery, anything else (or false) to Downloads. */
  var toGallery: Boolean = true
}

/**
 * Copies a file into shared storage through MediaStore, so it shows up in the Gallery
 * (Pictures/AlwyChat, Movies/AlwyChat) or in Download/AlwyChat. No permission needed (API 29+).
 */
@TauriPlugin
class GalleryPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun save(invoke: Invoke) {
    val args = invoke.parseArgs(SaveArgs::class.java)
    // Off the main thread: videos can be large.
    Thread {
      val source = File(args.path)
      try {
        invoke.resolve(JSObject().put("path", copyToMediaStore(source, args.toGallery)))
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      } finally {
        source.delete()
      }
    }.start()
  }

  private fun copyToMediaStore(source: File, toGallery: Boolean): String {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
      throw UnsupportedOperationException("saving needs Android 10 or newer")
    }
    val mime = MimeTypeMap.getSingleton()
      .getMimeTypeFromExtension(source.extension.lowercase())
      ?: "application/octet-stream"
    val (collection, dir) = when {
      toGallery && mime.startsWith("image/") ->
        MediaStore.Images.Media.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_PICTURES
      toGallery && mime.startsWith("video/") ->
        MediaStore.Video.Media.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_MOVIES
      else -> MediaStore.Downloads.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_DOWNLOADS
    }
    val folder = "$dir/AlwyChat"
    val resolver = activity.contentResolver
    val values = ContentValues().apply {
      put(MediaStore.MediaColumns.DISPLAY_NAME, source.name)
      put(MediaStore.MediaColumns.MIME_TYPE, mime)
      put(MediaStore.MediaColumns.RELATIVE_PATH, folder)
      put(MediaStore.MediaColumns.IS_PENDING, 1)
    }
    val uri = resolver.insert(collection, values) ?: throw IllegalStateException("MediaStore refused the file")
    try {
      resolver.openOutputStream(uri)!!.use { out -> source.inputStream().use { it.copyTo(out) } }
      values.clear()
      values.put(MediaStore.MediaColumns.IS_PENDING, 0)
      resolver.update(uri, values, null, null)
    } catch (e: Exception) {
      resolver.delete(uri, null, null)
      throw e
    }
    return "$folder/${source.name}"
  }
}
