// Saves a downloaded photo or video into the Photos library ("Add to Photos" permission only).
// Also the app's one native iOS plugin, so it hides the web view's keyboard bar (below).

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
  override func load(webview: WKWebView) {
    KeyboardBar.hide(in: webview)
  }

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

/// The ˄ ˅ ✓ bar Safari puts above the keyboard for forms: an app's composer doesn't want it
/// (it also left a gap above the keyboard). The web view's content view is given a subclass
/// whose `inputAccessoryView` is nil (what Capacitor's hideFormAccessoryBar does).
enum KeyboardBar {
  static func hide(in webview: WKWebView) {
    guard
      let content = webview.scrollView.subviews.first(where: {
        String(describing: type(of: $0)).hasPrefix("WKContent")
      })
    else { return }
    let base: AnyClass = type(of: content)
    let name = "\(NSStringFromClass(base))_AlwyNoBar"
    var cls: AnyClass? = NSClassFromString(name)
    if cls == nil, let sub = objc_allocateClassPair(base, name, 0) {
      let selector = #selector(getter: UIResponder.inputAccessoryView)
      let none: @convention(block) (AnyObject) -> UIView? = { _ in nil }
      if let method = class_getInstanceMethod(UIResponder.self, selector) {
        class_addMethod(sub, selector, imp_implementationWithBlock(none), method_getTypeEncoding(method))
      }
      objc_registerClassPair(sub)
      cls = sub
    }
    if let cls { object_setClass(content, cls) }
  }
}

@_cdecl("init_plugin_photos")
public func initPlugin() -> Plugin {
  return PhotosPlugin()
}
