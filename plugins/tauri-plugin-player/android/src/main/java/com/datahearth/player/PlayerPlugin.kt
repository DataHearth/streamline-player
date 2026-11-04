package com.datahearth.player

import android.app.Activity
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.SurfaceView
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class LoadArgs {
    lateinit var url: String
}

@InvokeArg
class SeekArgs {
    var seconds: Double = 0.0
}

private const val TAG = "PlayerPlugin"
private const val POSITION_INTERVAL_MS = 500L

@TauriPlugin
class PlayerPlugin(private val activity: Activity) : Plugin(activity) {
    // ExoPlayer must be touched only from the thread that created it (main).
    // Tauri 2.12 happens to dispatch commands on main too, but that is not a
    // documented contract, so every access is posted to the main looper.
    private val main = Handler(Looper.getMainLooper())
    private var player: ExoPlayer? = null
    private var surfaceView: SurfaceView? = null
    private var resumeOnForeground = false

    private val positionTicker = object : Runnable {
        override fun run() {
            emitPosition()
            main.postDelayed(this, POSITION_INTERVAL_MS)
        }
    }

    override fun load(webView: WebView) {
        main.post { attach(webView) }
    }

    private fun attach(webView: WebView) {
        webView.setBackgroundColor(Color.TRANSPARENT)

        val content = webView.parent as? ViewGroup
            ?: activity.findViewById(android.R.id.content)
        val surface = SurfaceView(activity).apply {
            isFocusable = false
            isFocusableInTouchMode = false
        }
        content.addView(
            surface,
            0,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        surfaceView = surface

        val p = ExoPlayer.Builder(activity).build()
        p.setVideoSurfaceView(surface)
        p.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(playbackState: Int) {
                emitState()
                if (playbackState == Player.STATE_ENDED) logDecoderCounters()
            }
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                emitState()
                if (isPlaying) startTicker() else stopTicker()
            }
            override fun onPlayerError(error: PlaybackException) {
                Log.e(TAG, "playback error", error)
                trigger("error", JSObject().apply {
                    put("code", error.errorCodeName)
                    put("message", error.message ?: "")
                })
            }
        })
        p.addAnalyticsListener(droppedFramesLogger)
        player = p
        // Without this the first D-pad press only moves Android focus into the
        // WebView instead of reaching the page.
        webView.requestFocus()
        Log.i(TAG, "attached SurfaceView behind ${webView.javaClass.simpleName} in ${content.javaClass.simpleName}")
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    private val droppedFramesLogger = object : AnalyticsListener {
        override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
            Log.w(TAG, "dropped $droppedFrames video frames in ${elapsedMs}ms")
        }
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    private fun logDecoderCounters() {
        val c = player?.videoDecoderCounters ?: return
        c.ensureUpdated()
        Log.i(TAG, "decoder counters: rendered=${c.renderedOutputBufferCount} dropped=${c.droppedBufferCount} skipped=${c.skippedOutputBufferCount} maxConsecutiveDropped=${c.maxConsecutiveDroppedBufferCount}")
    }

    private fun startTicker() {
        main.removeCallbacks(positionTicker)
        main.post(positionTicker)
    }

    private fun stopTicker() {
        main.removeCallbacks(positionTicker)
        emitPosition()
    }

    private fun emitPosition() {
        val p = player ?: return
        trigger("position", JSObject().apply {
            put("position", p.currentPosition / 1000.0)
            put("duration", if (p.duration > 0) p.duration / 1000.0 else 0.0)
        })
    }

    private fun emitState() {
        val p = player ?: return
        val state = when {
            p.playbackState == Player.STATE_ENDED -> "ended"
            p.playbackState == Player.STATE_BUFFERING -> "buffering"
            p.playbackState == Player.STATE_IDLE -> "idle"
            p.isPlaying -> "playing"
            else -> "paused"
        }
        trigger("state", JSObject().apply { put("state", state) })
    }

    private fun onPlayer(invoke: Invoke, block: (ExoPlayer) -> Unit) {
        main.post {
            val p = player
            if (p == null) {
                invoke.reject("player not attached")
            } else {
                block(p)
                invoke.resolve()
            }
        }
    }

    @Command
    fun load(invoke: Invoke) {
        val args = invoke.parseArgs(LoadArgs::class.java)
        onPlayer(invoke) {
            it.setMediaItem(MediaItem.fromUri(args.url))
            it.prepare()
        }
    }

    @Command
    fun capabilities(invoke: Invoke) = invoke.resolve(Capabilities.build(activity))

    @Command
    fun play(invoke: Invoke) = onPlayer(invoke) { it.play() }

    @Command
    fun pause(invoke: Invoke) = onPlayer(invoke) { it.pause() }

    @Command
    fun seek(invoke: Invoke) {
        val args = invoke.parseArgs(SeekArgs::class.java)
        onPlayer(invoke) {
            it.seekTo((args.seconds * 1000).toLong().coerceAtLeast(0))
            emitPosition()
        }
    }

    override fun onPause(activity: AppCompatActivity) {
        main.post {
            val p = player ?: return@post
            resumeOnForeground = p.playWhenReady
            p.pause()
            Log.i(TAG, "activity paused at ${p.currentPosition}ms, resume=$resumeOnForeground")
        }
    }

    override fun onResume(activity: AppCompatActivity) {
        main.post {
            Log.i(TAG, "activity resumed, resume=$resumeOnForeground")
            if (resumeOnForeground) player?.play()
            resumeOnForeground = false
        }
    }

    override fun onDestroy(activity: AppCompatActivity) {
        main.post {
            stopTicker()
            player?.release()
            player = null
        }
    }
}
