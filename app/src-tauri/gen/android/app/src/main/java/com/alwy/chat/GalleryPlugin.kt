package com.alwy.chat

import android.app.Activity
import android.content.ContentValues
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import androidx.activity.result.ActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
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

@InvokeArg
class PickArgs {
  /** "media": the system photo picker (photos + videos); "files": any document. */
  var kind: String = "media"
  /** Where to copy the picked files (the Rust side sends them from there, then deletes them). */
  lateinit var dir: String
}

/**
 * Photo/file picking and saving to shared storage.
 *
 * Saving copies a file into shared storage through MediaStore, so it shows up in the Gallery
 * (Pictures/AlwyChat, Movies/AlwyChat) or in Download/AlwyChat. No permission needed (API 29+).
 */
@TauriPlugin
class GalleryPlugin(private val activity: Activity) : Plugin(activity) {
  private var pickDir: File? = null

  /** Opens the system picker (several items at once); no storage permission needed. */
  @Command
  fun pick(invoke: Invoke) {
    val args = invoke.parseArgs(PickArgs::class.java)
    pickDir = File(args.dir)
    val intent = if (args.kind == "media") {
      ActivityResultContracts.PickMultipleVisualMedia().createIntent(
        activity,
        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo),
      )
    } else {
      Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "*/*"
        putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
      }
    }
    startActivityForResult(invoke, intent, "picked")
  }

  @ActivityCallback
  fun picked(invoke: Invoke, result: ActivityResult) {
    val data = result.data
    val dir = pickDir
    if (result.resultCode != Activity.RESULT_OK || data == null || dir == null) {
      invoke.resolve(JSObject().put("files", JSArray()))
      return
    }
    val uris = mutableListOf<Uri>()
    val clip = data.clipData
    if (clip != null) {
      for (i in 0 until clip.itemCount) uris.add(clip.getItemAt(i).uri)
    } else {
      data.data?.let { uris.add(it) }
    }
    // Copying can take a while (videos): off the main thread.
    Thread {
      try {
        dir.mkdirs()
        val files = JSArray()
        uris.forEachIndexed { i, uri -> files.put(copyPicked(uri, dir, i)) }
        invoke.resolve(JSObject().put("files", files))
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      }
    }.start()
  }

  private fun copyPicked(uri: Uri, dir: File, index: Int): JSObject {
    val resolver = activity.contentResolver
    var name = "file-$index"
    resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
      if (c.moveToFirst() && !c.isNull(0)) name = c.getString(0)
    }
    val mime = resolver.getType(uri) ?: "application/octet-stream"
    // Unique on disk, original name kept for the message.
    val target = File(dir, "${System.nanoTime()}-$index-${name.replace('/', '_')}")
    resolver.openInputStream(uri)!!.use { input -> target.outputStream().use { input.copyTo(it) } }
    return JSObject()
      .put("path", target.absolutePath)
      .put("name", name)
      .put("mime", mime)
      .put("size", target.length())
  }

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
