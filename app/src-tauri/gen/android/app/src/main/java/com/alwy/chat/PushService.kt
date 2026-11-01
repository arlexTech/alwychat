package com.alwy.chat

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import org.json.JSONObject

/**
 * FCM pushes from AlwyChat's gateway: "event X in room Y" (nothing readable). The Rust core
 * fetches and decrypts the event, then we show it, one notification per chat.
 */
class PushService : FirebaseMessagingService() {
  override fun onMessageReceived(message: RemoteMessage) {
    val roomId = message.data["room_id"] ?: return
    val eventId = message.data["event_id"] ?: return
    // Settings → Notifications → "Send a test notification": shown even with the app open.
    if (eventId == TEST_EVENT) {
      show(this, JSONObject()
        .put("room_id", roomId)
        .put("title", "AlwyChat")
        .put("body", "Notifications work: this one came from your server's push gateway, through Google, to this phone."))
      return
    }
    // On screen, the chat list already shows it.
    if (MainActivity.visible) return
    val json = try {
      NativePush.notification(applicationContext, applicationContext.dataDir.absolutePath, roomId, eventId)
    } catch (e: Throwable) {
      Log.w("AlwyChat", "push: $e")
      ""
    }
    if (json.isEmpty()) return
    show(this, JSONObject(json))
  }

  override fun onNewToken(token: String) {
    // Registered with the homeserver the next time the app starts (`setup_push`).
    Log.i("AlwyChat", "new push token")
  }

  companion object {
    const val CHANNEL = "messages"
    /** Matches `alwy_core::push::TEST_EVENT`. */
    const val TEST_EVENT = "\$alwychat-push-test"

    fun show(context: Context, n: JSONObject) {
      val manager = NotificationManagerCompat.from(context)
      if (!manager.areNotificationsEnabled()) return
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        val channel = NotificationChannel(CHANNEL, "Messages", NotificationManager.IMPORTANCE_HIGH)
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
      }
      val roomId = n.getString("room_id")
      // Tapping opens the chat (deep link handled in lib.rs).
      val open = Intent(Intent.ACTION_VIEW, Uri.parse("com.alwy.chat:/room?id=" + Uri.encode(roomId)))
        .setPackage(context.packageName)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
      val pending = PendingIntent.getActivity(
        context, roomId.hashCode(), open,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
      val notification = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_notification)
        .setContentTitle(n.getString("title"))
        .setContentText(n.getString("body"))
        .setStyle(NotificationCompat.BigTextStyle().bigText(n.getString("body")))
        .setCategory(NotificationCompat.CATEGORY_MESSAGE)
        .setPriority(NotificationCompat.PRIORITY_HIGH)
        .setSilent(!n.optBoolean("noisy", true))
        .setAutoCancel(true)
        .setContentIntent(pending)
        .build()
      try {
        // Tag = chat: a newer message replaces the older one, and opening the chat clears it.
        manager.notify(roomId, 1, notification)
      } catch (e: SecurityException) {
        Log.w("AlwyChat", "notification refused: $e")
      }
    }
  }
}
