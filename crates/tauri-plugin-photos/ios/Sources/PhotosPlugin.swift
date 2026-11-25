// Saves a downloaded photo or video into the Photos library ("Add to Photos" permission only).
// Also the app's one native iOS plugin, so it hides the web view's keyboard bar (below).

import Photos
import PhotosUI
import SwiftRs
import Tauri
import UIKit
import UniformTypeIdentifiers
import WebKit

struct SaveArgs: Decodable {
  let path: String
  let video: Bool
}

struct PickArgs: Decodable {
  /// "media": the Photos picker (photos and videos); anything else: the Files app.
  let kind: String
  /// Where the chosen files are copied (the app's `picked` folder).
  let dir: String
}

class PhotosPlugin: Plugin {
  /// The picker answering now (its delegate must stay alive while it's on screen).
  private var picking: FilePicker?
  private var keyboard: KeyboardFit?

  override func load(webview: WKWebView) {
    KeyboardBar.hide(in: webview)
    keyboard = KeyboardFit(webview: webview)
  }

  /// Opens the system's photo or file picker and copies what was chosen into `dir`:
  /// `{files: [{path, name, mime, size}]}` (empty when cancelled), like Android's picker.
  @objc public func pick(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PickArgs.self)
    let dir = URL(fileURLWithPath: args.dir, isDirectory: true)
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    DispatchQueue.main.async {
      guard let presenter = self.manager.viewController else {
        invoke.reject("no window to show the picker in")
        return
      }
      let picker = FilePicker(dir: dir) { files in
        self.picking = nil
        invoke.resolve(["files": files])
      }
      self.picking = picker
      if args.kind == "media" {
        var config = PHPickerConfiguration()
        config.selectionLimit = 0
        config.filter = .any(of: [.images, .videos])
        // JPEG / H.264 rather than HEIC / HEVC: every other app can show what we send.
        config.preferredAssetRepresentationMode = .compatible
        let controller = PHPickerViewController(configuration: config)
        controller.delegate = picker
        presenter.present(controller, animated: true)
      } else {
        let controller = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
        controller.allowsMultipleSelection = true
        controller.delegate = picker
        presenter.present(controller, animated: true)
      }
    }
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

/// Answers the photo and file pickers: copies each chosen item into `dir` under a unique name.
class FilePicker: NSObject, PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
  private let dir: URL
  private let done: ([[String: Any]]) -> Void

  init(dir: URL, done: @escaping ([[String: Any]]) -> Void) {
    self.dir = dir
    self.done = done
  }

  /// Copies (or moves, for the document picker's own copies) `url` into `dir`.
  private func keep(_ url: URL, name: String, move: Bool) -> [String: Any]? {
    let target = dir.appendingPathComponent("\(UUID().uuidString.prefix(8))-\(name)")
    do {
      if move {
        try FileManager.default.moveItem(at: url, to: target)
      } else {
        try FileManager.default.copyItem(at: url, to: target)
      }
    } catch {
      return nil
    }
    let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size] as? NSNumber)?.uint64Value ?? 0
    let mime = UTType(filenameExtension: target.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
    return ["path": target.path, "name": name, "mime": mime, "size": size]
  }

  func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
    picker.dismiss(animated: true)
    var files = [[String: Any]?](repeating: nil, count: results.count)
    let group = DispatchGroup()
    for (i, result) in results.enumerated() {
      let provider = result.itemProvider
      let type: UTType = provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) ? .movie : .image
      group.enter()
      // The file only exists during the callback: copied at once.
      provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, _ in
        if let url {
          let base = provider.suggestedName ?? (type == .movie ? "video" : "photo")
          let name = url.pathExtension.isEmpty ? base : "\(base).\(url.pathExtension)"
          files[i] = self.keep(url, name: name, move: false)
        }
        group.leave()
      }
    }
    group.notify(queue: .main) { self.done(files.compactMap { $0 }) }
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    done(urls.compactMap { keep($0, name: $0.lastPathComponent, move: true) })
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    done([])
  }
}

/// The keyboard shortens the web view (like Android and native apps) instead of iOS sliding the
/// whole page up under the status bar and back down when it closes: the page is laid out in the
/// space left above the keyboard, so the chat header never moves and the message bar sits right
/// on the keyboard (no home-bar gap: the web view no longer reaches the bottom edge).
final class KeyboardFit: NSObject {
  private weak var webview: WKWebView?

  init(webview: WKWebView) {
    self.webview = webview
    super.init()
    let center = NotificationCenter.default
    center.addObserver(
      self, selector: #selector(change(_:)),
      name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
    center.addObserver(
      self, selector: #selector(hide(_:)),
      name: UIResponder.keyboardWillHideNotification, object: nil)
  }

  deinit {
    NotificationCenter.default.removeObserver(self)
  }

  @objc private func change(_ note: Notification) {
    guard
      let webview, let host = webview.superview,
      let end = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
    else { return }
    // The keyboard's frame is in screen coordinates; what of it covers the host view.
    let keyboard = host.convert(end, from: nil)
    let covered = max(0, host.bounds.maxY - keyboard.minY)
    fit(webview, in: host, covered: covered)
  }

  @objc private func hide(_ note: Notification) {
    guard let webview, let host = webview.superview else { return }
    fit(webview, in: host, covered: 0)
  }

  private func fit(_ webview: WKWebView, in host: UIView, covered: CGFloat) {
    var frame = host.bounds
    frame.size.height -= covered
    if webview.frame != frame { webview.frame = frame }
    // Nothing left for iOS to slide into view.
    webview.scrollView.contentOffset = .zero
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
