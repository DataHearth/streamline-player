package com.datahearth.player

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.RectF
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.util.Log
import android.view.View
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.text.CuesWithTiming
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.extractor.text.pgs.PgsParser
import io.github.peerless2012.ass.Ass
import io.github.peerless2012.ass.AssTexType
import java.io.DataInputStream
import java.io.File
import java.io.RandomAccessFile
import java.net.URL
import java.util.concurrent.atomic.AtomicBoolean

private const val TAG = "SubtitleOverlay"
private const val FRAME_MS = 33L

internal class Piece(val bitmap: Bitmap, val dest: RectF)

internal interface SubtitleSource {
    /** What to show at [timeMs], or null when what is on screen is still right. */
    fun render(timeMs: Long, screen: Rect, video: Rect, scale: Float): List<Piece>?
    fun close()
}

/**
 * Draws the subtitles that are pictures (PGS) or that only libass can lay out (ASS), between the
 * video and the WebView. These run on the player's clock here instead of as ExoPlayer text tracks
 * because the sync offset has to shift them, and ExoPlayer has no offset for a text track.
 */
@SuppressLint("ViewConstructor")
internal class SubtitleOverlay(context: Context, private val positionMs: () -> Long) : View(context) {
    @Volatile var videoRect = Rect()
    @Volatile var offsetMs = 0L
    @Volatile var scale = 1f

    private val main = Handler(Looper.getMainLooper())
    private val thread = HandlerThread("subtitles").apply { start() }
    private val worker = Handler(thread.looper)
    private val rendering = AtomicBoolean(false)
    private val paint = Paint(Paint.FILTER_BITMAP_FLAG)
    private var source: SubtitleSource? = null
    private var pieces = emptyList<Piece>()
    private var generation = 0

    private val tick = object : Runnable {
        override fun run() {
            if (rendering.compareAndSet(false, true)) {
                val time = positionMs() - offsetMs
                val screen = Rect(0, 0, width, height)
                val video = videoRect
                val s = scale
                worker.post {
                    val next = try {
                        source?.render(time, screen, video, s)
                    } catch (e: Exception) {
                        Log.e(TAG, "render failed", e)
                        null
                    } finally {
                        rendering.set(false)
                    }
                    if (next != null) main.post { pieces = next; invalidate() }
                }
            }
            main.postDelayed(this, FRAME_MS)
        }
    }

    /** Replaces the subtitles; [load] runs off the main thread and null turns them off. */
    fun show(load: (() -> SubtitleSource)?, onFailed: () -> Unit) {
        val current = ++generation
        main.removeCallbacks(tick)
        worker.removeCallbacksAndMessages(null)
        rendering.set(false)
        pieces = emptyList()
        invalidate()
        worker.post {
            source?.close()
            source = null
            if (load == null) return@post
            try {
                val next = load()
                main.post {
                    if (current == generation) {
                        worker.post { source = next }
                        main.post(tick)
                    } else {
                        worker.post { next.close() }
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "load failed", e)
                main.post { if (current == generation) onFailed() }
            }
        }
    }

    fun release() {
        main.removeCallbacks(tick)
        worker.post {
            source?.close()
            source = null
        }
        thread.quitSafely()
    }

    override fun onDraw(canvas: Canvas) {
        for (p in pieces) canvas.drawBitmap(p.bitmap, null, p.dest, paint)
    }
}

/** Downloads once: a track or a font doesn't change under the same URL. */
internal fun fetchCached(url: String, dir: File): File {
    dir.mkdirs()
    val file = File(dir, "${Integer.toHexString(url.hashCode())}-${url.substringAfterLast('/').substringBefore('?')}")
    if (file.exists()) return file
    val partial = File(dir, "${file.name}.part")
    URL(url).openStream().use { input -> partial.outputStream().use { input.copyTo(it) } }
    partial.renameTo(file)
    return file
}

internal class AssSource(track: File, fonts: List<File>) : SubtitleSource {
    private val ass = Ass()
    private val assTrack = ass.createTrack()
    private val renderer = ass.createRender()
    private var frame = Rect()
    private var frameScale = 0f

    init {
        for (font in fonts) ass.addFont(font.name, font.readBytes())
        assTrack.readBuffer(track.readBytes())
        renderer.setTrack(assTrack)
    }

    override fun render(timeMs: Long, screen: Rect, video: Rect, scale: Float): List<Piece>? {
        if (video.isEmpty) return null
        val resized = video != frame || scale != frameScale
        if (resized) {
            renderer.setStorageSize(video.width(), video.height())
            renderer.setFrameSize(video.width(), video.height())
            renderer.setFontScale(scale)
            frame = Rect(video)
            frameScale = scale
        }
        val rendered = renderer.renderFrame(timeMs.coerceAtLeast(0), AssTexType.BITMAP_RGBA) ?: return null
        if (!resized && rendered.changed == 0) return null
        return rendered.images.orEmpty().mapNotNull { tex ->
            val left = (video.left + tex.x).toFloat()
            val top = (video.top + tex.y).toFloat()
            tex.bitmap?.let { Piece(it, RectF(left, top, left + tex.w, top + tex.h)) }
        }
    }

    override fun close() {
        renderer.release()
        assTrack.release()
        ass.release()
    }
}

/**
 * A `.sup` file is the Matroska PGS samples with a 10-byte `PG`/pts/dts header on each segment, so
 * Media3's parser reads a display set once those are stripped. Sets are decoded when they come up:
 * a film's track runs to tens of MB.
 */
@androidx.annotation.OptIn(UnstableApi::class)
internal class PgsSource(file: File) : SubtitleSource {
    private class DisplaySet(val ptsMs: Long, val start: Long, val end: Long)

    private val data = RandomAccessFile(file, "r")
    private val parser = PgsParser()
    private val sets = ArrayList<DisplaySet>()
    private var planeWidth = 1920
    private var planeHeight = 1080
    private var shown = -1
    private var shownScreen = Rect()

    init {
        DataInputStream(file.inputStream().buffered()).use { input ->
            var offset = 0L
            var start = -1L
            var pts = 0L
            while (input.available() > 0) {
                if (input.readUnsignedShort() != MAGIC) error("not a PGS file at $offset")
                val segmentPts = input.readInt().toLong() and 0xFFFFFFFFL
                input.skipBytes(4)
                val type = input.readUnsignedByte()
                val size = input.readUnsignedShort()
                if (start < 0) {
                    start = offset
                    pts = segmentPts / 90
                }
                if (type == SEGMENT_COMPOSITION && size >= 4) {
                    planeWidth = input.readUnsignedShort()
                    planeHeight = input.readUnsignedShort()
                    input.skipFully(size - 4)
                } else {
                    input.skipFully(size)
                }
                offset += HEADER + size
                if (type == SEGMENT_END) {
                    sets.add(DisplaySet(pts, start, offset))
                    start = -1
                }
            }
        }
    }

    override fun render(timeMs: Long, screen: Rect, video: Rect, scale: Float): List<Piece>? {
        if (screen.isEmpty) return null
        var low = 0
        var high = sets.size - 1
        var index = -1
        while (low <= high) {
            val mid = (low + high) ushr 1
            if (sets[mid].ptsMs <= timeMs) {
                index = mid
                low = mid + 1
            } else {
                high = mid - 1
            }
        }
        if (index == shown && screen == shownScreen) return null
        shown = index
        shownScreen = Rect(screen)
        return if (index < 0) emptyList() else place(decode(sets[index]), screen)
    }

    private fun decode(set: DisplaySet): List<Cue> {
        val raw = ByteArray((set.end - set.start).toInt())
        data.seek(set.start)
        data.readFully(raw)
        val out = ByteArray(raw.size)
        var read = 0
        var written = 0
        while (read < raw.size) {
            val size = ((raw[read + 11].toInt() and 0xFF) shl 8) or (raw[read + 12].toInt() and 0xFF)
            val segment = 3 + size
            System.arraycopy(raw, read + 10, out, written, segment)
            written += segment
            read += HEADER + size
        }
        val cues = ArrayList<Cue>()
        parser.parse(out, 0, written, SubtitleParser.OutputOptions.allCues()) { c: CuesWithTiming -> cues.addAll(c.cues) }
        return cues
    }

    // Positions are relative to the PGS plane, which can be taller than a cropped video: it is
    // fitted to the screen, not to the picture, or subtitles authored for the bars land off them.
    private fun place(cues: List<Cue>, screen: Rect): List<Piece> {
        val k = minOf(screen.width().toFloat() / planeWidth, screen.height().toFloat() / planeHeight)
        val w = planeWidth * k
        val h = planeHeight * k
        val left = (screen.width() - w) / 2
        val top = (screen.height() - h) / 2
        return cues.mapNotNull { cue ->
            val bitmap = cue.bitmap ?: return@mapNotNull null
            val x = left + cue.position * w
            val y = top + cue.line * h
            Piece(bitmap, RectF(x, y, x + cue.size * w, y + cue.bitmapHeight * h))
        }
    }

    override fun close() = data.close()

    private fun DataInputStream.skipFully(count: Int) {
        var left = count
        while (left > 0) {
            val skipped = skipBytes(left)
            if (skipped <= 0) error("truncated PGS file")
            left -= skipped
        }
    }

    private companion object {
        const val MAGIC = 0x5047
        const val HEADER = 13
        const val SEGMENT_COMPOSITION = 0x16
        const val SEGMENT_END = 0x80
    }
}
