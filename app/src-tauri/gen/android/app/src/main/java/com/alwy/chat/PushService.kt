package com.alwy.chat

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.Person
import androidx.core.app.RemoteInput
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import org.json.JSONObject

/**
 * FCM pushes from AlwyChat's gateway: "event X in room Y" (nothing readable). The Rust core
 * fetches and decrypts the event, then we show it, one notification per chat.
 */
class PushService : FirebaseMessagingService() {
  override fun onMessageReceived(message: RemoteMessage) {
    val roomId = message.data["room_id"]
    val eventId = message.data["event_id"]
    // No event: only the unread count changed, so something was read on another device.
    if (roomId.isNullOrEmpty() || eventId.isNullOrEmpty()) {
      clearRead(applicationContext, message.data["unread"])
      return
    }
    handle(applicationContext, roomId, eventId)
  }

  override fun onNewToken(token: String) {
    // Registered with the homeserver the next time the app starts (`setup_push`).
    Log.i("AlwyChat", "new push token")
  }

  companion object {
    const val CHANNEL = "messages"

    /** Removes the notifications of chats read elsewhere (`unread`: the server's new count). */
    fun clearRead(context: Context, unread: String?) {
      Crashes.watch(context)
      val manager = context.getSystemService(NotificationManager::class.java) ?: return
      val shown = manager.activeNotifications
        .filter { it.id == 1 && it.tag?.startsWith("!") == true }
        .map { it.tag }
      if (shown.isEmpty()) return
      val read = if (unread == "0") shown else try {
        NativePush.readChats(context, context.dataDir.absolutePath, shown.joinToString("\n"))
          .lines().filter { it.isNotEmpty() }
      } catch (e: Throwable) {
        Log.w("AlwyChat", "push: read chats: $e")
        emptyList()
      }
      for (roomId in read) manager.cancel(roomId, 1)
    }

    /** A pushed "event X in room Y" (from FCM or UnifiedPush): fetched, decrypted, shown. */
    fun handle(context: Context, roomId: String, eventId: String) {
      Crashes.watch(context)
      // Settings → Notifications → "Send a test notification": shown even with the app open.
      if (eventId == TEST_EVENT) {
        show(context, JSONObject()
          .put("room_id", roomId)
          .put("title", "AlwyChat")
          .put("body", "Notifications work: this one came from your server, through the push service, to this phone."))
        return
      }
      // On screen, the chat list already shows it.
      if (MainActivity.visible) return
      val json = try {
        NativePush.notification(context, context.dataDir.absolutePath, roomId, eventId)
      } catch (e: Throwable) {
        Log.w("AlwyChat", "push: $e")
        ""
      }
      if (json.isEmpty()) return
      val n = JSONObject(json)
      if (n.has("call") && !n.isNull("call")) IncomingCall.show(context, n) else show(context, n)
    }
    /** Matches `alwy_core::push::TEST_EVENT`. */
    const val TEST_EVENT = "\$alwychat-push-test"

    /** Notification texts in the app's language, saved by the UI (`notificationLabels`). */
    fun label(context: Context, key: String, fallback: String): String =
      context.getSharedPreferences(LABELS, Context.MODE_PRIVATE).getString(key, null) ?: fallback
    const val LABELS = "notification_labels"
    const val REPLY_KEY = "reply"
    /** Notification extra: the event IDs it shows. */
    const val EVENTS = "com.alwy.chat.events"

    /**
     * One notification per chat, as a conversation: each new message is added to the ones
     * already shown (up to 8), with Reply and Mark as read.
     */
    fun show(context: Context, n: JSONObject) {
      val manager = NotificationManagerCompat.from(context)
      if (!manager.areNotificationsEnabled()) return
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        val channel = NotificationChannel(CHANNEL, "Messages", NotificationManager.IMPORTANCE_HIGH)
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
      }
      val roomId = n.getString("room_id")
      val eventId = n.optString("event_id")
      // Pushed and also seen by the running app's sync: shown once.
      if (eventId.isNotEmpty() && eventId in shownEvents(context, roomId)) return
      // Texts made by the core are English: the UI saved their translation ("text:…").
      val sender = n.optString("sender").ifEmpty { n.getString("title") }.let { label(context, "text:$it", it) }
      val body = n.getString("body").let { label(context, "text:$it", it) }
      val roomName = if (n.isNull("room_name")) null else n.optString("room_name").ifEmpty { null }
      val sentAt = n.optLong("sent_at", System.currentTimeMillis())
      val style = conversation(context, roomId, roomName)
      style.addMessage(body, sentAt, Person.Builder().setName(sender).setKey(sender).build())
      post(context, roomId, eventId, style, silent = !n.optBoolean("noisy", true), onlyAlertOnce = false)
    }

    /** The messages a chat's notification on screen already shows (their event IDs). */
    fun shownEvents(context: Context, roomId: String): List<String> =
      context.getSystemService(NotificationManager::class.java).activeNotifications
        .firstOrNull { it.tag == roomId && it.id == 1 }
        ?.notification?.extras?.getStringArrayList(EVENTS)
        ?: emptyList()

    /** The chat's conversation so far (from its notification on screen), or a new one. */
    fun conversation(context: Context, roomId: String, roomName: String?): NotificationCompat.MessagingStyle {
      val me = Person.Builder().setName(label(context, "you", "You")).setKey("me").build()
      val shown = context.getSystemService(NotificationManager::class.java).activeNotifications
        .firstOrNull { it.tag == roomId && it.id == 1 }
      val style = shown?.let { NotificationCompat.MessagingStyle.extractMessagingStyleFromNotification(it.notification) }
        ?: NotificationCompat.MessagingStyle(me)
      if (roomName != null) {
        style.conversationTitle = roomName
        style.isGroupConversation = true
      }
      // Keep the latest few: the notification only has room for those.
      while (style.messages.size >= 8) style.messages.removeAt(0)
      return style
    }

    /** Show (or update) a chat's notification. `eventId`: what Reply / Mark as read answer. */
    fun post(
      context: Context, roomId: String, eventId: String, style: NotificationCompat.MessagingStyle,
      silent: Boolean, onlyAlertOnce: Boolean,
    ) {
      val manager = NotificationManagerCompat.from(context)
      // Tapping opens the chat (deep link handled in lib.rs).
      val open = Intent(Intent.ACTION_VIEW, Uri.parse("io.github.arlextech.alwychat:/room?id=" + Uri.encode(roomId)))
        .setPackage(context.packageName)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
      val pending = PendingIntent.getActivity(
        context, roomId.hashCode(), open,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
      val builder = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_notification)
        .setStyle(style)
        .setCategory(NotificationCompat.CATEGORY_MESSAGE)
        .setPriority(NotificationCompat.PRIORITY_HIGH)
        .setSilent(silent)
        .setOnlyAlertOnce(onlyAlertOnce)
        .setAutoCancel(true)
        .setContentIntent(pending)
        // No suggested replies / emoji added by Android: our own Reply is enough.
        .setAllowSystemGeneratedContextualActions(false)
      // Remember what it shows (the latest 20), so the same message isn't added again.
      if (eventId.isNotEmpty()) {
        val events = ArrayList((shownEvents(context, roomId) - eventId + eventId).takeLast(20))
        builder.addExtras(Bundle().apply { putStringArrayList(EVENTS, events) })
      }
      if (eventId.isNotEmpty() && eventId != TEST_EVENT) {
        val reply = RemoteInput.Builder(REPLY_KEY).setLabel(label(context, "message", "Message")).build()
        // Mutable: Android puts the typed text into it.
        val replyIntent = PendingIntent.getBroadcast(
          context, roomId.hashCode(), NotificationAnswer.intent(context, NotificationAnswer.REPLY, roomId, eventId),
          PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
        )
        val readIntent = PendingIntent.getBroadcast(
          context, roomId.hashCode(), NotificationAnswer.intent(context, NotificationAnswer.READ, roomId, eventId),
          PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        builder
          .addAction(
            NotificationCompat.Action.Builder(0, label(context, "reply", "Reply"), replyIntent)
              .addRemoteInput(reply)
              .setSemanticAction(NotificationCompat.Action.SEMANTIC_ACTION_REPLY)
              .setAllowGeneratedReplies(false)
              .setShowsUserInterface(false)
              .build(),
          )
          .addAction(
            NotificationCompat.Action.Builder(0, label(context, "read", "Mark as read"), readIntent)
              .setSemanticAction(NotificationCompat.Action.SEMANTIC_ACTION_MARK_AS_READ)
              .setShowsUserInterface(false)
              .build(),
          )
      }
      try {
        // Tag = chat: a newer message joins the older ones, and opening the chat clears it.
        manager.notify(roomId, 1, builder.build())
      } catch (e: SecurityException) {
        Log.w("AlwyChat", "notification refused: $e")
      }
    }
  }
}
