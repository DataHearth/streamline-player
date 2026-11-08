package com.datahearth.player

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.Rect
import android.os.Handler
import android.os.Looper
import android.speech.RecognizerIntent
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.SurfaceView
import android.view.ViewGroup
import android.view.Window
import android.webkit.WebView
import java.util.concurrent.atomic.AtomicLong
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResult
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaLibraryInfo
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.TransferListener
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.upstream.DefaultBandwidthMeter
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioRendererEventListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.MediaCodecAudioRenderer
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import app.tauri.annotation.ActivityCallback
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

@InvokeArg
class SpeedArgs {
    var speed: Double = 1.0
}

@InvokeArg
class SubtitleArgs {
    var kind: String? = null
    var url: String? = null
    var fonts: Array<String> = emptyArray()
}

@InvokeArg
class TimingArgs {
    var subtitleOffset: Double = 0.0
    var subtitleScale: Double = 1.0
    var audioDelay: Double = 0.0
}

/**
 * Audio delay: the audio renderer is ExoPlayer's master clock, and video is rendered against it.
 * Reporting a later position shows the picture earlier, so the sound plays later than the picture.
 */
@androidx.annotation.OptIn(UnstableApi::class)
private class DelayableAudioRenderer(
    context: Context,
    selector: MediaCodecSelector,
    fallback: Boolean,
    handler: Handler?,
    listener: AudioRendererEventListener?,
    sink: AudioSink,
    private val delayUs: () -> Long,
) : MediaCodecAudioRenderer(context, selector, fallback, handler, listener, sink) {
    override fun getPositionUs(): Long = super.getPositionUs() + delayUs()
}

private const val TAG = "PlayerPlugin"
private const val POSITION_INTERVAL_MS = 500L
// DefaultBandwidthMeter answers with its built-in guess for the network type until it has seen this much.
private const val METER_TRUSTED_BYTES = 512 * 1024L
private const val READS_WINDOW_MS = 2000L
// Replacing a direct stream closes its connection, and the meter counts all the time it sat open.
private const val METER_QUIET_AFTER_LOAD_MS = 1000L

@TauriPlugin
class PlayerPlugin(private val activity: Activity) : Plugin(activity) {
    // ExoPlayer must be touched only from the thread that created it (main).
    // Tauri 2.12 happens to dispatch commands on main too, but that is not a
    // documented contract, so every access is posted to the main looper.
    private val main = Handler(Looper.getMainLooper())
    private var player: ExoPlayer? = null
    private var surfaceView: SurfaceView? = null
    private var overlay: SubtitleOverlay? = null
    @Volatile private var audioDelayUs = 0L
    private var resumeOnForeground = false
    private var meteredBytes = 0L
    private var meterQuietUntil = 0L
    private val bytesRead = AtomicLong()
    private var windowStart = 0L
    private var loadedWholeWindow = false

    // The meter only learns when a transfer closes, and a direct stream holds one connection open
    // across its pauses. Bytes per window measure the link as long as loading never paused inside it.
    @androidx.annotation.OptIn(UnstableApi::class)
    private val reads = object : TransferListener {
        override fun onTransferInitializing(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
        override fun onTransferStart(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
        override fun onTransferEnd(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
        override fun onBytesTransferred(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean, bytesTransferred: Int) {
            if (isNetwork) bytesRead.addAndGet(bytesTransferred.toLong())
        }
    }

    private val readsTicker = object : Runnable {
        override fun run() {
            val now = System.currentTimeMillis()
            val bytes = bytesRead.getAndSet(0)
            if (loadedWholeWindow && bytes > 0) emitBandwidth(bytes * 8000 / (now - windowStart), "reads")
            windowStart = now
            loadedWholeWindow = player?.isLoading == true
            main.postDelayed(this, READS_WINDOW_MS)
        }
    }
    private val bandwidth by lazy { DefaultBandwidthMeter.getSingletonInstance(activity) }

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
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT, Gravity.CENTER),
        )
        surfaceView = surface
        val subtitles = SubtitleOverlay(activity) { player?.currentPosition ?: 0L }
        content.addView(
            subtitles,
            1,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        overlay = subtitles
        content.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> fitVideo() }

        val p = ExoPlayer.Builder(activity, renderers())
            .setBandwidthMeter(bandwidth)
            .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(activity).setTransferListener(reads)))
            .build()
        p.setVideoSurfaceView(surface)
        p.addListener(object : Player.Listener {
            override fun onVideoSizeChanged(videoSize: VideoSize) = fitVideo()
            override fun onIsLoadingChanged(isLoading: Boolean) {
                loadedWholeWindow = false
            }
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
        p.addAnalyticsListener(bandwidthEvents)
        player = p
        main.post(readsTicker)
        // The remote's Back belongs to the player's own stack: menu, controls, then leave.
        (activity as AppCompatActivity).onBackPressedDispatcher.addCallback(
            activity,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() = trigger("back", JSObject())
            },
        )
        // The remote's mic key opens our search instead of Android's global one. Caught on the
        // window: the WebView overrides dispatchKeyEvent, so a key listener on it never runs.
        val inner = activity.window.callback
        activity.window.callback = object : Window.Callback by inner {
            override fun dispatchKeyEvent(event: KeyEvent): Boolean {
                if (event.keyCode != KeyEvent.KEYCODE_SEARCH && event.keyCode != KeyEvent.KEYCODE_VOICE_ASSIST) return inner.dispatchKeyEvent(event)
                if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) trigger("voiceKey", JSObject())
                return true
            }
        }
        // Without this the first D-pad press only moves Android focus into the
        // WebView instead of reaching the page.
        webView.requestFocus()
        Log.i(TAG, "attached SurfaceView behind ${webView.javaClass.simpleName} in ${content.javaClass.simpleName}")
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    private fun renderers() = object : DefaultRenderersFactory(activity) {
        override fun buildAudioRenderers(
            context: Context,
            extensionRendererMode: Int,
            mediaCodecSelector: MediaCodecSelector,
            enableDecoderFallback: Boolean,
            audioSink: AudioSink,
            eventHandler: Handler,
            eventListener: AudioRendererEventListener,
            out: ArrayList<Renderer>,
        ) {
            out.add(DelayableAudioRenderer(context, mediaCodecSelector, enableDecoderFallback, eventHandler, eventListener, audioSink) { audioDelayUs })
        }
    }

    // ExoPlayer stretches to its surface, so the surface takes the picture's shape, centered.
    private fun fitVideo() {
        val surface = surfaceView ?: return
        val parent = surface.parent as? ViewGroup ?: return
        val video = player?.videoSize ?: return
        val (screenW, screenH) = parent.width to parent.height
        if (video.width == 0 || video.height == 0 || screenW == 0 || screenH == 0) return
        val aspect = video.width * video.pixelWidthHeightRatio / video.height
        val (w, h) = if (aspect >= screenW.toFloat() / screenH) {
            screenW to Math.round(screenW / aspect)
        } else {
            Math.round(screenH * aspect) to screenH
        }
        val left = (screenW - w) / 2
        val top = (screenH - h) / 2
        overlay?.videoRect = Rect(left, top, left + w, top + h)
        if (surface.layoutParams.width != w || surface.layoutParams.height != h) {
            surface.layoutParams = FrameLayout.LayoutParams(w, h, Gravity.CENTER)
        }
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    private val droppedFramesLogger = object : AnalyticsListener {
        override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
            Log.w(TAG, "dropped $droppedFrames video frames in ${elapsedMs}ms")
        }
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    private val bandwidthEvents = object : AnalyticsListener {
        override fun onBandwidthEstimate(eventTime: AnalyticsListener.EventTime, totalLoadTimeMs: Int, totalBytesLoaded: Long, bitrateEstimate: Long) {
            meteredBytes += totalBytesLoaded
            if (meteredBytes >= METER_TRUSTED_BYTES && System.currentTimeMillis() >= meterQuietUntil) emitBandwidth(bitrateEstimate, "meter")
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

    private fun emitBandwidth(bitsPerSecond: Long, source: String) {
        trigger("bandwidth", JSObject().apply {
            put("bitrate", bitsPerSecond)
            put("source", source)
        })
    }

    private fun emitPosition() {
        val p = player ?: return
        trigger("position", JSObject().apply {
            put("position", p.currentPosition / 1000.0)
            put("buffered", p.bufferedPosition / 1000.0)
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
            meterQuietUntil = System.currentTimeMillis() + METER_QUIET_AFTER_LOAD_MS
            it.setMediaItem(MediaItem.fromUri(args.url))
            it.prepare()
        }
    }

    @Command
    fun stop(invoke: Invoke) = onPlayer(invoke) {
        it.stop()
        it.clearMediaItems()
    }

    /** Android's own listening dialog: it needs no microphone permission of ours. */
    @Command
    fun voice(invoke: Invoke) {
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            .putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        try {
            startActivityForResult(invoke, intent, "voiceResult")
        } catch (e: ActivityNotFoundException) {
            invoke.reject("no speech recognizer on this device")
        }
    }

    @ActivityCallback
    private fun voiceResult(invoke: Invoke, result: ActivityResult) {
        val text = result.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()
        invoke.resolve(JSObject().apply { put("text", text ?: "") })
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

    @Command
    fun speed(invoke: Invoke) {
        val args = invoke.parseArgs(SpeedArgs::class.java)
        onPlayer(invoke) { it.playbackParameters = PlaybackParameters(args.speed.toFloat()) }
    }

    @androidx.annotation.OptIn(UnstableApi::class)
    @Command
    fun stats(invoke: Invoke) {
        main.post {
            val p = player
            if (p == null) {
                invoke.reject("player not attached")
                return@post
            }
            val frames = p.videoDecoderCounters?.also { it.ensureUpdated() }
            invoke.resolve(JSObject().apply {
                put("version", MediaLibraryInfo.VERSION)
                put("buffer", (p.bufferedPosition - p.currentPosition).coerceAtLeast(0) / 1000.0)
                put("videoBitrate", p.videoFormat?.bitrate ?: -1)
                put("audioBitrate", p.audioFormat?.bitrate ?: -1)
                put("rendered", frames?.renderedOutputBufferCount ?: 0)
                put("dropped", frames?.droppedBufferCount ?: 0)
                put("displayHz", activity.windowManager.defaultDisplay.refreshRate.toDouble())
            })
        }
    }

    @Command
    fun subtitle(invoke: Invoke) {
        val args = invoke.parseArgs(SubtitleArgs::class.java)
        val url = args.url
        val dir = activity.cacheDir.resolve("subtitles")
        val load: (() -> SubtitleSource)? = when {
            url == null -> null
            args.kind == "ass" -> { -> AssSource(fetchCached(url, dir), args.fonts.map { fetchCached(it, dir) }) }
            args.kind == "pgs" -> { -> PgsSource(fetchCached(url, dir)) }
            else -> null
        }
        main.post {
            overlay?.show(load) { trigger("subtitleFailed", JSObject()) }
            invoke.resolve()
        }
    }

    @Command
    fun timing(invoke: Invoke) {
        val args = invoke.parseArgs(TimingArgs::class.java)
        overlay?.offsetMs = (args.subtitleOffset * 1000).toLong()
        overlay?.scale = args.subtitleScale.toFloat()
        audioDelayUs = (args.audioDelay * 1000).toLong()
        invoke.resolve()
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
            main.removeCallbacks(readsTicker)
            player?.release()
            player = null
            overlay?.release()
            overlay = null
        }
    }
}
