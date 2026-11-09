package com.alwy.chat

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.Person
import androidx.core.app.RemoteInput

/**
 * A message notification's "Reply" and "Mark as read" (the app may not be running): the Rust
 * core sends the reply / the read receipt (`NativePush.answer`). A reply is then added to the
 * conversation in the notification (Android expects it, else it keeps spinning); a failed
 * one says so there, keeping the text visible.
 */
class NotificationAnswer : BroadcastReceiver() {
  companion object {
    const val REPLY = "com.alwy.chat.REPLY"
    const val READ = "com.alwy.chat.MARK_READ"

    fun intent(context: Context, action: String, roomId: String, eventId: String): Intent =
      Intent(context, NotificationAnswer::class.java)
        .setAction(action)
        // One PendingIntent per chat and action (extras don't make them distinct).
        .setData(Uri.parse("alwychat-notification:" + Uri.encode(roomId)))
        .putExtra("room_id", roomId)
        .putExtra("event_id", eventId)
  }

  override fun onReceive(context: Context, intent: Intent) {
    val app = context.applicationContext
    val roomId = intent.getStringExtra("room_id") ?: return
    val eventId = intent.getStringExtra("event_id") ?: return
    val text = if (intent.action == REPLY) {
      RemoteInput.getResultsFromIntent(intent)?.getCharSequence(PushService.REPLY_KEY)?.toString()?.trim()
        ?: return
    } else ""
    if (intent.action == READ) NotificationManagerCompat.from(app).cancel(roomId, 1)
    val pending = goAsync()
    Thread {
      try {
        val error = try {
          NativePush.answer(app, app.dataDir.absolutePath, roomId, eventId, text)
        } catch (e: Throwable) {
          e.toString()
        }
        if (text.isNotEmpty()) shown(app, roomId, eventId, text, error)
        else if (error.isNotEmpty()) Log.w("AlwyChat", "mark as read: $error")
      } finally {
        pending.finish()
      }
    }.start()
  }

  /** Our reply added to the chat's notification (or "not sent", with the text). */
  private fun shown(context: Context, roomId: String, eventId: String, text: String, error: String) {
    val style = PushService.conversation(context, roomId, null)
    if (error.isEmpty()) {
      style.addMessage(text, System.currentTimeMillis(), null as Person?)
    } else {
      Log.w("AlwyChat", "reply from the notification: $error")
      val failed = PushService.label(context, "not_sent", "Not sent: %s").replace("%s", text)
      style.addMessage(failed, System.currentTimeMillis(), null as Person?)
    }
    PushService.post(context, roomId, eventId, style, silent = true, onlyAlertOnce = true)
  }
}
