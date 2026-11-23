package com.alwy.chat

import android.app.Notification
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Build
import android.provider.Settings
import android.util.Base64
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.Person
import androidx.core.content.LocusIdCompat
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat

/**
 * Chat bubbles (Android 11+, like Messenger): a chat's message notification can float as a round
 * chat head that opens the chat in a small window ([BubbleActivity]). Android decides which chats
 * bubble (system setting: all / selected / none); we give every message notification what a
 * bubble needs: a conversation shortcut (the chat's name and photo) and bubble metadata.
 */
object Bubbles {
  private const val PREFS = "alwychat_bubbles"
  /** Bubble window height (dp). */
  private const val HEIGHT_DP = 640

  /** Shortcut-based bubbles exist from Android 11. */
  val supported get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R

  /**
   * Settings → Notifications → "Chat bubbles" (off by default: turning it on first checks that
   * the phone lets us bubble, see [probe]).
   */
  fun enabled(context: Context) =
    supported && context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean("on", false)

  /** "All chats can bubble" (on by default); off: only the chats picked one by one. */
  fun allChats(context: Context) = prefs(context).getBoolean("all", true)

  fun setAllChats(context: Context, on: Boolean) {
    prefs(context).edit().putBoolean("all", on).apply()
  }

  /** Chats picked one by one ("Bubble this chat"; also every chat opened as a bubble). */
  fun chosen(context: Context): Set<String> = prefs(context).getStringSet("rooms", emptySet()) ?: emptySet()

  fun setChosen(context: Context, roomId: String, on: Boolean) {
    val rooms = chosen(context).toMutableSet()
    if (on) rooms += roomId else rooms -= roomId
    prefs(context).edit().putStringSet("rooms", rooms).apply()
    if (!on && !allChats(context)) unbubble(context, roomId)
  }

  /** The phone threw away our bubble (ColorOS lets only a fixed list of apps bubble). */
  // "phone_blocked": a test build kept a date under "blocked" (reading that as a boolean throws).
  fun blocked(context: Context) = prefs(context).getBoolean("phone_blocked", false)

  fun setBlocked(context: Context, on: Boolean) {
    prefs(context).edit().remove("blocked").putBoolean("phone_blocked", on).apply()
  }

  /** Whether [probe] still has to run (bubbles turned on while Android's setting said none). */
  fun checked(context: Context) = prefs(context).getBoolean("checked", false)

  private fun setChecked(context: Context, on: Boolean) {
    prefs(context).edit().putBoolean("checked", on).apply()
  }

  /** Turning bubbles on: checked again once Android's own setting allows them. */
  fun turnOn(context: Context) {
    setEnabled(context, true)
    setChecked(context, false)
  }

  /**
   * Whether the phone really lets AlwyChat bubble (blocking, ≤2 s: never on the main thread).
   * Posts a quiet test bubble and looks whether Android kept it: some phones (ColorOS) throw
   * away the bubble of every app not on their list. Null: can't tell yet (Android's setting
   * says none, or the post never showed). A blocked phone turns bubbles off again.
   */
  fun probe(context: Context): Boolean? {
    if (!supported || Build.VERSION.SDK_INT < Build.VERSION_CODES.R || preference(context) == "none") return null
    val manager = context.getSystemService(NotificationManager::class.java) ?: return null
    if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return null
    manager.createNotificationChannel(
      android.app.NotificationChannel(PushService.CHANNEL, "Messages", NotificationManager.IMPORTANCE_HIGH),
    )
    val name = "AlwyChat"
    val shortcutId = shortcut(context, PROBE, name, null, false) ?: return null
    val me = Person.Builder().setName(name).setKey(PROBE).build()
    val note = NotificationCompat.Builder(context, PushService.CHANNEL)
      .setSmallIcon(R.drawable.ic_notification)
      .setStyle(NotificationCompat.MessagingStyle(me).addMessage(name, System.currentTimeMillis(), me))
      .setCategory(NotificationCompat.CATEGORY_MESSAGE)
      .setSilent(true)
      .setShortcutId(shortcutId)
      .setBubbleMetadata(
        NotificationCompat.BubbleMetadata.Builder(shortcutId).setDesiredHeight(HEIGHT_DP).setSuppressNotification(true).build(),
      )
      .build()
    val result = try {
      NotificationManagerCompat.from(context).notify(PROBE, 1, note)
      kept(context, PROBE)
    } catch (e: SecurityException) {
      null
    } finally {
      manager.cancel(PROBE, 1)
      ShortcutManagerCompat.removeDynamicShortcuts(context, listOf(PROBE))
    }
    if (result != null) {
      setChecked(context, true)
      setBlocked(context, !result)
      if (!result) setEnabled(context, false)
    }
    return result
  }

  /** Tag and shortcut of [probe]'s test bubble. */
  private const val PROBE = "alwychat-bubble-check"

  /**
   * Waits (≤2 s, off the main thread) for the chat's notification to be posted and tells whether
   * Android kept its bubble: it removes the bubble part when the app may not bubble.
   */
  fun kept(context: Context, roomId: String): Boolean? {
    val manager = context.getSystemService(NotificationManager::class.java) ?: return null
    repeat(20) {
      Thread.sleep(100)
      val shown = manager.activeNotifications.firstOrNull { it.tag == roomId && it.id == 1 }
      if (shown != null && shown.postTime >= System.currentTimeMillis() - 3000) {
        return NotificationCompat.getBubbleMetadata(shown.notification) != null
      }
    }
    return null
  }

  /** Whether this chat's notification may float as a bubble. */
  fun allowed(context: Context, roomId: String) =
    enabled(context) && (allChats(context) || roomId in chosen(context))

  private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

  /** A chat that may no longer bubble: its floating bubble goes (with its notification). */
  private fun unbubble(context: Context, roomId: String) {
    val manager = context.getSystemService(NotificationManager::class.java) ?: return
    val shown = manager.activeNotifications.firstOrNull { it.tag == roomId && it.id == 1 } ?: return
    if (shown.notification.flags and Notification.FLAG_BUBBLE != 0) manager.cancel(roomId, 1)
  }

  fun setEnabled(context: Context, on: Boolean) {
    context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean("on", on).apply()
    if (!on) {
      // Bubbles already floating go away with their notification's metadata.
      val manager = context.getSystemService(NotificationManager::class.java)
      for (n in manager.activeNotifications) {
        if (n.id == 1 && n.notification.flags and Notification.FLAG_BUBBLE != 0) manager.cancel(n.tag, n.id)
      }
    }
  }

  /** What Android allows: "all", "selected" or "none" (Android 12+ tells; 11: "selected" if allowed). */
  fun preference(context: Context): String {
    if (!supported) return "none"
    val manager = context.getSystemService(NotificationManager::class.java)
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      when (manager.bubblePreference) {
        NotificationManager.BUBBLE_PREFERENCE_ALL -> "all"
        NotificationManager.BUBBLE_PREFERENCE_SELECTED -> "selected"
        else -> "none"
      }
    } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      @Suppress("DEPRECATION")
      if (manager.areBubblesAllowed()) "all" else "selected"
    } else {
      "none"
    }
  }

  /** Android's bubble settings for the app. */
  fun openSettings(context: Context) {
    val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      Intent(Settings.ACTION_APP_NOTIFICATION_BUBBLE_SETTINGS)
    } else {
      Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
    }
      .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
      .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    context.startActivity(intent)
  }

  /**
   * Publishes the chat's conversation shortcut (Android 11+: needed for a bubble, and shows the
   * chat's photo in the notification's conversation section) and returns its ID.
   */
  fun shortcut(context: Context, roomId: String, name: String, avatar: String?, group: Boolean): String? {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return null
    val icon = IconCompat.createWithAdaptiveBitmap(photo(avatar) ?: letter(name))
    val person = Person.Builder().setName(name).setIcon(icon).setKey(roomId).build()
    val info = ShortcutInfoCompat.Builder(context, roomId)
      .setShortLabel(name)
      .setLongLabel(name)
      .setIcon(icon)
      .setLongLived(true)
      .setIsConversation()
      .setLocusId(LocusIdCompat(roomId))
      .setIntent(BubbleActivity.intent(context, roomId))
      .apply { if (!group) setPerson(person) }
      .build()
    return try {
      ShortcutManagerCompat.pushDynamicShortcut(context, info)
      roomId
    } catch (e: Exception) {
      android.util.Log.w("AlwyChat", "bubble shortcut: $e")
      null
    }
  }

  /** Bubble metadata for a chat's notification (`open`: float it open at once). */
  @androidx.annotation.RequiresApi(Build.VERSION_CODES.R)
  fun metadata(shortcutId: String, open: Boolean): NotificationCompat.BubbleMetadata =
    NotificationCompat.BubbleMetadata.Builder(shortcutId)
      .setDesiredHeight(HEIGHT_DP)
      .setAutoExpandBubble(open)
      .setSuppressNotification(open)
      .build()

  /**
   * Removes a chat's notification once read. A floating bubble would go with it: then the
   * notification stays, hidden from the shade (only the bubble shows).
   */
  fun clear(context: Context, roomId: String) {
    val manager = context.getSystemService(NotificationManager::class.java) ?: return
    val shown = manager.activeNotifications.firstOrNull { it.tag == roomId && it.id == 1 }
    if (!supported || Build.VERSION.SDK_INT < Build.VERSION_CODES.R || shown == null ||
      shown.notification.flags and Notification.FLAG_BUBBLE == 0
    ) {
      NotificationManagerCompat.from(context).cancel(roomId, 1)
      return
    }
    val current = NotificationCompat.getBubbleMetadata(shown.notification)
    // Re-posting a bubble that's already quiet reshuffles the bubbles on screen: leave it.
    if (current?.isNotificationSuppressed == true && !current.autoExpandBubble) return
    val quiet = NotificationCompat.BubbleMetadata.Builder(roomId)
      .setDesiredHeight(HEIGHT_DP)
      .setAutoExpandBubble(false)
      .setSuppressNotification(true)
      .build()
    val again = NotificationCompat.Builder(context, shown.notification)
      .setBubbleMetadata(quiet)
      .setSilent(true)
      .build()
    try {
      NotificationManagerCompat.from(context).notify(roomId, 1, again)
    } catch (e: SecurityException) {
      android.util.Log.w("AlwyChat", "bubble: $e")
    }
  }

  private fun photo(base64: String?): Bitmap? {
    if (base64.isNullOrEmpty()) return null
    return try {
      val bytes = Base64.decode(base64, Base64.DEFAULT)
      BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let(::adaptive)
    } catch (e: Exception) {
      null
    }
  }

  /** Adaptive icons are cropped to the middle 72 of 108: the photo fills that circle. */
  private fun adaptive(source: Bitmap): Bitmap {
    val size = 216
    val out = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(out)
    val side = minOf(source.width, source.height)
    val crop = android.graphics.Rect(
      (source.width - side) / 2, (source.height - side) / 2,
      (source.width + side) / 2, (source.height + side) / 2,
    )
    val inset = size / 6
    canvas.drawBitmap(
      source, crop, android.graphics.Rect(inset, inset, size - inset, size - inset),
      Paint(Paint.FILTER_BITMAP_FLAG),
    )
    return out
  }

  /** No photo: the first letter on a colour picked from the name (like the app's avatars). */
  private fun letter(name: String): Bitmap {
    val size = 216
    val colors = intArrayOf(
      0xFF7A5CE6.toInt(), 0xFF1E9AA8.toInt(), 0xFFE0567A.toInt(),
      0xFFE58A2E.toInt(), 0xFF3F8EE8.toInt(), 0xFF3BA55C.toInt(),
    )
    val out = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(out)
    canvas.drawColor(colors[Math.floorMod(name.hashCode(), colors.size)])
    val initial = name.trim().codePoints().findFirst().let { if (it.isPresent) String(Character.toChars(it.asInt)) else "?" }
      .uppercase()
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
      color = Color.WHITE
      textSize = size * 0.3f
      textAlign = Paint.Align.CENTER
      typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    }
    val y = size / 2f - (paint.descent() + paint.ascent()) / 2
    canvas.drawText(initial, size / 2f, y, paint)
    return out
  }
}
