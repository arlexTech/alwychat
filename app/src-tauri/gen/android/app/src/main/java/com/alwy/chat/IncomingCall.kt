package com.alwy.chat

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.RingtoneManager
import android.net.Uri
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.Person
import org.json.JSONObject

/**
 * A call rung while AlwyChat isn't on screen (from a push): Android's incoming-call notification,
 * full screen on a locked phone, ringing until answered, declined or the ring times out.
 * Answer opens the app into the call; Decline tells the caller without opening it. Opening the
 * app removes it (the app shows its own incoming call screen).
 */
object IncomingCall {
  private const val CHANNEL = "incoming_calls"
  private const val TAG = "ring:"
  private const val ID = 2

  fun show(context: Context, n: JSONObject) {
    val call = n.getJSONObject("call")
    val roomId = n.getString("room_id")
    val manager = NotificationManagerCompat.from(context)
    if (!manager.areNotificationsEnabled()) return
    val left = call.getLong("expires_at") - System.currentTimeMillis()
    if (left <= 0) return
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel = NotificationChannel(CHANNEL, "Incoming calls", NotificationManager.IMPORTANCE_HIGH)
      channel.setSound(
        RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE),
        AudioAttributes.Builder()
          .setUsage(AudioAttributes.USAGE_NOTIFICATION_RINGTONE)
          .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
          .build(),
      )
      channel.enableVibration(true)
      channel.vibrationPattern = longArrayOf(0, 800, 600, 800, 600)
      context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
    val name = call.optString("caller_name", n.optString("title", "AlwyChat"))
    val video = call.optBoolean("video", false)
    val link = Uri.Builder()
      .scheme("io.github.arlextech.alwychat").path("/call")
      .appendQueryParameter("id", roomId)
      .appendQueryParameter("event", call.getString("notification_id"))
      .appendQueryParameter("video", if (video) "1" else "0")
      .appendQueryParameter("expires", call.getLong("expires_at").toString())
      .appendQueryParameter("name", name)
    val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    fun open(answer: Boolean, code: Int): PendingIntent {
      val uri = if (answer) link.build().buildUpon().appendQueryParameter("answer", "1").build() else link.build()
      val intent = Intent(Intent.ACTION_VIEW, uri).setPackage(context.packageName)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
      return PendingIntent.getActivity(context, roomId.hashCode() * 4 + code, intent, flags)
    }
    val decline = PendingIntent.getBroadcast(
      context, roomId.hashCode() * 4 + 3,
      Intent(context, CallDeclineReceiver::class.java)
        .putExtra("room_id", roomId)
        .putExtra("event", call.getString("notification_id")),
      flags,
    )
    val caller = Person.Builder().setName(name).setImportant(true).build()
    val notification = NotificationCompat.Builder(context, CHANNEL)
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle(name)
      .setContentText(if (video) "Incoming video call" else "Incoming call")
      .setStyle(NotificationCompat.CallStyle.forIncomingCall(caller, decline, open(true, 1)))
      .setCategory(NotificationCompat.CATEGORY_CALL)
      .setPriority(NotificationCompat.PRIORITY_MAX)
      .setContentIntent(open(false, 0))
      // Full screen on a locked phone (Android 14+ asks the user to allow it; else a heads-up).
      .setFullScreenIntent(open(false, 2), true)
      .setOngoing(true)
      .setAutoCancel(true)
      .setTimeoutAfter(left)
      .build()
    // Rings until answered (FLAG_INSISTENT repeats the sound).
    notification.flags = notification.flags or android.app.Notification.FLAG_INSISTENT
    try {
      manager.notify(TAG + roomId, ID, notification)
    } catch (e: SecurityException) {
      Log.w("AlwyChat", "incoming call refused: $e")
    }
  }

  fun cancel(context: Context, roomId: String) {
    NotificationManagerCompat.from(context).cancel(TAG + roomId, ID)
  }

  /** The app is on screen: it shows calls itself. */
  fun cancelAll(context: Context) {
    val manager = context.getSystemService(NotificationManager::class.java)
    for (n in manager.activeNotifications) if (n.tag?.startsWith(TAG) == true) manager.cancel(n.tag, n.id)
  }
}

/** "Decline" on the incoming call: tells the caller (the Rust core sends the decline event). */
class CallDeclineReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    val roomId = intent.getStringExtra("room_id") ?: return
    val event = intent.getStringExtra("event") ?: return
    IncomingCall.cancel(context, roomId)
    val pending = goAsync()
    Thread {
      try {
        NativePush.decline(context.applicationContext, context.applicationContext.dataDir.absolutePath, roomId, event)
      } catch (e: Throwable) {
        Log.w("AlwyChat", "decline: $e")
      } finally {
        pending.finish()
      }
    }.start()
  }
}
