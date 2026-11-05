package com.datahearth.player

import android.content.Context
import android.hardware.display.DisplayManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.media.MediaCodecInfo
import android.media.MediaCodecInfo.CodecProfileLevel as P
import android.media.MediaCodecList
import android.os.Build
import android.view.Display
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioCapabilities
import app.tauri.plugin.JSObject
import org.json.JSONArray

// Builds the engine's capabilities document (engine/src/caps.rs). Names and
// levels are ffprobe's: HEVC level 5.1 is 153, H.264 4.1 is 41, AV1 uses seq_level_idx.
object Capabilities {
    private val H264_PROFILES = mapOf(
        P.AVCProfileBaseline to "baseline", P.AVCProfileConstrainedBaseline to "constrained baseline",
        P.AVCProfileMain to "main", P.AVCProfileHigh to "high", P.AVCProfileConstrainedHigh to "high",
        P.AVCProfileHigh10 to "high 10",
    )
    private val H264_LEVELS = mapOf(
        P.AVCLevel3 to 30, P.AVCLevel31 to 31, P.AVCLevel32 to 32, P.AVCLevel4 to 40, P.AVCLevel41 to 41,
        P.AVCLevel42 to 42, P.AVCLevel5 to 50, P.AVCLevel51 to 51, P.AVCLevel52 to 52, P.AVCLevel6 to 60,
        P.AVCLevel61 to 61, P.AVCLevel62 to 62,
    )
    private val HEVC_PROFILES = mapOf(
        P.HEVCProfileMain to "main", P.HEVCProfileMain10 to "main 10", P.HEVCProfileMain10HDR10 to "main 10",
        P.HEVCProfileMain10HDR10Plus to "main 10",
    )

    // Main and High tier constants alternate bit by bit from level 1 up.
    private val HEVC_LEVELS = listOf(30, 60, 63, 90, 93, 120, 123, 150, 153, 156, 180, 183, 186)
    private val AV1_PROFILES = mapOf(
        P.AV1ProfileMain8 to "main", P.AV1ProfileMain10 to "main", P.AV1ProfileMain10HDR10 to "main",
        P.AV1ProfileMain10HDR10Plus to "main",
    )
    private val DV_PROFILES = mapOf(
        P.DolbyVisionProfileDvheStn to 5, P.DolbyVisionProfileDvheDtb to 7, P.DolbyVisionProfileDvheSt to 8,
        P.DolbyVisionProfileDvavSe to 9, P.DolbyVisionProfileDvav110 to 10,
    )
    private val AUDIO = mapOf(
        "audio/mp4a-latm" to "aac", "audio/mpeg" to "mp3", "audio/ac3" to "ac3", "audio/eac3" to "eac3",
        "audio/eac3-joc" to "eac3", "audio/flac" to "flac", "audio/opus" to "opus", "audio/vorbis" to "vorbis",
        "audio/vnd.dts" to "dts", "audio/vnd.dts.hd" to "dts", "audio/true-hd" to "truehd",
    )
    private val PASSTHROUGH = mapOf(
        C.ENCODING_AC3 to "ac3", C.ENCODING_E_AC3 to "eac3", C.ENCODING_E_AC3_JOC to "eac3",
        C.ENCODING_DTS to "dts", C.ENCODING_DTS_HD to "dts", C.ENCODING_DOLBY_TRUEHD to "truehd",
    )

    fun build(context: Context): JSObject {
        val codecs = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.filter { !it.isEncoder }
        val modes = defaultDisplay(context).supportedModes.toList()
        val video = JSONArray()
        videoCap(codecs, modes, "video/avc", "h264", H264_PROFILES) { H264_LEVELS[it] }?.let(video::put)
        videoCap(codecs, modes, "video/hevc", "hevc", HEVC_PROFILES) { hevcLevel(it) }?.let(video::put)
        videoCap(codecs, modes, "video/av01", "av1", AV1_PROFILES) { Integer.numberOfTrailingZeros(it) }?.let(video::put)

        val dv = decoders(codecs, "video/dolby-vision")
            .flatMap { c -> c.getCapabilitiesForType("video/dolby-vision").profileLevels.mapNotNull { DV_PROFILES[it.profile] } }
            .toSortedSet()
        val formats = hdrTypes(context).filter { it != "dolby_vision" || dv.isNotEmpty() }

        return JSObject().apply {
            put("video", video)
            put("hdr", JSObject().apply {
                put("formats", JSONArray(formats))
                put("dv_profiles", JSONArray(dv.toList()))
            })
            put("audio", audio(context, codecs))
            put("containers", JSONArray(listOf("mp4", "mkv")))
            put("subtitles", JSONArray(listOf("webvtt", "srt", "ass", "pgs")))
            put("hls_keyframe_exact", false)
        }
    }

    private fun defaultDisplay(context: Context): Display =
        context.getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY)

    private fun hevcLevel(constant: Int): Int? = HEVC_LEVELS.getOrNull(Integer.numberOfTrailingZeros(constant) / 2)

    // Hardware decoders when the device has one for the type; the emulator only has software ones.
    private fun decoders(all: List<MediaCodecInfo>, mime: String): List<MediaCodecInfo> {
        val forMime = all.filter { c -> c.supportedTypes.any { it.equals(mime, ignoreCase = true) } }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return forMime
        return forMime.filter { it.isHardwareAccelerated }.ifEmpty { forMime }
    }

    private fun videoCap(
        all: List<MediaCodecInfo>, modes: List<Display.Mode>, mime: String, name: String,
        profiles: Map<Int, String>, level: (Int) -> Int?,
    ): JSObject? {
        val found = decoders(all, mime)
        if (found.isEmpty()) return null
        val caps = found.map { it.getCapabilitiesForType(mime) }
        val names = caps.flatMap { c -> c.profileLevels.mapNotNull { profiles[it.profile] } }.toSortedSet()
        val maxLevel = caps.flatMap { c -> c.profileLevels.mapNotNull { level(it.level) } }.maxOrNull()
        val v = caps.mapNotNull { it.videoCapabilities }
        return JSObject().apply {
            put("codec", name)
            put("profiles", JSONArray(names.toList()))
            put("max_level", maxLevel)
            put("max_width", minOf(v.maxOf { it.supportedWidths.upper }, modes.maxOf { it.physicalWidth }))
            put("max_height", minOf(v.maxOf { it.supportedHeights.upper }, modes.maxOf { it.physicalHeight }))
            put("max_fps", minOf(v.maxOf { it.supportedFrameRates.upper.toDouble() }, modes.maxOf { it.refreshRate.toDouble() }))
        }
    }

    @Suppress("DEPRECATION")
    private fun hdrTypes(context: Context): List<String> {
        val types = defaultDisplay(context).hdrCapabilities?.supportedHdrTypes ?: return emptyList()
        return types.toList().mapNotNull {
            when (it) {
                Display.HdrCapabilities.HDR_TYPE_HDR10 -> "hdr10"
                Display.HdrCapabilities.HDR_TYPE_HLG -> "hlg"
                Display.HdrCapabilities.HDR_TYPE_HDR10_PLUS -> "hdr10plus"
                Display.HdrCapabilities.HDR_TYPE_DOLBY_VISION -> "dolby_vision"
                else -> null
            }
        }.distinct()
    }

    @OptIn(UnstableApi::class)
    @Suppress("DEPRECATION")
    private fun audio(context: Context, all: List<MediaCodecInfo>): JSObject {
        val codecs = all.flatMap { it.supportedTypes.mapNotNull { t -> AUDIO[t.lowercase()] } }.toSortedSet()
        val passthrough = AudioCapabilities.getCapabilities(context).let { ac ->
            PASSTHROUGH.filterKeys { ac.supportsEncoding(it) }.values.toSortedSet()
        }
        // An empty channelCounts array means the device takes any count; stereo is the safe floor.
        val channels = context.getSystemService(AudioManager::class.java)
            .getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            .flatMap { d: AudioDeviceInfo -> d.channelCounts.toList() }
            .maxOrNull() ?: 2
        return JSObject().apply {
            put("codecs", JSONArray(codecs.toList()))
            put("max_channels", channels)
            put("passthrough", JSONArray(passthrough.toList()))
        }
    }
}
