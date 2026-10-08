package dev.quadra.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Holds the system VPN. It creates the tunnel device and hands its file descriptor to the Rust
 * side, which reads the packets; nothing travels through Kotlin. The app itself is left out of
 * the VPN, so the core's own connections to the server do not loop back into the tunnel.
 */
class QuadraVpnService : VpnService() {
    companion object {
        const val ACTION_START = "dev.quadra.vpn.START"
        const val ACTION_STOP = "dev.quadra.vpn.STOP"
        const val EXTRA_MTU = "mtu"
        const val EXTRA_ONLY = "only"
        const val EXTRA_APPS = "apps"
        private const val CHANNEL = "quadra-vpn"
        private const val NOTIFICATION = 1

        /** Called once with the tunnel's descriptor, or with -1 and the reason it failed. */
        @Volatile
        var onReady: ((Int, String) -> Unit)? = null
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return START_NOT_STICKY
        }
        val answer = onReady
        onReady = null
        try {
            foreground()
            val builder = Builder()
                .setSession("Quadra")
                .setMtu(intent?.getIntExtra(EXTRA_MTU, 1500) ?: 1500)
                .addAddress("172.19.0.1", 30)
                .addRoute("0.0.0.0", 0)
                .addAddress("fdfe:dcba:9876::1", 126)
                .addRoute("::", 0)
                .addDnsServer("1.1.1.1")
                .addDnsServer("8.8.8.8")
            // Which apps the VPN takes. Quadra itself always stays outside: its own connection
            // to the server must not loop back into the tunnel.
            val apps = (intent?.getStringArrayExtra(EXTRA_APPS) ?: arrayOf()).filter { it.isNotBlank() && it != packageName }
            if (intent?.getBooleanExtra(EXTRA_ONLY, false) == true && apps.isNotEmpty()) {
                var taken = 0
                for (app in apps) {
                    try { builder.addAllowedApplication(app); taken++ } catch (_: Exception) { }
                }
                // none of them is installed any more: fall back to everything but ourselves
                if (taken == 0) builder.addDisallowedApplication(packageName)
            } else {
                builder.addDisallowedApplication(packageName)
                for (app in apps) {
                    try { builder.addDisallowedApplication(app) } catch (_: Exception) { }
                }
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) builder.setMetered(false)
            val tunnel = builder.establish() ?: throw IllegalStateException("система не выдала VPN: разрешение не получено")
            // from here on the descriptor belongs to the Rust side, which closes it when done
            answer?.invoke(tunnel.detachFd(), "")
        } catch (e: Exception) {
            answer?.invoke(-1, e.message ?: e.toString())
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    /** The user switched the VPN off in the system settings, or another VPN took over. */
    override fun onRevoke() {
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        super.onRevoke()
    }

    private fun foreground() {
        val manager = getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(NotificationChannel(CHANNEL, "VPN", NotificationManager.IMPORTANCE_LOW))
        }
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
        val notification: Notification = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle("Quadra")
            .setContentText("VPN включён")
            .setOngoing(true)
            .setContentIntent(open)
            .build()
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED else 0
        ServiceCompat.startForeground(this, NOTIFICATION, notification, type)
    }
}
