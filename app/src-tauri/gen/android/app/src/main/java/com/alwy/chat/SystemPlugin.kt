package com.alwy.chat

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.net.ConnectivityManager
import android.net.Uri
import android.app.NotificationManager
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.firebase.messaging.FirebaseMessaging

@InvokeArg
class LocationArgs {
  var active: Boolean = false
  var title: String = ""
  var text: String = ""
  var stop: String = ""
  var channel: String = ""
  var roomId: String? = null
  /** When the (last) share ends, ms since epoch; 0 = until stopped. */
  var endsAt: Long = 0
}

@InvokeArg
class CallArgs {
  var active: Boolean = false
  var title: String = ""
  var text: String = ""
  var hangUp: String = ""
  var channel: String = ""
  var roomId: String? = null
  /** null: no mute button (push to talk). */
  var muted: Boolean? = null
  var mute: String = ""
  var unmute: String = ""
  /** The camera is on: the service may use it outside the foreground too. */
  var camera: Boolean = false
}

@InvokeArg
class FullScreenArgs {
  var on: Boolean = false
}

@InvokeArg
class PipShapeArgs {
  var wide: Boolean = false
}

@InvokeArg
class BackgroundCameraArgs {
  var on: Boolean = false
  var front: Boolean = true
}

@InvokeArg
class PipAllowedArgs {
  var on: Boolean = false
}

@InvokeArg
class RingArgs {
  var on: Boolean = false
}

@InvokeArg
class BrowserArgs {
  lateinit var url: String
}

@InvokeArg
class AudioArgs {
  /** "speaker", "earpiece", "headset" (the best connected one), "device" (`device` below),
   * "release" (no call device: a Bluetooth headset goes back to its music profile while our
   * mic is off), or null: the call ended (back to normal audio). */
  var route: String? = null
  /** An AudioDeviceInfo id from `audioRoutes`, for route "device". */
  var device: Int? = null
}

@InvokeArg
class InstallArgs {
  lateinit var path: String
}

@InvokeArg
class SecureArgs {
  var secure = false
}

@InvokeArg
class BarsArgs {
  var light = false
  lateinit var background: String
}

@InvokeArg
class DistributorArgs {
  /** A distributor's package, or null to stop using UnifiedPush. */
  var distributor: String? = null
}

@InvokeArg
class ClearArgs {
  lateinit var roomId: String
}

@InvokeArg
class LabelsArgs {
  var labels: Map<String, String> = emptyMap()
}

/**
 * What stops AlwyChat from notifying: the notification permission, battery optimisation
 * (the system freezes the app in the background) and restricted background data.
 * Each `fix*` command asks the system, or opens the right settings page when it can't.
 */
@TauriPlugin(
  permissions = [
    Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications"),
    Permission(strings = [Manifest.permission.BLUETOOTH_CONNECT], alias = "bluetooth"),
  ],
)
class SystemPlugin(private val activity: Activity) : Plugin(activity) {
  private val pkg get() = activity.packageName

  @Command
  fun status(invoke: Invoke) {
    invoke.resolve(currentStatus())
  }

  private fun currentStatus(): JSObject {
    val power = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
    val net = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    return JSObject().apply {
      put("notifications", NotificationManagerCompat.from(activity).areNotificationsEnabled())
      put("lastPush", PushService.lastReceived(activity))
      put("batteryUnrestricted", runCatching { power.isIgnoringBatteryOptimizations(pkg) }.getOrDefault(true))
      // Unknown counts as fine: a failed check shouldn't nag the user.
      put(
        "backgroundData",
        runCatching { net.restrictBackgroundStatus != ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED }
          .getOrDefault(true),
      )
      // Android 14+: incoming calls take over the locked screen only if the user allows it.
      put(
        "fullScreenCalls",
        if (Build.VERSION.SDK_INT >= 34)
          runCatching { activity.getSystemService(NotificationManager::class.java).canUseFullScreenIntent() }.getOrDefault(true)
        else true,
      )
    }
  }

  /** The system prompt if Android still allows asking, else the app's notification settings. */
  /**
   * Hand a downloaded update (an APK in the app's cache) to Android's installer, which only
   * accepts it if it's signed like the installed app. The first time, Android needs the user to
   * allow AlwyChat to install apps: that settings page opens and `needsPermission` is returned.
   */
  @Command
  fun installApk(invoke: Invoke) {
    val args = invoke.parseArgs(InstallArgs::class.java)
    val ret = JSObject()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !activity.packageManager.canRequestPackageInstalls()) {
      activity.startActivity(
        Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:$pkg"))
      )
      ret.put("needsPermission", true)
      invoke.resolve(ret)
      return
    }
    val uri = androidx.core.content.FileProvider.getUriForFile(
      activity, "$pkg.fileprovider", java.io.File(args.path)
    )
    val intent = Intent(Intent.ACTION_VIEW)
      .setDataAndType(uri, "application/vnd.android.package-archive")
      .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    activity.startActivity(intent)
    ret.put("needsPermission", false)
    invoke.resolve(ret)
  }

  /** Whether going home during a call opens picture-in-picture (Settings → Voice & video). */
  @Command
  fun pipAllowed(invoke: Invoke) {
    val args = invoke.parseArgs(PipAllowedArgs::class.java)
    if (MainActivity.pipEnabled != args.on) {
      MainActivity.pipEnabled = args.on
      (activity as? MainActivity)?.updatePip()
    }
    invoke.resolve()
  }

  /** The camera of a call while the app is out of sight (see BackgroundCamera): answers
   * `{ok}`; not ok = the page pauses the camera instead. */
  @Command
  fun backgroundCamera(invoke: Invoke) {
    val args = invoke.parseArgs(BackgroundCameraArgs::class.java)
    activity.runOnUiThread {
      val main = activity as? MainActivity
      val ok = if (!args.on) {
        BackgroundCamera.stop()
        true
      } else {
        val view = main?.pageView()
        view != null && runCatching {
          BackgroundCamera.start(activity, view, args.front, main.screenRotation())
        }.onFailure { Log.w("AlwyChat", "background camera: $it") }.getOrDefault(false)
      }
      invoke.resolve(JSObject().put("ok", ok))
    }
  }

  /** The picture-in-picture window's shape: wide for a screen share, else tall. */
  @Command
  fun pipShape(invoke: Invoke) {
    val args = invoke.parseArgs(PipShapeArgs::class.java)
    if (MainActivity.pipWide != args.wide) {
      MainActivity.pipWide = args.wide
      (activity as? MainActivity)?.updatePip()
    }
    invoke.resolve()
  }

  /** The call's big picture in real full screen: system bars hidden (a swipe shows them for a
   *  moment) and the picture turning with the phone, even with auto-rotate off (like video
   *  apps); off puts both back. */
  @Command
  fun fullScreen(invoke: Invoke) {
    val args = invoke.parseArgs(FullScreenArgs::class.java)
    activity.runOnUiThread {
      val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
      if (args.on) {
        controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR
      } else {
        controller.show(WindowInsetsCompat.Type.systemBars())
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
      }
    }
    invoke.resolve()
  }

  /** Show (or remove) the ongoing call notification, which keeps the call going in the background. */
  @Command
  fun callNotification(invoke: Invoke) {
    val args = invoke.parseArgs(CallArgs::class.java)
    MainActivity.inCall = args.active
    MainActivity.muted = if (args.active) args.muted else null
    if (args.mute.isNotEmpty()) MainActivity.muteLabel = args.mute
    if (args.unmute.isNotEmpty()) MainActivity.unmuteLabel = args.unmute
    if (args.hangUp.isNotEmpty()) MainActivity.hangUpLabel = args.hangUp
    (activity as? MainActivity)?.updatePip()
    if (!args.active) {
      BackgroundCamera.stop()
      (activity as? MainActivity)?.overLockScreen(false)
    }
    // Answered: the call's own screen rules take over (the proximity sensor on the earpiece).
    else (activity as? MainActivity)?.keepScreenOn(false)
    if (args.active) {
      CallService.start(activity, args.title, args.text, args.hangUp, args.channel, args.roomId, args.muted, args.mute, args.unmute, args.camera)
    } else {
      CallService.stop(activity)
      // The call ended in picture-in-picture: close that small window too.
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && activity.isInPictureInPictureMode) {
        activity.runOnUiThread { activity.moveTaskToBack(false) }
      }
    }
    invoke.resolve()
  }

  /** The live location notification (and its foreground service) while sharing. */
  @Command
  fun locationNotification(invoke: Invoke) {
    val args = invoke.parseArgs(LocationArgs::class.java)
    MainActivity.sharingLocation = args.active
    if (args.active) {
      LocationService.start(activity, args.title, args.text, args.stop, args.channel, args.roomId, args.endsAt)
    } else {
      LocationService.stop(activity)
    }
    invoke.resolve()
  }

  /**
   * The sign-in page in the browser, started from our activity (the opener plugin starts it
   * from the application context in a new task, which a Samsung phone never showed). No
   * browser set as default: the system's "Open with" list instead.
   */
  @Command
  fun openBrowser(invoke: Invoke) {
    val args = invoke.parseArgs(BrowserArgs::class.java)
    val view = Intent(Intent.ACTION_VIEW, Uri.parse(args.url)).addCategory(Intent.CATEGORY_BROWSABLE)
    try {
      activity.startActivity(view)
    } catch (e: Exception) {
      android.util.Log.w("AlwyChat", "no app opened the sign-in page: $e")
      try {
        activity.startActivity(Intent.createChooser(view, null))
      } catch (e2: Exception) {
        android.util.Log.w("AlwyChat", "no chooser either: $e2")
        invoke.reject("No browser could open the page: ${e2.message}")
        return
      }
    }
    invoke.resolve()
  }

  /** The app on screen rings for an incoming call (see IncomingCall.ringInApp). */
  @Command
  fun ringInApp(invoke: Invoke) {
    val args = invoke.parseArgs(RingArgs::class.java)
    IncomingCall.ringInApp(activity, args.on)
    // The ring ended: unless it was answered (the call starts within a moment), the app no
    // longer shows over the lock screen.
    if (!args.on) android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
      if (!MainActivity.inCall) (activity as? MainActivity)?.overLockScreen(false)
    }, 4000)
    invoke.resolve()
  }

  /** Tells the page (call-action "audio_routes") when headsets come and go or Android moves
   * the call's audio, while a call has the audio. */
  private var deviceCallback: android.media.AudioDeviceCallback? = null
  private var routeListener: android.media.AudioManager.OnCommunicationDeviceChangedListener? = null

  private fun kindOf(type: Int): String? = when (type) {
    android.media.AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "earpiece"
    android.media.AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "speaker"
    android.media.AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
    android.media.AudioDeviceInfo.TYPE_BLE_HEADSET,
    android.media.AudioDeviceInfo.TYPE_BLE_SPEAKER -> "bluetooth"
    android.media.AudioDeviceInfo.TYPE_WIRED_HEADSET,
    android.media.AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> "wired"
    android.media.AudioDeviceInfo.TYPE_USB_HEADSET,
    android.media.AudioDeviceInfo.TYPE_USB_DEVICE -> "usb"
    else -> null
  }

  /** A headset's name. Android often gives the PHONE's model as a Bluetooth device's product
   * name; the real name needs BLUETOOTH_CONNECT ("Show headset names" asks it).
   * "" = the page shows a generic name. */
  @SuppressLint("MissingPermission")
  private fun deviceName(d: android.media.AudioDeviceInfo, kind: String): String {
    val product = d.productName?.toString()?.trim().orEmpty()
    if (kind != "bluetooth") return if (kind == "wired" || kind == "usb") product.takeIf { it != Build.MODEL }.orEmpty() else ""
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
      activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == android.content.pm.PackageManager.PERMISSION_GRANTED
    ) {
      try {
        val bt = activity.getSystemService(android.bluetooth.BluetoothManager::class.java)?.adapter
        val byAddress = d.address.takeIf { it.isNotBlank() && android.bluetooth.BluetoothAdapter.checkBluetoothAddress(it) }
          ?.let { bt?.getRemoteDevice(it) }
          ?.let { it.alias ?: it.name }
        // Android may give a Bluetooth call device no address, or one that names nothing (seen on
        // OxygenOS 16): then the headset connected for calls. Often more than one (a watch counts
        // as a headset too): the one carrying the call's sound, else the only audio device.
        val real = byAddress?.takeIf { it.isNotBlank() } ?: headsetProxy?.let { hs ->
          val all = hs.connectedDevices
          (all.firstOrNull { hs.isAudioConnected(it) }
            ?: all.filter { it.bluetoothClass?.majorDeviceClass == android.bluetooth.BluetoothClass.Device.Major.AUDIO_VIDEO }.singleOrNull()
            ?: all.singleOrNull())?.let { it.alias ?: it.name }
        }
        if (!real.isNullOrBlank()) return real
      } catch (e: Exception) {
        android.util.Log.w("AlwyChat", "headset name failed", e)
      }
    }
    return if (product == Build.MODEL || product == Build.DEVICE) "" else product
  }

  /** Android's Bluetooth headset profile (which headsets are connected for calls), for names;
   * opened once "Nearby devices" is allowed. The page is told again once it's there. */
  private var headsetProxy: android.bluetooth.BluetoothHeadset? = null
  private var headsetAsked = false

  private fun openHeadsetProxy() {
    if (headsetAsked || !bluetoothGranted()) return
    val bt = activity.getSystemService(android.bluetooth.BluetoothManager::class.java)?.adapter ?: return
    headsetAsked = true
    bt.getProfileProxy(activity, object : android.bluetooth.BluetoothProfile.ServiceListener {
      override fun onServiceConnected(profile: Int, proxy: android.bluetooth.BluetoothProfile) {
        headsetProxy = proxy as? android.bluetooth.BluetoothHeadset
        try { NativeCall.action("audio_routes") } catch (_: Throwable) {}
      }
      override fun onServiceDisconnected(profile: Int) {
        headsetProxy = null
        headsetAsked = false
      }
    }, android.bluetooth.BluetoothProfile.HEADSET)
  }

  /** {devices: [{id, kind, name}], current: id | null} (Android 12+; older: empty). */
  private fun routes(am: android.media.AudioManager): JSObject {
    openHeadsetProxy()
    val out = JSObject()
    val list = app.tauri.plugin.JSArray()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      val seen = mutableSetOf<String>()
      for (d in am.availableCommunicationDevices) {
        val kind = kindOf(d.type) ?: continue
        // A headset can be listed once per profile (LE + classic): keep one per name and kind.
        val name = deviceName(d, kind)
        if (!seen.add("$kind/$name")) continue
        list.put(JSObject().apply {
          put("id", d.id)
          put("kind", kind)
          put("name", name)
        })
      }
      am.communicationDevice?.let { out.put("current", it.id) }
    }
    out.put("devices", list)
    return out
  }

  private fun watchRoutes(am: android.media.AudioManager, on: Boolean) {
    if (!on) {
      deviceCallback?.let { am.unregisterAudioDeviceCallback(it) }
      deviceCallback = null
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) routeListener?.let { am.removeOnCommunicationDeviceChangedListener(it) }
      routeListener = null
      return
    }
    if (deviceCallback != null) return
    val tell = { try { NativeCall.action("audio_routes") } catch (_: Throwable) {} }
    deviceCallback = object : android.media.AudioDeviceCallback() {
      override fun onAudioDevicesAdded(added: Array<out android.media.AudioDeviceInfo>) { tell() }
      override fun onAudioDevicesRemoved(removed: Array<out android.media.AudioDeviceInfo>) { tell() }
    }
    am.registerAudioDeviceCallback(deviceCallback, android.os.Handler(android.os.Looper.getMainLooper()))
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      routeListener = android.media.AudioManager.OnCommunicationDeviceChangedListener { tell() }
      am.addOnCommunicationDeviceChangedListener(activity.mainExecutor, routeListener!!)
    }
  }

  /**
   * Where call audio plays (see AudioArgs). Answers the routes as `audioRoutes` does. On
   * Android 11 and older only speaker on/off exists.
   */
  @Command
  fun callAudio(invoke: Invoke) {
    val args = invoke.parseArgs(AudioArgs::class.java)
    val am = activity.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
    // Switching during a call runs off the main thread: setCommunicationDevice only returns
    // once Android has moved the audio (~0.4 s), and the main thread also draws the WebView,
    // so the whole app froze (the speaker button changed a second late). Entering and leaving
    // call mode stay here: done in the background, the WebView's own audio set-up on the main
    // thread waited 3 s on the busy audio service (the call view froze half-open on join).
    val switching = args.route != null && args.route != "release" &&
      am.mode == android.media.AudioManager.MODE_IN_COMMUNICATION
    if (!switching) {
      audioThread.submit {}.get()  // a switch still running finishes first
      return applyCallAudio(invoke, args)
    }
    // One thread keeps quick taps in order.
    audioThread.execute {
      try {
        applyCallAudio(invoke, args)
      } catch (e: Exception) {
        invoke.reject(e.message ?: "call audio failed")
      }
    }
  }

  private val audioThread = java.util.concurrent.Executors.newSingleThreadExecutor()

  private fun applyCallAudio(invoke: Invoke, args: AudioArgs) {
    val am = activity.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
    val route = args.route
    if (route == null) {
      watchRoutes(am, false)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) am.clearCommunicationDevice()
      else @Suppress("DEPRECATION") { am.isSpeakerphoneOn = false }
      am.mode = android.media.AudioManager.MODE_NORMAL
      return invoke.resolve()
    }
    if (route == "release") {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) am.clearCommunicationDevice()
      am.mode = android.media.AudioManager.MODE_NORMAL
      watchRoutes(am, true)
      return invoke.resolve(routes(am))
    }
    am.mode = android.media.AudioManager.MODE_IN_COMMUNICATION
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      val devices = am.availableCommunicationDevices
      val headsets = listOf("bluetooth", "wired", "usb")
      val wanted = when (route) {
        "device" -> devices.firstOrNull { it.id == args.device }
        "speaker" -> devices.firstOrNull { kindOf(it.type) == "speaker" }
        "headset" -> headsets.firstNotNullOfOrNull { k -> devices.firstOrNull { kindOf(it.type) == k } }
        else -> null
      } ?: devices.firstOrNull { kindOf(it.type) == "earpiece" }
      if (wanted != null && !am.setCommunicationDevice(wanted)) {
        android.util.Log.w("AlwyChat", "call audio: Android refused ${wanted.productName} (${wanted.type})")
      }
    } else {
      @Suppress("DEPRECATION")
      am.isSpeakerphoneOn = route == "speaker"
    }
    watchRoutes(am, true)
    invoke.resolve(routes(am))
  }

  private fun bluetoothGranted() = Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
    activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == android.content.pm.PackageManager.PERMISSION_GRANTED

  /** Asks "Nearby devices" (for headset names) if needed; answers {granted}. */
  @Command
  fun bluetoothNames(invoke: Invoke) {
    if (bluetoothGranted()) return invoke.resolve(JSObject().apply { put("granted", true) })
    requestPermissionForAlias("bluetooth", invoke, "bluetoothAnswered")
  }

  @PermissionCallback
  private fun bluetoothAnswered(invoke: Invoke) {
    openHeadsetProxy()
    invoke.resolve(JSObject().apply { put("granted", bluetoothGranted()) })
  }

  /** The places call audio can go now, and the current one. */
  @Command
  fun audioRoutes(invoke: Invoke) {
    val am = activity.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
    invoke.resolve(routes(am))
  }

  /** The user's 12/24-hour choice (Settings → System → Date & time). */
  @Command
  fun clock(invoke: Invoke) {
    val ret = JSObject()
    ret.put("h24", android.text.format.DateFormat.is24HourFormat(activity))
    invoke.resolve(ret)
  }

  @Command
  fun fixNotifications(invoke: Invoke) {
    if (NotificationManagerCompat.from(activity).areNotificationsEnabled()) {
      return invoke.resolve(currentStatus())
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !askedBefore()) {
      markAsked()
      requestPermissionForAlias("notifications", invoke, "notificationsAnswered")
    } else {
      openNotificationSettings()
      invoke.resolve(currentStatus())
    }
  }

  @PermissionCallback
  private fun notificationsAnswered(invoke: Invoke) {
    // Refused (now or for good): the switch is in the settings page.
    if (!NotificationManagerCompat.from(activity).areNotificationsEnabled()) openNotificationSettings()
    invoke.resolve(currentStatus())
  }

  private fun openNotificationSettings() {
    activity.startActivity(
      Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, pkg),
    )
  }

  /** Asks to be left out of battery optimisation (falls back to the app's details page). */
  @SuppressLint("BatteryLife")
  @Command
  fun fixBattery(invoke: Invoke) {
    try {
      activity.startActivity(
        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$pkg")),
      )
    } catch (_: Exception) {
      openAppDetails()
    }
    invoke.resolve(currentStatus())
  }

  /** The app's system page, where the maker's battery setting lives ("Battery usage"). */
  @Command
  fun openAppSettings(invoke: Invoke) {
    openAppDetails()
    invoke.resolve(currentStatus())
  }

  @Command
  fun fixFullScreenCalls(invoke: Invoke) {
    try {
      activity.startActivity(Intent("android.settings.MANAGE_APP_USE_FULL_SCREEN_INTENT", Uri.parse("package:$pkg")))
    } catch (_: Exception) {
      openAppDetails()
    }
    invoke.resolve(currentStatus())
  }

  @Command
  fun fixBackgroundData(invoke: Invoke) {
    try {
      activity.startActivity(
        Intent(Settings.ACTION_IGNORE_BACKGROUND_DATA_RESTRICTIONS_SETTINGS, Uri.parse("package:$pkg")),
      )
    } catch (_: Exception) {
      openAppDetails()
    }
    invoke.resolve(currentStatus())
  }

  private fun openAppDetails() {
    activity.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:$pkg")))
  }

  /** This install's FCM token; fails on builds without `google-services.json`. */
  @Command
  fun pushToken(invoke: Invoke) {
    val messaging = try {
      FirebaseMessaging.getInstance()
    } catch (e: IllegalStateException) {
      return invoke.reject("push isn't set up in this build")
    }
    messaging.token.addOnCompleteListener { task ->
      if (task.isSuccessful && task.result != null) {
        invoke.resolve(JSObject().put("token", task.result))
      } else {
        invoke.reject("no push token: ${task.exception?.message}")
      }
    }
  }

  /** UnifiedPush: the distributor apps installed, the one in use and this device's endpoint. */
  @Command
  fun unifiedPushStatus(invoke: Invoke) {
    val pm = activity.packageManager
    val list = org.json.JSONArray()
    for (pkg in org.unifiedpush.android.connector.UnifiedPush.getDistributors(activity)) {
      // Our own package can show up (the connector's embedded FCM distributor): not a choice.
      if (pkg == activity.packageName) continue
      val name = try {
        pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString()
      } catch (e: Exception) {
        pkg
      }
      list.put(JSObject().put("id", pkg).put("name", name))
    }
    val result = JSObject()
      .put("distributors", list)
      .put("current", org.unifiedpush.android.connector.UnifiedPush.getSavedDistributor(activity))
      .put("endpoint", UnifiedPushService.endpointOf(activity))
    invoke.resolve(result)
  }

  /** Use a distributor (registers; the endpoint arrives in UnifiedPushService) or stop (null). */
  @Command
  fun useUnifiedPush(invoke: Invoke) {
    val args = invoke.parseArgs(DistributorArgs::class.java)
    val up = org.unifiedpush.android.connector.UnifiedPush
    val pkg = args.distributor
    if (pkg == null) {
      up.unregister(activity)
      up.removeDistributor(activity)
    } else {
      up.saveDistributor(activity, pkg)
      up.register(activity, messageForDistributor = "AlwyChat")
    }
    invoke.resolve()
  }

  /** Match the system bars to the app theme: dark icons on a light theme, and its background. */
  @Command
  fun setBars(invoke: Invoke) {
    val args = invoke.parseArgs(BarsArgs::class.java)
    activity.runOnUiThread {
      val window = activity.window
      window.decorView.setBackgroundColor(android.graphics.Color.parseColor(args.background))
      WindowCompat.getInsetsController(window, window.decorView).apply {
        isAppearanceLightStatusBars = args.light
        isAppearanceLightNavigationBars = args.light
      }
      invoke.resolve()
    }
  }

  /** The push test: a sample notification through the same code as real ones. */
  @Command
  fun testNotification(invoke: Invoke) {
    PushService.handle(activity, "!alwychat-push-test:example.org", PushService.TEST_EVENT)
    invoke.resolve()
  }

  /** Back to the app we came from (a share cancelled): the task goes behind, like Android's back
   * on a screen opened by another app; a floating window closes. */
  @Command
  fun moveToBack(invoke: Invoke) {
    activity.runOnUiThread {
      activity.moveTaskToBack(true)
      invoke.resolve()
    }
  }

  /** App lock: blank in the app switcher, no screenshots (FLAG_SECURE). */
  @Command
  fun setSecure(invoke: Invoke) {
    val args = invoke.parseArgs(SecureArgs::class.java)
    activity.runOnUiThread {
      val flag = android.view.WindowManager.LayoutParams.FLAG_SECURE
      if (args.secure) activity.window.addFlags(flag) else activity.window.clearFlags(flag)
      invoke.resolve()
    }
  }

  /** Notification texts in the app's language (PushService.label), for when the UI isn't running. */
  @Command
  fun notificationLabels(invoke: Invoke) {
    val args = invoke.parseArgs(LabelsArgs::class.java)
    val edit = activity.getSharedPreferences(PushService.LABELS, Context.MODE_PRIVATE).edit().clear()
    for ((key, value) in args.labels) edit.putString(key, value)
    edit.apply()
    invoke.resolve()
  }

  /** A message (or a ring) notified by the running app (no push): shown like a pushed one. */
  @Command
  fun showMessage(invoke: Invoke) {
    val n = invoke.getArgs()
    if (n.has("call") && !n.isNull("call")) IncomingCall.show(activity, n) else PushService.show(activity, n)
    invoke.resolve()
  }

  @Command
  fun clearNotifications(invoke: Invoke) {
    val args = invoke.parseArgs(ClearArgs::class.java)
    NotificationManagerCompat.from(activity).cancel(args.roomId, 1)
    invoke.resolve()
  }

  // Android only shows its prompt twice, then silently refuses: after our first ask, go to settings.
  private fun prefs() = activity.getSharedPreferences("system", Context.MODE_PRIVATE)
  private fun askedBefore() = prefs().getBoolean("asked_notifications", false)
  private fun markAsked() = prefs().edit().putBoolean("asked_notifications", true).apply()
}
