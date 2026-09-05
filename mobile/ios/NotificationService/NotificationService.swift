import MajuPushKit
import Foundation
import Security
import UserNotifications

final class NotificationService: UNNotificationServiceExtension {
  private var contentHandler: ((UNNotificationContent) -> Void)?
  private var bestAttemptContent: UNMutableNotificationContent?
  private let communicationPresenter = MajuCommunicationNotificationPresenter()
  private lazy var resolver: MajuPushNotificationResolving = {
    let appGroupIdentifier =
      Bundle.main.object(
        forInfoDictionaryKey: "MajuAppGroupIdentifier"
      ) as? String
    let keychainAccessGroup =
      Bundle.main.object(
        forInfoDictionaryKey: "MajuKeychainAccessGroup"
      ) as? String
    return MajuPushNotificationResolver(
      session: .shared,
      loadCommunitiesData: {
        Self.loadPushSnapshotData(appGroupIdentifier: appGroupIdentifier)
      },
      loadPrivateKey: { communityID in
        Self.loadPrivateKey(
          communityID: communityID,
          keychainAccessGroup: keychainAccessGroup
        )
      },
      loadPresentationCacheData: {
        Self.loadPushSnapshotData(appGroupIdentifier: appGroupIdentifier)
      }
    )
  }()

  override func didReceive(
    _ request: UNNotificationRequest,
    withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void
  ) {
    self.contentHandler = contentHandler
    guard let content = request.content.mutableCopy() as? UNMutableNotificationContent else {
      contentHandler(request.content)
      return
    }
    bestAttemptContent = content
    var cleanUserInfo = content.userInfo
    cleanUserInfo.removeValue(forKey: MajuPushNavigationTarget.userInfoKey)
    content.userInfo = cleanUserInfo

    resolver.resolve { [weak self] resolution in
      guard let self else { return }
      if let resolution {
        content.title = resolution.title
        content.body = resolution.body
        if let subtitle = resolution.subtitle {
          content.subtitle = subtitle
        }
        if let threadIdentifier = resolution.threadIdentifier {
          content.threadIdentifier = threadIdentifier
        }
        if let navigationTarget = resolution.navigationTarget {
          var userInfo = content.userInfo
          userInfo[MajuPushNavigationTarget.userInfoKey] = navigationTarget.userInfoValue
          content.userInfo = userInfo
        }
        self.bestAttemptContent = content
        self.communicationPresenter.present(
          ordinaryContent: content,
          resolution: resolution
        ) { [weak self] specializedContent in
          self?.finish(specializedContent)
        }
        return
      }
      self.finish(content)
    }
  }

  override func serviceExtensionTimeWillExpire() {
    if let bestAttemptContent {
      finish(bestAttemptContent)
    }
  }

  private func finish(_ content: UNNotificationContent) {
    guard let contentHandler else { return }
    self.contentHandler = nil
    contentHandler(content)
  }

  private static func loadPrivateKey(
    communityID: String,
    keychainAccessGroup: String?
  ) -> String? {
    var query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "maju.push.nse.signing",
      kSecAttrAccount as String: communityID,
      kSecReturnData as String: true,
      kSecMatchLimit as String: kSecMatchLimitOne,
    ]
    if let keychainAccessGroup, !keychainAccessGroup.isEmpty {
      query[kSecAttrAccessGroup as String] = keychainAccessGroup
    }
    var item: CFTypeRef?
    guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
      let data = item as? Data
    else { return nil }
    return String(data: data, encoding: .utf8)
  }

  private static func loadPushSnapshotData(appGroupIdentifier: String?) -> Data? {
    loadAppGroupData(
      fileName: MajuPushPresentationCacheStore.fileName,
      appGroupIdentifier: appGroupIdentifier,
      maximumBytes: MajuPushPresentationCacheStore.maximumSnapshotBytes
    )
  }

  private static func loadAppGroupData(
    fileName: String,
    appGroupIdentifier: String?,
    maximumBytes: Int? = nil
  ) -> Data? {
    guard let appGroupIdentifier,
      let container = FileManager.default.containerURL(
        forSecurityApplicationGroupIdentifier: appGroupIdentifier
      )
    else { return nil }
    let fileURL = container.appendingPathComponent(fileName)
    if let maximumBytes {
      guard let values = try? fileURL.resourceValues(forKeys: [.fileSizeKey]),
        let fileSize = values.fileSize,
        fileSize <= maximumBytes
      else { return nil }
    }
    return try? Data(contentsOf: fileURL)
  }
}
