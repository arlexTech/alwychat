package com.alwy.chat

import android.app.Activity
import androidx.appcompat.app.AppCompatActivity
import app.tauri.plugin.PluginManager
import java.lang.ref.WeakReference

/**
 * The app's activity in front: MainActivity or a chat bubble ([BubbleActivity]). Tauri's plugins
 * are tied to the first activity of the process, which may be a bubble that's long gone or
 * folded; our plugins and Tauri's pickers act in the one in front instead.
 */
object Front {
  private var current: WeakReference<Activity>? = null
  private var mainRef: WeakReference<MainActivity>? = null

  val activity: Activity?
    get() = current?.get()?.takeIf { !it.isFinishing && !it.isDestroyed }
      ?: main

  /** The app's own activity, if it's alive (picture-in-picture, lock screen, screen on…). */
  val main: MainActivity?
    get() = mainRef?.get()?.takeIf { !it.isFinishing && !it.isDestroyed }

  fun resumed(activity: AppCompatActivity) {
    current = WeakReference(activity)
    if (activity is MainActivity) mainRef = WeakReference(activity)
    // Tauri's own launchers (permission prompts, activity results) follow too.
    PluginManager.activity = activity
  }

  fun created(activity: MainActivity) {
    mainRef = WeakReference(activity)
  }
}
