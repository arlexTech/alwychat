package com.alwy.chat

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.media.Image
import android.media.ImageReader
import android.net.Uri
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.util.Log
import android.util.Range
import android.util.Size
import android.view.Surface
import android.webkit.WebView
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebMessagePortCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.abs

/**
 * The call's camera while the app is out of sight. The WebView stops handing camera frames to
 * the page once none of our activities is visible (Chromium suspends capture then), so the
 * others saw a frozen picture. Here the camera is opened natively instead (the call service is a
 * camera foreground service, so Android allows it in the background) and each frame goes to the
 * page as I420 bytes over a message port: `[width int32 LE][height int32 LE][Y][U][V]`, already
 * upright. The page feeds them into the published video track until we're back on screen.
 */
object BackgroundCamera {
  private const val TAG = "AlwyChat"
  // The camera's usual call quality (720p, up to 30 fps), as in the foreground.
  private const val WIDTH = 1280
  private const val HEIGHT = 720
  private const val FRAME_MS = 30L

  private var thread: HandlerThread? = null
  private var device: CameraDevice? = null
  private var session: CameraCaptureSession? = null
  private var reader: ImageReader? = null
  private var port: WebMessagePortCompat? = null
  private var lastFrame = 0L
  /** Bumped on every start/stop: callbacks of an older camera are ignored. */
  @Volatile private var generation = 0

  fun supported(): Boolean =
    WebViewFeature.isFeatureSupported(WebViewFeature.CREATE_WEB_MESSAGE_CHANNEL) &&
      WebViewFeature.isFeatureSupported(WebViewFeature.POST_WEB_MESSAGE) &&
      WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_ARRAY_BUFFER)

  /** Opens the camera; must run on the main thread (the page gets its port from there).
   * `rotation`: the screen's rotation in degrees when the app was left. */
  @SuppressLint("MissingPermission")
  fun start(context: Context, webView: WebView, front: Boolean, rotation: Int): Boolean {
    stop()
    if (!supported()) return false
    val manager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
    val id = manager.cameraIdList.firstOrNull {
      val facing = manager.getCameraCharacteristics(it).get(CameraCharacteristics.LENS_FACING)
      facing == if (front) CameraCharacteristics.LENS_FACING_FRONT else CameraCharacteristics.LENS_FACING_BACK
    } ?: manager.cameraIdList.firstOrNull() ?: return false
    val chars = manager.getCameraCharacteristics(id)
    val sizes = chars.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
      ?.getOutputSizes(ImageFormat.YUV_420_888) ?: return false
    val size = sizes.minByOrNull { abs(it.width * it.height - WIDTH * HEIGHT) + abs(it.width * 9 - it.height * 16) * 4 }
      ?: Size(WIDTH, HEIGHT)
    val sensor = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
    val isFront = chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
    // Same rule as WebRTC's Android capturer: the picture comes out upright, not mirrored.
    val turn = ((if (isFront) sensor + rotation else sensor - rotation) % 360 + 360) % 360

    val ports = WebViewCompat.createWebMessageChannel(webView)
    WebViewCompat.postWebMessage(webView, WebMessageCompat("alwychat-camera", arrayOf(ports[1])), Uri.parse("*"))
    port = ports[0]

    val gen = ++generation
    val t = HandlerThread("background-camera").also { it.start() }
    thread = t
    val handler = Handler(t.looper)
    val r = ImageReader.newInstance(size.width, size.height, ImageFormat.YUV_420_888, 3)
    reader = r
    r.setOnImageAvailableListener({ ir ->
      val image = ir.acquireLatestImage() ?: return@setOnImageAvailableListener
      try {
        val now = System.currentTimeMillis()
        if (gen != generation || now - lastFrame < FRAME_MS) return@setOnImageAvailableListener
        lastFrame = now
        val bytes = toI420(image, turn)
        port?.postMessage(WebMessageCompat(bytes))
      } catch (e: Throwable) {
        Log.w(TAG, "background camera frame: $e")
      } finally {
        image.close()
      }
    }, handler)

    try {
      manager.openCamera(id, object : CameraDevice.StateCallback() {
        override fun onOpened(camera: CameraDevice) {
          if (gen != generation) return camera.close()
          device = camera
          @Suppress("DEPRECATION")
          camera.createCaptureSession(listOf(r.surface), object : CameraCaptureSession.StateCallback() {
            override fun onConfigured(s: CameraCaptureSession) {
              if (gen != generation) return s.close()
              session = s
              val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_RECORD).apply {
                addTarget(r.surface)
                set(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, Range(15, 30))
              }
              runCatching { s.setRepeatingRequest(request.build(), null, handler) }
                .onFailure { Log.w(TAG, "background camera request: $it") }
            }

            override fun onConfigureFailed(s: CameraCaptureSession) {
              Log.w(TAG, "background camera: session failed")
            }
          }, handler)
        }

        override fun onDisconnected(camera: CameraDevice) {
          camera.close()
        }

        override fun onError(camera: CameraDevice, error: Int) {
          Log.w(TAG, "background camera error $error")
          camera.close()
        }
      }, handler)
    } catch (e: Throwable) {
      Log.w(TAG, "background camera: $e")
      stop()
      return false
    }
    Log.i(TAG, "background camera on: ${size.width}x${size.height}, turned $turn°")
    return true
  }

  /** Closes the camera and the page's port (the page then goes back to its own camera). */
  fun stop() {
    if (thread == null && port == null) return
    generation++
    val s = session
    val d = device
    val r = reader
    val p = port
    val t = thread
    session = null
    device = null
    reader = null
    port = null
    thread = null
    runCatching { s?.close() }
    runCatching { d?.close() }
    runCatching { p?.close() }
    // The reader and thread go once the camera has stopped using them.
    Handler(Looper.getMainLooper()).postDelayed({
      runCatching { r?.close() }
      t?.quitSafely()
    }, 300)
    Log.i(TAG, "background camera off")
  }

  /** One camera image as upright I420, after the 8-byte size header. */
  private fun toI420(image: Image, turn: Int): ByteArray {
    val w = image.width
    val h = image.height
    val (ow, oh) = if (turn == 90 || turn == 270) h to w else w to h
    val out = ByteArray(8 + w * h * 3 / 2)
    ByteBuffer.wrap(out, 0, 8).order(ByteOrder.LITTLE_ENDIAN).putInt(ow).putInt(oh)
    val planes = image.planes
    copyPlane(planes[0].buffer, planes[0].rowStride, planes[0].pixelStride, w, h, turn, out, 8)
    val cw = w / 2
    val ch = h / 2
    copyPlane(planes[1].buffer, planes[1].rowStride, planes[1].pixelStride, cw, ch, turn, out, 8 + w * h)
    copyPlane(planes[2].buffer, planes[2].rowStride, planes[2].pixelStride, cw, ch, turn, out, 8 + w * h + cw * ch)
    return out
  }

  /** Copies a `w`×`h` plane into `out` at `offset`, turned clockwise by `turn` degrees. */
  private fun copyPlane(src: ByteBuffer, rowStride: Int, pixelStride: Int, w: Int, h: Int, turn: Int, out: ByteArray, offset: Int) {
    val row = ByteArray(rowStride)
    for (y in 0 until h) {
      src.position(y * rowStride)
      src.get(row, 0, minOf(rowStride, src.remaining()))
      // One loop per angle: the destination steps by a fixed amount along a source row.
      var s = 0
      when (turn) {
        90 -> { var d = offset + (h - 1 - y); for (x in 0 until w) { out[d] = row[s]; s += pixelStride; d += h } }
        180 -> { var d = offset + (h - 1 - y) * w + (w - 1); for (x in 0 until w) { out[d] = row[s]; s += pixelStride; d-- } }
        270 -> { var d = offset + (w - 1) * h + y; for (x in 0 until w) { out[d] = row[s]; s += pixelStride; d -= h } }
        else -> if (pixelStride == 1) System.arraycopy(row, 0, out, offset + y * w, w)
          else { var d = offset + y * w; for (x in 0 until w) { out[d] = row[s]; s += pixelStride; d++ } }
      }
    }
  }
}
