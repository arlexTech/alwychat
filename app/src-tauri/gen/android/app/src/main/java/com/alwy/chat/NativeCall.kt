package com.alwy.chat

/** The Rust side of the call notification (`android::Java_com_alwy_chat_NativeCall_action`). */
object NativeCall {
  /** A button of the call notification was tapped ("hang_up", "toggle_mic"): the UI acts on it. */
  @JvmStatic
  external fun action(name: String)
}
