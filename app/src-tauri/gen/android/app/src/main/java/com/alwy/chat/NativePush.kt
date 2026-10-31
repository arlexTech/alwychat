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
  external fun notification(context: Context, dataDir: String, roomId: String, eventId: String): String
}
