package com.alwy.chat

import android.Manifest
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
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.PermissionState
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
class FileArgs {
  /** File written by the Rust core in the app's cache. */
  lateinit var path: String
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
@InvokeArg
class CaptureArgs {
  /** "photo" or "video". */
  var kind: String = "photo"
  /** Where the camera app writes the file (the Rust side sends it from there, then deletes it). */
  lateinit var dir: String
}

@TauriPlugin(
  // The manifest declares CAMERA (video calls), so Android requires it for camera intents too.
  permissions = [Permission(strings = [Manifest.permission.CAMERA], alias = "camera")],
)
class GalleryPlugin(private val activity: Activity) : Plugin(activity) {
  private var pickDir: File? = null

  private var captureFile: File? = null
  private var captureMime = "image/jpeg"

  /** Take a photo or record a video with the phone's camera app; resolves like `pick`. */
  @Command
  fun capture(invoke: Invoke) {
    if (getPermissionState("camera") != PermissionState.GRANTED) {
      requestPermissionForAlias("camera", invoke, "cameraAnswered")
      return
    }
    startCapture(invoke)
  }

  @PermissionCallback
  private fun cameraAnswered(invoke: Invoke) {
    if (getPermissionState("camera") == PermissionState.GRANTED) startCapture(invoke)
    else invoke.reject("AlwyChat isn't allowed to use the camera")
  }

  private fun startCapture(invoke: Invoke) {
    val args = invoke.parseArgs(CaptureArgs::class.java)
    val video = args.kind == "video"
    val dir = File(args.dir).apply { mkdirs() }
    val stamp = java.text.SimpleDateFormat("yyyyMMdd-HHmmss", java.util.Locale.ROOT).format(java.util.Date())
    val file = File(dir, if (video) "VID_$stamp.mp4" else "IMG_$stamp.jpg")
    captureFile = file
    captureMime = if (video) "video/mp4" else "image/jpeg"
    val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
    val intent = Intent(if (video) MediaStore.ACTION_VIDEO_CAPTURE else MediaStore.ACTION_IMAGE_CAPTURE)
      .putExtra(MediaStore.EXTRA_OUTPUT, uri)
      .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION)
    try {
      startActivityForResult(invoke, intent, "captured")
    } catch (e: Exception) {
      file.delete()
      invoke.reject("no camera app found")
    }
  }

  @ActivityCallback
  fun captured(invoke: Invoke, result: ActivityResult) {
    val file = captureFile
    captureFile = null
    if (result.resultCode != Activity.RESULT_OK || file == null || !file.exists() || file.length() == 0L) {
      file?.delete()
      invoke.resolve(JSObject().put("files", JSArray()))
      return
    }
    val picked = JSObject()
      .put("path", file.absolutePath)
      .put("name", file.name)
      .put("mime", captureMime)
      .put("size", file.length())
    invoke.resolve(JSObject().put("files", JSArray().put(picked)))
  }

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

  /** Images, GIFs and stickers the keyboard inserted (KeyboardContent), once. */
  @Command
  fun takeKeyboard(invoke: Invoke) {
    val files = JSArray()
    KeyboardContent.take().forEach { files.put(it) }
    invoke.resolve(JSObject().put("files", files))
  }

  /**
   * What another app shared with us (text and/or files), once: the files are copied to `dir`
   * like picked ones, so the UI previews and sends them the same way. Empty when nothing waits.
   */
  @Command
  fun takeShare(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    val intent = MainActivity.share
    MainActivity.share = null
    if (intent == null) {
      invoke.resolve(JSObject().put("files", JSArray()))
      return
    }
    val uris = mutableListOf<Uri>()
    if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
      @Suppress("DEPRECATION")
      intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.let { uris.addAll(it) }
    } else {
      @Suppress("DEPRECATION")
      (intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))?.let { uris.add(it) }
    }
    // Only files another app offers through its content provider: a file:// link could point
    // into AlwyChat's own private folder (its session) and get it attached to a message.
    uris.retainAll { it.scheme == "content" && it.authority != "${activity.packageName}.fileprovider" }
    val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
    val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)
    Thread {
      try {
        val dir = File(args.path).apply { mkdirs() }
        val files = JSArray()
        uris.forEachIndexed { i, uri -> files.put(copyPicked(uri, dir, i)) }
        val result = JSObject().put("files", files)
        // Some apps put the page title in the subject and only the link in the text.
        val body = listOfNotNull(subject?.takeIf { s -> text?.contains(s) != true }, text)
          .filter { it.isNotBlank() }
          .joinToString("\n")
        if (body.isNotEmpty()) result.put("text", body)
        invoke.resolve(result)
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      }
    }.start()
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

  /** Opens a downloaded file in another app (PDF viewer, text editor…), with a chooser. */
  @Command
  fun open(invoke: Invoke) {
    val file = File(invoke.parseArgs(FileArgs::class.java).path)
    val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
    val view = Intent(Intent.ACTION_VIEW)
      .setDataAndType(uri, mimeOf(file))
      .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    try {
      activity.startActivity(Intent.createChooser(view, file.name))
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("no app can open this file")
    }
  }

  private var saveAsSource: File? = null

  /** "Save to…": the system's document picker chooses the folder and name. */
  @Command
  fun saveAs(invoke: Invoke) {
    val file = File(invoke.parseArgs(FileArgs::class.java).path)
    saveAsSource = file
    val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
      addCategory(Intent.CATEGORY_OPENABLE)
      type = mimeOf(file)
      putExtra(Intent.EXTRA_TITLE, file.name)
    }
    startActivityForResult(invoke, intent, "savedAs")
  }

  @ActivityCallback
  fun savedAs(invoke: Invoke, result: ActivityResult) {
    val source = saveAsSource
    val target = result.data?.data
    if (result.resultCode != Activity.RESULT_OK || target == null || source == null) {
      source?.delete()
      invoke.resolve(JSObject().put("saved", false))
      return
    }
    Thread {
      try {
        activity.contentResolver.openOutputStream(target)!!.use { out -> source.inputStream().use { it.copyTo(out) } }
        invoke.resolve(JSObject().put("saved", true))
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      } finally {
        source.delete()
      }
    }.start()
  }

  private fun mimeOf(file: File) =
    MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "application/octet-stream"
}
