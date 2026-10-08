package dev.quadra.vpn

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.net.VpnService
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.util.Base64
import android.view.View
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.ByteArrayOutputStream
import java.io.File

@InvokeArg
class StartArgs {
    var mtu: Int = 1500
    /** true: only the listed apps go through the VPN; false: all but the listed ones. */
    var only: Boolean = false
    var apps: Array<String> = arrayOf()
}

@InvokeArg
class OpenArgs {
    lateinit var url: String
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pending: StartArgs? = null

    /**
     * The window is drawn edge to edge, and the webview knows nothing of the status bar, the
     * navigation buttons or the keyboard. Keep the page clear of all three.
     */
    override fun load(webView: WebView) {
        val background = 0xFF090C0B.toInt()
        val root = activity.findViewById<View>(android.R.id.content)
        activity.window.decorView.setBackgroundColor(background)
        root.setBackgroundColor(background)
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime()
            )
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        ViewCompat.requestApplyInsets(root)
        // light icons on our dark bars
        WindowInsetsControllerCompat(activity.window, root).apply {
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
        }
    }

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
            .putExtra(QuadraVpnService.EXTRA_ONLY, args.only)
            .putExtra(QuadraVpnService.EXTRA_APPS, args.apps)
        try {
            ContextCompat.startForegroundService(activity, intent)
        } catch (e: Exception) {
            QuadraVpnService.onReady = null
            invoke.reject(e.message ?: e.toString())
        }
    }

    /** Apps with a launcher icon, for the exclusions list: package, label and a small icon. */
    @Command
    fun listApps(invoke: Invoke) {
        Thread {
            try {
                val pm = activity.packageManager
                val launcher = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
                val seen = HashSet<String>()
                val apps = JSArray()
                for (info in pm.queryIntentActivities(launcher, 0).sortedBy { it.loadLabel(pm).toString().lowercase() }) {
                    val pkg = info.activityInfo.packageName
                    if (pkg == activity.packageName || !seen.add(pkg)) continue
                    val item = JSObject()
                    item.put("id", pkg)
                    item.put("name", info.loadLabel(pm).toString())
                    try {
                        val size = 72
                        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
                        val drawable = info.loadIcon(pm)
                        drawable.setBounds(0, 0, size, size)
                        drawable.draw(Canvas(bitmap))
                        val bytes = ByteArrayOutputStream()
                        bitmap.compress(Bitmap.CompressFormat.PNG, 100, bytes)
                        item.put("icon", "data:image/png;base64," + Base64.encodeToString(bytes.toByteArray(), Base64.NO_WRAP))
                    } catch (_: Exception) {
                    }
                    apps.put(item)
                }
                val result = JSObject()
                result.put("apps", apps)
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
        }.start()
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
