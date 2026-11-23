package com.alwy.chat

/** Asks Rust for an activity's webview window (`bubbles.rs`). */
object NativeBubble {
  init {
    System.loadLibrary("alwychat_lib")
  }

  /** `activity` (local class name) with WryActivity's `id` was created; `room`: a bubble's chat ("" for the app). */
  @JvmStatic
  external fun window(activity: String, id: Int, room: String)
}
