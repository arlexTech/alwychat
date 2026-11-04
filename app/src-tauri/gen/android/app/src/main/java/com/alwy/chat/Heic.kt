package com.alwy.chat

import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.os.Build
import android.util.Log
import java.io.File

/**
 * HEIC/HEIF photos (the default camera format on many phones): the app's image processing and
 * the web views can't read them, so Android converts them to JPEG (its decoder applies the
 * photo's rotation). Used for photos we send, and for HEIC photos others sent us.
 */
object Heic {
  private val BRANDS = setOf("heic", "heix", "heim", "heis", "hevc", "hevx", "mif1", "msf1", "heif")

  /** HEIC/HEIF by content: an ISO-BMFF "ftyp" box with a HEIF brand (names and MIME types lie). */
  fun isHeic(file: File): Boolean = try {
    val head = ByteArray(12)
    file.inputStream().use { it.read(head) } == 12 &&
      String(head, 4, 4, Charsets.US_ASCII) == "ftyp" &&
      String(head, 8, 4, Charsets.US_ASCII) in BRANDS
  } catch (e: Exception) {
    false
  }

  /** Decode `src` and write it to `dst` as JPEG (longest side ≤ `maxSide` if > 0). */
  fun toJpeg(src: File, dst: File, maxSide: Int = 0, quality: Int = 92): Boolean {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) return false
    return try {
      val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(src)) { decoder, info, _ ->
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        val (w, h) = info.size.width to info.size.height
        val longest = maxOf(w, h)
        if (maxSide in 1 until longest) {
          decoder.setTargetSize(w * maxSide / longest, h * maxSide / longest)
        }
      }
      dst.outputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, quality, it) }
      bitmap.recycle()
      true
    } catch (e: Exception) {
      Log.w("AlwyChat", "HEIC to JPEG: $e")
      dst.delete()
      false
    }
  }
}
