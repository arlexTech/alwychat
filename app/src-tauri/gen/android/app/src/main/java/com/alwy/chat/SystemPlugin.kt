package com.alwy.chat

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import androidx.core.view.WindowCompat
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
class PipShapeArgs {
  var wide: Boolean = false
}

@InvokeArg
class AudioArgs {
  /** null: the call ended (back to normal audio). */
  var speaker: Boolean? = null
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

/**
 * What stops AlwyChat from notifying: the notification permission, battery optimisation
 * (the system freezes the app in the background) and restricted background data.
 * Each `fix*` command asks the system, or opens the right settings page when it can't.
 */
@TauriPlugin(
  permissions = [Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications")],
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
      put("batteryUnrestricted", runCatching { power.isIgnoringBatteryOptimizations(pkg) }.getOrDefault(true))
      // Unknown counts as fine: a failed check shouldn't nag the user.
      put(
        "backgroundData",
        runCatching { net.restrictBackgroundStatus != ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED }
          .getOrDefault(true),
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

  /**
   * Where call audio plays: the loudspeaker, or else headphones / Bluetooth when connected, the
   * earpiece otherwise. `speaker` null ends the call mode. Answers `{speaker}` as applied.
   */
  @Command
  fun callAudio(invoke: Invoke) {
    val args = invoke.parseArgs(AudioArgs::class.java)
    val am = activity.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
    val speaker = args.speaker
    if (speaker == null) {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) am.clearCommunicationDevice()
      else @Suppress("DEPRECATION") { am.isSpeakerphoneOn = false }
      am.mode = android.media.AudioManager.MODE_NORMAL
      return invoke.resolve()
    }
    am.mode = android.media.AudioManager.MODE_IN_COMMUNICATION
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      val devices = am.availableCommunicationDevices
      val headset = listOf(
        android.media.AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        android.media.AudioDeviceInfo.TYPE_BLE_HEADSET,
        android.media.AudioDeviceInfo.TYPE_WIRED_HEADSET,
        android.media.AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
        android.media.AudioDeviceInfo.TYPE_USB_HEADSET,
      )
      val wanted = if (speaker) {
        devices.firstOrNull { it.type == android.media.AudioDeviceInfo.TYPE_BUILTIN_SPEAKER }
      } else {
        devices.firstOrNull { it.type in headset }
          ?: devices.firstOrNull { it.type == android.media.AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
      }
      if (wanted != null) am.setCommunicationDevice(wanted)
    } else {
      @Suppress("DEPRECATION")
      am.isSpeakerphoneOn = speaker
    }
    invoke.resolve(JSObject().apply { put("speaker", speaker) })
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
