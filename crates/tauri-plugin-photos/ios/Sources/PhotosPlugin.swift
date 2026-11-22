// Saves a downloaded photo or video into the Photos library ("Add to Photos" permission only).

import Photos
import SwiftRs
import Tauri
import UIKit
import WebKit

struct SaveArgs: Decodable {
  let path: String
  let video: Bool
}

class PhotosPlugin: Plugin {
  @objc public func save(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(SaveArgs.self)
    let url = URL(fileURLWithPath: args.path)
    let ask: (@escaping (PHAuthorizationStatus) -> Void) -> Void = { done in
      if #available(iOS 14, *) {
        PHPhotoLibrary.requestAuthorization(for: .addOnly, handler: done)
      } else {
        PHPhotoLibrary.requestAuthorization(done)
      }
    }
    ask { status in
      var allowed = status == .authorized
      if #available(iOS 14, *) { allowed = allowed || status == .limited }
      guard allowed else {
        invoke.reject("not allowed to add to Photos (Settings → AlwyChat → Photos)")
        return
      }
      PHPhotoLibrary.shared().performChanges({
        let options = PHAssetResourceCreationOptions()
        // The app's copy is temporary: Photos takes the file instead of copying it.
        options.shouldMoveFile = true
        PHAssetCreationRequest.forAsset().addResource(
          with: args.video ? .video : .photo, fileURL: url, options: options)
      }) { ok, error in
        if ok {
          invoke.resolve()
        } else {
          try? FileManager.default.removeItem(at: url)
          invoke.reject(error?.localizedDescription ?? "couldn't save to Photos")
        }
      }
    }
  }
}

@_cdecl("init_plugin_photos")
public func initPlugin() -> Plugin {
  return PhotosPlugin()
}
