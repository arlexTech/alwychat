package com.alwy.chat

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
 * A live location share in progress: an ongoing notification (tap = the chat, "Stop sharing")
 * and, as a location foreground service, the app keeps following the position while it's in
 * the background. Started, updated and stopped by the UI through SystemPlugin.locationNotification.
 */
class LocationService : Service() {
  companion object {
    private const val CHANNEL = "live_location"
    private const val ID = 4244
    const val STOP = "stop_location"

    fun start(context: Context, title: String, text: String, stop: String, channel: String, roomId: String?, endsAt: Long) {
      val intent = Intent(context, LocationService::class.java)
        .putExtra("title", title)
        .putExtra("text", text)
        .putExtra("stop", stop)
        .putExtra("channel", channel)
        .putExtra("roomId", roomId)
        .putExtra("endsAt", endsAt)
      ContextCompat.startForegroundService(context, intent)
    }

    fun stop(context: Context) {
      context.stopService(Intent(context, LocationService::class.java))
    }
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == STOP) {
      // The UI stops the shares (and then this service).
      NativeCall.action(STOP)
      return START_NOT_STICKY
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel = NotificationChannel(CHANNEL, intent?.getStringExtra("channel")?.ifEmpty { null } ?: "Live location", NotificationManager.IMPORTANCE_LOW)
      channel.setShowBadge(false)
      getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
    val roomId = intent?.getStringExtra("roomId")
    val open = Intent(
      Intent.ACTION_VIEW,
      Uri.parse(if (roomId != null) "io.github.arlextech.alwychat:/room?id=" + Uri.encode(roomId) else "io.github.arlextech.alwychat:/"),
    ).setPackage(packageName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
    val pending = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    val endsAt = intent?.getLongExtra("endsAt", 0L) ?: 0L
    val builder = NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle(intent?.getStringExtra("title") ?: "AlwyChat")
      .setContentText(intent?.getStringExtra("text"))
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setCategory(NotificationCompat.CATEGORY_LOCATION_SHARING)
      .setContentIntent(PendingIntent.getActivity(this, 20, open, pending))
      .addAction(
        0,
        intent?.getStringExtra("stop") ?: "Stop sharing",
        PendingIntent.getService(this, 21, Intent(this, LocationService::class.java).setAction(STOP), pending),
      )
      .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
    // A countdown to the end of the share (none for "until I stop").
    if (endsAt > System.currentTimeMillis()) {
      builder.setWhen(endsAt).setShowWhen(true).setUsesChronometer(true)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) builder.setChronometerCountDown(true)
    }
    try {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        startForeground(ID, builder.build(), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)
      } else {
        startForeground(ID, builder.build())
      }
    } catch (e: Exception) {
      // No location permission any more: Android refuses a location service.
      Log.w("AlwyChat", "location notification: $e")
      stopSelf()
    }
    return START_NOT_STICKY
  }
}
