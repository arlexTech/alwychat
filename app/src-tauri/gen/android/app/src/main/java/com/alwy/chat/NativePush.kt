package com.alwy.chat

import android.content.Context

/** The Rust side of push handling (`android::Java_com_alwy_chat_NativePush_notification`). */
object NativePush {
  init {
    // Already loaded when the UI runs; a push can start the process without it.
    System.loadLibrary("alwychat_lib")
  }

  /** The notification for a pushed event as JSON (`PushNotification`), or "" for none. Blocks. */
  @JvmStatic
  external fun readChats(context: Context, dataDir: String, roomIds: String): String

  @JvmStatic
  external fun notification(context: Context, dataDir: String, roomId: String, eventId: String): String

  /**
   * A new UnifiedPush endpoint for this device (`endpoint` empty: none any more); `old` is the
   * previous one, whose pusher is removed. Registers with the homeserver. Blocks.
   */
  @JvmStatic
  external fun setEndpoint(context: Context, dataDir: String, endpoint: String, old: String)

  /**
   * A message notification's Reply (`text`) or Mark as read (`text` empty), up to `eventId`.
   * Returns "" when done, else the error. Blocks.
   */
  @JvmStatic
  external fun answer(context: Context, dataDir: String, roomId: String, eventId: String, text: String): String

  /** Decline the call rung by `eventId` (from the incoming-call notification). Blocks. */
  @JvmStatic
  external fun decline(context: Context, dataDir: String, roomId: String, eventId: String)
}
