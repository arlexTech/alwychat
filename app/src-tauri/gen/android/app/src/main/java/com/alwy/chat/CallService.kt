package com.alwy.chat

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

/**
 * A call in progress: an ongoing notification (tap = back to the chat, "Hang up") and, as a
 * foreground service using the microphone, the call keeps going while AlwyChat is in the
 * background. Started and stopped by the UI through SystemPlugin.callNotification.
 */
class CallService : Service() {
  companion object {
    private const val CHANNEL = "calls"
    private const val ID = 4242
    const val HANG_UP = "hang_up"
    const val TOGGLE_MIC = "toggle_mic"

    /** Also called again to update the notification (mic muted or not). */
    fun start(
      context: Context, title: String, text: String, hangUp: String, channel: String, roomId: String?,
      muted: Boolean?, mute: String, unmute: String, camera: Boolean,
    ) {
      val intent = Intent(context, CallService::class.java)
        .putExtra("title", title)
        .putExtra("text", text)
        .putExtra("hangUp", hangUp)
        .putExtra("channel", channel)
        .putExtra("roomId", roomId)
        .putExtra("hasMic", muted != null)
        .putExtra("muted", muted == true)
        .putExtra("mute", mute)
        .putExtra("unmute", unmute)
        .putExtra("camera", camera)
      ContextCompat.startForegroundService(context, intent)
    }

    fun stop(context: Context) {
      context.stopService(Intent(context, CallService::class.java))
    }
  }

  private var since = 0L

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    when (intent?.action) {
      HANG_UP -> {
        NativeCall.action(HANG_UP)
        stopSelf()
        return START_NOT_STICKY
      }
      // The UI toggles the mic, then updates this notification.
      TOGGLE_MIC -> {
        NativeCall.action(TOGGLE_MIC)
        return START_NOT_STICKY
      }
    }
    if (since == 0L) since = System.currentTimeMillis()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel = NotificationChannel(CHANNEL, intent?.getStringExtra("channel")?.ifEmpty { null } ?: "Calls", NotificationManager.IMPORTANCE_LOW)
      channel.setShowBadge(false)
      getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
    val roomId = intent?.getStringExtra("roomId")
    val open = Intent(
      Intent.ACTION_VIEW,
      Uri.parse(if (roomId != null) "io.github.arlextech.alwychat:/room?id=" + Uri.encode(roomId) else "io.github.arlextech.alwychat:/"),
    ).setPackage(packageName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
    val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    val builder = NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle(intent?.getStringExtra("title") ?: "AlwyChat")
      .setContentText(intent?.getStringExtra("text"))
      .setOngoing(true)
      .setCategory(NotificationCompat.CATEGORY_CALL)
      .setUsesChronometer(true)
      .setWhen(since)
      .setShowWhen(true)
      .setContentIntent(PendingIntent.getActivity(this, 0, open, flags))
    if (intent?.getBooleanExtra("hasMic", false) == true) {
      val muted = intent.getBooleanExtra("muted", false)
      builder.addAction(
        0,
        intent.getStringExtra(if (muted) "unmute" else "mute") ?: if (muted) "Unmute" else "Mute",
        PendingIntent.getService(this, 2, Intent(this, CallService::class.java).setAction(TOGGLE_MIC), flags),
      )
    }
    val notification = builder
      .addAction(
        0,
        intent?.getStringExtra("hangUp") ?: "Hang up",
        PendingIntent.getService(this, 1, Intent(this, CallService::class.java).setAction(HANG_UP), flags),
      )
      .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
      .build()
    try {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        // With the camera on, also a camera service: Android only lets an app use the camera
        // outside the foreground (picture-in-picture, another app on top) through one.
        val camera = intent?.getBooleanExtra("camera", false) == true &&
          ContextCompat.checkSelfPermission(this, android.Manifest.permission.CAMERA) ==
          android.content.pm.PackageManager.PERMISSION_GRANTED
        var types = ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        if (camera) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA
        startForeground(ID, notification, types)
      } else {
        startForeground(ID, notification)
      }
    } catch (e: Exception) {
      // No microphone permission (listening only): Android refuses a microphone service.
      Log.w("AlwyChat", "call notification: $e")
      stopSelf()
    }
    return START_NOT_STICKY
  }
}
