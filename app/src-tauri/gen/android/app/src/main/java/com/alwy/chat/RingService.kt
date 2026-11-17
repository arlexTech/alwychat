package com.alwy.chat

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.content.ContextCompat

/**
 * Holds an incoming call's notification while it rings (30 s), as a short foreground service.
 * Without it, phones like OnePlus freeze the app as soon as the push is handled: the vibration
 * loop, the "is the caller still there?" checks and the end of the ring never ran, and the call
 * stayed on screen after the caller hung up. `IncomingCall` drives the ring; this only keeps the
 * app awake and owns the notification.
 */
class RingService : Service() {
  companion object {
    const val ID = 4243
    /** The notification to show (set before starting the service). */
    @Volatile private var pending: Notification? = null

    /** Show `notification` through the service. False if Android doesn't allow it here. */
    fun start(context: Context, notification: Notification): Boolean {
      pending = notification
      return try {
        ContextCompat.startForegroundService(context, Intent(context, RingService::class.java))
        true
      } catch (e: Exception) {
        Log.w("AlwyChat", "ring service refused: $e")
        false
      }
    }

    fun stop(context: Context) {
      context.stopService(Intent(context, RingService::class.java))
    }
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val notification = pending ?: return START_NOT_STICKY.also { stopSelf() }
    try {
      if (Build.VERSION.SDK_INT >= 34) {
        startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SHORT_SERVICE)
      } else {
        startForeground(ID, notification)
      }
    } catch (e: Exception) {
      // Not allowed from the background here: the plain notification instead.
      Log.w("AlwyChat", "ring service: $e")
      IncomingCall.fallback(this, notification)
      stopSelf()
    }
    return START_NOT_STICKY
  }

  // A short service must end within ~3 minutes; a ring lasts 2 at most.
  override fun onTimeout(startId: Int) {
    stopSelf()
  }

  override fun onTimeout(startId: Int, fgsType: Int) {
    stopSelf()
  }
}
