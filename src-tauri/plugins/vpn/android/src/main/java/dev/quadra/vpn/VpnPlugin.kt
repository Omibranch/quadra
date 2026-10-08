package dev.quadra.vpn

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.net.VpnService
import android.os.Build
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class StartArgs {
    var mtu: Int = 1500
}

@InvokeArg
class OpenArgs {
    lateinit var url: String
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pending: StartArgs? = null

    /** Paths and device facts the Rust side cannot find out on its own. */
    @Command
    fun info(invoke: Invoke) {
        try {
            unpackData()
            val result = JSObject()
            result.put("nativeDir", activity.applicationInfo.nativeLibraryDir)
            result.put("filesDir", activity.filesDir.absolutePath)
            result.put("model", listOf(Build.MANUFACTURER, Build.MODEL).filter { !it.isNullOrBlank() }.joinToString(" "))
            result.put("release", Build.VERSION.RELEASE ?: "")
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject(e.message ?: e.toString())
        }
    }

    /** The core's data files (geoip.dat, geosite.dat) ship as assets; the core needs them as files. */
    private fun unpackData() {
        val target = File(activity.filesDir, "xray")
        target.mkdirs()
        // unpacked once per installed build: the stamp is the time the package was last updated
        val installed = activity.packageManager.getPackageInfo(activity.packageName, 0).lastUpdateTime.toString()
        val stamp = File(target, ".stamp")
        if (stamp.exists() && stamp.readText() == installed) return
        for (name in activity.assets.list("xray") ?: emptyArray()) {
            activity.assets.open("xray/$name").use { input -> File(target, name).outputStream().use { input.copyTo(it) } }
        }
        stamp.writeText(installed)
    }

    /** Brings the VPN up; the first time the system asks the user to allow it. */
    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(StartArgs::class.java)
        val consent = VpnService.prepare(activity)
        if (consent != null) {
            pending = args
            startActivityForResult(invoke, consent, "consentResult")
        } else {
            launch(invoke, args)
        }
    }

    @ActivityCallback
    fun consentResult(invoke: Invoke, result: ActivityResult) {
        val args = pending ?: StartArgs()
        pending = null
        if (result.resultCode == Activity.RESULT_OK) launch(invoke, args)
        else invoke.reject("разрешение на VPN не выдано")
    }

    private fun launch(invoke: Invoke, args: StartArgs) {
        QuadraVpnService.onReady = { fd, error ->
            if (fd >= 0) {
                val result = JSObject()
                result.put("fd", fd)
                invoke.resolve(result)
            } else {
                invoke.reject(error)
            }
        }
        val intent = Intent(activity, QuadraVpnService::class.java)
            .setAction(QuadraVpnService.ACTION_START)
            .putExtra(QuadraVpnService.EXTRA_MTU, args.mtu)
        try {
            ContextCompat.startForegroundService(activity, intent)
        } catch (e: Exception) {
            QuadraVpnService.onReady = null
            invoke.reject(e.message ?: e.toString())
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        try {
            activity.startService(Intent(activity, QuadraVpnService::class.java).setAction(QuadraVpnService.ACTION_STOP))
        } catch (_: Exception) {
        }
        invoke.resolve()
    }

    @Command
    fun openUrl(invoke: Invoke) {
        val args = invoke.parseArgs(OpenArgs::class.java)
        try {
            activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(args.url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: e.toString())
        }
    }
}
