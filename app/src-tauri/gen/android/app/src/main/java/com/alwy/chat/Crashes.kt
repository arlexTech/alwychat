package com.alwy.chat

import android.content.Context
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/**
 * Java/Kotlin crashes go to the same `last_crash.txt` as Rust panics (alwy_core::logs, in the
 * data folder), so the next start offers to send them in a problem report.
 */
object Crashes {
  @Volatile private var watching = false

  fun watch(context: Context) {
    if (watching) return
    watching = true
    val file = File(context.applicationContext.dataDir, "last_crash.txt")
    val previous = Thread.getDefaultUncaughtExceptionHandler()
    Thread.setDefaultUncaughtExceptionHandler { thread, error ->
      try {
        val time = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.ROOT)
          .apply { timeZone = TimeZone.getTimeZone("UTC") }
          .format(Date())
        file.writeText("$time Java crash in thread ${thread.name}:\n${Log.getStackTraceString(error)}")
      } catch (_: Throwable) {
      }
      previous?.uncaughtException(thread, error)
    }
  }
}
