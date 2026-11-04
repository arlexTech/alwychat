package com.alwy.chat

import android.content.Context
import android.util.Log
import org.json.JSONObject
import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService as UpService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

/**
 * UnifiedPush (no Google): the distributor app the person chose (ntfy…) delivers what the
 * homeserver's pusher sends through a Matrix push gateway: "event X in room Y", handled like an
 * FCM push. New endpoints go straight to the core (`NativePush.setEndpoint`), which registers
 * them with the homeserver, so it works while the app isn't open.
 */
class UnifiedPushService : UpService() {
  override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
    // A temporary fallback endpoint (the distributor's server is down) isn't worth a pusher.
    if (endpoint.temporary) return
    val old = endpointOf(this)
    if (old == endpoint.url) return
    prefs(this).edit().putString(KEY, endpoint.url).apply()
    register(applicationContext, endpoint.url, old)
  }

  override fun onMessage(message: PushMessage, instance: String) {
    // The gateway forwards the homeserver's JSON: {"notification": {"room_id", "event_id", …}}.
    val json = try {
      JSONObject(String(message.content, Charsets.UTF_8))
    } catch (e: Exception) {
      Log.w("AlwyChat", "UnifiedPush: unreadable message")
      return
    }
    val n = json.optJSONObject("notification") ?: json
    val roomId = n.optString("room_id").ifEmpty { return }
    val eventId = n.optString("event_id").ifEmpty { return }
    PushService.handle(applicationContext, roomId, eventId)
  }

  override fun onRegistrationFailed(reason: FailedReason, instance: String) {
    Log.w("AlwyChat", "UnifiedPush registration failed: $reason")
  }

  override fun onUnregistered(instance: String) {
    val old = endpointOf(this) ?: return
    prefs(this).edit().remove(KEY).apply()
    register(applicationContext, "", old)
  }

  companion object {
    private const val PREFS = "alwychat_push"
    private const val KEY = "up_endpoint"

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** The current UnifiedPush endpoint, if registered. */
    fun endpointOf(context: Context): String? = prefs(context).getString(KEY, null)

    /** Tell the core (blocking network: on a thread). `endpoint` empty: removed. */
    private fun register(context: Context, endpoint: String, old: String?) {
      Thread {
        try {
          NativePush.setEndpoint(context, context.dataDir.absolutePath, endpoint, old ?: "")
        } catch (e: Throwable) {
          Log.w("AlwyChat", "UnifiedPush endpoint: $e")
        }
      }.start()
    }
  }
}
