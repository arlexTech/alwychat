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
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

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
      put("batteryUnrestricted", power.isIgnoringBatteryOptimizations(pkg))
      put(
        "backgroundData",
        net.restrictBackgroundStatus != ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED,
      )
    }
  }

  /** The system prompt if Android still allows asking, else the app's notification settings. */
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

  // Android only shows its prompt twice, then silently refuses: after our first ask, go to settings.
  private fun prefs() = activity.getSharedPreferences("system", Context.MODE_PRIVATE)
  private fun askedBefore() = prefs().getBoolean("asked_notifications", false)
  private fun markAsked() = prefs().edit().putBoolean("asked_notifications", true).apply()
}
